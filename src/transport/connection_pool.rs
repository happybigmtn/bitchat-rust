//! Enhanced connection pooling for Bluetooth transport

use crate::error::Result;
use crate::protocol::PeerId;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};

#[cfg(not(test))]
use num_cpus;

use super::bounded_queue::{BoundedEventQueue, QueueConfig};
use super::TransportEvent;

/// Quality of Service requirements for connections
#[derive(Debug, Clone, Copy)]
pub enum QoSPriority {
    /// Real-time gaming messages (dice rolls, bets)
    RealTime,
    /// Normal mesh routing messages
    Normal,
    /// Background sync and maintenance
    Background,
}

/// Load balancing strategy for connection pool
#[derive(Debug, Clone, Copy)]
pub enum LoadBalancingStrategy {
    /// Round-robin selection
    RoundRobin,
    /// Weighted round-robin based on connection quality
    WeightedRoundRobin,
    /// Least connections first
    LeastConnections,
    /// Quality-based selection (best connection first)
    QualityBased,
}

/// Pool configuration for tuning
#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub max_connections: usize,
    pub rebalance_interval: Duration,
    pub health_check_interval: Duration,
    pub connection_timeout: Duration,
    pub max_idle_time: Duration,
    pub load_balancing: LoadBalancingStrategy,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: Self::calculate_optimal_pool_size(),
            rebalance_interval: Duration::from_secs(15), // More frequent rebalancing for better utilization
            health_check_interval: Duration::from_secs(30), // Faster health checks
            connection_timeout: Duration::from_secs(20), // Reduced timeout for faster failover
            max_idle_time: Duration::from_secs(180), // Shorter idle time to free resources
            load_balancing: LoadBalancingStrategy::QualityBased, // Better strategy for gaming
        }
    }
}

impl PoolConfig {
    /// Calculate optimal pool size based on system capabilities
    fn calculate_optimal_pool_size() -> usize {
        let cpu_cores = num_cpus::get();
        let base_connections = match cpu_cores {
            1..=2 => 20,     // Low-end devices
            3..=4 => 40,     // Mid-range devices  
            5..=8 => 80,     // High-end devices
            _ => 120,        // Desktop/server class
        };
        
        // Scale based on available memory (rough heuristic)
        let memory_factor = if cfg!(target_os = "android") || cfg!(target_os = "ios") {
            0.7 // Mobile devices have memory constraints
        } else {
            1.0 // Desktop has more memory
        };
        
        (base_connections as f32 * memory_factor) as usize
    }

    /// Create adaptive configuration that adjusts based on load
    pub fn adaptive_config(current_load: f32) -> Self {
        let mut config = Self::default();
        
        // Adjust pool size based on current load
        if current_load > 0.8 {
            config.max_connections = (config.max_connections as f32 * 1.5) as usize;
            config.rebalance_interval = Duration::from_secs(10); // More aggressive rebalancing
        } else if current_load < 0.3 {
            config.max_connections = (config.max_connections as f32 * 0.7) as usize;
            config.max_idle_time = Duration::from_secs(120); // Shorter idle time when load is low
        }
        
        config
    }
}

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionScore {
    pub peer_id: PeerId,
    pub latency_ms: f32,
    pub packet_loss: f32,
    pub reliability_score: f32,
    pub bandwidth_mbps: f32,
    pub last_updated: Instant,
    pub successful_messages: u64,
    pub failed_messages: u64,
}

impl ConnectionScore {
    /// Calculate overall quality score (0.0 to 1.0)
    pub fn quality_score(&self) -> f32 {
        let latency_score = (100.0 - self.latency_ms.min(100.0)) / 100.0;
        let loss_score = 1.0 - self.packet_loss;
        let reliability = self.reliability_score;

        // Weighted average
        (latency_score * 0.3 + loss_score * 0.4 + reliability * 0.3).clamp(0.0, 1.0)
    }

    /// Categorize connection quality
    pub fn quality_tier(&self) -> ConnectionQuality {
        let score = self.quality_score();
        if score >= 0.8 {
            ConnectionQuality::High
        } else if score >= 0.5 {
            ConnectionQuality::Medium
        } else {
            ConnectionQuality::Low
        }
    }
}

/// Connection quality tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    High,
    Medium,
    Low,
}

/// Pooled connection wrapper
pub struct PooledConnection {
    pub peer_id: PeerId,
    pub connection: Arc<dyn ConnectionHandle>,
    pub acquired_at: Instant,
    pub last_used: Instant,
    pub usage_count: u64,
}

/// Connection handle trait
pub trait ConnectionHandle: Send + Sync {
    fn is_connected(&self) -> bool;
    fn peer_id(&self) -> PeerId;
    fn close(&self);
}

/// Enhanced Bluetooth connection pool with bounded queues
pub struct BluetoothConnectionPool {
    /// Separate pools by connection quality
    high_quality: Arc<RwLock<VecDeque<PooledConnection>>>,
    medium_quality: Arc<RwLock<VecDeque<PooledConnection>>>,
    low_quality: Arc<RwLock<VecDeque<PooledConnection>>>,

    /// Active connections (not in pool)
    active_connections: Arc<RwLock<HashMap<PeerId, PooledConnection>>>,

    /// Connection scoring system
    connection_scores: Arc<RwLock<HashMap<PeerId, ConnectionScore>>>,

    /// Pool configuration
    config: PoolConfig,

    /// Semaphore for connection limits
    connection_semaphore: Arc<Semaphore>,

    /// Metrics
    metrics: Arc<RwLock<PoolMetrics>>,

    /// Bounded event queue for pool events
    event_queue: BoundedEventQueue<TransportEvent>,

    /// Load balancing strategy
    load_balancer: LoadBalancingStrategy,
}

/// Enhanced pool configuration with tuning parameters
#[derive(Debug, Clone)]
pub struct EnhancedPoolConfig {
    /// Maximum total connections
    pub max_connections: usize,
    /// Rebalancing interval
    pub rebalance_interval: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Max idle time before cleanup
    pub max_idle_time: Duration,
    /// Load balancing strategy
    pub load_balancing: LoadBalancingStrategy,
    /// Event queue configuration
    pub event_queue_config: QueueConfig,
}

/// Legacy pool configuration (kept for compatibility)
#[derive(Debug, Clone)]
pub struct LegacyPoolConfig {
    /// Maximum total connections
    pub max_connections: usize,
    /// Maximum idle connections per quality tier
    pub max_idle_per_tier: usize,
    /// Connection idle timeout
    pub idle_timeout: Duration,
    /// Connection max lifetime
    pub max_lifetime: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
}

// Duplicate Default implementation removed - using the first one

/// Pool metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct PoolMetrics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub connection_acquisitions: u64,
    pub connection_releases: u64,
    pub connection_failures: u64,
    pub total_evictions: u64,
    pub average_acquisition_time_ms: f32,
    pub connection_reuse_rate: f32,
}

/// Detailed efficiency report for pool optimization
#[derive(Debug, Clone)]
pub struct PoolEfficiencyReport {
    pub overall_utilization: f32,
    pub tier_distribution: TierDistribution,
    pub efficiency_score: f32,
    pub recommended_adjustments: Vec<String>,
}

/// Distribution of connections across quality tiers
#[derive(Debug, Clone)]
pub struct TierDistribution {
    pub high_quality_percentage: f32,
    pub medium_quality_percentage: f32,
    pub low_quality_percentage: f32,
}

impl BluetoothConnectionPool {
    /// Create new connection pool
    pub fn new(config: PoolConfig) -> Self {
        let max_connections = config.max_connections;
        let load_balancer = config.load_balancing.clone();

        Self {
            high_quality: Arc::new(RwLock::new(VecDeque::new())),
            medium_quality: Arc::new(RwLock::new(VecDeque::new())),
            low_quality: Arc::new(RwLock::new(VecDeque::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_scores: Arc::new(RwLock::new(HashMap::new())),
            config,
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            metrics: Arc::new(RwLock::new(PoolMetrics::default())),
            event_queue: BoundedEventQueue::new(),
            load_balancer,
        }
    }

    /// Get best available connection for requirements
    pub async fn get_connection(&self, requirements: QoSPriority) -> Result<PooledConnection> {
        let start_time = Instant::now();

        // Acquire semaphore permit
        let _permit =
            self.connection_semaphore.acquire().await.map_err(|_| {
                crate::error::Error::Network("Connection limit reached".to_string())
            })?;

        // Try to get connection from appropriate pool
        let connection = match requirements {
            QoSPriority::RealTime => self
                .get_from_tier(ConnectionQuality::High)
                .await
                .or(self.get_from_tier(ConnectionQuality::Medium).await),
            QoSPriority::Normal => self
                .get_from_tier(ConnectionQuality::Medium)
                .await
                .or(self.get_from_tier(ConnectionQuality::High).await)
                .or(self.get_from_tier(ConnectionQuality::Low).await),
            QoSPriority::Background => self
                .get_from_tier(ConnectionQuality::Low)
                .await
                .or(self.get_from_tier(ConnectionQuality::Medium).await),
        };

        // Update metrics
        let acquisition_time = start_time.elapsed().as_millis() as f32;
        self.update_acquisition_metrics(acquisition_time, connection.is_some())
            .await;

        connection
            .ok_or_else(|| crate::error::Error::Network("No connections available".to_string()))
    }

    /// Get connection from specific quality tier
    async fn get_from_tier(&self, quality: ConnectionQuality) -> Option<PooledConnection> {
        let pool = match quality {
            ConnectionQuality::High => &self.high_quality,
            ConnectionQuality::Medium => &self.medium_quality,
            ConnectionQuality::Low => &self.low_quality,
        };

        let mut pool_guard = pool.write().await;

        // Find healthy connection
        while let Some(mut conn) = pool_guard.pop_front() {
            if conn.connection.is_connected() {
                conn.last_used = Instant::now();
                conn.usage_count += 1;

                // Move to active connections
                let _peer_id = conn.peer_id;

                return Some(conn);
            }
        }

        None
    }

    /// Return connection to pool with adaptive sizing
    pub async fn return_connection(&self, mut connection: PooledConnection) {
        // Remove from active connections
        self.active_connections
            .write()
            .await
            .remove(&connection.peer_id);

        // Check if connection should be retired
        if !connection.connection.is_connected()
            || connection.acquired_at.elapsed() > self.config.max_idle_time
        {
            connection.connection.close();
            self.update_metrics_on_close().await;
            return;
        }

        // Get connection quality and current load
        let quality = self.get_connection_quality(&connection.peer_id).await;
        let current_load = self.calculate_current_load().await;

        // Return to appropriate pool
        let pool = match quality {
            ConnectionQuality::High => &self.high_quality,
            ConnectionQuality::Medium => &self.medium_quality,
            ConnectionQuality::Low => &self.low_quality,
        };

        let mut pool_guard = pool.write().await;

        // Adaptive pool sizing based on quality tier and current load
        let max_idle_for_tier = self.calculate_max_idle_for_tier(quality, current_load);

        if pool_guard.len() < max_idle_for_tier {
            connection.last_used = Instant::now();
            pool_guard.push_back(connection);
        } else {
            // Close excess connection
            connection.connection.close();
        }

        self.update_return_metrics().await;
    }

    /// Calculate current system load for adaptive sizing
    async fn calculate_current_load(&self) -> f32 {
        let active_count = self.active_connections.read().await.len();
        let total_capacity = self.config.max_connections;
        
        if total_capacity == 0 {
            return 0.0;
        }
        
        active_count as f32 / total_capacity as f32
    }

    /// Calculate maximum idle connections for a tier based on quality and load
    fn calculate_max_idle_for_tier(&self, quality: ConnectionQuality, current_load: f32) -> usize {
        let base_allocation = match quality {
            ConnectionQuality::High => 0.5,   // 50% of pool for high quality
            ConnectionQuality::Medium => 0.35, // 35% for medium quality
            ConnectionQuality::Low => 0.15,   // 15% for low quality
        };
        
        // Adjust based on current load
        let load_multiplier = if current_load > 0.7 {
            1.3 // Keep more idle connections when load is high
        } else if current_load < 0.3 {
            0.6 // Keep fewer idle connections when load is low
        } else {
            1.0
        };
        
        ((self.config.max_connections as f32 * base_allocation * load_multiplier) as usize).max(1)
    }

    /// Update connection score
    pub async fn update_score(&self, peer_id: PeerId, latency_ms: f32, success: bool) {
        let mut scores = self.connection_scores.write().await;

        let score = scores.entry(peer_id).or_insert_with(|| ConnectionScore {
            peer_id,
            latency_ms: 50.0,
            packet_loss: 0.0,
            reliability_score: 1.0,
            bandwidth_mbps: 1.0,
            last_updated: Instant::now(),
            successful_messages: 0,
            failed_messages: 0,
        });

        // Update metrics with exponential moving average
        const ALPHA: f32 = 0.1; // Smoothing factor

        score.latency_ms = score.latency_ms * (1.0 - ALPHA) + latency_ms * ALPHA;

        if success {
            score.successful_messages += 1;
        } else {
            score.failed_messages += 1;
        }

        // Update packet loss
        let total = score.successful_messages + score.failed_messages;
        if total > 0 {
            score.packet_loss = score.failed_messages as f32 / total as f32;
        }

        // Update reliability score
        if total >= 10 {
            score.reliability_score = score.successful_messages as f32 / total as f32;
        }

        score.last_updated = Instant::now();
    }

    /// Get connection quality for peer
    async fn get_connection_quality(&self, peer_id: &PeerId) -> ConnectionQuality {
        let scores = self.connection_scores.read().await;

        if let Some(score) = scores.get(peer_id) {
            score.quality_tier()
        } else {
            ConnectionQuality::Medium // Default for unknown connections
        }
    }

    /// Perform health checks on idle connections with predictive eviction
    pub async fn health_check(&self) {
        let current_load = self.calculate_current_load().await;
        let now = Instant::now();

        // Check each pool with load-aware health checking
        for quality in [
            ConnectionQuality::High,
            ConnectionQuality::Medium,
            ConnectionQuality::Low,
        ] {
            let pool = match quality {
                ConnectionQuality::High => &self.high_quality,
                ConnectionQuality::Medium => &self.medium_quality,
                ConnectionQuality::Low => &self.low_quality,
            };

            let mut pool_guard = pool.write().await;
            let mut healthy_connections = VecDeque::new();
            let mut evicted_count = 0;

            // Adaptive idle timeout based on load and connection quality
            let adaptive_idle_timeout = self.calculate_adaptive_idle_timeout(quality, current_load);

            while let Some(conn) = pool_guard.pop_front() {
                let idle_duration = now.duration_since(conn.last_used);
                
                if conn.connection.is_connected() && idle_duration < adaptive_idle_timeout {
                    // Connection is healthy and within adaptive timeout
                    healthy_connections.push_back(conn);
                } else {
                    // Evict connection
                    conn.connection.close();
                    evicted_count += 1;
                }
            }

            *pool_guard = healthy_connections;

            // Update eviction metrics
            if evicted_count > 0 {
                let mut metrics = self.metrics.write().await;
                metrics.total_evictions += evicted_count;
            }
        }
    }

    /// Calculate adaptive idle timeout based on connection quality and system load
    fn calculate_adaptive_idle_timeout(&self, quality: ConnectionQuality, current_load: f32) -> Duration {
        let base_timeout = self.config.max_idle_time;
        
        // Quality-based multiplier
        let quality_multiplier = match quality {
            ConnectionQuality::High => 1.5,  // Keep high-quality connections longer
            ConnectionQuality::Medium => 1.0,
            ConnectionQuality::Low => 0.7,   // Evict low-quality connections sooner
        };
        
        // Load-based multiplier
        let load_multiplier = if current_load > 0.7 {
            1.2 // Keep connections longer when load is high (expect reuse)
        } else if current_load < 0.3 {
            0.6 // Evict connections sooner when load is low
        } else {
            1.0
        };
        
        let adjusted_seconds = (base_timeout.as_secs() as f32 * quality_multiplier * load_multiplier) as u64;
        Duration::from_secs(adjusted_seconds.max(30)) // Minimum 30 seconds
    }

    /// Update acquisition metrics
    async fn update_acquisition_metrics(&self, time_ms: f32, success: bool) {
        let mut metrics = self.metrics.write().await;

        metrics.connection_acquisitions += 1;

        if !success {
            metrics.connection_failures += 1;
        }

        // Update average acquisition time (exponential moving average)
        const ALPHA: f32 = 0.1;
        metrics.average_acquisition_time_ms =
            metrics.average_acquisition_time_ms * (1.0 - ALPHA) + time_ms * ALPHA;
    }

    /// Update return metrics
    async fn update_return_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.connection_releases += 1;

        // Calculate reuse rate
        if metrics.connection_acquisitions > 0 {
            metrics.connection_reuse_rate =
                metrics.connection_releases as f32 / metrics.connection_acquisitions as f32;
        }
    }

    /// Update metrics on connection close
    async fn update_metrics_on_close(&self) {
        let mut metrics = self.metrics.write().await;
        if metrics.total_connections > 0 {
            metrics.total_connections -= 1;
        }
    }

    /// Get current pool metrics with efficiency calculations
    pub async fn get_metrics(&self) -> PoolMetrics {
        let mut metrics = self.metrics.write().await;

        // Update current counts
        metrics.active_connections = self.active_connections.read().await.len();

        let high = self.high_quality.read().await.len();
        let medium = self.medium_quality.read().await.len();
        let low = self.low_quality.read().await.len();
        metrics.idle_connections = high + medium + low;

        metrics.total_connections = metrics.active_connections + metrics.idle_connections;

        // Calculate advanced efficiency metrics
        let pool_utilization = if self.config.max_connections > 0 {
            metrics.active_connections as f32 / self.config.max_connections as f32
        } else {
            0.0
        };

        // Update connection reuse rate with efficiency scoring
        if metrics.connection_acquisitions > 0 {
            let base_reuse_rate = metrics.connection_releases as f32 / metrics.connection_acquisitions as f32;
            
            // Efficiency bonus for balanced pool utilization (50-80% is optimal)
            let efficiency_bonus = if pool_utilization >= 0.5 && pool_utilization <= 0.8 {
                1.1 // 10% efficiency bonus for optimal utilization
            } else if pool_utilization > 0.8 {
                0.9 // Slight penalty for over-utilization
            } else {
                0.95 // Small penalty for under-utilization
            };
            
            metrics.connection_reuse_rate = base_reuse_rate * efficiency_bonus;
        }

        metrics.clone()
    }

    /// Get detailed pool efficiency report
    pub async fn get_efficiency_report(&self) -> PoolEfficiencyReport {
        let metrics = self.get_metrics().await;
        let current_load = self.calculate_current_load().await;
        
        let high_count = self.high_quality.read().await.len();
        let medium_count = self.medium_quality.read().await.len();
        let low_count = self.low_quality.read().await.len();
        
        PoolEfficiencyReport {
            overall_utilization: current_load,
            tier_distribution: TierDistribution {
                high_quality_percentage: (high_count as f32 / metrics.idle_connections as f32 * 100.0),
                medium_quality_percentage: (medium_count as f32 / metrics.idle_connections as f32 * 100.0),
                low_quality_percentage: (low_count as f32 / metrics.idle_connections as f32 * 100.0),
            },
            efficiency_score: self.calculate_efficiency_score(&metrics, current_load),
            recommended_adjustments: self.get_optimization_recommendations(&metrics, current_load),
        }
    }

    /// Calculate overall pool efficiency score (0.0 to 1.0)
    fn calculate_efficiency_score(&self, metrics: &PoolMetrics, current_load: f32) -> f32 {
        let mut score = 0.0;
        
        // Utilization score (optimal: 50-80%)
        let utilization_score = if current_load >= 0.5 && current_load <= 0.8 {
            1.0
        } else if current_load > 0.8 {
            (1.0 - (current_load - 0.8) * 2.0).max(0.0) // Penalty for over-utilization
        } else {
            current_load / 0.5 // Proportional score for under-utilization
        };
        score += utilization_score * 0.4;
        
        // Reuse rate score
        let reuse_score = metrics.connection_reuse_rate.min(1.0);
        score += reuse_score * 0.3;
        
        // Failure rate score (inverse of failure rate)
        let failure_rate = if metrics.connection_acquisitions > 0 {
            metrics.connection_failures as f32 / metrics.connection_acquisitions as f32
        } else {
            0.0
        };
        let failure_score = (1.0 - failure_rate).max(0.0);
        score += failure_score * 0.3;
        
        score.clamp(0.0, 1.0)
    }

    /// Get optimization recommendations based on current metrics
    fn get_optimization_recommendations(&self, metrics: &PoolMetrics, current_load: f32) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        if current_load > 0.9 {
            recommendations.push("Consider increasing max_connections - pool is over-utilized".to_string());
        } else if current_load < 0.3 {
            recommendations.push("Consider decreasing max_connections - pool is under-utilized".to_string());
        }
        
        if metrics.connection_reuse_rate < 0.5 {
            recommendations.push("Low connection reuse - consider increasing idle timeout".to_string());
        }
        
        let failure_rate = if metrics.connection_acquisitions > 0 {
            metrics.connection_failures as f32 / metrics.connection_acquisitions as f32
        } else {
            0.0
        };
        
        if failure_rate > 0.1 {
            recommendations.push("High failure rate - investigate connection stability".to_string());
        }
        
        if metrics.average_acquisition_time_ms > 100.0 {
            recommendations.push("High acquisition latency - consider tuning pool parameters".to_string());
        }
        
        recommendations
    }

    /// Shutdown pool and close all connections
    pub async fn shutdown(&self) {
        // Close active connections
        let active = self.active_connections.write().await;
        for (_, conn) in active.iter() {
            conn.connection.close();
        }

        // Close idle connections
        for pool in [&self.high_quality, &self.medium_quality, &self.low_quality] {
            let mut pool_guard = pool.write().await;
            while let Some(conn) = pool_guard.pop_front() {
                conn.connection.close();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConnection {
        peer_id: PeerId,
        connected: Arc<RwLock<bool>>,
    }

    impl ConnectionHandle for MockConnection {
        fn is_connected(&self) -> bool {
            futures::executor::block_on(self.connected.read()).clone()
        }

        fn peer_id(&self) -> PeerId {
            self.peer_id
        }

        fn close(&self) {
            *futures::executor::block_on(self.connected.write()) = false;
        }
    }

    #[tokio::test]
    async fn test_connection_pooling() {
        let pool = BluetoothConnectionPool::new(PoolConfig::default());

        // Create mock connection
        let peer_id = [1u8; 32];
        let mock_conn = Arc::new(MockConnection {
            peer_id,
            connected: Arc::new(RwLock::new(true)),
        });

        let pooled = PooledConnection {
            peer_id,
            connection: mock_conn,
            acquired_at: Instant::now(),
            last_used: Instant::now(),
            usage_count: 0,
        };

        // Return connection to pool
        pool.return_connection(pooled).await;

        // Should be able to get it back
        let conn = pool.get_connection(QoSPriority::Normal).await;
        assert!(conn.is_ok());
    }

    #[tokio::test]
    async fn test_connection_scoring() {
        let pool = BluetoothConnectionPool::new(PoolConfig::default());
        let peer_id = [2u8; 32];

        // Update scores
        pool.update_score(peer_id, 10.0, true).await;
        pool.update_score(peer_id, 15.0, true).await;
        pool.update_score(peer_id, 20.0, false).await;

        // Check quality
        let quality = pool.get_connection_quality(&peer_id).await;
        assert_eq!(quality, ConnectionQuality::High); // Should still be high quality
    }
}
