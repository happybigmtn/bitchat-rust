//! Alert Condition Evaluation for BitCraps Monitoring
//!
//! This module handles the evaluation of alert rules against system metrics
//! and determines when alerts should be triggered.

use std::collections::HashMap;
use std::sync::{atomic::Ordering, Arc};
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::alert_types::*;
use crate::monitoring::metrics::METRICS;

/// Alert rules engine for evaluating conditions
pub struct AlertRulesEngine {
    rules: Vec<AlertRule>,
    last_evaluation: Arc<RwLock<HashMap<String, SystemTime>>>,
    metric_cache: Arc<RwLock<HashMap<String, MetricValue>>>,
}

/// Cached metric value with timestamp
#[derive(Debug, Clone)]
struct MetricValue {
    value: f64,
    timestamp: SystemTime,
    ttl_seconds: u64,
}

impl AlertRulesEngine {
    /// Create new alert rules engine
    pub fn new(rules: Vec<AlertRule>) -> Self {
        Self {
            rules,
            last_evaluation: Arc::new(RwLock::new(HashMap::new())),
            metric_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new alert rule
    pub async fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    /// Remove an alert rule by name
    pub async fn remove_rule(&mut self, rule_name: &str) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|rule| rule.name != rule_name);
        self.rules.len() != initial_len
    }

    /// Update an existing alert rule
    pub async fn update_rule(&mut self, rule_name: &str, new_rule: AlertRule) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.name == rule_name) {
            *rule = new_rule;
            true
        } else {
            false
        }
    }

    /// Get all alert rules
    pub fn get_rules(&self) -> &[AlertRule] {
        &self.rules
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

        debug!("Evaluated {} rules, triggered {} alerts", self.rules.len(), triggered_alerts.len());
        Ok(triggered_alerts)
    }

    /// Evaluate rules for specific metrics only
    pub async fn evaluate_rules_for_metrics(&self, metric_names: &[String]) -> Result<Vec<Alert>, AlertingError> {
        let mut triggered_alerts = Vec::new();

        for rule in &self.rules {
            if metric_names.contains(&rule.metric_name) {
                if self.should_evaluate_rule(rule).await? {
                    if let Some(alert) = self.evaluate_rule(rule).await? {
                        triggered_alerts.push(alert);
                        self.last_evaluation
                            .write()
                            .await
                            .insert(rule.name.clone(), SystemTime::now());
                    }
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
                .unwrap_or_default();

            Ok(elapsed >= rule.evaluation_interval)
        } else {
            Ok(true) // First evaluation
        }
    }

    /// Evaluate individual rule against current metrics
    async fn evaluate_rule(&self, rule: &AlertRule) -> Result<Option<Alert>, AlertingError> {
        let current_value = self.get_metric_value(&rule.metric_name).await?;

        if rule.condition.evaluate(current_value) {
            let alert = Alert::new(
                rule.name.clone(),
                rule.description.clone(),
                rule.severity,
                rule.category.clone(),
                rule.metric_name.clone(),
                current_value,
                rule.condition.primary_threshold(),
            )
            .with_tags(rule.tags.clone());

            debug!("Alert triggered: {} ({}={})", rule.name, rule.metric_name, current_value);
            Ok(Some(alert))
        } else {
            Ok(None)
        }
    }

    /// Get current value of a metric with caching
    async fn get_metric_value(&self, metric_name: &str) -> Result<f64, AlertingError> {
        // Check cache first
        {
            let cache = self.metric_cache.read().await;
            if let Some(cached) = cache.get(metric_name) {
                if cached.timestamp.elapsed().unwrap_or_default().as_secs() < cached.ttl_seconds {
                    return Ok(cached.value);
                }
            }
        }

        // Get fresh value
        let value = self.fetch_metric_value(metric_name).await?;
        
        // Update cache
        {
            let mut cache = self.metric_cache.write().await;
            cache.insert(metric_name.to_string(), MetricValue {
                value,
                timestamp: SystemTime::now(),
                ttl_seconds: 30, // Cache for 30 seconds
            });
        }

        Ok(value)
    }

    /// Fetch fresh metric value from the monitoring system
    async fn fetch_metric_value(&self, metric_name: &str) -> Result<f64, AlertingError> {
        match metric_name {
            // System resources
            "cpu_usage_percent" => {
                Ok(METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64)
            }
            "memory_usage_mb" => Ok(
                (METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024) as f64,
            ),
            "memory_usage_percent" => {
                let used_mb = METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024;
                let total_bytes = METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed);
                let total_mb = total_bytes / (1024 * 1024);
                if total_mb > 0 {
                    Ok((used_mb as f64 / total_mb as f64) * 100.0)
                } else {
                    Ok(0.0)
                }
            }
            "disk_usage_percent" => {
                let disk_bytes = METRICS.resources.disk_usage_bytes.load(Ordering::Relaxed);
                let memory_bytes = METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed);
                // Estimate disk usage as percentage relative to memory usage
                Ok((disk_bytes as f64 / memory_bytes.max(1) as f64) * 100.0)
            }
            "disk_usage_gb" => {
                Ok(METRICS.resources.disk_usage_bytes.load(Ordering::Relaxed) as f64 / 1024.0 / 1024.0 / 1024.0)
            }

            // Network metrics
            "active_connections" => {
                Ok(METRICS.network.active_connections.load(Ordering::Relaxed) as f64)
            }
            "messages_sent_rate" => {
                Ok(METRICS.network.messages_sent.load(Ordering::Relaxed) as f64 / 60.0) // per minute
            }
            "messages_received_rate" => {
                Ok(METRICS.network.messages_received.load(Ordering::Relaxed) as f64 / 60.0)
            }
            "bytes_sent_mb" => {
                Ok(METRICS.network.bytes_sent.load(Ordering::Relaxed) as f64 / 1024.0 / 1024.0)
            }
            "bytes_received_mb" => {
                Ok(METRICS.network.bytes_received.load(Ordering::Relaxed) as f64 / 1024.0 / 1024.0)
            }

            // Consensus metrics
            "consensus_latency_ms" => Ok(METRICS.consensus.average_latency_ms()),
            "consensus_throughput" => {
                Ok(METRICS.consensus.proposals_submitted.load(Ordering::Relaxed) as f64)
            }
            "pending_transactions" => {
                Ok((METRICS.consensus.proposals_submitted.load(Ordering::Relaxed) - METRICS.consensus.proposals_accepted.load(Ordering::Relaxed)) as f64)
            }
            "consensus_rounds" => {
                Ok(METRICS.consensus.consensus_rounds.load(Ordering::Relaxed) as f64)
            }

            // Error metrics
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
            "critical_errors" => {
                Ok(METRICS.errors.critical_errors.load(Ordering::Relaxed) as f64)
            }
            "warning_count" => {
                Ok((METRICS.errors.total_errors.load(Ordering::Relaxed) - METRICS.errors.critical_errors.load(Ordering::Relaxed)) as f64)
            }

            // Gaming metrics
            "active_games" => {
                Ok(METRICS.gaming.active_games.load(Ordering::Relaxed) as f64)
            }
            "total_players" => {
                Ok(METRICS.gaming.active_games.load(Ordering::Relaxed) as f64)
            }
            "games_per_hour" => {
                Ok(METRICS.gaming.total_games.load(Ordering::Relaxed) as f64)
            }

            // Custom computed metrics
            "system_load_score" => {
                self.calculate_system_load_score().await
            }
            "health_score" => {
                self.calculate_health_score().await
            }
            "performance_index" => {
                self.calculate_performance_index().await
            }

            _ => Err(AlertingError::UnknownMetric(metric_name.to_string())),
        }
    }

    /// Calculate composite system load score (0-100)
    async fn calculate_system_load_score(&self) -> Result<f64, AlertingError> {
        let cpu = METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64;
        let memory_bytes = METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed);
        let memory = (memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)) * 100.0; // Convert to percentage
        let disk_bytes = METRICS.resources.disk_usage_bytes.load(Ordering::Relaxed);
        let disk = (disk_bytes as f64 / memory_bytes.max(1) as f64) * 100.0;
        
        // Weighted average: CPU 40%, Memory 40%, Disk 20%
        let score = (cpu * 0.4) + (memory * 0.4) + (disk * 0.2);
        Ok(score.min(100.0))
    }

    /// Calculate system health score (0-100, higher is better)
    async fn calculate_health_score(&self) -> Result<f64, AlertingError> {
        let error_rate = METRICS.errors.total_errors.load(Ordering::Relaxed) as f64;
        let load_score = self.calculate_system_load_score().await.unwrap_or(0.0);
        let connections = METRICS.network.active_connections.load(Ordering::Relaxed) as f64;
        
        // Health decreases with errors and high load, increases with connectivity
        let health_score = 100.0 
            - (error_rate * 10.0).min(50.0)     // Errors reduce health significantly
            - (load_score * 0.3).min(30.0)      // High load reduces health
            + (connections * 2.0).min(20.0);    // Good connectivity improves health
        
        Ok(health_score.max(0.0).min(100.0))
    }

    /// Calculate performance index (0-100, higher is better)
    async fn calculate_performance_index(&self) -> Result<f64, AlertingError> {
        let latency = METRICS.consensus.latency_samples.read().average();
        let throughput = METRICS.consensus.proposals_accepted.load(Ordering::Relaxed) as f64;
        let cpu_available = 100.0 - METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64;
        
        // Performance increases with low latency, high throughput, and available CPU
        let performance_index = (cpu_available * 0.4) 
            + ((1000.0 - latency.min(1000.0)) / 10.0 * 0.3)  // Lower latency is better
            + (throughput.min(100.0) * 0.3);                 // Higher throughput is better
        
        Ok(performance_index.max(0.0).min(100.0))
    }

    /// Clear the metric cache
    pub async fn clear_cache(&self) {
        let mut cache = self.metric_cache.write().await;
        cache.clear();
        debug!("Metric cache cleared");
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> HashMap<String, (f64, u64)> {
        let cache = self.metric_cache.read().await;
        cache.iter()
            .map(|(name, metric)| {
                let age_seconds = metric.timestamp.elapsed().unwrap_or_default().as_secs();
                (name.clone(), (metric.value, age_seconds))
            })
            .collect()
    }

    /// Force refresh of specific metrics
    pub async fn refresh_metrics(&self, metric_names: &[String]) -> Result<(), AlertingError> {
        let mut cache = self.metric_cache.write().await;
        
        for metric_name in metric_names {
            cache.remove(metric_name);
        }
        
        // Pre-fetch the metrics
        drop(cache);
        for metric_name in metric_names {
            if let Err(e) = self.get_metric_value(metric_name).await {
                warn!("Failed to refresh metric {}: {}", metric_name, e);
            }
        }
        
        Ok(())
    }
}

/// Metric evaluation context for custom rules
pub struct EvaluationContext {
    pub current_time: SystemTime,
    pub metric_values: HashMap<String, f64>,
    pub historical_values: HashMap<String, Vec<f64>>,
}

impl EvaluationContext {
    /// Create new evaluation context
    pub fn new() -> Self {
        Self {
            current_time: SystemTime::now(),
            metric_values: HashMap::new(),
            historical_values: HashMap::new(),
        }
    }

    /// Add a metric value
    pub fn add_metric(&mut self, name: String, value: f64) {
        self.metric_values.insert(name, value);
    }

    /// Add historical values for a metric
    pub fn add_historical(&mut self, name: String, values: Vec<f64>) {
        self.historical_values.insert(name, values);
    }

    /// Get metric value
    pub fn get_metric(&self, name: &str) -> Option<f64> {
        self.metric_values.get(name).copied()
    }

    /// Get average of last N values for a metric
    pub fn get_average(&self, name: &str, count: usize) -> Option<f64> {
        self.historical_values.get(name).and_then(|values| {
            if values.is_empty() {
                None
            } else {
                let recent: Vec<f64> = values.iter().rev().take(count).copied().collect();
                Some(recent.iter().sum::<f64>() / recent.len() as f64)
            }
        })
    }

    /// Check if metric is trending upward
    pub fn is_trending_up(&self, name: &str, min_points: usize) -> bool {
        self.historical_values.get(name)
            .map(|values| {
                if values.len() < min_points {
                    false
                } else {
                    let recent: Vec<f64> = values.iter().rev().take(min_points).copied().collect();
                    recent.windows(2).all(|w| w[0] >= w[1])
                }
            })
            .unwrap_or(false)
    }

    /// Check if metric is trending downward
    pub fn is_trending_down(&self, name: &str, min_points: usize) -> bool {
        self.historical_values.get(name)
            .map(|values| {
                if values.len() < min_points {
                    false
                } else {
                    let recent: Vec<f64> = values.iter().rev().take(min_points).copied().collect();
                    recent.windows(2).all(|w| w[0] <= w[1])
                }
            })
            .unwrap_or(false)
    }
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_alert_rules_engine() {
        let rules = vec![
            AlertRule::new(
                "Test Rule".to_string(),
                "cpu_usage_percent".to_string(),
                AlertCondition::GreaterThan(50.0),
                AlertSeverity::Medium,
            )
            .with_interval(Duration::from_secs(10)),
        ];

        let engine = AlertRulesEngine::new(rules);
        assert_eq!(engine.get_rules().len(), 1);
    }

    #[tokio::test]
    async fn test_rule_evaluation_cooldown() {
        let rules = vec![
            AlertRule::new(
                "Test Rule".to_string(),
                "cpu_usage_percent".to_string(),
                AlertCondition::GreaterThan(0.0), // Always triggers
                AlertSeverity::Low,
            )
            .with_interval(Duration::from_secs(60)),
        ];

        let engine = AlertRulesEngine::new(rules);
        
        // First evaluation should be allowed
        assert!(engine.should_evaluate_rule(&engine.rules[0]).await.unwrap());
        
        // Mark as evaluated
        engine.last_evaluation.write().await.insert(
            "Test Rule".to_string(), 
            SystemTime::now()
        );
        
        // Second evaluation should be blocked by cooldown
        assert!(!engine.should_evaluate_rule(&engine.rules[0]).await.unwrap());
    }

    #[tokio::test]
    async fn test_rule_management() {
        let mut engine = AlertRulesEngine::new(vec![]);
        
        let rule = AlertRule::new(
            "CPU Alert".to_string(),
            "cpu_usage_percent".to_string(),
            AlertCondition::GreaterThan(80.0),
            AlertSeverity::High,
        );

        // Add rule
        engine.add_rule(rule.clone()).await;
        assert_eq!(engine.get_rules().len(), 1);

        // Update rule
        let updated_rule = AlertRule::new(
            "CPU Alert".to_string(),
            "cpu_usage_percent".to_string(),
            AlertCondition::GreaterThan(90.0),
            AlertSeverity::Critical,
        );
        
        assert!(engine.update_rule("CPU Alert", updated_rule).await);
        assert_eq!(engine.get_rules()[0].severity, AlertSeverity::Critical);

        // Remove rule
        assert!(engine.remove_rule("CPU Alert").await);
        assert_eq!(engine.get_rules().len(), 0);
    }

    #[test]
    fn test_evaluation_context() {
        let mut context = EvaluationContext::new();
        
        context.add_metric("cpu".to_string(), 75.0);
        context.add_historical("cpu".to_string(), vec![60.0, 65.0, 70.0, 75.0]);
        
        assert_eq!(context.get_metric("cpu"), Some(75.0));
        assert_eq!(context.get_average("cpu", 4), Some(67.5));
        assert!(context.is_trending_up("cpu", 3));
        assert!(!context.is_trending_down("cpu", 3));
    }

    #[test]
    fn test_complex_conditions() {
        let between_condition = AlertCondition::Between(20.0, 80.0);
        assert!(between_condition.evaluate(50.0));
        assert!(!between_condition.evaluate(90.0));
        assert!(!between_condition.evaluate(10.0));

        let outside_condition = AlertCondition::Outside(20.0, 80.0);
        assert!(!outside_condition.evaluate(50.0));
        assert!(outside_condition.evaluate(90.0));
        assert!(outside_condition.evaluate(10.0));
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let engine = AlertRulesEngine::new(vec![]);
        
        // Cache should be empty initially
        let stats = engine.get_cache_stats().await;
        assert!(stats.is_empty());
        
        // Clear cache should work even when empty
        engine.clear_cache().await;
        
        // Refresh non-existent metrics should handle gracefully
        let result = engine.refresh_metrics(&["nonexistent_metric".to_string()]).await;
        assert!(result.is_ok());
    }
}