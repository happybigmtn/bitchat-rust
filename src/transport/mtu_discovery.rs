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

    /// Adaptive MTU based on network conditions
    pub async fn adaptive_mtu(&self, peer: &PeerId, packet_size: usize) -> Vec<Vec<u8>> {
        let mtu = self.get_mtu(peer).await;

        // Account for protocol overhead (headers, encryption, etc.)
        let effective_mtu = mtu.saturating_sub(20); // Conservative overhead estimate

        if packet_size <= effective_mtu {
            // No fragmentation needed
            return vec![];
        }

        // Fragment into MTU-sized chunks
        let num_fragments = packet_size.div_ceil(effective_mtu);
        let mut fragments = Vec::with_capacity(num_fragments);

        for i in 0..num_fragments {
            let start = i * effective_mtu;
            let end = ((i + 1) * effective_mtu).min(packet_size);
            let fragment_size = end - start;

            // Add fragment header (simplified)
            let mut fragment = Vec::with_capacity(fragment_size + 4);
            fragment.push((i as u8) << 4 | (num_fragments as u8)); // Fragment info
            fragment.push((fragment_size >> 8) as u8); // Size high byte
            fragment.push((fragment_size & 0xFF) as u8); // Size low byte
            fragment.push(0); // Reserved/flags

            fragments.push(fragment);
        }

        fragments
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
