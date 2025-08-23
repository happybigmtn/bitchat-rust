//! Enhanced connection pooling for Bluetooth transport

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use crate::protocol::PeerId;
use crate::error::Result;

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
        (latency_score * 0.3 + loss_score * 0.4 + reliability * 0.3).max(0.0).min(1.0)
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

/// Enhanced Bluetooth connection pool
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
}

/// Pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
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

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections: 500,  // 10x improvement from original 50
            max_idle_per_tier: 50,
            idle_timeout: Duration::from_secs(300),  // 5 minutes
            max_lifetime: Duration::from_secs(3600),  // 1 hour
            health_check_interval: Duration::from_secs(30),
        }
    }
}

/// Pool metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct PoolMetrics {
    pub total_connections: usize,
    pub active_connections: usize,
    pub idle_connections: usize,
    pub connection_acquisitions: u64,
    pub connection_releases: u64,
    pub connection_failures: u64,
    pub average_acquisition_time_ms: f32,
    pub connection_reuse_rate: f32,
}

impl BluetoothConnectionPool {
    /// Create new connection pool
    pub fn new(config: PoolConfig) -> Self {
        let max_connections = config.max_connections;
        
        Self {
            high_quality: Arc::new(RwLock::new(VecDeque::new())),
            medium_quality: Arc::new(RwLock::new(VecDeque::new())),
            low_quality: Arc::new(RwLock::new(VecDeque::new())),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            connection_scores: Arc::new(RwLock::new(HashMap::new())),
            config,
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            metrics: Arc::new(RwLock::new(PoolMetrics::default())),
        }
    }
    
    /// Get best available connection for requirements
    pub async fn get_connection(&self, requirements: QoSPriority) -> Result<PooledConnection> {
        let start_time = Instant::now();
        
        // Acquire semaphore permit
        let _permit = self.connection_semaphore.acquire().await
            .map_err(|_| crate::error::Error::Network("Connection limit reached".to_string()))?;
        
        // Try to get connection from appropriate pool
        let connection = match requirements {
            QoSPriority::RealTime => {
                self.get_from_tier(ConnectionQuality::High).await
                    .or(self.get_from_tier(ConnectionQuality::Medium).await)
            }
            QoSPriority::Normal => {
                self.get_from_tier(ConnectionQuality::Medium).await
                    .or(self.get_from_tier(ConnectionQuality::High).await)
                    .or(self.get_from_tier(ConnectionQuality::Low).await)
            }
            QoSPriority::Background => {
                self.get_from_tier(ConnectionQuality::Low).await
                    .or(self.get_from_tier(ConnectionQuality::Medium).await)
            }
        };
        
        // Update metrics
        let acquisition_time = start_time.elapsed().as_millis() as f32;
        self.update_acquisition_metrics(acquisition_time, connection.is_some()).await;
        
        connection.ok_or_else(|| crate::error::Error::Network("No connections available".to_string()))
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
    
    /// Return connection to pool
    pub async fn return_connection(&self, mut connection: PooledConnection) {
        // Remove from active connections
        self.active_connections.write().await.remove(&connection.peer_id);
        
        // Check if connection should be retired
        if !connection.connection.is_connected() 
            || connection.acquired_at.elapsed() > self.config.max_lifetime {
            connection.connection.close();
            self.update_metrics_on_close().await;
            return;
        }
        
        // Get connection quality
        let quality = self.get_connection_quality(&connection.peer_id).await;
        
        // Return to appropriate pool
        let pool = match quality {
            ConnectionQuality::High => &self.high_quality,
            ConnectionQuality::Medium => &self.medium_quality,
            ConnectionQuality::Low => &self.low_quality,
        };
        
        let mut pool_guard = pool.write().await;
        
        // Respect max idle limit
        if pool_guard.len() < self.config.max_idle_per_tier {
            connection.last_used = Instant::now();
            pool_guard.push_back(connection);
        } else {
            // Close excess connection
            connection.connection.close();
        }
        
        self.update_return_metrics().await;
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
        const ALPHA: f32 = 0.1;  // Smoothing factor
        
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
            ConnectionQuality::Medium  // Default for unknown connections
        }
    }
    
    /// Perform health checks on idle connections
    pub async fn health_check(&self) {
        // Check each pool
        for quality in [ConnectionQuality::High, ConnectionQuality::Medium, ConnectionQuality::Low] {
            let pool = match quality {
                ConnectionQuality::High => &self.high_quality,
                ConnectionQuality::Medium => &self.medium_quality,
                ConnectionQuality::Low => &self.low_quality,
            };
            
            let mut pool_guard = pool.write().await;
            let mut healthy_connections = VecDeque::new();
            
            while let Some(conn) = pool_guard.pop_front() {
                if conn.connection.is_connected() 
                    && conn.last_used.elapsed() < self.config.idle_timeout {
                    healthy_connections.push_back(conn);
                } else {
                    conn.connection.close();
                }
            }
            
            *pool_guard = healthy_connections;
        }
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
    
    /// Get current pool metrics
    pub async fn get_metrics(&self) -> PoolMetrics {
        let mut metrics = self.metrics.write().await;
        
        // Update current counts
        metrics.active_connections = self.active_connections.read().await.len();
        
        let high = self.high_quality.read().await.len();
        let medium = self.medium_quality.read().await.len();
        let low = self.low_quality.read().await.len();
        metrics.idle_connections = high + medium + low;
        
        metrics.total_connections = metrics.active_connections + metrics.idle_connections;
        
        metrics.clone()
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
        assert_eq!(quality, ConnectionQuality::High);  // Should still be high quality
    }
}