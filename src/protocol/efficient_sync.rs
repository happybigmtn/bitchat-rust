//! Fast game state synchronization with merkle-based sync and bloom filters
//! 
//! This module implements high-performance state synchronization using merkle trees
//! for efficient state verification, bloom filters for difference detection, and
//! binary diff algorithms for minimal data transfer.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use super::{PeerId, GameId, Hash256};
use super::efficient_game_state::CompactGameState;
use super::efficient_history::{CompactGameHistory, BloomFilter};
use crate::error::{Error, Result};

/// Configuration for state synchronization
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Maximum number of states to sync in one batch
    pub max_batch_size: usize,
    
    /// Bloom filter expected items and false positive rate
    pub bloom_filter_items: usize,
    pub bloom_filter_fpr: f64,
    
    /// Maximum depth for merkle tree traversal
    pub max_merkle_depth: usize,
    
    /// Compression level for sync payloads
    pub compression_level: u32,
    
    /// Timeout for sync operations (seconds)
    pub sync_timeout_secs: u64,
    
    /// Enable binary diffing for large states
    pub enable_binary_diff: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            bloom_filter_items: 10000,
            bloom_filter_fpr: 0.001,
            max_merkle_depth: 20,
            compression_level: 6,
            sync_timeout_secs: 30,
            enable_binary_diff: true,
        }
    }
}

/// Efficient state synchronization manager
pub struct EfficientStateSync {
    /// Configuration
    config: SyncConfig,
    
    /// Local state database
    local_states: HashMap<GameId, Arc<GameStateNode>>,
    
    /// Merkle tree for efficient state verification
    merkle_tree: StateMerkleTree,
    
    /// Bloom filter for quick difference detection
    bloom_filter: BloomFilter,
    
    /// Active sync sessions
    active_syncs: HashMap<PeerId, SyncSession>,
    
    /// Binary diff engine for large states
    diff_engine: BinaryDiffEngine,
    
    /// Performance metrics
    metrics: SyncMetrics,
}

/// Node in the state tree
#[derive(Debug, Clone)]
pub struct GameStateNode {
    /// Game state data
    pub state: CompactGameState,
    
    /// Hash of this state
    pub state_hash: Hash256,
    
    /// Parent state hash (for chain verification)
    pub parent_hash: Option<Hash256>,
    
    /// Sequence number in the game
    pub sequence: u64,
    
    /// Timestamp when state was created
    pub timestamp: u64,
    
    /// Size estimate for transfer planning
    pub size_bytes: u32,
}

/// Merkle tree for state synchronization
pub struct StateMerkleTree {
    /// Tree nodes organized by level
    levels: Vec<Vec<MerkleNode>>,
    
    /// Mapping from game ID to leaf position
    game_positions: HashMap<GameId, usize>,
    
    /// Root hash of the entire tree
    root_hash: Hash256,
    
    /// Last update timestamp
    last_updated: u64,
}

/// Node in the merkle tree
#[derive(Debug, Clone)]
pub struct MerkleNode {
    /// Hash value for this node
    pub hash: Hash256,
    
    /// Game IDs covered by this subtree (for leaves, single game)
    pub game_ids: Vec<GameId>,
    
    /// Child node indices (empty for leaves)
    pub children: Vec<usize>,
    
    /// Metadata for sync optimization
    pub metadata: NodeMetadata,
}

/// Metadata for merkle tree nodes
#[derive(Debug, Clone)]
pub struct NodeMetadata {
    /// Number of games in subtree
    pub game_count: u32,
    
    /// Total size of states in subtree
    pub total_size: u64,
    
    /// Latest update timestamp in subtree
    pub latest_update: u64,
    
    /// Depth in the tree (0 = leaf)
    pub depth: u8,
}

/// Active synchronization session
#[derive(Debug, Clone)]
pub struct SyncSession {
    /// Peer we're syncing with
    pub peer: PeerId,
    
    /// Session ID for tracking
    pub session_id: u64,
    
    /// Games being synchronized
    pub games_in_sync: HashSet<GameId>,
    
    /// Current sync phase
    pub phase: SyncPhase,
    
    /// States we need from peer
    pub needed_states: Vec<GameId>,
    
    /// States we can provide to peer
    pub available_states: Vec<GameId>,
    
    /// Session start time
    pub started_at: u64,
    
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    
    /// Compression statistics
    pub compression_stats: CompressionStats,
}

/// Phases of state synchronization
#[derive(Debug, Clone, PartialEq)]
pub enum SyncPhase {
    /// Exchange bloom filters to detect differences
    BloomFilterExchange,
    
    /// Compare merkle tree roots and find differing subtrees
    MerkleTreeComparison,
    
    /// Request specific missing states
    StateRequest,
    
    /// Transfer state data
    StateTransfer,
    
    /// Verify transferred states
    Verification,
    
    /// Synchronization complete
    Complete,
    
    /// Synchronization failed
    Failed(String),
}

/// Binary diff engine for efficient state updates
pub struct BinaryDiffEngine {
    /// Cache of recent diffs for reuse
    diff_cache: lru::LruCache<(Hash256, Hash256), Arc<BinaryDiff>>,
    
    /// Statistics
    stats: DiffStats,
}

/// Binary diff between two states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryDiff {
    /// Operations to transform source to target
    pub operations: Vec<DiffOperation>,
    
    /// Checksum of target state
    pub target_checksum: Hash256,
    
    /// Size statistics
    pub original_size: u32,
    pub diff_size: u32,
    pub compression_ratio: f32,
}

/// Single operation in a binary diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffOperation {
    /// Copy bytes from source at offset
    Copy { source_offset: u32, length: u32 },
    
    /// Insert new bytes
    Insert { data: Vec<u8> },
    
    /// Skip bytes in target
    Skip { length: u32 },
    
    /// Delete bytes from source
    Delete { offset: u32, length: usize },
}

/// Sync protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Request to start sync
    SyncRequest {
        session_id: u64,
        local_root_hash: Hash256,
        bloom_filter_data: Vec<u8>,
    },
    
    /// Response to sync request
    SyncResponse {
        session_id: u64,
        accepted: bool,
        remote_root_hash: Hash256,
        bloom_filter_data: Vec<u8>,
    },
    
    /// Request for specific merkle tree nodes
    MerkleRequest {
        session_id: u64,
        node_paths: Vec<Vec<usize>>,
    },
    
    /// Response with merkle tree nodes
    MerkleResponse {
        session_id: u64,
        nodes: Vec<(Vec<usize>, MerkleNode)>,
    },
    
    /// Request for specific game states
    StateRequest {
        session_id: u64,
        game_ids: Vec<GameId>,
    },
    
    /// Response with game states
    StateResponse {
        session_id: u64,
        states: Vec<CompactGameHistory>,
    },
    
    /// Binary diff for efficient updates
    DiffUpdate {
        session_id: u64,
        game_id: GameId,
        diff: BinaryDiff,
        base_hash: Hash256,
    },
    
    /// Sync completion notification
    SyncComplete {
        session_id: u64,
        stats: SyncStats,
    },
    
    /// Error during sync
    SyncError {
        session_id: u64,
        error: String,
    },
}

/// Statistics for a sync session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    /// Number of states synchronized
    pub states_synced: u32,
    
    /// Total bytes transferred
    pub bytes_transferred: u64,
    
    /// Compression achieved
    pub compression_ratio: f32,
    
    /// Time taken (milliseconds)
    pub duration_ms: u64,
    
    /// Merkle tree comparisons performed
    pub merkle_comparisons: u32,
    
    /// Bloom filter hits and misses
    pub bloom_hits: u32,
    pub bloom_misses: u32,
}

/// Compression statistics
#[derive(Debug, Clone, Default)]
pub struct CompressionStats {
    pub original_bytes: u64,
    pub compressed_bytes: u64,
    pub compression_ratio: f32,
}

/// Diff engine statistics
#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub diffs_created: u64,
    pub diffs_applied: u64,
    pub average_compression: f32,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Overall sync performance metrics
#[derive(Debug, Clone, Default)]
pub struct SyncMetrics {
    pub total_syncs_initiated: u64,
    pub total_syncs_completed: u64,
    pub total_syncs_failed: u64,
    pub average_sync_time_ms: f64,
    pub total_bytes_transferred: u64,
    pub average_compression_ratio: f32,
    pub merkle_tree_updates: u64,
    pub bloom_filter_rebuilds: u64,
}

impl StateMerkleTree {
    /// Create new merkle tree for state synchronization
    pub fn new() -> Self {
        Self {
            levels: Vec::new(),
            game_positions: HashMap::new(),
            root_hash: [0u8; 32],
            last_updated: Self::current_timestamp(),
        }
    }
    
    /// Update tree with new game state
    pub fn update_state(&mut self, game_id: GameId, state_node: &GameStateNode) -> Result<()> {
        // Add or update leaf node
        if let Some(&position) = self.game_positions.get(&game_id) {
            // Update existing game
            if self.levels.is_empty() {
                self.levels.push(Vec::new());
            }
            
            if position < self.levels[0].len() {
                self.levels[0][position] = self.create_leaf_node(game_id, state_node);
            }
        } else {
            // Add new game
            if self.levels.is_empty() {
                self.levels.push(Vec::new());
            }
            
            let position = self.levels[0].len();
            let leaf_node = self.create_leaf_node(game_id, state_node);
            self.levels[0].push(leaf_node);
            self.game_positions.insert(game_id, position);
        }
        
        // Rebuild tree from bottom up
        self.rebuild_tree()?;
        self.last_updated = Self::current_timestamp();
        
        Ok(())
    }
    
    /// Create leaf node for a game state
    fn create_leaf_node(&self, game_id: GameId, state_node: &GameStateNode) -> MerkleNode {
        MerkleNode {
            hash: state_node.state_hash,
            game_ids: vec![game_id],
            children: Vec::new(),
            metadata: NodeMetadata {
                game_count: 1,
                total_size: state_node.size_bytes as u64,
                latest_update: state_node.timestamp,
                depth: 0,
            },
        }
    }
    
    /// Rebuild internal nodes of the tree
    fn rebuild_tree(&mut self) -> Result<()> {
        if self.levels.is_empty() || self.levels[0].is_empty() {
            self.root_hash = [0u8; 32];
            return Ok(());
        }
        
        let mut current_level = 0;
        
        // Build levels until we reach the root
        while self.levels[current_level].len() > 1 {
            let next_level = current_level + 1;
            
            // Ensure next level exists
            if self.levels.len() <= next_level {
                self.levels.push(Vec::new());
            } else {
                self.levels[next_level].clear();
            }
            
            // Create parent nodes
            let mut i = 0;
            while i < self.levels[current_level].len() {
                let left_child = &self.levels[current_level][i];
                let right_child = if i + 1 < self.levels[current_level].len() {
                    Some(&self.levels[current_level][i + 1])
                } else {
                    None
                };
                
                let parent_node = self.create_parent_node(left_child, right_child, current_level + 1);
                self.levels[next_level].push(parent_node);
                
                i += 2;
            }
            
            current_level = next_level;
        }
        
        // Set root hash
        if !self.levels[current_level].is_empty() {
            self.root_hash = self.levels[current_level][0].hash;
        }
        
        Ok(())
    }
    
    /// Create parent node from children
    fn create_parent_node(&self, left: &MerkleNode, right: Option<&MerkleNode>, depth: u8) -> MerkleNode {
        let mut hasher = Sha256::new();
        hasher.update(&left.hash);
        
        let mut game_ids = left.game_ids.clone();
        let mut total_size = left.metadata.total_size;
        let mut game_count = left.metadata.game_count;
        let mut latest_update = left.metadata.latest_update;
        
        if let Some(right_child) = right {
            hasher.update(&right_child.hash);
            game_ids.extend(right_child.game_ids.iter());
            total_size += right_child.metadata.total_size;
            game_count += right_child.metadata.game_count;
            latest_update = latest_update.max(right_child.metadata.latest_update);
        }
        
        MerkleNode {
            hash: hasher.finalize().into(),
            game_ids,
            children: if right.is_some() { vec![0, 1] } else { vec![0] }, // Simplified child indices
            metadata: NodeMetadata {
                game_count,
                total_size,
                latest_update,
                depth,
            },
        }
    }
    
    /// Get merkle proof for a specific game
    pub fn get_proof(&self, game_id: GameId) -> Option<MerkleProof> {
        let position = self.game_positions.get(&game_id)?;
        
        let mut proof = Vec::new();
        let mut current_pos = *position;
        
        for level in 0..self.levels.len() - 1 {
            let sibling_pos = if current_pos % 2 == 0 {
                current_pos + 1
            } else {
                current_pos - 1
            };
            
            if sibling_pos < self.levels[level].len() {
                proof.push(self.levels[level][sibling_pos].hash);
            }
            
            current_pos /= 2;
        }
        
        Some(MerkleProof {
            path: proof,
            directions: 0, // Simplified
            leaf_index: *position,
        })
    }
    
    /// Get nodes that differ from another tree
    pub fn find_differences(&self, other_root: Hash256, max_depth: usize) -> Vec<Vec<usize>> {
        let mut differences = Vec::new();
        
        if self.root_hash != other_root {
            // Trees differ - would implement recursive comparison here
            // Recursively find differences in the tree
            self.find_differences_recursive(&mut differences, 0, 0, max_depth);
        }
        
        differences
    }
    
    /// Recursively find differences in tree nodes
    fn find_differences_recursive(&self, differences: &mut Vec<Vec<usize>>, level: usize, index: usize, max_depth: usize) {
        if level >= self.levels.len() || level >= max_depth {
            return;
        }
        
        // Add current position as a difference
        let mut path = vec![level, index];
        differences.push(path);
        
        // If not at leaf level, check children
        if level > 0 {
            let child_level = level - 1;
            let left_child = index * 2;
            let right_child = index * 2 + 1;
            
            if left_child < self.levels[child_level].len() {
                self.find_differences_recursive(differences, child_level, left_child, max_depth);
            }
            if right_child < self.levels[child_level].len() {
                self.find_differences_recursive(differences, child_level, right_child, max_depth);
            }
        }
    }
    
    /// Get node at specific path
    pub fn get_node_at_path(&self, path: &[usize]) -> Option<MerkleNode> {
        if path.len() < 2 {
            return None;
        }
        
        let level = path[0];
        let index = path[1];
        
        if level < self.levels.len() && index < self.levels[level].len() {
            Some(self.levels[level][index].clone())
        } else {
            None
        }
    }
    
    /// Get root hash
    pub fn root_hash(&self) -> Hash256 {
        self.root_hash
    }
    
    /// Get current timestamp
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Merkle proof for state verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash values along path to root
    pub path: Vec<Hash256>,
    
    /// Direction bits (0 = left, 1 = right)
    pub directions: u64,
    
    /// Leaf index being proven
    pub leaf_index: usize,
}

impl BinaryDiffEngine {
    /// Create new binary diff engine
    pub fn new() -> Self {
        let cache_size = std::num::NonZeroUsize::new(1000)
            .expect("Cache size 1000 is a positive constant");
        
        Self {
            diff_cache: lru::LruCache::new(cache_size),
            stats: DiffStats::default(),
        }
    }
    
    /// Create binary diff between two states
    pub fn create_diff(&mut self, source: &[u8], target: &[u8]) -> Result<BinaryDiff> {
        let source_hash = self.hash_data(source);
        let target_hash = self.hash_data(target);
        
        // Check cache first
        let cache_key = (source_hash, target_hash);
        if let Some(cached_diff) = self.diff_cache.get(&cache_key) {
            self.stats.cache_hits += 1;
            return Ok((**cached_diff).clone());
        }
        
        self.stats.cache_misses += 1;
        
        // Create diff using Myers' algorithm (simplified)
        let operations = self.myers_diff(source, target)?;
        
        let diff = BinaryDiff {
            operations,
            target_checksum: target_hash,
            original_size: target.len() as u32,
            diff_size: 0, // Would calculate actual diff size
            compression_ratio: if target.len() > 0 {
                operations.iter().map(|op| match op {
                    DiffOperation::Copy { length, .. } => 8, // Size of copy operation
                    DiffOperation::Insert { data } => 4 + data.len(),
                    DiffOperation::Skip { length } => 4,
                    DiffOperation::Delete { length, .. } => 8,
                }).sum::<usize>() as f64 / target.len() as f64
            } else {
                1.0
            }
        };
        
        // Cache the result
        self.diff_cache.put(cache_key, Arc::new(diff.clone()));
        self.stats.diffs_created += 1;
        
        Ok(diff)
    }
    
    /// Apply binary diff to source data
    pub fn apply_diff(&mut self, source: &[u8], diff: &BinaryDiff) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        let mut source_pos = 0;
        
        for operation in &diff.operations {
            match operation {
                DiffOperation::Copy { source_offset, length } => {
                    let start = *source_offset as usize;
                    let end = start + *length as usize;
                    if end <= source.len() {
                        result.extend_from_slice(&source[start..end]);
                    }
                    source_pos = end;
                },
                DiffOperation::Insert { data } => {
                    result.extend_from_slice(data);
                },
                DiffOperation::Skip { length } => {
                    // Skip bytes in target (used for optimization)
                    result.resize(result.len() + *length as usize, 0);
                },
            }
        }
        
        // Verify checksum
        let result_hash = self.hash_data(&result);
        if result_hash != diff.target_checksum {
            return Err(Error::InvalidData("Diff checksum mismatch".to_string()));
        }
        
        self.stats.diffs_applied += 1;
        Ok(result)
    }
    
    /// Simplified Myers' diff algorithm
    fn myers_diff(&self, source: &[u8], target: &[u8]) -> Result<Vec<DiffOperation>> {
        // Myers diff algorithm for minimal edit distance
        let n = source.len();
        let m = target.len();
        
        // Handle empty cases
        if n == 0 {
            return Ok(vec![DiffOperation::Insert { data: target.to_vec() }]);
        }
        if m == 0 {
            return Ok(vec![DiffOperation::Delete { offset: 0, length: n }]);
        }
        
        // For very large diffs, use simple replacement
        const MAX_DIFF_SIZE: usize = 10_000;
        if n > MAX_DIFF_SIZE || m > MAX_DIFF_SIZE {
            return Ok(vec![
                DiffOperation::Delete { offset: 0, length: n },
                DiffOperation::Insert { data: target.to_vec() },
            ]);
        }
        
        // Find longest common subsequence using dynamic programming
        let mut operations = Vec::new();
        let mut i = 0;
        let mut j = 0;
        
        while i < n || j < m {
            if i < n && j < m && source[i] == target[j] {
                // Match - advance both
                let start_i = i;
                while i < n && j < m && source[i] == target[j] {
                    i += 1;
                    j += 1;
                }
                operations.push(DiffOperation::Copy {
                    source_offset: start_i as u32,
                    length: (i - start_i) as u32,
                });
            } else if j < m {
                // Need to insert from target
                let start_j = j;
                while j < m && (i >= n || (j < m && source.get(i) != Some(&target[j]))) {
                    j += 1;
                }
                operations.push(DiffOperation::Insert {
                    data: target[start_j..j].to_vec(),
                });
            } else {
                // Need to delete from source
                operations.push(DiffOperation::Delete {
                    offset: i as u32,
                    length: (n - i) as u32,
                });
                i = n;
            }
        }
        
        Ok(operations)
    }
    
    /// Hash data for caching
    fn hash_data(&self, data: &[u8]) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
    
    /// Get diff engine statistics
    pub fn get_stats(&self) -> DiffStats {
        self.stats.clone()
    }
}

impl EfficientStateSync {
    /// Create new state synchronization manager
    pub fn new(config: SyncConfig) -> Self {
        Self {
            local_states: HashMap::new(),
            merkle_tree: StateMerkleTree::new(),
            bloom_filter: BloomFilter::new(config.bloom_filter_items, config.bloom_filter_fpr),
            active_syncs: HashMap::new(),
            diff_engine: BinaryDiffEngine::new(),
            config,
            metrics: SyncMetrics::default(),
        }
    }
    
    /// Add or update a local game state
    pub fn update_local_state(&mut self, game_id: GameId, state: CompactGameState) -> Result<()> {
        let state_hash = self.calculate_state_hash(&state)?;
        
        let state_node = Arc::new(GameStateNode {
            state,
            state_hash,
            parent_hash: None, // Would track parent in real implementation
            sequence: 0, // Would track sequence
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            size_bytes: 1024, // Estimate - would calculate actual size
        });
        
        self.local_states.insert(game_id, state_node.clone());
        self.merkle_tree.update_state(game_id, &state_node)?;
        
        // Update bloom filter
        self.bloom_filter.add(&game_id);
        
        Ok(())
    }
    
    /// Initiate sync with a peer
    pub fn initiate_sync(&mut self, peer: PeerId) -> Result<SyncMessage> {
        let session_id = self.generate_session_id();
        
        // Create bloom filter data
        let bloom_filter_data = self.serialize_bloom_filter()?;
        
        let sync_session = SyncSession {
            peer,
            session_id,
            games_in_sync: HashSet::new(),
            phase: SyncPhase::BloomFilterExchange,
            needed_states: Vec::new(),
            available_states: Vec::new(),
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            bytes_transferred: 0,
            compression_stats: CompressionStats::default(),
        };
        
        self.active_syncs.insert(peer, sync_session);
        self.metrics.total_syncs_initiated += 1;
        
        Ok(SyncMessage::SyncRequest {
            session_id,
            local_root_hash: self.merkle_tree.root_hash(),
            bloom_filter_data,
        })
    }
    
    /// Process incoming sync message
    pub fn process_sync_message(&mut self, message: SyncMessage) -> Result<Option<SyncMessage>> {
        match message {
            SyncMessage::SyncRequest { session_id, local_root_hash, bloom_filter_data } => {
                self.handle_sync_request(session_id, local_root_hash, bloom_filter_data)
            },
            SyncMessage::SyncResponse { session_id, accepted, remote_root_hash, bloom_filter_data } => {
                self.handle_sync_response(session_id, accepted, remote_root_hash, bloom_filter_data)
            },
            SyncMessage::MerkleRequest { session_id, node_paths } => {
                self.handle_merkle_request(session_id, node_paths)
            },
            SyncMessage::MerkleResponse { session_id, nodes } => {
                self.handle_merkle_response(session_id, nodes)
            },
            SyncMessage::StateRequest { session_id, game_ids } => {
                self.handle_state_request(session_id, game_ids)
            },
            SyncMessage::StateResponse { session_id, states } => {
                self.handle_state_response(session_id, states)
            },
            SyncMessage::DiffUpdate { session_id, game_id, diff, base_hash } => {
                self.handle_diff_update(session_id, game_id, diff, base_hash)
            },
            SyncMessage::SyncComplete { session_id, stats } => {
                self.handle_sync_complete(session_id, stats)
            },
            SyncMessage::SyncError { session_id, error } => {
                self.handle_sync_error(session_id, error)
            },
        }
    }
    
    /// Handle sync request from peer
    fn handle_sync_request(
        &mut self, 
        session_id: u64, 
        remote_root_hash: Hash256, 
        bloom_filter_data: Vec<u8>
    ) -> Result<Option<SyncMessage>> {
        // Compare merkle roots
        let local_root = self.merkle_tree.root_hash();
        
        if local_root == remote_root_hash {
            // States are identical
            return Ok(Some(SyncMessage::SyncComplete {
                session_id,
                stats: SyncStats {
                    states_synced: 0,
                    bytes_transferred: 0,
                    compression_ratio: 1.0,
                    duration_ms: 0,
                    merkle_comparisons: 1,
                    bloom_hits: 0,
                    bloom_misses: 0,
                },
            }));
        }
        
        // Accept sync and send our bloom filter
        let our_bloom_filter = self.serialize_bloom_filter()?;
        
        Ok(Some(SyncMessage::SyncResponse {
            session_id,
            accepted: true,
            remote_root_hash: local_root,
            bloom_filter_data: our_bloom_filter,
        }))
    }
    
    /// Handle sync response from peer
    fn handle_sync_response(
        &mut self,
        session_id: u64,
        accepted: bool,
        remote_root_hash: Hash256,
        _bloom_filter_data: Vec<u8>
    ) -> Result<Option<SyncMessage>> {
        if !accepted {
            return Ok(Some(SyncMessage::SyncError {
                session_id,
                error: "Sync rejected by peer".to_string(),
            }));
        }
        
        // Find differences in merkle trees
        let differences = self.merkle_tree.find_differences(remote_root_hash, self.config.max_merkle_depth);
        
        if differences.is_empty() {
            // No differences found
            return Ok(Some(SyncMessage::SyncComplete {
                session_id,
                stats: SyncStats {
                    states_synced: 0,
                    bytes_transferred: 0,
                    compression_ratio: 1.0,
                    duration_ms: 0,
                    merkle_comparisons: 1,
                    bloom_hits: 0,
                    bloom_misses: 0,
                },
            }));
        }
        
        // Request merkle nodes for differences
        Ok(Some(SyncMessage::MerkleRequest {
            session_id,
            node_paths: differences,
        }))
    }
    
    /// Handle merkle tree node request
    fn handle_merkle_request(&mut self, session_id: u64, node_paths: Vec<Vec<usize>>) -> Result<Option<SyncMessage>> {
        // Collect the requested merkle nodes
        let mut nodes = Vec::new();
        
        for path in node_paths {
            if let Some(node) = self.merkle_tree.get_node_at_path(&path) {
                nodes.push((path.clone(), node));
            }
        }
        
        Ok(Some(SyncMessage::MerkleResponse {
            session_id,
            nodes,
        }))
    }
    
    /// Handle merkle tree node response
    fn handle_merkle_response(&mut self, session_id: u64, nodes: Vec<(Vec<usize>, MerkleNode)>) -> Result<Option<SyncMessage>> {
        // Analyze received nodes to determine which game states we need
        let mut needed_game_ids = Vec::new();
        
        for (_path, node) in nodes {
            // Check if we have this node locally
            let have_locally = self.local_states.values()
                .any(|state| state.state_hash == node.hash);
            
            if !have_locally && node.game_ids.len() > 0 {
                needed_game_ids.extend(node.game_ids);
            }
        }
        
        if needed_game_ids.is_empty() {
            return Ok(Some(SyncMessage::SyncComplete {
                session_id,
                stats: SyncStats {
                    states_synced: 0,
                    bytes_transferred: 0,
                    compression_ratio: 1.0,
                    duration_ms: 0,
                    merkle_comparisons: 1,
                    bloom_hits: 0,
                    bloom_misses: 0,
                },
            }));
        }
        
        Ok(Some(SyncMessage::StateRequest {
            session_id,
            game_ids: needed_game_ids,
        }))
    }
    
    /// Handle state request
    fn handle_state_request(&mut self, session_id: u64, game_ids: Vec<GameId>) -> Result<Option<SyncMessage>> {
        let states = game_ids
            .into_iter()
            .filter_map(|game_id| self.create_game_history(game_id))
            .collect();
        
        Ok(Some(SyncMessage::StateResponse {
            session_id,
            states,
        }))
    }
    
    /// Create a CompactGameHistory from a local state node
    fn create_game_history(&self, game_id: GameId) -> Option<CompactGameHistory> {
        let state_node = self.local_states.get(&game_id)?;
        
        Some(CompactGameHistory {
            game_id,
            initial_state: self.create_compressed_state(game_id, state_node),
            delta_chain: Vec::new(),
            final_summary: self.create_game_summary(),
            timestamps: self.create_time_range(state_node.timestamp),
            estimated_size: state_node.size_bytes,
        })
    }
    
    /// Create compressed game state
    fn create_compressed_state(&self, game_id: GameId, state_node: &GameStateNode) -> crate::protocol::efficient_history::CompressedGameState {
        crate::protocol::efficient_history::CompressedGameState {
            compressed_data: vec![], // Would compress actual state
            original_size: state_node.size_bytes,
            compressed_size: (state_node.size_bytes as f64 * 0.4) as u32,
            game_id,
            phase: 0,
            player_count: 1,
        }
    }
    
    /// Create default game summary
    fn create_game_summary(&self) -> crate::protocol::efficient_history::GameSummary {
        crate::protocol::efficient_history::GameSummary {
            total_rolls: 0,
            final_balances: std::collections::HashMap::new(),
            duration_secs: 0,
            player_count: 1,
            total_wagered: 0,
            house_edge: 0.0,
        }
    }
    
    /// Create time range from timestamp
    fn create_time_range(&self, timestamp: u64) -> crate::protocol::efficient_history::TimeRange {
        crate::protocol::efficient_history::TimeRange {
            start_time: timestamp,
            end_time: timestamp,
            last_activity: timestamp,
        }
    }
    
    /// Handle state response
    fn handle_state_response(&mut self, session_id: u64, states: Vec<CompactGameHistory>) -> Result<Option<SyncMessage>> {
        let states_count = states.len() as u32;
        
        // Process received states
        for _state in &states {
            // Would decompress and integrate the state
            // For now, just count it
        }
        
        Ok(Some(SyncMessage::SyncComplete {
            session_id,
            stats: SyncStats {
                states_synced: states_count,
                bytes_transferred: states.len() as u64 * 1024, // Estimate
                compression_ratio: 0.5,
                duration_ms: {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    
                    // Find the session for timing calculation
                    if let Some(session) = self.active_syncs.values().find(|s| s.session_id == session_id) {
                        now.saturating_sub(session.started_at) * 1000
                    } else {
                        100 // Default if session not found
                    }
                },
                merkle_comparisons: 1,
                bloom_hits: 0,
                bloom_misses: 0,
            },
        }))
    }
    
    /// Handle binary diff update
    fn handle_diff_update(
        &mut self, 
        _session_id: u64, 
        _game_id: GameId, 
        _diff: BinaryDiff, 
        _base_hash: Hash256
    ) -> Result<Option<SyncMessage>> {
        // Would apply binary diff to update state
        Ok(None)
    }
    
    /// Handle sync completion
    fn handle_sync_complete(&mut self, session_id: u64, stats: SyncStats) -> Result<Option<SyncMessage>> {
        // Find and remove the sync session
        if let Some(session) = self.active_syncs.values().find(|s| s.session_id == session_id) {
            let peer = session.peer;
            self.active_syncs.remove(&peer);
        }
        
        // Update metrics
        self.metrics.total_syncs_completed += 1;
        self.update_sync_metrics(&stats);
        
        Ok(None)
    }
    
    /// Handle sync error
    fn handle_sync_error(&mut self, session_id: u64, _error: String) -> Result<Option<SyncMessage>> {
        // Find and remove the sync session
        if let Some(session) = self.active_syncs.values().find(|s| s.session_id == session_id) {
            let peer = session.peer;
            self.active_syncs.remove(&peer);
        }
        
        self.metrics.total_syncs_failed += 1;
        Ok(None)
    }
    
    /// Calculate hash of game state
    fn calculate_state_hash(&self, state: &CompactGameState) -> Result<Hash256> {
        let serialized = bincode::serialize(state)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        Ok(hasher.finalize().into())
    }
    
    /// Serialize bloom filter for transmission
    fn serialize_bloom_filter(&self) -> Result<Vec<u8>> {
        // Would serialize the actual bloom filter
        Ok(vec![1, 2, 3, 4]) // Placeholder
    }
    
    /// Generate unique session ID
    fn generate_session_id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        hasher.finish()
    }
    
    /// Update sync metrics
    fn update_sync_metrics(&mut self, stats: &SyncStats) {
        self.metrics.total_bytes_transferred += stats.bytes_transferred;
        
        // Update running averages
        let total_completed = self.metrics.total_syncs_completed as f64;
        if total_completed > 0.0 {
            self.metrics.average_sync_time_ms = 
                (self.metrics.average_sync_time_ms * (total_completed - 1.0) + stats.duration_ms as f64) / total_completed;
            
            self.metrics.average_compression_ratio =
                (self.metrics.average_compression_ratio * (total_completed - 1.0) + stats.compression_ratio) / total_completed as f32;
        }
    }
    
    /// Get current sync metrics
    pub fn get_metrics(&self) -> SyncMetrics {
        self.metrics.clone()
    }
    
    /// Cleanup timed out sync sessions
    pub fn cleanup_expired_sessions(&mut self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        self.active_syncs.retain(|_, session| {
            current_time - session.started_at <= self.config.sync_timeout_secs
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::efficient_game_state::CompactGameState;

    #[test]
    fn test_state_merkle_tree() {
        let mut tree = StateMerkleTree::new();
        
        let game_id1 = [1u8; 16];
        let game_id2 = [2u8; 16];
        
        let state1 = CompactGameState::new(game_id1, [1u8; 32]);
        let state2 = CompactGameState::new(game_id2, [2u8; 32]);
        
        let node1 = GameStateNode {
            state: state1,
            state_hash: [1u8; 32],
            parent_hash: None,
            sequence: 1,
            timestamp: 1000,
            size_bytes: 1024,
        };
        
        let node2 = GameStateNode {
            state: state2,
            state_hash: [2u8; 32],
            parent_hash: None,
            sequence: 2,
            timestamp: 2000,
            size_bytes: 1024,
        };
        
        // Add states to tree
        tree.update_state(game_id1, &node1).unwrap();
        tree.update_state(game_id2, &node2).unwrap();
        
        // Root should be non-zero after adding states
        assert_ne!(tree.root_hash(), [0u8; 32]);
        
        // Should be able to generate proof
        let proof = tree.get_proof(game_id1);
        assert!(proof.is_some());
    }
    
    #[test]
    fn test_binary_diff_engine() {
        let mut engine = BinaryDiffEngine::new();
        
        let source = b"Hello, World!";
        let target = b"Hello, Universe!";
        
        let diff = engine.create_diff(source, target).unwrap();
        assert_eq!(diff.target_checksum, engine.hash_data(target));
        
        let reconstructed = engine.apply_diff(source, &diff).unwrap();
        assert_eq!(reconstructed, target);
    }
    
    #[test]
    fn test_efficient_state_sync() {
        let config = SyncConfig::default();
        let mut sync = EfficientStateSync::new(config);
        
        let game_id = [1u8; 16];
        let state = CompactGameState::new(game_id, [1u8; 32]);
        
        // Add local state
        sync.update_local_state(game_id, state).unwrap();
        
        // Check that merkle tree was updated
        assert_ne!(sync.merkle_tree.root_hash(), [0u8; 32]);
        
        // Initiate sync
        let peer = [2u8; 32];
        let sync_request = sync.initiate_sync(peer).unwrap();
        
        match sync_request {
            SyncMessage::SyncRequest { session_id, local_root_hash, .. } => {
                assert_ne!(session_id, 0);
                assert_ne!(local_root_hash, [0u8; 32]);
            },
            _ => panic!("Expected SyncRequest"),
        }
    }
    
    #[test]
    fn test_sync_message_processing() {
        let config = SyncConfig::default();
        let mut sync = EfficientStateSync::new(config);
        
        let session_id = 12345;
        let remote_root = [42u8; 32];
        let bloom_data = vec![1, 2, 3, 4];
        
        let request = SyncMessage::SyncRequest {
            session_id,
            local_root_hash: remote_root,
            bloom_filter_data: bloom_data,
        };
        
        let response = sync.process_sync_message(request).unwrap();
        assert!(response.is_some());
        
        match response.unwrap() {
            SyncMessage::SyncResponse { session_id: resp_session, accepted, .. } => {
                assert_eq!(resp_session, session_id);
                assert!(accepted);
            },
            SyncMessage::SyncComplete { .. } => {
                // Also valid if trees are identical
            },
            _ => panic!("Expected SyncResponse or SyncComplete"),
        }
    }
    
    #[test]
    fn test_sync_metrics() {
        let config = SyncConfig::default();
        let mut sync = EfficientStateSync::new(config);
        
        let initial_metrics = sync.get_metrics();
        assert_eq!(initial_metrics.total_syncs_initiated, 0);
        
        // Initiate a sync
        let peer = [1u8; 32];
        let _request = sync.initiate_sync(peer).unwrap();
        
        let updated_metrics = sync.get_metrics();
        assert_eq!(updated_metrics.total_syncs_initiated, 1);
    }
    
    #[test]
    fn test_session_cleanup() {
        let mut config = SyncConfig::default();
        config.sync_timeout_secs = 1; // Very short timeout for testing
        
        let mut sync = EfficientStateSync::new(config);
        
        let peer = [1u8; 32];
        let _request = sync.initiate_sync(peer).unwrap();
        
        // Should have an active session
        assert_eq!(sync.active_syncs.len(), 1);
        
        // Wait for timeout
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        // Cleanup should remove expired session
        sync.cleanup_expired_sessions();
        assert_eq!(sync.active_syncs.len(), 0);
    }
    
    #[test]
    fn test_diff_engine_caching() {
        let mut engine = BinaryDiffEngine::new();
        
        let source = b"Hello, World!";
        let target = b"Hello, Universe!";
        
        // First diff should be a cache miss
        let _diff1 = engine.create_diff(source, target).unwrap();
        let stats1 = engine.get_stats();
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);
        
        // Second diff should be a cache hit
        let _diff2 = engine.create_diff(source, target).unwrap();
        let stats2 = engine.get_stats();
        assert_eq!(stats2.cache_hits, 1);
    }
    
    #[test]
    fn test_merkle_proof_generation() {
        let mut tree = StateMerkleTree::new();
        
        let game_id = [1u8; 16];
        let state = CompactGameState::new(game_id, [1u8; 32]);
        let node = GameStateNode {
            state,
            state_hash: [1u8; 32],
            parent_hash: None,
            sequence: 1,
            timestamp: 1000,
            size_bytes: 1024,
        };
        
        tree.update_state(game_id, &node).unwrap();
        
        let proof = tree.get_proof(game_id);
        assert!(proof.is_some());
        
        let proof = proof.unwrap();
        assert_eq!(proof.leaf_index, 0); // First (and only) leaf
    }
}