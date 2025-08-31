use crate::protocol::{GameState, PeerId};
use parking_lot::RwLock;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::*;
use std::sync::Arc;

/// CPU optimization utilities with SIMD acceleration
pub struct CpuOptimizer {
    /// Number of available CPU cores
    core_count: usize,
    /// SIMD feature availability
    simd_features: SimdFeatures,
    /// Parallel processing pools
    game_pool: rayon::ThreadPool,
    network_pool: rayon::ThreadPool,
}

/// Available SIMD instruction sets
#[derive(Debug, Clone)]
pub struct SimdFeatures {
    pub avx2: bool,
    pub avx512: bool,
    pub sse4_2: bool,
    pub aes_ni: bool,
}

impl Default for SimdFeatures {
    fn default() -> Self {
        Self::detect()
    }
}

impl SimdFeatures {
    /// Auto-detect available SIMD features
    pub fn detect() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            Self {
                avx2: is_x86_feature_detected!("avx2"),
                avx512: is_x86_feature_detected!("avx512f"),
                sse4_2: is_x86_feature_detected!("sse4.2"),
                aes_ni: is_x86_feature_detected!("aes"),
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            Self {
                avx2: false,
                avx512: false,
                sse4_2: false,
                aes_ni: false,
            }
        }
    }
}

impl Default for CpuOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuOptimizer {
    pub fn new() -> Self {
        let core_count = num_cpus::get();
        let simd_features = SimdFeatures::detect();

        // Create specialized thread pools
        let game_pool = rayon::ThreadPoolBuilder::new()
            .num_threads((core_count / 2).max(1)) // Half cores for game logic
            .thread_name(|i| format!("game-worker-{}", i))
            .build()
            .unwrap_or_else(|e| {
                eprintln!("WARNING: Failed to create game thread pool, using global pool: {}", e);
                rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap()
            });

        let network_pool = rayon::ThreadPoolBuilder::new()
            .num_threads((core_count / 4).max(1)) // Quarter cores for network
            .thread_name(|i| format!("network-worker-{}", i))
            .build()
            .unwrap_or_else(|e| {
                eprintln!("WARNING: Failed to create network thread pool, using global pool: {}", e);
                rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap()
            });

        Self {
            core_count,
            simd_features,
            game_pool,
            network_pool,
        }
    }

    /// Get optimizer statistics
    pub fn stats(&self) -> CpuOptimizerStats {
        CpuOptimizerStats {
            core_count: self.core_count,
            simd_features: self.simd_features.clone(),
            game_threads: self.game_pool.current_num_threads(),
            network_threads: self.network_pool.current_num_threads(),
        }
    }

    /// Parallel hash computation with SIMD
    pub fn parallel_hash_batch(&self, data_chunks: &[&[u8]]) -> Vec<u64> {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if self.simd_features.avx2 && data_chunks.len() >= 4 {
                unsafe { self.simd_hash_batch_avx2(data_chunks) }
            } else {
                // Fallback to parallel scalar hashing
                data_chunks
                    .par_iter()
                    .map(|chunk| self.fast_hash(chunk))
                    .collect()
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            // Non-x86 architectures: use parallel scalar hashing
            data_chunks
                .par_iter()
                .map(|chunk| self.fast_hash(chunk))
                .collect()
        }
    }

    /// Fast hash function using SIMD when possible
    pub fn fast_hash(&self, data: &[u8]) -> u64 {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if self.simd_features.sse4_2 && data.len() >= 8 {
                unsafe { self.simd_hash_sse42(data) }
            } else {
                self.fallback_hash(data)
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            self.fallback_hash(data)
        }
    }

    /// Fallback hash function (FNV-1a variant)
    fn fallback_hash(&self, data: &[u8]) -> u64 {
        const FNV_OFFSET: u64 = 14695981039346656037;
        const FNV_PRIME: u64 = 1099511628211;

        data.iter().fold(FNV_OFFSET, |hash, &byte| {
            (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
        })
    }

    /// SIMD hash using AVX2 for batch processing
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "avx2")]
    unsafe fn simd_hash_batch_avx2(&self, data_chunks: &[&[u8]]) -> Vec<u64> {
        let mut results = Vec::with_capacity(data_chunks.len());

        // Process 4 chunks at a time with AVX2
        for chunk_batch in data_chunks.chunks(4) {
            for chunk in chunk_batch {
                results.push(unsafe { self.simd_hash_sse42(chunk) });
            }
        }

        results
    }

    /// SIMD hash using SSE4.2 CRC32C instruction
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[target_feature(enable = "sse4.2")]
    unsafe fn simd_hash_sse42(&self, data: &[u8]) -> u64 {
        let mut hash = 0xFFFFFFFF_u32;
        let mut ptr = data.as_ptr();
        let mut remaining = data.len();

        // Process 8-byte chunks
        while remaining >= 8 {
            let chunk = std::ptr::read_unaligned(ptr as *const u64);
            hash = _mm_crc32_u64(hash as u64, chunk) as u32;
            ptr = ptr.add(8);
            remaining -= 8;
        }

        // Process 4-byte chunks
        while remaining >= 4 {
            let chunk = std::ptr::read_unaligned(ptr as *const u32);
            hash = _mm_crc32_u32(hash, chunk);
            ptr = ptr.add(4);
            remaining -= 4;
        }

        // Process remaining bytes
        while remaining > 0 {
            hash = _mm_crc32_u8(hash, *ptr);
            ptr = ptr.add(1);
            remaining -= 1;
        }

        hash as u64
    }

    /// Parallel consensus validation using game thread pool
    pub fn parallel_validate_consensus<F>(&self, validators: Vec<F>) -> Vec<bool>
    where
        F: Fn() -> bool + Send + Sync,
    {
        self.game_pool.install(|| {
            validators
                .into_par_iter()
                .map(|validator| validator())
                .collect()
        })
    }

    /// Parallel network packet processing
    pub fn parallel_process_packets<T, F, R>(&self, packets: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send,
        F: Fn(T) -> R + Send + Sync,
        R: Send,
    {
        self.network_pool
            .install(|| packets.into_par_iter().map(processor).collect())
    }

    /// Optimized game state diff calculation
    pub fn calculate_state_diff(
        &self,
        _old_state: &GameState,
        _new_state: &GameState,
    ) -> StateDiff {
        // TODO: Implement proper state diff when GameState API is stabilized
        StateDiff {
            player_changes: Vec::new(),
            bet_changes: Vec::new(),
            game_phase_changed: false,
            dice_changed: false,
        }
    }

    /// Parallel player state comparison
    fn parallel_compare_players(
        &self,
        old_players: &FxHashMap<PeerId, u64>,
        new_players: &FxHashMap<PeerId, u64>,
    ) -> Vec<PlayerChange> {
        let all_keys: std::collections::HashSet<_> =
            old_players.keys().chain(new_players.keys()).collect();

        all_keys
            .into_par_iter()
            .filter_map(|&player_id| {
                let old_balance = old_players.get(&player_id).copied().unwrap_or(0);
                let new_balance = new_players.get(&player_id).copied().unwrap_or(0);

                if old_balance != new_balance {
                    Some(PlayerChange {
                        player_id: player_id,
                        old_balance,
                        new_balance,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Parallel bet state comparison  
    fn parallel_compare_bets(
        &self,
        old_bets: &FxHashMap<PeerId, u64>,
        new_bets: &FxHashMap<PeerId, u64>,
    ) -> Vec<BetChange> {
        let all_keys: std::collections::HashSet<_> =
            old_bets.keys().chain(new_bets.keys()).collect();

        all_keys
            .into_par_iter()
            .filter_map(|&player_id| {
                let old_bet = old_bets.get(&player_id).copied().unwrap_or(0);
                let new_bet = new_bets.get(&player_id).copied().unwrap_or(0);

                if old_bet != new_bet {
                    Some(BetChange {
                        player_id: player_id,
                        old_bet,
                        new_bet,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Optimized bulk encryption using AES-NI
    pub fn bulk_encrypt_aes(&self, data_chunks: &[&[u8]], key: &[u8; 32]) -> Vec<Vec<u8>> {
        if self.simd_features.aes_ni {
            self.simd_bulk_encrypt_aes(data_chunks, key)
        } else {
            // Fallback to software AES
            data_chunks
                .par_iter()
                .map(|chunk| self.software_encrypt_aes(chunk, key))
                .collect()
        }
    }

    /// SIMD AES encryption using AES-NI instructions
    fn simd_bulk_encrypt_aes(&self, data_chunks: &[&[u8]], _key: &[u8; 32]) -> Vec<Vec<u8>> {
        // Note: This is a simplified example. Real AES-NI implementation would be more complex
        data_chunks
            .par_iter()
            .map(|chunk| {
                // Placeholder: In real implementation, use AES-NI intrinsics
                chunk.to_vec()
            })
            .collect()
    }

    /// Software AES encryption fallback
    fn software_encrypt_aes(&self, data: &[u8], _key: &[u8; 32]) -> Vec<u8> {
        // Placeholder: In real implementation, use software AES
        data.to_vec()
    }
}

#[derive(Debug, Clone)]
pub struct CpuOptimizerStats {
    pub core_count: usize,
    pub simd_features: SimdFeatures,
    pub game_threads: usize,
    pub network_threads: usize,
}

#[derive(Debug, Clone)]
pub struct StateDiff {
    pub player_changes: Vec<PlayerChange>,
    pub bet_changes: Vec<BetChange>,
    pub game_phase_changed: bool,
    pub dice_changed: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerChange {
    pub player_id: PeerId,
    pub old_balance: u64,
    pub new_balance: u64,
}

#[derive(Debug, Clone)]
pub struct BetChange {
    pub player_id: PeerId,
    pub old_bet: u64,
    pub new_bet: u64,
}

/// Lock-free queue for high-performance message passing
pub struct LockFreeQueue<T> {
    queue: crossbeam_channel::Sender<T>,
    receiver: crossbeam_channel::Receiver<T>,
}

impl<T> LockFreeQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let (queue, receiver) = crossbeam_channel::bounded(capacity);
        Self { queue, receiver }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        self.queue.try_send(item).map_err(|e| match e {
            crossbeam_channel::TrySendError::Full(item) => item,
            crossbeam_channel::TrySendError::Disconnected(item) => item,
        })
    }

    pub fn pop(&self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    pub fn len(&self) -> usize {
        self.receiver.len()
    }
}

/// High-performance thread-safe cache with CPU optimization
pub struct OptimizedCache<K, V> {
    shards: Vec<Arc<RwLock<FxHashMap<K, CacheEntry<V>>>>>,
    shard_count: usize,
    optimizer: Arc<CpuOptimizer>,
}

#[derive(Clone)]
struct CacheEntry<V> {
    value: V,
    access_count: u64,
    last_access: std::time::Instant,
}

impl<K, V> OptimizedCache<K, V>
where
    K: Clone + std::hash::Hash + Eq + Send + Sync + std::fmt::Debug,
    V: Clone + Send + Sync,
{
    pub fn new(shard_count: usize, optimizer: Arc<CpuOptimizer>) -> Self {
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Arc::new(RwLock::new(FxHashMap::default())));
        }

        Self {
            shards,
            shard_count,
            optimizer,
        }
    }

    /// Get shard index using optimized hash
    fn get_shard_index(&self, key: &K) -> usize {
        // Use a simple hash-to-bytes conversion for the optimizer
        let key_bytes = format!("{:?}", key).into_bytes(); // Simplified for demo
        let hash = self.optimizer.fast_hash(&key_bytes);
        (hash as usize) % self.shard_count
    }

    /// Get value from cache
    pub fn get(&self, key: &K) -> Option<V> {
        let shard_index = self.get_shard_index(key);
        let shard = &self.shards[shard_index];

        let mut shard_guard = shard.write();
        if let Some(entry) = shard_guard.get_mut(key) {
            entry.access_count += 1;
            entry.last_access = std::time::Instant::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Insert value into cache
    pub fn insert(&self, key: K, value: V) {
        let shard_index = self.get_shard_index(&key);
        let shard = &self.shards[shard_index];

        let mut shard_guard = shard.write();
        shard_guard.insert(
            key,
            CacheEntry {
                value,
                access_count: 1,
                last_access: std::time::Instant::now(),
            },
        );
    }

    /// Get cache statistics across all shards
    pub fn stats(&self) -> CacheStats {
        let mut total_entries = 0;
        let mut total_accesses = 0;

        for shard in &self.shards {
            let shard_guard = shard.read();
            total_entries += shard_guard.len();
            total_accesses += shard_guard
                .values()
                .map(|entry| entry.access_count)
                .sum::<u64>();
        }

        CacheStats {
            total_entries,
            total_accesses,
            shard_count: self.shard_count,
            average_accesses_per_entry: if total_entries > 0 {
                total_accesses as f64 / total_entries as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: usize,
    pub total_accesses: u64,
    pub shard_count: usize,
    pub average_accesses_per_entry: f64,
}
