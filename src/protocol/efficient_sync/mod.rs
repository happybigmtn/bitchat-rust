//! Fast game state synchronization with merkle-based sync and bloom filters
//! 
//! This module implements high-performance state synchronization using merkle trees
//! for efficient state verification, bloom filters for difference detection, and
//! binary diff algorithms for minimal data transfer.

pub mod merkle;
pub mod diff_engine;
pub mod sync_protocol;
pub mod state_manager;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, GameId, Hash256};
use crate::protocol::efficient_game_state::CompactGameState;
use crate::protocol::efficient_history::{CompactGameHistory, BloomFilter};
use crate::error::{Error, Result};

// Re-export main types
pub use merkle::{StateMerkleTree, MerkleNode, NodeMetadata, MerkleProof};
pub use diff_engine::{BinaryDiffEngine, BinaryDiff, DiffOperation, DiffStats};
pub use sync_protocol::{SyncMessage, SyncPhase, SyncSession};
pub use state_manager::{EfficientStateSync, GameStateNode};

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

/// Performance metrics for synchronization
#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    /// Total bytes synchronized
    pub bytes_synced: u64,
    
    /// Number of states synchronized
    pub states_synced: u32,
    
    /// Number of sync sessions completed
    pub sessions_completed: u32,
    
    /// Number of sync sessions failed
    pub sessions_failed: u32,
    
    /// Average sync time per session (milliseconds)
    pub avg_sync_time_ms: f64,
    
    /// Cache hit rate for diff operations
    pub diff_cache_hit_rate: f64,
    
    /// Compression efficiency
    pub compression_stats: CompressionStats,
}

/// Compression statistics
#[derive(Debug, Default, Clone)]
pub struct CompressionStats {
    /// Total bytes before compression
    pub original_bytes: u64,
    
    /// Total bytes after compression
    pub compressed_bytes: u64,
    
    /// Average compression ratio
    pub avg_ratio: f32,
}

/// Overall sync metrics
#[derive(Debug, Default, Clone)]
pub struct SyncMetrics {
    pub stats: SyncStats,
    pub last_updated: u64,
}