//! Notification Channels for BitCraps Monitoring
//!
//! This module handles sending alerts through various notification channels
//! including Slack, Discord, email, SMS, PagerDuty, and custom webhooks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use serde_json::json;

use super::alert_types::*;

/// Notification dispatcher for sending alerts through various channels
pub struct NotificationDispatcher {
    channels: Vec<NotificationChannel>,
    rate_limiter: Arc<NotificationRateLimiter>,
    client: reqwest::Client,
}

impl NotificationDispatcher {
    /// Create new notification dispatcher
    pub fn new(config: NotificationConfig) -> Self {
        Self {
            channels: config.channels,
            rate_limiter: Arc::new(NotificationRateLimiter::new(config.rate_limit)),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Add a notification channel
    pub fn add_channel(&mut self, channel: NotificationChannel) {
        self.channels.push(channel);
    }

    /// Remove a notification channel by name
    pub fn remove_channel(&mut self, channel_name: &str) -> bool {
        let initial_len = self.channels.len();
        self.channels.retain(|ch| ch.name != channel_name);
        self.channels.len() != initial_len
    }

    /// Send notification through all configured channels
    pub async fn send_notification(&self, alert: &Alert) -> Result<(), AlertingError> {
        // Check global rate limit
        if !self.rate_limiter.can_send(&alert.name).await {
            debug!("Rate limiting notification for alert: {}", alert.name);
            return Err(AlertingError::RateLimitExceeded);
        }

        let mut successful_sends = 0;
        let mut errors = Vec::new();

        for channel in &self.channels {
            if self.should_send_to_channel(channel, alert) {
                match self.send_to_channel(channel, alert).await {
                    Ok(()) => {
                        successful_sends += 1;
                        info!("Alert sent via {}: {}", channel.name, alert.name);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to send via {}: {}", channel.name, e);
                        warn!("{}", error_msg);
                        errors.push(error_msg);
                    }
                }
            } else {
                debug!("Skipping channel {} for alert {} (filtering)", channel.name, alert.name);
            }
        }

        if successful_sends > 0 {
            self.rate_limiter.record_sent(&alert.name).await;
            info!("Alert {} sent via {} channels", alert.name, successful_sends);
            Ok(())
        } else if errors.is_empty() {
            warn!("No channels matched filters for alert: {}", alert.name);
            Ok(()) // Not an error if no channels match
        } else {
            Err(AlertingError::NotificationError(format!(
                "All notification attempts failed: {}",
                errors.join("; ")
            )))
        }
    }

    /// Send test notification to verify channel configuration
    pub async fn test_channel(&self, channel_name: &str) -> Result<(), AlertingError> {
        let channel = self.channels.iter()
            .find(|ch| ch.name == channel_name)
            .ok_or_else(|| AlertingError::ChannelNotFound(channel_name.to_string()))?;

        let test_alert = Alert::new(
            "Test Alert".to_string(),
            "This is a test notification to verify channel configuration".to_string(),
            AlertSeverity::Info,
            "test".to_string(),
            "test_metric".to_string(),
            42.0,
            0.0,
        );

        self.send_to_channel(channel, &test_alert).await
    }

    /// Check if alert should be sent to specific channel based on filters
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
            NotificationChannelType::Teams { webhook_url } => {
                self.send_teams_notification(webhook_url, alert).await
            }
            NotificationChannelType::PagerDuty { integration_key } => {
                self.send_pagerduty_notification(integration_key, alert).await
            }
            NotificationChannelType::Webhook { url, headers } => {
                self.send_webhook_notification(url, headers, alert).await
            }
            NotificationChannelType::SMS { phone_number, api_config } => {
                self.send_sms_notification(phone_number, api_config, alert).await
            }
        }
    }

    async fn send_email_notification(
        &self,
        _to: &str,
        _smtp_config: &SMTPConfig,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        // Email implementation would use lettre or similar SMTP crate
        info!("Would send email notification for alert: {}", alert.name);
        Ok(())
    }

    async fn send_slack_notification(
        &self,
        webhook_url: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let color = self.get_slack_color(&alert.severity);
        let icon = self.get_slack_icon(&alert.severity);
        
        let payload = json!({
            "text": format!("{} Alert: {}", icon, alert.name),
            "attachments": [{
                "color": color,
                "title": format!("{} {}", icon, alert.name),
                "text": alert.description,
                "fields": [
                    {"title": "Severity", "value": format!("{:?}", alert.severity), "short": true},
                    {"title": "Category", "value": &alert.category, "short": true},
                    {"title": "Metric", "value": &alert.metric_name, "short": true},
                    {"title": "Current Value", "value": format!("{:.2}", alert.current_value), "short": true},
                    {"title": "Threshold", "value": format!("{:.2}", alert.threshold_value), "short": true},
                    {"title": "Alert ID", "value": &alert.id, "short": true}
                ],
                "footer": "BitCraps Monitoring",
                "ts": alert.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                "mrkdwn_in": ["text", "fields"]
            }]
        });

        let response = self
            .client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AlertingError::HttpError(format!("Slack request failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Slack notification sent successfully for alert: {}", alert.name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            Err(AlertingError::HttpError(format!(
                "Slack notification failed with status {}: {}",
                status, body
            )))
        }
    }

    async fn send_discord_notification(
        &self,
        webhook_url: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let color = self.get_discord_color(&alert.severity);
        let icon = self.get_discord_icon(&alert.severity);
        
        let payload = json!({
            "embeds": [{
                "title": format!("{} {}", icon, alert.name),
                "description": alert.description,
                "color": color,
                "fields": [
                    {"name": "Severity", "value": format!("{:?}", alert.severity), "inline": true},
                    {"name": "Category", "value": &alert.category, "inline": true},
                    {"name": "Metric", "value": &alert.metric_name, "inline": true},
                    {"name": "Current Value", "value": format!("{:.2}", alert.current_value), "inline": true},
                    {"name": "Threshold", "value": format!("{:.2}", alert.threshold_value), "inline": true},
                    {"name": "Alert ID", "value": &alert.id, "inline": true}
                ],
                "footer": {"text": "BitCraps Monitoring"},
                "timestamp": chrono::DateTime::<chrono::Utc>::from(alert.timestamp).to_rfc3339()
            }]
        });

        let response = self
            .client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AlertingError::HttpError(format!("Discord request failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Discord notification sent successfully for alert: {}", alert.name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            Err(AlertingError::HttpError(format!(
                "Discord notification failed with status {}: {}",
                status, body
            )))
        }
    }

    async fn send_teams_notification(
        &self,
        webhook_url: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let theme_color = self.get_teams_color(&alert.severity);
        let icon = self.get_teams_icon(&alert.severity);
        
        let payload = json!({
            "@type": "MessageCard",
            "@context": "https://schema.org/extensions",
            "summary": format!("BitCraps Alert: {}", alert.name),
            "themeColor": theme_color,
            "title": format!("{} BitCraps Alert: {}", icon, alert.name),
            "text": alert.description,
            "sections": [{
                "facts": [
                    {"name": "Severity", "value": format!("{:?}", alert.severity)},
                    {"name": "Category", "value": &alert.category},
                    {"name": "Metric", "value": &alert.metric_name},
                    {"name": "Current Value", "value": format!("{:.2}", alert.current_value)},
                    {"name": "Threshold", "value": format!("{:.2}", alert.threshold_value)},
                    {"name": "Alert ID", "value": &alert.id},
                    {"name": "Time", "value": format!("{:?}", alert.timestamp)}
                ]
            }],
            "potentialAction": [{
                "@type": "OpenUri",
                "name": "View Dashboard",
                "targets": [{
                    "os": "default",
                    "uri": std::env::var("MONITORING_DASHBOARD_URL")
                        .unwrap_or_else(|_| "http://monitoring.bitcraps.local:3000".to_string())
                }]
            }]
        });

        let response = self
            .client
            .post(webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AlertingError::HttpError(format!("Teams request failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Teams notification sent successfully for alert: {}", alert.name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            Err(AlertingError::HttpError(format!(
                "Teams notification failed with status {}: {}",
                status, body
            )))
        }
    }

    async fn send_pagerduty_notification(
        &self,
        integration_key: &str,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let payload = json!({
            "routing_key": integration_key,
            "event_action": "trigger",
            "dedup_key": format!("bitcraps-{}-{}", alert.category, alert.name),
            "payload": {
                "summary": format!("{}: {}", alert.name, alert.description),
                "severity": self.get_pagerduty_severity(&alert.severity),
                "source": "BitCraps Monitoring System",
                "component": alert.category,
                "group": "monitoring",
                "class": alert.metric_name,
                "custom_details": {
                    "alert_id": alert.id,
                    "metric_name": alert.metric_name,
                    "current_value": alert.current_value,
                    "threshold_value": alert.threshold_value,
                    "timestamp": alert.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                    "tags": alert.tags,
                    "severity": format!("{:?}", alert.severity)
                }
            }
        });

        let response = self
            .client
            .post("https://events.pagerduty.com/v2/enqueue")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AlertingError::HttpError(format!("PagerDuty request failed: {}", e)))?;

        if response.status().is_success() {
            debug!("PagerDuty notification sent successfully for alert: {}", alert.name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            Err(AlertingError::HttpError(format!(
                "PagerDuty notification failed with status {}: {}",
                status, body
            )))
        }
    }

    async fn send_webhook_notification(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        let mut request = self.client.post(url);

        // Add custom headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        let payload = json!({
            "alert": {
                "id": alert.id,
                "name": alert.name,
                "description": alert.description,
                "severity": alert.severity,
                "category": alert.category,
                "metric_name": alert.metric_name,
                "current_value": alert.current_value,
                "threshold_value": alert.threshold_value,
                "timestamp": alert.timestamp.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                "tags": alert.tags,
                "is_resolved": alert.is_resolved(),
                "age_seconds": alert.age_seconds()
            },
            "system": {
                "name": "BitCraps",
                "version": env!("CARGO_PKG_VERSION"),
                "environment": std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string())
            }
        });

        let response = request
            .json(&payload)
            .send()
            .await
            .map_err(|e| AlertingError::HttpError(format!("Webhook request failed: {}", e)))?;

        if response.status().is_success() {
            debug!("Webhook notification sent successfully for alert: {}", alert.name);
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "No response body".to_string());
            Err(AlertingError::HttpError(format!(
                "Webhook notification failed with status {}: {}",
                status, body
            )))
        }
    }

    async fn send_sms_notification(
        &self,
        _phone_number: &str,
        _api_config: &SMSConfig,
        alert: &Alert,
    ) -> Result<(), AlertingError> {
        // SMS implementation would integrate with provider API (Twilio, etc.)
        let message = format!(
            "🚨 BitCraps Alert: {} - {} (Severity: {:?})",
            alert.name, alert.description, alert.severity
        );
        
        info!("Would send SMS notification: {}", message);
        Ok(())
    }

    // Color and icon helpers
    fn get_slack_color(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "danger",
            AlertSeverity::High => "warning",
            AlertSeverity::Medium => "warning",
            AlertSeverity::Low => "good",
            AlertSeverity::Info => "#439FE0",
        }
    }

    fn get_slack_icon(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "🚨",
            AlertSeverity::High => "⚠️",
            AlertSeverity::Medium => "🔶",
            AlertSeverity::Low => "ℹ️",
            AlertSeverity::Info => "💡",
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

    fn get_discord_icon(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "🚨",
            AlertSeverity::High => "⚠️",
            AlertSeverity::Medium => "🔶",
            AlertSeverity::Low => "✅",
            AlertSeverity::Info => "ℹ️",
        }
    }

    fn get_teams_color(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "FF0000",
            AlertSeverity::High => "FF8C00",
            AlertSeverity::Medium => "FFD700",
            AlertSeverity::Low => "32CD32",
            AlertSeverity::Info => "439FE0",
        }
    }

    fn get_teams_icon(&self, severity: &AlertSeverity) -> &'static str {
        match severity {
            AlertSeverity::Critical => "🚨",
            AlertSeverity::High => "⚠️",
            AlertSeverity::Medium => "🔶",
            AlertSeverity::Low => "✅",
            AlertSeverity::Info => "ℹ️",
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

/// Rate limiter for notifications
pub struct NotificationRateLimiter {
    rate_limit: NotificationRateLimit,
    per_minute_counts: Arc<RwLock<HashMap<String, u32>>>,
    per_hour_counts: Arc<RwLock<HashMap<String, u32>>>,
    per_day_counts: Arc<RwLock<HashMap<String, u32>>>,
}

impl NotificationRateLimiter {
    pub fn new(rate_limit: NotificationRateLimit) -> Self {
        Self {
            rate_limit,
            per_minute_counts: Arc::new(RwLock::new(HashMap::new())),
            per_hour_counts: Arc::new(RwLock::new(HashMap::new())),
            per_day_counts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn can_send(&self, alert_name: &str) -> bool {
        // Check minute rate limit (prevent spam)
        let per_minute = self.per_minute_counts.read().await;
        let minute_count = per_minute.get(alert_name).unwrap_or(&0);
        if *minute_count >= 10 { // Max 10 per minute per alert type
            return false;
        }

        // Check hourly rate limit
        let per_hour = self.per_hour_counts.read().await;
        let hour_count = per_hour.get(alert_name).unwrap_or(&0);
        if *hour_count >= self.rate_limit.max_per_hour {
            return false;
        }

        // Check daily rate limit
        let per_day = self.per_day_counts.read().await;
        let day_count = per_day.get(alert_name).unwrap_or(&0);
        if *day_count >= self.rate_limit.max_per_day {
            return false;
        }

        true
    }

    pub async fn record_sent(&self, alert_name: &str) {
        // Increment all counters
        {
            let mut per_minute = self.per_minute_counts.write().await;
            *per_minute.entry(alert_name.to_string()).or_insert(0) += 1;
        }

        {
            let mut per_hour = self.per_hour_counts.write().await;
            *per_hour.entry(alert_name.to_string()).or_insert(0) += 1;
        }

        {
            let mut per_day = self.per_day_counts.write().await;
            *per_day.entry(alert_name.to_string()).or_insert(0) += 1;
        }

        debug!("Recorded sent notification for {}", alert_name);
    }

    /// Reset minute counters (call every minute)
    pub async fn reset_minute_counters(&self) {
        let mut per_minute = self.per_minute_counts.write().await;
        per_minute.clear();
        debug!("Reset minute counters");
    }

    /// Reset hour counters (call every hour)
    pub async fn reset_hour_counters(&self) {
        let mut per_hour = self.per_hour_counts.write().await;
        per_hour.clear();
        debug!("Reset hour counters");
    }

    /// Reset day counters (call every day)
    pub async fn reset_day_counters(&self) {
        let mut per_day = self.per_day_counts.write().await;
        per_day.clear();
        debug!("Reset day counters");
    }

    /// Get current rate limit status
    pub async fn get_status(&self) -> HashMap<String, (u32, u32, u32)> {
        let per_minute = self.per_minute_counts.read().await;
        let per_hour = self.per_hour_counts.read().await;
        let per_day = self.per_day_counts.read().await;

        let mut status = HashMap::new();
        let all_keys: std::collections::HashSet<String> = per_minute.keys()
            .chain(per_hour.keys())
            .chain(per_day.keys())
            .cloned()
            .collect();

        for key in all_keys {
            let minute = *per_minute.get(&key).unwrap_or(&0);
            let hour = *per_hour.get(&key).unwrap_or(&0);
            let day = *per_day.get(&key).unwrap_or(&0);
            status.insert(key, (minute, hour, day));
        }

        status
    }
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

    #[test]
    fn test_notification_channel_filtering() {
        let dispatcher = NotificationDispatcher::new(NotificationConfig::default());
        let alert = create_test_alert();

        // Channel with no filters should accept all alerts
        let channel = NotificationChannel {
            name: "all".to_string(),
            channel_type: NotificationChannelType::Webhook {
                url: "http://test".to_string(),
                headers: HashMap::new(),
            },
            min_severity: None,
            categories: vec![],
            required_tags: vec![],
        };
        assert!(dispatcher.should_send_to_channel(&channel, &alert));

        // Channel with severity filter
        let high_severity_channel = NotificationChannel {
            name: "high_only".to_string(),
            channel_type: NotificationChannelType::Webhook {
                url: "http://test".to_string(),
                headers: HashMap::new(),
            },
            min_severity: Some(AlertSeverity::High),
            categories: vec![],
            required_tags: vec![],
        };
        assert!(dispatcher.should_send_to_channel(&high_severity_channel, &alert));

        let critical_only_channel = NotificationChannel {
            name: "critical_only".to_string(),
            channel_type: NotificationChannelType::Webhook {
                url: "http://test".to_string(),
                headers: HashMap::new(),
            },
            min_severity: Some(AlertSeverity::Critical),
            categories: vec![],
            required_tags: vec![],
        };
        assert!(!dispatcher.should_send_to_channel(&critical_only_channel, &alert));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limit = NotificationRateLimit {
            max_per_hour: 5,
            max_per_day: 10,
        };
        let limiter = NotificationRateLimiter::new(rate_limit);

        // First few sends should be allowed
        for i in 0..5 {
            assert!(limiter.can_send("test_alert").await, "Send {} should be allowed", i);
            limiter.record_sent("test_alert").await;
        }

        // 6th send in hour should be rate limited
        assert!(!limiter.can_send("test_alert").await);

        // Different alert should still be allowed
        assert!(limiter.can_send("other_alert").await);
    }

    #[tokio::test]
    async fn test_rate_limiter_status() {
        let rate_limit = NotificationRateLimit {
            max_per_hour: 100,
            max_per_day: 1000,
        };
        let limiter = NotificationRateLimiter::new(rate_limit);

        // Record some sends
        limiter.record_sent("alert1").await;
        limiter.record_sent("alert1").await;
        limiter.record_sent("alert2").await;

        let status = limiter.get_status().await;
        assert_eq!(status.get("alert1"), Some(&(2, 2, 2))); // (minute, hour, day)
        assert_eq!(status.get("alert2"), Some(&(1, 1, 1)));
    }

    #[test]
    fn test_color_and_icon_helpers() {
        let dispatcher = NotificationDispatcher::new(NotificationConfig::default());

        // Test Slack colors
        assert_eq!(dispatcher.get_slack_color(&AlertSeverity::Critical), "danger");
        assert_eq!(dispatcher.get_slack_color(&AlertSeverity::Low), "good");

        // Test Discord colors
        assert_eq!(dispatcher.get_discord_color(&AlertSeverity::Critical), 0xFF0000);
        assert_eq!(dispatcher.get_discord_color(&AlertSeverity::Info), 0x1E90FF);

        // Test icons
        assert_eq!(dispatcher.get_slack_icon(&AlertSeverity::Critical), "🚨");
        assert_eq!(dispatcher.get_discord_icon(&AlertSeverity::High), "⚠️");
    }

    #[test]
    fn test_pagerduty_severity_mapping() {
        let dispatcher = NotificationDispatcher::new(NotificationConfig::default());

        assert_eq!(dispatcher.get_pagerduty_severity(&AlertSeverity::Critical), "critical");
        assert_eq!(dispatcher.get_pagerduty_severity(&AlertSeverity::High), "error");
        assert_eq!(dispatcher.get_pagerduty_severity(&AlertSeverity::Medium), "warning");
        assert_eq!(dispatcher.get_pagerduty_severity(&AlertSeverity::Info), "info");
    }

    #[tokio::test]
    async fn test_rate_limiter_resets() {
        let rate_limit = NotificationRateLimit {
            max_per_hour: 2,
            max_per_day: 5,
        };
        let limiter = NotificationRateLimiter::new(rate_limit);

        // Fill up the rate limit
        limiter.record_sent("test").await;
        limiter.record_sent("test").await;
        assert!(!limiter.can_send("test").await);

        // Reset hour counters should allow more sends
        limiter.reset_hour_counters().await;
        assert!(limiter.can_send("test").await);
    }
}