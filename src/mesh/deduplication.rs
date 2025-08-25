use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use dashmap::DashMap;
use sha2::{Sha256, Digest};
use crate::protocol::BitchatPacket;

/// Message deduplication with sliding window (Lock-free implementation)
/// 
/// Feynman: This is like a bouncer with perfect memory - it remembers
/// every guest (message) who entered recently and won't let duplicates in.
/// After a while, it forgets old guests to save memory.
/// Now with lock-free concurrent access for high throughput!
pub struct MessageDeduplicator {
    // Lock-free concurrent hashmap for seen messages
    seen_messages: Arc<DashMap<[u8; 32], u64>>, // Value is timestamp as u64
    window_duration: Duration,
    max_entries: usize,
    // Atomic counters for statistics
    cache_hits: Arc<AtomicUsize>,
    cache_misses: Arc<AtomicUsize>,
    // Cleanup management
    last_cleanup: Arc<AtomicU64>, // Timestamp of last cleanup
    cleanup_interval: Duration,
}

impl MessageDeduplicator {
    /// Create a new deduplicator
    pub fn new(window_duration: Duration) -> Self {
        let now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_millis() as u64;
            
        Self {
            seen_messages: Arc::new(DashMap::new()),
            window_duration,
            max_entries: 100000, // Maximum entries to prevent memory exhaustion
            cache_hits: Arc::new(AtomicUsize::new(0)),
            cache_misses: Arc::new(AtomicUsize::new(0)),
            last_cleanup: Arc::new(AtomicU64::new(now_timestamp)),
            cleanup_interval: Duration::from_secs(30), // Cleanup every 30 seconds
        }
    }
    
    /// Check if a message is a duplicate (lock-free implementation)
    pub async fn is_duplicate(&self, packet: &BitchatPacket) -> bool {
        let hash = self.compute_packet_hash(packet);
        let now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_millis() as u64;
        
        // Trigger cleanup if interval has passed (non-blocking)
        self.maybe_trigger_cleanup(now_timestamp).await;
        
        // Check if we've seen this message (lock-free read)
        if let Some(entry) = self.seen_messages.get(&hash) {
            let timestamp = *entry;
            let age_ms = now_timestamp.saturating_sub(timestamp);
            
            // Check if entry is still valid (within window)
            if age_ms <= self.window_duration.as_millis() as u64 {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return true; // Valid duplicate found
            }
            // Entry is expired, we'll remove it and treat as new
            drop(entry);
            self.seen_messages.remove(&hash);
        }
        
        // Not a duplicate - add to cache (lock-free write)
        self.seen_messages.insert(hash, now_timestamp);
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        
        // Enforce max entries limit (probabilistic cleanup)
        if self.seen_messages.len() > self.max_entries {
            self.emergency_cleanup().await;
        }
        
        false // Not a duplicate
    }
    
    /// Compute hash of a packet for deduplication
    fn compute_packet_hash(&self, packet: &BitchatPacket) -> [u8; 32] {
        let mut hasher = Sha256::new();
        
        // Hash relevant packet fields (exclude TTL and timestamp)
        hasher.update([packet.version]);
        hasher.update([packet.packet_type]);
        hasher.update([packet.flags]);
        hasher.update(packet.sequence.to_le_bytes());
        hasher.update(packet.total_length.to_le_bytes());
        
        // Hash TLV data for uniqueness
        for tlv in &packet.tlv_data {
            hasher.update([tlv.field_type]);
            hasher.update(tlv.length.to_le_bytes());
            hasher.update(&tlv.value);
        }
        
        // Hash payload if present
        if let Some(ref payload) = packet.payload {
            hasher.update(payload);
        }
        
        hasher.finalize().into()
    }
    
    /// Maybe trigger cleanup if interval has passed (non-blocking)
    async fn maybe_trigger_cleanup(&self, now_timestamp: u64) {
        let last_cleanup = self.last_cleanup.load(Ordering::Relaxed);
        let cleanup_interval_ms = self.cleanup_interval.as_millis() as u64;
        
        if now_timestamp.saturating_sub(last_cleanup) >= cleanup_interval_ms {
            // Try to acquire cleanup responsibility atomically
            if self.last_cleanup.compare_exchange_weak(
                last_cleanup,
                now_timestamp,
                Ordering::Relaxed,
                Ordering::Relaxed
            ).is_ok() {
                // We won the race - perform cleanup in background
                let seen_messages = Arc::clone(&self.seen_messages);
                let window_duration = self.window_duration;
                
                tokio::spawn(async move {
                    Self::cleanup_expired_entries(seen_messages, window_duration, now_timestamp).await;
                });
            }
        }
    }
    
    /// Clean up expired entries (background task)
    async fn cleanup_expired_entries(
        seen_messages: Arc<DashMap<[u8; 32], u64>>,
        window_duration: Duration,
        now_timestamp: u64
    ) {
        let window_ms = window_duration.as_millis() as u64;
        let mut expired_keys = Vec::new();
        
        // Collect expired keys
        for entry in seen_messages.iter() {
            let age_ms = now_timestamp.saturating_sub(*entry.value());
            if age_ms > window_ms {
                expired_keys.push(*entry.key());
            }
        }
        
        // Remove expired entries
        for key in expired_keys {
            seen_messages.remove(&key);
        }
    }
    
    /// Emergency cleanup when max entries exceeded
    async fn emergency_cleanup(&self) {
        let _now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_millis() as u64;
            
        // Remove oldest 25% of entries to free up space
        let target_size = (self.max_entries * 3) / 4;
        let mut entries_to_remove = Vec::new();
        
        // Collect entries with timestamps for sorting
        let mut timestamped_entries: Vec<([u8; 32], u64)> = Vec::new();
        for entry in self.seen_messages.iter() {
            timestamped_entries.push((*entry.key(), *entry.value()));
        }
        
        if timestamped_entries.len() > target_size {
            // Sort by timestamp (oldest first)
            timestamped_entries.sort_by_key(|(_, timestamp)| *timestamp);
            
            // Mark oldest entries for removal
            let remove_count = timestamped_entries.len() - target_size;
            for (hash, _) in timestamped_entries.into_iter().take(remove_count) {
                entries_to_remove.push(hash);
            }
            
            // Remove the entries
            for hash in entries_to_remove {
                self.seen_messages.remove(&hash);
            }
        }
    }
    
    /// Get current cache size (lock-free)
    pub async fn cache_size(&self) -> usize {
        self.seen_messages.len()
    }
    
    /// Clear all cached messages (lock-free)
    pub async fn clear(&self) {
        self.seen_messages.clear();
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        let now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0)) // Fallback for clock issues
            .as_millis() as u64;
        self.last_cleanup.store(now_timestamp, Ordering::Relaxed);
    }
    
    /// Get statistics about deduplication (lock-free)
    pub async fn get_stats(&self) -> DeduplicationStats {
        DeduplicationStats {
            cache_size: self.seen_messages.len(),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            window_duration: self.window_duration,
            max_entries: self.max_entries,
            cleanup_interval: self.cleanup_interval,
        }
    }
}

/// Statistics about the deduplicator
#[derive(Debug, Clone)]
pub struct DeduplicationStats {
    pub cache_size: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub window_duration: Duration,
    pub max_entries: usize,
    pub cleanup_interval: Duration,
}

/// Advanced deduplication with Bloom filters for efficiency
/// 
/// Feynman: Bloom filters are like a "maybe" detector - they can
/// definitely tell you "no, haven't seen it" but only "maybe" for "yes".
/// Perfect for quick filtering before expensive checks.
pub struct BloomDeduplicator {
    bloom_filter: Arc<RwLock<BloomFilter>>,
    exact_cache: Arc<MessageDeduplicator>,
}

/// Simple Bloom filter implementation
struct BloomFilter {
    bits: Vec<bool>,
    hash_count: usize,
    size: usize,
}

impl BloomFilter {
    fn new(size: usize, hash_count: usize) -> Self {
        Self {
            bits: vec![false; size],
            hash_count,
            size,
        }
    }
    
    fn add(&mut self, data: &[u8]) {
        for i in 0..self.hash_count {
            let hash = self.hash_with_seed(data, i as u32);
            let index = (hash as usize) % self.size;
            self.bits[index] = true;
        }
    }
    
    fn possibly_contains(&self, data: &[u8]) -> bool {
        for i in 0..self.hash_count {
            let hash = self.hash_with_seed(data, i as u32);
            let index = (hash as usize) % self.size;
            if !self.bits[index] {
                return false; // Definitely not in set
            }
        }
        true // Possibly in set
    }
    
    fn hash_with_seed(&self, data: &[u8], seed: u32) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(seed.to_le_bytes());
        hasher.update(data);
        let result = hasher.finalize();
        u64::from_le_bytes(result[0..8].try_into().unwrap_or([0u8; 8]))
    }
    
    fn clear(&mut self) {
        self.bits.fill(false);
    }
}

impl BloomDeduplicator {
    /// Create a new Bloom filter deduplicator
    pub fn new(bloom_size: usize, hash_count: usize, window_duration: Duration) -> Self {
        Self {
            bloom_filter: Arc::new(RwLock::new(BloomFilter::new(bloom_size, hash_count))),
            exact_cache: Arc::new(MessageDeduplicator::new(window_duration)),
        }
    }
    
    /// Check if a packet is a duplicate using Bloom filter first
    pub async fn is_duplicate(&self, packet: &BitchatPacket) -> bool {
        let hash = self.compute_packet_hash(packet);
        
        // Quick check with Bloom filter
        let bloom = self.bloom_filter.read().await;
        if !bloom.possibly_contains(&hash) {
            // Definitely not a duplicate
            drop(bloom);
            
            // Add to Bloom filter
            self.bloom_filter.write().await.add(&hash);
            
            // Add to exact cache
            let _ = self.exact_cache.is_duplicate(packet).await;
            
            return false;
        }
        
        // Might be duplicate, check exact cache
        self.exact_cache.is_duplicate(packet).await
    }
    
    fn compute_packet_hash(&self, packet: &BitchatPacket) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update([packet.version]);
        hasher.update([packet.packet_type]);
        hasher.update([packet.flags]);
        hasher.update(packet.sequence.to_le_bytes());
        hasher.update(packet.total_length.to_le_bytes());
        
        // Hash TLV data for uniqueness
        for tlv in &packet.tlv_data {
            hasher.update([tlv.field_type]);
            hasher.update(tlv.length.to_le_bytes());
            hasher.update(&tlv.value);
        }
        
        if let Some(ref payload) = packet.payload {
            hasher.update(payload);
        }
        hasher.finalize().into()
    }
    
    /// Clear both Bloom filter and exact cache
    pub async fn clear(&self) {
        self.bloom_filter.write().await.clear();
        self.exact_cache.clear().await;
    }
}

// Include tests inline to avoid import issues
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Create a minimal test packet
    fn create_test_packet(sequence: u32) -> BitchatPacket {
        BitchatPacket {
            version: 1,
            packet_type: 1,
            flags: 0,
            ttl: 10,
            total_length: 0,
            checksum: 0,
            tlv_data: vec![],
            source: [1u8; 32],
            target: [2u8; 32],
            sequence: sequence.into(),
            payload: Some(vec![1, 2, 3, 4]),
        }
    }

    #[tokio::test]
    async fn test_lock_free_deduplication() {
        let deduplicator = MessageDeduplicator::new(Duration::from_secs(5));
        let packet = create_test_packet(1);

        // First time should not be duplicate
        assert!(!deduplicator.is_duplicate(&packet).await);
        
        // Second time should be duplicate
        assert!(deduplicator.is_duplicate(&packet).await);
        
        // Different packet should not be duplicate
        let packet2 = create_test_packet(2);
        assert!(!deduplicator.is_duplicate(&packet2).await);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let deduplicator = Arc::new(MessageDeduplicator::new(Duration::from_secs(5)));
        let mut handles = vec![];

        // Spawn multiple concurrent tasks
        for i in 0..10 {
            let dedup = Arc::clone(&deduplicator);
            let handle = tokio::spawn(async move {
                let packet = create_test_packet(i);
                // Each task should see its packet as new
                assert!(!dedup.is_duplicate(&packet).await);
                // And duplicate on second check
                assert!(dedup.is_duplicate(&packet).await);
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Cache should have 10 entries
        assert_eq!(deduplicator.cache_size().await, 10);
    }

    #[tokio::test]
    async fn test_expiration() {
        let deduplicator = MessageDeduplicator::new(Duration::from_millis(100));
        let packet = create_test_packet(1);

        // Add packet
        assert!(!deduplicator.is_duplicate(&packet).await);
        assert!(deduplicator.is_duplicate(&packet).await);

        // Wait for expiration
        sleep(Duration::from_millis(150)).await;

        // Should not be duplicate anymore (expired)
        assert!(!deduplicator.is_duplicate(&packet).await);
    }

    #[tokio::test]
    async fn test_stats() {
        let deduplicator = MessageDeduplicator::new(Duration::from_secs(5));
        let packet = create_test_packet(1);

        // Initial stats
        let stats = deduplicator.get_stats().await;
        assert_eq!(stats.cache_size, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);

        // Add a packet (cache miss)
        assert!(!deduplicator.is_duplicate(&packet).await);
        let stats = deduplicator.get_stats().await;
        assert_eq!(stats.cache_size, 1);
        assert_eq!(stats.cache_misses, 1);

        // Check duplicate (cache hit)
        assert!(deduplicator.is_duplicate(&packet).await);
        let stats = deduplicator.get_stats().await;
        assert_eq!(stats.cache_hits, 1);
    }
}