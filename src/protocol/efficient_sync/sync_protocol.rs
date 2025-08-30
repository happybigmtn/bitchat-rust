//! Sync messages and protocol definitions

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::protocol::efficient_history::CompactGameHistory;
use crate::protocol::{GameId, Hash256, PeerId};

use super::diff_engine::BinaryDiff;
use super::merkle::MerkleNode;
use super::CompressionStats;

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
    SyncComplete { session_id: u64, stats: SyncStats },

    /// Error during sync
    SyncError { session_id: u64, error: String },
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

impl SyncSession {
    /// Create new sync session
    pub fn new(peer: PeerId, session_id: u64) -> Self {
        Self {
            peer,
            session_id,
            games_in_sync: HashSet::new(),
            phase: SyncPhase::BloomFilterExchange,
            needed_states: Vec::new(),
            available_states: Vec::new(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            bytes_transferred: 0,
            compression_stats: CompressionStats::default(),
        }
    }

    /// Update session progress
    pub fn update_progress(&mut self, phase: SyncPhase, bytes_transferred: u64) {
        self.phase = phase;
        self.bytes_transferred += bytes_transferred;
    }

    /// Check if session is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.phase, SyncPhase::Complete | SyncPhase::Failed(_))
    }

    /// Get session duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (now - self.started_at) * 1000
    }
}

impl SyncMessage {
    /// Get session ID from message
    pub fn session_id(&self) -> u64 {
        match self {
            SyncMessage::SyncRequest { session_id, .. }
            | SyncMessage::SyncResponse { session_id, .. }
            | SyncMessage::MerkleRequest { session_id, .. }
            | SyncMessage::MerkleResponse { session_id, .. }
            | SyncMessage::StateRequest { session_id, .. }
            | SyncMessage::StateResponse { session_id, .. }
            | SyncMessage::DiffUpdate { session_id, .. }
            | SyncMessage::SyncComplete { session_id, .. }
            | SyncMessage::SyncError { session_id, .. } => *session_id,
        }
    }

    /// Check if message is an error
    pub fn is_error(&self) -> bool {
        matches!(self, SyncMessage::SyncError { .. })
    }

    /// Get error message if this is an error message
    pub fn error_message(&self) -> Option<&str> {
        match self {
            SyncMessage::SyncError { error, .. } => Some(error),
            _ => None,
        }
    }
}
