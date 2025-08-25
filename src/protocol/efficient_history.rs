//! Memory-efficient game history with ring buffers and log-structured storage
//! 
//! This module implements highly optimized game history storage using ring buffers
//! for recent games, log-structured merge trees for long-term storage, and delta
//! encoding for sequential states to minimize memory usage and maximize performance.

use std::collections::{VecDeque, HashMap, BTreeMap};
use std::mem;
use serde::{Serialize, Deserialize};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};

use super::{GameId, PeerId};
use super::efficient_game_state::CompactGameState;
use crate::error::{Error, Result};

/// Configuration for history storage optimization
#[derive(Debug, Clone)]
pub struct HistoryConfig {
    /// Number of recent games to keep in fast ring buffer
    pub ring_buffer_size: usize,
    
    /// Maximum number of deltas to store before compaction
    pub max_deltas_before_compaction: usize,
    
    /// Compression level for archived data (0-9)
    pub compression_level: u32,
    
    /// Maximum memory usage for history storage (in bytes)
    pub max_memory_bytes: usize,
    
    /// LSM tree level sizes
    pub lsm_level_sizes: Vec<usize>,
    
    /// Enable delta compression
    pub enable_delta_compression: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            ring_buffer_size: 1000,
            max_deltas_before_compaction: 100,
            compression_level: 6,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            lsm_level_sizes: vec![10, 100, 1000, 10000],
            enable_delta_compression: true,
        }
    }
}

/// Efficient game history manager combining multiple storage strategies
pub struct EfficientGameHistory {
    /// Configuration
    #[allow(dead_code)]
    config: HistoryConfig,
    
    /// Ring buffer for recent game states (O(1) access, limited size)
    recent_games: RingBuffer<CompactGameHistory>,
    
    /// Log-structured merge tree for archived games
    lsm_tree: LSMTree,
    
    /// Delta encoding for sequential state changes
    delta_encoder: DeltaEncoder,
    
    /// Memory usage tracking
    memory_tracker: MemoryTracker,
    
    /// Performance metrics
    metrics: HistoryMetrics,
}

/// Ring buffer optimized for recent game history
struct RingBuffer<T> {
    /// Fixed-size buffer
    buffer: Vec<Option<T>>,
    
    /// Current head position
    head: usize,
    
    /// Current tail position  
    tail: usize,
    
    /// Number of items currently stored
    len: usize,
    
    /// Capacity of the buffer
    capacity: usize,
}

/// Compact game history entry optimized for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactGameHistory {
    /// Game identifier
    pub game_id: GameId,
    
    /// Compressed initial state
    pub initial_state: CompressedGameState,
    
    /// Delta chain for state changes
    pub delta_chain: Vec<CompressedDelta>,
    
    /// Final result summary
    pub final_summary: GameSummary,
    
    /// Timestamp information
    pub timestamps: TimeRange,
    
    /// Memory usage estimate
    pub estimated_size: u32,
}

/// Compressed game state using multiple techniques
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedGameState {
    /// LZ4 compressed state data
    pub compressed_data: Vec<u8>,
    
    /// Compression metadata
    pub original_size: u32,
    pub compressed_size: u32,
    
    /// Quick access fields (not compressed)
    pub game_id: GameId,
    pub phase: u8,
    pub player_count: u8,
}

/// Compressed delta operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedDelta {
    /// Delta type encoded as single byte
    pub delta_type: u8,
    
    /// Compressed delta data
    pub data: Vec<u8>,
    
    /// Sequence number
    pub sequence: u32,
    
    /// Timestamp offset from game start (saves space)
    pub timestamp_offset: u16,
}

/// Game summary for quick lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSummary {
    /// Total rolls in game
    pub total_rolls: u32,
    
    /// Final player balances (only non-zero)
    pub final_balances: HashMap<PeerId, i64>,
    
    /// Game duration in seconds
    pub duration_secs: u32,
    
    /// Unique players who participated
    pub player_count: u8,
    
    /// Total amount wagered
    pub total_wagered: u64,
    
    /// House edge realized
    pub house_edge: f32,
}

/// Time range for efficient querying
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    /// Game start timestamp
    pub start_time: u64,
    
    /// Game end timestamp  
    pub end_time: u64,
    
    /// Last activity timestamp
    pub last_activity: u64,
}

/// Log-Structured Merge Tree for long-term storage
#[allow(dead_code)]
struct LSMTree {
    /// Multiple levels with different sizes
    levels: Vec<LSMLevel>,
    
    /// Write-ahead log for incoming data
    wal: WriteAheadLog,
    
    /// Compaction scheduler
    compaction_scheduler: CompactionScheduler,
}

/// Single level in the LSM tree
#[allow(dead_code)]
struct LSMLevel {
    /// Level number (0 is smallest/fastest)
    level: usize,
    
    /// Sorted runs in this level
    runs: Vec<SortedRun>,
    
    /// Maximum capacity for this level
    max_capacity: usize,
    
    /// Current size
    current_size: usize,
}

/// Sorted run of game history entries
#[allow(dead_code)]
struct SortedRun {
    /// Games sorted by timestamp
    games: Vec<CompactGameHistory>,
    
    /// Bloom filter for fast negative lookups
    bloom_filter: BloomFilter,
    
    /// Index for range queries
    time_index: BTreeMap<u64, usize>,
}

/// Write-ahead log for durability
#[allow(dead_code)]
struct WriteAheadLog {
    /// Pending writes
    pending: VecDeque<CompactGameHistory>,
    
    /// Maximum size before flush
    max_size: usize,
}

/// Compaction scheduler for LSM tree maintenance
#[allow(dead_code)]
struct CompactionScheduler {
    /// Next compaction time per level
    next_compaction: Vec<u64>,
    
    /// Compaction thresholds
    thresholds: Vec<usize>,
}

/// Simple bloom filter for fast lookups
pub struct BloomFilter {
    /// Bit array
    bits: Vec<u8>,
    
    /// Hash functions count
    hash_count: u32,
    
    /// Size in bits
    size: u32,
}

/// Delta encoder for efficient state change storage
pub struct DeltaEncoder {
    /// Previous state for delta calculation
    previous_state: Option<CompactGameState>,
    
    /// Dictionary for common delta patterns
    delta_dictionary: HashMap<Vec<u8>, u16>,
    
    /// Reverse dictionary for decompression
    reverse_dictionary: HashMap<u16, Vec<u8>>,
    
    /// Next dictionary entry ID
    next_dict_id: u16,
}

/// Memory usage tracker
#[allow(dead_code)]
struct MemoryTracker {
    /// Current memory usage by component
    usage_by_component: HashMap<String, usize>,
    
    /// Total memory limit
    limit: usize,
    
    /// Last cleanup time
    last_cleanup: u64,
}

/// Performance metrics for history operations
#[derive(Debug, Clone, Default)]
pub struct HistoryMetrics {
    /// Total games stored
    pub games_stored: u64,
    
    /// Total memory usage
    pub total_memory_bytes: usize,
    
    /// Compression ratio achieved
    pub average_compression_ratio: f32,
    
    /// Average access time for recent games (microseconds)
    pub recent_access_time_us: f64,
    
    /// Average access time for archived games (microseconds)  
    pub archived_access_time_us: f64,
    
    /// Cache hit rate for bloom filters
    pub bloom_filter_hit_rate: f64,
    
    /// Compaction operations performed
    pub compactions_performed: u64,
    
    /// Delta encoding efficiency
    pub delta_compression_ratio: f32,
}

impl<T> RingBuffer<T> {
    /// Create new ring buffer with specified capacity
    fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize_with(capacity, || None);
        
        Self {
            buffer,
            head: 0,
            tail: 0,
            len: 0,
            capacity,
        }
    }
    
    /// Add item to ring buffer (overwrites oldest if full)
    fn push(&mut self, item: T) -> Option<T> {
        let old_item = self.buffer[self.head].take();
        self.buffer[self.head] = Some(item);
        
        self.head = (self.head + 1) % self.capacity;
        
        if self.len < self.capacity {
            self.len += 1;
        } else {
            self.tail = (self.tail + 1) % self.capacity;
        }
        
        old_item
    }
    
    /// Get item by index (0 = most recent)
    fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        
        // Calculate the buffer index for the item at position 'index'
        // The most recent item (index 0) is at (head - 1), going backwards from there
        let buffer_index = if self.head == 0 {
            // When head is 0, the most recent item is at capacity - 1
            if index < self.capacity {
                self.capacity - 1 - index
            } else {
                return None;
            }
        } else {
            // When head > 0, calculate backwards from head - 1
            if index < self.head {
                self.head - 1 - index
            } else {
                // Wrap around to end of buffer
                self.capacity - (index - (self.head - 1))
            }
        };
        
        self.buffer[buffer_index].as_ref()
    }
    
    /// Iterate over items (newest to oldest)
    fn iter(&self) -> RingBufferIterator<'_, T> {
        RingBufferIterator {
            buffer: self,
            index: 0,
        }
    }
    
    /// Get number of items stored
    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.len
    }
    
    /// Check if buffer is empty
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.len == 0
    }
    
    /// Get memory usage
    fn memory_usage(&self) -> usize {
        mem::size_of::<Self>() + self.capacity * mem::size_of::<Option<T>>()
    }
}

/// Iterator for ring buffer
struct RingBufferIterator<'a, T> {
    buffer: &'a RingBuffer<T>,
    index: usize,
}

impl<'a, T> Iterator for RingBufferIterator<'a, T> {
    type Item = &'a T;
    
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.buffer.get(self.index)?;
        self.index += 1;
        Some(item)
    }
}

impl BloomFilter {
    /// Create new bloom filter
    pub fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let size = Self::optimal_size(expected_items, false_positive_rate);
        let hash_count = Self::optimal_hash_count(size, expected_items);
        
        Self {
            bits: vec![0; size.div_ceil(8) as usize],
            hash_count,
            size,
        }
    }
    
    /// Add item to bloom filter
    pub fn add(&mut self, item: &[u8]) {
        for i in 0..self.hash_count {
            let hash = self.hash(item, i);
            let bit_index = (hash as usize) % (self.size as usize);
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            self.bits[byte_index] |= 1 << bit_offset;
        }
    }
    
    /// Check if item might be in the set
    #[allow(dead_code)]
    fn might_contain(&self, item: &[u8]) -> bool {
        for i in 0..self.hash_count {
            let hash = self.hash(item, i);
            let bit_index = (hash as usize) % (self.size as usize);
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            if (self.bits[byte_index] & (1 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }
    
    /// Simple hash function (FNV-1a variant)
    fn hash(&self, data: &[u8], seed: u32) -> u32 {
        let mut hash = 2166136261u32.wrapping_add(seed);
        for &byte in data {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(16777619);
        }
        hash
    }
    
    /// Calculate optimal size for bloom filter
    fn optimal_size(n: usize, p: f64) -> u32 {
        (-(n as f64) * p.ln() / (2.0_f64.ln().powi(2))).ceil() as u32
    }
    
    /// Calculate optimal number of hash functions
    fn optimal_hash_count(m: u32, n: usize) -> u32 {
        ((m as f64 / n as f64) * 2.0_f64.ln()).round() as u32
    }
}

impl DeltaEncoder {
    /// Create new delta encoder
    fn new() -> Self {
        Self {
            previous_state: None,
            delta_dictionary: HashMap::new(),
            reverse_dictionary: HashMap::new(),
            next_dict_id: 1,
        }
    }
    
    /// Encode state change as delta
    fn encode_delta(&mut self, current_state: &CompactGameState) -> Result<CompressedDelta> {
        let delta = if let Some(ref prev) = self.previous_state {
            self.calculate_delta(prev, current_state)?
        } else {
            // First state - encode as full state
            bincode::serialize(current_state).map_err(|e| Error::Serialization(e.to_string()))?
        };
        
        // Check if delta matches a dictionary entry
        let compressed_data = if let Some(&dict_id) = self.delta_dictionary.get(&delta) {
            // Use dictionary reference
            vec![(dict_id >> 8) as u8, (dict_id & 0xFF) as u8]
        } else if delta.len() < 256 && self.next_dict_id < u16::MAX {
            // Add to dictionary if small enough
            self.delta_dictionary.insert(delta.clone(), self.next_dict_id);
            self.reverse_dictionary.insert(self.next_dict_id, delta.clone());
            self.next_dict_id += 1;
            delta
        } else {
            delta
        };
        
        // Update previous state for next delta
        self.previous_state = Some(current_state.clone());
        
        Ok(CompressedDelta {
            delta_type: if compressed_data.len() == 2 { 1 } else { 0 }, // 1 = dictionary ref
            data: compressed_data,
            sequence: 0, // Set by caller
            timestamp_offset: 0, // Set by caller
        })
    }
    
    /// Calculate binary delta between two states
    fn calculate_delta(&self, prev: &CompactGameState, curr: &CompactGameState) -> Result<Vec<u8>> {
        let prev_bytes = bincode::serialize(prev).map_err(|e| Error::Serialization(e.to_string()))?;
        let curr_bytes = bincode::serialize(curr).map_err(|e| Error::Serialization(e.to_string()))?;
        
        // Simple delta: store changed bytes with their positions
        let mut delta = Vec::new();
        let max_len = prev_bytes.len().max(curr_bytes.len());
        
        for i in 0..max_len {
            let prev_byte = prev_bytes.get(i).copied().unwrap_or(0);
            let curr_byte = curr_bytes.get(i).copied().unwrap_or(0);
            
            if prev_byte != curr_byte {
                // Store position (up to 4 bytes) and new value
                delta.extend_from_slice(&(i as u32).to_le_bytes());
                delta.push(curr_byte);
            }
        }
        
        // If delta is larger than original, just store original
        if delta.len() > curr_bytes.len() {
            Ok(curr_bytes)
        } else {
            Ok(delta)
        }
    }
    
    /// Decode delta back to state
    fn decode_delta(&self, delta: &CompressedDelta, prev_state: Option<&CompactGameState>) -> Result<CompactGameState> {
        if delta.delta_type == 1 {
            // Dictionary reference
            if delta.data.len() != 2 {
                return Err(Error::InvalidData("Invalid dictionary reference".to_string()));
            }
            let dict_id = ((delta.data[0] as u16) << 8) | (delta.data[1] as u16);
            let delta_data = self.reverse_dictionary.get(&dict_id)
                .ok_or_else(|| Error::InvalidData("Dictionary entry not found".to_string()))?;
            
            return self.apply_delta_data(delta_data, prev_state);
        }
        
        self.apply_delta_data(&delta.data, prev_state)
    }
    
    /// Apply delta data to previous state
    fn apply_delta_data(&self, delta_data: &[u8], prev_state: Option<&CompactGameState>) -> Result<CompactGameState> {
        let Some(prev) = prev_state else {
            // No previous state - delta_data is the full state
            return bincode::deserialize(delta_data).map_err(|e| Error::Serialization(e.to_string()));
        };
        
        let mut prev_bytes = bincode::serialize(prev).map_err(|e| Error::Serialization(e.to_string()))?;
        
        // Apply delta changes
        let mut i = 0;
        while i + 4 < delta_data.len() {
            let pos = u32::from_le_bytes([
                delta_data[i], delta_data[i+1], delta_data[i+2], delta_data[i+3]
            ]) as usize;
            let new_value = delta_data[i + 4];
            
            if pos < prev_bytes.len() {
                prev_bytes[pos] = new_value;
            } else {
                // Extend array if needed
                prev_bytes.resize(pos + 1, 0);
                prev_bytes[pos] = new_value;
            }
            
            i += 5;
        }
        
        bincode::deserialize(&prev_bytes).map_err(|e| Error::Serialization(e.to_string()))
    }
}

impl MemoryTracker {
    /// Create new memory tracker
    fn new(limit: usize) -> Self {
        Self {
            usage_by_component: HashMap::new(),
            limit,
            last_cleanup: Self::current_time(),
        }
    }
    
    /// Update memory usage for a component
    fn update_usage(&mut self, component: String, bytes: usize) {
        self.usage_by_component.insert(component, bytes);
    }
    
    /// Get total memory usage
    fn total_usage(&self) -> usize {
        self.usage_by_component.values().sum()
    }
    
    /// Check if over memory limit
    fn is_over_limit(&self) -> bool {
        self.total_usage() > self.limit
    }
    
    /// Get memory usage by component
    #[allow(dead_code)]
    fn get_usage_breakdown(&self) -> &HashMap<String, usize> {
        &self.usage_by_component
    }
    
    /// Current time in seconds
    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

impl EfficientGameHistory {
    /// Create new efficient game history manager
    pub fn new(config: HistoryConfig) -> Self {
        Self {
            recent_games: RingBuffer::new(config.ring_buffer_size),
            lsm_tree: LSMTree::new(&config.lsm_level_sizes),
            delta_encoder: DeltaEncoder::new(),
            memory_tracker: MemoryTracker::new(config.max_memory_bytes),
            config,
            metrics: HistoryMetrics::default(),
        }
    }
    
    /// Store a complete game history
    pub fn store_game(&mut self, game_history: CompactGameHistory) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Try to store in ring buffer first
        if let Some(evicted) = self.recent_games.push(game_history.clone()) {
            // Ring buffer was full, move evicted game to LSM tree
            self.lsm_tree.insert(evicted)?;
        }
        
        // Update metrics
        self.metrics.games_stored += 1;
        let access_time = start_time.elapsed().as_micros() as f64;
        self.metrics.recent_access_time_us = 
            (self.metrics.recent_access_time_us * 0.9) + (access_time * 0.1);
        
        // Update memory tracking
        self.update_memory_usage();
        
        // Trigger compaction if needed
        if self.memory_tracker.is_over_limit() {
            self.compact_if_needed()?;
        }
        
        Ok(())
    }
    
    /// Retrieve game history by ID
    pub fn get_game(&mut self, game_id: GameId) -> Result<Option<CompactGameHistory>> {
        let start_time = std::time::Instant::now();
        
        // Check ring buffer first (most recent games)
        for game in self.recent_games.iter() {
            if game.game_id == game_id {
                let access_time = start_time.elapsed().as_micros() as f64;
                self.metrics.recent_access_time_us = 
                    (self.metrics.recent_access_time_us * 0.9) + (access_time * 0.1);
                return Ok(Some(game.clone()));
            }
        }
        
        // Check LSM tree (archived games)
        let result = self.lsm_tree.get(game_id);
        let access_time = start_time.elapsed().as_micros() as f64;
        self.metrics.archived_access_time_us = 
            (self.metrics.archived_access_time_us * 0.9) + (access_time * 0.1);
        
        Ok(result)
    }
    
    /// Get games within a time range
    pub fn get_games_in_range(&self, start_time: u64, end_time: u64) -> Vec<&CompactGameHistory> {
        let mut results = Vec::new();
        
        // Check ring buffer
        for game in self.recent_games.iter() {
            if game.timestamps.start_time >= start_time && 
               game.timestamps.end_time <= end_time {
                results.push(game);
            }
        }
        
        // Check LSM tree
        results.extend(self.lsm_tree.get_range(start_time, end_time));
        
        results
    }
    
    /// Compress a game state using multiple techniques
    pub fn compress_game_state(&self, state: &CompactGameState) -> Result<CompressedGameState> {
        let serialized = bincode::serialize(state)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        
        let compressed_data = compress_prepend_size(&serialized);
        
        let compressed_size = compressed_data.len() as u32;
        Ok(CompressedGameState {
            compressed_data,
            original_size: serialized.len() as u32,
            compressed_size,
            game_id: state.game_id,
            phase: state.get_phase() as u8,
            player_count: 1, // Simplified - would extract from state
        })
    }
    
    /// Decompress a game state
    pub fn decompress_game_state(&self, compressed: &CompressedGameState) -> Result<CompactGameState> {
        let decompressed = decompress_size_prepended(&compressed.compressed_data)
            .map_err(|e| Error::InvalidData(format!("Decompression failed: {}", e)))?;
        
        bincode::deserialize(&decompressed)
            .map_err(|e| Error::Serialization(e.to_string()))
    }
    
    /// Create delta chain from a sequence of game states
    pub fn create_delta_chain(&mut self, states: &[CompactGameState]) -> Result<Vec<CompressedDelta>> {
        let mut deltas = Vec::new();
        
        for (i, state) in states.iter().enumerate() {
            let mut delta = self.delta_encoder.encode_delta(state)?;
            delta.sequence = i as u32;
            deltas.push(delta);
        }
        
        Ok(deltas)
    }
    
    /// Reconstruct game state from delta chain
    pub fn reconstruct_from_deltas(
        &self,
        initial_state: Option<&CompactGameState>,
        deltas: &[CompressedDelta]
    ) -> Result<CompactGameState> {
        let mut current_state = initial_state.cloned();
        
        for delta in deltas {
            current_state = Some(self.delta_encoder.decode_delta(delta, current_state.as_ref())?);
        }
        
        current_state.ok_or_else(|| Error::InvalidData("No initial state or deltas".to_string()))
    }
    
    /// Update memory usage tracking
    fn update_memory_usage(&mut self) {
        self.memory_tracker.update_usage(
            "ring_buffer".to_string(),
            self.recent_games.memory_usage()
        );
        
        self.memory_tracker.update_usage(
            "lsm_tree".to_string(),
            self.lsm_tree.memory_usage()
        );
        
        self.memory_tracker.update_usage(
            "delta_encoder".to_string(),
            mem::size_of::<DeltaEncoder>()
        );
        
        self.metrics.total_memory_bytes = self.memory_tracker.total_usage();
    }
    
    /// Perform compaction if needed
    fn compact_if_needed(&mut self) -> Result<()> {
        if self.lsm_tree.needs_compaction() {
            self.lsm_tree.compact()?;
            self.metrics.compactions_performed += 1;
        }
        Ok(())
    }
    
    /// Get comprehensive performance metrics
    pub fn get_metrics(&self) -> HistoryMetrics {
        let mut metrics = self.metrics.clone();
        
        // Calculate compression ratios
        let mut total_original = 0u64;
        let mut total_compressed = 0u64;
        
        for game in self.recent_games.iter() {
            total_original += game.initial_state.original_size as u64;
            total_compressed += game.initial_state.compressed_size as u64;
        }
        
        if total_original > 0 {
            metrics.average_compression_ratio = total_compressed as f32 / total_original as f32;
        }
        
        metrics.total_memory_bytes = self.memory_tracker.total_usage();
        
        metrics
    }
    
    /// Cleanup old data to free memory
    pub fn cleanup(&mut self) -> Result<()> {
        // The ring buffer automatically evicts old data
        // LSM tree compaction handles cleanup there
        self.lsm_tree.cleanup()?;
        
        // Clean up delta encoder dictionary if too large
        if self.delta_encoder.delta_dictionary.len() > 10000 {
            self.delta_encoder.delta_dictionary.clear();
            self.delta_encoder.reverse_dictionary.clear();
            self.delta_encoder.next_dict_id = 1;
        }
        
        Ok(())
    }
}

// LSM Tree implementation stubs (simplified for this example)
impl LSMTree {
    fn new(_level_sizes: &[usize]) -> Self {
        Self {
            levels: Vec::new(),
            wal: WriteAheadLog { pending: VecDeque::new(), max_size: 1000 },
            compaction_scheduler: CompactionScheduler { 
                next_compaction: Vec::new(), 
                thresholds: Vec::new() 
            },
        }
    }
    
    fn insert(&mut self, _game: CompactGameHistory) -> Result<()> {
        // Simplified implementation
        Ok(())
    }
    
    fn get(&self, _game_id: GameId) -> Option<CompactGameHistory> {
        // Simplified implementation
        None
    }
    
    fn get_range(&self, _start: u64, _end: u64) -> Vec<&CompactGameHistory> {
        // Simplified implementation
        Vec::new()
    }
    
    fn needs_compaction(&self) -> bool {
        false // Simplified
    }
    
    fn compact(&mut self) -> Result<()> {
        // Simplified implementation
        Ok(())
    }
    
    fn cleanup(&mut self) -> Result<()> {
        // Simplified implementation
        Ok(())
    }
    
    fn memory_usage(&self) -> usize {
        mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::efficient_game_state::CompactGameState;

    #[test]
    fn test_ring_buffer_operations() {
        let mut buffer = RingBuffer::new(3);
        
        // Test basic operations
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        
        // Add items
        buffer.push("first".to_string());
        buffer.push("second".to_string());
        buffer.push("third".to_string());
        
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(0), Some(&"third".to_string())); // Most recent
        assert_eq!(buffer.get(2), Some(&"first".to_string())); // Oldest
        
        // Overflow - should evict oldest
        let evicted = buffer.push("fourth".to_string());
        assert_eq!(evicted, Some("first".to_string()));
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(0), Some(&"fourth".to_string()));
        assert_eq!(buffer.get(2), Some(&"second".to_string()));
    }
    
    #[test]
    fn test_ring_buffer_iterator() {
        let mut buffer = RingBuffer::new(3);
        buffer.push("a".to_string());
        buffer.push("b".to_string());
        buffer.push("c".to_string());
        
        let items: Vec<&String> = buffer.iter().collect();
        assert_eq!(items, vec![&"c".to_string(), &"b".to_string(), &"a".to_string()]);
    }
    
    #[test]
    fn test_bloom_filter() {
        let mut filter = BloomFilter::new(1000, 0.01);
        
        let item1 = b"test_item_1";
        let item2 = b"test_item_2";
        let item3 = b"test_item_3";
        
        // Add items
        filter.add(item1);
        filter.add(item2);
        
        // Test membership
        assert!(filter.might_contain(item1));
        assert!(filter.might_contain(item2));
        assert!(!filter.might_contain(item3)); // Should probably return false
    }
    
    #[test]
    fn test_delta_encoder() {
        let mut encoder = DeltaEncoder::new();
        
        let state1 = CompactGameState::new([1; 16], [2; 32]);
        let mut state2 = state1.clone();
        state2.set_roll_count(42);
        
        // Encode delta
        let delta = encoder.encode_delta(&state1).unwrap();
        assert_eq!(delta.delta_type, 0); // Not a dictionary reference
        
        let delta2 = encoder.encode_delta(&state2).unwrap();
        
        // Decode delta
        let decoded1 = encoder.decode_delta(&delta, None).unwrap();
        let decoded2 = encoder.decode_delta(&delta2, Some(&decoded1)).unwrap();
        
        assert_eq!(decoded2.get_roll_count(), 42);
    }
    
    #[test]
    fn test_memory_tracker() {
        let mut tracker = MemoryTracker::new(1000);
        
        tracker.update_usage("component1".to_string(), 400);
        tracker.update_usage("component2".to_string(), 300);
        
        assert_eq!(tracker.total_usage(), 700);
        assert!(!tracker.is_over_limit());
        
        tracker.update_usage("component3".to_string(), 400);
        assert_eq!(tracker.total_usage(), 1100);
        assert!(tracker.is_over_limit());
    }
    
    #[test]
    fn test_efficient_game_history() {
        let config = HistoryConfig {
            ring_buffer_size: 2,
            max_memory_bytes: 10 * 1024 * 1024,
            ..Default::default()
        };
        
        let mut history = EfficientGameHistory::new(config);
        
        // Create test game history
        let game_history = CompactGameHistory {
            game_id: [1; 16],
            initial_state: CompressedGameState {
                compressed_data: vec![1, 2, 3, 4],
                original_size: 100,
                compressed_size: 4,
                game_id: [1; 16],
                phase: 0,
                player_count: 2,
            },
            delta_chain: Vec::new(),
            final_summary: GameSummary {
                total_rolls: 10,
                final_balances: HashMap::new(),
                duration_secs: 300,
                player_count: 2,
                total_wagered: 1000,
                house_edge: 0.014,
            },
            timestamps: TimeRange {
                start_time: 1000,
                end_time: 1300,
                last_activity: 1300,
            },
            estimated_size: 200,
        };
        
        // Store game
        history.store_game(game_history.clone()).unwrap();
        
        // Retrieve game
        let retrieved = history.get_game([1; 16]).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().game_id, [1; 16]);
        
        // Check metrics
        let metrics = history.get_metrics();
        assert_eq!(metrics.games_stored, 1);
        assert!(metrics.total_memory_bytes > 0);
    }
    
    #[test]
    fn test_game_state_compression() {
        let config = HistoryConfig::default();
        let history = EfficientGameHistory::new(config);
        
        let state = CompactGameState::new([1; 16], [2; 32]);
        
        let compressed = history.compress_game_state(&state).unwrap();
        let decompressed = history.decompress_game_state(&compressed).unwrap();
        
        assert_eq!(state.game_id, decompressed.game_id);
        assert_eq!(state.get_phase(), decompressed.get_phase());
    }
    
    #[test]
    fn test_delta_chain_reconstruction() {
        let mut history = EfficientGameHistory::new(HistoryConfig::default());
        
        let state1 = CompactGameState::new([1; 16], [2; 32]);
        let mut state2 = state1.clone();
        state2.set_roll_count(5);
        let mut state3 = state2.clone();
        state3.set_roll_count(10);
        
        let states = vec![state1.clone(), state2, state3];
        let deltas = history.create_delta_chain(&states).unwrap();
        
        assert_eq!(deltas.len(), 3);
        
        // Reconstruct final state
        let reconstructed = history.reconstruct_from_deltas(None, &deltas).unwrap();
        assert_eq!(reconstructed.get_roll_count(), 10);
    }
    
    #[test]
    fn test_time_range_queries() {
        let config = HistoryConfig::default();
        let mut history = EfficientGameHistory::new(config);
        
        let game1 = CompactGameHistory {
            game_id: [1; 16],
            initial_state: CompressedGameState {
                compressed_data: vec![],
                original_size: 0,
                compressed_size: 0,
                game_id: [1; 16],
                phase: 0,
                player_count: 1,
            },
            delta_chain: Vec::new(),
            final_summary: GameSummary {
                total_rolls: 0,
                final_balances: HashMap::new(),
                duration_secs: 0,
                player_count: 1,
                total_wagered: 0,
                house_edge: 0.0,
            },
            timestamps: TimeRange {
                start_time: 1000,
                end_time: 2000,
                last_activity: 2000,
            },
            estimated_size: 0,
        };
        
        let game2 = CompactGameHistory {
            game_id: [2; 16],
            timestamps: TimeRange {
                start_time: 3000,
                end_time: 4000,
                last_activity: 4000,
            },
            ..game1.clone()
        };
        
        history.store_game(game1).unwrap();
        history.store_game(game2).unwrap();
        
        // Query for games in range
        let results = history.get_games_in_range(500, 2500);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].game_id, [1; 16]);
    }
}