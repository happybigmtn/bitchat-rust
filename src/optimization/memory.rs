use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use bytes::{Bytes, BytesMut};
use bitvec::prelude::*;
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use crate::protocol::PeerId;

/// Enhanced message pool with better memory management and statistics
pub struct MessagePool {
    small_pool: VecDeque<BytesMut>,
    medium_pool: VecDeque<BytesMut>,
    large_pool: VecDeque<BytesMut>,
    // Statistics
    allocations: u64,
    deallocations: u64,
    peak_usage: (usize, usize, usize), // (small, medium, large)
}

impl MessagePool {
    pub fn new() -> Self {
        Self {
            small_pool: VecDeque::with_capacity(100),
            medium_pool: VecDeque::with_capacity(50),
            large_pool: VecDeque::with_capacity(10),
            allocations: 0,
            deallocations: 0,
            peak_usage: (0, 0, 0),
        }
    }
    
    pub fn get_buffer(&mut self, size: usize) -> BytesMut {
        self.allocations += 1;
        
        match size {
            0..=1024 => {
                self.small_pool.pop_front()
                    .unwrap_or_else(|| BytesMut::with_capacity(1024))
            },
            1025..=8192 => {
                self.medium_pool.pop_front()
                    .unwrap_or_else(|| BytesMut::with_capacity(8192))
            },
            _ => {
                self.large_pool.pop_front()
                    .unwrap_or_else(|| BytesMut::with_capacity(size.next_power_of_two()))
            }
        }
    }
    
    pub fn return_buffer(&mut self, mut buffer: BytesMut) {
        self.deallocations += 1;
        buffer.clear();
        
        match buffer.capacity() {
            0..=1024 if self.small_pool.len() < 100 => {
                self.small_pool.push_back(buffer);
                self.peak_usage.0 = self.peak_usage.0.max(self.small_pool.len());
            }
            1025..=8192 if self.medium_pool.len() < 50 => {
                self.medium_pool.push_back(buffer);
                self.peak_usage.1 = self.peak_usage.1.max(self.medium_pool.len());
            }
            _ if self.large_pool.len() < 10 => {
                self.large_pool.push_back(buffer);
                self.peak_usage.2 = self.peak_usage.2.max(self.large_pool.len());
            }
            _ => {} // Drop oversized or excess buffers
        }
    }
    
    /// Get pool statistics for monitoring
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            small_available: self.small_pool.len(),
            medium_available: self.medium_pool.len(),
            large_available: self.large_pool.len(),
            total_allocations: self.allocations,
            total_deallocations: self.deallocations,
            peak_usage: self.peak_usage,
            pool_efficiency: if self.allocations > 0 {
                (self.deallocations as f32 / self.allocations as f32) * 100.0
            } else {
                0.0
            },
        }
    }
    
    /// Trim pools to reduce memory usage
    pub fn trim(&mut self) {
        // Keep only half of current capacity to free memory
        let small_keep = self.small_pool.len() / 2;
        let medium_keep = self.medium_pool.len() / 2;
        let large_keep = self.large_pool.len() / 2;
        
        self.small_pool.truncate(small_keep);
        self.medium_pool.truncate(medium_keep);
        self.large_pool.truncate(large_keep);
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub small_available: usize,
    pub medium_available: usize,
    pub large_available: usize,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub peak_usage: (usize, usize, usize),
    pub pool_efficiency: f32, // Percentage of buffers returned to pool
}

// Enhanced message header with integrity and routing info
#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub msg_type: u8,
    pub length: u32,
    pub timestamp: u64,
    pub sequence: u32,    // For ordering and deduplication
    pub priority: u8,     // Message priority (0=low, 255=high)
    pub flags: u8,        // Feature flags (compression, encryption, etc.)
}

/// Circular buffer for efficient history management with automatic cleanup
/// Feynman: Like a revolving door - old entries automatically fall out as new ones enter
pub struct CircularBuffer<T> {
    buffer: VecDeque<T>,
    max_size: usize,
    total_items_added: u64,
}

impl<T> CircularBuffer<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            total_items_added: 0,
        }
    }
    
    /// Add item, automatically evicting oldest if full
    pub fn push(&mut self, item: T) -> Option<T> {
        self.total_items_added += 1;
        
        if self.buffer.len() >= self.max_size {
            let evicted = self.buffer.pop_front();
            self.buffer.push_back(item);
            evicted
        } else {
            self.buffer.push_back(item);
            None
        }
    }
    
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
    
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.max_size
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter()
    }
    
    pub fn total_processed(&self) -> u64 {
        self.total_items_added
    }
}

/// Efficient vote tracking with bit vectors
/// Feynman: Instead of storing each vote separately, we pack them as bits
/// This uses 64x less memory and enables super-fast counting
pub struct VoteTracker {
    /// Bit vector where index = peer_index, bit = vote (0=no, 1=yes)
    votes: BitVec,
    /// Map peer IDs to indices for O(1) lookup
    peer_indices: HashMap<PeerId, usize>,
    /// Total number of registered peers
    total_peers: usize,
    /// Current vote counts (cached for performance)
    yes_votes: usize,
    no_votes: usize,
}

impl VoteTracker {
    pub fn new() -> Self {
        Self {
            votes: BitVec::new(),
            peer_indices: HashMap::new(),
            total_peers: 0,
            yes_votes: 0,
            no_votes: 0,
        }
    }
    
    /// Register a new peer for voting (must be done before voting)
    pub fn register_peer(&mut self, peer_id: PeerId) -> bool {
        if self.peer_indices.contains_key(&peer_id) {
            return false; // Already registered
        }
        
        let index = self.total_peers;
        self.peer_indices.insert(peer_id, index);
        self.votes.push(false); // Default to "no" vote
        self.total_peers += 1;
        self.no_votes += 1;
        
        true
    }
    
    /// Cast a vote (true = yes, false = no)
    pub fn cast_vote(&mut self, peer_id: &PeerId, vote: bool) -> bool {
        if let Some(&index) = self.peer_indices.get(peer_id) {
            let old_vote = self.votes[index];
            
            if old_vote != vote {
                // Update the bit
                self.votes.set(index, vote);
                
                // Update cached counts
                if vote {
                    self.yes_votes += 1;
                    self.no_votes -= 1;
                } else {
                    self.yes_votes -= 1;
                    self.no_votes += 1;
                }
            }
            
            true
        } else {
            false // Peer not registered
        }
    }
    
    /// Get current vote counts in O(1) time
    pub fn get_counts(&self) -> (usize, usize) {
        (self.yes_votes, self.no_votes)
    }
    
    /// Check if majority vote is reached
    pub fn has_majority(&self) -> Option<bool> {
        let threshold = self.total_peers / 2 + 1;
        
        if self.yes_votes >= threshold {
            Some(true)
        } else if self.no_votes >= threshold {
            Some(false)
        } else {
            None // No majority yet
        }
    }
    
    /// Get participation rate
    pub fn participation_rate(&self) -> f32 {
        if self.total_peers == 0 {
            0.0
        } else {
            (self.yes_votes + self.no_votes) as f32 / self.total_peers as f32
        }
    }
}

/// Memory-mapped storage for large data with compression
/// Feynman: Instead of keeping everything in RAM, we use the OS virtual memory
/// system to efficiently handle large datasets that don't fit in memory
#[allow(dead_code)]
pub struct MmapStorage {
    file_path: String,
    mmap: Option<MmapMut>,
    capacity: usize,
    used: usize,
    compress_threshold: usize,
}

impl MmapStorage {
    pub fn new(file_path: String, capacity_mb: usize) -> Result<Self, std::io::Error> {
        // Validate capacity to prevent excessive allocations
        const MAX_CAPACITY_MB: usize = 1024; // 1GB max
        const MIN_CAPACITY_MB: usize = 1; // 1MB min
        
        if capacity_mb > MAX_CAPACITY_MB {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Capacity {}MB exceeds maximum {}MB", capacity_mb, MAX_CAPACITY_MB)
            ));
        }
        
        if capacity_mb < MIN_CAPACITY_MB {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Capacity {}MB below minimum {}MB", capacity_mb, MIN_CAPACITY_MB)
            ));
        }
        
        let capacity = capacity_mb
            .checked_mul(1024)
            .and_then(|kb| kb.checked_mul(1024))
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Capacity overflow"
            ))?;
        
        // Validate file path
        if file_path.is_empty() || file_path.contains('\0') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid file path"
            ));
        }
        
        // Create or open file with proper permissions
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600) // Restrict to owner only
            .open(&file_path)?;
        
        // Set file size with validation
        file.set_len(capacity as u64)?;
        
        // Verify file size was set correctly
        let metadata = file.metadata()?;
        if metadata.len() != capacity as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to set file size correctly"
            ));
        }
        
        // Create memory map with safety checks
        let mmap = unsafe {
            // SAFETY: We've validated the file exists, has the correct size,
            // and we have exclusive access through the file handle
            MmapOptions::new()
                .len(capacity)
                .map_mut(&file)?
        };
        
        Ok(Self {
            file_path,
            mmap: Some(mmap),
            capacity,
            used: 0,
            compress_threshold: 1024, // Compress data larger than 1KB
        })
    }
    
    /// Store data with automatic compression for large payloads
    pub fn store(&mut self, key: &[u8], data: &[u8]) -> Result<bool, std::io::Error> {
        if self.mmap.is_none() {
            return Ok(false);
        }
        
        // Validate key and data sizes
        const MAX_KEY_SIZE: usize = 1024; // 1KB max key
        const MAX_DATA_SIZE: usize = 10 * 1024 * 1024; // 10MB max data
        
        if key.is_empty() || key.len() > MAX_KEY_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid key size: {}", key.len())
            ));
        }
        
        if data.len() > MAX_DATA_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Data size {} exceeds maximum {}MB", data.len(), MAX_DATA_SIZE / 1024 / 1024)
            ));
        }
        
        // Determine if we should compress
        let (final_data, compressed) = if data.len() > self.compress_threshold {
            (compress_prepend_size(data), true)
        } else {
            (data.to_vec(), false)
        };
        
        // Calculate required space with overflow checking
        let required_space = 4_usize
            .checked_add(key.len())
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_add(final_data.len()))
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Integer overflow in space calculation"
            ))?;
        
        // Check available space with overflow protection
        let new_used = self.used.checked_add(required_space)
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Integer overflow in used space"
            ))?;
        
        if new_used > self.capacity {
            return Ok(false); // Not enough space
        }
        
        let mmap = match self.mmap.as_mut() {
            Some(mmap) => mmap,
            None => return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Memory map not initialized"
            )),
        };
        let mut offset = self.used;
        
        // Bounds check before each write
        if offset + 4 > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Buffer overflow prevented in key length write"
            ));
        }
        
        // Write key length
        let key_len = key.len() as u32;
        mmap[offset..offset + 4].copy_from_slice(&key_len.to_le_bytes());
        offset += 4;
        
        // Bounds check for key
        if offset + key.len() > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Buffer overflow prevented in key write"
            ));
        }
        
        // Write key
        mmap[offset..offset + key.len()].copy_from_slice(key);
        offset += key.len();
        
        // Bounds check for compression flag
        if offset + 1 > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Buffer overflow prevented in flag write"
            ));
        }
        
        // Write compression flag
        mmap[offset] = if compressed { 1 } else { 0 };
        offset += 1;
        
        // Bounds check for data length
        if offset + 4 > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Buffer overflow prevented in data length write"
            ));
        }
        
        // Write data length
        let data_len = final_data.len() as u32;
        mmap[offset..offset + 4].copy_from_slice(&data_len.to_le_bytes());
        offset += 4;
        
        // Final bounds check for data
        if offset + final_data.len() > self.capacity {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Buffer overflow prevented in data write"
            ));
        }
        
        // Write data
        mmap[offset..offset + final_data.len()].copy_from_slice(&final_data);
        
        self.used = new_used;
        Ok(true)
    }
    
    /// Retrieve and decompress data
    pub fn retrieve(&self, target_key: &[u8]) -> Result<Option<Vec<u8>>, std::io::Error> {
        if self.mmap.is_none() || self.used == 0 {
            return Ok(None);
        }
        
        let mmap = match self.mmap.as_ref() {
            Some(mmap) => mmap,
            None => return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Memory map not initialized"
            )),
        };
        let mut offset = 0;
        
        while offset < self.used {
            // Read key length
            if offset + 4 > self.used {
                break;
            }
            let key_len = u32::from_le_bytes([
                mmap[offset], mmap[offset + 1], mmap[offset + 2], mmap[offset + 3]
            ]) as usize;
            offset += 4;
            
            // Read key
            if offset + key_len > self.used {
                break;
            }
            let key = &mmap[offset..offset + key_len];
            offset += key_len;
            
            // Read compression flag
            if offset + 1 > self.used {
                break;
            }
            let compressed = mmap[offset] == 1;
            offset += 1;
            
            // Read data length
            if offset + 4 > self.used {
                break;
            }
            let data_len = u32::from_le_bytes([
                mmap[offset], mmap[offset + 1], mmap[offset + 2], mmap[offset + 3]
            ]) as usize;
            offset += 4;
            
            // Read data
            if offset + data_len > self.used {
                break;
            }
            let data = &mmap[offset..offset + data_len];
            
            // Check if this is the key we're looking for
            if key == target_key {
                let result = if compressed {
                    decompress_size_prepended(data)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
                } else {
                    data.to_vec()
                };
                return Ok(Some(result));
            }
            
            offset += data_len;
        }
        
        Ok(None)
    }
    
    /// Get storage statistics
    pub fn stats(&self) -> (usize, usize, f32) {
        let usage_percent = if self.capacity > 0 {
            (self.used as f32 / self.capacity as f32) * 100.0
        } else {
            0.0
        };
        (self.used, self.capacity, usage_percent)
    }
    
    /// Force sync to disk
    pub fn sync(&self) -> Result<(), std::io::Error> {
        if let Some(ref mmap) = self.mmap {
            mmap.flush()?;
        }
        Ok(())
    }
}

/// Automatic garbage collector for expired data
/// Feynman: Like a janitor that automatically cleans up old data
/// based on time-to-live (TTL) and usage patterns
pub struct AutoGarbageCollector<K, V> {
    data: HashMap<K, (V, Instant, Duration)>, // (value, created_at, ttl)
    access_times: HashMap<K, Instant>,
    cleanup_interval: Duration,
    last_cleanup: Instant,
    max_items: usize,
}

impl<K: Clone + Eq + std::hash::Hash, V> AutoGarbageCollector<K, V> {
    pub fn new(max_items: usize, cleanup_interval: Duration) -> Self {
        Self {
            data: HashMap::new(),
            access_times: HashMap::new(),
            cleanup_interval,
            last_cleanup: Instant::now(),
            max_items,
        }
    }
    
    /// Insert item with TTL
    pub fn insert(&mut self, key: K, value: V, ttl: Duration) {
        let now = Instant::now();
        
        // Trigger cleanup if needed
        self.maybe_cleanup(now);
        
        // Insert new item
        self.data.insert(key.clone(), (value, now, ttl));
        self.access_times.insert(key, now);
        
        // Evict oldest if over capacity
        if self.data.len() > self.max_items {
            self.evict_oldest();
        }
    }
    
    /// Get item and update access time
    pub fn get(&mut self, key: &K) -> Option<V> 
    where 
        V: Clone,
    {
        let now = Instant::now();
        
        // Check if item exists and hasn't expired
        if let Some((value, created_at, ttl)) = self.data.get(key) {
            if now.duration_since(*created_at) < *ttl {
                // Update access time
                self.access_times.insert(key.clone(), now);
                return Some(value.clone());
            } else {
                // Item expired, remove it
                let k = key.clone();
                self.data.remove(&k);
                self.access_times.remove(&k);
                return None;
            }
        }
        
        None
    }
    
    /// Get item without updating access time (read-only)
    pub fn peek(&self, key: &K) -> Option<&V> {
        let now = Instant::now();
        
        if let Some((value, created_at, ttl)) = self.data.get(key) {
            if now.duration_since(*created_at) < *ttl {
                Some(value)
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Check if key exists and is not expired
    pub fn contains_key(&self, key: &K) -> bool {
        if let Some((_, created_at, ttl)) = self.data.get(key) {
            Instant::now().duration_since(*created_at) < *ttl
        } else {
            false
        }
    }
    
    /// Remove expired items
    fn maybe_cleanup(&mut self, now: Instant) {
        if now.duration_since(self.last_cleanup) >= self.cleanup_interval {
            self.cleanup_expired(now);
            self.last_cleanup = now;
        }
    }
    
    /// Remove all expired items
    fn cleanup_expired(&mut self, now: Instant) {
        let expired_keys: Vec<K> = self.data
            .iter()
            .filter(|(_, (_, created_at, ttl))| now.duration_since(*created_at) >= *ttl)
            .map(|(k, _)| k.clone())
            .collect();
        
        for key in expired_keys {
            self.data.remove(&key);
            self.access_times.remove(&key);
        }
    }
    
    /// Evict the least recently accessed item
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.access_times
            .iter()
            .min_by_key(|(_, &access_time)| access_time)
            .map(|(k, _)| k.clone()) {
            self.data.remove(&oldest_key);
            self.access_times.remove(&oldest_key);
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> (usize, usize, f32) {
        let usage_percent = (self.data.len() as f32 / self.max_items as f32) * 100.0;
        (self.data.len(), self.max_items, usage_percent)
    }
    
    /// Force cleanup now
    pub fn force_cleanup(&mut self) {
        let now = Instant::now();
        self.cleanup_expired(now);
        self.last_cleanup = now;
    }
}

// Zero-copy message passing with improved chunking
pub struct ZeroCopyMessage {
    pub header: MessageHeader,
    pub payload: Bytes,
    pub checksum: u32, // For integrity verification
}

impl ZeroCopyMessage {
    pub fn new(header: MessageHeader, payload: Bytes) -> Self {
        let checksum = crc32fast::hash(&payload);
        Self { header, payload, checksum }
    }
    
    /// Verify message integrity
    pub fn verify(&self) -> bool {
        crc32fast::hash(&self.payload) == self.checksum
    }
    
    /// Split payload into optimal chunks for network transmission
    pub fn split_payload(&self, chunk_size: usize) -> Vec<Bytes> {
        if self.payload.len() <= chunk_size {
            return vec![self.payload.clone()];
        }
        
        let mut chunks = Vec::with_capacity((self.payload.len() + chunk_size - 1) / chunk_size);
        let mut offset = 0;
        
        while offset < self.payload.len() {
            let end = std::cmp::min(offset + chunk_size, self.payload.len());
            chunks.push(self.payload.slice(offset..end));
            offset = end;
        }
        
        chunks
    }
    
    /// Create compressed version for large messages
    pub fn compressed(&self) -> Result<Self, std::io::Error> {
        let compressed_payload = compress_prepend_size(&self.payload);
        let compressed_bytes = Bytes::from(compressed_payload);
        
        let mut compressed_header = self.header.clone();
        compressed_header.length = compressed_bytes.len() as u32;
        
        Ok(ZeroCopyMessage::new(compressed_header, compressed_bytes))
    }
}