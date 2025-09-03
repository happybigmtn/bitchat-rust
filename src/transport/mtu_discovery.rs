//! Adaptive MTU discovery for optimal Bluetooth packet sizes

use crate::error::Result;
use crate::protocol::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// MTU discovery constants
const MIN_MTU: usize = 23; // BLE minimum
const MAX_MTU: usize = 512; // Conservative maximum for compatibility
const DEFAULT_MTU: usize = 247; // BLE 4.2 default
const _PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const MTU_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

/// MTU metrics for monitoring
#[derive(Debug, Clone, Default)]
pub struct MtuMetrics {
    pub discovery_attempts: u64,
    pub discovery_successes: u64,
    pub average_mtu: f64,
    pub min_discovered: usize,
    pub max_discovered: usize,
    pub fragmentation_events: u64,
    pub reassembly_timeouts: u64,
    pub fragment_loss_rate: f32,
}

/// Fragmentation policy for different network conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentationPolicy {
    /// Conservative: Smaller fragments, higher reliability
    Conservative,
    /// Adaptive: Balance between reliability and throughput
    Adaptive,
    /// Aggressive: Larger fragments, maximum throughput
    Aggressive,
}

/// Network fragment with metadata
#[derive(Debug, Clone)]
pub struct Fragment {
    pub id: u16,
    pub index: u16,
    pub total: u16,
    pub data: Vec<u8>,
    pub timestamp: std::time::Instant,
}

impl Fragment {
    /// Convert fragment to wire format with header
    pub fn to_wire_format(&self) -> Vec<u8> {
        let mut wire = Vec::with_capacity(self.data.len() + 8);

        // Fragment header
        wire.extend_from_slice(&self.id.to_be_bytes());
        wire.extend_from_slice(&self.index.to_be_bytes());
        wire.extend_from_slice(&self.total.to_be_bytes());
        wire.extend_from_slice(&(self.data.len() as u16).to_be_bytes());

        // Payload
        wire.extend_from_slice(&self.data);

        wire
    }

    /// Parse fragment from wire format
    pub fn from_wire_format(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let id = u16::from_be_bytes([data[0], data[1]]);
        let index = u16::from_be_bytes([data[2], data[3]]);
        let total = u16::from_be_bytes([data[4], data[5]]);
        let payload_len = u16::from_be_bytes([data[6], data[7]]) as usize;

        if data.len() < 8 + payload_len {
            return None;
        }

        Some(Fragment {
            id,
            index,
            total,
            data: data[8..8 + payload_len].to_vec(),
            timestamp: std::time::Instant::now(),
        })
    }
}

/// MTU probe result
#[derive(Debug, Clone)]
struct MtuProbe {
    _peer_id: PeerId,
    _tested_size: usize,
    _success: bool,
    _latency_ms: u64,
    _timestamp: Instant,
}

/// Cached MTU information
#[derive(Debug, Clone)]
struct CachedMtu {
    mtu_size: usize,
    discovered_at: Instant,
    _probe_count: u32,
    last_verified: Instant,
}

/// Adaptive MTU discovery system
pub struct AdaptiveMTU {
    /// Discovered MTU values per peer
    discovered_mtu: Arc<RwLock<HashMap<PeerId, CachedMtu>>>,
    /// Performance metrics
    metrics: Arc<RwLock<MtuMetrics>>,
    /// Probe history for analysis
    probe_history: Arc<RwLock<Vec<MtuProbe>>>,
}

impl Default for AdaptiveMTU {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveMTU {
    /// Create new MTU discovery system
    pub fn new() -> Self {
        Self {
            discovered_mtu: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(MtuMetrics::default())),
            probe_history: Arc::new(RwLock::new(Vec::with_capacity(1000))),
        }
    }

    /// Get optimal MTU for a peer
    pub async fn get_mtu(&self, peer: &PeerId) -> usize {
        let cache = self.discovered_mtu.read().await;

        if let Some(cached) = cache.get(peer) {
            // Check if cache is still valid
            if cached.discovered_at.elapsed() < MTU_CACHE_TTL {
                return cached.mtu_size;
            }
        }

        // Return conservative default if not discovered
        DEFAULT_MTU
    }

    /// Discover MTU for a peer using binary search
    pub async fn discover_mtu(
        &self,
        peer: PeerId,
        probe_fn: impl Fn(usize) -> bool,
    ) -> Result<usize> {
        let mut metrics = self.metrics.write().await;
        metrics.discovery_attempts += 1;

        let start_time = Instant::now();
        let mut low = MIN_MTU;
        let mut high = MAX_MTU;
        let mut best_mtu = DEFAULT_MTU;
        let mut probe_count = 0;

        // Binary search for optimal MTU
        while low <= high && probe_count < 10 {
            // Limit probes to avoid excessive testing
            let test_size = (low + high) / 2;

            // Test MTU size
            let probe_start = Instant::now();
            let success = probe_fn(test_size);
            let latency_ms = probe_start.elapsed().as_millis() as u64;

            // Record probe result
            let probe = MtuProbe {
                _peer_id: peer,
                _tested_size: test_size,
                _success: success,
                _latency_ms: latency_ms,
                _timestamp: Instant::now(),
            };

            self.record_probe(probe.clone()).await;
            probe_count += 1;

            if success {
                best_mtu = test_size;
                low = test_size + 1;
            } else {
                high = test_size - 1;
            }
        }

        // Apply safety margin (5% reduction)
        let final_mtu = (best_mtu * 95) / 100;
        let final_mtu = final_mtu.max(MIN_MTU);

        // Update cache
        let cached = CachedMtu {
            mtu_size: final_mtu,
            discovered_at: Instant::now(),
            _probe_count: probe_count as u32,
            last_verified: Instant::now(),
        };

        let mut cache = self.discovered_mtu.write().await;
        cache.insert(peer, cached);

        // Update metrics
        metrics.discovery_successes += 1;
        self.update_metrics(final_mtu).await;

        log::info!(
            "MTU discovery for peer {:?}: {} bytes ({}ms, {} probes)",
            peer,
            final_mtu,
            start_time.elapsed().as_millis(),
            probe_count
        );

        Ok(final_mtu)
    }

    /// Adaptive fragmentation with policy-based MTU handling
    pub async fn fragment_packet(
        &self,
        peer: &PeerId,
        data: &[u8],
        policy: FragmentationPolicy,
    ) -> Result<Vec<Fragment>> {
        let mtu = self.get_mtu(peer).await;
        let effective_mtu = self.calculate_effective_mtu(mtu, policy).await;

        if data.len() <= effective_mtu {
            // No fragmentation needed - return single fragment
            return Ok(vec![Fragment {
                id: 0,
                index: 0,
                total: 1,
                data: data.to_vec(),
                timestamp: std::time::Instant::now(),
            }]);
        }

        // Fragment into appropriately sized chunks based on policy
        let fragment_id = rand::random::<u16>();
        let num_fragments = data.len().div_ceil(effective_mtu);
        let mut fragments = Vec::with_capacity(num_fragments);

        for i in 0..num_fragments {
            let start = i * effective_mtu;
            let end = ((i + 1) * effective_mtu).min(data.len());

            fragments.push(Fragment {
                id: fragment_id,
                index: i as u16,
                total: num_fragments as u16,
                data: data[start..end].to_vec(),
                timestamp: std::time::Instant::now(),
            });
        }

        Ok(fragments)
    }

    /// Calculate effective MTU based on fragmentation policy
    async fn calculate_effective_mtu(&self, raw_mtu: usize, policy: FragmentationPolicy) -> usize {
        let overhead = match policy {
            FragmentationPolicy::Conservative => 32, // Extra safety margin
            FragmentationPolicy::Adaptive => 24,     // Standard protocol overhead
            FragmentationPolicy::Aggressive => 16,   // Minimal overhead for max throughput
        };

        let base_mtu = raw_mtu.saturating_sub(overhead);

        // Apply policy-specific adjustments
        match policy {
            FragmentationPolicy::Conservative => (base_mtu * 85) / 100, // 15% safety margin
            FragmentationPolicy::Adaptive => base_mtu,
            FragmentationPolicy::Aggressive => base_mtu,
        }
    }

    /// Legacy method for backward compatibility
    pub async fn adaptive_mtu(&self, peer: &PeerId, packet_size: usize) -> Vec<Vec<u8>> {
        match self
            .fragment_packet(peer, &vec![0; packet_size], FragmentationPolicy::Adaptive)
            .await
        {
            Ok(fragments) => fragments.into_iter().map(|f| f.to_wire_format()).collect(),
            Err(_) => vec![], // Return empty on error
        }
    }

    /// Periodic MTU verification for cached values
    pub async fn verify_cached_mtu(
        &self,
        peer: PeerId,
        probe_fn: impl Fn(usize) -> bool,
    ) -> Result<()> {
        let should_verify = {
            let cache = self.discovered_mtu.read().await;
            if let Some(cached) = cache.get(&peer) {
                // Verify if last check was over 5 minutes ago
                cached.last_verified.elapsed() > Duration::from_secs(300)
            } else {
                false
            }
        };

        if should_verify {
            let current_mtu = self.get_mtu(&peer).await;

            // Quick verification probe
            if probe_fn(current_mtu) {
                // MTU still valid, update timestamp
                let mut cache = self.discovered_mtu.write().await;
                if let Some(cached) = cache.get_mut(&peer) {
                    cached.last_verified = Instant::now();
                }
            } else {
                // MTU no longer valid, rediscover
                log::warn!("MTU verification failed for peer {:?}, rediscovering", peer);
                self.discover_mtu(peer, probe_fn).await?;
            }
        }

        Ok(())
    }

    /// Reassemble fragments into complete message
    pub async fn reassemble_fragments(
        &self,
        fragments: Vec<Fragment>,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        if fragments.is_empty() {
            return Err(crate::error::Error::Network(
                "No fragments provided for reassembly".to_string(),
            ));
        }

        // Validate all fragments belong to same message
        let fragment_id = fragments[0].id;
        let total_expected = fragments[0].total;

        if !fragments
            .iter()
            .all(|f| f.id == fragment_id && f.total == total_expected)
        {
            return Err(crate::error::Error::Network(
                "Fragment validation failed: inconsistent fragment set".to_string(),
            ));
        }

        // Check for timeout
        let oldest_fragment = fragments.iter().min_by_key(|f| f.timestamp).unwrap();

        if oldest_fragment.timestamp.elapsed() > timeout {
            let mut metrics = self.metrics.write().await;
            metrics.reassembly_timeouts += 1;
            return Err(crate::error::Error::Network(
                "Fragment reassembly timeout".to_string(),
            ));
        }

        // Sort fragments by index
        let mut sorted_fragments = fragments;
        sorted_fragments.sort_by_key(|f| f.index);

        // Check for missing fragments
        for (i, fragment) in sorted_fragments.iter().enumerate() {
            if fragment.index != i as u16 {
                return Err(crate::error::Error::Network(format!(
                    "Missing fragment at index {}",
                    i
                )));
            }
        }

        // Reassemble data
        let mut reassembled = Vec::new();
        for fragment in sorted_fragments {
            reassembled.extend_from_slice(&fragment.data);
        }

        Ok(reassembled)
    }

    /// Record probe result for analysis
    async fn record_probe(&self, probe: MtuProbe) {
        let mut history = self.probe_history.write().await;

        // Keep last 1000 probes for analysis
        if history.len() >= 1000 {
            history.remove(0);
        }

        history.push(probe);
    }

    /// Update metrics with discovered MTU
    async fn update_metrics(&self, mtu: usize) {
        let mut metrics = self.metrics.write().await;

        // Update min/max
        if metrics.min_discovered == 0 || mtu < metrics.min_discovered {
            metrics.min_discovered = mtu;
        }
        if mtu > metrics.max_discovered {
            metrics.max_discovered = mtu;
        }

        // Update average (simple moving average)
        let success_count = metrics.discovery_successes as f64;
        if success_count > 0.0 {
            metrics.average_mtu =
                (metrics.average_mtu * (success_count - 1.0) + mtu as f64) / success_count;
        } else {
            metrics.average_mtu = mtu as f64;
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> MtuMetrics {
        self.metrics.read().await.clone()
    }

    /// Clear cached MTU for a peer
    pub async fn clear_cache(&self, peer: &PeerId) {
        let mut cache = self.discovered_mtu.write().await;
        cache.remove(peer);
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.discovered_mtu.read().await;
        let total = cache.len();
        let valid = cache
            .values()
            .filter(|c| c.discovered_at.elapsed() < MTU_CACHE_TTL)
            .count();
        (total, valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mtu_discovery() {
        let mtu = AdaptiveMTU::new();
        let peer = [1u8; 32];

        // Simulate probe function that accepts up to 400 bytes
        let probe_fn = |size: usize| -> bool { size <= 400 };

        let discovered = mtu.discover_mtu(peer, probe_fn).await.unwrap();

        // Should discover ~380 bytes (400 * 0.95 safety margin)
        assert!(discovered >= 350 && discovered <= 400);

        // Should be cached
        let cached = mtu.get_mtu(&peer).await;
        assert_eq!(cached, discovered);
    }

    #[tokio::test]
    async fn test_fragmentation() {
        let mtu = AdaptiveMTU::new();
        let peer = [2u8; 32];

        // Set a known MTU
        let cached = CachedMtu {
            mtu_size: 100,
            discovered_at: Instant::now(),
            _probe_count: 1,
            last_verified: Instant::now(),
        };

        mtu.discovered_mtu.write().await.insert(peer, cached);

        // Test fragmentation
        let fragments = mtu.adaptive_mtu(&peer, 250).await;

        // Should create 3 fragments (250 / 80 effective MTU)
        assert_eq!(fragments.len(), 3);
    }
}
