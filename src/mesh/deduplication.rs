use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use sha2::{Sha256, Digest};
use crate::protocol::BitchatPacket;

/// Message deduplication with sliding window
/// 
/// Feynman: This is like a bouncer with perfect memory - it remembers
/// every guest (message) who entered recently and won't let duplicates in.
/// After a while, it forgets old guests to save memory.
pub struct MessageDeduplicator {
    seen_messages: Arc<RwLock<HashMap<[u8; 32], Instant>>>,
    message_order: Arc<RwLock<VecDeque<([u8; 32], Instant)>>>,
    window_duration: Duration,
    max_entries: usize,
}

impl MessageDeduplicator {
    /// Create a new deduplicator
    pub fn new(window_duration: Duration) -> Self {
        Self {
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            message_order: Arc::new(RwLock::new(VecDeque::new())),
            window_duration,
            max_entries: 100000, // Maximum entries to prevent memory exhaustion
        }
    }
    
    /// Check if a message is a duplicate
    pub async fn is_duplicate(&self, packet: &BitchatPacket) -> bool {
        let hash = self.compute_packet_hash(packet);
        let now = Instant::now();
        
        // Clean up old entries
        self.cleanup_expired(now).await;
        
        // Check if we've seen this message
        let mut seen = self.seen_messages.write().await;
        if seen.contains_key(&hash) {
            return true; // Duplicate found
        }
        
        // Add to seen messages
        seen.insert(hash, now);
        
        // Add to order queue
        let mut order = self.message_order.write().await;
        order.push_back((hash, now));
        
        // Enforce max entries limit
        if order.len() > self.max_entries {
            if let Some((old_hash, _)) = order.pop_front() {
                seen.remove(&old_hash);
            }
        }
        
        false // Not a duplicate
    }
    
    /// Compute hash of a packet for deduplication
    fn compute_packet_hash(&self, packet: &BitchatPacket) -> [u8; 32] {
        let mut hasher = Sha256::new();
        
        // Hash relevant packet fields (exclude TTL and timestamp)
        hasher.update(&packet.source);
        hasher.update(&packet.target);
        hasher.update(&[packet.packet_type as u8]);
        hasher.update(&packet.sequence.to_le_bytes());
        
        // Hash payload if present
        if let Some(ref payload) = packet.payload {
            hasher.update(payload);
        }
        
        hasher.finalize().into()
    }
    
    /// Clean up expired entries
    async fn cleanup_expired(&self, now: Instant) {
        let mut seen = self.seen_messages.write().await;
        let mut order = self.message_order.write().await;
        
        // Remove expired entries from front of queue
        while let Some(&(hash, timestamp)) = order.front() {
            if now.duration_since(timestamp) > self.window_duration {
                order.pop_front();
                seen.remove(&hash);
            } else {
                break; // Rest are still valid
            }
        }
    }
    
    /// Get current cache size
    pub async fn cache_size(&self) -> usize {
        self.seen_messages.read().await.len()
    }
    
    /// Clear all cached messages
    pub async fn clear(&self) {
        self.seen_messages.write().await.clear();
        self.message_order.write().await.clear();
    }
    
    /// Get statistics about deduplication
    pub async fn get_stats(&self) -> DeduplicationStats {
        DeduplicationStats {
            cache_size: self.seen_messages.read().await.len(),
            queue_size: self.message_order.read().await.len(),
            window_duration: self.window_duration,
            max_entries: self.max_entries,
        }
    }
}

/// Statistics about the deduplicator
#[derive(Debug, Clone)]
pub struct DeduplicationStats {
    pub cache_size: usize,
    pub queue_size: usize,
    pub window_duration: Duration,
    pub max_entries: usize,
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
        hasher.update(&seed.to_le_bytes());
        hasher.update(data);
        let result = hasher.finalize();
        u64::from_le_bytes(result[0..8].try_into().unwrap())
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
        hasher.update(&packet.source);
        hasher.update(&packet.target);
        hasher.update(&[packet.packet_type as u8]);
        hasher.update(&packet.sequence.to_le_bytes());
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