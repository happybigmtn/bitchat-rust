//! Alert State Management for BitCraps Monitoring
//!
//! This module handles the tracking and management of alert states,
//! including deduplication, history, and lifecycle management.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::alert_types::*;

/// Alert state manager for tracking active alerts and history
pub struct AlertStateManager {
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    alert_fingerprints: Arc<RwLock<HashMap<String, AlertFingerprint>>>,
    resolved_alerts: Arc<RwLock<VecDeque<Alert>>>,
    alert_counts: Arc<RwLock<AlertCounts>>,
    config: AlertStateConfig,
}

/// Alert fingerprint for deduplication
#[derive(Debug, Clone)]
struct AlertFingerprint {
    fingerprint: String,
    first_seen: SystemTime,
    last_seen: SystemTime,
    occurrence_count: u32,
}

/// Alert state configuration
#[derive(Debug, Clone)]
pub struct AlertStateConfig {
    pub dedup_window_minutes: u32,
    pub max_resolved_history: usize,
    pub auto_resolve_timeout_minutes: Option<u32>,
    pub cleanup_interval_minutes: u32,
}

impl Default for AlertStateConfig {
    fn default() -> Self {
        Self {
            dedup_window_minutes: 5,
            max_resolved_history: 1000,
            auto_resolve_timeout_minutes: Some(60), // Auto-resolve after 1 hour if no updates
            cleanup_interval_minutes: 15,
        }
    }
}

/// Counters for alert statistics
#[derive(Debug, Default)]
struct AlertCounts {
    total_processed: u64,
    total_duplicates_suppressed: u64,
    total_auto_resolved: u64,
    alerts_by_severity: HashMap<AlertSeverity, u64>,
    alerts_by_category: HashMap<String, u64>,
}

impl AlertStateManager {
    /// Create new alert state manager
    pub fn new(config: AlertStateConfig) -> Self {
        Self {
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_fingerprints: Arc::new(RwLock::new(HashMap::new())),
            resolved_alerts: Arc::new(RwLock::new(VecDeque::new())),
            alert_counts: Arc::new(RwLock::new(AlertCounts::default())),
            config,
        }
    }

    /// Add active alert with deduplication
    pub async fn add_active_alert(&self, alert: Alert) -> AlertAddResult {
        let fingerprint = self.calculate_fingerprint(&alert);

        // Check for duplicates
        {
            let mut fingerprints = self.alert_fingerprints.write().await;
            if let Some(existing) = fingerprints.get_mut(&fingerprint) {
                let time_since_last = SystemTime::now()
                    .duration_since(existing.last_seen)
                    .unwrap_or_default();

                if time_since_last < Duration::from_secs(self.config.dedup_window_minutes as u64 * 60) {
                    // This is a duplicate within the dedup window
                    existing.last_seen = SystemTime::now();
                    existing.occurrence_count += 1;

                    // Update counts
                    let mut counts = self.alert_counts.write().await;
                    counts.total_duplicates_suppressed += 1;

                    debug!(
                        "Suppressed duplicate alert: {} (occurrence: {})",
                        alert.name, existing.occurrence_count
                    );
                    return AlertAddResult::Suppressed {
                        fingerprint: fingerprint.clone(),
                        occurrence_count: existing.occurrence_count,
                    };
                }
            }

            // Not a duplicate or outside dedup window, record the fingerprint
            fingerprints.insert(fingerprint.clone(), AlertFingerprint {
                fingerprint: fingerprint.clone(),
                first_seen: SystemTime::now(),
                last_seen: SystemTime::now(),
                occurrence_count: 1,
            });
        }

        // Add to active alerts
        {
            let mut active = self.active_alerts.write().await;
            active.insert(alert.id.clone(), alert.clone());
        }

        // Update statistics
        {
            let mut counts = self.alert_counts.write().await;
            counts.total_processed += 1;
            *counts.alerts_by_severity.entry(alert.severity).or_insert(0) += 1;
            *counts.alerts_by_category.entry(alert.category.clone()).or_insert(0) += 1;
        }

        info!("Added active alert: {} ({})", alert.name, alert.id);
        AlertAddResult::Added { alert_id: alert.id }
    }

    /// Resolve an active alert
    pub async fn resolve_alert(&self, alert_id: &str) -> bool {
        let mut alert = {
            let mut active = self.active_alerts.write().await;
            match active.remove(alert_id) {
                Some(alert) => alert,
                None => {
                    warn!("Attempted to resolve non-existent alert: {}", alert_id);
                    return false;
                }
            }
        };

        // Mark as resolved
        alert.resolve();

        // Add to resolved history
        {
            let mut resolved = self.resolved_alerts.write().await;
            resolved.push_back(alert.clone());

            // Maintain history size limit
            while resolved.len() > self.config.max_resolved_history {
                resolved.pop_front();
            }
        }

        info!("Resolved alert: {} ({})", alert.name, alert.id);
        true
    }

    /// Auto-resolve alerts that have been active too long without updates
    pub async fn auto_resolve_stale_alerts(&self) -> Vec<Alert> {
        if self.config.auto_resolve_timeout_minutes.is_none() {
            return Vec::new();
        }

        let timeout_minutes = self.config.auto_resolve_timeout_minutes.unwrap();
        let timeout_duration = Duration::from_secs(timeout_minutes as u64 * 60);
        let cutoff_time = SystemTime::now() - timeout_duration;

        let mut auto_resolved = Vec::new();
        let alert_ids_to_resolve: Vec<String> = {
            let active = self.active_alerts.read().await;
            active
                .values()
                .filter(|alert| alert.timestamp < cutoff_time)
                .map(|alert| alert.id.clone())
                .collect()
        };

        for alert_id in alert_ids_to_resolve {
            if self.resolve_alert(&alert_id).await {
                if let Some(alert) = self.get_resolved_alert(&alert_id).await {
                    auto_resolved.push(alert);
                }
            }
        }

        if !auto_resolved.is_empty() {
            let mut counts = self.alert_counts.write().await;
            counts.total_auto_resolved += auto_resolved.len() as u64;
            
            info!("Auto-resolved {} stale alerts", auto_resolved.len());
        }

        auto_resolved
    }

    /// Get all active alerts
    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active = self.active_alerts.read().await;
        active.values().cloned().collect()
    }

    /// Get active alerts by severity
    pub async fn get_active_alerts_by_severity(&self, severity: AlertSeverity) -> Vec<Alert> {
        let active = self.active_alerts.read().await;
        active
            .values()
            .filter(|alert| alert.severity == severity)
            .cloned()
            .collect()
    }

    /// Get active alerts by category
    pub async fn get_active_alerts_by_category(&self, category: &str) -> Vec<Alert> {
        let active = self.active_alerts.read().await;
        active
            .values()
            .filter(|alert| alert.category == category)
            .cloned()
            .collect()
    }

    /// Get recently resolved alerts
    pub async fn get_recent_resolved_alerts(&self, count: usize) -> Vec<Alert> {
        let resolved = self.resolved_alerts.read().await;
        resolved
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    /// Get specific resolved alert by ID
    pub async fn get_resolved_alert(&self, alert_id: &str) -> Option<Alert> {
        let resolved = self.resolved_alerts.read().await;
        resolved
            .iter()
            .find(|alert| alert.id == alert_id)
            .cloned()
    }

    /// Check if alert is duplicate
    pub async fn is_duplicate(&self, alert: &Alert) -> bool {
        let fingerprint = self.calculate_fingerprint(alert);
        let fingerprints = self.alert_fingerprints.read().await;

        if let Some(existing) = fingerprints.get(&fingerprint) {
            let time_since_last = SystemTime::now()
                .duration_since(existing.last_seen)
                .unwrap_or_default();

            time_since_last < Duration::from_secs(self.config.dedup_window_minutes as u64 * 60)
        } else {
            false
        }
    }

    /// Get alert statistics
    pub async fn get_statistics(&self) -> AlertStatistics {
        let counts = self.alert_counts.read().await;
        let active = self.active_alerts.read().await;
        let resolved = self.resolved_alerts.read().await;

        // Calculate average resolution time
        let resolution_times: Vec<u64> = resolved
            .iter()
            .filter_map(|alert| alert.resolution_time_seconds())
            .collect();

        let average_resolution_time_minutes = if resolution_times.is_empty() {
            0.0
        } else {
            resolution_times.iter().sum::<u64>() as f64 / resolution_times.len() as f64 / 60.0
        };

        // Calculate false positive rate (simplified)
        let false_positive_rate = if counts.total_processed > 0 {
            (counts.total_auto_resolved as f64 / counts.total_processed as f64) * 100.0
        } else {
            0.0
        };

        AlertStatistics {
            total_alerts_processed: counts.total_processed as usize,
            active_alerts: active.len(),
            alerts_last_hour: self.count_alerts_in_last_hours(1).await,
            alerts_last_24_hours: self.count_alerts_in_last_hours(24).await,
            alerts_by_severity: counts.alerts_by_severity.iter().map(|(k, v)| (*k, *v as usize)).collect(),
            alerts_by_category: counts.alerts_by_category.iter().map(|(k, v)| (k.clone(), *v as usize)).collect(),
            average_resolution_time_minutes,
            false_positive_rate,
        }
    }

    /// Get current alert status
    pub async fn get_alert_status(&self) -> AlertStatus {
        let active = self.active_alerts.read().await;
        let active_count = active.len();
        let critical_count = active
            .values()
            .filter(|alert| alert.severity == AlertSeverity::Critical)
            .count();

        let health = if critical_count > 0 {
            SystemHealth::Critical
        } else if active_count > 10 {
            SystemHealth::Warning
        } else {
            SystemHealth::Healthy
        };

        AlertStatus {
            active_alerts: active_count,
            critical_alerts: critical_count,
            total_alerts_last_24h: self.count_alerts_in_last_hours(24).await,
            system_health: health,
        }
    }

    /// Count alerts in the last N hours (from resolved + active)
    async fn count_alerts_in_last_hours(&self, hours: u32) -> usize {
        let cutoff = SystemTime::now() - Duration::from_secs(hours as u64 * 3600);
        let mut count = 0;

        // Count active alerts
        {
            let active = self.active_alerts.read().await;
            count += active.values().filter(|alert| alert.timestamp > cutoff).count();
        }

        // Count resolved alerts
        {
            let resolved = self.resolved_alerts.read().await;
            count += resolved.iter().filter(|alert| alert.timestamp > cutoff).count();
        }

        count
    }

    /// Cleanup old fingerprints and resolved alerts
    pub async fn cleanup_old_data(&self) {
        let now = SystemTime::now();
        let fingerprint_retention = Duration::from_secs(self.config.dedup_window_minutes as u64 * 60 * 2);

        // Clean up old fingerprints
        {
            let mut fingerprints = self.alert_fingerprints.write().await;
            fingerprints.retain(|_, fp| {
                now.duration_since(fp.last_seen).unwrap_or_default() < fingerprint_retention
            });
            debug!("Cleaned up fingerprints, {} remaining", fingerprints.len());
        }

        // Resolved alerts are already size-limited, but we can clean up very old ones
        {
            let mut resolved = self.resolved_alerts.write().await;
            let retention_days = 7; // Keep resolved alerts for 7 days
            let retention_duration = Duration::from_secs(retention_days * 24 * 3600);
            let cutoff = now - retention_duration;

            let initial_len = resolved.len();
            resolved.retain(|alert| alert.timestamp > cutoff);
            
            if resolved.len() != initial_len {
                debug!(
                    "Cleaned up resolved alerts: {} -> {}",
                    initial_len,
                    resolved.len()
                );
            }
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
        alert.severity.hash(&mut hasher);
        
        // Include threshold but not current value for better deduplication
        ((alert.threshold_value * 100.0) as i64).hash(&mut hasher);
        
        format!("{:x}", hasher.finish())
    }

    /// Get deduplication statistics
    pub async fn get_dedup_stats(&self) -> DedupStats {
        let fingerprints = self.alert_fingerprints.read().await;
        let counts = self.alert_counts.read().await;

        DedupStats {
            unique_fingerprints: fingerprints.len(),
            total_duplicates_suppressed: counts.total_duplicates_suppressed,
            dedup_window_minutes: self.config.dedup_window_minutes,
            most_frequent_alerts: fingerprints
                .values()
                .filter(|fp| fp.occurrence_count > 1)
                .map(|fp| (fp.fingerprint.clone(), fp.occurrence_count))
                .collect(),
        }
    }

    /// Force cleanup of all data (for testing or maintenance)
    pub async fn force_cleanup(&self) {
        {
            let mut active = self.active_alerts.write().await;
            active.clear();
        }
        {
            let mut fingerprints = self.alert_fingerprints.write().await;
            fingerprints.clear();
        }
        {
            let mut resolved = self.resolved_alerts.write().await;
            resolved.clear();
        }
        {
            let mut counts = self.alert_counts.write().await;
            *counts = AlertCounts::default();
        }

        info!("Force cleaned up all alert state data");
    }
}

impl Default for AlertStateManager {
    fn default() -> Self {
        Self::new(AlertStateConfig::default())
    }
}

/// Result of adding an alert
#[derive(Debug)]
pub enum AlertAddResult {
    Added { alert_id: String },
    Suppressed { fingerprint: String, occurrence_count: u32 },
}

/// Deduplication statistics
#[derive(Debug)]
pub struct DedupStats {
    pub unique_fingerprints: usize,
    pub total_duplicates_suppressed: u64,
    pub dedup_window_minutes: u32,
    pub most_frequent_alerts: Vec<(String, u32)>,
}

/// Alert history storage with retention
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
        self.cleanup_old_alerts();
    }

    pub fn cleanup_old_alerts(&mut self) {
        let cutoff = SystemTime::now() - Duration::from_secs(self.retention_days as u64 * 24 * 3600);
        while let Some(front) = self.alerts.front() {
            if front.timestamp <= cutoff {
                self.alerts.pop_front();
            } else {
                break;
            }
        }
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
                    .unwrap_or_default()
            })
            .sum();

        total_time.as_secs_f64() / 60.0 / resolved_alerts.len() as f64
    }

    pub fn calculate_false_positive_rate(&self) -> f64 {
        // Simplified false positive calculation
        // In practice, this would require more sophisticated analysis
        let total = self.alerts.len();
        if total == 0 {
            return 0.0;
        }

        let auto_resolved = self
            .alerts
            .iter()
            .filter(|alert| {
                // Consider alerts that were resolved very quickly as potential false positives
                alert.resolution_time_seconds().map_or(false, |t| t < 60) // Resolved in < 1 minute
            })
            .count();

        (auto_resolved as f64 / total as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_alert(name: &str, severity: AlertSeverity) -> Alert {
        Alert::new(
            name.to_string(),
            "Test alert".to_string(),
            severity,
            "test".to_string(),
            "test_metric".to_string(),
            100.0,
            80.0,
        )
    }

    #[tokio::test]
    async fn test_alert_state_manager_basic() {
        let manager = AlertStateManager::new(AlertStateConfig::default());
        let alert = create_test_alert("Test Alert", AlertSeverity::High);
        let alert_id = alert.id.clone();

        // Add alert
        let result = manager.add_active_alert(alert).await;
        assert!(matches!(result, AlertAddResult::Added { .. }));

        // Check active alerts
        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Test Alert");

        // Resolve alert
        assert!(manager.resolve_alert(&alert_id).await);

        // Check active alerts (should be empty)
        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 0);

        // Check resolved alerts
        let resolved = manager.get_recent_resolved_alerts(10).await;
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].is_resolved());
    }

    #[tokio::test]
    async fn test_alert_deduplication() {
        let config = AlertStateConfig {
            dedup_window_minutes: 5,
            ..Default::default()
        };
        let manager = AlertStateManager::new(config);

        let alert1 = create_test_alert("Duplicate Alert", AlertSeverity::Medium);
        let alert2 = create_test_alert("Duplicate Alert", AlertSeverity::Medium);

        // First alert should be added
        let result1 = manager.add_active_alert(alert1).await;
        assert!(matches!(result1, AlertAddResult::Added { .. }));

        // Second identical alert should be suppressed
        let result2 = manager.add_active_alert(alert2).await;
        assert!(matches!(result2, AlertAddResult::Suppressed { occurrence_count: 2, .. }));

        // Only one active alert should exist
        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn test_alert_filtering_by_severity() {
        let manager = AlertStateManager::new(AlertStateConfig::default());

        let critical = create_test_alert("Critical", AlertSeverity::Critical);
        let high = create_test_alert("High", AlertSeverity::High);
        let low = create_test_alert("Low", AlertSeverity::Low);

        manager.add_active_alert(critical).await;
        manager.add_active_alert(high).await;
        manager.add_active_alert(low).await;

        let critical_alerts = manager.get_active_alerts_by_severity(AlertSeverity::Critical).await;
        assert_eq!(critical_alerts.len(), 1);
        assert_eq!(critical_alerts[0].name, "Critical");

        let high_alerts = manager.get_active_alerts_by_severity(AlertSeverity::High).await;
        assert_eq!(high_alerts.len(), 1);
        assert_eq!(high_alerts[0].name, "High");
    }

    #[tokio::test]
    async fn test_alert_filtering_by_category() {
        let manager = AlertStateManager::new(AlertStateConfig::default());

        let mut perf_alert = create_test_alert("Performance Alert", AlertSeverity::High);
        perf_alert.category = "performance".to_string();

        let mut security_alert = create_test_alert("Security Alert", AlertSeverity::Critical);
        security_alert.category = "security".to_string();

        manager.add_active_alert(perf_alert).await;
        manager.add_active_alert(security_alert).await;

        let perf_alerts = manager.get_active_alerts_by_category("performance").await;
        assert_eq!(perf_alerts.len(), 1);
        assert_eq!(perf_alerts[0].name, "Performance Alert");

        let security_alerts = manager.get_active_alerts_by_category("security").await;
        assert_eq!(security_alerts.len(), 1);
        assert_eq!(security_alerts[0].name, "Security Alert");
    }

    #[tokio::test]
    async fn test_auto_resolve_stale_alerts() {
        let config = AlertStateConfig {
            auto_resolve_timeout_minutes: Some(1), // 1 minute timeout for testing
            ..Default::default()
        };
        let manager = AlertStateManager::new(config);

        let mut old_alert = create_test_alert("Old Alert", AlertSeverity::Medium);
        old_alert.timestamp = SystemTime::now() - Duration::from_secs(120); // 2 minutes ago

        manager.add_active_alert(old_alert).await;

        let auto_resolved = manager.auto_resolve_stale_alerts().await;
        assert_eq!(auto_resolved.len(), 1);
        assert_eq!(auto_resolved[0].name, "Old Alert");

        // Alert should no longer be active
        let active = manager.get_active_alerts().await;
        assert_eq!(active.len(), 0);
    }

    #[tokio::test]
    async fn test_alert_statistics() {
        let manager = AlertStateManager::new(AlertStateConfig::default());

        let high_alert = create_test_alert("High Alert", AlertSeverity::High);
        let critical_alert = create_test_alert("Critical Alert", AlertSeverity::Critical);

        manager.add_active_alert(high_alert).await;
        manager.add_active_alert(critical_alert).await;

        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_alerts_processed, 2);
        assert_eq!(stats.active_alerts, 2);
        assert!(stats.alerts_by_severity.contains_key(&AlertSeverity::High));
        assert!(stats.alerts_by_severity.contains_key(&AlertSeverity::Critical));
    }

    #[tokio::test]
    async fn test_alert_status() {
        let manager = AlertStateManager::new(AlertStateConfig::default());

        let critical = create_test_alert("Critical", AlertSeverity::Critical);
        let high = create_test_alert("High", AlertSeverity::High);

        manager.add_active_alert(critical).await;
        manager.add_active_alert(high).await;

        let status = manager.get_alert_status().await;
        assert_eq!(status.active_alerts, 2);
        assert_eq!(status.critical_alerts, 1);
        assert!(matches!(status.system_health, SystemHealth::Critical));
    }

    #[test]
    fn test_alert_history() {
        let mut history = AlertHistory::new(7); // 7 days retention
        
        let alert1 = create_test_alert("Alert 1", AlertSeverity::High);
        let alert2 = create_test_alert("Alert 2", AlertSeverity::Medium);
        
        history.add_alert(alert1);
        history.add_alert(alert2);
        
        assert_eq!(history.total_count(), 2);
        
        let by_severity = history.count_by_severity();
        assert_eq!(by_severity.get(&AlertSeverity::High), Some(&1));
        assert_eq!(by_severity.get(&AlertSeverity::Medium), Some(&1));
        
        let by_category = history.count_by_category();
        assert_eq!(by_category.get("test"), Some(&2));
    }

    #[tokio::test]
    async fn test_cleanup_operations() {
        let manager = AlertStateManager::new(AlertStateConfig::default());

        // Add some alerts
        let alert1 = create_test_alert("Alert 1", AlertSeverity::High);
        let alert2 = create_test_alert("Alert 2", AlertSeverity::Medium);
        
        manager.add_active_alert(alert1).await;
        manager.add_active_alert(alert2).await;

        // Verify they exist
        assert_eq!(manager.get_active_alerts().await.len(), 2);

        // Force cleanup
        manager.force_cleanup().await;

        // Verify everything is cleaned up
        assert_eq!(manager.get_active_alerts().await.len(), 0);
        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_alerts_processed, 0);
    }
}