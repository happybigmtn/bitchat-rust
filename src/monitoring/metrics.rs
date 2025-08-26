//! Comprehensive monitoring and metrics collection for production deployment

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::collections::VecDeque;
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};

/// System-wide metrics collector
pub struct MetricsCollector {
    /// Network metrics
    pub network: NetworkMetrics,
    /// Consensus metrics
    pub consensus: ConsensusMetrics,
    /// Gaming metrics
    pub gaming: GamingMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Resource metrics
    pub resources: ResourceMetrics,
    /// Error tracking
    pub errors: ErrorMetrics,
    /// Start time for uptime calculation
    start_time: Instant,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            network: NetworkMetrics::new(),
            consensus: ConsensusMetrics::new(),
            gaming: GamingMetrics::new(),
            performance: PerformanceMetrics::new(),
            resources: ResourceMetrics::new(),
            errors: ErrorMetrics::new(),
            start_time: Instant::now(),
        }
    }
    
    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
    
    /// Update resource metrics from real system monitoring
    pub fn update_from_system_monitor(&self) {
        if let Ok(system_metrics) = crate::monitoring::system::global_system_monitor().collect_metrics() {
            self.resources.update_from_system_metrics(&system_metrics);
            
            // Log system monitoring status
            log::debug!("Updated metrics from system monitor: CPU {}%, Memory {} MB, Battery: {:?}%", 
                        system_metrics.cpu_usage_percent,
                        system_metrics.used_memory_bytes / 1024 / 1024,
                        system_metrics.battery_level);
        } else {
            log::warn!("Failed to collect system metrics, using fallback values");
        }
    }
    
    /// Start periodic system monitoring updates
    pub fn start_system_monitoring() -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval_timer.tick().await;
                METRICS.update_from_system_monitor();
            }
        })
    }
    
    /// Check if we have real system monitoring (vs simulated)
    pub fn is_real_system_monitoring(&self) -> bool {
        crate::monitoring::system::global_system_monitor().is_real_monitoring()
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String {
        let mut output = String::new();
        
        // Network metrics
        output.push_str(&format!(
            "# HELP bitcraps_network_messages_sent Total messages sent\n\
             # TYPE bitcraps_network_messages_sent counter\n\
             bitcraps_network_messages_sent {}\n",
            self.network.messages_sent.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_network_messages_received Total messages received\n\
             # TYPE bitcraps_network_messages_received counter\n\
             bitcraps_network_messages_received {}\n",
            self.network.messages_received.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_network_bytes_sent Total bytes sent\n\
             # TYPE bitcraps_network_bytes_sent counter\n\
             bitcraps_network_bytes_sent {}\n",
            self.network.bytes_sent.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_network_active_connections Active connections\n\
             # TYPE bitcraps_network_active_connections gauge\n\
             bitcraps_network_active_connections {}\n",
            self.network.active_connections.load(Ordering::Relaxed)
        ));
        
        // Consensus metrics
        output.push_str(&format!(
            "# HELP bitcraps_consensus_proposals_accepted Accepted proposals\n\
             # TYPE bitcraps_consensus_proposals_accepted counter\n\
             bitcraps_consensus_proposals_accepted {}\n",
            self.consensus.proposals_accepted.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_consensus_latency_ms Average consensus latency\n\
             # TYPE bitcraps_consensus_latency_ms gauge\n\
             bitcraps_consensus_latency_ms {}\n",
            self.consensus.average_latency_ms()
        ));
        
        // Gaming metrics
        output.push_str(&format!(
            "# HELP bitcraps_games_total Total games played\n\
             # TYPE bitcraps_games_total counter\n\
             bitcraps_games_total {}\n",
            self.gaming.total_games.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_bets_total Total bets placed\n\
             # TYPE bitcraps_bets_total counter\n\
             bitcraps_bets_total {}\n",
            self.gaming.total_bets.load(Ordering::Relaxed)
        ));
        
        // Resource metrics
        output.push_str(&format!(
            "# HELP bitcraps_memory_usage_bytes Current memory usage\n\
             # TYPE bitcraps_memory_usage_bytes gauge\n\
             bitcraps_memory_usage_bytes {}\n",
            self.resources.memory_usage_bytes.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "# HELP bitcraps_cpu_usage_percent CPU usage percentage\n\
             # TYPE bitcraps_cpu_usage_percent gauge\n\
             bitcraps_cpu_usage_percent {}\n",
            self.resources.cpu_usage_percent.load(Ordering::Relaxed)
        ));
        
        // Battery metrics (if available)
        if let Some(battery_level) = self.resources.get_battery_level() {
            output.push_str(&format!(
                "# HELP bitcraps_battery_level Battery level percentage\n\
                 # TYPE bitcraps_battery_level gauge\n\
                 bitcraps_battery_level {}\n",
                battery_level
            ));
        }
        
        if let Some(battery_charging) = self.resources.is_battery_charging() {
            output.push_str(&format!(
                "# HELP bitcraps_battery_charging Battery charging status (1=charging, 0=discharging)\n\
                 # TYPE bitcraps_battery_charging gauge\n\
                 bitcraps_battery_charging {}\n",
                if battery_charging { 1 } else { 0 }
            ));
        }
        
        // Temperature metrics (if available)
        if let Some(temperature) = self.resources.get_temperature() {
            output.push_str(&format!(
                "# HELP bitcraps_temperature_celsius Device temperature in Celsius\n\
                 # TYPE bitcraps_temperature_celsius gauge\n\
                 bitcraps_temperature_celsius {}\n",
                temperature
            ));
        }
        
        output.push_str(&format!(
            "# HELP bitcraps_thermal_throttling Thermal throttling active (1=yes, 0=no)\n\
             # TYPE bitcraps_thermal_throttling gauge\n\
             bitcraps_thermal_throttling {}\n",
            if self.resources.is_thermal_throttling() { 1 } else { 0 }
        ));
        
        // Error metrics
        output.push_str(&format!(
            "# HELP bitcraps_errors_total Total errors\n\
             # TYPE bitcraps_errors_total counter\n\
             bitcraps_errors_total {}\n",
            self.errors.total_errors.load(Ordering::Relaxed)
        ));
        
        // Uptime
        output.push_str(&format!(
            "# HELP bitcraps_uptime_seconds Uptime in seconds\n\
             # TYPE bitcraps_uptime_seconds counter\n\
             bitcraps_uptime_seconds {}\n",
            self.uptime_seconds()
        ));
        
        output
    }
    
    /// Export metrics as JSON
    pub fn export_json(&self) -> serde_json::Result<String> {
        let snapshot = MetricsSnapshot {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            uptime_seconds: self.uptime_seconds(),
            network: NetworkSnapshot {
                messages_sent: self.network.messages_sent.load(Ordering::Relaxed),
                messages_received: self.network.messages_received.load(Ordering::Relaxed),
                bytes_sent: self.network.bytes_sent.load(Ordering::Relaxed),
                bytes_received: self.network.bytes_received.load(Ordering::Relaxed),
                active_connections: self.network.active_connections.load(Ordering::Relaxed),
                connection_errors: self.network.connection_errors.load(Ordering::Relaxed),
            },
            consensus: ConsensusSnapshot {
                proposals_submitted: self.consensus.proposals_submitted.load(Ordering::Relaxed),
                proposals_accepted: self.consensus.proposals_accepted.load(Ordering::Relaxed),
                proposals_rejected: self.consensus.proposals_rejected.load(Ordering::Relaxed),
                average_latency_ms: self.consensus.average_latency_ms(),
                fork_count: self.consensus.fork_count.load(Ordering::Relaxed),
            },
            gaming: GamingSnapshot {
                total_games: self.gaming.total_games.load(Ordering::Relaxed),
                active_games: self.gaming.active_games.load(Ordering::Relaxed),
                total_bets: self.gaming.total_bets.load(Ordering::Relaxed),
                total_volume: self.gaming.total_volume.load(Ordering::Relaxed),
                total_payouts: self.gaming.total_payouts.load(Ordering::Relaxed),
            },
            resources: ResourceSnapshot {
                memory_usage_bytes: self.resources.memory_usage_bytes.load(Ordering::Relaxed),
                cpu_usage_percent: self.resources.cpu_usage_percent.load(Ordering::Relaxed),
                disk_usage_bytes: self.resources.disk_usage_bytes.load(Ordering::Relaxed),
                thread_count: self.resources.thread_count.load(Ordering::Relaxed),
            },
            errors: ErrorSnapshot {
                total_errors: self.errors.total_errors.load(Ordering::Relaxed),
                network_errors: self.errors.network_errors.load(Ordering::Relaxed),
                consensus_errors: self.errors.consensus_errors.load(Ordering::Relaxed),
                gaming_errors: self.errors.gaming_errors.load(Ordering::Relaxed),
            },
        };
        
        serde_json::to_string_pretty(&snapshot)
    }
}

/// Network-related metrics
pub struct NetworkMetrics {
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub active_connections: AtomicUsize,
    pub connection_errors: AtomicU64,
    pub packet_loss_rate: Arc<RwLock<f64>>,
    pub average_latency: Arc<RwLock<LatencyTracker>>,
}

impl NetworkMetrics {
    fn new() -> Self {
        Self {
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            connection_errors: AtomicU64::new(0),
            packet_loss_rate: Arc::new(RwLock::new(0.0)),
            average_latency: Arc::new(RwLock::new(LatencyTracker::new(100))),
        }
    }
    
    pub fn record_message_sent(&self, bytes: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
    }
    
    pub fn record_message_received(&self, bytes: usize) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
    }
    
    pub fn record_latency(&self, latency_ms: f64) {
        self.average_latency.write().add_sample(latency_ms);
    }
}

/// Consensus-related metrics
pub struct ConsensusMetrics {
    pub proposals_submitted: AtomicU64,
    pub proposals_accepted: AtomicU64,
    pub proposals_rejected: AtomicU64,
    pub consensus_rounds: AtomicU64,
    pub fork_count: AtomicU64,
    pub latency_samples: Arc<RwLock<LatencyTracker>>,
}

impl ConsensusMetrics {
    fn new() -> Self {
        Self {
            proposals_submitted: AtomicU64::new(0),
            proposals_accepted: AtomicU64::new(0),
            proposals_rejected: AtomicU64::new(0),
            consensus_rounds: AtomicU64::new(0),
            fork_count: AtomicU64::new(0),
            latency_samples: Arc::new(RwLock::new(LatencyTracker::new(100))),
        }
    }
    
    pub fn record_proposal(&self, accepted: bool, latency_ms: f64) {
        self.proposals_submitted.fetch_add(1, Ordering::Relaxed);
        if accepted {
            self.proposals_accepted.fetch_add(1, Ordering::Relaxed);
        } else {
            self.proposals_rejected.fetch_add(1, Ordering::Relaxed);
        }
        self.latency_samples.write().add_sample(latency_ms);
    }
    
    pub fn average_latency_ms(&self) -> f64 {
        self.latency_samples.read().average()
    }
}

/// Gaming-related metrics
pub struct GamingMetrics {
    pub total_games: AtomicU64,
    pub active_games: AtomicUsize,
    pub total_bets: AtomicU64,
    pub total_volume: AtomicU64,
    pub total_payouts: AtomicU64,
    pub dice_rolls: AtomicU64,
    pub disputes: AtomicU64,
}

impl GamingMetrics {
    fn new() -> Self {
        Self {
            total_games: AtomicU64::new(0),
            active_games: AtomicUsize::new(0),
            total_bets: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
            total_payouts: AtomicU64::new(0),
            dice_rolls: AtomicU64::new(0),
            disputes: AtomicU64::new(0),
        }
    }
    
    pub fn record_bet(&self, amount: u64) {
        self.total_bets.fetch_add(1, Ordering::Relaxed);
        self.total_volume.fetch_add(amount, Ordering::Relaxed);
    }
    
    pub fn record_payout(&self, amount: u64) {
        self.total_payouts.fetch_add(amount, Ordering::Relaxed);
    }
}

/// Performance metrics
pub struct PerformanceMetrics {
    pub operation_latencies: Arc<RwLock<LatencyTracker>>,
    pub throughput_ops_per_sec: Arc<RwLock<f64>>,
    pub cache_hit_rate: Arc<RwLock<f64>>,
    pub compression_ratio: Arc<RwLock<f64>>,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            operation_latencies: Arc::new(RwLock::new(LatencyTracker::new(1000))),
            throughput_ops_per_sec: Arc::new(RwLock::new(0.0)),
            cache_hit_rate: Arc::new(RwLock::new(0.0)),
            compression_ratio: Arc::new(RwLock::new(1.0)),
        }
    }
    
    pub fn record_operation(&self, latency_ms: f64) {
        self.operation_latencies.write().add_sample(latency_ms);
    }
    
    pub fn update_throughput(&self, ops_per_sec: f64) {
        *self.throughput_ops_per_sec.write() = ops_per_sec;
    }
}

/// Resource usage metrics
pub struct ResourceMetrics {
    pub memory_usage_bytes: AtomicU64,
    pub cpu_usage_percent: AtomicUsize,
    pub disk_usage_bytes: AtomicU64,
    pub thread_count: AtomicUsize,
    pub open_file_descriptors: AtomicUsize,
    /// Battery level (0-100) if available
    pub battery_level: Arc<RwLock<Option<f32>>>,
    /// Battery charging status
    pub battery_charging: Arc<RwLock<Option<bool>>>,
    /// Temperature in Celsius if available
    pub temperature_celsius: Arc<RwLock<Option<f32>>>,
    /// Whether thermal throttling is active
    pub thermal_throttling: Arc<RwLock<bool>>,
}

impl ResourceMetrics {
    fn new() -> Self {
        Self {
            memory_usage_bytes: AtomicU64::new(0),
            cpu_usage_percent: AtomicUsize::new(0),
            disk_usage_bytes: AtomicU64::new(0),
            thread_count: AtomicUsize::new(0),
            open_file_descriptors: AtomicUsize::new(0),
            battery_level: Arc::new(RwLock::new(None)),
            battery_charging: Arc::new(RwLock::new(None)),
            temperature_celsius: Arc::new(RwLock::new(None)),
            thermal_throttling: Arc::new(RwLock::new(false)),
        }
    }
    
    pub fn update_memory(&self, bytes: u64) {
        self.memory_usage_bytes.store(bytes, Ordering::Relaxed);
    }
    
    pub fn update_cpu(&self, percent: usize) {
        self.cpu_usage_percent.store(percent.min(100), Ordering::Relaxed);
    }
    
    /// Update resource metrics from real system monitoring
    pub fn update_from_system_metrics(&self, system_metrics: &crate::monitoring::system::SystemMetrics) {
        // Update basic metrics
        self.update_memory(system_metrics.used_memory_bytes);
        self.update_cpu(system_metrics.cpu_usage_percent as usize);
        self.thread_count.store(system_metrics.thread_count as usize, Ordering::Relaxed);
        
        // Update battery metrics
        *self.battery_level.write() = system_metrics.battery_level;
        *self.battery_charging.write() = system_metrics.battery_charging;
        
        // Update thermal metrics
        *self.temperature_celsius.write() = system_metrics.temperature_celsius;
        *self.thermal_throttling.write() = system_metrics.thermal_throttling;
    }
    
    /// Get current battery level if available
    pub fn get_battery_level(&self) -> Option<f32> {
        *self.battery_level.read()
    }
    
    /// Get current battery charging status if available
    pub fn is_battery_charging(&self) -> Option<bool> {
        *self.battery_charging.read()
    }
    
    /// Get current temperature if available
    pub fn get_temperature(&self) -> Option<f32> {
        *self.temperature_celsius.read()
    }
    
    /// Check if thermal throttling is active
    pub fn is_thermal_throttling(&self) -> bool {
        *self.thermal_throttling.read()
    }
}

/// Error tracking metrics
pub struct ErrorMetrics {
    pub total_errors: AtomicU64,
    pub network_errors: AtomicU64,
    pub consensus_errors: AtomicU64,
    pub gaming_errors: AtomicU64,
    pub critical_errors: AtomicU64,
    pub recent_errors: Arc<RwLock<VecDeque<ErrorEvent>>>,
}

impl ErrorMetrics {
    fn new() -> Self {
        Self {
            total_errors: AtomicU64::new(0),
            network_errors: AtomicU64::new(0),
            consensus_errors: AtomicU64::new(0),
            gaming_errors: AtomicU64::new(0),
            critical_errors: AtomicU64::new(0),
            recent_errors: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
        }
    }
    
    pub fn record_error(&self, category: ErrorCategory, message: String, is_critical: bool) {
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        
        match category {
            ErrorCategory::Network => { self.network_errors.fetch_add(1, Ordering::Relaxed); },
            ErrorCategory::Consensus => { self.consensus_errors.fetch_add(1, Ordering::Relaxed); },
            ErrorCategory::Gaming => { self.gaming_errors.fetch_add(1, Ordering::Relaxed); },
            ErrorCategory::Other => {},
        };
        
        if is_critical {
            self.critical_errors.fetch_add(1, Ordering::Relaxed);
        }
        
        let event = ErrorEvent {
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            category,
            message,
            is_critical,
        };
        
        let mut errors = self.recent_errors.write();
        if errors.len() >= 100 {
            errors.pop_front();
        }
        errors.push_back(event);
    }
}

/// Error categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorCategory {
    Network,
    Consensus,
    Gaming,
    Other,
}

/// Error event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub timestamp: u64,
    pub category: ErrorCategory,
    pub message: String,
    pub is_critical: bool,
}

/// Latency tracker with rolling window
pub struct LatencyTracker {
    samples: VecDeque<f64>,
    max_samples: usize,
}

impl LatencyTracker {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }
    
    pub fn add_sample(&mut self, latency_ms: f64) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
    }
    
    pub fn average(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.samples.iter().sum::<f64>() / self.samples.len() as f64
        }
    }
    
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        
        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let index = ((p / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[index]
    }
}

/// Metrics snapshot for export
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub network: NetworkSnapshot,
    pub consensus: ConsensusSnapshot,
    pub gaming: GamingSnapshot,
    pub resources: ResourceSnapshot,
    pub errors: ErrorSnapshot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkSnapshot {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: usize,
    pub connection_errors: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsensusSnapshot {
    pub proposals_submitted: u64,
    pub proposals_accepted: u64,
    pub proposals_rejected: u64,
    pub average_latency_ms: f64,
    pub fork_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GamingSnapshot {
    pub total_games: u64,
    pub active_games: usize,
    pub total_bets: u64,
    pub total_volume: u64,
    pub total_payouts: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: usize,
    pub disk_usage_bytes: u64,
    pub thread_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorSnapshot {
    pub total_errors: u64,
    pub network_errors: u64,
    pub consensus_errors: u64,
    pub gaming_errors: u64,
}

lazy_static::lazy_static! {
    /// Global metrics instance
    pub static ref METRICS: Arc<MetricsCollector> = Arc::new(MetricsCollector::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_collection() {
        let metrics = MetricsCollector::new();
        
        // Record some metrics
        metrics.network.record_message_sent(100);
        metrics.network.record_message_received(200);
        metrics.consensus.record_proposal(true, 10.0);
        metrics.gaming.record_bet(100);
        
        // Check values
        assert_eq!(metrics.network.messages_sent.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.network.bytes_sent.load(Ordering::Relaxed), 100);
        assert_eq!(metrics.consensus.proposals_accepted.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.gaming.total_bets.load(Ordering::Relaxed), 1);
    }
    
    #[test]
    fn test_prometheus_export() {
        let metrics = MetricsCollector::new();
        metrics.network.record_message_sent(100);
        
        let prometheus = metrics.export_prometheus();
        assert!(prometheus.contains("bitcraps_network_messages_sent 1"));
    }
    
    #[test]
    fn test_json_export() {
        let metrics = MetricsCollector::new();
        metrics.network.record_message_sent(100);
        
        let json = metrics.export_json().unwrap();
        assert!(json.contains("\"messages_sent\": 1"));
    }
}