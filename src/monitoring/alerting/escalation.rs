//! Alert Escalation System for BitCraps Monitoring
//!
//! This module handles escalation of unresolved alerts to higher priority
//! channels and personnel based on configurable rules and timeouts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::alert_types::*;

/// Escalation manager for handling alert escalations
pub struct EscalationManager {
    config: EscalationConfig,
    escalation_state: Arc<RwLock<HashMap<String, EscalationState>>>,
    escalation_history: Arc<RwLock<Vec<EscalationRecord>>>,
}

/// State tracking for alert escalations
#[derive(Debug, Clone)]
struct EscalationState {
    alert_id: String,
    current_level: u32,
    first_escalated_at: SystemTime,
    last_escalated_at: SystemTime,
    escalation_count: u32,
    acknowledgments: Vec<Acknowledgment>,
}

/// Record of an escalation event
#[derive(Debug, Clone)]
pub struct EscalationRecord {
    pub alert_id: String,
    pub alert_name: String,
    pub from_level: u32,
    pub to_level: u32,
    pub escalated_at: SystemTime,
    pub reason: EscalationReason,
    pub channels_notified: Vec<String>,
}

/// Reasons for escalation
#[derive(Debug, Clone)]
pub enum EscalationReason {
    TimeoutExpired,
    SeverityIncrease,
    ManualEscalation,
    NoAcknowledgment,
    RepeatOffender,
}

/// Acknowledgment of an alert
#[derive(Debug, Clone)]
pub struct Acknowledgment {
    pub acknowledger: String,
    pub timestamp: SystemTime,
    pub message: Option<String>,
}

/// Escalation statistics
#[derive(Debug)]
pub struct EscalationStats {
    pub total_escalations: usize,
    pub escalations_by_level: HashMap<u32, usize>,
    pub escalations_by_reason: HashMap<String, usize>,
    pub average_escalation_time_minutes: f64,
    pub most_escalated_alerts: Vec<(String, u32)>,
}

impl EscalationManager {
    /// Create new escalation manager
    pub fn new(config: EscalationConfig) -> Self {
        Self {
            config,
            escalation_state: Arc::new(RwLock::new(HashMap::new())),
            escalation_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if alert needs escalation and return escalated alert if so
    pub async fn check_escalation(&self, alert: &Alert) -> Result<Option<Alert>, AlertingError> {
        if !self.config.enable_escalation {
            return Ok(None);
        }

        let mut state_map = self.escalation_state.write().await;
        let should_escalate = if let Some(state) = state_map.get_mut(&alert.id) {
            self.should_escalate_existing(state, alert).await
        } else {
            self.should_escalate_new(alert).await
        };

        match should_escalate {
            Some((new_level, reason)) => {
                let escalated_alert = self.perform_escalation(
                    alert,
                    new_level,
                    reason,
                    &mut state_map,
                ).await?;
                Ok(Some(escalated_alert))
            }
            None => Ok(None),
        }
    }

    /// Manually escalate an alert to a specific level
    pub async fn manual_escalation(
        &self,
        alert: &Alert,
        target_level: u32,
        escalator: &str,
    ) -> Result<Alert, AlertingError> {
        if target_level > self.config.max_escalation_level {
            return Err(AlertingError::ConfigurationError(format!(
                "Target level {} exceeds maximum level {}",
                target_level, self.config.max_escalation_level
            )));
        }

        let mut state_map = self.escalation_state.write().await;
        let current_level = state_map
            .get(&alert.id)
            .map(|s| s.current_level)
            .unwrap_or(0);

        if target_level <= current_level {
            return Err(AlertingError::ConfigurationError(format!(
                "Cannot escalate to level {} from current level {}",
                target_level, current_level
            )));
        }

        let mut escalated_alert = alert.clone();
        escalated_alert.description = format!(
            "{} [ESCALATED by {} to level {}]",
            alert.description, escalator, target_level
        );

        // Update escalation state
        self.update_escalation_state(
            &mut state_map,
            &alert.id,
            target_level,
            EscalationReason::ManualEscalation,
        ).await;

        info!(
            "Manually escalated alert {} to level {} by {}",
            alert.name, target_level, escalator
        );

        Ok(escalated_alert)
    }

    /// Acknowledge an alert to prevent further escalation
    pub async fn acknowledge_alert(
        &self,
        alert_id: &str,
        acknowledger: String,
        message: Option<String>,
    ) -> Result<(), AlertingError> {
        let mut state_map = self.escalation_state.write().await;
        
        if let Some(state) = state_map.get_mut(alert_id) {
            state.acknowledgments.push(Acknowledgment {
                acknowledger: acknowledger.clone(),
                timestamp: SystemTime::now(),
                message: message.clone(),
            });

            info!(
                "Alert {} acknowledged by {}: {}",
                alert_id,
                acknowledger,
                message.unwrap_or_else(|| "No message".to_string())
            );

            Ok(())
        } else {
            Err(AlertingError::ProcessingError(format!(
                "Alert {} not found in escalation tracking",
                alert_id
            )))
        }
    }

    /// Remove alert from escalation tracking (when resolved)
    pub async fn remove_alert(&self, alert_id: &str) -> bool {
        let mut state_map = self.escalation_state.write().await;
        state_map.remove(alert_id).is_some()
    }

    /// Get escalation status for an alert
    pub async fn get_escalation_status(&self, alert_id: &str) -> Option<EscalationStatus> {
        let state_map = self.escalation_state.read().await;
        state_map.get(alert_id).map(|state| EscalationStatus {
            alert_id: state.alert_id.clone(),
            current_level: state.current_level,
            escalation_count: state.escalation_count,
            first_escalated_at: state.first_escalated_at,
            last_escalated_at: state.last_escalated_at,
            acknowledgments: state.acknowledgments.clone(),
            next_escalation_at: self.calculate_next_escalation_time(state),
        })
    }

    /// Get escalation statistics
    pub async fn get_statistics(&self) -> EscalationStats {
        let history = self.escalation_history.read().await;
        
        let mut escalations_by_level = HashMap::new();
        let mut escalations_by_reason = HashMap::new();
        let mut alert_escalation_counts = HashMap::new();
        let mut escalation_times = Vec::new();

        for record in &*history {
            *escalations_by_level.entry(record.to_level).or_insert(0) += 1;
            *escalations_by_reason
                .entry(format!("{:?}", record.reason))
                .or_insert(0) += 1;
            *alert_escalation_counts
                .entry(record.alert_name.clone())
                .or_insert(0) += 1;

            // Calculate escalation time (simplified)
            escalation_times.push(record.escalated_at);
        }

        let average_escalation_time_minutes = if escalation_times.is_empty() {
            0.0
        } else {
            // This is a simplified calculation - in practice you'd track
            // time from alert creation to escalation
            30.0 // Placeholder
        };

        let mut most_escalated: Vec<_> = alert_escalation_counts.into_iter().collect();
        most_escalated.sort_by(|a, b| b.1.cmp(&a.1));
        most_escalated.truncate(10);

        EscalationStats {
            total_escalations: history.len(),
            escalations_by_level,
            escalations_by_reason,
            average_escalation_time_minutes,
            most_escalated_alerts: most_escalated,
        }
    }

    /// Clean up old escalation records
    pub async fn cleanup_old_records(&self, retention_days: u32) {
        let cutoff = SystemTime::now() - Duration::from_secs(retention_days as u64 * 24 * 3600);
        
        {
            let mut history = self.escalation_history.write().await;
            let initial_len = history.len();
            history.retain(|record| record.escalated_at > cutoff);
            
            if history.len() != initial_len {
                debug!("Cleaned up escalation records: {} -> {}", initial_len, history.len());
            }
        }

        // Clean up resolved alert states
        {
            let mut state_map = self.escalation_state.write().await;
            let initial_len = state_map.len();
            state_map.retain(|_, state| state.last_escalated_at > cutoff);
            
            if state_map.len() != initial_len {
                debug!("Cleaned up escalation states: {} -> {}", initial_len, state_map.len());
            }
        }
    }

    /// Check if new alert should be escalated immediately
    async fn should_escalate_new(&self, alert: &Alert) -> Option<(u32, EscalationReason)> {
        // Check if alert severity warrants immediate escalation
        for rule in &self.config.escalation_rules {
            if let Some(min_severity) = rule.severity_filter {
                if alert.severity >= min_severity && rule.level == 1 {
                    return Some((rule.level, EscalationReason::SeverityIncrease));
                }
            }
        }
        None
    }

    /// Check if existing alert should be escalated
    async fn should_escalate_existing(
        &self,
        state: &mut EscalationState,
        alert: &Alert,
    ) -> Option<(u32, EscalationReason)> {
        // Check if alert has been acknowledged
        if !state.acknowledgments.is_empty() {
            let last_ack = state.acknowledgments.last().unwrap().timestamp;
            let time_since_ack = SystemTime::now()
                .duration_since(last_ack)
                .unwrap_or_default();
            
            // If acknowledged recently, don't escalate
            if time_since_ack < Duration::from_secs(self.config.escalation_delay_minutes as u64 * 60) {
                return None;
            }
        }

        // Find applicable escalation rule for next level
        let next_level = state.current_level + 1;
        if next_level > self.config.max_escalation_level {
            return None; // Already at maximum level
        }

        if let Some(rule) = self.find_escalation_rule(next_level) {
            let time_since_last = SystemTime::now()
                .duration_since(state.last_escalated_at)
                .unwrap_or_default();
            
            let required_delay = Duration::from_secs(rule.delay_minutes as u64 * 60);
            
            if time_since_last >= required_delay {
                // Check if severity still matches
                if let Some(min_severity) = rule.severity_filter {
                    if alert.severity >= min_severity {
                        return Some((next_level, EscalationReason::TimeoutExpired));
                    }
                } else {
                    return Some((next_level, EscalationReason::TimeoutExpired));
                }
            }
        }

        None
    }

    /// Perform the escalation
    async fn perform_escalation(
        &self,
        alert: &Alert,
        new_level: u32,
        reason: EscalationReason,
        state_map: &mut HashMap<String, EscalationState>,
    ) -> Result<Alert, AlertingError> {
        let current_level = state_map
            .get(&alert.id)
            .map(|s| s.current_level)
            .unwrap_or(0);

        // Create escalated alert
        let mut escalated_alert = alert.clone();
        escalated_alert.severity = self.escalate_severity(alert.severity, new_level);
        escalated_alert.description = format!(
            "{} [ESCALATED to level {} - {:?}]",
            alert.description, new_level, reason
        );

        // Get channels for this escalation level
        let channels = self.get_channels_for_level(new_level);

        // Update escalation state
        self.update_escalation_state(state_map, &alert.id, new_level, reason.clone()).await;

        // Record escalation
        let record = EscalationRecord {
            alert_id: alert.id.clone(),
            alert_name: alert.name.clone(),
            from_level: current_level,
            to_level: new_level,
            escalated_at: SystemTime::now(),
            reason: reason.clone(),
            channels_notified: channels.clone(),
        };

        {
            let mut history = self.escalation_history.write().await;
            history.push(record);
        }

        info!(
            "Escalated alert {} from level {} to level {} ({:?}) - channels: {:?}",
            alert.name, current_level, new_level, reason, channels
        );

        Ok(escalated_alert)
    }

    /// Update escalation state
    async fn update_escalation_state(
        &self,
        state_map: &mut HashMap<String, EscalationState>,
        alert_id: &str,
        new_level: u32,
        reason: EscalationReason,
    ) {
        let now = SystemTime::now();
        
        state_map
            .entry(alert_id.to_string())
            .and_modify(|state| {
                state.current_level = new_level;
                state.last_escalated_at = now;
                state.escalation_count += 1;
            })
            .or_insert(EscalationState {
                alert_id: alert_id.to_string(),
                current_level: new_level,
                first_escalated_at: now,
                last_escalated_at: now,
                escalation_count: 1,
                acknowledgments: Vec::new(),
            });
    }

    /// Find escalation rule for specific level
    fn find_escalation_rule(&self, level: u32) -> Option<&EscalationRule> {
        self.config.escalation_rules.iter().find(|rule| rule.level == level)
    }

    /// Get notification channels for escalation level
    fn get_channels_for_level(&self, level: u32) -> Vec<String> {
        if let Some(rule) = self.find_escalation_rule(level) {
            rule.channels.clone()
        } else {
            Vec::new()
        }
    }

    /// Escalate alert severity based on escalation level
    fn escalate_severity(&self, current: AlertSeverity, level: u32) -> AlertSeverity {
        match (current, level) {
            (AlertSeverity::Info, _) | (AlertSeverity::Low, _) if level >= 2 => AlertSeverity::Medium,
            (AlertSeverity::Medium, _) if level >= 3 => AlertSeverity::High,
            (AlertSeverity::High, _) if level >= 4 => AlertSeverity::Critical,
            _ => current, // No change
        }
    }

    /// Calculate when next escalation should occur
    fn calculate_next_escalation_time(&self, state: &EscalationState) -> Option<SystemTime> {
        let next_level = state.current_level + 1;
        if next_level > self.config.max_escalation_level {
            return None;
        }

        if let Some(rule) = self.find_escalation_rule(next_level) {
            Some(state.last_escalated_at + Duration::from_secs(rule.delay_minutes as u64 * 60))
        } else {
            None
        }
    }
}

impl Default for EscalationManager {
    fn default() -> Self {
        Self::new(EscalationConfig::default())
    }
}

/// Current escalation status for an alert
#[derive(Debug, Clone)]
pub struct EscalationStatus {
    pub alert_id: String,
    pub current_level: u32,
    pub escalation_count: u32,
    pub first_escalated_at: SystemTime,
    pub last_escalated_at: SystemTime,
    pub acknowledgments: Vec<Acknowledgment>,
    pub next_escalation_at: Option<SystemTime>,
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

    fn create_test_config() -> EscalationConfig {
        EscalationConfig {
            enable_escalation: true,
            escalation_delay_minutes: 1, // 1 minute for testing
            max_escalation_level: 3,
            escalation_rules: vec![
                EscalationRule {
                    level: 1,
                    delay_minutes: 1,
                    severity_filter: Some(AlertSeverity::High),
                    channels: vec!["level1".to_string()],
                },
                EscalationRule {
                    level: 2,
                    delay_minutes: 2,
                    severity_filter: Some(AlertSeverity::High),
                    channels: vec!["level2".to_string()],
                },
                EscalationRule {
                    level: 3,
                    delay_minutes: 3,
                    severity_filter: Some(AlertSeverity::Critical),
                    channels: vec!["level3".to_string()],
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_escalation_manager_creation() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);

        // Should start with no escalations
        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_escalations, 0);
    }

    #[tokio::test]
    async fn test_immediate_escalation_for_high_severity() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        // High severity alert should escalate immediately to level 1
        let result = manager.check_escalation(&alert).await.unwrap();
        assert!(result.is_some());

        let escalated = result.unwrap();
        assert!(escalated.description.contains("ESCALATED to level 1"));
    }

    #[tokio::test]
    async fn test_manual_escalation() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        let escalated = manager
            .manual_escalation(&alert, 2, "test_user")
            .await
            .unwrap();

        assert!(escalated.description.contains("ESCALATED by test_user to level 2"));

        // Check escalation status
        let status = manager.get_escalation_status(&alert.id).await.unwrap();
        assert_eq!(status.current_level, 2);
        assert_eq!(status.escalation_count, 1);
    }

    #[tokio::test]
    async fn test_acknowledge_alert() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        // First escalate the alert
        let _ = manager.manual_escalation(&alert, 1, "system").await;

        // Then acknowledge it
        let result = manager
            .acknowledge_alert(&alert.id, "user".to_string(), Some("Working on it".to_string()))
            .await;
        assert!(result.is_ok());

        // Check acknowledgment is recorded
        let status = manager.get_escalation_status(&alert.id).await.unwrap();
        assert_eq!(status.acknowledgments.len(), 1);
        assert_eq!(status.acknowledgments[0].acknowledger, "user");
        assert_eq!(status.acknowledgments[0].message, Some("Working on it".to_string()));
    }

    #[tokio::test]
    async fn test_escalation_level_limits() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        // Try to escalate beyond maximum level
        let result = manager.manual_escalation(&alert, 5, "test_user").await;
        assert!(result.is_err());

        // Should contain error about exceeding maximum level
        assert!(result.unwrap_err().to_string().contains("exceeds maximum level"));
    }

    #[tokio::test]
    async fn test_remove_alert_from_tracking() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        // Escalate alert
        let _ = manager.manual_escalation(&alert, 1, "system").await;
        
        // Verify it's being tracked
        assert!(manager.get_escalation_status(&alert.id).await.is_some());

        // Remove from tracking
        assert!(manager.remove_alert(&alert.id).await);

        // Should no longer be tracked
        assert!(manager.get_escalation_status(&alert.id).await.is_none());

        // Removing non-existent alert should return false
        assert!(!manager.remove_alert("nonexistent").await);
    }

    #[tokio::test]
    async fn test_escalation_statistics() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);

        let alert1 = create_test_alert();
        let mut alert2 = create_test_alert();
        alert2.id = "alert2".to_string();
        alert2.name = "Alert 2".to_string();

        // Perform some escalations
        let _ = manager.manual_escalation(&alert1, 1, "user1").await;
        let _ = manager.manual_escalation(&alert2, 2, "user2").await;
        let _ = manager.manual_escalation(&alert1, 2, "user1").await;

        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_escalations, 3);
        assert_eq!(stats.escalations_by_level.get(&1), Some(&1));
        assert_eq!(stats.escalations_by_level.get(&2), Some(&2));
    }

    #[tokio::test]
    async fn test_severity_escalation() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);

        // Test severity escalation logic
        assert_eq!(
            manager.escalate_severity(AlertSeverity::Low, 2),
            AlertSeverity::Medium
        );
        assert_eq!(
            manager.escalate_severity(AlertSeverity::Medium, 3),
            AlertSeverity::High
        );
        assert_eq!(
            manager.escalate_severity(AlertSeverity::High, 4),
            AlertSeverity::Critical
        );
        assert_eq!(
            manager.escalate_severity(AlertSeverity::Critical, 5),
            AlertSeverity::Critical
        ); // No change beyond Critical
    }

    #[tokio::test]
    async fn test_cleanup_old_records() {
        let config = create_test_config();
        let manager = EscalationManager::new(config);
        let alert = create_test_alert();

        // Add an escalation
        let _ = manager.manual_escalation(&alert, 1, "system").await;

        // Verify statistics show the escalation
        let stats_before = manager.get_statistics().await;
        assert_eq!(stats_before.total_escalations, 1);

        // Clean up with very short retention (should remove everything)
        manager.cleanup_old_records(0).await;

        // Statistics should show cleanup occurred
        let stats_after = manager.get_statistics().await;
        assert_eq!(stats_after.total_escalations, 0);
    }
}