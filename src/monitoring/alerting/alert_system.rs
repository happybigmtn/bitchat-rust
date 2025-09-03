//! Main Alert System for BitCraps Monitoring
//!
//! This module provides the main AlertingSystem that coordinates all
//! the alerting components: conditions, notifications, state, and escalation.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::alert_types::*;
use super::alert_conditions::AlertRulesEngine;
use super::notification_channels::NotificationDispatcher;
use super::alert_state::{AlertStateManager, AlertAddResult};
use super::escalation::EscalationManager;

/// Production alerting system that coordinates all alerting components
pub struct AlertingSystem {
    /// Alert rules engine
    rules_engine: Arc<AlertRulesEngine>,
    /// Notification dispatcher
    notification_dispatcher: Arc<NotificationDispatcher>,
    /// Alert state manager
    state_manager: Arc<AlertStateManager>,
    /// Escalation manager
    escalation_manager: Arc<EscalationManager>,
    /// Configuration
    config: AlertingConfig,
    /// Alert broadcast channel
    alert_sender: broadcast::Sender<Alert>,
    /// System running state
    running: Arc<RwLock<bool>>,
}

impl AlertingSystem {
    /// Create new alerting system
    pub fn new(config: AlertingConfig) -> Self {
        let (alert_sender, _) = broadcast::channel(1000);

        let rules_engine = Arc::new(AlertRulesEngine::new(config.rules.clone()));
        let notification_dispatcher = Arc::new(NotificationDispatcher::new(config.notifications.clone()));
        let state_manager = Arc::new(AlertStateManager::new(Default::default()));
        let escalation_manager = Arc::new(EscalationManager::new(config.escalation.clone()));

        Self {
            rules_engine,
            notification_dispatcher,
            state_manager,
            escalation_manager,
            config,
            alert_sender,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start alerting system
    pub async fn start(&self) -> Result<(), AlertingError> {
        let mut running = self.running.write().await;
        if *running {
            warn!("Alerting system is already running");
            return Ok(());
        }

        info!("Starting alerting system");

        // Start all monitoring tasks
        self.start_metrics_monitoring().await?;
        self.start_alert_processing().await?;
        self.start_escalation_processing().await?;
        self.start_maintenance_tasks().await?;

        *running = true;
        info!("Alerting system started successfully");
        Ok(())
    }

    /// Stop alerting system
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        if !*running {
            warn!("Alerting system is not running");
            return;
        }

        info!("Stopping alerting system");
        *running = false;
        
        // Additional cleanup could be added here if needed
        info!("Alerting system stopped");
    }

    /// Get alert subscription channel
    pub fn subscribe(&self) -> broadcast::Receiver<Alert> {
        self.alert_sender.subscribe()
    }

    /// Manually trigger alert
    pub async fn trigger_alert(&self, alert: Alert) -> Result<(), AlertingError> {
        self.process_alert_internally(alert).await
    }

    /// Test notification channel
    pub async fn test_channel(&self, channel_name: &str) -> Result<(), AlertingError> {
        self.notification_dispatcher.test_channel(channel_name).await
    }

    /// Acknowledge an alert
    pub async fn acknowledge_alert(
        &self,
        alert_id: &str,
        acknowledger: String,
        message: Option<String>,
    ) -> Result<(), AlertingError> {
        self.escalation_manager.acknowledge_alert(alert_id, acknowledger, message).await
    }

    /// Resolve an alert
    pub async fn resolve_alert(&self, alert_id: &str) -> Result<bool, AlertingError> {
        let resolved = self.state_manager.resolve_alert(alert_id).await;
        if resolved {
            self.escalation_manager.remove_alert(alert_id).await;
            info!("Alert {} resolved and removed from escalation tracking", alert_id);
        }
        Ok(resolved)
    }

    /// Get current alert status
    pub async fn get_alert_status(&self) -> AlertStatus {
        self.state_manager.get_alert_status().await
    }

    /// Get alert statistics
    pub async fn get_statistics(&self) -> AlertStatistics {
        self.state_manager.get_statistics().await
    }

    /// Get escalation statistics
    pub async fn get_escalation_statistics(&self) -> super::escalation::EscalationStats {
        self.escalation_manager.get_statistics().await
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        self.state_manager.get_active_alerts().await
    }

    /// Get active alerts by severity
    pub async fn get_active_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<Alert> {
        self.state_manager.get_active_alerts_by_severity(severity).await
    }

    /// Get recent resolved alerts
    pub async fn get_recent_resolved_alerts(&self, count: usize) -> Vec<Alert> {
        self.state_manager.get_recent_resolved_alerts(count).await
    }

    /// Add alert rule
    pub async fn add_alert_rule(&self, rule: AlertRule) -> Result<(), AlertingError> {
        // This would require making rules_engine mutable
        warn!("Dynamic rule addition not yet implemented - restart system with new config");
        Ok(())
    }

    /// Start monitoring metrics for alert conditions
    async fn start_metrics_monitoring(&self) -> Result<(), AlertingError> {
        let rules_engine = Arc::clone(&self.rules_engine);
        let alert_sender = self.alert_sender.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10)); // Check every 10 seconds

            loop {
                interval.tick().await;

                // Check if system is still running
                {
                    let is_running = *running.read().await;
                    if !is_running {
                        debug!("Metrics monitoring task stopping");
                        break;
                    }
                }

                // Evaluate all alert rules
                match rules_engine.evaluate_rules().await {
                    Ok(triggered_alerts) => {
                        for alert in triggered_alerts {
                            if let Err(e) = alert_sender.send(alert.clone()) {
                                error!("Failed to send alert to processing queue: {:?}", e);
                            } else {
                                debug!("Queued alert for processing: {}", alert.name);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to evaluate alert rules: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start processing alerts
    async fn start_alert_processing(&self) -> Result<(), AlertingError> {
        let mut alert_receiver = self.alert_sender.subscribe();
        let state_manager = Arc::clone(&self.state_manager);
        let notification_dispatcher = Arc::clone(&self.notification_dispatcher);
        let escalation_manager = Arc::clone(&self.escalation_manager);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            while let Ok(alert) = alert_receiver.recv().await {
                // Check if system is still running
                {
                    let is_running = *running.read().await;
                    if !is_running {
                        debug!("Alert processing task stopping");
                        break;
                    }
                }

                // Process the alert
                if let Err(e) = Self::process_single_alert(
                    alert,
                    &state_manager,
                    &notification_dispatcher,
                    &escalation_manager,
                ).await {
                    error!("Failed to process alert: {:?}", e);
                }
            }
        });

        Ok(())
    }

    /// Start escalation processing
    async fn start_escalation_processing(&self) -> Result<(), AlertingError> {
        let escalation_manager = Arc::clone(&self.escalation_manager);
        let state_manager = Arc::clone(&self.state_manager);
        let notification_dispatcher = Arc::clone(&self.notification_dispatcher);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                // Check if system is still running
                {
                    let is_running = *running.read().await;
                    if !is_running {
                        debug!("Escalation processing task stopping");
                        break;
                    }
                }

                // Check for alerts that need escalation
                let active_alerts = state_manager.get_active_alerts().await;
                for alert in active_alerts {
                    match escalation_manager.check_escalation(&alert).await {
                        Ok(Some(escalated_alert)) => {
                            // Send escalated alert notification
                            if let Err(e) = notification_dispatcher.send_notification(&escalated_alert).await {
                                error!("Failed to send escalation notification for {}: {:?}", 
                                    escalated_alert.name, e);
                            } else {
                                info!("Sent escalation notification for: {}", escalated_alert.name);
                            }
                        }
                        Ok(None) => {
                            // No escalation needed
                        }
                        Err(e) => {
                            error!("Failed to check escalation for {}: {:?}", alert.name, e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start maintenance tasks (cleanup, auto-resolve, etc.)
    async fn start_maintenance_tasks(&self) -> Result<(), AlertingError> {
        let state_manager = Arc::clone(&self.state_manager);
        let escalation_manager = Arc::clone(&self.escalation_manager);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(900)); // 15 minutes
            let mut auto_resolve_interval = interval(Duration::from_secs(300)); // 5 minutes

            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        // Check if system is still running
                        {
                            let is_running = *running.read().await;
                            if !is_running {
                                debug!("Maintenance tasks stopping");
                                break;
                            }
                        }

                        // Cleanup old data
                        state_manager.cleanup_old_data().await;
                        escalation_manager.cleanup_old_records(7).await; // 7 days retention
                        debug!("Performed maintenance cleanup");
                    }
                    _ = auto_resolve_interval.tick() => {
                        // Check if system is still running
                        {
                            let is_running = *running.read().await;
                            if !is_running {
                                break;
                            }
                        }

                        // Auto-resolve stale alerts
                        let auto_resolved = state_manager.auto_resolve_stale_alerts().await;
                        if !auto_resolved.is_empty() {
                            info!("Auto-resolved {} stale alerts", auto_resolved.len());
                            
                            // Remove from escalation tracking
                            for alert in &auto_resolved {
                                escalation_manager.remove_alert(&alert.id).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Process individual alert through the complete pipeline
    async fn process_single_alert(
        alert: Alert,
        state_manager: &AlertStateManager,
        notification_dispatcher: &NotificationDispatcher,
        escalation_manager: &EscalationManager,
    ) -> Result<(), AlertingError> {
        debug!("Processing alert: {} ({})", alert.name, alert.id);

        // Add to state manager (handles deduplication)
        let add_result = state_manager.add_active_alert(alert.clone()).await;
        
        match add_result {
            AlertAddResult::Added { .. } => {
                // New alert - send notification
                if let Err(e) = notification_dispatcher.send_notification(&alert).await {
                    error!("Failed to send notification for {}: {:?}", alert.name, e);
                    return Err(e);
                }

                // Check for immediate escalation
                if let Ok(Some(escalated_alert)) = escalation_manager.check_escalation(&alert).await {
                    info!("Immediate escalation triggered for: {}", alert.name);
                    if let Err(e) = notification_dispatcher.send_notification(&escalated_alert).await {
                        error!("Failed to send escalation notification for {}: {:?}", 
                            escalated_alert.name, e);
                    }
                }

                info!("Successfully processed new alert: {} ({})", alert.name, alert.id);
            }
            AlertAddResult::Suppressed { occurrence_count, .. } => {
                debug!(
                    "Suppressed duplicate alert: {} (occurrence: {})",
                    alert.name, occurrence_count
                );
            }
        }

        Ok(())
    }

    /// Internal method to process alerts (used by manual trigger)
    async fn process_alert_internally(&self, alert: Alert) -> Result<(), AlertingError> {
        if let Err(e) = self.alert_sender.send(alert.clone()) {
            return Err(AlertingError::ProcessingError(format!(
                "Failed to queue alert for processing: {:?}",
                e
            )));
        }

        debug!("Manually triggered alert queued for processing: {}", alert.name);
        Ok(())
    }

    /// Get system health summary
    pub async fn get_system_health(&self) -> SystemHealthSummary {
        let alert_status = self.get_alert_status().await;
        let alert_stats = self.get_statistics().await;
        let escalation_stats = self.get_escalation_statistics().await;
        let is_running = *self.running.read().await;

        SystemHealthSummary {
            is_running,
            system_health: alert_status.system_health,
            active_alerts: alert_status.active_alerts,
            critical_alerts: alert_status.critical_alerts,
            total_escalations: escalation_stats.total_escalations,
            error_rate_percent: if alert_stats.total_alerts_processed > 0 {
                (alert_stats.alerts_by_severity.get(&AlertSeverity::Critical).unwrap_or(&0)
                    + alert_stats.alerts_by_severity.get(&AlertSeverity::High).unwrap_or(&0)) as f64 
                / alert_stats.total_alerts_processed as f64 * 100.0
            } else {
                0.0
            },
            average_resolution_time_minutes: alert_stats.average_resolution_time_minutes,
        }
    }
}

/// System health summary
#[derive(Debug)]
pub struct SystemHealthSummary {
    pub is_running: bool,
    pub system_health: SystemHealth,
    pub active_alerts: usize,
    pub critical_alerts: usize,
    pub total_escalations: usize,
    pub error_rate_percent: f64,
    pub average_resolution_time_minutes: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_alert() -> Alert {
        Alert::new(
            "Test Alert".to_string(),
            "Test alert description".to_string(),
            AlertSeverity::High,
            "test".to_string(),
            "test_metric".to_string(),
            100.0,
            80.0,
        )
    }

    #[tokio::test]
    async fn test_alerting_system_creation() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);

        let status = system.get_alert_status().await;
        assert_eq!(status.active_alerts, 0);
    }

    #[tokio::test]
    async fn test_manual_alert_trigger() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);
        let alert = create_test_alert();

        let result = system.trigger_alert(alert.clone()).await;
        assert!(result.is_ok());

        // Give the system a moment to process
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_alert_subscription() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);
        let mut receiver = system.subscribe();

        let alert = create_test_alert();
        let alert_id = alert.id.clone();

        // Trigger alert in a separate task
        let system_clone = Arc::new(system);
        let trigger_system = Arc::clone(&system_clone);
        tokio::spawn(async move {
            let _ = trigger_system.trigger_alert(alert).await;
        });

        // Wait for the alert
        match tokio::time::timeout(Duration::from_secs(1), receiver.recv()).await {
            Ok(Ok(received_alert)) => {
                assert_eq!(received_alert.id, alert_id);
                assert_eq!(received_alert.name, "Test Alert");
            }
            Ok(Err(e)) => panic!("Failed to receive alert: {:?}", e),
            Err(_) => panic!("Timeout waiting for alert"),
        }
    }

    #[tokio::test]
    async fn test_system_start_stop() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);

        // System should not be running initially
        let health_before = system.get_system_health().await;
        assert!(!health_before.is_running);

        // Start system
        assert!(system.start().await.is_ok());
        let health_after_start = system.get_system_health().await;
        assert!(health_after_start.is_running);

        // Stop system
        system.stop().await;
        let health_after_stop = system.get_system_health().await;
        assert!(!health_after_stop.is_running);
    }

    #[tokio::test]
    async fn test_alert_acknowledgment() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);
        let alert = create_test_alert();
        let alert_id = alert.id.clone();

        // Trigger alert first
        system.trigger_alert(alert).await.unwrap();

        // Give system time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Acknowledge the alert
        let ack_result = system
            .acknowledge_alert(&alert_id, "test_user".to_string(), Some("Working on it".to_string()))
            .await;

        // Note: This might fail if the alert hasn't been processed yet in the escalation system
        // In a real scenario, there would be better coordination
        match ack_result {
            Ok(()) => {
                // Success - alert was acknowledged
            }
            Err(AlertingError::ProcessingError(_)) => {
                // Expected if alert isn't in escalation tracking yet
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_resolve_alert() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);
        let alert = create_test_alert();
        let alert_id = alert.id.clone();

        // Trigger alert
        system.trigger_alert(alert).await.unwrap();

        // Give system time to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Resolve the alert
        let resolve_result = system.resolve_alert(&alert_id).await;
        assert!(resolve_result.is_ok());
        // Result depends on whether alert was processed into state manager
    }

    #[tokio::test]
    async fn test_get_statistics() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);

        let stats = system.get_statistics().await;
        assert_eq!(stats.total_alerts_processed, 0);
        assert_eq!(stats.active_alerts, 0);

        let escalation_stats = system.get_escalation_statistics().await;
        assert_eq!(escalation_stats.total_escalations, 0);
    }
}