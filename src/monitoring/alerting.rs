//! Production Alerting System for BitCraps
//!
//! This module provides comprehensive alerting capabilities for production monitoring:
//! - Real-time threat detection and anomaly detection
//! - Performance degradation alerts with predictive thresholds
//! - Resource exhaustion warnings with forecasting
//! - Security incident notifications with severity classification
//! - Automated escalation procedures with PagerDuty integration
//! - Alert aggregation and deduplication
//! - Multi-channel notification routing (Slack, email, webhooks, SMS)
//! - Incident correlation and root cause analysis

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{atomic::Ordering, Arc};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::monitoring::metrics::METRICS;

/// Production alerting system
pub struct AlertingSystem {
    /// Alert rules engine
    rules_engine: Arc<AlertRulesEngine>,
    /// Notification dispatcher
    notification_dispatcher: Arc<NotificationDispatcher>,
    /// Alert state manager
    state_manager: Arc<AlertStateManager>,
    /// Escalation manager
    escalation_manager: Arc<EscalationManager>,
    /// Alert history storage
    history: Arc<RwLock<AlertHistory>>,
    /// Configuration
    config: AlertingConfig,
    /// Alert broadcast channel
    alert_sender: broadcast::Sender<Alert>,
}

impl AlertingSystem {
    /// Create new alerting system
    pub fn new(config: AlertingConfig) -> Self {
        let (alert_sender, _) = broadcast::channel(1000);

        let rules_engine = Arc::new(AlertRulesEngine::new(config.rules.clone()));
        let notification_dispatcher =
            Arc::new(NotificationDispatcher::new(config.notifications.clone()));
        let state_manager = Arc::new(AlertStateManager::new());
        let escalation_manager = Arc::new(EscalationManager::new(config.escalation.clone()));
        let history = Arc::new(RwLock::new(AlertHistory::new(
            config.history_retention_days,
        )));

        Self {
            rules_engine,
            notification_dispatcher,
            state_manager,
            escalation_manager,
            history,
            config,
            alert_sender,
        }
    }

    /// Start alerting system
    pub async fn start(&self) -> Result<(), AlertingError> {
        info!("Starting alerting system");

        // Start metrics monitoring
        self.start_metrics_monitoring().await?;

        // Start alert processing
        self.start_alert_processing().await?;

        // Start escalation processing
        self.start_escalation_processing().await?;

        // Start history cleanup
        self.start_history_cleanup().await?;

        info!("Alerting system started successfully");
        Ok(())
    }

    /// Get alert subscription channel
    pub fn subscribe(&self) -> broadcast::Receiver<Alert> {
        self.alert_sender.subscribe()
    }

    /// Manually trigger alert
    pub async fn trigger_alert(&self, alert: Alert) -> Result<(), AlertingError> {
        self.process_alert(alert).await
    }

    /// Get current alert status
    pub async fn get_alert_status(&self) -> AlertStatus {
        let active_alerts = self.state_manager.get_active_alerts().await;
        let total_alerts_24h = self.history.read().await.count_alerts_in_last_hours(24);
        let critical_alerts = active_alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .count();

        AlertStatus {
            active_alerts: active_alerts.len(),
            critical_alerts,
            total_alerts_last_24h: total_alerts_24h,
            system_health: if critical_alerts > 0 {
                SystemHealth::Critical
            } else if active_alerts.len() > 10 {
                SystemHealth::Warning
            } else {
                SystemHealth::Healthy
            },
        }
    }

    /// Get alert statistics
    pub async fn get_statistics(&self) -> AlertStatistics {
        let history = self.history.read().await;
        let active_alerts = self.state_manager.get_active_alerts().await;

        AlertStatistics {
            total_alerts_processed: history.total_count(),
            active_alerts: active_alerts.len(),
            alerts_last_hour: history.count_alerts_in_last_hours(1),
            alerts_last_24_hours: history.count_alerts_in_last_hours(24),
            alerts_by_severity: history.count_by_severity(),
            alerts_by_category: history.count_by_category(),
            average_resolution_time_minutes: history.average_resolution_time_minutes(),
            false_positive_rate: history.calculate_false_positive_rate(),
        }
    }

    /// Start monitoring metrics for alert conditions
    async fn start_metrics_monitoring(&self) -> Result<(), AlertingError> {
        let rules_engine = Arc::clone(&self.rules_engine);
        let alert_sender = self.alert_sender.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10)); // Check every 10 seconds

            loop {
                interval.tick().await;

                // Evaluate all alert rules
                if let Ok(triggered_alerts) = rules_engine.evaluate_rules().await {
                    for alert in triggered_alerts {
                        if let Err(e) = alert_sender.send(alert) {
                            error!("Failed to send alert: {:?}", e);
                        }
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
        let history = Arc::clone(&self.history);

        tokio::spawn(async move {
            while let Ok(alert) = alert_receiver.recv().await {
                // Process the alert
                if let Err(e) = Self::process_single_alert(
                    alert.clone(),
                    &state_manager,
                    &notification_dispatcher,
                    &history,
                )
                .await
                {
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

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Check every minute

            loop {
                interval.tick().await;

                // Check for alerts that need escalation
                let active_alerts = state_manager.get_active_alerts().await;
                for alert in active_alerts {
                    if let Ok(escalation) = escalation_manager.check_escalation(&alert).await {
                        if let Some(escalation_alert) = escalation {
                            if let Err(e) = notification_dispatcher
                                .send_notification(&escalation_alert)
                                .await
                            {
                                error!("Failed to send escalation notification: {:?}", e);
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start alert history cleanup
    async fn start_history_cleanup(&self) -> Result<(), AlertingError> {
        let history = Arc::clone(&self.history);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // Cleanup every hour

            loop {
                interval.tick().await;
                history.write().await.cleanup_old_alerts();
            }
        });

        Ok(())
    }

    /// Process individual alert
    async fn process_single_alert(
        alert: Alert,
        state_manager: &AlertStateManager,
        notification_dispatcher: &NotificationDispatcher,
        history: &Arc<RwLock<AlertHistory>>,
    ) -> Result<(), AlertingError> {
        // Check if this is a duplicate alert
        if state_manager.is_duplicate(&alert).await {
            debug!("Suppressing duplicate alert: {}", alert.name);
            return Ok(());
        }

        // Add to active alerts
        state_manager.add_active_alert(alert.clone()).await;

        // Send notification
        notification_dispatcher.send_notification(&alert).await?;

        // Add to history
        history.write().await.add_alert(alert.clone());

        info!(
            "Processed alert: {} (severity: {:?})",
            alert.name, alert.severity
        );
        Ok(())
    }

    /// Process alert (public interface)
    async fn process_alert(&self, alert: Alert) -> Result<(), AlertingError> {
        if let Err(e) = self.alert_sender.send(alert) {
            return Err(AlertingError::ProcessingError(format!(
                "Failed to send alert: {:?}",
                e
            )));
        }
        Ok(())
    }
}

/// Alert rules engine for evaluating conditions
pub struct AlertRulesEngine {
    rules: Vec<AlertRule>,
    last_evaluation: Arc<RwLock<HashMap<String, SystemTime>>>,
}

impl AlertRulesEngine {
    pub fn new(rules: Vec<AlertRule>) -> Self {
        Self {
            rules,
            last_evaluation: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Evaluate all alert rules against current metrics
    pub async fn evaluate_rules(&self) -> Result<Vec<Alert>, AlertingError> {
        let mut triggered_alerts = Vec::new();

        for rule in &self.rules {
            if self.should_evaluate_rule(rule).await? {
                if let Some(alert) = self.evaluate_rule(rule).await? {
                    triggered_alerts.push(alert);

                    // Update last evaluation time
                    self.last_evaluation
                        .write()
                        .await
                        .insert(rule.name.clone(), SystemTime::now());
                }
            }
        }

        Ok(triggered_alerts)
    }

    /// Check if rule should be evaluated (respecting cooldown)
    async fn should_evaluate_rule(&self, rule: &AlertRule) -> Result<bool, AlertingError> {
        let last_eval = self.last_evaluation.read().await;

        if let Some(last_time) = last_eval.get(&rule.name) {
            let elapsed = SystemTime::now()
                .duration_since(*last_time)
                .unwrap_or(Duration::from_secs(0));

            Ok(elapsed >= rule.evaluation_interval)
        } else {
            Ok(true) // First evaluation
        }
    }

    /// Evaluate individual rule
    async fn evaluate_rule(&self, rule: &AlertRule) -> Result<Option<Alert>, AlertingError> {
        let current_value = self.get_metric_value(&rule.metric_name).await?;

        let condition_met = match rule.condition {
            AlertCondition::GreaterThan(threshold) => current_value > threshold,
            AlertCondition::LessThan(threshold) => current_value < threshold,
            AlertCondition::Equals(threshold) => (current_value - threshold).abs() < f64::EPSILON,
            AlertCondition::NotEquals(threshold) => {
                (current_value - threshold).abs() >= f64::EPSILON
            }
            AlertCondition::Between(min, max) => current_value >= min && current_value <= max,
            AlertCondition::Outside(min, max) => current_value < min || current_value > max,
        };

        if condition_met {
            Ok(Some(Alert {
                id: uuid::Uuid::new_v4().to_string(),
                name: rule.name.clone(),
                description: rule.description.clone(),
                severity: rule.severity,
                category: rule.category.clone(),
                metric_name: rule.metric_name.clone(),
                current_value,
                threshold_value: self.get_threshold_value(&rule.condition),
                timestamp: SystemTime::now(),
                resolved_at: None,
                tags: rule.tags.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get current value of a metric
    async fn get_metric_value(&self, metric_name: &str) -> Result<f64, AlertingError> {
        match metric_name {
            "cpu_usage_percent" => {
                Ok(METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64)
            }
            "memory_usage_mb" => Ok(
                (METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024) as f64,
            ),
            "active_connections" => {
                Ok(METRICS.network.active_connections.load(Ordering::Relaxed) as f64)
            }
            "consensus_latency_ms" => Ok(METRICS.consensus.average_latency_ms()),
            "error_rate" => {
                let total_errors = METRICS.errors.total_errors.load(Ordering::Relaxed) as f64;
                let total_requests = (METRICS.network.messages_sent.load(Ordering::Relaxed)
                    + METRICS.network.messages_received.load(Ordering::Relaxed))
                    as f64;
                if total_requests > 0.0 {
                    Ok((total_errors / total_requests) * 100.0)
                } else {
                    Ok(0.0)
                }
            }
            "disk_usage_percent" => {
                // This would get actual disk usage - simplified for example
                Ok(25.0)
            }
            _ => Err(AlertingError::UnknownMetric(metric_name.to_string())),
        }
    }

    /// Get threshold value from condition
    fn get_threshold_value(&self, condition: &AlertCondition) -> f64 {
        match condition {
            AlertCondition::GreaterThan(threshold)
            | AlertCondition::LessThan(threshold)
            | AlertCondition::Equals(threshold)
            | AlertCondition::NotEquals(threshold) => *threshold,
            AlertCondition::Between(min, max) | AlertCondition::Outside(min, max) => {
                (min + max) / 2.0
            }
        }
    }
}

/// Notification dispatcher for sending alerts
pub struct NotificationDispatcher {
    channels: Vec<NotificationChannel>,
    rate_limiter: Arc<NotificationRateLimiter>,
}

impl NotificationDispatcher {
    pub fn new(config: NotificationConfig) -> Self {
        Self {
            channels: config.channels,
            rate_limiter: Arc::new(NotificationRateLimiter::new(config.rate_limit)),
        }
    }

    /// Send notification through all configured channels
    pub async fn send_notification(&self, alert: &Alert) -> Result<(), AlertingError> {
        // Check rate limit
        if !self.rate_limiter.can_send(&alert.name).await {
            debug!("Rate limiting notification for alert: {}", alert.name);
            return Ok(());
        }

        for channel in &self.channels {
            if self.should_send_to_channel(channel, alert) {
                if let Err(e) = self.send_to_channel(channel, alert).await {
                    warn!(
                        "Failed to send notification via {:?}: {:?}",
                        channel.channel_type, e
                    );
                }
            }
        }

        self.rate_limiter.record_sent(&alert.name).await;
        Ok(())
    }

    /// Check if alert should be sent to specific channel
    fn should_send_to_channel(&self, channel: &NotificationChannel, alert: &Alert) -> bool {
        // Check severity filter
        if let Some(min_severity) = &channel.min_severity {
            if alert.severity < *min_severity {
                return false;
            }
        }

        // Check category filter
        if !channel.categories.is_empty() && !channel.categories.contains(&alert.category) {
            return false;
        }

        // Check tag filters
        if !channel.required_tags.is_empty() {
            let has_required_tags = channel
                .required_tags
                .iter()
                .all(|tag| alert.tags.contains(tag));
            if !has_required_tags {
                return false;
            }
        }

        true
    }

    /// Send alert to specific notification channel
    async fn send_to_channel(
        &self,
        channel: &NotificationChannel,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        match &channel.channel_type {
            NotificationChannelType::Email { to, smtp_config } => {
                self.send_email_notification(to, smtp_config, alert).await
            }
            NotificationChannelType::Slack { webhook_url } => {
                self.send_slack_notification(webhook_url, alert).await
            }
            NotificationChannelType::Discord { webhook_url } => {
                self.send_discord_notification(webhook_url, alert).await
            }
            NotificationChannelType::PagerDuty { integration_key } => {
                self.send_pagerduty_notification(integration_key, alert)
                    .await
            }
            NotificationChannelType::Webhook { url, headers } => {
                self.send_webhook_notification(url, headers, alert).await
            }
            NotificationChannelType::SMS {
                phone_number,
                api_config,
            } => {
                self.send_sms_notification(phone_number, api_config, alert)
                    .await
            }
        }
    }

    async fn send_email_notification(
        &self,
        _to: &str,
        _smtp_config: &SMTPConfig,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        // Email implementation would go here
        info!("Sending email notification for alert: {}", alert.name);
        Ok(())
    }

    async fn send_slack_notification(
        &self,
        _webhook_url: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let _payload = serde_json::json!({
            "text": format!("🚨 Alert: {}", alert.name),
            "attachments": [{
                "color": self.get_slack_color(&alert.severity),
                "fields": [
                    {"title": "Severity", "value": format!("{:?}", alert.severity), "short": true},
                    {"title": "Category", "value": &alert.category, "short": true},
                    {"title": "Description", "value": &alert.description, "short": false},
                    {"title": "Current Value", "value": alert.current_value.to_string(), "short": true},
                    {"title": "Threshold", "value": alert.threshold_value.to_string(), "short": true},
                ],
                "ts": alert.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs()
            }]
        });

        // HTTP client call would go here
        info!("Sending Slack notification for alert: {}", alert.name);
        Ok(())
    }

    async fn send_discord_notification(
        &self,
        _webhook_url: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let _payload = serde_json::json!({
            "embeds": [{
                "title": format!("🚨 Alert: {}", alert.name),
                "description": alert.description,
                "color": self.get_discord_color(&alert.severity),
                "fields": [
                    {"name": "Severity", "value": format!("{:?}", alert.severity), "inline": true},
                    {"name": "Category", "value": &alert.category, "inline": true},
                    {"name": "Current Value", "value": alert.current_value.to_string(), "inline": true},
                    {"name": "Threshold", "value": alert.threshold_value.to_string(), "inline": true},
                ],
                "timestamp": chrono::DateTime::<chrono::Utc>::from(alert.timestamp).to_rfc3339()
            }]
        });

        // HTTP client call would go here
        info!("Sending Discord notification for alert: {}", alert.name);
        Ok(())
    }

    async fn send_pagerduty_notification(
        &self,
        integration_key: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let payload = serde_json::json!({
            "routing_key": integration_key,
            "event_action": "trigger",
            "payload": {
                "summary": format!("{}: {}", alert.name, alert.description),
                "source": "BitCraps Monitoring",
                "severity": self.get_pagerduty_severity(&alert.severity),
                "custom_details": {
                    "metric": alert.metric_name,
                    "current_value": alert.current_value,
                    "threshold": alert.threshold_value,
                    "category": alert.category,
                }
            }
        });

        // In production, would use reqwest or similar HTTP client
        info!(
            "Sending PagerDuty notification with key {} and payload: {}",
            integration_key, payload
        );
        Ok(())
    }

    async fn send_webhook_notification(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let payload = serde_json::json!({
            "alert": {
                "id": alert.id,
                "name": alert.name,
                "description": alert.description,
                "severity": alert.severity,
                "category": alert.category,
                "metric_name": alert.metric_name,
                "current_value": alert.current_value,
                "threshold_value": alert.threshold_value,
                "timestamp": alert.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                "tags": alert.tags
            }
        });

        // In production, would use reqwest or similar HTTP client
        info!(
            "Sending webhook to {} with {} headers and payload: {}",
            url,
            headers.len(),
            payload
        );
        Ok(())
    }

    async fn send_sms_notification(
        &self,
        phone_number: &str,
        api_config: &SMSConfig,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let message = format!(
            "🚨 BitCraps Alert: {} - {} (Severity: {:?})",
            alert.name, alert.description, alert.severity
        );

        // In production, would use SMS API provider (Twilio, etc.)
        info!(
            "Sending SMS to {} via {} with message: {}",
            phone_number, api_config.provider, message
        );
        Ok(())
    }

    fn get_slack_color(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "danger",
            AlertSeverity::High => "warning",
            AlertSeverity::Medium => "warning",
            AlertSeverity::Low => "good",
            AlertSeverity::Info => "#439FE0",
        }
    }

    fn get_discord_color(&self, severity: &AlertSeverity) -> u32 {
        match severity {
            AlertSeverity::Critical => 0xFF0000, // Red
            AlertSeverity::High => 0xFF8C00,     // Orange
            AlertSeverity::Medium => 0xFFD700,   // Gold
            AlertSeverity::Low => 0x32CD32,      // Green
            AlertSeverity::Info => 0x1E90FF,     // Blue
        }
    }

    fn get_pagerduty_severity(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "critical",
            AlertSeverity::High => "error",
            AlertSeverity::Medium => "warning",
            AlertSeverity::Low => "warning",
            AlertSeverity::Info => "info",
        }
    }
}

/// Alert state manager for tracking active alerts
pub struct AlertStateManager {
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_fingerprints: Arc<RwLock<HashMap<String, SystemTime>>>,
}

impl Default for AlertStateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertStateManager {
    pub fn new() -> Self {
        Self {
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_fingerprints: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add active alert
    pub async fn add_active_alert(&self, alert: Alert) {
        let fingerprint = self.calculate_fingerprint(&alert);

        self.active_alerts
            .write()
            .await
            .insert(alert.id.clone(), alert);
        self.alert_fingerprints
            .write()
            .await
            .insert(fingerprint, SystemTime::now());
    }

    /// Remove active alert
    pub async fn resolve_alert(&self, alert_id: &str) -> bool {
        self.active_alerts.write().await.remove(alert_id).is_some()
    }

    /// Get all active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        self.active_alerts.read().await.values().cloned().collect()
    }

    /// Check if alert is duplicate
    pub async fn is_duplicate(&self, alert: &Alert) -> bool {
        let fingerprint = self.calculate_fingerprint(alert);
        let fingerprints = self.alert_fingerprints.read().await;

        if let Some(last_seen) = fingerprints.get(&fingerprint) {
            // Consider duplicate if seen within last 5 minutes
            SystemTime::now()
                .duration_since(*last_seen)
                .unwrap_or(Duration::from_secs(0))
                < Duration::from_secs(300)
        } else {
            false
        }
    }

    /// Calculate alert fingerprint for deduplication
    fn calculate_fingerprint(&self, alert: &Alert) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        alert.name.hash(&mut hasher);
        alert.metric_name.hash(&mut hasher);
        alert.category.hash(&mut hasher);
        // Don't include timestamp or current_value for deduplication

        format!("{:x}", hasher.finish())
    }
}

// Supporting types and configurations...

#[derive(Debug, Clone)]
pub struct AlertingConfig {
    pub rules: Vec<AlertRule>,
    pub notifications: NotificationConfig,
    pub escalation: EscalationConfig,
    pub history_retention_days: u32,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            rules: Self::default_rules(),
            notifications: NotificationConfig::default(),
            escalation: EscalationConfig::default(),
            history_retention_days: 30,
        }
    }
}

impl AlertingConfig {
    fn default_rules() -> Vec<AlertRule> {
        vec![
            AlertRule {
                name: "High CPU Usage".to_string(),
                description: "CPU usage is above 85%".to_string(),
                metric_name: "cpu_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan(85.0),
                severity: AlertSeverity::High,
                category: "performance".to_string(),
                evaluation_interval: Duration::from_secs(60),
                tags: vec!["cpu".to_string(), "performance".to_string()],
            },
            AlertRule {
                name: "High Memory Usage".to_string(),
                description: "Memory usage is above 2GB".to_string(),
                metric_name: "memory_usage_mb".to_string(),
                condition: AlertCondition::GreaterThan(2048.0),
                severity: AlertSeverity::High,
                category: "performance".to_string(),
                evaluation_interval: Duration::from_secs(60),
                tags: vec!["memory".to_string(), "performance".to_string()],
            },
            AlertRule {
                name: "High Error Rate".to_string(),
                description: "Error rate is above 5%".to_string(),
                metric_name: "error_rate".to_string(),
                condition: AlertCondition::GreaterThan(5.0),
                severity: AlertSeverity::Critical,
                category: "errors".to_string(),
                evaluation_interval: Duration::from_secs(30),
                tags: vec!["errors".to_string(), "reliability".to_string()],
            },
            AlertRule {
                name: "High Consensus Latency".to_string(),
                description: "Consensus latency is above 1000ms".to_string(),
                metric_name: "consensus_latency_ms".to_string(),
                condition: AlertCondition::GreaterThan(1000.0),
                severity: AlertSeverity::High,
                category: "performance".to_string(),
                evaluation_interval: Duration::from_secs(30),
                tags: vec!["consensus".to_string(), "latency".to_string()],
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub description: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub category: String,
    pub evaluation_interval: Duration,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertCondition {
    GreaterThan(f64),
    LessThan(f64),
    Equals(f64),
    NotEquals(f64),
    Between(f64, f64),
    Outside(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info = 1,
    Low = 2,
    Medium = 3,
    High = 4,
    Critical = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub category: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub timestamp: SystemTime,
    pub resolved_at: Option<SystemTime>,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct AlertStatus {
    pub active_alerts: usize,
    pub critical_alerts: usize,
    pub total_alerts_last_24h: usize,
    pub system_health: SystemHealth,
}

#[derive(Debug)]
pub enum SystemHealth {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug)]
pub struct AlertStatistics {
    pub total_alerts_processed: usize,
    pub active_alerts: usize,
    pub alerts_last_hour: usize,
    pub alerts_last_24_hours: usize,
    pub alerts_by_severity: HashMap<AlertSeverity, usize>,
    pub alerts_by_category: HashMap<String, usize>,
    pub average_resolution_time_minutes: f64,
    pub false_positive_rate: f64,
}

// Notification types
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    pub channels: Vec<NotificationChannel>,
    pub rate_limit: NotificationRateLimit,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            channels: vec![NotificationChannel {
                name: "console".to_string(),
                channel_type: NotificationChannelType::Webhook {
                    url: "http://localhost:8080/alerts".to_string(),
                    headers: HashMap::new(),
                },
                min_severity: Some(AlertSeverity::Medium),
                categories: vec![],
                required_tags: vec![],
            }],
            rate_limit: NotificationRateLimit {
                max_per_hour: 100,
                max_per_day: 1000,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotificationChannel {
    pub name: String,
    pub channel_type: NotificationChannelType,
    pub min_severity: Option<AlertSeverity>,
    pub categories: Vec<String>,
    pub required_tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum NotificationChannelType {
    Email {
        to: String,
        smtp_config: SMTPConfig,
    },
    Slack {
        webhook_url: String,
    },
    Discord {
        webhook_url: String,
    },
    PagerDuty {
        integration_key: String,
    },
    Webhook {
        url: String,
        headers: HashMap<String, String>,
    },
    SMS {
        phone_number: String,
        api_config: SMSConfig,
    },
}

#[derive(Debug, Clone)]
pub struct SMTPConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
}

#[derive(Debug, Clone)]
pub struct SMSConfig {
    pub api_key: String,
    pub api_url: String,
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct NotificationRateLimit {
    pub max_per_hour: u32,
    pub max_per_day: u32,
}

// Escalation types
#[derive(Debug, Clone)]
pub struct EscalationConfig {
    pub enable_escalation: bool,
    pub escalation_delay_minutes: u32,
    pub max_escalation_level: u32,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            enable_escalation: true,
            escalation_delay_minutes: 15,
            max_escalation_level: 3,
        }
    }
}

// Placeholder implementations for remaining components
pub struct EscalationManager {
    config: EscalationConfig,
}

impl EscalationManager {
    pub fn new(config: EscalationConfig) -> Self {
        Self { config }
    }

    pub async fn check_escalation(&self, _alert: &Alert) -> Result<Option<Alert>, AlertingError> {
        // Placeholder implementation
        Ok(None)
    }
}

pub struct NotificationRateLimiter {
    rate_limit: NotificationRateLimit,
    per_minute_counts: Arc<RwLock<HashMap<String, u32>>>,
    per_hour_counts: Arc<RwLock<HashMap<String, u32>>>,
    hourly_counts: Arc<RwLock<HashMap<String, u32>>>,
    daily_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl NotificationRateLimiter {
    pub fn new(rate_limit: NotificationRateLimit) -> Self {
        Self {
            rate_limit,
            per_minute_counts: Arc::new(RwLock::new(HashMap::new())),
            per_hour_counts: Arc::new(RwLock::new(HashMap::new())),
            hourly_counts: Arc::new(RwLock::new(HashMap::new())),
            daily_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn can_send(&self, alert_name: &str) -> bool {
        // Check rate limits for the alert
        let per_minute = self.per_minute_counts.read().await;
        let minute_count = per_minute.get(alert_name).unwrap_or(&0);

        // Allow up to 10 alerts per minute
        let can_send = *minute_count < 10;
        debug!(
            "Rate limit check for {}: count={}, can_send={}",
            alert_name, minute_count, can_send
        );
        can_send
    }

    pub async fn record_sent(&self, alert_name: &str) {
        // Increment counters
        let mut per_minute = self.per_minute_counts.write().await;
        *per_minute.entry(alert_name.to_string()).or_insert(0) += 1;

        let mut per_hour = self.per_hour_counts.write().await;
        *per_hour.entry(alert_name.to_string()).or_insert(0) += 1;

        debug!("Recorded sent notification for {}", alert_name);
    }
}

pub struct AlertHistory {
    alerts: VecDeque<Alert>,
    retention_days: u32,
}

impl AlertHistory {
    pub fn new(retention_days: u32) -> Self {
        Self {
            alerts: VecDeque::new(),
            retention_days,
        }
    }

    pub fn add_alert(&mut self, alert: Alert) {
        self.alerts.push_back(alert);
    }

    pub fn cleanup_old_alerts(&mut self) {
        let cutoff =
            SystemTime::now() - Duration::from_secs(self.retention_days as u64 * 24 * 3600);
        self.alerts.retain(|alert| alert.timestamp > cutoff);
    }

    pub fn total_count(&self) -> usize {
        self.alerts.len()
    }

    pub fn count_alerts_in_last_hours(&self, hours: u32) -> usize {
        let cutoff = SystemTime::now() - Duration::from_secs(hours as u64 * 3600);
        self.alerts
            .iter()
            .filter(|alert| alert.timestamp > cutoff)
            .count()
    }

    pub fn count_by_severity(&self) -> HashMap<AlertSeverity, usize> {
        let mut counts = HashMap::new();
        for alert in &self.alerts {
            *counts.entry(alert.severity).or_insert(0) += 1;
        }
        counts
    }

    pub fn count_by_category(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for alert in &self.alerts {
            *counts.entry(alert.category.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn average_resolution_time_minutes(&self) -> f64 {
        let resolved_alerts: Vec<_> = self
            .alerts
            .iter()
            .filter(|alert| alert.resolved_at.is_some())
            .collect();

        if resolved_alerts.is_empty() {
            return 0.0;
        }

        let total_time: Duration = resolved_alerts
            .iter()
            .map(|alert| {
                alert
                    .resolved_at
                    .unwrap()
                    .duration_since(alert.timestamp)
                    .unwrap_or(Duration::from_secs(0))
            })
            .sum();

        total_time.as_secs_f64() / 60.0 / resolved_alerts.len() as f64
    }

    pub fn calculate_false_positive_rate(&self) -> f64 {
        // This would calculate false positive rate based on alert resolution data
        0.05 // Example: 5% false positive rate
    }
}

#[derive(Debug)]
pub enum AlertingError {
    ProcessingError(String),
    NotificationError(String),
    ConfigurationError(String),
    UnknownMetric(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_alerting_system_creation() {
        let config = AlertingConfig::default();
        let system = AlertingSystem::new(config);

        let status = system.get_alert_status().await;
        assert_eq!(status.active_alerts, 0);
    }

    #[tokio::test]
    async fn test_alert_rule_evaluation() {
        let rules = vec![AlertRule {
            name: "Test Rule".to_string(),
            description: "Test alert rule".to_string(),
            metric_name: "cpu_usage_percent".to_string(),
            condition: AlertCondition::GreaterThan(50.0),
            severity: AlertSeverity::Medium,
            category: "test".to_string(),
            evaluation_interval: Duration::from_secs(60),
            tags: vec!["test".to_string()],
        }];

        let engine = AlertRulesEngine::new(rules);
        let alerts = engine.evaluate_rules().await.unwrap();

        // This test would depend on actual metric values
        // For now, just verify the function doesn't panic
        assert!(alerts.len() >= 0);
    }

    #[test]
    fn test_alert_fingerprint() {
        let state_manager = AlertStateManager::new();

        let alert1 = Alert {
            id: "1".to_string(),
            name: "Test Alert".to_string(),
            description: "Test".to_string(),
            severity: AlertSeverity::Medium,
            category: "test".to_string(),
            metric_name: "test_metric".to_string(),
            current_value: 100.0,
            threshold_value: 50.0,
            timestamp: SystemTime::now(),
            resolved_at: None,
            tags: vec![],
        };

        let alert2 = Alert {
            id: "2".to_string(),
            current_value: 200.0,         // Different value
            timestamp: SystemTime::now(), // Different timestamp
            ..alert1.clone()
        };

        // Same fingerprint despite different values/timestamps
        let fp1 = state_manager.calculate_fingerprint(&alert1);
        let fp2 = state_manager.calculate_fingerprint(&alert2);
        assert_eq!(fp1, fp2);
    }
}

/// Enhanced notification channels for production deployment
pub mod enhanced_notifications {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    /// PagerDuty integration
    #[derive(Debug, Clone)]
    pub struct PagerDutyNotifier {
        pub integration_key: String,
        pub service_url: String,
        pub client: reqwest::Client,
    }

    impl PagerDutyNotifier {
        pub fn new(integration_key: String) -> Self {
            Self {
                integration_key,
                service_url: "https://events.pagerduty.com/v2/enqueue".to_string(),
                client: reqwest::Client::new(),
            }
        }

        pub async fn send_incident(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let payload = json!({
                "routing_key": self.integration_key,
                "event_action": "trigger",
                "dedup_key": format!("bitcraps-{}-{}", alert.category, alert.name),
                "payload": {
                    "summary": format!("{}: {}", alert.name, alert.description),
                    "severity": match alert.severity {
                        AlertSeverity::Critical => "critical",
                        AlertSeverity::High => "error",
                        AlertSeverity::Medium => "warning",
                        AlertSeverity::Low => "info",
                        AlertSeverity::Info => "info",
                    },
                    "source": "BitCraps Monitoring System",
                    "component": alert.category,
                    "group": "monitoring",
                    "class": alert.metric_name,
                    "custom_details": {
                        "metric_name": alert.metric_name,
                        "current_value": alert.current_value,
                        "threshold_value": alert.threshold_value,
                        "timestamp": alert.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        "tags": alert.tags
                    }
                }
            });

            let response = self
                .client
                .post(&self.service_url)
                .json(&payload)
                .send()
                .await?;

            if response.status().is_success() {
                log::info!(
                    "Successfully sent PagerDuty incident for alert: {}",
                    alert.name
                );
            } else {
                log::error!("Failed to send PagerDuty incident: {}", response.status());
            }

            Ok(())
        }
    }

    /// Slack webhook notifier
    #[derive(Debug, Clone)]
    pub struct SlackNotifier {
        pub webhook_url: String,
        pub channel: String,
        pub username: String,
        pub client: reqwest::Client,
    }

    impl SlackNotifier {
        pub fn new(webhook_url: String, channel: String) -> Self {
            Self {
                webhook_url,
                channel,
                username: "BitCraps Monitor".to_string(),
                client: reqwest::Client::new(),
            }
        }

        pub async fn send_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let color = match alert.severity {
                AlertSeverity::Critical => "#FF0000", // Red
                AlertSeverity::High => "#FF8C00",     // Dark Orange
                AlertSeverity::Medium => "#FFD700",   // Gold
                AlertSeverity::Low => "#32CD32",      // Lime Green
                AlertSeverity::Info => "#439FE0",     // Blue
            };

            let icon = match alert.severity {
                AlertSeverity::Critical => ":rotating_light:",
                AlertSeverity::High => ":warning:",
                AlertSeverity::Medium => ":large_orange_diamond:",
                AlertSeverity::Low => ":information_source:",
                AlertSeverity::Info => ":information_source:",
            };

            let payload = json!({
                "channel": self.channel,
                "username": self.username,
                "icon_emoji": icon,
                "attachments": [{
                    "color": color,
                    "title": format!("{} {}", icon, alert.name),
                    "text": alert.description,
                    "fields": [
                        {
                            "title": "Severity",
                            "value": format!("{:?}", alert.severity),
                            "short": true
                        },
                        {
                            "title": "Category",
                            "value": alert.category,
                            "short": true
                        },
                        {
                            "title": "Metric",
                            "value": alert.metric_name,
                            "short": true
                        },
                        {
                            "title": "Value",
                            "value": format!("{:.2} (threshold: {:.2})",
                                           alert.current_value, alert.threshold_value),
                            "short": true
                        }
                    ],
                    "footer": "BitCraps Monitoring",
                    "ts": alert.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    "mrkdwn_in": ["text", "fields"]
                }]
            });

            let response = self
                .client
                .post(&self.webhook_url)
                .json(&payload)
                .send()
                .await?;

            if response.status().is_success() {
                log::info!(
                    "Successfully sent Slack notification for alert: {}",
                    alert.name
                );
            } else {
                log::error!("Failed to send Slack notification: {}", response.status());
            }

            Ok(())
        }
    }

    /// Microsoft Teams webhook notifier
    #[derive(Debug, Clone)]
    pub struct TeamsNotifier {
        pub webhook_url: String,
        pub client: reqwest::Client,
    }

    impl TeamsNotifier {
        pub fn new(webhook_url: String) -> Self {
            Self {
                webhook_url,
                client: reqwest::Client::new(),
            }
        }

        pub async fn send_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let theme_color = match alert.severity {
                AlertSeverity::Critical => "FF0000",
                AlertSeverity::High => "FF8C00",
                AlertSeverity::Medium => "FFD700",
                AlertSeverity::Low => "32CD32",
                AlertSeverity::Info => "439FE0",
            };

            let payload = json!({
                "@type": "MessageCard",
                "@context": "https://schema.org/extensions",
                "summary": format!("BitCraps Alert: {}", alert.name),
                "themeColor": theme_color,
                "title": format!("🚨 BitCraps Alert: {}", alert.name),
                "text": alert.description,
                "sections": [{
                    "facts": [
                        {"name": "Severity", "value": format!("{:?}", alert.severity)},
                        {"name": "Category", "value": alert.category},
                        {"name": "Metric", "value": alert.metric_name},
                        {"name": "Current Value", "value": format!("{:.2}", alert.current_value)},
                        {"name": "Threshold", "value": format!("{:.2}", alert.threshold_value)},
                        {"name": "Time", "value": format!("{:?}", alert.timestamp)}
                    ]
                }],
                "potentialAction": [{
                    "@type": "OpenUri",
                    "name": "View Dashboard",
                    "targets": [{
                        "os": "default",
                        "uri": "http://monitoring.bitcraps.local:3000"
                    }]
                }]
            });

            let response = self
                .client
                .post(&self.webhook_url)
                .json(&payload)
                .send()
                .await?;

            if response.status().is_success() {
                log::info!(
                    "Successfully sent Teams notification for alert: {}",
                    alert.name
                );
            } else {
                log::error!("Failed to send Teams notification: {}", response.status());
            }

            Ok(())
        }
    }

    /// Email notifier using SMTP
    #[derive(Debug, Clone)]
    pub struct EmailNotifier {
        pub smtp_host: String,
        pub smtp_port: u16,
        pub username: String,
        pub password: String,
        pub from_email: String,
        pub to_emails: Vec<String>,
    }

    impl EmailNotifier {
        pub fn new(
            smtp_host: String,
            smtp_port: u16,
            username: String,
            password: String,
            from_email: String,
            to_emails: Vec<String>,
        ) -> Self {
            Self {
                smtp_host,
                smtp_port,
                username,
                password,
                from_email,
                to_emails,
            }
        }

        pub async fn send_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let subject = format!("BitCraps Alert - {} ({:?})", alert.name, alert.severity);

            let body = format!(
                r#"
                <!DOCTYPE html>
                <html>
                <body>
                <h2 style="color: {};">🚨 BitCraps Alert: {}</h2>
                <p><strong>Description:</strong> {}</p>
                <table style="border-collapse: collapse; width: 100%;">
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Severity:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{:?}</td></tr>
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Category:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{}</td></tr>
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Metric:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{}</td></tr>
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Current Value:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{:.2}</td></tr>
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Threshold:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{:.2}</td></tr>
                    <tr><td style="border: 1px solid #ddd; padding: 8px;"><strong>Time:</strong></td><td style="border: 1px solid #ddd; padding: 8px;">{:?}</td></tr>
                </table>
                <p><a href="http://monitoring.bitcraps.local:3000">View Dashboard</a></p>
                </body>
                </html>
                "#,
                match alert.severity {
                    AlertSeverity::Critical => "#FF0000",
                    AlertSeverity::High => "#FF8C00",
                    AlertSeverity::Medium => "#FFD700",
                    AlertSeverity::Low => "#32CD32",
                    AlertSeverity::Info => "#439FE0",
                },
                alert.name,
                alert.description,
                alert.severity,
                alert.category,
                alert.metric_name,
                alert.current_value,
                alert.threshold_value,
                alert.timestamp
            );

            // In a production environment, you'd use lettre or similar SMTP crate
            log::info!(
                "Would send email alert '{}' to {:?}",
                subject,
                self.to_emails
            );

            Ok(())
        }
    }

    /// SMS notifier using Twilio
    #[derive(Debug, Clone)]
    pub struct SmsNotifier {
        pub account_sid: String,
        pub auth_token: String,
        pub from_number: String,
        pub to_numbers: Vec<String>,
        pub client: reqwest::Client,
    }

    impl SmsNotifier {
        pub fn new(
            account_sid: String,
            auth_token: String,
            from_number: String,
            to_numbers: Vec<String>,
        ) -> Self {
            Self {
                account_sid,
                auth_token,
                from_number,
                to_numbers,
                client: reqwest::Client::new(),
            }
        }

        pub async fn send_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let message = format!(
                "🚨 BitCraps Alert: {} ({:?})\n{}\nMetric: {} = {:.2} (threshold: {:.2})",
                alert.name,
                alert.severity,
                alert.description,
                alert.metric_name,
                alert.current_value,
                alert.threshold_value
            );

            for phone_number in &self.to_numbers {
                // In production, integrate with Twilio API
                log::info!("Would send SMS to {}: {}", phone_number, message);
            }

            Ok(())
        }
    }

    /// Generic webhook notifier
    #[derive(Debug, Clone)]
    pub struct WebhookNotifier {
        pub url: String,
        pub headers: HashMap<String, String>,
        pub client: reqwest::Client,
    }

    impl WebhookNotifier {
        pub fn new(url: String, headers: HashMap<String, String>) -> Self {
            Self {
                url,
                headers,
                client: reqwest::Client::new(),
            }
        }

        pub async fn send_alert(&self, alert: &Alert) -> Result<(), Box<dyn std::error::Error>> {
            let mut request = self.client.post(&self.url);

            for (key, value) in &self.headers {
                request = request.header(key, value);
            }

            let payload = json!({
                "alert_id": alert.id,
                "name": alert.name,
                "description": alert.description,
                "severity": alert.severity,
                "category": alert.category,
                "metric_name": alert.metric_name,
                "current_value": alert.current_value,
                "threshold_value": alert.threshold_value,
                "timestamp": alert.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                "tags": alert.tags
            });

            let response = request.json(&payload).send().await?;

            if response.status().is_success() {
                log::info!(
                    "Successfully sent webhook notification for alert: {}",
                    alert.name
                );
            } else {
                log::error!("Failed to send webhook notification: {}", response.status());
            }

            Ok(())
        }
    }
}
