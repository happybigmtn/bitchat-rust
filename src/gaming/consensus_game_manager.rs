//! Consensus-based Game Manager
//!
//! This module manages game sessions with distributed consensus,
//! integrating the game framework with the P2P consensus system.

use arc_swap::ArcSwap;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use uuid;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::{ConsensusMessageHandler, MeshService};
use crate::mesh::{MeshMessage, MeshMessageType};
use crate::protocol::consensus::engine::{GameConsensusState, GameOperation};
use crate::protocol::craps::{Bet, BetType, CrapTokens, CrapsGame, DiceRoll, GamePhase};
use crate::protocol::network_consensus_bridge::NetworkConsensusBridge;
use crate::protocol::{GameId, PeerId};
use std::time::{SystemTime, UNIX_EPOCH};

/// Game discovery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDiscoveryInfo {
    pub game_id: GameId,
    pub host: PeerId,
    pub participants: Vec<PeerId>,
    pub state: String,
    pub created_at: u64,
    pub is_joinable: bool,
}

/// Game discovery request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDiscoveryRequest {
    pub requester: PeerId,
    pub timestamp: u64,
}

/// Search criteria for finding games
#[derive(Debug, Clone, Default)]
pub struct GameSearchCriteria {
    pub min_players: Option<usize>,
    pub max_players: Option<usize>,
    pub state_filter: Option<String>,
    pub only_joinable: bool,
    pub host_filter: Option<PeerId>,
}

/// Game state sync request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSyncRequest {
    pub game_id: GameId,
    pub requester: PeerId,
    pub timestamp: u64,
}

/// Game state sync data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSyncData {
    pub game_id: GameId,
    pub game_state: CrapsGame,
    pub participants: Vec<PeerId>,
    pub consensus_state: GameConsensusState,
    pub pending_operations: Vec<GameOperation>,
    pub timestamp: u64,
}

/// Configuration for consensus-based game management
#[derive(Debug, Clone)]
pub struct ConsensusGameConfig {
    /// Maximum time to wait for consensus on operations
    pub consensus_timeout: Duration,
    /// How often to sync game state
    pub state_sync_interval: Duration,
    /// Maximum number of concurrent games
    pub max_concurrent_games: usize,
    /// Minimum participants for a game
    pub min_participants: usize,
    /// Maximum bet amount in CRAP tokens
    pub max_bet_amount: u64,
}

impl Default for ConsensusGameConfig {
    fn default() -> Self {
        Self {
            consensus_timeout: Duration::from_secs(30),
            state_sync_interval: Duration::from_secs(5),
            max_concurrent_games: 10,
            min_participants: 2,
            max_bet_amount: 1000,
        }
    }
}

/// Game operation with consensus tracking
#[derive(Debug, Clone)]
struct PendingGameOperation {
    operation: GameOperation,
    game_id: GameId,
    submitted_at: Instant,
    consensus_achieved: bool,
}

/// Consensus-based game manager
pub struct ConsensusGameManager {
    // Core components
    identity: Arc<BitchatIdentity>,
    mesh_service: Arc<MeshService>,
    consensus_handler: Arc<ConsensusMessageHandler>,

    // Configuration
    config: ConsensusGameConfig,

    // Game management - using lock-free data structures
    active_games: Arc<DashMap<GameId, ConsensusGameSession>>,
    consensus_bridges: Arc<DashMap<GameId, Arc<NetworkConsensusBridge>>>,

    // Operation tracking - using lock-free hashmap
    pending_operations: Arc<DashMap<String, PendingGameOperation>>,

    // Event handling - bounded channel with backpressure
    game_events: mpsc::Sender<GameEvent>,
    event_receiver: Arc<Mutex<mpsc::Receiver<GameEvent>>>,

    // Statistics - atomic counters for performance
    total_games_created: Arc<AtomicUsize>,
    total_games_completed: Arc<AtomicUsize>,
    total_operations_processed: Arc<AtomicUsize>,
    total_consensus_failures: Arc<AtomicUsize>,
    stats_snapshot: Arc<ArcSwap<GameManagerStats>>,
}

/// Game session with consensus integration
#[derive(Debug, Clone)]
pub struct ConsensusGameSession {
    pub game: CrapsGame,
    pub participants: Vec<PeerId>,
    pub consensus_state: GameConsensusState,
    pub last_updated: Instant,
    pub is_active: bool,
}

/// Game events for external handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    GameCreated {
        game_id: GameId,
        creator: PeerId,
    },
    PlayerJoined {
        game_id: GameId,
        player: PeerId,
    },
    PlayerLeft {
        game_id: GameId,
        player: PeerId,
    },
    BetPlaced {
        game_id: GameId,
        player: PeerId,
        bet: Bet,
    },
    DiceRolled {
        game_id: GameId,
        roll: DiceRoll,
    },
    GamePhaseChanged {
        game_id: GameId,
        new_phase: GamePhase,
    },
    ConsensusAchieved {
        game_id: GameId,
        operation: String,
    },
    ConsensusFailed {
        game_id: GameId,
        operation: String,
        reason: String,
    },
    StateSynced {
        game_id: GameId,
        peer_id: PeerId,
    },
}

/// Statistics for game management
#[derive(Debug, Clone, Default)]
pub struct GameManagerStats {
    pub total_games_created: u64,
    pub total_games_completed: u64,
    pub total_operations_processed: u64,
    pub total_consensus_failures: u64,
    pub average_consensus_time_ms: u64,
    pub active_game_count: usize,
}

impl ConsensusGameManager {
    /// Create new consensus game manager
    pub fn new(
        identity: Arc<BitchatIdentity>,
        mesh_service: Arc<MeshService>,
        consensus_handler: Arc<ConsensusMessageHandler>,
        config: ConsensusGameConfig,
    ) -> Self {
        let (game_events, event_receiver) = mpsc::channel(1000); // Bounded channel for backpressure

        Self {
            identity,
            mesh_service,
            consensus_handler,
            config,
            active_games: Arc::new(DashMap::new()),
            consensus_bridges: Arc::new(DashMap::new()),
            pending_operations: Arc::new(DashMap::new()),
            game_events,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            total_games_created: Arc::new(AtomicUsize::new(0)),
            total_games_completed: Arc::new(AtomicUsize::new(0)),
            total_operations_processed: Arc::new(AtomicUsize::new(0)),
            total_consensus_failures: Arc::new(AtomicUsize::new(0)),
            stats_snapshot: Arc::new(ArcSwap::from_pointee(GameManagerStats::default())),
        }
    }

    /// Start the consensus game manager
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting consensus game manager");

        // Start background tasks
        self.start_game_maintenance().await;
        self.start_state_synchronization().await;
        self.start_operation_timeout_handler().await;
        self.start_event_processor().await;

        Ok(())
    }

    /// Create a new game session
    pub async fn create_game(&self, participants: Vec<PeerId>) -> Result<GameId> {
        // Check game limits - lock-free read
        let active_count = self.active_games.len();
        if active_count >= self.config.max_concurrent_games {
            return Err(Error::GameLogic(
                "Maximum concurrent games reached".to_string(),
            ));
        }

        // Check minimum participants
        if participants.len() < self.config.min_participants {
            return Err(Error::GameLogic(format!(
                "Need at least {} participants, got {}",
                self.config.min_participants,
                participants.len()
            )));
        }

        // Generate game ID
        let game_id = self.generate_game_id();

        // Create consensus bridge for this game
        let consensus_engine = self.create_consensus_engine(game_id, &participants)?;
        let bridge = self
            .create_network_bridge(consensus_engine, game_id, &participants)
            .await?;

        // Start the bridge
        bridge.start().await?;

        // Register bridge with consensus handler
        self.consensus_handler
            .register_consensus_bridge(game_id, bridge.clone())
            .await;
        self.consensus_bridges.insert(game_id, bridge.clone());

        // Create game session
        let craps_game = CrapsGame::new(game_id, self.identity.peer_id);
        let consensus_state = bridge.get_current_state().await?;

        let session = ConsensusGameSession {
            game: craps_game,
            participants: participants.clone(),
            consensus_state,
            last_updated: Instant::now(),
            is_active: true,
        };

        // Store session - lock-free insert
        self.active_games.insert(game_id, session);

        // Update stats - atomic operations
        self.total_games_created.fetch_add(1, Ordering::Relaxed);
        self.update_stats_snapshot().await;

        // Send event
        let _ = self.game_events.send(GameEvent::GameCreated {
            game_id,
            creator: self.identity.peer_id,
        });

        // Broadcast game creation to the mesh network for discovery
        let game_info = self.create_discovery_info(game_id, &participants);

        let broadcast_msg = self.create_announcement_message(game_info)?;

        // Broadcast game announcement
        self.mesh_service.broadcast_message(broadcast_msg).await?;

        log::info!(
            "Created and broadcast new game {:?} with {} participants",
            game_id,
            participants.len()
        );
        Ok(game_id)
    }

    /// Discover available games on the mesh network
    pub async fn discover_games(&self) -> Result<Vec<GameDiscoveryInfo>> {
        // Broadcast game discovery request to mesh network
        let discovery_msg = MeshMessage {
            message_type: MeshMessageType::GameDiscovery,
            payload: bincode::serialize(&GameDiscoveryRequest {
                requester: self.identity.peer_id,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            })?,
            sender: self.identity.peer_id,
            recipient: None, // Broadcast to all peers
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        // Send discovery request via mesh
        self.mesh_service.broadcast_message(discovery_msg).await?;

        // Collect responses for 2 seconds
        let start_time = Instant::now();
        let mut discovered_games = Vec::new();

        while start_time.elapsed() < Duration::from_secs(2) {
            // Check for discovery responses in mesh service
            if let Some(response) = self.mesh_service.poll_discovery_response().await {
                if let Ok(game_info) = bincode::deserialize::<GameDiscoveryInfo>(&response.payload)
                {
                    // Verify game is still active
                    if self.verify_game_active(&game_info).await {
                        discovered_games.push(game_info);
                    }
                }
            }

            // Small delay to avoid busy waiting
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Also include local active games
        for entry in self.active_games.iter() {
            let session = entry.value();
            if session.is_active {
                discovered_games.push(GameDiscoveryInfo {
                    game_id: *entry.key(),
                    host: session
                        .participants
                        .first()
                        .copied()
                        .unwrap_or(self.identity.peer_id),
                    participants: session.participants.clone(),
                    state: format!("{:?}", session.consensus_state.game_state.phase),
                    created_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        - start_time.elapsed().as_secs(),
                    is_joinable: session.participants.len() < 8, // Max 8 players
                });
            }
        }

        // Sort by creation time (newest first)
        discovered_games.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(discovered_games)
    }

    /// Find games matching specific criteria
    pub async fn find_games_by_criteria(
        &self,
        criteria: GameSearchCriteria,
    ) -> Result<Vec<GameDiscoveryInfo>> {
        let all_games = self.discover_games().await?;

        let filtered_games: Vec<_> = all_games
            .into_iter()
            .filter(|game| {
                // Filter by minimum players
                if let Some(min_players) = criteria.min_players {
                    if game.participants.len() < min_players {
                        return false;
                    }
                }

                // Filter by maximum players
                if let Some(max_players) = criteria.max_players {
                    if game.participants.len() > max_players {
                        return false;
                    }
                }

                // Filter by game state
                if let Some(ref state) = criteria.state_filter {
                    if &game.state != state {
                        return false;
                    }
                }

                // Filter by joinability
                if criteria.only_joinable && !game.is_joinable {
                    return false;
                }

                // Filter by specific host
                if let Some(host) = criteria.host_filter {
                    if game.host != host {
                        return false;
                    }
                }

                true
            })
            .collect();

        Ok(filtered_games)
    }

    /// Verify if a discovered game is still active
    async fn verify_game_active(&self, game_info: &GameDiscoveryInfo) -> bool {
        // Check if we have the game locally
        if let Some(session) = self.active_games.get(&game_info.game_id) {
            return session.is_active;
        }

        // Send verification request to game host
        let verify_msg = MeshMessage {
            message_type: MeshMessageType::GameVerification,
            payload: bincode::serialize(&game_info.game_id).unwrap_or_default(),
            sender: self.identity.peer_id,
            recipient: Some(game_info.host),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        // Send and wait for response with timeout
        if let Ok(response) = tokio::time::timeout(
            Duration::from_millis(500),
            self.mesh_service
                .send_and_wait_response(verify_msg, game_info.host),
        )
        .await
        {
            if let Ok(Some(resp)) = response {
                return resp.message_type == MeshMessageType::GameVerificationAck;
            }
        }

        false
    }

    /// Handle incoming game discovery requests
    pub async fn handle_discovery_request(&self, requester: PeerId) -> Result<()> {
        // Respond with our active games
        for entry in self.active_games.iter() {
            let session = entry.value();
            if session.is_active && session.participants.len() < 8 {
                let game_info = GameDiscoveryInfo {
                    game_id: *entry.key(),
                    host: self.identity.peer_id,
                    participants: session.participants.clone(),
                    state: format!("{:?}", session.consensus_state.game_state.phase),
                    created_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    is_joinable: true,
                };

                let response_msg = MeshMessage {
                    message_type: MeshMessageType::GameDiscoveryResponse,
                    payload: bincode::serialize(&game_info)?,
                    sender: self.identity.peer_id,
                    recipient: Some(requester),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    signature: vec![],
                };

                self.mesh_service
                    .send_message(response_msg, requester)
                    .await?;
            }
        }

        Ok(())
    }

    /// Join an existing game with full state synchronization
    pub async fn join_game(&self, game_id: GameId) -> Result<()> {
        // First, request current game state from host
        let game_state = self.request_game_state_sync(game_id).await?;

        // Try to get and update the game session atomically
        if let Some(mut session_entry) = self.active_games.get_mut(&game_id) {
            if !session_entry.participants.contains(&self.identity.peer_id) {
                session_entry.participants.push(self.identity.peer_id);
                session_entry.last_updated = Instant::now();

                // Sync the received state
                self.sync_game_state(&mut session_entry, game_state).await?;

                // Add participant to consensus - lock-free lookup
                if let Some(bridge) = self.consensus_bridges.get(&game_id) {
                    bridge.add_participant(self.identity.peer_id).await?;
                }

                // Send event with backpressure handling
                if let Err(mpsc::error::TrySendError::Full(_)) =
                    self.game_events.try_send(GameEvent::PlayerJoined {
                        game_id,
                        player: self.identity.peer_id,
                    })
                {
                    log::warn!("Event queue full, dropping PlayerJoined event");
                }

                log::info!("Joined game {:?}", game_id);
            }
        } else {
            return Err(Error::GameLogic("Game not found".to_string()));
        }

        Ok(())
    }

    /// Place a bet in a game
    pub async fn place_bet(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount: CrapTokens,
    ) -> Result<()> {
        // Validate bet amount
        if amount.0 > self.config.max_bet_amount {
            return Err(Error::GameLogic(format!(
                "Bet amount {} exceeds maximum {}",
                amount.0, self.config.max_bet_amount
            )));
        }

        // Check if game exists - lock-free lookup
        if !self.active_games.contains_key(&game_id) {
            return Err(Error::GameLogic("Game not found".to_string()));
        }

        // Create bet operation
        let bet = Bet {
            id: uuid::Uuid::new_v4().into_bytes(),
            player: self.identity.peer_id,
            game_id,
            bet_type,
            amount,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let operation = GameOperation::PlaceBet {
            player: self.identity.peer_id,
            bet: bet.clone(),
            nonce: self.generate_nonce(),
        };

        // Submit through consensus
        self.submit_consensus_operation(game_id, operation, "place_bet")
            .await?;

        // Send event
        let _ = self.game_events.send(GameEvent::BetPlaced {
            game_id,
            player: self.identity.peer_id,
            bet,
        });

        log::info!(
            "Placed bet {:?} of {} CRAP in game {:?}",
            bet_type,
            amount.to_crap(),
            game_id
        );
        Ok(())
    }

    /// Roll dice in a game (if player is shooter) - optimized with lock-free access
    pub async fn roll_dice(&self, game_id: GameId) -> Result<DiceRoll> {
        // Check if game exists and player can roll - lock-free read
        let session = self
            .active_games
            .get(&game_id)
            .ok_or_else(|| Error::GameLogic("Game not found".to_string()))?;

        // Check if it's come-out or point phase
        if !matches!(session.game.phase, GamePhase::ComeOut | GamePhase::Point) {
            return Err(Error::GameLogic(
                "Cannot roll dice in current phase".to_string(),
            ));
        }

        // Generate dice roll
        let dice_roll = DiceRoll::generate();

        // Create operation
        let operation = GameOperation::ProcessRoll {
            round_id: self.generate_round_id(),
            dice_roll,
            entropy_proof: vec![], // Simplified - would include cryptographic proof
        };

        // Submit through consensus
        self.submit_consensus_operation(game_id, operation, "roll_dice")
            .await?;

        // Send event
        let _ = self.game_events.send(GameEvent::DiceRolled {
            game_id,
            roll: dice_roll,
        });

        log::info!("Rolled dice: {} in game {:?}", dice_roll, game_id);
        Ok(dice_roll)
    }

    /// Get game state - lock-free access
    pub async fn get_game_state(&self, game_id: &GameId) -> Option<ConsensusGameSession> {
        self.active_games
            .get(game_id)
            .map(|entry| entry.value().clone())
    }

    /// List active games - lock-free iteration
    pub async fn list_active_games(&self) -> Vec<(GameId, ConsensusGameSession)> {
        self.active_games
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    /// Submit operation through consensus system
    async fn submit_consensus_operation(
        &self,
        game_id: GameId,
        operation: GameOperation,
        operation_type: &str,
    ) -> Result<()> {
        let bridge = self
            .consensus_bridges
            .get(&game_id)
            .ok_or_else(|| Error::GameLogic("No consensus bridge for game".to_string()))?;

        // Submit operation
        let proposal_id = bridge.submit_operation(operation.clone()).await?;

        // Track pending operation
        let pending_op = PendingGameOperation {
            operation,
            game_id,
            submitted_at: Instant::now(),
            consensus_achieved: false,
        };

        let operation_key = format!("{:?}_{}", proposal_id, operation_type);
        self.pending_operations.insert(operation_key, pending_op);

        // Update stats - atomic operation
        self.total_operations_processed
            .fetch_add(1, Ordering::Relaxed);
        self.update_stats_snapshot().await;

        Ok(())
    }

    /// Start game maintenance task
    async fn start_game_maintenance(&self) {
        let active_games = self.active_games.clone();
        let consensus_bridges = self.consensus_bridges.clone();
        let consensus_handler = self.consensus_handler.clone();
        let total_completed = self.total_games_completed.clone();

        tokio::spawn(async move {
            let mut maintenance_interval = interval(Duration::from_secs(60));

            loop {
                maintenance_interval.tick().await;

                // Clean up inactive games - lock-free operations
                let cutoff = Instant::now() - Duration::from_secs(3600); // 1 hour timeout
                let mut completed_games = 0;
                let mut expired_games = Vec::new();

                // Identify expired games
                for entry in active_games.iter() {
                    let session = entry.value();
                    if !session.is_active || session.last_updated < cutoff {
                        expired_games.push(*entry.key());
                    }
                }

                // Remove expired games
                for game_id in expired_games {
                    if active_games.remove(&game_id).is_some() {
                        completed_games += 1;

                        // Remove consensus bridge
                        if consensus_bridges.remove(&game_id).is_some() {
                            let handler = consensus_handler.clone();
                            tokio::spawn(async move {
                                handler.unregister_consensus_bridge(&game_id).await;
                            });
                        }
                    }
                }

                // Update stats atomically
                if completed_games > 0 {
                    total_completed.fetch_add(completed_games, Ordering::Relaxed);
                }

                log::debug!(
                    "Game maintenance: {} active games, {} cleaned up",
                    active_games.len(),
                    completed_games
                );
            }
        });
    }

    /// Start state synchronization task
    async fn start_state_synchronization(&self) {
        let active_games = self.active_games.clone();
        let consensus_bridges = self.consensus_bridges.clone();
        let sync_interval = self.config.state_sync_interval;

        tokio::spawn(async move {
            let mut sync_interval = interval(sync_interval);

            loop {
                sync_interval.tick().await;

                // Sync state for all active games - parallel processing
                let sync_tasks: Vec<_> = active_games
                    .iter()
                    .map(|entry| {
                        let game_id = *entry.key();
                        let bridges_clone = consensus_bridges.clone();

                        tokio::spawn(async move {
                            if let Some(bridge) = bridges_clone.get(&game_id) {
                                // Get updated consensus state
                                if let Ok(_consensus_state) = bridge.get_current_state().await {
                                    log::debug!("Synced state for game {:?}", game_id);
                                }
                            }
                        })
                    })
                    .collect();

                // Wait for all sync tasks to complete
                for task in sync_tasks {
                    let _ = task.await;
                }
            }
        });
    }

    /// Start operation timeout handler
    async fn start_operation_timeout_handler(&self) {
        let pending_operations = self.pending_operations.clone();
        let game_events = self.game_events.clone();
        let consensus_failures = self.total_consensus_failures.clone();
        let timeout = self.config.consensus_timeout;

        tokio::spawn(async move {
            let mut timeout_interval = interval(Duration::from_secs(10));

            loop {
                timeout_interval.tick().await;

                let mut expired_operations = Vec::new();
                let mut failed_count = 0;

                // Identify expired operations
                for entry in pending_operations.iter() {
                    if entry.value().submitted_at.elapsed() > timeout {
                        expired_operations.push(entry.key().clone());
                        failed_count += 1;
                    }
                }

                // Remove expired operations and send events
                for operation_key in expired_operations {
                    if let Some((_, op)) = pending_operations.remove(&operation_key) {
                        // Send failure event with backpressure handling
                        if let Err(mpsc::error::TrySendError::Full(_)) =
                            game_events.try_send(GameEvent::ConsensusFailed {
                                game_id: op.game_id,
                                operation: operation_key,
                                reason: "Consensus timeout".to_string(),
                            })
                        {
                            log::warn!("Event queue full, dropping ConsensusFailed event");
                        }
                    }
                }

                // Update stats atomically
                if failed_count > 0 {
                    consensus_failures.fetch_add(failed_count, Ordering::Relaxed);
                }
            }
        });
    }

    /// Start event processor
    async fn start_event_processor(&self) {
        let event_receiver = self.event_receiver.clone();

        tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;

            while let Some(event) = receiver.recv().await {
                // Process game events
                match event {
                    GameEvent::ConsensusAchieved { game_id, operation } => {
                        log::info!("Consensus achieved for {} in game {:?}", operation, game_id);
                    }
                    GameEvent::ConsensusFailed {
                        game_id,
                        operation,
                        reason,
                    } => {
                        log::warn!(
                            "Consensus failed for {} in game {:?}: {}",
                            operation,
                            game_id,
                            reason
                        );
                    }
                    _ => {
                        log::debug!("Game event: {:?}", event);
                    }
                }
            }
        });
    }

    /// Generate unique game ID
    fn generate_game_id(&self) -> GameId {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(self.identity.peer_id);
        hasher.update(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_be_bytes(),
        );

        let hash = hasher.finalize();
        let mut game_id = [0u8; 16];
        game_id.copy_from_slice(&hash[..16]);
        game_id
    }

    /// Generate nonce for operations
    fn generate_nonce(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Generate round ID
    fn generate_round_id(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Request game state synchronization from host
    async fn request_game_state_sync(&self, game_id: GameId) -> Result<GameStateSyncData> {
        // Find the game host
        let host = if let Some(session) = self.active_games.get(&game_id) {
            session
                .participants
                .first()
                .copied()
                .unwrap_or(self.identity.peer_id)
        } else {
            // Try to discover the game
            let games = self.discover_games().await?;
            games
                .iter()
                .find(|g| g.game_id == game_id)
                .map(|g| g.host)
                .ok_or_else(|| Error::GameNotFound)?
        };

        // Create state sync request
        let request = GameStateSyncRequest {
            game_id,
            requester: self.identity.peer_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let msg = MeshMessage {
            message_type: MeshMessageType::GameStateSync,
            payload: bincode::serialize(&request)?,
            sender: self.identity.peer_id,
            recipient: Some(host),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        // Send request and wait for response
        let response = match tokio::time::timeout(
            Duration::from_secs(5),
            self.mesh_service.send_and_wait_response(msg, host),
        )
        .await
        {
            Ok(Ok(Some(resp))) => resp,
            Ok(Ok(None)) => return Err(Error::Network("No response from game host".to_string())),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(Error::Network("Game state sync timeout".to_string())),
        };

        // Parse response
        bincode::deserialize(&response.payload).map_err(|e| Error::Serialization(e.to_string()))
    }

    /// Sync received game state to local session
    async fn sync_game_state(
        &self,
        session: &mut ConsensusGameSession,
        state: GameStateSyncData,
    ) -> Result<()> {
        let game_id = state.game_id;
        let participants = state.participants.clone();

        // Update game state
        session.game = state.game_state;
        session.participants = state.participants;
        session.consensus_state = state.consensus_state.clone();
        session.last_updated = Instant::now();

        // Sync consensus bridge if it exists
        if let Some(bridge) = self.consensus_bridges.get(&game_id) {
            // Update bridge with latest consensus state
            bridge.sync_state(state.consensus_state).await?;

            // Add all participants to bridge
            for participant in &participants {
                if *participant != self.identity.peer_id {
                    bridge.add_participant(*participant).await?;
                }
            }
        }

        // Process any pending operations
        for operation in state.pending_operations {
            // Submit to consensus for processing
            if let Some(bridge) = self.consensus_bridges.get(&game_id) {
                bridge.submit_operation(operation).await?;
            }
        }

        // Emit sync completed event
        let _ = self.game_events.send(GameEvent::StateSynced {
            game_id,
            peer_id: self.identity.peer_id,
        });

        Ok(())
    }

    /// Get list of active games
    pub async fn get_active_games(&self) -> Result<Vec<GameId>> {
        // Return all active game IDs
        let game_ids: Vec<GameId> = self
            .active_games
            .iter()
            .filter(|entry| entry.is_active)
            .map(|entry| entry.key().clone())
            .collect();

        Ok(game_ids)
    }

    /// Handle incoming state sync requests
    pub async fn handle_state_sync_request(&self, request: GameStateSyncRequest) -> Result<()> {
        // Get game session
        let session = self
            .active_games
            .get(&request.game_id)
            .ok_or_else(|| Error::GameNotFound)?;

        // Get consensus state from bridge
        let consensus_state = if let Some(bridge) = self.consensus_bridges.get(&request.game_id) {
            bridge.get_current_state().await?
        } else {
            session.consensus_state.clone()
        };

        // Collect pending operations
        let pending_operations = if let Some(bridge) = self.consensus_bridges.get(&request.game_id)
        {
            bridge.get_pending_operations().await?
        } else {
            vec![]
        };

        // Create sync data
        let sync_data = GameStateSyncData {
            game_id: request.game_id,
            game_state: session.game.clone(),
            participants: session.participants.clone(),
            consensus_state,
            pending_operations,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Send response
        let response_msg = MeshMessage {
            message_type: MeshMessageType::GameStateSyncResponse,
            payload: bincode::serialize(&sync_data)?,
            sender: self.identity.peer_id,
            recipient: Some(request.requester),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: vec![],
        };

        self.mesh_service
            .send_message(response_msg, request.requester)
            .await?;

        Ok(())
    }

    /// Update statistics snapshot atomically
    async fn update_stats_snapshot(&self) {
        let stats = GameManagerStats {
            total_games_created: self.total_games_created.load(Ordering::Relaxed) as u64,
            total_games_completed: self.total_games_completed.load(Ordering::Relaxed) as u64,
            total_operations_processed: self.total_operations_processed.load(Ordering::Relaxed)
                as u64,
            total_consensus_failures: self.total_consensus_failures.load(Ordering::Relaxed) as u64,
            active_game_count: self.active_games.len(),
            average_consensus_time_ms: 0, // TODO: Calculate from metrics
        };
        self.stats_snapshot.store(Arc::new(stats));
    }

    /// Get manager statistics - lock-free read
    pub async fn get_stats(&self) -> GameManagerStats {
        self.update_stats_snapshot().await;
        (**self.stats_snapshot.load()).clone()
    }

    /// Get event receiver for external handling (creates a new channel)
    pub async fn create_event_receiver(&self) -> tokio::sync::mpsc::Receiver<GameEvent> {
        let (tx, rx) = mpsc::channel(100);
        // In practice, you'd use a broadcast channel for multiple consumers
        // For now, return a new receiver
        rx
    }

    // Helper methods for improved code organization

    /// Create a consensus engine for the game
    fn create_consensus_engine(
        &self,
        game_id: GameId,
        participants: &[PeerId],
    ) -> Result<Arc<Mutex<crate::protocol::consensus::engine::ConsensusEngine>>> {
        let engine = crate::protocol::consensus::engine::ConsensusEngine::new(
            game_id,
            participants.to_vec(),
            self.identity.peer_id,
            crate::protocol::consensus::ConsensusConfig::default(),
        )?;
        Ok(Arc::new(Mutex::new(engine)))
    }

    /// Create a network consensus bridge
    async fn create_network_bridge(
        &self,
        consensus_engine: Arc<Mutex<crate::protocol::consensus::engine::ConsensusEngine>>,
        game_id: GameId,
        participants: &[PeerId],
    ) -> Result<Arc<NetworkConsensusBridge>> {
        let bridge = NetworkConsensusBridge::new(
            consensus_engine,
            self.mesh_service.clone(),
            self.identity.clone(),
            game_id,
            participants.to_vec(),
        )
        .await?;
        Ok(Arc::new(bridge))
    }

    /// Create game discovery info
    fn create_discovery_info(&self, game_id: GameId, participants: &[PeerId]) -> GameDiscoveryInfo {
        GameDiscoveryInfo {
            game_id,
            host: self.identity.peer_id,
            participants: participants.to_vec(),
            state: "WaitingForPlayers".to_string(),
            created_at: self.get_current_timestamp(),
            is_joinable: true,
        }
    }

    /// Create game announcement message
    fn create_announcement_message(&self, game_info: GameDiscoveryInfo) -> Result<MeshMessage> {
        Ok(MeshMessage {
            message_type: MeshMessageType::GameAnnouncement,
            payload: bincode::serialize(&game_info)?,
            sender: self.identity.peer_id,
            recipient: None, // Broadcast to all
            timestamp: self.get_current_timestamp(),
            signature: vec![],
        })
    }

    /// Get current Unix timestamp
    fn get_current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use crate::mesh::ConsensusMessageConfig;
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_consensus_game_manager_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity.clone(), transport));
        let consensus_handler = Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            identity.clone(),
            ConsensusMessageConfig::default(),
        ));

        let config = ConsensusGameConfig::default();
        let manager = ConsensusGameManager::new(identity, mesh_service, consensus_handler, config);

        let stats = manager.get_stats().await;
        assert_eq!(stats.active_game_count, 0);
        assert_eq!(stats.total_games_created, 0);
    }

    #[tokio::test]
    async fn test_game_creation() {
        let keypair1 = BitchatKeypair::generate();
        let keypair2 = BitchatKeypair::generate();
        let identity1 = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair1, 8,
        ));
        let identity2 = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair2, 8,
        ));

        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity1.clone(), transport));
        let consensus_handler = Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            identity1.clone(),
            ConsensusMessageConfig::default(),
        ));

        let config = ConsensusGameConfig::default();
        let manager =
            ConsensusGameManager::new(identity1.clone(), mesh_service, consensus_handler, config);

        let participants = vec![identity1.peer_id, identity2.peer_id];
        let result = manager.create_game(participants).await;

        // Should succeed with enough participants
        assert!(result.is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_games_created, 1);
    }
}
