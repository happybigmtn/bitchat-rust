//! Network Consensus Bridge
//!
//! This module provides the bridge between the P2P consensus coordinator
//! and the local consensus engine, enabling distributed consensus over the mesh network.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::utils::AdaptiveInterval;
use crate::protocol::consensus::engine::{ConsensusEngine, GameConsensusState, GameOperation};
use crate::protocol::consensus::ProposalId;
use crate::protocol::consensus_coordinator::ConsensusCoordinator;
use crate::protocol::p2p_messages::{
    CompressedGameState, ConsensusMessage, ConsensusPayload, RoundId,
};
use crate::protocol::{BitchatPacket, GameId, PeerId, PACKET_TYPE_CONSENSUS_VOTE};

/// Configuration for the network consensus bridge
#[derive(Debug, Clone)]
pub struct NetworkConsensusBridgeConfig {
    /// How often to sync state with peers
    pub state_sync_interval: Duration,
    /// Timeout for waiting on consensus
    pub consensus_timeout: Duration,
    /// Maximum number of pending operations
    pub max_pending_operations: usize,
    /// State compression threshold (bytes)
    pub state_compression_threshold: usize,
}

impl Default for NetworkConsensusBridgeConfig {
    fn default() -> Self {
        Self {
            state_sync_interval: Duration::from_secs(10),
            consensus_timeout: Duration::from_secs(30),
            max_pending_operations: 100,
            state_compression_threshold: 1024,
        }
    }
}

/// Bridge connecting local consensus engine to network consensus coordinator
pub struct NetworkConsensusBridge {
    // Core components
    consensus_engine: Arc<Mutex<ConsensusEngine>>,
    consensus_coordinator: Arc<ConsensusCoordinator>,
    mesh_service: Arc<MeshService>,
    identity: Arc<BitchatIdentity>,

    // Configuration
    config: NetworkConsensusBridgeConfig,
    game_id: GameId,

    // State management
    current_round: Arc<RwLock<RoundId>>,
    pending_operations: Arc<RwLock<HashMap<ProposalId, PendingOperation>>>,

    // Event processing
    message_sender: mpsc::Sender<ConsensusMessage>,
    message_receiver: Arc<RwLock<mpsc::Receiver<ConsensusMessage>>>,

    // State tracking
    last_state_sync: Arc<RwLock<Instant>>,
    participants: Arc<RwLock<Vec<PeerId>>>,

    // Performance metrics
    messages_processed: Arc<RwLock<u64>>,
    consensus_rounds_completed: Arc<RwLock<u64>>,
    failed_operations: Arc<RwLock<u64>>,
}

/// Pending operation waiting for consensus
#[derive(Debug, Clone)]
struct PendingOperation {
    operation: GameOperation,
    proposal_id: ProposalId,
    submitted_at: Instant,
    votes_received: u32,
    required_votes: u32,
}

impl NetworkConsensusBridge {
    /// Create new network consensus bridge
    pub async fn new(
        consensus_engine: Arc<Mutex<ConsensusEngine>>,
        mesh_service: Arc<MeshService>,
        identity: Arc<BitchatIdentity>,
        game_id: GameId,
        participants: Vec<PeerId>,
    ) -> Result<Self> {
        // Create consensus coordinator
        let consensus_coordinator = Arc::new(
            ConsensusCoordinator::new(
                consensus_engine.clone(),
                mesh_service.clone(),
                identity.clone(),
                game_id,
                participants.clone(),
            )
            .await?,
        );

        let (message_sender, message_receiver) = mpsc::channel(10000); // High capacity for consensus messages

        Ok(Self {
            consensus_engine,
            consensus_coordinator,
            mesh_service,
            identity: identity.clone(),
            config: NetworkConsensusBridgeConfig::default(),
            game_id,
            current_round: Arc::new(RwLock::new(0)),
            pending_operations: Arc::new(RwLock::new(HashMap::new())),
            message_sender,
            message_receiver: Arc::new(RwLock::new(message_receiver)),
            last_state_sync: Arc::new(RwLock::new(Instant::now())),
            participants: Arc::new(RwLock::new(participants)),
            messages_processed: Arc::new(RwLock::new(0)),
            consensus_rounds_completed: Arc::new(RwLock::new(0)),
            failed_operations: Arc::new(RwLock::new(0)),
        })
    }

    /// Start the network consensus bridge
    pub async fn start(&self) -> Result<()> {
        log::info!(
            "Starting network consensus bridge for game {:?}",
            self.game_id
        );

        // Start consensus coordinator
        self.consensus_coordinator.start().await?;

        // Start bridge tasks
        self.start_message_processing_task().await;
        self.start_state_sync_task().await;
        self.start_mesh_message_handler().await;
        self.start_cleanup_task().await;

        log::info!("Network consensus bridge started successfully");
        Ok(())
    }

    /// Submit a game operation for distributed consensus
    pub async fn submit_operation(&self, operation: GameOperation) -> Result<ProposalId> {
        // Check pending operations limit
        let pending_count = self.pending_operations.read().await.len();
        if pending_count >= self.config.max_pending_operations {
            return Err(Error::Consensus("Too many pending operations".to_string()));
        }

        // Submit to consensus coordinator (which will handle network broadcast)
        let proposal_id = self
            .consensus_coordinator
            .submit_operation(operation.clone())
            .await?;

        // Track pending operation
        let participants_count = self.participants.read().await.len();
        let required_votes = (participants_count * 2) / 3 + 1; // Byzantine threshold

        let pending_op = PendingOperation {
            operation,
            proposal_id,
            submitted_at: Instant::now(),
            votes_received: 0,
            required_votes: required_votes as u32,
        };

        self.pending_operations
            .write()
            .await
            .insert(proposal_id, pending_op);

        log::debug!("Submitted operation {:?} for consensus", proposal_id);
        Ok(proposal_id)
    }

    /// Vote on a proposal (both locally and broadcast to network)
    pub async fn vote_on_proposal(&self, proposal_id: ProposalId, vote: bool) -> Result<()> {
        // Vote through consensus coordinator (handles local consensus engine + network broadcast)
        self.consensus_coordinator
            .vote_on_proposal(proposal_id, vote)
            .await?;

        // Update pending operation tracking
        if let Some(pending_op) = self.pending_operations.write().await.get_mut(&proposal_id) {
            pending_op.votes_received += 1;

            // Check if we have enough votes for consensus
            if pending_op.votes_received >= pending_op.required_votes {
                log::info!("Consensus reached for proposal {:?}", proposal_id);
                *self.consensus_rounds_completed.write().await += 1;
            }
        }

        Ok(())
    }

    /// Get current consensus state
    pub async fn get_current_state(&self) -> Result<GameConsensusState> {
        let consensus = self.consensus_engine.lock().await;
        Ok(consensus.get_current_state().clone())
    }

    /// Get network participants
    pub async fn get_participants(&self) -> Vec<PeerId> {
        self.participants.read().await.clone()
    }

    /// Add a new participant to consensus
    pub async fn add_participant(&self, participant: PeerId) -> Result<()> {
        // Add to consensus engine
        {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.add_participant(participant)?;
        }

        // Add to local participants list
        let mut participants = self.participants.write().await;
        if !participants.contains(&participant) {
            participants.push(participant);
        }

        log::info!("Added participant {:?} to consensus", participant);
        Ok(())
    }

    /// Remove a participant from consensus
    pub async fn remove_participant(&self, participant: PeerId) -> Result<()> {
        // Remove from consensus engine
        {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.remove_participant(participant)?;
        }

        // Remove from local participants list
        let mut participants = self.participants.write().await;
        participants.retain(|&p| p != participant);

        log::info!("Removed participant {:?} from consensus", participant);
        Ok(())
    }

    /// Start task to process consensus messages
    async fn start_message_processing_task(&self) {
        let message_receiver = self.message_receiver.clone();
        let consensus_coordinator = self.consensus_coordinator.clone();
        let messages_processed = self.messages_processed.clone();

        tokio::spawn(async move {
            let mut receiver = message_receiver.write().await;

            while let Some(message) = receiver.recv().await {
                // Process message through consensus coordinator
                if let Err(e) = consensus_coordinator.handle_message(message).await {
                    log::error!("Failed to process consensus message: {}", e);
                } else {
                    *messages_processed.write().await += 1;
                }
            }
        });
    }

    /// Start task to sync state with peers periodically
    async fn start_state_sync_task(&self) {
        let consensus_engine = self.consensus_engine.clone();
        let mesh_service = self.mesh_service.clone();
        let identity = self.identity.clone();
        let game_id = self.game_id;
        let current_round = self.current_round.clone();
        let last_state_sync = self.last_state_sync.clone();
        let sync_interval = self.config.state_sync_interval;

        tokio::spawn(async move {
            let mut sync_interval = interval(sync_interval);

            loop {
                sync_interval.tick().await;

                // Check if we need to sync state
                let last_sync = *last_state_sync.read().await;
                if last_sync.elapsed() < sync_interval.period() {
                    continue;
                }

                // Get current state from consensus engine
                let current_state = {
                    let consensus = consensus_engine.lock().await;
                    consensus.get_current_state().clone()
                };

                // Create state sync message
                let compressed_state = Self::compress_game_state(&current_state);

                let message = ConsensusMessage::new(
                    identity.peer_id,
                    game_id,
                    *current_round.read().await,
                    ConsensusPayload::StateSync {
                        state_hash: current_state.state_hash,
                        sequence_number: current_state.sequence_number,
                        partial_state: Some(compressed_state),
                    },
                );

                // Convert to BitchatPacket and broadcast
                if let Ok(packet) = Self::message_to_packet(message) {
                    if let Err(e) = mesh_service.broadcast_packet(packet).await {
                        log::error!("Failed to broadcast state sync: {}", e);
                    } else {
                        *last_state_sync.write().await = Instant::now();
                        log::debug!(
                            "Broadcasted state sync for sequence {}",
                            current_state.sequence_number
                        );
                    }
                }
            }
        });
    }

    /// Start task to handle messages from mesh service
    async fn start_mesh_message_handler(&self) {
        let mesh_service = self.mesh_service.clone();
        let message_sender = self.message_sender.clone();
        let identity = self.identity.clone();

        tokio::spawn(async move {
            // Use adaptive interval for mesh event handling
            // 100ms is acceptable for consensus operations, but can back off when idle
            let mut event_interval = AdaptiveInterval::for_consensus();

            loop {
                event_interval.tick().await;

                // In practice, this would receive actual mesh events
                // For now, we simulate by checking for packets periodically
                // The actual implementation would integrate with MeshService's event system
                
                // When real mesh events are processed, signal activity:
                // if mesh_events_processed {
                //     event_interval.signal_activity();
                // }
            }
        });
    }

    /// Start cleanup task for expired operations
    async fn start_cleanup_task(&self) {
        let pending_operations = self.pending_operations.clone();
        let failed_operations = self.failed_operations.clone();
        let timeout = self.config.consensus_timeout;

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(30));

            loop {
                cleanup_interval.tick().await;

                let mut operations = pending_operations.write().await;
                let mut failed_count = 0;

                // Remove expired operations
                operations.retain(|_id, op| {
                    if op.submitted_at.elapsed() > timeout {
                        failed_count += 1;
                        log::warn!(
                            "Operation {:?} timed out after {:?}",
                            op.proposal_id,
                            timeout
                        );
                        false
                    } else {
                        true
                    }
                });

                if failed_count > 0 {
                    *failed_operations.write().await += failed_count;
                }

                log::debug!(
                    "Cleanup: {} pending operations, {} failed",
                    operations.len(),
                    failed_count
                );
            }
        });
    }

    /// Compress game state for network transmission
    fn compress_game_state(state: &GameConsensusState) -> CompressedGameState {
        // Serialize state
        let serialized = bincode::serialize(state).unwrap_or_default();

        // Compress with LZ4
        let compressed_data = lz4_flex::compress_prepend_size(&serialized);

        CompressedGameState {
            sequence: state.sequence_number,
            data: compressed_data,
            checksum: crc32fast::hash(&serialized),
            original_size: serialized.len() as u32,
        }
    }

    /// Decompress game state from network
    fn decompress_game_state(compressed: &CompressedGameState) -> Result<GameConsensusState> {
        // Decompress data
        let decompressed = lz4_flex::decompress_size_prepended(&compressed.data)
            .map_err(|e| Error::Serialization(format!("Decompression failed: {}", e)))?;

        // Verify checksum
        let checksum = crc32fast::hash(&decompressed);
        if checksum != compressed.checksum {
            return Err(Error::Serialization("State checksum mismatch".to_string()));
        }

        // Deserialize
        bincode::deserialize(&decompressed)
            .map_err(|e| Error::Serialization(format!("Deserialization failed: {}", e)))
    }

    /// Convert consensus message to BitchatPacket
    fn message_to_packet(message: ConsensusMessage) -> Result<BitchatPacket> {
        let mut packet = BitchatPacket::new(PACKET_TYPE_CONSENSUS_VOTE);

        // Serialize message as payload
        let payload =
            bincode::serialize(&message).map_err(|e| Error::Serialization(e.to_string()))?;

        packet.payload = Some(payload);
        packet.source = message.sender;
        packet.target = [0u8; 32]; // Broadcast

        Ok(packet)
    }

    /// Convert BitchatPacket to consensus message
    fn packet_to_message(packet: &BitchatPacket) -> Result<ConsensusMessage> {
        if let Some(payload) = &packet.payload {
            bincode::deserialize(payload).map_err(|e| {
                Error::Serialization(format!("Failed to deserialize consensus message: {}", e))
            })
        } else {
            Err(Error::Protocol("Packet has no payload".to_string()))
        }
    }

    /// Handle incoming consensus message from network
    pub async fn handle_network_message(&self, packet: BitchatPacket) -> Result<()> {
        // Convert packet to consensus message
        let message = Self::packet_to_message(&packet)?;

        // Verify message is for our game
        if message.game_id != self.game_id {
            return Err(Error::Protocol("Message for wrong game".to_string()));
        }

        // Send to message processing task
        self.message_sender
            .send(message)
            .await
            .map_err(|e| Error::Network(format!("Failed to queue message: {}", e)))?;

        Ok(())
    }

    /// Get bridge statistics
    /// Sync consensus state
    pub async fn sync_state(&self, state: GameConsensusState) -> Result<()> {
        let mut consensus = self.consensus_engine.lock().await;
        consensus.sync_state(state)?;
        Ok(())
    }

    /// Get pending operations
    pub async fn get_pending_operations(&self) -> Result<Vec<GameOperation>> {
        let pending = self.pending_operations.read().await;
        Ok(pending.iter().map(|p| p.1.operation.clone()).collect())
    }

    pub async fn get_stats(&self) -> NetworkConsensusBridgeStats {
        let consensus_stats = self.consensus_coordinator.get_stats().await;

        NetworkConsensusBridgeStats {
            messages_processed: *self.messages_processed.read().await,
            consensus_rounds_completed: *self.consensus_rounds_completed.read().await,
            failed_operations: *self.failed_operations.read().await,
            pending_operations: self.pending_operations.read().await.len(),
            active_participants: self.participants.read().await.len(),
            coordinator_stats: consensus_stats,
        }
    }

    /// Check if consensus is healthy
    pub async fn is_consensus_healthy(&self) -> bool {
        let consensus = self.consensus_engine.lock().await;
        let is_engine_healthy = consensus.is_consensus_healthy();
        let pending_count = self.pending_operations.read().await.len();

        is_engine_healthy && pending_count < self.config.max_pending_operations
    }
}

/// Statistics for the network consensus bridge
#[derive(Debug, Clone)]
pub struct NetworkConsensusBridgeStats {
    pub messages_processed: u64,
    pub consensus_rounds_completed: u64,
    pub failed_operations: u64,
    pub pending_operations: usize,
    pub active_participants: usize,
    pub coordinator_stats: crate::protocol::consensus_coordinator::ConsensusStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use crate::protocol::consensus::ConsensusConfig;
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_network_consensus_bridge_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity.clone(), transport));

        let game_id = [1u8; 16];
        let participants = vec![identity.peer_id];

        let consensus_engine = Arc::new(Mutex::new(
            ConsensusEngine::new(
                game_id,
                participants.clone(),
                identity.peer_id,
                ConsensusConfig::default(),
            )
            .expect("Failed to create consensus engine"),
        ));

        let bridge = NetworkConsensusBridge::new(
            consensus_engine,
            mesh_service,
            identity,
            game_id,
            participants,
        )
        .await;

        assert!(bridge.is_ok());

        let bridge = bridge.unwrap();
        assert_eq!(bridge.game_id, game_id);
        assert_eq!(bridge.get_participants().await.len(), 1);
    }

    #[tokio::test]
    async fn test_state_compression_decompression() {
        use crate::protocol::craps::CrapsGame;
        use rustc_hash::FxHashMap;

        let game_id = [1u8; 16];
        let peer_id = [2u8; 32];

        let state = GameConsensusState {
            game_id,
            state_hash: [3u8; 32],
            sequence_number: 42,
            timestamp: 1234567890,
            game_state: CrapsGame::new(game_id, peer_id),
            player_balances: FxHashMap::default(),
            last_proposer: peer_id,
            confirmations: 5,
            is_finalized: true,
        };

        let compressed = NetworkConsensusBridge::compress_game_state(&state);
        let decompressed = NetworkConsensusBridge::decompress_game_state(&compressed)
            .expect("Failed to decompress state");

        assert_eq!(state.sequence_number, decompressed.sequence_number);
        assert_eq!(state.state_hash, decompressed.state_hash);
        assert_eq!(state.is_finalized, decompressed.is_finalized);
    }
}
