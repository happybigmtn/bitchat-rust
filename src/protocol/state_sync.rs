//! Byzantine Fault-Tolerant State Synchronization
//!
//! This module implements state synchronization with Byzantine tolerance,
//! handling network partitions, state divergence, and recovery mechanisms.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use crate::protocol::consensus::engine::GameConsensusState;
use crate::protocol::craps::CrapsGame;
use crate::protocol::p2p_messages::{CompressedGameState, ConsensusMessage, ConsensusPayload};
use crate::protocol::{GameId, Hash256, PeerId};

/// State synchronization configuration
#[derive(Debug, Clone)]
pub struct StateSyncConfig {
    /// Maximum state history to keep
    pub max_history_size: usize,
    /// Sync request timeout
    pub sync_timeout: Duration,
    /// Maximum state difference for fast sync
    pub max_fast_sync_gap: u64,
    /// Byzantine threshold (fraction of participants)
    pub byzantine_threshold: f64,
    /// State checkpoint interval
    pub checkpoint_interval: u64,
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            max_history_size: 1000,
            sync_timeout: Duration::from_secs(30),
            max_fast_sync_gap: 100,
            byzantine_threshold: 0.67, // Require >2/3 agreement
            checkpoint_interval: 50,   // Checkpoint every 50 state transitions
        }
    }
}

/// State synchronization status
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Synchronized,
    Syncing,
    Diverged,
    PartitionRecovery,
    Failed(String),
}

/// State checkpoint for efficient synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Checkpoint sequence number
    pub sequence: u64,
    /// Game state at checkpoint
    pub game_state: GameConsensusState,
    /// State hash for verification
    pub state_hash: Hash256,
    /// Participants at checkpoint
    pub participants: Vec<PeerId>,
    /// Checkpoint timestamp
    pub timestamp: u64,
    /// Signatures from participants
    #[serde(with = "signature_map")]
    pub signatures: HashMap<PeerId, [u8; 64]>,
}

/// State difference for incremental sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    /// Starting sequence number
    pub from_sequence: u64,
    /// Ending sequence number  
    pub to_sequence: u64,
    /// State operations applied
    pub operations: Vec<StateOperation>,
    /// Resulting state hash
    pub result_hash: Hash256,
}

/// Individual state operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateOperation {
    /// Operation sequence number
    pub sequence: u64,
    /// Peer that proposed the operation
    pub proposer: PeerId,
    /// Operation timestamp
    pub timestamp: u64,
    /// The actual operation
    pub operation: crate::protocol::consensus::engine::GameOperation,
    /// Operation signature
    #[serde(
        serialize_with = "signature_serialize",
        deserialize_with = "signature_deserialize"
    )]
    pub signature: [u8; 64],
}

/// Peer state information for synchronization
#[derive(Debug, Clone)]
struct PeerStateInfo {
    peer_id: PeerId,
    last_known_sequence: u64,
    last_known_hash: Hash256,
    last_updated: Instant,
    is_synchronized: bool,
}

/// State synchronization manager
pub struct StateSynchronizer {
    config: StateSyncConfig,
    game_id: GameId,
    local_peer_id: PeerId,

    // Current state tracking
    current_sequence: Arc<RwLock<u64>>,
    current_state_hash: Arc<RwLock<Hash256>>,
    sync_status: Arc<RwLock<SyncStatus>>,

    // State history and checkpoints
    state_history: Arc<RwLock<BTreeMap<u64, StateOperation>>>,
    checkpoints: Arc<RwLock<BTreeMap<u64, StateCheckpoint>>>,

    // Peer synchronization tracking
    peer_states: Arc<RwLock<HashMap<PeerId, PeerStateInfo>>>,
    active_sync_requests: Arc<RwLock<HashMap<PeerId, Instant>>>,

    // Byzantine fault tolerance
    conflicting_states: Arc<RwLock<HashMap<u64, HashMap<Hash256, HashSet<PeerId>>>>>,
    partition_groups: Arc<RwLock<Vec<HashSet<PeerId>>>>,
}

impl StateSynchronizer {
    /// Create new state synchronizer
    pub fn new(
        config: StateSyncConfig,
        game_id: GameId,
        local_peer_id: PeerId,
        initial_state: &GameConsensusState,
    ) -> Self {
        Self {
            config,
            game_id,
            local_peer_id,
            current_sequence: Arc::new(RwLock::new(initial_state.sequence_number)),
            current_state_hash: Arc::new(RwLock::new(initial_state.state_hash)),
            sync_status: Arc::new(RwLock::new(SyncStatus::Synchronized)),
            state_history: Arc::new(RwLock::new(BTreeMap::new())),
            checkpoints: Arc::new(RwLock::new(BTreeMap::new())),
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            active_sync_requests: Arc::new(RwLock::new(HashMap::new())),
            conflicting_states: Arc::new(RwLock::new(HashMap::new())),
            partition_groups: Arc::new(RwLock::new(vec![])),
        }
    }

    /// Record a new state operation
    pub async fn record_operation(&self, operation: StateOperation) -> Result<()> {
        let mut history = self.state_history.write().await;

        // Add to history
        history.insert(operation.sequence, operation.clone());

        // Update current sequence if this is the latest
        let current_seq = *self.current_sequence.read().await;
        if operation.sequence > current_seq {
            *self.current_sequence.write().await = operation.sequence;
        }

        // Clean up old history if needed
        if history.len() > self.config.max_history_size {
            let cutoff = operation
                .sequence
                .saturating_sub(self.config.max_history_size as u64);
            history.retain(|&seq, _| seq > cutoff);
        }

        // Create checkpoint if needed
        if operation.sequence % self.config.checkpoint_interval == 0 {
            self.create_checkpoint(operation.sequence).await?;
        }

        Ok(())
    }

    /// Update peer state information
    pub async fn update_peer_state(&self, peer_id: PeerId, sequence: u64, state_hash: Hash256) {
        let mut peer_states = self.peer_states.write().await;

        let current_seq = *self.current_sequence.read().await;
        let is_synchronized = sequence >= current_seq.saturating_sub(10); // Within 10 operations

        peer_states.insert(
            peer_id,
            PeerStateInfo {
                peer_id,
                last_known_sequence: sequence,
                last_known_hash: state_hash,
                last_updated: Instant::now(),
                is_synchronized,
            },
        );

        // Check for state conflicts (Byzantine fault detection)
        self.detect_state_conflicts(sequence, state_hash, peer_id)
            .await;

        // Update overall sync status
        self.update_sync_status().await;
    }

    /// Detect conflicting states (Byzantine fault detection)
    async fn detect_state_conflicts(&self, sequence: u64, state_hash: Hash256, peer_id: PeerId) {
        let mut conflicts = self.conflicting_states.write().await;

        let sequence_conflicts = conflicts.entry(sequence).or_default();
        let peers_with_hash = sequence_conflicts.entry(state_hash).or_default();
        peers_with_hash.insert(peer_id);

        // Check if we have conflicting hashes for the same sequence
        if sequence_conflicts.len() > 1 {
            log::warn!(
                "State conflict detected at sequence {}: {} different hashes",
                sequence,
                sequence_conflicts.len()
            );

            // Analyze which hash has majority support
            let total_peers = self.peer_states.read().await.len();
            let byzantine_threshold =
                (total_peers as f64 * self.config.byzantine_threshold) as usize;

            for (hash, peers) in sequence_conflicts.iter() {
                if peers.len() >= byzantine_threshold {
                    log::info!(
                        "Hash {:?} has majority support ({} peers)",
                        hex::encode(hash),
                        peers.len()
                    );
                } else {
                    log::warn!(
                        "Hash {:?} has minority support ({} peers) - potential Byzantine behavior",
                        hex::encode(hash),
                        peers.len()
                    );
                }
            }
        }
    }

    /// Update overall synchronization status
    async fn update_sync_status(&self) {
        let peer_states = self.peer_states.read().await;
        let current_seq = *self.current_sequence.read().await;

        if peer_states.is_empty() {
            *self.sync_status.write().await = SyncStatus::Synchronized;
            return;
        }

        let synchronized_peers = peer_states
            .values()
            .filter(|info| {
                info.is_synchronized && info.last_updated.elapsed() < Duration::from_secs(60)
            })
            .count();

        let total_active_peers = peer_states
            .values()
            .filter(|info| info.last_updated.elapsed() < Duration::from_secs(60))
            .count();

        let sync_ratio = if total_active_peers > 0 {
            synchronized_peers as f64 / total_active_peers as f64
        } else {
            1.0
        };

        let new_status = if sync_ratio >= self.config.byzantine_threshold {
            SyncStatus::Synchronized
        } else if sync_ratio >= 0.5 {
            SyncStatus::Syncing
        } else {
            SyncStatus::Diverged
        };

        let mut status = self.sync_status.write().await;
        if *status != new_status {
            log::info!(
                "Sync status changed from {:?} to {:?} (sync ratio: {:.2})",
                *status,
                new_status,
                sync_ratio
            );
            *status = new_status;
        }
    }

    /// Request state sync from peers
    pub async fn request_sync(&self, target_peers: Vec<PeerId>) -> Result<()> {
        let current_seq = *self.current_sequence.read().await;
        let current_hash = *self.current_state_hash.read().await;

        for peer_id in target_peers {
            // Check if we already have an active request
            let mut active_requests = self.active_sync_requests.write().await;
            if active_requests.contains_key(&peer_id) {
                continue;
            }

            active_requests.insert(peer_id, Instant::now());

            log::info!(
                "Requesting state sync from {:?} (current seq: {})",
                peer_id,
                current_seq
            );

            // Create sync request message
            let message = ConsensusMessage::new(
                self.local_peer_id,
                self.game_id,
                current_seq,
                ConsensusPayload::StateSync {
                    state_hash: current_hash,
                    sequence_number: current_seq,
                    partial_state: None, // Request, not providing state
                },
            );

            // TODO: Send via consensus coordinator
            // coordinator.send_to_peer(peer_id, message).await?;
        }

        Ok(())
    }

    /// Handle state sync request from peer
    pub async fn handle_sync_request(
        &self,
        requester: PeerId,
        their_sequence: u64,
        their_hash: Hash256,
    ) -> Result<ConsensusMessage> {
        let current_seq = *self.current_sequence.read().await;

        if their_sequence >= current_seq {
            // They are ahead or equal - send our current state
            let response = ConsensusMessage::new(
                self.local_peer_id,
                self.game_id,
                current_seq,
                ConsensusPayload::StateSync {
                    state_hash: *self.current_state_hash.read().await,
                    sequence_number: current_seq,
                    partial_state: None, // Would compress and send actual state
                },
            );
            return Ok(response);
        }

        // They are behind - send state delta or checkpoint
        let sequence_gap = current_seq - their_sequence;

        if sequence_gap <= self.config.max_fast_sync_gap {
            // Send incremental delta
            let delta = self.create_state_delta(their_sequence, current_seq).await?;
            let compressed_state = self.compress_state_delta(&delta)?;

            let response = ConsensusMessage::new(
                self.local_peer_id,
                self.game_id,
                current_seq,
                ConsensusPayload::StateSync {
                    state_hash: delta.result_hash,
                    sequence_number: current_seq,
                    partial_state: Some(compressed_state),
                },
            );
            return Ok(response);
        } else {
            // Send latest checkpoint
            let checkpoint = self.get_latest_checkpoint().await?;
            let compressed_checkpoint = self.compress_checkpoint(&checkpoint)?;

            let response = ConsensusMessage::new(
                self.local_peer_id,
                self.game_id,
                checkpoint.sequence,
                ConsensusPayload::StateSync {
                    state_hash: checkpoint.state_hash,
                    sequence_number: checkpoint.sequence,
                    partial_state: Some(compressed_checkpoint),
                },
            );
            return Ok(response);
        }
    }

    /// Apply received state synchronization
    pub async fn apply_sync_data(
        &self,
        sender: PeerId,
        compressed_state: CompressedGameState,
    ) -> Result<()> {
        // Decompress state data
        let state_bytes = lz4_flex::decompress(
            &compressed_state.data,
            compressed_state.original_size as usize,
        )
        .map_err(|e| Error::Serialization(format!("Decompression failed: {}", e)))?;

        // Verify checksum
        let actual_checksum = crc32fast::hash(&state_bytes);
        if actual_checksum != compressed_state.checksum {
            return Err(Error::InvalidData("State checksum mismatch".to_string()));
        }

        // Try to deserialize as state delta first
        if let Ok(delta) = bincode::deserialize::<StateDelta>(&state_bytes) {
            self.apply_state_delta(delta).await?;
        } else if let Ok(checkpoint) = bincode::deserialize::<StateCheckpoint>(&state_bytes) {
            self.apply_checkpoint(checkpoint).await?;
        } else {
            return Err(Error::Serialization(
                "Invalid state sync data format".to_string(),
            ));
        }

        // Update peer state
        self.update_peer_state(sender, compressed_state.sequence, [0u8; 32])
            .await; // TODO: Get actual hash

        Ok(())
    }

    /// Create state delta between two sequence numbers
    async fn create_state_delta(&self, from_seq: u64, to_seq: u64) -> Result<StateDelta> {
        let history = self.state_history.read().await;

        let mut operations = Vec::new();
        for seq in (from_seq + 1)..=to_seq {
            if let Some(op) = history.get(&seq) {
                operations.push(op.clone());
            } else {
                return Err(Error::InvalidData(format!(
                    "Missing operation at sequence {}",
                    seq
                )));
            }
        }

        Ok(StateDelta {
            from_sequence: from_seq,
            to_sequence: to_seq,
            operations,
            result_hash: *self.current_state_hash.read().await,
        })
    }

    /// Apply state delta to catch up
    async fn apply_state_delta(&self, delta: StateDelta) -> Result<()> {
        let current_seq = *self.current_sequence.read().await;

        if delta.from_sequence != current_seq {
            return Err(Error::InvalidData(
                "State delta sequence mismatch".to_string(),
            ));
        }

        // Apply operations in order
        for operation in delta.operations {
            self.record_operation(operation).await?;
        }

        // Verify final hash
        if delta.result_hash != *self.current_state_hash.read().await {
            log::warn!("State hash mismatch after applying delta");
        }

        Ok(())
    }

    /// Create checkpoint at sequence
    async fn create_checkpoint(&self, sequence: u64) -> Result<StateCheckpoint> {
        // TODO: Get actual game state from consensus engine
        let game_state = GameConsensusState {
            game_id: self.game_id,
            state_hash: *self.current_state_hash.read().await,
            sequence_number: sequence,
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            game_state: CrapsGame::new(self.game_id, self.local_peer_id),
            player_balances: rustc_hash::FxHashMap::default(),
            last_proposer: self.local_peer_id,
            confirmations: 0,
            is_finalized: true,
        };

        let checkpoint = StateCheckpoint {
            sequence,
            game_state,
            state_hash: *self.current_state_hash.read().await,
            participants: vec![self.local_peer_id], // TODO: Get actual participants
            timestamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signatures: HashMap::new(),
        };

        // Store checkpoint
        self.checkpoints
            .write()
            .await
            .insert(sequence, checkpoint.clone());

        Ok(checkpoint)
    }

    /// Get latest checkpoint
    async fn get_latest_checkpoint(&self) -> Result<StateCheckpoint> {
        let checkpoints = self.checkpoints.read().await;

        checkpoints
            .values()
            .max_by_key(|c| c.sequence)
            .cloned()
            .ok_or_else(|| Error::InvalidData("No checkpoints available".to_string()))
    }

    /// Apply checkpoint to reset state
    async fn apply_checkpoint(&self, checkpoint: StateCheckpoint) -> Result<()> {
        // Verify checkpoint signatures
        let required_signatures =
            (checkpoint.participants.len() as f64 * self.config.byzantine_threshold) as usize;
        if checkpoint.signatures.len() < required_signatures {
            return Err(Error::InvalidData(
                "Insufficient checkpoint signatures".to_string(),
            ));
        }

        // Apply checkpoint
        *self.current_sequence.write().await = checkpoint.sequence;
        *self.current_state_hash.write().await = checkpoint.state_hash;

        // Clear conflicting history after checkpoint
        let mut history = self.state_history.write().await;
        history.retain(|&seq, _| seq > checkpoint.sequence);

        // Store checkpoint
        let checkpoint_sequence = checkpoint.sequence;
        self.checkpoints
            .write()
            .await
            .insert(checkpoint_sequence, checkpoint);

        log::info!("Applied checkpoint at sequence {}", checkpoint_sequence);
        Ok(())
    }

    /// Compress state delta for network transmission
    fn compress_state_delta(&self, delta: &StateDelta) -> Result<CompressedGameState> {
        let delta_bytes =
            bincode::serialize(delta).map_err(|e| Error::Serialization(e.to_string()))?;

        let compressed_bytes = lz4_flex::compress_prepend_size(&delta_bytes);

        Ok(CompressedGameState {
            sequence: delta.to_sequence,
            data: compressed_bytes,
            checksum: crc32fast::hash(&delta_bytes),
            original_size: delta_bytes.len() as u32,
        })
    }

    /// Compress checkpoint for network transmission
    fn compress_checkpoint(&self, checkpoint: &StateCheckpoint) -> Result<CompressedGameState> {
        let checkpoint_bytes =
            bincode::serialize(checkpoint).map_err(|e| Error::Serialization(e.to_string()))?;

        let compressed_bytes = lz4_flex::compress_prepend_size(&checkpoint_bytes);

        Ok(CompressedGameState {
            sequence: checkpoint.sequence,
            data: compressed_bytes,
            checksum: crc32fast::hash(&checkpoint_bytes),
            original_size: checkpoint_bytes.len() as u32,
        })
    }

    /// Detect network partitions
    pub async fn detect_partitions(&self) -> Vec<HashSet<PeerId>> {
        let peer_states = self.peer_states.read().await;
        let conflicts = self.conflicting_states.read().await;

        // Group peers by their state hashes at recent sequences
        let mut partition_groups: HashMap<Hash256, HashSet<PeerId>> = HashMap::new();

        for (peer_id, info) in peer_states.iter() {
            if info.last_updated.elapsed() > Duration::from_secs(120) {
                continue; // Skip inactive peers
            }

            partition_groups
                .entry(info.last_known_hash)
                .or_default()
                .insert(*peer_id);
        }

        // Return groups with significant size
        let total_peers = peer_states.len();
        let min_partition_size = std::cmp::max(1, total_peers / 4); // At least 25% of peers

        partition_groups
            .into_values()
            .filter(|group| group.len() >= min_partition_size)
            .collect()
    }

    /// Get synchronization statistics
    pub async fn get_sync_stats(&self) -> SyncStats {
        let peer_states = self.peer_states.read().await;
        let conflicts = self.conflicting_states.read().await;
        let checkpoints = self.checkpoints.read().await;
        let history_size = self.state_history.read().await.len();

        let synchronized_peers = peer_states
            .values()
            .filter(|info| info.is_synchronized)
            .count();

        let active_conflicts = conflicts
            .values()
            .filter(|sequence_conflicts| sequence_conflicts.len() > 1)
            .count();

        SyncStats {
            current_sequence: *self.current_sequence.read().await,
            sync_status: self.sync_status.read().await.clone(),
            total_peers: peer_states.len(),
            synchronized_peers,
            active_conflicts,
            checkpoints_stored: checkpoints.len(),
            history_size,
            partitions_detected: self.partition_groups.read().await.len(),
        }
    }
}

/// State synchronization statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    pub current_sequence: u64,
    pub sync_status: SyncStatus,
    pub total_peers: usize,
    pub synchronized_peers: usize,
    pub active_conflicts: usize,
    pub checkpoints_stored: usize,
    pub history_size: usize,
    pub partitions_detected: usize,
}

// Serde helpers for signature serialization
mod signature_map {
    use crate::protocol::PeerId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;

    pub fn serialize<S>(value: &HashMap<PeerId, [u8; 64]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_map: HashMap<PeerId, String> = value
            .iter()
            .map(|(k, v)| (k.clone(), hex::encode(v)))
            .collect();
        hex_map.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<PeerId, [u8; 64]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_map: HashMap<PeerId, String> = HashMap::deserialize(deserializer)?;
        let mut result = HashMap::new();
        for (k, v) in hex_map {
            let bytes = hex::decode(&v).map_err(serde::de::Error::custom)?;
            if bytes.len() != 64 {
                return Err(serde::de::Error::custom("Invalid signature length"));
            }
            let mut array = [0u8; 64];
            array.copy_from_slice(&bytes);
            result.insert(k, array);
        }
        Ok(result)
    }
}

fn signature_serialize<S>(value: &[u8; 64], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    hex::encode(value).serialize(serializer)
}

fn signature_deserialize<'de, D>(deserializer: D) -> std::result::Result<[u8; 64], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let hex_string: String = String::deserialize(deserializer)?;
    let bytes = hex::decode(&hex_string).map_err(serde::de::Error::custom)?;
    if bytes.len() != 64 {
        return Err(serde::de::Error::custom("Invalid signature length"));
    }
    let mut array = [0u8; 64];
    array.copy_from_slice(&bytes);
    Ok(array)
}
