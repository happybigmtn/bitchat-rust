//! Resource Quota Management for Per-Peer Limits
//!
//! This module implements comprehensive resource quotas to prevent any single peer
//! from exhausting system resources through malicious or buggy behavior.

use crate::protocol::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Resource types that can be limited
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ResourceType {
    /// Network bandwidth in bytes/second
    Bandwidth,
    /// Number of concurrent connections
    Connections,
    /// Messages per second
    MessageRate,
    /// Memory usage in bytes
    Memory,
    /// CPU time in milliseconds/second
    CpuTime,
    /// Storage space in bytes
    Storage,
    /// Pending operations count
    PendingOps,
    /// Transaction rate per second
    TransactionRate,
}

/// Resource quota configuration per peer
#[derive(Debug, Clone)]
pub struct QuotaConfig {
    // TODO: [Security] Implement dynamic quota adjustment based on peer reputation
    //       - Track peer behavior history for trust scoring
    //       - Increase quotas for well-behaved peers
    //       - Implement gradual quota recovery after violations
    //       - Add emergency circuit breakers for system protection
    //       Priority: MEDIUM - Important for production scalability
    /// Maximum bandwidth in bytes/second
    pub max_bandwidth: u64,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Maximum messages per second
    pub max_message_rate: u32,
    /// Maximum memory usage in bytes
    pub max_memory: usize,
    /// Maximum CPU time in ms/second
    pub max_cpu_ms: u32,
    /// Maximum storage in bytes
    pub max_storage: u64,
    /// Maximum pending operations
    pub max_pending_ops: usize,
    /// Maximum transactions per second
    pub max_tx_rate: u32,
    /// Time window for rate calculations
    pub window_duration: Duration,
    /// Penalty duration for quota violations
    pub penalty_duration: Duration,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            max_bandwidth: 10 * 1024 * 1024, // 10 MB/s
            max_connections: 10,             // 10 concurrent
            max_message_rate: 100,           // 100 msg/s
            max_memory: 50 * 1024 * 1024,    // 50 MB
            max_cpu_ms: 100,                 // 100ms CPU/s (10%)
            max_storage: 100 * 1024 * 1024,  // 100 MB
            max_pending_ops: 50,             // 50 pending
            max_tx_rate: 10,                 // 10 tx/s
            window_duration: Duration::from_secs(1),
            penalty_duration: Duration::from_secs(60),
        }
    }
}

/// Tracks resource usage for a single peer
#[derive(Debug)]
struct PeerQuota {
    /// Current usage counters
    usage: HashMap<ResourceType, u64>,
    /// Window start time for rate calculations
    window_start: Instant,
    /// Number of violations
    violations: u32,
    /// Penalty expiry time (if penalized)
    penalty_until: Option<Instant>,
    /// Historical usage for trending
    history: Vec<(Instant, HashMap<ResourceType, u64>)>,
}

impl PeerQuota {
    fn new() -> Self {
        Self {
            usage: HashMap::with_capacity(8),
            window_start: Instant::now(),
            violations: 0,
            penalty_until: None,
            history: Vec::with_capacity(60), // Keep 1 minute of history
        }
    }

    /// Reset usage counters for new time window
    fn reset_window(&mut self) {
        // Save to history before reset
        if !self.usage.is_empty() {
            self.history.push((self.window_start, self.usage.clone()));
            // Keep only last 60 entries
            if self.history.len() > 60 {
                self.history.remove(0);
            }
        }

        self.usage.clear();
        self.window_start = Instant::now();
    }

    /// Check if peer is currently penalized
    fn is_penalized(&self) -> bool {
        self.penalty_until
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    /// Apply penalty for quota violation
    fn apply_penalty(&mut self, duration: Duration) {
        self.violations += 1;
        // Exponential backoff for repeat offenders
        let penalty_multiplier = 2_u32.pow(self.violations.min(5));
        let penalty_duration = duration * penalty_multiplier;
        self.penalty_until = Some(Instant::now() + penalty_duration);
    }
}

/// Resource quota manager for all peers
pub struct ResourceQuotaManager {
    /// Per-peer quota tracking
    peer_quotas: Arc<RwLock<HashMap<PeerId, PeerQuota>>>,
    /// Global quota configuration
    config: Arc<RwLock<QuotaConfig>>,
    /// Special quotas for trusted peers
    trusted_peers: Arc<RwLock<HashMap<PeerId, QuotaConfig>>>,
}

impl ResourceQuotaManager {
    /// Create new quota manager with default config
    pub fn new() -> Self {
        Self {
            peer_quotas: Arc::new(RwLock::new(HashMap::with_capacity(100))),
            config: Arc::new(RwLock::new(QuotaConfig::default())),
            trusted_peers: Arc::new(RwLock::new(HashMap::with_capacity(10))),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: QuotaConfig) -> Self {
        Self {
            peer_quotas: Arc::new(RwLock::new(HashMap::with_capacity(100))),
            config: Arc::new(RwLock::new(config)),
            trusted_peers: Arc::new(RwLock::new(HashMap::with_capacity(10))),
        }
    }

    /// Check if a resource request should be allowed
    pub async fn check_quota(
        &self,
        peer_id: &PeerId,
        resource: ResourceType,
        amount: u64,
    ) -> Result<(), QuotaViolation> {
        let mut quotas = self.peer_quotas.write().await;
        let quota = quotas.entry(*peer_id).or_insert_with(PeerQuota::new);

        // Check if penalized
        if quota.is_penalized() {
            return Err(QuotaViolation::Penalized {
                until: quota.penalty_until.unwrap(),
            });
        }

        // Reset window if needed
        let config = self.config.read().await;
        if quota.window_start.elapsed() > config.window_duration {
            quota.reset_window();
        }

        // Get limit for this resource
        let limit = self.get_limit(&resource, &config).await;

        // Check current usage
        let current = quota.usage.get(&resource).copied().unwrap_or(0);
        if current + amount > limit {
            quota.apply_penalty(config.penalty_duration);
            return Err(QuotaViolation::ExceededLimit {
                resource,
                requested: amount,
                current,
                limit,
            });
        }

        // Update usage
        *quota.usage.entry(resource).or_insert(0) += amount;

        Ok(())
    }

    /// Record resource consumption (after the fact)
    pub async fn record_usage(&self, peer_id: &PeerId, resource: ResourceType, amount: u64) {
        let mut quotas = self.peer_quotas.write().await;
        let quota = quotas.entry(*peer_id).or_insert_with(PeerQuota::new);

        // Reset window if needed
        let config = self.config.read().await;
        if quota.window_start.elapsed() > config.window_duration {
            quota.reset_window();
        }

        *quota.usage.entry(resource).or_insert(0) += amount;
    }

    /// Get current usage for a peer
    pub async fn get_usage(&self, peer_id: &PeerId) -> HashMap<ResourceType, u64> {
        let quotas = self.peer_quotas.read().await;
        quotas
            .get(peer_id)
            .map(|q| q.usage.clone())
            .unwrap_or_default()
    }

    /// Add a trusted peer with higher quotas
    pub async fn add_trusted_peer(&self, peer_id: PeerId, config: QuotaConfig) {
        let mut trusted = self.trusted_peers.write().await;
        trusted.insert(peer_id, config);
    }

    /// Remove a peer from trusted list
    pub async fn remove_trusted_peer(&self, peer_id: &PeerId) {
        let mut trusted = self.trusted_peers.write().await;
        trusted.remove(peer_id);
    }

    /// Clear penalty for a peer
    pub async fn clear_penalty(&self, peer_id: &PeerId) {
        let mut quotas = self.peer_quotas.write().await;
        if let Some(quota) = quotas.get_mut(peer_id) {
            quota.penalty_until = None;
            quota.violations = 0;
        }
    }

    /// Get resource limit for a specific resource type
    async fn get_limit(&self, resource: &ResourceType, config: &QuotaConfig) -> u64 {
        match resource {
            ResourceType::Bandwidth => config.max_bandwidth,
            ResourceType::Connections => config.max_connections as u64,
            ResourceType::MessageRate => config.max_message_rate as u64,
            ResourceType::Memory => config.max_memory as u64,
            ResourceType::CpuTime => config.max_cpu_ms as u64,
            ResourceType::Storage => config.max_storage,
            ResourceType::PendingOps => config.max_pending_ops as u64,
            ResourceType::TransactionRate => config.max_tx_rate as u64,
        }
    }

    /// Get statistics for monitoring
    pub async fn get_statistics(&self) -> QuotaStatistics {
        let quotas = self.peer_quotas.read().await;

        let total_peers = quotas.len();
        let penalized_peers = quotas.values().filter(|q| q.is_penalized()).count();

        let mut resource_usage = HashMap::with_capacity(8);
        for quota in quotas.values() {
            for (resource, amount) in &quota.usage {
                *resource_usage.entry(resource.clone()).or_insert(0) += amount;
            }
        }

        QuotaStatistics {
            total_peers,
            penalized_peers,
            resource_usage,
        }
    }

    /// Cleanup expired penalties and old peer data
    pub async fn cleanup(&self) {
        let mut quotas = self.peer_quotas.write().await;
        let now = Instant::now();

        // Remove peers with no recent activity
        quotas.retain(|_, quota| {
            // Keep if penalized, has recent usage, or recent window
            quota.is_penalized()
                || !quota.usage.is_empty()
                || quota.window_start.elapsed() < Duration::from_secs(300)
        });

        // Clear expired penalties
        for quota in quotas.values_mut() {
            if let Some(until) = quota.penalty_until {
                if now >= until {
                    quota.penalty_until = None;
                }
            }
        }
    }
}

/// Quota violation error types
#[derive(Debug, Clone)]
pub enum QuotaViolation {
    /// Peer exceeded resource limit
    ExceededLimit {
        resource: ResourceType,
        requested: u64,
        current: u64,
        limit: u64,
    },
    /// Peer is penalized until specified time
    Penalized { until: Instant },
}

/// Statistics for monitoring quota system
#[derive(Debug)]
pub struct QuotaStatistics {
    /// Total number of tracked peers
    pub total_peers: usize,
    /// Number of currently penalized peers
    pub penalized_peers: usize,
    /// Total resource usage across all peers
    pub resource_usage: HashMap<ResourceType, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PeerIdExt;

    #[tokio::test]
    async fn test_quota_enforcement() {
        let manager = ResourceQuotaManager::new();
        let peer_id = PeerId::random();

        // Should allow within quota
        assert!(manager
            .check_quota(&peer_id, ResourceType::MessageRate, 50)
            .await
            .is_ok());

        // Should reject over quota
        assert!(manager
            .check_quota(&peer_id, ResourceType::MessageRate, 100)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_penalty_system() {
        let manager = ResourceQuotaManager::new();
        let peer_id = PeerId::random();

        // Exceed quota to trigger penalty
        let _ = manager
            .check_quota(&peer_id, ResourceType::MessageRate, 200)
            .await;

        // Should be penalized
        assert!(matches!(
            manager
                .check_quota(&peer_id, ResourceType::MessageRate, 1)
                .await,
            Err(QuotaViolation::Penalized { .. })
        ));
    }
}
