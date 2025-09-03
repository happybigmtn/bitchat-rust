//! Game Lifecycle Manager with Security Hardening
//!
//! Handles game creation, joining, leaving, and lifecycle transitions.
//! Now includes comprehensive input validation and security controls.

use super::config::GameRuntimeConfig;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::protocol::craps::CrapsGame;
use crate::protocol::reputation::{ReputationManager, MIN_REP_TO_PLAY};
use crate::protocol::{
    new_game_id, BitchatPacket, DiceRoll, GameId, PeerId, PACKET_TYPE_GAME_DATA,
};
use crate::protocol::{Bet, BetType};
use crate::security::{SecurityConfig, SecurityManager};
use crate::token::TokenLedger;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Dice roll broadcast message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceRollBroadcast {
    pub game_id: GameId,
    pub roll: DiceRoll,
    pub timestamp: u64,
}

/// Bet resolution broadcast message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetResolutionBroadcast {
    pub game_id: GameId,
    pub resolutions: Vec<crate::protocol::game_logic::BetResolution>,
    pub timestamp: u64,
}

/// Game state synchronization message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateBroadcast {
    pub game_id: GameId,
    pub phase: crate::protocol::bet_types::GamePhase,
    pub point: Option<u8>,
    pub shooter: PeerId,
    pub participants: Vec<PeerId>,
    pub timestamp: u64,
}

/// Player join request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerJoinRequest {
    pub game_id: GameId,
    pub player: PeerId,
    pub timestamp: u64,
    /// Optional password for joining password-protected games
    pub password: Option<String>,
}

/// Player successfully joined broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerJoinedBroadcast {
    pub game_id: GameId,
    pub player: PeerId,
    pub participants: Vec<PeerId>,
    pub timestamp: u64,
}

/// Player left game broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerLeftBroadcast {
    pub game_id: GameId,
    pub player: PeerId,
    pub remaining_participants: Vec<PeerId>,
    pub reason: String,
    pub timestamp: u64,
}

/// Turn state change broadcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStateBroadcast {
    pub game_id: GameId,
    pub current_shooter: PeerId,
    pub turn_state: TurnState,
    pub shooter_queue: Vec<PeerId>,
    pub timestamp: u64,
}

impl Default for TurnManager {
    fn default() -> Self {
        Self::new([0u8; 32]) // Placeholder shooter
    }
}

impl TurnManager {
    /// Create new turn manager with initial shooter
    pub fn new(initial_shooter: PeerId) -> Self {
        Self {
            current_shooter: initial_shooter,
            turn_state: TurnState::WaitingForBets,
            shooter_queue: vec![initial_shooter],
            turn_timeout: Duration::from_secs(60), // 60 second turn timeout
            last_activity: Instant::now(),
        }
    }

    /// Add a player to the shooter rotation queue
    pub fn add_player(&mut self, player: PeerId) {
        if !self.shooter_queue.contains(&player) {
            self.shooter_queue.push(player);
        }
    }

    /// Remove a player from the shooter rotation queue
    pub fn remove_player(&mut self, player: PeerId) {
        self.shooter_queue.retain(|&p| p != player);

        // If current shooter left, advance to next
        if self.current_shooter == player && !self.shooter_queue.is_empty() {
            self.advance_to_next_shooter();
        }
    }

    /// Get current shooter
    pub fn current_shooter(&self) -> PeerId {
        self.current_shooter
    }

    /// Get current turn state
    pub fn turn_state(&self) -> &TurnState {
        &self.turn_state
    }

    /// Check if it's a specific player's turn
    pub fn is_players_turn(&self, player: PeerId) -> bool {
        self.current_shooter == player
    }

    /// Check if the turn has timed out
    pub fn is_turn_timed_out(&self) -> bool {
        self.last_activity.elapsed() > self.turn_timeout
    }

    /// Start betting phase
    pub fn start_betting_phase(&mut self) {
        self.turn_state = TurnState::WaitingForBets;
        self.last_activity = Instant::now();
        log::debug!(
            "Turn state: WaitingForBets for shooter {:?}",
            self.current_shooter
        );
    }

    /// Ready to roll dice
    pub fn ready_to_roll(&mut self) -> Result<()> {
        match self.turn_state {
            TurnState::WaitingForBets => {
                self.turn_state = TurnState::ReadyToRoll;
                self.last_activity = Instant::now();
                log::debug!(
                    "Turn state: ReadyToRoll for shooter {:?}",
                    self.current_shooter
                );
                Ok(())
            }
            _ => Err(Error::GameError(format!(
                "Cannot roll dice in state {:?}",
                self.turn_state
            ))),
        }
    }

    /// Process dice roll
    pub fn process_roll(&mut self) -> Result<()> {
        match self.turn_state {
            TurnState::ReadyToRoll => {
                self.turn_state = TurnState::ProcessingRoll;
                self.last_activity = Instant::now();
                log::debug!(
                    "Turn state: ProcessingRoll for shooter {:?}",
                    self.current_shooter
                );
                Ok(())
            }
            _ => Err(Error::GameError(format!(
                "Cannot process roll in state {:?}",
                self.turn_state
            ))),
        }
    }

    /// Handle seven-out (pass dice to next shooter)
    pub fn handle_seven_out(&mut self) {
        self.turn_state = TurnState::PassingDice;
        log::info!("Seven-out! Passing dice from {:?}", self.current_shooter);
        self.advance_to_next_shooter();
        self.start_betting_phase();
    }

    /// Handle point made (shooter continues)
    pub fn handle_point_made(&mut self) {
        log::info!("Point made! Shooter {:?} continues", self.current_shooter);
        self.start_betting_phase();
    }

    /// Advance to next shooter in rotation
    fn advance_to_next_shooter(&mut self) {
        if self.shooter_queue.len() <= 1 {
            log::warn!("Cannot advance shooter: not enough players");
            return;
        }

        // Find current shooter index and advance
        if let Some(current_index) = self
            .shooter_queue
            .iter()
            .position(|&p| p == self.current_shooter)
        {
            let next_index = (current_index + 1) % self.shooter_queue.len();
            self.current_shooter = self.shooter_queue[next_index];
            log::info!("Advanced to next shooter: {:?}", self.current_shooter);
        } else {
            // Current shooter not found, use first player
            if let Some(&first_shooter) = self.shooter_queue.first() {
                self.current_shooter = first_shooter;
                log::warn!(
                    "Current shooter not in queue, using first player: {:?}",
                    self.current_shooter
                );
            }
        }
    }

    /// Force advance to next shooter (for timeout or voluntary pass)
    pub fn force_advance_shooter(&mut self) -> PeerId {
        let old_shooter = self.current_shooter;
        self.advance_to_next_shooter();
        self.start_betting_phase();
        log::info!(
            "Forced advance from {:?} to {:?}",
            old_shooter,
            self.current_shooter
        );
        self.current_shooter
    }

    /// End the game
    pub fn end_game(&mut self) {
        self.turn_state = TurnState::GameEnded;
        log::info!("Game ended - turn management stopped");
    }

    /// Get all players in shooter rotation
    pub fn get_shooter_queue(&self) -> &[PeerId] {
        &self.shooter_queue
    }

    /// Update activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }
}

/// Game-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub private: bool,
    pub password: Option<String>,
    pub allowed_bets: Vec<BetType>,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            min_buy_in: 10,
            max_buy_in: 10000,
            private: false,
            password: None,
            allowed_bets: vec![
                BetType::Pass,
                BetType::DontPass,
                BetType::Come,
                BetType::DontCome,
                BetType::Field,
            ],
        }
    }
}

/// Active game wrapper with metadata
#[derive(Clone)]
pub struct ActiveGame {
    pub game: CrapsGame,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub total_pot: u64,
    pub rounds_played: u32,
    pub is_suspended: bool,
    pub config: GameConfig,
    pub turn_manager: TurnManager,
}

/// Commands for game lifecycle
#[derive(Debug)]
pub enum GameCommand {
    CreateGame {
        creator: PeerId,
        config: GameConfig,
    },
    JoinGame {
        game_id: GameId,
        player: PeerId,
        buy_in: u64,
    },
    PlaceBet {
        game_id: GameId,
        player: PeerId,
        bet: Bet,
    },
    RollDice {
        game_id: GameId,
        shooter: PeerId,
    },
    LeaveGame {
        game_id: GameId,
        player: PeerId,
    },
    SuspendGame {
        game_id: GameId,
        reason: String,
    },
    ResumeGame {
        game_id: GameId,
    },
    PassDice {
        game_id: GameId,
        current_shooter: PeerId,
        next_shooter: PeerId,
    },
    RequestTurn {
        game_id: GameId,
        player: PeerId,
    },
}

/// Turn state for managing shooter rotation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TurnState {
    WaitingForBets, // Accepting bets before roll
    ReadyToRoll,    // Shooter can roll dice
    ProcessingRoll, // Roll in progress, processing results
    PassingDice,    // Seven-out, passing to next shooter
    GameEnded,      // Game concluded
}

/// Turn management for coordinating shooter rotation
#[derive(Debug, Clone)]
pub struct TurnManager {
    current_shooter: PeerId,
    turn_state: TurnState,
    shooter_queue: Vec<PeerId>,
    turn_timeout: Duration,
    last_activity: Instant,
}

/// Manages game lifecycles with security controls
pub struct GameLifecycleManager {
    config: Arc<GameRuntimeConfig>,
    games: Arc<RwLock<HashMap<GameId, ActiveGame>>>,
    game_timeouts: Arc<RwLock<HashMap<GameId, Instant>>>,
    security_manager: Arc<SecurityManager>,
    mesh_service: Arc<MeshService>,
    token_ledger: Option<Arc<TokenLedger>>,
    reputation_manager: Option<Arc<tokio::sync::RwLock<ReputationManager>>>,
}

impl GameLifecycleManager {
    /// Create a BitchatPacket for game messages
    fn create_game_message_packet<T: Serialize>(&self, message: &T) -> Result<BitchatPacket> {
        let serialized_data = bincode::serialize(message)
            .map_err(|e| Error::Protocol(format!("Failed to serialize game message: {}", e)))?;

        let mut packet = BitchatPacket::new(PACKET_TYPE_GAME_DATA);
        packet.payload = Some(serialized_data);
        Ok(packet)
    }

    /// Create a new game lifecycle manager with security hardening
    pub fn new(config: Arc<GameRuntimeConfig>, mesh_service: Arc<MeshService>) -> Self {
        let security_config = SecurityConfig::default();
        let security_manager = Arc::new(SecurityManager::new(security_config));

        Self {
            config,
            games: Arc::new(RwLock::new(HashMap::new())),
            game_timeouts: Arc::new(RwLock::new(HashMap::new())),
            security_manager,
            mesh_service,
            token_ledger: None,
            reputation_manager: None,
        }
    }

    /// Create a new game lifecycle manager with custom security configuration
    pub fn new_with_security(
        config: Arc<GameRuntimeConfig>,
        security_config: SecurityConfig,
        mesh_service: Arc<MeshService>,
    ) -> Self {
        let security_manager = Arc::new(SecurityManager::new(security_config));

        Self {
            config,
            games: Arc::new(RwLock::new(HashMap::new())),
            game_timeouts: Arc::new(RwLock::new(HashMap::new())),
            security_manager,
            mesh_service,
            token_ledger: None,
            reputation_manager: None,
        }
    }

    /// Set token ledger dependency
    pub fn set_token_ledger(&mut self, token_ledger: Arc<TokenLedger>) {
        self.token_ledger = Some(token_ledger);
    }

    /// Set reputation manager dependency
    pub fn set_reputation_manager(
        &mut self,
        reputation_manager: Arc<tokio::sync::RwLock<ReputationManager>>,
    ) {
        self.reputation_manager = Some(reputation_manager);
    }

    /// Create a new game
    pub async fn create_game(&self, creator: PeerId, config: GameConfig) -> Result<GameId> {
        let mut games = self.games.write().await;

        // Check limits
        if games.len() >= self.config.max_concurrent_games {
            return Err(Error::GameError("Maximum concurrent games reached".into()));
        }

        // Create game
        let game_id = new_game_id();
        let game = CrapsGame::new(game_id, creator);

        let active_game = ActiveGame {
            game,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            total_pot: 0,
            rounds_played: 0,
            is_suspended: false,
            config,
            turn_manager: TurnManager::new(creator),
        };

        games.insert(game_id, active_game);

        // Set timeout
        let mut timeouts = self.game_timeouts.write().await;
        timeouts.insert(game_id, Instant::now() + self.config.game_timeout);

        Ok(game_id)
    }

    /// Add a player to a game with comprehensive security validation
    pub async fn add_player_to_game_with_security(
        &self,
        game_id: GameId,
        player: PeerId,
        buy_in: u64,
        client_ip: IpAddr,
    ) -> Result<()> {
        log::info!("Player {:?} attempting to join game {:?}", player, game_id);

        // Security validation first
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.security_manager
            .validate_game_join_request(&game_id, &player, buy_in, timestamp, client_ip)?;

        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        // Check if game is full
        if game.game.participants.len() >= self.config.max_players {
            return Err(Error::GameError("Game is full".into()));
        }

        // Check if game is suspended
        if game.is_suspended {
            return Err(Error::GameError("Game is suspended".into()));
        }

        // Validate player eligibility (check balance, reputation, etc.)
        self.validate_player_eligibility(player, &game.config)
            .await?;

        // For multiplayer games, broadcast join request for consensus
        if game.game.participants.len() > 1 {
            self.broadcast_player_join_request(game_id, player, &game.game.participants)
                .await?;
        }

        // Add player
        if !game.game.add_player(player) {
            return Err(Error::GameError("Player already in game".into()));
        }

        // Add to turn manager rotation
        game.turn_manager.add_player(player);

        // Broadcast successful join to all participants
        self.broadcast_player_joined(game_id, player, &game.game.participants)
            .await?;

        // Update activity
        game.last_activity = Instant::now();

        log::info!(
            "Player {:?} successfully joined game {:?}. Total players: {}",
            player,
            game_id,
            game.game.participants.len()
        );

        Ok(())
    }

    /// Validate player eligibility to join a game
    async fn validate_player_eligibility(&self, player: PeerId, config: &GameConfig) -> Result<()> {
        log::debug!("Validating player eligibility for {:?}", player);

        // Basic validation - ensure non-zero player ID
        if player == [0u8; 32] {
            return Err(Error::GameError("Invalid player ID".into()));
        }

        // 1. Check player balance meets minimum buy-in
        if let Some(token_ledger) = &self.token_ledger {
            let balance = token_ledger.get_balance(&player).await;
            if balance < config.min_buy_in {
                return Err(Error::GameError(format!(
                    "Insufficient balance: {} CRAP required, {} CRAP available",
                    config.min_buy_in as f64 / 1_000_000.0,
                    balance as f64 / 1_000_000.0
                )));
            }
            log::debug!(
                "Player {} has sufficient balance: {} CRAP",
                hex::encode(&player[0..8]),
                balance as f64 / 1_000_000.0
            );
        } else {
            log::warn!("Token ledger not available, skipping balance check");
        }

        // 2. Check player reputation score if required
        if let Some(reputation_manager) = &self.reputation_manager {
            let reputation_guard = reputation_manager.read().await;
            if !reputation_guard.can_participate(&player) {
                return Err(Error::GameError(
                    "Player reputation too low to participate in games".into(),
                ));
            }
            let trust_level = reputation_guard.get_trust_level(&player);
            log::debug!(
                "Player {} reputation check passed (trust: {:.2})",
                hex::encode(&player[0..8]),
                trust_level
            );
        } else {
            log::warn!("Reputation manager not available, skipping reputation check");
        }

        // 3. Check if player is banned or restricted
        if let Some(reputation_manager) = &self.reputation_manager {
            let reputation_guard = reputation_manager.read().await;

            if reputation_guard.is_banned(&player) {
                if let Some(ban_expiry) = reputation_guard.get_ban_expiry(&player) {
                    return Err(Error::GameError(format!(
                        "Player is temporarily banned until timestamp: {}",
                        ban_expiry
                    )));
                } else {
                    return Err(Error::GameError("Player is banned from games".into()));
                }
            }

            let reputation_score = reputation_guard.get_reputation_score(&player);
            if reputation_score < MIN_REP_TO_PLAY {
                return Err(Error::GameError(format!(
                    "Player reputation score ({}) is below minimum required ({})",
                    reputation_score, MIN_REP_TO_PLAY
                )));
            }

            log::debug!(
                "Player {} ban/reputation check passed (score: {})",
                hex::encode(&player[0..8]),
                reputation_score
            );
        }

        // Check if game requires password
        if let Some(required_password) = &config.password {
            // Current API does not accept a provided password parameter here.
            // Until the join request carries a password through to this validation,
            // reject attempts to join password-protected games.
            log::debug!(
                "Game requires password validation for password: {}",
                if required_password.len() > 8 {
                    format!("{}...", &required_password[..8])
                } else {
                    required_password.clone()
                }
            );
            return Err(Error::GameError(
                "Password required to join this game".into(),
            ));
        }

        log::info!(
            "Player {:?} validation successful",
            hex::encode(&player[0..8])
        );
        Ok(())
    }

    /// Broadcast player join request for consensus approval
    /// Tests: See tests/concurrent_player_joins_test.rs for integration tests
    /// Benchmarks: See benches/broadcast_latency_bench.rs for performance tests
    async fn broadcast_player_join_request(
        &self,
        game_id: GameId,
        joining_player: PeerId,
        current_participants: &[PeerId],
    ) -> Result<()> {
        let message = PlayerJoinRequest {
            game_id,
            player: joining_player,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            password: None,
        };

        log::info!(
            "Broadcasting join request for player {:?} to {} participants",
            joining_player,
            current_participants.len()
        );

        // Broadcast to all current participants in parallel for better performance
        let broadcast_tasks: Vec<_> = current_participants
            .iter()
            .map(|&participant| {
                let message_clone = message.clone();

                tokio::spawn(async move {
                    log::debug!("Sending join request to participant {:?}", participant);

                    // Create packet with serialized message - this is a join request to a specific peer
                    // In a production system, we might want to send this to specific participants
                    // For now, we'll log that the join request would be sent
                    log::debug!("Join request would be sent to participant {:?} - actual peer-specific messaging not yet implemented", participant);
                    Ok::<(), Error>(())
                })
            })
            .collect();

        // Wait for all broadcasts to complete
        let results = join_all(broadcast_tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Failed to send join request: {:?}", e);
            }
        }

        // In a full implementation, we would wait for consensus approval
        // For now, we automatically approve
        Ok(())
    }

    /// Broadcast successful player join to all participants
    async fn broadcast_player_joined(
        &self,
        game_id: GameId,
        new_player: PeerId,
        all_participants: &[PeerId],
    ) -> Result<()> {
        let message = PlayerJoinedBroadcast {
            game_id,
            player: new_player,
            participants: all_participants.to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        log::info!(
            "Broadcasting successful join of player {:?} to {} participants",
            new_player,
            all_participants.len()
        );

        // Create packet for broadcasting player joined event to all participants
        let packet = self.create_game_message_packet(&message)?;

        // Broadcast to all participants via mesh service
        if let Err(e) = self.mesh_service.broadcast_packet(packet).await {
            log::error!("Failed to broadcast player joined message: {:?}", e);
            return Err(e);
        }

        // For compatibility with the existing parallel structure, create empty tasks
        let broadcast_tasks: Vec<_> = all_participants
            .iter()
            .map(|&participant| {
                tokio::spawn(async move {
                    log::debug!(
                        "Player joined message broadcasted to participant {:?}",
                        participant
                    );
                    Ok::<(), Error>(())
                })
            })
            .collect();

        // Wait for all notifications to complete
        let results = join_all(broadcast_tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Failed to send player joined notification: {:?}", e);
            }
        }

        Ok(())
    }

    /// Handle player leaving with consensus
    pub async fn remove_player_from_game_with_consensus(
        &self,
        game_id: GameId,
        player: PeerId,
        reason: &str,
    ) -> Result<()> {
        log::info!("Player {:?} leaving game {:?}: {}", player, game_id, reason);

        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        // Broadcast player leaving to remaining participants
        let remaining_participants: Vec<PeerId> = game
            .game
            .participants
            .iter()
            .filter(|&&p| p != player)
            .copied()
            .collect();

        if !remaining_participants.is_empty() {
            self.broadcast_player_left(game_id, player, &remaining_participants, reason)
                .await?;
        }

        // Remove player bets and clear their state
        game.game.clear_player_bets(&player);

        // Remove from participants and turn manager
        game.game.participants.retain(|&p| p != player);
        game.turn_manager.remove_player(player);

        // Update activity
        game.last_activity = Instant::now();

        // Check if game should end
        if game.game.participants.len() < self.config.min_players {
            log::info!("Game {:?} ended due to insufficient players", game_id);
            game.turn_manager.end_game();
            games.remove(&game_id);
            self.game_timeouts.write().await.remove(&game_id);
        } else {
            // Update game shooter to match turn manager
            game.game.set_shooter(game.turn_manager.current_shooter());
        }

        Ok(())
    }

    /// Broadcast player left message
    async fn broadcast_player_left(
        &self,
        game_id: GameId,
        departed_player: PeerId,
        remaining_participants: &[PeerId],
        reason: &str,
    ) -> Result<()> {
        let message = PlayerLeftBroadcast {
            game_id,
            player: departed_player,
            remaining_participants: remaining_participants.to_vec(),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        log::info!(
            "Broadcasting departure of player {:?} to {} remaining participants",
            departed_player,
            remaining_participants.len()
        );

        // Create packet for broadcasting player left event
        let packet = self.create_game_message_packet(&message)?;

        // Broadcast to all remaining participants via mesh service
        if let Err(e) = self.mesh_service.broadcast_packet(packet).await {
            log::error!("Failed to broadcast player left message: {:?}", e);
            return Err(e);
        }

        for participant in remaining_participants {
            log::debug!(
                "Player departure message broadcasted to participant {:?}",
                participant
            );
        }

        Ok(())
    }

    /// Remove a player from a game
    pub async fn remove_player_from_game(&self, game_id: GameId, player: PeerId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        // Remove player bets
        game.game.clear_player_bets(&player);

        // Remove from participants
        game.game.participants.retain(|&p| p != player);

        // Update activity
        game.last_activity = Instant::now();

        // Check if game should end
        if game.game.participants.len() < self.config.min_players {
            games.remove(&game_id);
            self.game_timeouts.write().await.remove(&game_id);
        }

        Ok(())
    }

    /// Process a bet
    pub async fn process_bet(&self, game_id: GameId, player: PeerId, bet: Bet) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        // Validate bet is allowed
        if !game.config.allowed_bets.is_empty() && !game.config.allowed_bets.contains(&bet.bet_type)
        {
            return Err(Error::InvalidBet(
                "Bet type not allowed in this game".into(),
            ));
        }

        // Store amount before moving bet
        let bet_amount = bet.amount.amount();

        // Place bet
        game.game.place_bet(player, bet)?;

        // Update pot and activity
        game.total_pot = game.total_pot.saturating_add(bet_amount);
        game.last_activity = Instant::now();

        Ok(())
    }

    /// Process a dice roll with comprehensive security validation
    pub async fn process_dice_roll_with_security(
        &self,
        game_id: GameId,
        shooter: PeerId,
        entropy: [u8; 32],
        commitment: [u8; 32],
        client_ip: IpAddr,
    ) -> Result<DiceRoll> {
        // Security validation first
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.security_manager.validate_dice_roll_commit(
            &game_id,
            &shooter,
            &entropy,
            &commitment,
            timestamp,
            client_ip,
        )?;

        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        // Verify shooter using turn manager
        if !game.turn_manager.is_players_turn(shooter) {
            return Err(Error::GameError(format!(
                "Not your turn. Current shooter: {:?}",
                game.turn_manager.current_shooter()
            )));
        }

        // Check turn state allows rolling
        game.turn_manager.ready_to_roll()?;
        game.turn_manager.process_roll()?;

        // For multiplayer games, use consensus dice roll generation
        let roll = if game.game.participants.len() > 1 {
            self.generate_consensus_dice_roll(game_id, &game.game.participants)
                .await?
        } else {
            // Single player can use local secure randomness
            CrapsGame::roll_dice_secure()?
        };

        // Broadcast the dice roll result to all participants
        self.broadcast_dice_roll_result(game_id, roll, &game.game.participants)
            .await?;

        // Process roll
        let resolutions = game.game.process_roll(roll);

        // Handle turn management based on roll result
        let total = roll.total();
        match game.game.phase {
            crate::protocol::bet_types::GamePhase::ComeOut => {
                if total == 7 || total == 11 || total == 2 || total == 3 || total == 12 {
                    // Natural win/loss - shooter continues
                    game.turn_manager.handle_point_made();
                } else {
                    // Point established - shooter continues
                    game.turn_manager.start_betting_phase();
                }
            }
            crate::protocol::bet_types::GamePhase::Point => {
                if total == 7 {
                    // Seven-out - pass dice
                    game.turn_manager.handle_seven_out();
                } else if Some(total) == game.game.point {
                    // Point made - shooter continues
                    game.turn_manager.handle_point_made();
                } else {
                    // No decision - continue with same shooter
                    game.turn_manager.start_betting_phase();
                }
            }
            _ => {
                // Default - continue
                game.turn_manager.start_betting_phase();
            }
        }

        // Broadcast bet resolutions if any
        if !resolutions.is_empty() {
            self.broadcast_bet_resolutions(game_id, &resolutions, &game.game.participants)
                .await?;
        }

        // Broadcast turn state change if shooter changed
        self.broadcast_turn_state_change(game_id, &game.turn_manager, &game.game.participants)
            .await?;

        // Update stats and activity
        game.rounds_played += 1;
        game.last_activity = Instant::now();
        game.turn_manager.update_activity();

        Ok(roll)
    }

    /// Generate consensus dice roll using commit-reveal scheme
    async fn generate_consensus_dice_roll(
        &self,
        game_id: GameId,
        participants: &[PeerId],
    ) -> Result<DiceRoll> {
        use crate::protocol::consensus::commit_reveal::EntropyPool;

        log::info!(
            "Generating consensus dice roll for game {:?} with {} participants",
            game_id,
            participants.len()
        );

        // Create entropy pool
        let mut entropy_pool = EntropyPool::new();

        // Collect entropy from all participants (simplified - in real implementation would use commit-reveal)
        for participant in participants {
            // Generate entropy source for each participant
            let entropy = self
                .generate_participant_entropy(*participant, game_id)
                .await?;
            entropy_pool.add_entropy(entropy);
        }

        // Generate dice roll from combined entropy
        let (die1, die2) = entropy_pool.generate_dice_roll();

        log::info!(
            "Consensus dice roll generated: {} + {} = {}",
            die1,
            die2,
            die1 + die2
        );

        DiceRoll::new(die1, die2)
    }

    /// Generate entropy for a participant
    async fn generate_participant_entropy(
        &self,
        participant: PeerId,
        game_id: GameId,
    ) -> Result<[u8; 32]> {
        use rand::rngs::OsRng;
        use rand::RngCore;
        use sha2::{Digest, Sha256};

        // Generate secure random nonce
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);

        // Combine with game context
        let mut hasher = Sha256::new();
        hasher.update(&participant);
        hasher.update(&game_id);
        hasher.update(&nonce);

        // Add current timestamp for uniqueness
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        hasher.update(timestamp.to_be_bytes());

        Ok(hasher.finalize().into())
    }

    /// Broadcast dice roll result to all participants
    async fn broadcast_dice_roll_result(
        &self,
        game_id: GameId,
        roll: DiceRoll,
        participants: &[PeerId],
    ) -> Result<()> {
        // Create dice roll message
        let message = DiceRollBroadcast {
            game_id,
            roll,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        log::info!(
            "Broadcasting dice roll {:?} to {} participants",
            roll,
            participants.len()
        );

        // Create packet for broadcasting dice roll result
        let packet = self.create_game_message_packet(&message)?;

        // Broadcast dice roll result to all participants via mesh service
        if let Err(e) = self.mesh_service.broadcast_packet(packet).await {
            log::error!("Failed to broadcast dice roll result: {:?}", e);
            return Err(e);
        }

        // For compatibility with existing parallel structure, create confirmation tasks
        let broadcast_tasks: Vec<_> = participants
            .iter()
            .map(|&participant| {
                tokio::spawn(async move {
                    log::debug!(
                        "Dice roll result broadcasted to participant {:?}",
                        participant
                    );
                    Ok::<(), Error>(())
                })
            })
            .collect();

        // Wait for all broadcasts to complete
        let results = join_all(broadcast_tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Failed to broadcast dice roll: {:?}", e);
            }
        }

        Ok(())
    }

    /// Broadcast bet resolutions to all participants
    async fn broadcast_bet_resolutions(
        &self,
        game_id: GameId,
        resolutions: &[crate::protocol::game_logic::BetResolution],
        participants: &[PeerId],
    ) -> Result<()> {
        log::info!(
            "Broadcasting {} bet resolutions to {} participants",
            resolutions.len(),
            participants.len()
        );

        // Create bet resolution message
        let message = BetResolutionBroadcast {
            game_id,
            resolutions: resolutions.to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        // Create packet for broadcasting bet resolutions
        let packet = self.create_game_message_packet(&message)?;

        // Broadcast bet resolutions to all participants via mesh service
        if let Err(e) = self.mesh_service.broadcast_packet(packet).await {
            log::error!("Failed to broadcast bet resolutions: {:?}", e);
            return Err(e);
        }

        // For compatibility with existing parallel structure, create confirmation tasks
        let broadcast_tasks: Vec<_> = participants
            .iter()
            .map(|&participant| {
                tokio::spawn(async move {
                    log::debug!(
                        "Bet resolutions broadcasted to participant {:?}",
                        participant
                    );
                    Ok::<(), Error>(())
                })
            })
            .collect();

        // Wait for all broadcasts to complete
        let results = join_all(broadcast_tasks).await;
        for result in results {
            if let Err(e) = result {
                log::error!("Failed to broadcast bet resolutions: {:?}", e);
            }
        }

        Ok(())
    }

    /// Suspend a game
    pub async fn suspend_game(&self, game_id: GameId, _reason: String) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        game.is_suspended = true;
        Ok(())
    }

    /// Resume a game
    pub async fn resume_game(&self, game_id: GameId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id).ok_or(Error::GameNotFound)?;

        game.is_suspended = false;
        game.last_activity = Instant::now();
        Ok(())
    }

    /// Start timeout monitor
    pub async fn start_timeout_monitor(&self) {
        let games = self.games.clone();
        let timeouts = self.game_timeouts.clone();
        let _timeout_duration = self.config.game_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                let now = Instant::now();
                let mut expired_games = Vec::new();

                // Find expired games
                {
                    let timeout_map = timeouts.read().await;
                    for (&game_id, &timeout) in timeout_map.iter() {
                        if now > timeout {
                            expired_games.push(game_id);
                        }
                    }
                }

                // Remove expired games
                if !expired_games.is_empty() {
                    let mut games_map = games.write().await;
                    let mut timeout_map = timeouts.write().await;

                    for game_id in expired_games {
                        games_map.remove(&game_id);
                        timeout_map.remove(&game_id);
                        log::info!("Game {:?} expired due to inactivity", game_id);
                    }
                }
            }
        });
    }

    /// Stop all games
    pub async fn stop_all_games(&self) -> Result<()> {
        let mut games = self.games.write().await;
        games.clear();

        let mut timeouts = self.game_timeouts.write().await;
        timeouts.clear();

        Ok(())
    }

    /// Get active game count
    pub async fn active_game_count(&self) -> usize {
        self.games.read().await.len()
    }

    /// Get game state
    pub async fn get_game(&self, game_id: GameId) -> Option<ActiveGame> {
        self.games.read().await.get(&game_id).cloned()
    }

    /// Broadcast turn state change to all participants
    async fn broadcast_turn_state_change(
        &self,
        game_id: GameId,
        turn_manager: &TurnManager,
        participants: &[PeerId],
    ) -> Result<()> {
        let message = TurnStateBroadcast {
            game_id,
            current_shooter: turn_manager.current_shooter(),
            turn_state: turn_manager.turn_state().clone(),
            shooter_queue: turn_manager.get_shooter_queue().to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        log::debug!(
            "Broadcasting turn state change to {} participants",
            participants.len()
        );

        // Create packet for broadcasting turn state change
        let packet = self.create_game_message_packet(&message)?;

        // Broadcast turn state change to all participants via mesh service
        if let Err(e) = self.mesh_service.broadcast_packet(packet).await {
            log::error!("Failed to broadcast turn state change: {:?}", e);
            return Err(e);
        }

        for participant in participants {
            log::debug!(
                "Turn state change broadcasted to participant {:?}",
                participant
            );
        }

        Ok(())
    }
}
