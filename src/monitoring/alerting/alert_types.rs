//! Alert Type Definitions for BitCraps Monitoring
//!
//! This module contains all the core data structures and types used
//! for representing alerts in the monitoring system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info = 1,
    Low = 2,
    Medium = 3,
    High = 4,
    Critical = 5,
}

/// Alert conditions for rule evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum AlertCondition {
    GreaterThan(f64),
    LessThan(f64),
    Equals(f64),
    NotEquals(f64),
    Between(f64, f64),
    Outside(f64, f64),
}

/// Alert instance with all relevant information
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

/// Alert rule definition for triggering conditions
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

/// Current alert status for the system
#[derive(Debug)]
pub struct AlertStatus {
    pub active_alerts: usize,
    pub critical_alerts: usize,
    pub total_alerts_last_24h: usize,
    pub system_health: SystemHealth,
}

/// Overall system health status
#[derive(Debug)]
pub enum SystemHealth {
    Healthy,
    Warning,
    Critical,
}

/// Alert statistics and metrics
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

/// Alert configuration
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
    pub fn default_rules() -> Vec<AlertRule> {
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
            AlertRule {
                name: "Low Disk Space".to_string(),
                description: "Disk usage is above 90%".to_string(),
                metric_name: "disk_usage_percent".to_string(),
                condition: AlertCondition::GreaterThan(90.0),
                severity: AlertSeverity::High,
                category: "storage".to_string(),
                evaluation_interval: Duration::from_secs(300),
                tags: vec!["disk".to_string(), "storage".to_string()],
            },
            AlertRule {
                name: "Network Connection Issues".to_string(),
                description: "Active connections dropped below minimum".to_string(),
                metric_name: "active_connections".to_string(),
                condition: AlertCondition::LessThan(1.0),
                severity: AlertSeverity::Critical,
                category: "network".to_string(),
                evaluation_interval: Duration::from_secs(30),
                tags: vec!["network".to_string(), "connectivity".to_string()],
            },
        ]
    }
}

// Notification configuration types
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
                    url: std::env::var("ALERT_WEBHOOK_URL")
                        .unwrap_or_else(|_| "http://localhost:8080/alerts".to_string()),
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

/// Notification channel configuration
#[derive(Debug, Clone)]
pub struct NotificationChannel {
    pub name: String,
    pub channel_type: NotificationChannelType,
    pub min_severity: Option<AlertSeverity>,
    pub categories: Vec<String>,
    pub required_tags: Vec<String>,
}

/// Types of notification channels available
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
    Teams {
        webhook_url: String,
    },
}

/// SMTP configuration for email notifications
#[derive(Debug, Clone)]
pub struct SMTPConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
}

/// SMS API configuration
#[derive(Debug, Clone)]
pub struct SMSConfig {
    pub api_key: String,
    pub api_url: String,
    pub provider: String,
}

/// Notification rate limiting configuration
#[derive(Debug, Clone)]
pub struct NotificationRateLimit {
    pub max_per_hour: u32,
    pub max_per_day: u32,
}

// Escalation configuration types
#[derive(Debug, Clone)]
pub struct EscalationConfig {
    pub enable_escalation: bool,
    pub escalation_delay_minutes: u32,
    pub max_escalation_level: u32,
    pub escalation_rules: Vec<EscalationRule>,
}

impl Default for EscalationConfig {
    fn default() -> Self {
        Self {
            enable_escalation: true,
            escalation_delay_minutes: 15,
            max_escalation_level: 3,
            escalation_rules: vec![
                EscalationRule {
                    level: 1,
                    delay_minutes: 15,
                    severity_filter: Some(AlertSeverity::High),
                    channels: vec!["primary_oncall".to_string()],
                },
                EscalationRule {
                    level: 2,
                    delay_minutes: 30,
                    severity_filter: Some(AlertSeverity::Critical),
                    channels: vec!["secondary_oncall".to_string(), "manager".to_string()],
                },
                EscalationRule {
                    level: 3,
                    delay_minutes: 60,
                    severity_filter: Some(AlertSeverity::Critical),
                    channels: vec!["executive_oncall".to_string()],
                },
            ],
        }
    }
}

/// Escalation rule for automated escalation
#[derive(Debug, Clone)]
pub struct EscalationRule {
    pub level: u32,
    pub delay_minutes: u32,
    pub severity_filter: Option<AlertSeverity>,
    pub channels: Vec<String>,
}

/// Error types for alerting system
#[derive(Debug, thiserror::Error)]
pub enum AlertingError {
    #[error("Processing error: {0}")]
    ProcessingError(String),
    
    #[error("Notification error: {0}")]
    NotificationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Unknown metric: {0}")]
    UnknownMetric(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("HTTP error: {0}")]
    HttpError(String),
}

impl Alert {
    /// Create a new alert instance
    pub fn new(
        name: String,
        description: String,
        severity: AlertSeverity,
        category: String,
        metric_name: String,
        current_value: f64,
        threshold_value: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            severity,
            category,
            metric_name,
            current_value,
            threshold_value,
            timestamp: SystemTime::now(),
            resolved_at: None,
            tags: Vec::new(),
        }
    }

    /// Add a tag to the alert
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    /// Add multiple tags to the alert
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Check if alert is resolved
    pub fn is_resolved(&self) -> bool {
        self.resolved_at.is_some()
    }

    /// Resolve the alert
    pub fn resolve(&mut self) {
        self.resolved_at = Some(SystemTime::now());
    }

    /// Get alert age in seconds
    pub fn age_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }

    /// Get resolution time in seconds (if resolved)
    pub fn resolution_time_seconds(&self) -> Option<u64> {
        self.resolved_at.map(|resolved| {
            resolved
                .duration_since(self.timestamp)
                .unwrap_or(Duration::from_secs(0))
                .as_secs()
        })
    }
}

impl AlertCondition {
    /// Evaluate condition against a value
    pub fn evaluate(&self, value: f64) -> bool {
        match self {
            AlertCondition::GreaterThan(threshold) => value > *threshold,
            AlertCondition::LessThan(threshold) => value < *threshold,
            AlertCondition::Equals(threshold) => (value - threshold).abs() < f64::EPSILON,
            AlertCondition::NotEquals(threshold) => (value - threshold).abs() >= f64::EPSILON,
            AlertCondition::Between(min, max) => value >= *min && value <= *max,
            AlertCondition::Outside(min, max) => value < *min || value > *max,
        }
    }

    /// Get threshold value(s) as a string for display
    pub fn threshold_display(&self) -> String {
        match self {
            AlertCondition::GreaterThan(threshold) => format!("> {}", threshold),
            AlertCondition::LessThan(threshold) => format!("< {}", threshold),
            AlertCondition::Equals(threshold) => format!("= {}", threshold),
            AlertCondition::NotEquals(threshold) => format!("≠ {}", threshold),
            AlertCondition::Between(min, max) => format!("{} - {}", min, max),
            AlertCondition::Outside(min, max) => format!("< {} or > {}", min, max),
        }
    }

    /// Get the primary threshold value for comparison
    pub fn primary_threshold(&self) -> f64 {
        match self {
            AlertCondition::GreaterThan(threshold)
            | AlertCondition::LessThan(threshold)
            | AlertCondition::Equals(threshold)
            | AlertCondition::NotEquals(threshold) => *threshold,
            AlertCondition::Between(min, max) | AlertCondition::Outside(min, max) => (min + max) / 2.0,
        }
    }
}

impl AlertRule {
    /// Create a new alert rule
    pub fn new(
        name: String,
        metric_name: String,
        condition: AlertCondition,
        severity: AlertSeverity,
    ) -> Self {
        Self {
            name: name.clone(),
            description: format!("Alert rule for {}", name),
            metric_name,
            condition,
            severity,
            category: "general".to_string(),
            evaluation_interval: Duration::from_secs(60),
            tags: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Set the category
    pub fn with_category(mut self, category: String) -> Self {
        self.category = category;
        self
    }

    /// Set the evaluation interval
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.evaluation_interval = interval;
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_creation() {
        let alert = Alert::new(
            "Test Alert".to_string(),
            "Test description".to_string(),
            AlertSeverity::Medium,
            "test".to_string(),
            "test_metric".to_string(),
            100.0,
            80.0,
        );

        assert_eq!(alert.name, "Test Alert");
        assert_eq!(alert.severity, AlertSeverity::Medium);
        assert!(!alert.is_resolved());
        assert!(alert.age_seconds() >= 0);
    }

    #[test]
    fn test_alert_resolution() {
        let mut alert = Alert::new(
            "Test Alert".to_string(),
            "Test description".to_string(),
            AlertSeverity::Low,
            "test".to_string(),
            "test_metric".to_string(),
            50.0,
            40.0,
        );

        assert!(!alert.is_resolved());
        alert.resolve();
        assert!(alert.is_resolved());
        assert!(alert.resolution_time_seconds().is_some());
    }

    #[test]
    fn test_alert_condition_evaluation() {
        let gt_condition = AlertCondition::GreaterThan(50.0);
        assert!(gt_condition.evaluate(60.0));
        assert!(!gt_condition.evaluate(40.0));

        let between_condition = AlertCondition::Between(10.0, 20.0);
        assert!(between_condition.evaluate(15.0));
        assert!(!between_condition.evaluate(25.0));

        let outside_condition = AlertCondition::Outside(10.0, 20.0);
        assert!(outside_condition.evaluate(5.0));
        assert!(outside_condition.evaluate(25.0));
        assert!(!outside_condition.evaluate(15.0));
    }

    #[test]
    fn test_alert_rule_builder() {
        let rule = AlertRule::new(
            "CPU High".to_string(),
            "cpu_usage".to_string(),
            AlertCondition::GreaterThan(80.0),
            AlertSeverity::High,
        )
        .with_description("CPU usage too high".to_string())
        .with_category("performance".to_string())
        .with_tags(vec!["cpu".to_string(), "performance".to_string()]);

        assert_eq!(rule.name, "CPU High");
        assert_eq!(rule.category, "performance");
        assert_eq!(rule.tags.len(), 2);
    }

    #[test]
    fn test_condition_threshold_display() {
        let gt = AlertCondition::GreaterThan(85.0);
        assert_eq!(gt.threshold_display(), "> 85");

        let between = AlertCondition::Between(10.0, 20.0);
        assert_eq!(between.threshold_display(), "10 - 20");

        let outside = AlertCondition::Outside(10.0, 20.0);
        assert_eq!(outside.threshold_display(), "< 10 or > 20");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(AlertSeverity::Critical > AlertSeverity::High);
        assert!(AlertSeverity::High > AlertSeverity::Medium);
        assert!(AlertSeverity::Medium > AlertSeverity::Low);
        assert!(AlertSeverity::Low > AlertSeverity::Info);
    }

    #[test]
    fn test_default_alerting_config() {
        let config = AlertingConfig::default();
        assert!(!config.rules.is_empty());
        assert_eq!(config.history_retention_days, 30);
        assert!(config.escalation.enable_escalation);
    }
}