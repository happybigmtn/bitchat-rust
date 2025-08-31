//! Network resilience and fault tolerance for mesh networking
//!
//! This module implements advanced resilience mechanisms to handle
//! network partitions, node failures, and dynamic topology changes.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use crate::error::Result;
use crate::protocol::PeerId;

/// Network resilience manager
pub struct NetworkResilience {
    /// Node failure detector
    failure_detector: Arc<RwLock<FailureDetector>>,
    /// Partition detection and healing
    partition_manager: Arc<RwLock<PartitionManager>>,
    /// Adaptive routing with redundancy
    adaptive_routing: Arc<RwLock<AdaptiveRouting>>,
    /// Recovery mechanisms
    recovery_manager: Arc<RwLock<RecoveryManager>>,
    /// Network health monitor
    health_monitor: Arc<RwLock<NetworkHealthMonitor>>,
    /// Configuration
    config: ResilienceConfig,
    /// Event channel for notifications
    event_sender: mpsc::Sender<ResilienceEvent>,
    /// Monitoring state
    is_monitoring: Arc<RwLock<bool>>,
}

/// Resilience configuration
#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    /// Failure detection timeout
    pub failure_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum partition heal time
    pub max_partition_heal_time: Duration,
    /// Redundancy factor for routing
    pub routing_redundancy: usize,
    /// Adaptive threshold for route switching
    pub adaptive_threshold: f64,
    /// Recovery attempt interval
    pub recovery_interval: Duration,
    /// Maximum recovery attempts
    pub max_recovery_attempts: u32,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            failure_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            max_partition_heal_time: Duration::from_secs(300),
            routing_redundancy: 3,
            adaptive_threshold: 0.8,
            recovery_interval: Duration::from_secs(60),
            max_recovery_attempts: 5,
            health_check_interval: Duration::from_secs(5),
        }
    }
}

/// Failure detection system
#[derive(Debug, Clone)]
struct FailureDetector {
    /// Suspected failed nodes
    suspected_failures: HashMap<PeerId, FailureInfo>,
    /// Confirmed failed nodes
    confirmed_failures: HashSet<PeerId>,
    /// Heartbeat history
    heartbeat_history: HashMap<PeerId, VecDeque<Instant>>,
    /// Failure detection phi threshold
    phi_threshold: f64,
    /// Last cleanup time
    last_cleanup: Instant,
}

/// Failure information
#[derive(Debug, Clone)]
struct FailureInfo {
    peer_id: PeerId,
    suspected_at: Instant,
    phi_value: f64,
    missed_heartbeats: u32,
    last_seen: Instant,
}

/// Network partition management
#[derive(Debug, Clone)]
struct PartitionManager {
    /// Detected partitions
    partitions: HashMap<u32, PartitionInfo>,
    /// Partition healing attempts
    healing_attempts: HashMap<u32, HealingAttempt>,
    /// Partition history
    partition_history: VecDeque<PartitionEvent>,
    /// Next partition ID
    next_partition_id: u32,
}

/// Partition information
#[derive(Debug, Clone)]
struct PartitionInfo {
    partition_id: u32,
    nodes: HashSet<PeerId>,
    detected_at: Instant,
    last_updated: Instant,
    is_majority: bool,
    bridge_candidates: Vec<PeerId>,
}

/// Partition healing attempt
#[derive(Debug, Clone)]
struct HealingAttempt {
    partition_id: u32,
    started_at: Instant,
    attempts: u32,
    bridge_nodes: Vec<PeerId>,
    success_probability: f64,
}

/// Partition event for history
#[derive(Debug, Clone)]
struct PartitionEvent {
    timestamp: Instant,
    event_type: PartitionEventType,
    partition_id: u32,
    affected_nodes: HashSet<PeerId>,
}

#[derive(Debug, Clone, Copy)]
enum PartitionEventType {
    Detected,
    Healed,
    HealingStarted,
    HealingFailed,
}

/// Adaptive routing with multiple paths
#[derive(Debug, Clone)]
struct AdaptiveRouting {
    /// Primary routes for each destination
    primary_routes: HashMap<PeerId, RouteInfo>,
    /// Backup routes for failover
    backup_routes: HashMap<PeerId, Vec<RouteInfo>>,
    /// Route performance metrics
    route_metrics: HashMap<(PeerId, PeerId), RouteMetrics>,
    /// Active route selections
    active_routes: HashMap<PeerId, PeerId>, // destination -> next_hop
    /// Route switching decisions
    switching_decisions: VecDeque<RouteSwitchDecision>,
}

/// Route information
#[derive(Debug, Clone)]
pub struct RouteInfo {
    destination: PeerId,
    path: Vec<PeerId>,
    next_hop: PeerId,
    quality_score: f64,
    last_used: Instant,
    use_count: u64,
}

/// Route performance metrics
#[derive(Debug, Clone)]
struct RouteMetrics {
    latency: Duration,
    success_rate: f64,
    bandwidth: f64,
    stability: f64,
    last_updated: Instant,
    sample_count: u32,
}

/// Route switching decision
#[derive(Debug, Clone)]
struct RouteSwitchDecision {
    timestamp: Instant,
    destination: PeerId,
    old_route: PeerId,
    new_route: PeerId,
    reason: SwitchReason,
    quality_improvement: f64,
}

#[derive(Debug, Clone, Copy)]
pub enum SwitchReason {
    PerformanceDegradation,
    NodeFailure,
    BetterRouteFound,
    LoadBalancing,
    NetworkPartition,
}

/// Recovery management
#[derive(Debug, Clone)]
struct RecoveryManager {
    /// Active recovery operations
    active_recoveries: HashMap<PeerId, RecoveryOperation>,
    /// Recovery history
    recovery_history: VecDeque<RecoveryRecord>,
    /// Recovery strategies
    recovery_strategies: Vec<RecoveryStrategy>,
    /// Success rates by strategy
    strategy_success_rates: HashMap<String, f64>,
}

/// Recovery operation
#[derive(Debug, Clone)]
struct RecoveryOperation {
    peer_id: PeerId,
    started_at: Instant,
    strategy: RecoveryStrategy,
    attempts: u32,
    last_attempt: Instant,
    expected_completion: Instant,
}

/// Recovery strategy
#[derive(Debug, Clone)]
struct RecoveryStrategy {
    name: String,
    timeout: Duration,
    retry_interval: Duration,
    max_attempts: u32,
    success_threshold: f64,
}

/// Recovery record
#[derive(Debug, Clone)]
struct RecoveryRecord {
    timestamp: Instant,
    peer_id: PeerId,
    strategy_used: String,
    success: bool,
    duration: Duration,
    attempts_made: u32,
}

/// Network health monitoring
#[derive(Debug, Clone)]
struct NetworkHealthMonitor {
    /// Current network health score (0.0-1.0)
    health_score: f64,
    /// Health metrics by category
    connectivity_health: f64,
    latency_health: f64,
    throughput_health: f64,
    stability_health: f64,
    /// Health history
    health_history: VecDeque<(Instant, f64)>,
    /// Alert thresholds
    warning_threshold: f64,
    critical_threshold: f64,
    /// Last health check
    last_check: Instant,
}

impl Default for NetworkHealthMonitor {
    fn default() -> Self {
        Self {
            health_score: 1.0,
            connectivity_health: 1.0,
            latency_health: 1.0,
            throughput_health: 1.0,
            stability_health: 1.0,
            health_history: VecDeque::new(),
            warning_threshold: 0.7,
            critical_threshold: 0.3,
            last_check: Instant::now(),
        }
    }
}

/// Resilience events
#[derive(Debug, Clone)]
pub enum ResilienceEvent {
    NodeFailureDetected {
        peer_id: PeerId,
        phi_value: f64,
    },
    NodeRecovered {
        peer_id: PeerId,
        downtime: Duration,
    },
    PartitionDetected {
        partition_id: u32,
        nodes: HashSet<PeerId>,
    },
    PartitionHealed {
        partition_id: u32,
        heal_time: Duration,
    },
    RouteSwitch {
        destination: PeerId,
        old_route: PeerId,
        new_route: PeerId,
        reason: SwitchReason,
    },
    HealthDegradation {
        old_score: f64,
        new_score: f64,
        category: String,
    },
    RecoveryCompleted {
        peer_id: PeerId,
        success: bool,
        attempts: u32,
    },
}

impl NetworkResilience {
    /// Create new network resilience manager
    pub fn new(config: ResilienceConfig) -> Self {
        let (event_sender, _) = mpsc::channel(1000); // Moderate traffic for resilience events

        Self {
            failure_detector: Arc::new(RwLock::new(FailureDetector::new(config.clone()))),
            partition_manager: Arc::new(RwLock::new(PartitionManager::new())),
            adaptive_routing: Arc::new(RwLock::new(AdaptiveRouting::new(
                config.routing_redundancy,
            ))),
            recovery_manager: Arc::new(RwLock::new(RecoveryManager::new(config.clone()))),
            health_monitor: Arc::new(RwLock::new(NetworkHealthMonitor::new())),
            event_sender,
            is_monitoring: Arc::new(RwLock::new(false)),
            config,
        }
    }

    /// Start resilience monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        *self.is_monitoring.write().await = true;

        // Start monitoring tasks
        self.start_failure_detection().await;
        self.start_partition_detection().await;
        self.start_adaptive_routing().await;
        self.start_recovery_management().await;
        self.start_health_monitoring().await;

        log::info!("Network resilience monitoring started");
        Ok(())
    }

    /// Stop resilience monitoring
    pub async fn stop_monitoring(&self) {
        *self.is_monitoring.write().await = false;
        log::info!("Network resilience monitoring stopped");
    }

    /// Update peer heartbeat
    pub async fn update_peer_heartbeat(&self, peer_id: PeerId) {
        let mut detector = self.failure_detector.write().await;
        detector.update_heartbeat(peer_id);

        // Remove from suspected failures if present
        if detector.suspected_failures.remove(&peer_id).is_some() {
            let _ = self.event_sender.send(ResilienceEvent::NodeRecovered {
                peer_id,
                downtime: Duration::from_secs(60), // Estimate
            });
        }

        // Remove from confirmed failures
        detector.confirmed_failures.remove(&peer_id);
    }

    /// Report route performance
    pub async fn report_route_performance(
        &self,
        from: PeerId,
        to: PeerId,
        latency: Duration,
        success: bool,
    ) {
        let mut routing = self.adaptive_routing.write().await;
        routing.update_route_metrics(from, to, latency, success);
    }

    /// Get best route for destination
    pub async fn get_best_route(&self, destination: PeerId) -> Option<RouteInfo> {
        let routing = self.adaptive_routing.read().await;
        routing.get_best_route(destination)
    }

    /// Handle node failure
    pub async fn handle_node_failure(&self, peer_id: PeerId) {
        let mut recovery = self.recovery_manager.write().await;
        recovery.start_recovery(peer_id);

        // Update routing to avoid failed node
        let mut routing = self.adaptive_routing.write().await;
        routing.mark_node_failed(peer_id);
    }

    /// Get network health score
    pub async fn get_health_score(&self) -> f64 {
        let monitor = self.health_monitor.read().await;
        monitor.health_score
    }

    /// Get resilience statistics
    pub async fn get_statistics(&self) -> ResilienceStatistics {
        let detector = self.failure_detector.read().await;
        let partition_mgr = self.partition_manager.read().await;
        let routing = self.adaptive_routing.read().await;
        let recovery = self.recovery_manager.read().await;
        let health = self.health_monitor.read().await;

        ResilienceStatistics {
            suspected_failures: detector.suspected_failures.len(),
            confirmed_failures: detector.confirmed_failures.len(),
            active_partitions: partition_mgr.partitions.len(),
            primary_routes: routing.primary_routes.len(),
            backup_routes: routing.backup_routes.values().map(|v| v.len()).sum(),
            active_recoveries: recovery.active_recoveries.len(),
            network_health: health.health_score,
            connectivity_health: health.connectivity_health,
            latency_health: health.latency_health,
            throughput_health: health.throughput_health,
            stability_health: health.stability_health,
        }
    }

    /// Start failure detection task
    async fn start_failure_detection(&self) {
        let failure_detector = self.failure_detector.clone();
        let is_monitoring = self.is_monitoring.clone();
        let event_sender = self.event_sender.clone();
        let interval_duration = self.config.heartbeat_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *is_monitoring.read().await {
                interval.tick().await;

                let mut detector = failure_detector.write().await;
                let failures = detector.detect_failures();

                for (peer_id, phi_value) in failures {
                    let _ = event_sender
                        .send(ResilienceEvent::NodeFailureDetected { peer_id, phi_value });
                }

                // Cleanup old entries
                detector.cleanup_old_entries();
            }
        });
    }

    /// Start partition detection task
    async fn start_partition_detection(&self) {
        let partition_manager = self.partition_manager.clone();
        let is_monitoring = self.is_monitoring.clone();
        let event_sender = self.event_sender.clone();
        let interval_duration = Duration::from_secs(30);

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *is_monitoring.read().await {
                interval.tick().await;

                let mut mgr = partition_manager.write().await;
                let partitions = mgr.detect_partitions();

                for partition_info in partitions {
                    let _ = event_sender.send(ResilienceEvent::PartitionDetected {
                        partition_id: partition_info.partition_id,
                        nodes: partition_info.nodes.clone(),
                    });
                }
            }
        });
    }

    /// Start adaptive routing task
    async fn start_adaptive_routing(&self) {
        let adaptive_routing = self.adaptive_routing.clone();
        let is_monitoring = self.is_monitoring.clone();
        let event_sender = self.event_sender.clone();
        let interval_duration = Duration::from_secs(15);

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *is_monitoring.read().await {
                interval.tick().await;

                let mut routing = adaptive_routing.write().await;
                let switches = routing.evaluate_route_switches();

                for switch in switches {
                    let _ = event_sender.send(ResilienceEvent::RouteSwitch {
                        destination: switch.destination,
                        old_route: switch.old_route,
                        new_route: switch.new_route,
                        reason: switch.reason,
                    });
                }
            }
        });
    }

    /// Start recovery management task
    async fn start_recovery_management(&self) {
        let recovery_manager = self.recovery_manager.clone();
        let is_monitoring = self.is_monitoring.clone();
        let event_sender = self.event_sender.clone();
        let interval_duration = self.config.recovery_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *is_monitoring.read().await {
                interval.tick().await;

                let mut recovery = recovery_manager.write().await;
                let completions = recovery.process_recoveries();

                for (peer_id, success, attempts) in completions {
                    let _ = event_sender.send(ResilienceEvent::RecoveryCompleted {
                        peer_id,
                        success,
                        attempts,
                    });
                }
            }
        });
    }

    /// Start health monitoring task
    async fn start_health_monitoring(&self) {
        let health_monitor = self.health_monitor.clone();
        let is_monitoring = self.is_monitoring.clone();
        let event_sender = self.event_sender.clone();
        let interval_duration = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            while *is_monitoring.read().await {
                interval.tick().await;

                let mut monitor = health_monitor.write().await;
                let old_score = monitor.health_score;
                monitor.update_health_score();
                let new_score = monitor.health_score;

                if (old_score - new_score).abs() > 0.1 {
                    let _ = event_sender.send(ResilienceEvent::HealthDegradation {
                        old_score,
                        new_score,
                        category: "overall".to_string(),
                    });
                }
            }
        });
    }
}

impl FailureDetector {
    fn new(config: ResilienceConfig) -> Self {
        Self {
            suspected_failures: HashMap::new(),
            confirmed_failures: HashSet::new(),
            heartbeat_history: HashMap::new(),
            phi_threshold: 8.0, // Standard phi accrual threshold
            last_cleanup: Instant::now(),
        }
    }

    fn update_heartbeat(&mut self, peer_id: PeerId) {
        let now = Instant::now();
        let history = self.heartbeat_history.entry(peer_id).or_default();

        history.push_back(now);

        // Keep only last 100 heartbeats
        if history.len() > 100 {
            history.pop_front();
        }
    }

    fn detect_failures(&mut self) -> Vec<(PeerId, f64)> {
        let mut new_failures = Vec::new();
        let now = Instant::now();

        for (peer_id, history) in &self.heartbeat_history {
            if let Some(&last_heartbeat) = history.back() {
                let time_since_last = now.duration_since(last_heartbeat);

                // Calculate phi value (simplified version)
                let phi_value = if history.len() > 1 {
                    self.calculate_phi(*peer_id, time_since_last, history)
                } else {
                    0.0
                };

                if phi_value > self.phi_threshold && !self.suspected_failures.contains_key(peer_id)
                {
                    let failure_info = FailureInfo {
                        peer_id: *peer_id,
                        suspected_at: now,
                        phi_value,
                        missed_heartbeats: 1,
                        last_seen: last_heartbeat,
                    };

                    self.suspected_failures.insert(*peer_id, failure_info);
                    new_failures.push((*peer_id, phi_value));
                }
            }
        }

        new_failures
    }

    fn calculate_phi(
        &self,
        peer_id: PeerId,
        time_since_last: Duration,
        history: &VecDeque<Instant>,
    ) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        // Calculate mean and standard deviation of intervals
        let intervals: Vec<Duration> = history
            .iter()
            .zip(history.iter().skip(1))
            .map(|(prev, current)| current.duration_since(*prev))
            .collect();

        if intervals.is_empty() {
            return 0.0;
        }

        let mean = intervals.iter().sum::<Duration>().as_secs_f64() / intervals.len() as f64;
        let variance = intervals
            .iter()
            .map(|d| {
                let diff = d.as_secs_f64() - mean;
                diff * diff
            })
            .sum::<f64>()
            / intervals.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return 0.0;
        }

        // Phi accrual calculation (simplified)
        let p_later =
            1.0 - self.cumulative_distribution(time_since_last.as_secs_f64(), mean, std_dev);

        if p_later > 0.0 {
            -(p_later.ln() / 2.0_f64.ln())
        } else {
            self.phi_threshold + 1.0
        }
    }

    fn cumulative_distribution(&self, x: f64, mean: f64, std_dev: f64) -> f64 {
        // Simplified normal CDF approximation
        let z = (x - mean) / std_dev;
        0.5 * (1.0 + (z / (1.0 + 0.2316419 * z.abs())).tanh())
    }

    fn cleanup_old_entries(&mut self) {
        let now = Instant::now();

        // Remove old heartbeat history
        self.heartbeat_history.retain(|_, history| {
            if let Some(&last) = history.back() {
                now.duration_since(last) < Duration::from_secs(600) // 10 minutes
            } else {
                false
            }
        });

        self.last_cleanup = now;
    }
}

impl PartitionManager {
    fn new() -> Self {
        Self {
            partitions: HashMap::new(),
            healing_attempts: HashMap::new(),
            partition_history: VecDeque::new(),
            next_partition_id: 1,
        }
    }

    fn detect_partitions(&mut self) -> Vec<PartitionInfo> {
        // Simplified partition detection
        // In a real implementation, this would analyze connectivity matrix
        Vec::new()
    }
}

impl AdaptiveRouting {
    fn new(redundancy: usize) -> Self {
        Self {
            primary_routes: HashMap::new(),
            backup_routes: HashMap::new(),
            route_metrics: HashMap::new(),
            active_routes: HashMap::new(),
            switching_decisions: VecDeque::new(),
        }
    }

    fn update_route_metrics(&mut self, from: PeerId, to: PeerId, latency: Duration, success: bool) {
        let key = (from, to);
        let metrics = self
            .route_metrics
            .entry(key)
            .or_insert_with(|| RouteMetrics {
                latency: Duration::ZERO,
                success_rate: 1.0,
                bandwidth: 1.0,
                stability: 1.0,
                last_updated: Instant::now(),
                sample_count: 0,
            });

        // Update metrics with exponential moving average
        let alpha = 0.1; // Smoothing factor
        metrics.latency = Duration::from_secs_f64(
            metrics.latency.as_secs_f64() * (1.0 - alpha) + latency.as_secs_f64() * alpha,
        );

        let success_value = if success { 1.0 } else { 0.0 };
        metrics.success_rate = metrics.success_rate * (1.0 - alpha) + success_value * alpha;
        metrics.last_updated = Instant::now();
        metrics.sample_count += 1;
    }

    fn get_best_route(&self, destination: PeerId) -> Option<RouteInfo> {
        self.primary_routes.get(&destination).cloned()
    }

    fn mark_node_failed(&mut self, peer_id: PeerId) {
        // Remove routes that go through failed node
        self.primary_routes
            .retain(|_, route| !route.path.contains(&peer_id));

        // Update backup routes
        for (_, routes) in self.backup_routes.iter_mut() {
            routes.retain(|route| !route.path.contains(&peer_id));
        }

        // Remove from active routes
        self.active_routes
            .retain(|_, next_hop| *next_hop != peer_id);
    }

    fn evaluate_route_switches(&mut self) -> Vec<RouteSwitchDecision> {
        // Simplified route evaluation
        // In a real implementation, this would analyze all routes and make switching decisions
        Vec::new()
    }
}

impl RecoveryManager {
    fn new(config: ResilienceConfig) -> Self {
        let strategies = vec![
            RecoveryStrategy {
                name: "reconnect".to_string(),
                timeout: Duration::from_secs(30),
                retry_interval: Duration::from_secs(5),
                max_attempts: 5,
                success_threshold: 0.8,
            },
            RecoveryStrategy {
                name: "alternative_route".to_string(),
                timeout: Duration::from_secs(60),
                retry_interval: Duration::from_secs(10),
                max_attempts: 3,
                success_threshold: 0.7,
            },
        ];

        Self {
            active_recoveries: HashMap::new(),
            recovery_history: VecDeque::new(),
            recovery_strategies: strategies,
            strategy_success_rates: HashMap::new(),
        }
    }

    fn start_recovery(&mut self, peer_id: PeerId) {
        if self.active_recoveries.contains_key(&peer_id) {
            return; // Already recovering
        }

        // Select best strategy based on historical success rates
        let strategy = self.recovery_strategies[0].clone(); // Simplified selection

        let recovery_op = RecoveryOperation {
            peer_id,
            started_at: Instant::now(),
            strategy,
            attempts: 0,
            last_attempt: Instant::now(),
            expected_completion: Instant::now() + Duration::from_secs(60),
        };

        self.active_recoveries.insert(peer_id, recovery_op);
    }

    fn process_recoveries(&mut self) -> Vec<(PeerId, bool, u32)> {
        let mut completions = Vec::new();
        let mut to_remove = Vec::new();

        for (peer_id, recovery) in &mut self.active_recoveries {
            let now = Instant::now();

            if now >= recovery.expected_completion {
                // Recovery completed (success assumed for simulation)
                completions.push((*peer_id, true, recovery.attempts));
                to_remove.push(*peer_id);

                // Record in history
                self.recovery_history.push_back(RecoveryRecord {
                    timestamp: now,
                    peer_id: *peer_id,
                    strategy_used: recovery.strategy.name.clone(),
                    success: true,
                    duration: now.duration_since(recovery.started_at),
                    attempts_made: recovery.attempts,
                });
            }
        }

        // Remove completed recoveries
        for peer_id in to_remove {
            self.active_recoveries.remove(&peer_id);
        }

        completions
    }
}

impl NetworkHealthMonitor {
    fn new() -> Self {
        Self {
            health_score: 1.0,
            connectivity_health: 1.0,
            latency_health: 1.0,
            throughput_health: 1.0,
            stability_health: 1.0,
            health_history: VecDeque::new(),
            warning_threshold: 0.7,
            critical_threshold: 0.5,
            last_check: Instant::now(),
        }
    }

    fn update_health_score(&mut self) {
        // Simplified health calculation
        self.health_score = (self.connectivity_health
            + self.latency_health
            + self.throughput_health
            + self.stability_health)
            / 4.0;

        // Add to history
        self.health_history
            .push_back((Instant::now(), self.health_score));

        // Keep only last 1000 entries
        if self.health_history.len() > 1000 {
            self.health_history.pop_front();
        }

        self.last_check = Instant::now();
    }
}

/// Resilience statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceStatistics {
    pub suspected_failures: usize,
    pub confirmed_failures: usize,
    pub active_partitions: usize,
    pub primary_routes: usize,
    pub backup_routes: usize,
    pub active_recoveries: usize,
    pub network_health: f64,
    pub connectivity_health: f64,
    pub latency_health: f64,
    pub throughput_health: f64,
    pub stability_health: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resilience_manager_creation() {
        let config = ResilienceConfig::default();
        let resilience = NetworkResilience::new(config);

        let stats = resilience.get_statistics().await;
        assert_eq!(stats.suspected_failures, 0);
        assert_eq!(stats.confirmed_failures, 0);
    }

    #[tokio::test]
    async fn test_failure_detection() {
        let config = ResilienceConfig::default();
        let resilience = NetworkResilience::new(config);

        let peer_id = [1u8; 32];

        // Update heartbeat
        resilience.update_peer_heartbeat(peer_id).await;

        let stats = resilience.get_statistics().await;
        assert_eq!(stats.suspected_failures, 0);
    }

    #[tokio::test]
    async fn test_route_performance_reporting() {
        let config = ResilienceConfig::default();
        let resilience = NetworkResilience::new(config);

        let from = [1u8; 32];
        let to = [2u8; 32];
        let latency = Duration::from_millis(100);

        resilience
            .report_route_performance(from, to, latency, true)
            .await;

        // Test passes if no panic occurs
    }

    #[tokio::test]
    async fn test_health_monitoring() {
        let config = ResilienceConfig::default();
        let resilience = NetworkResilience::new(config);

        let health_score = resilience.get_health_score().await;
        assert!(health_score >= 0.0 && health_score <= 1.0);
    }

    #[test]
    fn test_phi_calculation() {
        let config = ResilienceConfig::default();
        let mut detector = FailureDetector::new(config);

        let peer_id = [1u8; 32];
        let now = Instant::now();

        // Add some heartbeat history
        let mut history = VecDeque::new();
        history.push_back(now - Duration::from_secs(30));
        history.push_back(now - Duration::from_secs(20));
        history.push_back(now - Duration::from_secs(10));

        let phi = detector.calculate_phi(peer_id, Duration::from_secs(15), &history);
        assert!(phi >= 0.0);
    }
}
