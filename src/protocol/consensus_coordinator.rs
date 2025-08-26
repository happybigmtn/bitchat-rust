//! Consensus Network Coordinator
//! 
//! This module provides the bridge between the ConsensusEngine and the mesh network,
//! handling message dispatch, state synchronization, and failure recovery.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::interval;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::protocol::{PeerId, GameId, BitchatPacket, PACKET_TYPE_CONSENSUS_VOTE};
use crate::protocol::consensus::engine::{ConsensusEngine, GameProposal, GameOperation};
use crate::protocol::consensus::ProposalId;
use crate::protocol::p2p_messages::{
    ConsensusMessage, ConsensusPayload, RoundId, MessagePriority, 
    NetworkView, CheatType
};
use crate::mesh::MeshService;
use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};

/// Configuration for consensus networking
#[derive(Debug, Clone)]
pub struct ConsensusNetworkConfig {
    /// Maximum message cache size for deduplication
    pub max_message_cache: usize,
    /// Heartbeat interval for liveness detection
    pub heartbeat_interval: Duration,
    /// Timeout for consensus rounds
    pub consensus_timeout: Duration,
    /// Maximum retries for failed messages
    pub max_retries: u32,
    /// BLE MTU size limit
    pub mtu_limit: usize,
    /// Leader election timeout
    pub leader_election_timeout: Duration,
    /// Partition recovery timeout
    pub partition_recovery_timeout: Duration,
}

impl Default for ConsensusNetworkConfig {
    fn default() -> Self {
        Self {
            max_message_cache: 10000,
            heartbeat_interval: Duration::from_secs(5),
            consensus_timeout: Duration::from_secs(30),
            max_retries: 3,
            mtu_limit: 244, // BLE MTU minus headers
            leader_election_timeout: Duration::from_secs(30),
            partition_recovery_timeout: Duration::from_secs(60),
        }
    }
}

/// Message retry tracking
#[derive(Debug, Clone)]
struct RetryInfo {
    message: ConsensusMessage,
    attempts: u32,
    last_attempt: Instant,
    target_peers: HashSet<PeerId>,
}

/// Network coordinator managing consensus over mesh network
pub struct ConsensusCoordinator {
    // Core components
    consensus_engine: Arc<Mutex<ConsensusEngine>>,
    mesh_service: Arc<MeshService>,
    identity: Arc<BitchatIdentity>,
    
    // Configuration
    config: ConsensusNetworkConfig,
    game_id: GameId,
    
    // Network state
    participants: Arc<RwLock<HashSet<PeerId>>>,
    current_leader: Arc<RwLock<Option<PeerId>>>,
    current_term: Arc<RwLock<u64>>,
    current_round: Arc<RwLock<RoundId>>,
    
    // Message handling
    message_cache: Arc<RwLock<LruCache<[u8; 32], Instant>>>,
    pending_messages: Arc<RwLock<VecDeque<ConsensusMessage>>>,
    retry_queue: Arc<RwLock<HashMap<[u8; 32], RetryInfo>>>,
    
    // Event channels
    inbound_messages: mpsc::UnboundedReceiver<ConsensusMessage>,
    outbound_sender: mpsc::UnboundedSender<ConsensusMessage>,
    
    // State tracking
    last_heartbeat: Arc<RwLock<Instant>>,
    partition_detected: Arc<RwLock<bool>>,
    network_view: Arc<RwLock<NetworkView>>,
    
    // Anti-cheat tracking
    suspicious_behavior: Arc<RwLock<HashMap<PeerId, Vec<CheatType>>>>,
    
    // Performance metrics
    messages_sent: Arc<RwLock<u64>>,
    messages_received: Arc<RwLock<u64>>,
    consensus_rounds_completed: Arc<RwLock<u64>>,
}

impl ConsensusCoordinator {
    /// Create new consensus coordinator
    pub async fn new(
        consensus_engine: Arc<Mutex<ConsensusEngine>>,
        mesh_service: Arc<MeshService>,
        identity: Arc<BitchatIdentity>,
        game_id: GameId,
        participants: Vec<PeerId>,
    ) -> Result<Self> {
        let (outbound_sender, inbound_messages) = mpsc::unbounded_channel();
        let config = ConsensusNetworkConfig::default();
        
        let coordinator = Self {
            consensus_engine,
            mesh_service,
            identity: identity.clone(),
            config: config.clone(),
            game_id,
            participants: Arc::new(RwLock::new(participants.into_iter().collect())),
            current_leader: Arc::new(RwLock::new(None)),
            current_term: Arc::new(RwLock::new(0)),
            current_round: Arc::new(RwLock::new(0)),
            message_cache: Arc::new(RwLock::new(
                LruCache::new(NonZeroUsize::new(config.max_message_cache).unwrap())
            )),
            pending_messages: Arc::new(RwLock::new(VecDeque::new())),
            retry_queue: Arc::new(RwLock::new(HashMap::new())),
            inbound_messages,
            outbound_sender,
            last_heartbeat: Arc::new(RwLock::new(Instant::now())),
            partition_detected: Arc::new(RwLock::new(false)),
            network_view: Arc::new(RwLock::new(NetworkView {
                participants: vec![identity.peer_id],
                connections: vec![],
                partition_id: None,
                leader: None,
            })),
            suspicious_behavior: Arc::new(RwLock::new(HashMap::new())),
            messages_sent: Arc::new(RwLock::new(0)),
            messages_received: Arc::new(RwLock::new(0)),
            consensus_rounds_completed: Arc::new(RwLock::new(0)),
        };
        
        Ok(coordinator)
    }
    
    /// Start the consensus coordinator
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting consensus coordinator for game {:?}", self.game_id);
        
        // Start background tasks
        self.start_heartbeat_task().await;
        self.start_message_processing_task().await;
        self.start_retry_task().await;
        self.start_leader_election_task().await;
        self.start_partition_detection_task().await;
        
        // Initialize network view
        self.update_network_view().await;
        
        Ok(())
    }
    
    /// Submit a game operation for consensus
    pub async fn submit_operation(&self, operation: GameOperation) -> Result<ProposalId> {
        let proposal_id = {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.submit_proposal(operation)?
        };
        
        // Get the proposal and broadcast it
        let proposal = {
            let consensus = self.consensus_engine.lock().await;
            consensus.get_pending_proposals()
                .get(&proposal_id)
                .cloned()
        };
        
        if let Some(proposal) = proposal {
            let message = ConsensusMessage::new(
                self.identity.peer_id,
                self.game_id,
                *self.current_round.read().await,
                ConsensusPayload::Proposal {
                    proposal,
                    priority: 1, // Normal priority
                },
            );
            
            self.broadcast_message(message).await?;
        }
        
        Ok(proposal_id)
    }
    
    /// Vote on a proposal
    pub async fn vote_on_proposal(&self, proposal_id: ProposalId, vote: bool) -> Result<()> {
        // Submit vote to consensus engine
        {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.vote_on_proposal(proposal_id, vote)?;
        }
        
        // Broadcast vote to network
        let message = ConsensusMessage::new(
            self.identity.peer_id,
            self.game_id,
            *self.current_round.read().await,
            ConsensusPayload::Vote {
                proposal_id,
                vote,
                reasoning: None,
            },
        );
        
        self.broadcast_message(message).await?;
        
        Ok(())
    }
    
    /// Handle incoming consensus message
    pub async fn handle_message(&self, message: ConsensusMessage) -> Result<()> {
        // Check for duplicate messages
        if self.is_duplicate_message(&message.message_id).await {
            log::debug!("Dropping duplicate message {:?}", message.message_id);
            return Ok(());
        }
        
        // Verify message is recent
        if !message.is_recent(300) { // 5 minutes
            log::warn!("Dropping old message from {:?}", message.sender);
            return Ok(());
        }
        
        // Verify sender is a participant
        if !self.participants.read().await.contains(&message.sender) {
            log::warn!("Dropping message from non-participant {:?}", message.sender);
            return Ok(());
        }
        
        // Add to cache and update metrics
        self.message_cache.write().await.put(message.message_id, Instant::now());
        *self.messages_received.write().await += 1;
        
        // Process message based on payload type
        match &message.payload {
            ConsensusPayload::Proposal { proposal, .. } => {
                self.handle_proposal(message.sender, proposal.clone()).await?;
            }
            
            ConsensusPayload::Vote { proposal_id, vote, .. } => {
                self.handle_vote(message.sender, *proposal_id, *vote, message.signature).await?;
            }
            
            ConsensusPayload::StateSync { .. } => {
                self.handle_state_sync(&message).await?;
            }
            
            ConsensusPayload::RandomnessCommit { round_id, commitment } => {
                self.handle_randomness_commit(*round_id, commitment.clone()).await?;
            }
            
            ConsensusPayload::RandomnessReveal { round_id, reveal } => {
                self.handle_randomness_reveal(*round_id, reveal.clone()).await?;
            }
            
            ConsensusPayload::DisputeClaim { dispute, .. } => {
                self.handle_dispute_claim(dispute.clone()).await?;
            }
            
            ConsensusPayload::Heartbeat { alive_participants, network_view } => {
                self.handle_heartbeat(message.sender, alive_participants.clone(), network_view.clone()).await?;
            }
            
            ConsensusPayload::LeaderProposal { proposed_leader, term, .. } => {
                self.handle_leader_proposal(message.sender, *proposed_leader, *term).await?;
            }
            
            ConsensusPayload::CheatAlert { suspected_peer, violation_type, evidence } => {
                self.handle_cheat_alert(message.sender, *suspected_peer, violation_type.clone(), evidence.clone()).await?;
            }
            
            _ => {
                log::debug!("Unhandled message type from {:?}", message.sender);
            }
        }
        
        Ok(())
    }
    
    /// Broadcast message to all participants
    async fn broadcast_message(&self, mut message: ConsensusMessage) -> Result<()> {
        // Sign message
        message.signature = self.sign_message(&message)?;
        
        // Compress if beneficial for BLE
        message.compress()?;
        
        // Convert to BitchatPacket
        let packet = self.message_to_packet(message.clone())?;
        
        // Send via mesh service
        self.mesh_service.broadcast_packet(packet).await?;
        
        // Update metrics
        *self.messages_sent.write().await += 1;
        
        // Add to retry queue for critical messages
        if message.payload.priority() == MessagePriority::Critical {
            let retry_info = RetryInfo {
                message: message.clone(),
                attempts: 0,
                last_attempt: Instant::now(),
                target_peers: self.participants.read().await.clone(),
            };
            self.retry_queue.write().await.insert(message.message_id, retry_info);
        }
        
        Ok(())
    }
    
    /// Handle proposal from peer
    async fn handle_proposal(&self, sender: PeerId, proposal: GameProposal) -> Result<()> {
        log::debug!("Received proposal {:?} from {:?}", proposal.id, sender);
        
        // Validate proposal with consensus engine
        let accepted = {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.process_proposal(proposal)?
        };
        
        if accepted {
            log::info!("Accepted proposal {:?}", sender);
        } else {
            log::warn!("Rejected proposal from {:?}", sender);
        }
        
        Ok(())
    }
    
    /// Handle vote from peer
    async fn handle_vote(&self, sender: PeerId, proposal_id: ProposalId, vote: bool, signature: crate::protocol::Signature) -> Result<()> {
        log::debug!("Received vote {:?} on proposal {:?} from {:?}", vote, proposal_id, sender);
        
        // Process vote with consensus engine
        {
            let mut consensus = self.consensus_engine.lock().await;
            consensus.process_peer_vote(proposal_id, sender, vote, signature)?;
        }
        
        Ok(())
    }
    
    /// Handle state synchronization
    async fn handle_state_sync(&self, _message: &ConsensusMessage) -> Result<()> {
        // TODO: Implement state synchronization logic
        // This would handle catching up on missed state updates
        Ok(())
    }
    
    /// Handle randomness commitment
    async fn handle_randomness_commit(&self, _round_id: RoundId, _commitment: crate::protocol::consensus::commit_reveal::RandomnessCommit) -> Result<()> {
        // TODO: Implement randomness commit handling
        Ok(())
    }
    
    /// Handle randomness reveal
    async fn handle_randomness_reveal(&self, _round_id: RoundId, _reveal: crate::protocol::consensus::commit_reveal::RandomnessReveal) -> Result<()> {
        // TODO: Implement randomness reveal handling  
        Ok(())
    }
    
    /// Handle dispute claim
    async fn handle_dispute_claim(&self, _dispute: crate::protocol::consensus::validation::Dispute) -> Result<()> {
        // TODO: Implement dispute handling
        Ok(())
    }
    
    /// Handle heartbeat from peer
    async fn handle_heartbeat(&self, sender: PeerId, _alive_participants: Vec<PeerId>, network_view: NetworkView) -> Result<()> {
        log::debug!("Received heartbeat from {:?}", sender);
        
        // Update our network view
        let mut current_view = self.network_view.write().await;
        
        // Merge network views (simplified)
        for participant in network_view.participants {
            if !current_view.participants.contains(&participant) {
                current_view.participants.push(participant);
            }
        }
        
        // Update leader if consensus
        if let Some(leader) = network_view.leader {
            if current_view.leader != Some(leader) {
                current_view.leader = Some(leader);
                *self.current_leader.write().await = Some(leader);
            }
        }
        
        Ok(())
    }
    
    /// Handle leader proposal
    async fn handle_leader_proposal(&self, sender: PeerId, proposed_leader: PeerId, term: u64) -> Result<()> {
        log::debug!("Received leader proposal for {:?} (term {}) from {:?}", proposed_leader, term, sender);
        
        let current_term = *self.current_term.read().await;
        
        // Accept if higher term or we have no leader
        if term > current_term || self.current_leader.read().await.is_none() {
            *self.current_term.write().await = term;
            *self.current_leader.write().await = Some(proposed_leader);
            
            // Broadcast acceptance
            let message = ConsensusMessage::new(
                self.identity.peer_id,
                self.game_id,
                *self.current_round.read().await,
                ConsensusPayload::LeaderAccept {
                    term,
                    leader: proposed_leader,
                },
            );
            
            self.broadcast_message(message).await?;
        }
        
        Ok(())
    }
    
    /// Handle cheat alert
    async fn handle_cheat_alert(&self, _sender: PeerId, suspected_peer: PeerId, violation_type: CheatType, _evidence: Vec<u8>) -> Result<()> {
        log::warn!("Cheat alert for {:?}: {:?}", suspected_peer, violation_type);
        
        // Track suspicious behavior
        let mut behavior = self.suspicious_behavior.write().await;
        behavior.entry(suspected_peer)
            .or_default()
            .push(violation_type);
        
        // TODO: Implement cheat response logic
        
        Ok(())
    }
    
    /// Start heartbeat task
    async fn start_heartbeat_task(&self) {
        let participants = self.participants.clone();
        let network_view = self.network_view.clone();
        let outbound_sender = self.outbound_sender.clone();
        let game_id = self.game_id;
        let current_round = self.current_round.clone();
        let identity = self.identity.clone();
        let interval_duration = self.config.heartbeat_interval;
        
        tokio::spawn(async move {
            let mut heartbeat_interval = interval(interval_duration);
            
            loop {
                heartbeat_interval.tick().await;
                
                let participants_list = participants.read().await.iter().copied().collect();
                let view = network_view.read().await.clone();
                let round = *current_round.read().await;
                
                let heartbeat = ConsensusMessage::new(
                    identity.peer_id,
                    game_id,
                    round,
                    ConsensusPayload::Heartbeat {
                        alive_participants: participants_list,
                        network_view: view,
                    },
                );
                
                let _ = outbound_sender.send(heartbeat);
            }
        });
    }
    
    /// Start message processing task
    async fn start_message_processing_task(&self) {
        // TODO: Implement message processing from mesh service
    }
    
    /// Start retry task for failed messages
    async fn start_retry_task(&self) {
        let retry_queue = self.retry_queue.clone();
        let outbound_sender = self.outbound_sender.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut retry_interval = interval(Duration::from_secs(5));
            
            loop {
                retry_interval.tick().await;
                
                let mut queue = retry_queue.write().await;
                let mut to_retry = Vec::new();
                let mut to_remove = Vec::new();
                
                for (message_id, retry_info) in queue.iter_mut() {
                    if retry_info.attempts >= config.max_retries {
                        to_remove.push(*message_id);
                    } else if retry_info.last_attempt.elapsed() > Duration::from_secs(10) {
                        retry_info.attempts += 1;
                        retry_info.last_attempt = Instant::now();
                        to_retry.push(retry_info.message.clone());
                    }
                }
                
                // Remove expired retries
                for message_id in to_remove {
                    queue.remove(&message_id);
                }
                
                drop(queue);
                
                // Send retries
                for message in to_retry {
                    let _ = outbound_sender.send(message);
                }
            }
        });
    }
    
    /// Start leader election task
    async fn start_leader_election_task(&self) {
        // TODO: Implement leader election logic
    }
    
    /// Start partition detection task
    async fn start_partition_detection_task(&self) {
        // TODO: Implement partition detection and recovery
    }
    
    /// Check if message is duplicate
    async fn is_duplicate_message(&self, message_id: &[u8; 32]) -> bool {
        self.message_cache.read().await.contains(message_id)
    }
    
    /// Sign a consensus message
    fn sign_message(&self, message: &ConsensusMessage) -> Result<crate::protocol::Signature> {
        // Serialize message data for signing
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&message.message_id);
        sign_data.extend_from_slice(&message.sender);
        sign_data.extend_from_slice(&message.game_id);
        sign_data.extend_from_slice(&message.round.to_le_bytes());
        sign_data.extend_from_slice(&message.timestamp.to_le_bytes());
        
        // Add payload hash
        let payload_bytes = bincode::serialize(&message.payload)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        sign_data.extend_from_slice(&payload_bytes);
        
        // Sign with identity
        let signature = self.identity.keypair.sign(&sign_data);
        let sig_bytes: [u8; 64] = signature.signature.try_into()
            .unwrap_or([0u8; 64]);
        
        Ok(crate::protocol::Signature(sig_bytes))
    }
    
    /// Convert consensus message to BitchatPacket
    fn message_to_packet(&self, message: ConsensusMessage) -> Result<BitchatPacket> {
        let mut packet = BitchatPacket::new(PACKET_TYPE_CONSENSUS_VOTE);
        
        // Serialize message as payload
        let payload = bincode::serialize(&message)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        
        packet.payload = Some(payload);
        packet.source = message.sender;
        packet.target = [0u8; 32]; // Broadcast
        
        Ok(packet)
    }
    
    /// Update network view
    async fn update_network_view(&self) {
        let participants = self.participants.read().await.clone();
        let leader = *self.current_leader.read().await;
        
        let mut view = self.network_view.write().await;
        view.participants = participants.into_iter().collect();
        view.leader = leader;
    }
    
    /// Get consensus statistics
    pub async fn get_stats(&self) -> ConsensusStats {
        ConsensusStats {
            messages_sent: *self.messages_sent.read().await,
            messages_received: *self.messages_received.read().await,
            consensus_rounds: *self.consensus_rounds_completed.read().await,
            active_participants: self.participants.read().await.len(),
            current_leader: *self.current_leader.read().await,
            partition_detected: *self.partition_detected.read().await,
            pending_messages: self.pending_messages.read().await.len(),
            retry_queue_size: self.retry_queue.read().await.len(),
        }
    }
}

/// Consensus coordinator statistics
#[derive(Debug, Clone)]
pub struct ConsensusStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub consensus_rounds: u64,
    pub active_participants: usize,
    pub current_leader: Option<PeerId>,
    pub partition_detected: bool,
    pub pending_messages: usize,
    pub retry_queue_size: usize,
}