//! Game Orchestrator - Phase 2.1 Game Coordination Logic
//!
//! This module implements the core game coordination functionality including:
//! - Game discovery and advertisement protocol
//! - Distributed game session management
//! - Consensus-based game state synchronization
//! - Turn management and conflict resolution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::time::interval;
use uuid::Uuid;

use super::consensus_game_manager::{ConsensusGameManager, GameEvent};
use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::protocol::craps::{BetType, CrapTokens, DiceRoll};
use crate::protocol::{GameId, PeerId};

/// Game advertisement message broadcast over mesh network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameAdvertisement {
    pub game_id: GameId,
    pub creator: PeerId,
    pub game_type: String,
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_players: usize,
    pub current_players: usize,
    pub created_at: u64,
    pub join_deadline: Option<u64>,
    pub game_config: GameConfig,
}

/// Game discovery request/response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDiscoveryRequest {
    pub requester: PeerId,
    pub game_type_filter: Option<String>,
    pub min_bet_filter: Option<u64>,
    pub max_bet_filter: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDiscoveryResponse {
    pub responder: PeerId,
    pub available_games: Vec<GameAdvertisement>,
    pub timestamp: u64,
}

/// Join game request with authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameJoinRequest {
    pub game_id: GameId,
    pub player_id: PeerId,
    pub player_signature: Vec<u8>,
    pub initial_balance: CrapTokens,
    pub capabilities: PlayerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCapabilities {
    pub supports_fast_sync: bool,
    pub max_latency_tolerance: u64,       // milliseconds
    pub preferred_consensus_timeout: u64, // seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameJoinResponse {
    pub game_id: GameId,
    pub accepted: bool,
    pub reason: Option<String>,
    pub current_state: Option<GameStateSnapshot>,
    pub participants: Vec<PeerId>,
}

/// Game configuration for advertisements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub game_type: String,
    pub min_bet: u64,
    pub max_bet: u64,
    pub player_limit: usize,
    pub timeout_seconds: u32,
    pub consensus_threshold: f32, // 0.0 to 1.0
    pub allow_spectators: bool,
}

/// Snapshot of game state for new joiners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSnapshot {
    pub current_phase: String,
    pub active_bets: Vec<BetRecord>,
    pub game_history: Vec<GameEvent>,
    pub current_turn: Option<PeerId>,
    pub turn_deadline: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetRecord {
    pub player: PeerId,
    pub bet_type: BetType,
    pub amount: CrapTokens,
    pub timestamp: u64,
}

/// Dice roll commit/reveal for fairness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceCommit {
    pub game_id: GameId,
    pub round_id: u64,
    pub roller: PeerId,
    pub commitment_hash: [u8; 32],
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceReveal {
    pub game_id: GameId,
    pub round_id: u64,
    pub roller: PeerId,
    pub dice_roll: DiceRoll,
    pub nonce: [u8; 32],
    pub signature: Vec<u8>,
}

/// Turn management system
#[derive(Debug, Clone)]
pub struct TurnManager {
    pub current_player: Option<PeerId>,
    pub turn_started: Option<Instant>,
    pub turn_timeout: Duration,
    pub turn_order: Vec<PeerId>,
    pub turn_index: usize,
}

impl TurnManager {
    pub fn new(players: Vec<PeerId>, turn_timeout: Duration) -> Self {
        Self {
            current_player: players.first().copied(),
            turn_started: Some(Instant::now()),
            turn_timeout,
            turn_order: players,
            turn_index: 0,
        }
    }

    pub fn next_turn(&mut self) -> Option<PeerId> {
        if self.turn_order.is_empty() {
            return None;
        }

        self.turn_index = (self.turn_index + 1) % self.turn_order.len();
        self.current_player = self.turn_order.get(self.turn_index).copied();
        self.turn_started = Some(Instant::now());

        self.current_player
    }

    pub fn is_turn_expired(&self) -> bool {
        if let Some(started) = self.turn_started {
            started.elapsed() > self.turn_timeout
        } else {
            false
        }
    }
}

/// Core game orchestrator for P2P game coordination
#[derive(Clone)]
pub struct GameOrchestrator {
    identity: Arc<BitchatIdentity>,
    mesh_service: Arc<MeshService>,
    consensus_manager: Arc<ConsensusGameManager>,

    // Game discovery and advertisement
    advertised_games: Arc<RwLock<HashMap<GameId, GameAdvertisement>>>,
    discovered_games: Arc<RwLock<HashMap<GameId, GameAdvertisement>>>,

    // Active game sessions with turn management
    active_sessions: Arc<RwLock<HashMap<GameId, GameSession>>>,

    // Event channels
    event_tx: broadcast::Sender<OrchestratorEvent>,
    command_rx: Arc<Mutex<mpsc::UnboundedReceiver<OrchestratorCommand>>>,

    // Configuration
    config: OrchestratorConfig,

    // Statistics
    stats: Arc<RwLock<OrchestratorStats>>,
}

/// Extended game session with orchestrator-specific data
#[derive(Debug)]
pub struct GameSession {
    pub game_id: GameId,
    pub participants: Vec<PeerId>,
    pub turn_manager: Arc<Mutex<TurnManager>>,
    pub state_snapshot: Arc<RwLock<GameStateSnapshot>>,
    pub pending_commits: Arc<RwLock<HashMap<u64, Vec<DiceCommit>>>>,
    pub pending_reveals: Arc<RwLock<HashMap<u64, Vec<DiceReveal>>>>,
    pub bet_history: Arc<RwLock<Vec<BetRecord>>>,
    pub created_at: Instant,
    pub last_activity: Arc<RwLock<Instant>>,
}

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub game_discovery_interval: Duration,
    pub advertisement_ttl: Duration,
    pub join_request_timeout: Duration,
    pub turn_timeout: Duration,
    pub state_sync_interval: Duration,
    pub max_concurrent_games: usize,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            game_discovery_interval: Duration::from_secs(30),
            advertisement_ttl: Duration::from_secs(300),
            join_request_timeout: Duration::from_secs(30),
            turn_timeout: Duration::from_secs(60),
            state_sync_interval: Duration::from_secs(5),
            max_concurrent_games: 10,
        }
    }
}

/// Events emitted by the orchestrator
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    GameAdvertised {
        game_id: GameId,
        advertisement: GameAdvertisement,
    },
    GameDiscovered {
        game_id: GameId,
        advertisement: GameAdvertisement,
    },
    PlayerJoinRequested {
        game_id: GameId,
        player: PeerId,
    },
    PlayerJoined {
        game_id: GameId,
        player: PeerId,
    },
    PlayerLeft {
        game_id: GameId,
        player: PeerId,
        reason: String,
    },
    TurnChanged {
        game_id: GameId,
        current_player: Option<PeerId>,
        previous_player: Option<PeerId>,
    },
    TurnTimeout {
        game_id: GameId,
        player: PeerId,
    },
    BetValidated {
        game_id: GameId,
        bet: BetRecord,
    },
    DiceCommitted {
        game_id: GameId,
        round_id: u64,
        roller: PeerId,
    },
    DiceRevealed {
        game_id: GameId,
        round_id: u64,
        roll: DiceRoll,
    },
    PayoutCalculated {
        game_id: GameId,
        payouts: HashMap<PeerId, CrapTokens>,
    },
    StateConflictDetected {
        game_id: GameId,
        conflicting_peers: Vec<PeerId>,
    },
}

/// Commands for orchestrator control
#[derive(Debug)]
pub enum OrchestratorCommand {
    AdvertiseGame {
        config: GameConfig,
        max_players: usize,
    },
    DiscoverGames {
        filters: GameDiscoveryRequest,
    },
    JoinGame {
        game_id: GameId,
        initial_balance: CrapTokens,
    },
    LeaveGame {
        game_id: GameId,
    },
    PlaceBet {
        game_id: GameId,
        bet_type: BetType,
        amount: CrapTokens,
    },
    CommitDiceRoll {
        game_id: GameId,
        commitment_hash: [u8; 32],
    },
    RevealDiceRoll {
        game_id: GameId,
        dice_roll: DiceRoll,
        nonce: [u8; 32],
    },
    ForceStateSync {
        game_id: GameId,
    },
    ResolveConflict {
        game_id: GameId,
        resolution_data: Vec<u8>,
    },
}

#[derive(Debug, Default, Clone)]
pub struct OrchestratorStats {
    pub games_advertised: u64,
    pub games_discovered: u64,
    pub join_requests_sent: u64,
    pub join_requests_received: u64,
    pub successful_joins: u64,
    pub bets_validated: u64,
    pub dice_commits: u64,
    pub dice_reveals: u64,
    pub payouts_calculated: u64,
    pub conflicts_resolved: u64,
    pub active_games: usize,
}

impl GameOrchestrator {
    /// Create new game orchestrator
    pub fn new(
        identity: Arc<BitchatIdentity>,
        mesh_service: Arc<MeshService>,
        consensus_manager: Arc<ConsensusGameManager>,
        config: OrchestratorConfig,
    ) -> (
        Self,
        mpsc::UnboundedSender<OrchestratorCommand>,
        broadcast::Receiver<OrchestratorEvent>,
    ) {
        let (event_tx, event_rx) = broadcast::channel(1000);
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let orchestrator = Self {
            identity,
            mesh_service,
            consensus_manager,
            advertised_games: Arc::new(RwLock::new(HashMap::new())),
            discovered_games: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            command_rx: Arc::new(Mutex::new(command_rx)),
            config,
            stats: Arc::new(RwLock::new(OrchestratorStats::default())),
        };

        (orchestrator, command_tx, event_rx)
    }

    /// Start the orchestrator service
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting Game Orchestrator");

        // Start background tasks
        self.start_game_discovery_task().await;
        self.start_command_processor().await;
        self.start_turn_timeout_monitor().await;
        self.start_state_sync_task().await;
        self.start_conflict_resolver().await;

        Ok(())
    }

    /// Advertise a new game on the network
    pub async fn advertise_game(&self, config: GameConfig) -> Result<GameId> {
        let game_id = self.generate_game_id();

        let advertisement = GameAdvertisement {
            game_id,
            creator: self.identity.peer_id,
            game_type: config.game_type.clone(),
            min_bet: config.min_bet,
            max_bet: config.max_bet,
            max_players: config.player_limit,
            current_players: 1, // Creator is first player
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            join_deadline: None,
            game_config: config.clone(),
        };

        // Store advertisement
        self.advertised_games
            .write()
            .await
            .insert(game_id, advertisement.clone());

        // Broadcast advertisement over mesh
        let packet = self.create_advertisement_packet(&advertisement)?;
        self.broadcast_message(packet).await?;

        // Create game session in consensus manager
        let participants = vec![self.identity.peer_id];
        self.consensus_manager.create_game(participants).await?;

        // Create local session
        let turn_manager = Arc::new(Mutex::new(TurnManager::new(
            vec![self.identity.peer_id],
            self.config.turn_timeout,
        )));

        let session = GameSession {
            game_id,
            participants: vec![self.identity.peer_id],
            turn_manager,
            state_snapshot: Arc::new(RwLock::new(GameStateSnapshot {
                current_phase: "waiting".to_string(),
                active_bets: Vec::new(),
                game_history: Vec::new(),
                current_turn: Some(self.identity.peer_id),
                turn_deadline: None,
            })),
            pending_commits: Arc::new(RwLock::new(HashMap::new())),
            pending_reveals: Arc::new(RwLock::new(HashMap::new())),
            bet_history: Arc::new(RwLock::new(Vec::new())),
            created_at: Instant::now(),
            last_activity: Arc::new(RwLock::new(Instant::now())),
        };

        self.active_sessions.write().await.insert(game_id, session);

        // Update stats
        self.stats.write().await.games_advertised += 1;
        self.stats.write().await.active_games += 1;

        // Emit event
        let _ = self.event_tx.send(OrchestratorEvent::GameAdvertised {
            game_id,
            advertisement,
        });

        log::info!(
            "Advertised game {:?} of type '{}'",
            game_id,
            config.game_type
        );
        Ok(game_id)
    }

    /// Process join request from another peer
    pub async fn process_join_request(&self, request: GameJoinRequest) -> Result<GameJoinResponse> {
        let game_id = request.game_id;

        // Verify the request signature
        if !self.verify_join_request_signature(&request)? {
            return Ok(GameJoinResponse {
                game_id,
                accepted: false,
                reason: Some("Invalid signature".to_string()),
                current_state: None,
                participants: Vec::new(),
            });
        }

        // Check if game exists and has space
        let sessions = self.active_sessions.read().await;
        if let Some(session) = sessions.get(&game_id) {
            let advertisement = self.advertised_games.read().await.get(&game_id).cloned();

            if let Some(ad) = advertisement {
                if session.participants.len() >= ad.max_players {
                    return Ok(GameJoinResponse {
                        game_id,
                        accepted: false,
                        reason: Some("Game is full".to_string()),
                        current_state: None,
                        participants: Vec::new(),
                    });
                }

                // Accept the join request
                drop(sessions);

                // Add to consensus manager
                self.consensus_manager.join_game(game_id).await?;

                // Update local session
                let mut sessions = self.active_sessions.write().await;
                if let Some(session) = sessions.get_mut(&game_id) {
                    session.participants.push(request.player_id);
                    *session.last_activity.write().await = Instant::now();

                    // Update turn manager
                    let mut turn_manager = session.turn_manager.lock().await;
                    turn_manager.turn_order.push(request.player_id);
                }

                // Get current state for new player
                let state_snapshot = sessions
                    .get(&game_id)
                    .map(|s| s.state_snapshot.clone())
                    .unwrap_or_else(|| {
                        Arc::new(RwLock::new(GameStateSnapshot {
                            current_phase: "waiting".to_string(),
                            active_bets: Vec::new(),
                            game_history: Vec::new(),
                            current_turn: None,
                            turn_deadline: None,
                        }))
                    });

                let current_state = state_snapshot.read().await.clone();
                let participants = sessions
                    .get(&game_id)
                    .map(|s| s.participants.clone())
                    .unwrap_or_default();

                // Update stats
                self.stats.write().await.successful_joins += 1;

                // Emit event
                let _ = self.event_tx.send(OrchestratorEvent::PlayerJoined {
                    game_id,
                    player: request.player_id,
                });

                log::info!("Player {:?} joined game {:?}", request.player_id, game_id);

                return Ok(GameJoinResponse {
                    game_id,
                    accepted: true,
                    reason: None,
                    current_state: Some(current_state),
                    participants,
                });
            }
        }

        Ok(GameJoinResponse {
            game_id,
            accepted: false,
            reason: Some("Game not found".to_string()),
            current_state: None,
            participants: Vec::new(),
        })
    }

    /// Create advertisement packet for mesh broadcast
    fn create_advertisement_packet(&self, advertisement: &GameAdvertisement) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(advertisement).map_err(|e| {
            Error::Serialization(format!("Failed to serialize advertisement: {}", e))
        })?;

        Ok(serialized)
    }

    /// Broadcast message over mesh network
    async fn broadcast_message(&self, packet: Vec<u8>) -> Result<()> {
        // This would integrate with the actual mesh service broadcasting
        // For now, we'll use a placeholder
        log::debug!("Broadcasting packet of {} bytes", packet.len());
        Ok(())
    }

    /// Verify join request signature
    fn verify_join_request_signature(&self, _request: &GameJoinRequest) -> Result<bool> {
        // In production, this would verify the cryptographic signature
        // For now, we'll accept all requests
        Ok(true)
    }

    /// Generate unique game ID
    fn generate_game_id(&self) -> GameId {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(self.identity.peer_id);
        hasher.update(Uuid::new_v4().as_bytes());
        hasher.update(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_be_bytes(),
        );

        let hash = hasher.finalize();
        let mut game_id = [0u8; 16];
        game_id.copy_from_slice(&hash[..16]);
        game_id
    }

    /// Start game discovery task - broadcasts discovery requests and processes responses
    async fn start_game_discovery_task(&self) {
        let discovered_games = self.discovered_games.clone();
        let advertised_games = self.advertised_games.clone();
        let identity = self.identity.clone();
        let mesh_service = self.mesh_service.clone();
        let event_tx = self.event_tx.clone();
        let stats = self.stats.clone();
        let discovery_interval = self.config.game_discovery_interval;

        tokio::spawn(async move {
            let mut discovery_timer = interval(discovery_interval);

            loop {
                discovery_timer.tick().await;

                // Create discovery request
                let discovery_request = GameDiscoveryRequest {
                    requester: identity.peer_id,
                    game_type_filter: None,
                    min_bet_filter: None,
                    max_bet_filter: None,
                };

                // Broadcast discovery request
                if let Ok(packet) = Self::create_discovery_packet(&identity, &discovery_request) {
                    if let Err(e) = Self::broadcast_via_mesh(&mesh_service, packet).await {
                        log::warn!("Failed to broadcast discovery request: {}", e);
                    }
                }

                // Clean up expired advertisements
                let mut games = discovered_games.write().await;
                let mut expired_games = Vec::new();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                for (game_id, ad) in games.iter() {
                    if now > ad.created_at + 300 {
                        // 5 minute TTL
                        expired_games.push(*game_id);
                    }
                }

                for game_id in expired_games {
                    games.remove(&game_id);
                }

                // Also clean up our own advertisements
                let mut our_games = advertised_games.write().await;
                let mut our_expired = Vec::new();

                for (game_id, ad) in our_games.iter() {
                    if now > ad.created_at + 300 {
                        our_expired.push(*game_id);
                    }
                }

                for game_id in our_expired {
                    our_games.remove(&game_id);
                    let _ = event_tx.send(OrchestratorEvent::PlayerLeft {
                        game_id,
                        player: identity.peer_id,
                        reason: "Advertisement expired".to_string(),
                    });
                }

                stats.write().await.games_discovered = games.len() as u64;
            }
        });

        log::debug!("Started game discovery task");
    }

    /// Start command processor - handles orchestrator commands
    async fn start_command_processor(&self) {
        let command_rx = self.command_rx.clone();
        let orchestrator_weak = Arc::downgrade(&Arc::new(self.clone()));

        tokio::spawn(async move {
            let mut receiver = command_rx.lock().await;

            while let Some(command) = receiver.recv().await {
                if let Some(orchestrator) = orchestrator_weak.upgrade() {
                    if let Err(e) = orchestrator.process_command(command).await {
                        log::error!("Failed to process orchestrator command: {}", e);
                    }
                } else {
                    break; // Orchestrator has been dropped
                }
            }
        });

        log::debug!("Started command processor");
    }

    /// Start turn timeout monitor - ensures turns don't hang indefinitely
    async fn start_turn_timeout_monitor(&self) {
        let active_sessions = self.active_sessions.clone();
        let event_tx = self.event_tx.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut timeout_timer = interval(Duration::from_secs(5));

            loop {
                timeout_timer.tick().await;

                let sessions = active_sessions.read().await;
                let mut timeouts = Vec::new();

                for (game_id, session) in sessions.iter() {
                    let turn_manager = session.turn_manager.lock().await;
                    if turn_manager.is_turn_expired() {
                        if let Some(current_player) = turn_manager.current_player {
                            timeouts.push((*game_id, current_player));
                        }
                    }
                }
                drop(sessions);

                // Process timeouts
                for (game_id, timed_out_player) in timeouts {
                    let _ = event_tx.send(OrchestratorEvent::TurnTimeout {
                        game_id,
                        player: timed_out_player,
                    });

                    // Advance to next player
                    if let Some(session) = active_sessions.read().await.get(&game_id) {
                        let mut turn_manager = session.turn_manager.lock().await;
                        let next_player = turn_manager.next_turn();

                        let _ = event_tx.send(OrchestratorEvent::TurnChanged {
                            game_id,
                            current_player: next_player,
                            previous_player: Some(timed_out_player),
                        });
                    }

                    stats.write().await.conflicts_resolved += 1;
                    log::warn!(
                        "Turn timeout for player {:?} in game {:?}",
                        timed_out_player,
                        game_id
                    );
                }
            }
        });

        log::debug!("Started turn timeout monitor");
    }

    /// Start state synchronization task - keeps game state consistent
    async fn start_state_sync_task(&self) {
        let active_sessions = self.active_sessions.clone();
        let consensus_manager = self.consensus_manager.clone();
        let mesh_service = self.mesh_service.clone();
        let identity = self.identity.clone();
        let sync_interval = self.config.state_sync_interval;

        tokio::spawn(async move {
            let mut sync_timer = interval(sync_interval);

            loop {
                sync_timer.tick().await;

                let sessions = active_sessions.read().await;

                for (game_id, session) in sessions.iter() {
                    // Get consensus state from manager
                    if let Some(consensus_session) = consensus_manager.get_game_state(game_id).await
                    {
                        // Update local state snapshot
                        let mut state = session.state_snapshot.write().await;

                        // Sync betting history
                        let bet_history = session.bet_history.read().await;
                        state.active_bets = bet_history.clone();

                        // Sync turn information
                        let turn_manager = session.turn_manager.lock().await;
                        state.current_turn = turn_manager.current_player;

                        if let Some(started) = turn_manager.turn_started {
                            let deadline = started + turn_manager.turn_timeout;
                            // Convert Instant to SystemTime for Unix timestamp
                            let now_instant = Instant::now();
                            let now_system = SystemTime::now();
                            if let Some(deadline_system) = now_system
                                .checked_sub(now_instant.saturating_duration_since(deadline))
                            {
                                state.turn_deadline = Some(
                                    deadline_system
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                );
                            } else if let Some(deadline_system) = now_system
                                .checked_add(deadline.saturating_duration_since(now_instant))
                            {
                                state.turn_deadline = Some(
                                    deadline_system
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                );
                            }
                        }

                        *session.last_activity.write().await = Instant::now();
                    }
                }
            }
        });

        log::debug!("Started state sync task");
    }

    /// Start conflict resolver - handles state disagreements between peers
    async fn start_conflict_resolver(&self) {
        let active_sessions = self.active_sessions.clone();
        let consensus_manager = self.consensus_manager.clone();
        let event_tx = self.event_tx.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            let mut conflict_timer = interval(Duration::from_secs(10));

            loop {
                conflict_timer.tick().await;

                let sessions = active_sessions.read().await;

                for (game_id, session) in sessions.iter() {
                    // Check for consensus failures or state disagreements
                    if let Some(consensus_session) = consensus_manager.get_game_state(game_id).await
                    {
                        // Compare local state with consensus state
                        let local_state = session.state_snapshot.read().await;
                        let consensus_state = &consensus_session.consensus_state;

                        // Check for conflicts (simplified - would need more sophisticated detection)
                        let participants_match =
                            session.participants.len() == consensus_session.participants.len();

                        if !participants_match {
                            let conflicting_peers = consensus_session.participants.clone();

                            let _ = event_tx.send(OrchestratorEvent::StateConflictDetected {
                                game_id: *game_id,
                                conflicting_peers,
                            });

                            // Attempt resolution by syncing with majority
                            // In production, this would use consensus voting
                            log::warn!("State conflict detected in game {:?}", game_id);
                            stats.write().await.conflicts_resolved += 1;
                        }
                    }
                }
            }
        });

        log::debug!("Started conflict resolver");
    }

    /// Process orchestrator commands
    async fn process_command(&self, command: OrchestratorCommand) -> Result<()> {
        match command {
            OrchestratorCommand::AdvertiseGame {
                config,
                max_players,
            } => {
                let mut game_config = config;
                game_config.player_limit = max_players;
                self.advertise_game(game_config).await?;
            }

            OrchestratorCommand::DiscoverGames { filters } => {
                self.process_game_discovery(filters).await?;
            }

            OrchestratorCommand::JoinGame {
                game_id,
                initial_balance,
            } => {
                self.send_join_request(game_id, initial_balance).await?;
            }

            OrchestratorCommand::LeaveGame { game_id } => {
                self.leave_game_session(game_id).await?;
            }

            OrchestratorCommand::PlaceBet {
                game_id,
                bet_type,
                amount,
            } => {
                self.process_bet_placement(game_id, bet_type, amount)
                    .await?;
            }

            OrchestratorCommand::CommitDiceRoll {
                game_id,
                commitment_hash,
            } => {
                self.commit_dice_roll(game_id, commitment_hash).await?;
            }

            OrchestratorCommand::RevealDiceRoll {
                game_id,
                dice_roll,
                nonce,
            } => {
                self.reveal_dice_roll(game_id, dice_roll, nonce).await?;
            }

            OrchestratorCommand::ForceStateSync { game_id } => {
                self.force_state_synchronization(game_id).await?;
            }

            OrchestratorCommand::ResolveConflict {
                game_id,
                resolution_data,
            } => {
                self.resolve_state_conflict(game_id, resolution_data)
                    .await?;
            }
        }

        Ok(())
    }

    /// Helper methods for command processing
    async fn process_game_discovery(&self, _filters: GameDiscoveryRequest) -> Result<()> {
        // Implementation would broadcast discovery request
        Ok(())
    }

    async fn send_join_request(&self, game_id: GameId, initial_balance: CrapTokens) -> Result<()> {
        // Implementation would send join request to game creator
        self.stats.write().await.join_requests_sent += 1;
        Ok(())
    }

    async fn leave_game_session(&self, game_id: GameId) -> Result<()> {
        // Remove from active sessions
        let mut sessions = self.active_sessions.write().await;
        if sessions.remove(&game_id).is_some() {
            self.stats.write().await.active_games = sessions.len();

            let _ = self.event_tx.send(OrchestratorEvent::PlayerLeft {
                game_id,
                player: self.identity.peer_id,
                reason: "Player left voluntarily".to_string(),
            });
        }
        Ok(())
    }

    async fn process_bet_placement(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount: CrapTokens,
    ) -> Result<()> {
        // Validate bet through consensus
        self.consensus_manager
            .place_bet(game_id, bet_type.clone(), amount)
            .await?;

        // Record bet locally
        if let Some(session) = self.active_sessions.read().await.get(&game_id) {
            let bet_record = BetRecord {
                player: self.identity.peer_id,
                bet_type,
                amount,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };

            session.bet_history.write().await.push(bet_record.clone());
            *session.last_activity.write().await = Instant::now();

            let _ = self.event_tx.send(OrchestratorEvent::BetValidated {
                game_id,
                bet: bet_record,
            });
            self.stats.write().await.bets_validated += 1;
        }

        Ok(())
    }

    async fn commit_dice_roll(&self, game_id: GameId, commitment_hash: [u8; 32]) -> Result<()> {
        let round_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(session) = self.active_sessions.read().await.get(&game_id) {
            let commit = DiceCommit {
                game_id,
                round_id,
                roller: self.identity.peer_id,
                commitment_hash,
                timestamp: round_id,
            };

            session
                .pending_commits
                .write()
                .await
                .entry(round_id)
                .or_insert_with(Vec::new)
                .push(commit);

            let _ = self.event_tx.send(OrchestratorEvent::DiceCommitted {
                game_id,
                round_id,
                roller: self.identity.peer_id,
            });

            self.stats.write().await.dice_commits += 1;
        }

        Ok(())
    }

    async fn reveal_dice_roll(
        &self,
        game_id: GameId,
        dice_roll: DiceRoll,
        nonce: [u8; 32],
    ) -> Result<()> {
        let round_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(session) = self.active_sessions.read().await.get(&game_id) {
            let reveal = DiceReveal {
                game_id,
                round_id,
                roller: self.identity.peer_id,
                dice_roll,
                nonce,
                signature: Vec::new(), // Would be signed in production
            };

            session
                .pending_reveals
                .write()
                .await
                .entry(round_id)
                .or_insert_with(Vec::new)
                .push(reveal);

            // Process through consensus manager
            self.consensus_manager.roll_dice(game_id).await?;

            let _ = self.event_tx.send(OrchestratorEvent::DiceRevealed {
                game_id,
                round_id,
                roll: dice_roll,
            });

            self.stats.write().await.dice_reveals += 1;
        }

        Ok(())
    }

    async fn force_state_synchronization(&self, _game_id: GameId) -> Result<()> {
        // Force immediate state sync
        Ok(())
    }

    async fn resolve_state_conflict(
        &self,
        _game_id: GameId,
        _resolution_data: Vec<u8>,
    ) -> Result<()> {
        // Resolve consensus conflict
        Ok(())
    }

    /// Helper method to create discovery packet
    fn create_discovery_packet(
        identity: &BitchatIdentity,
        request: &GameDiscoveryRequest,
    ) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(request).map_err(|e| {
            Error::Serialization(format!("Failed to serialize discovery request: {}", e))
        })?;

        Ok(serialized)
    }

    /// Helper method to broadcast via mesh
    async fn broadcast_via_mesh(_mesh_service: &Arc<MeshService>, _packet: Vec<u8>) -> Result<()> {
        // Would integrate with actual mesh service
        Ok(())
    }

    /// Get orchestrator statistics
    pub async fn get_stats(&self) -> OrchestratorStats {
        self.stats.read().await.clone()
    }

    /// Get list of discovered games
    pub async fn get_discovered_games(&self) -> Vec<GameAdvertisement> {
        self.discovered_games
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// Get list of active sessions
    pub async fn get_active_sessions(&self) -> Vec<GameId> {
        self.active_sessions.read().await.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;
    use crate::mesh::ConsensusMessageHandler;
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_game_orchestrator_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity.clone(), transport));
        let consensus_handler = Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            identity.clone(),
            Default::default(),
        ));
        let consensus_manager = Arc::new(ConsensusGameManager::new(
            identity.clone(),
            mesh_service.clone(),
            consensus_handler,
            Default::default(),
        ));

        let config = OrchestratorConfig::default();
        let (orchestrator, _command_tx, _event_rx) =
            GameOrchestrator::new(identity, mesh_service, consensus_manager, config);

        let stats = orchestrator.get_stats().await;
        assert_eq!(stats.active_games, 0);
        assert_eq!(stats.games_advertised, 0);
    }

    #[tokio::test]
    async fn test_game_advertisement() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(crate::crypto::BitchatIdentity::from_keypair_with_pow(
            keypair, 8,
        ));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh_service = Arc::new(MeshService::new(identity.clone(), transport));
        let consensus_handler = Arc::new(ConsensusMessageHandler::new(
            mesh_service.clone(),
            identity.clone(),
            Default::default(),
        ));
        let consensus_manager = Arc::new(ConsensusGameManager::new(
            identity.clone(),
            mesh_service.clone(),
            consensus_handler,
            Default::default(),
        ));

        let config = OrchestratorConfig::default();
        let (orchestrator, _command_tx, _event_rx) =
            GameOrchestrator::new(identity, mesh_service, consensus_manager, config);

        let game_config = GameConfig {
            game_type: "craps".to_string(),
            min_bet: 10,
            max_bet: 1000,
            player_limit: 8,
            timeout_seconds: 60,
            consensus_threshold: 0.67,
            allow_spectators: true,
        };

        let result = orchestrator.advertise_game(game_config).await;
        assert!(result.is_ok());

        let stats = orchestrator.get_stats().await;
        assert_eq!(stats.games_advertised, 1);
        assert_eq!(stats.active_games, 1);
    }
}
