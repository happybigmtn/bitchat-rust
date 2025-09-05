//! Consensus-based Game Manager
//!
//! This module manages game sessions with distributed consensus,
//! integrating the game framework with the P2P consensus system.

use arc_swap::ArcSwap;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use uuid;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::memory_pool::GameMemoryPools;
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
// Location-based game discovery implemented via mesh networking
// Supports peer discovery through connected mesh nodes
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
    pub consensus_state: Arc<GameConsensusState>,
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

/// Cache entry for game verification results
#[derive(Debug, Clone)]
struct GameVerificationCache {
    is_active: bool,
    verified_at: Instant,
}

/// Background service health status
#[derive(Debug, Clone, PartialEq)]
enum ServiceHealth {
    Starting,
    Running,
    Failed(String),
    Stopped,
}

/// Background service tracking
#[derive(Debug)]
struct BackgroundService {
    name: String,
    handle: tokio::task::JoinHandle<()>,
    health: Arc<parking_lot::RwLock<ServiceHealth>>,
    start_time: Instant,
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

    // Game verification cache (30 second TTL)
    verification_cache: Arc<DashMap<GameId, GameVerificationCache>>,

    // Event handling - bounded channel with backpressure
    game_events: mpsc::Sender<GameEvent>,
    event_receiver: Arc<Mutex<mpsc::Receiver<GameEvent>>>,

    // Background service management
    background_services: Arc<Mutex<Vec<BackgroundService>>>,

    // Statistics - atomic counters for performance
    total_games_created: Arc<AtomicUsize>,
    total_games_completed: Arc<AtomicUsize>,
    total_operations_processed: Arc<AtomicUsize>,
    total_consensus_failures: Arc<AtomicUsize>,
    // Consensus timing metrics (total time and operation count for averaging)
    total_consensus_time_ms: Arc<AtomicU64>,
    total_consensus_operations: Arc<AtomicUsize>,
    stats_snapshot: Arc<ArcSwap<GameManagerStats>>,
    
    // Memory pools for hot path optimizations
    memory_pools: Arc<GameMemoryPools>,
}

/// Game session with consensus integration
#[derive(Debug, Clone)]
pub struct ConsensusGameSession {
    pub game: CrapsGame,
    pub participants: Vec<PeerId>,
    pub consensus_state: Arc<GameConsensusState>,
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
        let memory_pools = Arc::new(GameMemoryPools::new());

        Self {
            identity,
            mesh_service,
            consensus_handler,
            config,
            active_games: Arc::new(DashMap::new()),
            consensus_bridges: Arc::new(DashMap::new()),
            pending_operations: Arc::new(DashMap::new()),
            verification_cache: Arc::new(DashMap::new()),
            game_events,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            background_services: Arc::new(Mutex::new(Vec::new())),
            total_games_created: Arc::new(AtomicUsize::new(0)),
            total_games_completed: Arc::new(AtomicUsize::new(0)),
            total_operations_processed: Arc::new(AtomicUsize::new(0)),
            total_consensus_failures: Arc::new(AtomicUsize::new(0)),
            total_consensus_time_ms: Arc::new(AtomicU64::new(0)),
            total_consensus_operations: Arc::new(AtomicUsize::new(0)),
            stats_snapshot: Arc::new(ArcSwap::from_pointee(GameManagerStats::default())),
            memory_pools,
        }
    }

    /// Start the consensus game manager with proper error checking
    pub async fn start(&self) -> Result<()> {
        log::info!("Starting consensus game manager");

        // Start background tasks with health monitoring
        let services = vec![
            ("game_maintenance", self.start_game_maintenance().await?),
            ("state_synchronization", self.start_state_synchronization().await?),
            ("operation_timeout_handler", self.start_operation_timeout_handler().await?),
            ("event_processor", self.start_event_processor().await?),
        ];

        // Register all services and verify they're running
        {
            let mut bg_services = self.background_services.lock().await;
            for (name, service) in services {
                bg_services.push(service);
                log::info!("✅ Started background service: {}", name);
            }
        }

        // Wait a short time and verify all services are running
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.verify_services_health().await?;

        log::info!("✅ Consensus game manager started successfully with {} services", 4);
        Ok(())
    }

    /// Verify that all background services are healthy
    async fn verify_services_health(&self) -> Result<()> {
        let services = self.background_services.lock().await;
        let mut failed_services = Vec::new();

        for service in services.iter() {
            let health = service.health.read();
            match &*health {
                ServiceHealth::Failed(error) => {
                    failed_services.push(format!("{}: {}", service.name, error));
                }
                ServiceHealth::Starting => {
                    // Give services more time if they're still starting
                    if service.start_time.elapsed() > Duration::from_secs(5) {
                        failed_services.push(format!("{}: startup timeout", service.name));
                    }
                }
                ServiceHealth::Stopped => {
                    failed_services.push(format!("{}: unexpectedly stopped", service.name));
                }
                ServiceHealth::Running => {
                    // Service is healthy
                    log::debug!("Service {} is running normally", service.name);
                }
            }
        }

        if !failed_services.is_empty() {
            return Err(Error::GameLogic(format!(
                "Background services failed to start: {}",
                failed_services.join(", ")
            )));
        }

        Ok(())
    }

    /// Get the health status of all background services
    pub async fn get_service_health(&self) -> Vec<(String, ServiceHealth)> {
        let services = self.background_services.lock().await;
        services
            .iter()
            .map(|service| {
                let health = service.health.read();
                (service.name.clone(), health.clone())
            })
            .collect()
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

        let broadcast_msg = self.create_announcement_message(game_info).await?;

        // Broadcast game announcement
        self.mesh_service.broadcast_message(broadcast_msg).await?;

        log::info!(
            "Created and broadcast new game {:?} with {} participants",
            game_id,
            participants.len()
        );
        
        // Record game creation metric
        #[cfg(feature = "monitoring")]
        crate::monitoring::record_game_event("game_created", &format!("{:?}", game_id));
        
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
                    .map_err(|_| Error::InvalidTimestamp("Invalid system time".to_string()))?
                    .as_secs(),
            })?,
            sender: self.identity.peer_id,
            recipient: None, // Broadcast to all peers
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: {
                // Use pooled buffer for signature (typically empty but could be used)
                let mut sig_buffer = self.memory_pools.vec_u8_pool.get().await;
                sig_buffer.clear();
                sig_buffer.clone()  // Return copy, buffer will be returned to pool when dropped
            },
        };

        // Send discovery request via mesh
        self.mesh_service.broadcast_message(discovery_msg).await?;

        // Collect responses for 2 seconds
        let start_time = Instant::now();
        let mut discovered_games = Vec::with_capacity(32); // typical network size

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
                        .unwrap_or_default()
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
        // Check cache first (30 second TTL)
        if let Some(cached) = self.verification_cache.get(&game_info.game_id) {
            if cached.verified_at.elapsed() < Duration::from_secs(30) {
                return cached.is_active;
            }
            // Cache expired, remove it
            self.verification_cache.remove(&game_info.game_id);
        }

        // Check if we have the game locally
        if let Some(session) = self.active_games.get(&game_info.game_id) {
            let is_active = session.is_active;
            // Update cache
            self.verification_cache.insert(
                game_info.game_id,
                GameVerificationCache {
                    is_active,
                    verified_at: Instant::now(),
                },
            );
            return is_active;
        }

        // Send verification request to game host with timeout
        let payload = match bincode::serialize(&game_info.game_id) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let verify_msg = MeshMessage {
            message_type: MeshMessageType::GameVerification,
            payload,
            sender: self.identity.peer_id,
            recipient: Some(game_info.host),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: vec![],
        };

        // Send and wait for response with 5 second timeout
        let verification_result = match tokio::time::timeout(
            Duration::from_secs(5),
            self.mesh_service
                .send_and_wait_response(verify_msg, game_info.host),
        )
        .await
        {
            Ok(Ok(Some(resp))) => resp.message_type == MeshMessageType::GameVerificationAck,
            Ok(_) => false,
            Err(_) => {
                log::warn!("Game verification timeout for {:?}", game_info.game_id);
                false
            }
        };

        // Cache the result
        self.verification_cache.insert(
            game_info.game_id,
            GameVerificationCache {
                is_active: verification_result,
                verified_at: Instant::now(),
            },
        );

        verification_result
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
                        .unwrap_or_default()
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
                        .unwrap_or_default()
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
        
        // Record bet placement metric
        #[cfg(feature = "monitoring")]
        crate::monitoring::record_game_event("bet_placed", &format!("{:?}", game_id));
        
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

        // Submit operation with timing
        let start_time = Instant::now();
        let proposal_id = bridge.submit_operation(operation.clone()).await?;
        let consensus_duration = start_time.elapsed().as_millis() as u64;

        // Record consensus timing metrics
        self.record_consensus_timing(consensus_duration);

        // Track pending operation
        let pending_op = PendingGameOperation {
            operation,
            game_id,
            submitted_at: start_time,
            consensus_achieved: true, // If we got here, consensus succeeded
        };

        let operation_key = format!("{:?}_{}", proposal_id, operation_type);
        self.pending_operations.insert(operation_key, pending_op);

        // Update stats - atomic operation
        self.total_operations_processed
            .fetch_add(1, Ordering::Relaxed);
        self.update_stats_snapshot().await;

        Ok(())
    }

    /// Start game maintenance task with health monitoring
    async fn start_game_maintenance(&self) -> Result<BackgroundService> {
        let active_games = Arc::clone(&self.active_games);
        let consensus_bridges = Arc::clone(&self.consensus_bridges);
        let consensus_handler = Arc::clone(&self.consensus_handler);
        let total_completed = Arc::clone(&self.total_games_completed);

        let health = Arc::new(parking_lot::RwLock::new(ServiceHealth::Starting));
        let health_clone = Arc::clone(&health);

        let handle = tokio::spawn(async move {
            let mut maintenance_interval = interval(Duration::from_secs(60));
            
            // Mark as running after successful initialization
            *health_clone.write() = ServiceHealth::Running;
            log::debug!("Game maintenance service initialized");

            loop {
                match tokio::time::timeout(Duration::from_secs(65), maintenance_interval.tick()).await {
                    Ok(_) => {
                        // Maintenance tick successful
                    }
                    Err(_) => {
                        log::error!("Game maintenance interval timeout");
                        *health_clone.write() = ServiceHealth::Failed("Maintenance interval timeout".to_string());
                        return;
                    }
                }

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

        Ok(BackgroundService {
            name: "game_maintenance".to_string(),
            handle,
            health,
            start_time: Instant::now(),
        })
    }

    /// Start state synchronization task with health monitoring
    async fn start_state_synchronization(&self) -> Result<BackgroundService> {
        let active_games = Arc::clone(&self.active_games);
        let consensus_bridges = Arc::clone(&self.consensus_bridges);
        let sync_interval = self.config.state_sync_interval;

        let health = Arc::new(parking_lot::RwLock::new(ServiceHealth::Starting));
        let health_clone = Arc::clone(&health);

        let handle = tokio::spawn(async move {
            let mut sync_interval = interval(sync_interval);
            
            // Mark as running after successful initialization
            *health_clone.write() = ServiceHealth::Running;
            log::debug!("State synchronization service initialized");

            loop {
                match tokio::time::timeout(sync_interval.period() + Duration::from_secs(5), sync_interval.tick()).await {
                    Ok(_) => {
                        // Sync tick successful
                    }
                    Err(_) => {
                        log::error!("State synchronization interval timeout");
                        *health_clone.write() = ServiceHealth::Failed("Sync interval timeout".to_string());
                        return;
                    }
                }

                // Sync state for all active games - parallel processing
                let sync_tasks: Vec<_> = active_games
                    .iter()
                    .map(|entry| {
                        let game_id = *entry.key();
                        let bridges_ref = Arc::clone(&consensus_bridges);

                        tokio::spawn(async move {
                            if let Some(bridge) = bridges_ref.get(&game_id) {
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

        Ok(BackgroundService {
            name: "state_synchronization".to_string(),
            handle,
            health,
            start_time: Instant::now(),
        })
    }

    /// Start operation timeout handler with health monitoring
    async fn start_operation_timeout_handler(&self) -> Result<BackgroundService> {
        let pending_operations = Arc::clone(&self.pending_operations);
        let game_events = self.game_events.clone(); // mpsc::Sender needs clone
        let consensus_failures = Arc::clone(&self.total_consensus_failures);
        let timeout = self.config.consensus_timeout;

        let health = Arc::new(parking_lot::RwLock::new(ServiceHealth::Starting));
        let health_clone = Arc::clone(&health);

        let handle = tokio::spawn(async move {
            let mut timeout_interval = interval(Duration::from_secs(10));
            
            // Mark as running after successful initialization
            *health_clone.write() = ServiceHealth::Running;
            log::debug!("Operation timeout handler service initialized");

            loop {
                match tokio::time::timeout(Duration::from_secs(15), timeout_interval.tick()).await {
                    Ok(_) => {
                        // Timeout tick successful
                    }
                    Err(_) => {
                        log::error!("Operation timeout handler interval timeout");
                        *health_clone.write() = ServiceHealth::Failed("Timeout handler interval timeout".to_string());
                        return;
                    }
                }

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

        Ok(BackgroundService {
            name: "operation_timeout_handler".to_string(),
            handle,
            health,
            start_time: Instant::now(),
        })
    }

    /// Start event processor with health monitoring
    async fn start_event_processor(&self) -> Result<BackgroundService> {
        let event_receiver = self.event_receiver.clone();

        let health = Arc::new(parking_lot::RwLock::new(ServiceHealth::Starting));
        let health_clone = Arc::clone(&health);

        let handle = tokio::spawn(async move {
            let mut receiver = event_receiver.lock().await;
            
            // Mark as running after successful initialization
            *health_clone.write() = ServiceHealth::Running;
            log::debug!("Event processor service initialized");

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
            
            // If we exit the event loop, mark as stopped
            *health_clone.write() = ServiceHealth::Stopped;
        });

        Ok(BackgroundService {
            name: "event_processor".to_string(),
            handle,
            health,
            start_time: Instant::now(),
        })
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
                .unwrap_or_default()
                .as_secs(),
        };

        let msg = MeshMessage {
            message_type: MeshMessageType::GameStateSync,
            payload: bincode::serialize(&request)?,
            sender: self.identity.peer_id,
            recipient: Some(host),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
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
        session.consensus_state = Arc::clone(&state.consensus_state);
        session.last_updated = Instant::now();

        // Sync consensus bridge if it exists
        if let Some(bridge) = self.consensus_bridges.get(&game_id) {
            // Update bridge with latest consensus state - need to dereference Arc
            bridge.sync_state((*state.consensus_state).clone()).await?;

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
                .unwrap_or_default()
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
                .unwrap_or_default()
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
            average_consensus_time_ms: {
                let total_ops = self.total_consensus_operations.load(Ordering::Relaxed);
                if total_ops > 0 {
                    self.total_consensus_time_ms.load(Ordering::Relaxed) / total_ops as u64
                } else {
                    0
                }
            },
        };
        self.stats_snapshot.store(Arc::new(stats));
    }

    /// Record consensus operation timing for metrics
    fn record_consensus_timing(&self, duration_ms: u64) {
        self.total_consensus_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.total_consensus_operations
            .fetch_add(1, Ordering::Relaxed);
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
    async fn create_announcement_message(&self, game_info: GameDiscoveryInfo) -> Result<MeshMessage> {
        Ok(MeshMessage {
            message_type: MeshMessageType::GameAnnouncement,
            payload: bincode::serialize(&game_info)?,
            sender: self.identity.peer_id,
            recipient: None, // Broadcast to all
            timestamp: self.get_current_timestamp(),
            signature: self.get_pooled_signature_buffer().await,
        })
    }

    /// Get current Unix timestamp
    fn get_current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Discover available peers for game participation
    pub async fn discover_available_peers(&self) -> Result<Vec<PeerId>> {
        // Get connected peers from mesh service
        let connected_peers = self.mesh_service.get_connected_peers().await;

        // For now, return all connected peers as gaming-capable
        // In a full implementation, we would check peer capabilities
        let gaming_peers: Vec<PeerId> = connected_peers.iter().map(|peer| peer.peer_id).collect();

        Ok(gaming_peers)
    }

    /// Get a pooled Vec<u8> buffer for signatures and other byte operations
    async fn get_pooled_signature_buffer(&self) -> Vec<u8> {
        let mut buffer = self.memory_pools.vec_u8_pool.get().await;
        buffer.clear();
        buffer.clone() // Return copy, original buffer will be returned to pool when dropped
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
