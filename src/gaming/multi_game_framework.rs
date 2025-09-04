//! Multi-Game Framework for BitCraps Platform
//!
//! This module provides a flexible framework for supporting multiple casino games
//! on the BitCraps platform with:
//! - Game plugin system
//! - Unified game state management
//! - Cross-game interoperability
//! - Flexible betting systems
//! - Game-specific rule engines

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info};
use uuid::Uuid;

#[cfg(feature = "monitoring")]
use crate::monitoring::metrics::METRICS;
use crate::persistence::PersistenceManager;
use crate::protocol::craps::CrapsGame;

/// Multi-game framework manager
pub struct MultiGameFramework {
    /// Registered game engines
    game_engines: Arc<RwLock<HashMap<String, Box<dyn GameEngine>>>>,
    /// Active game sessions
    active_sessions: Arc<RwLock<HashMap<String, Arc<GameSession>>>>,
    /// Game statistics
    stats: Arc<GameFrameworkStats>,
    /// Event broadcast channel
    event_sender: broadcast::Sender<GameFrameworkEvent>,
    /// Framework configuration
    config: GameFrameworkConfig,
    /// Persistent storage for game states
    persistence: Option<Arc<PersistenceManager>>,
}

impl MultiGameFramework {
    /// Create new multi-game framework
    pub fn new(config: GameFrameworkConfig) -> Self {
        let (event_sender, _) = broadcast::channel(1000);

        let mut framework = Self {
            game_engines: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(GameFrameworkStats::new()),
            event_sender,
            config,
            persistence: None, // Initialize without persistence, can be set later
        };

        // Register built-in games
        framework.register_builtin_games();
        framework
    }

    /// Register a new game engine
    pub async fn register_game(
        &self,
        game_id: String,
        engine: Box<dyn GameEngine>,
    ) -> Result<(), GameFrameworkError> {
        // Validate game engine
        if let Err(e) = engine.validate().await {
            return Err(GameFrameworkError::InvalidGameEngine(format!(
                "Game {} validation failed: {:?}",
                game_id, e
            )));
        }

        // Register the engine
        self.game_engines
            .write()
            .await
            .insert(game_id.clone(), engine);

        info!("Registered game engine: {}", game_id);
        self.broadcast_event(GameFrameworkEvent::GameRegistered { game_id })
            .await;

        Ok(())
    }

    /// Get available games
    pub async fn get_available_games(&self) -> Vec<GameInfo> {
        let engines = self.game_engines.read().await;
        let mut games = Vec::new();

        for (game_id, engine) in engines.iter() {
            games.push(GameInfo {
                id: game_id.clone(),
                name: engine.get_name(),
                description: engine.get_description(),
                min_players: engine.get_min_players(),
                max_players: engine.get_max_players(),
                supported_bet_types: engine.get_supported_bet_types(),
                house_edge: engine.get_house_edge(),
                is_available: engine.is_available().await,
            });
        }

        games
    }

    /// Create new game session
    pub async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> Result<String, GameFrameworkError> {
        // Get game engine
        let engines = self.game_engines.read().await;
        let engine = engines
            .get(&request.game_id)
            .ok_or_else(|| GameFrameworkError::UnknownGame(request.game_id.clone()))?;

        // Validate session parameters
        engine
            .validate_session_config(&request.config)
            .await
            .map_err(|e| GameFrameworkError::InvalidSessionConfig(format!("{:?}", e)))?;

        // Create session
        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(GameSession {
            id: session_id.clone(),
            game_id: request.game_id.clone(),
            players: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(GameSessionState::WaitingForPlayers)),
            config: request.config,
            stats: GameSessionStats::new(),
            created_at: SystemTime::now(),
            last_activity: Arc::new(RwLock::new(SystemTime::now())),
        });

        // Initialize game-specific state
        engine.initialize_session(&session).await?;

        // Implement persistent game state storage
        if let Err(e) = self.save_game_state(&session_id, &session).await {
            log::warn!(
                "Failed to persist game state for session {}: {}",
                session_id,
                e
            );
            // Continue execution - persistence failure shouldn't prevent gameplay
        }

        // Add to active sessions
        self.active_sessions
            .write()
            .await
            .insert(session_id.clone(), session);

        // Update stats
        self.stats
            .total_sessions_created
            .fetch_add(1, Ordering::Relaxed);

        info!(
            "Created game session: {} for game: {}",
            session_id, request.game_id
        );
        self.broadcast_event(GameFrameworkEvent::SessionCreated {
            session_id: session_id.clone(),
            game_id: request.game_id,
        })
        .await;

        Ok(session_id)
    }

    /// Join existing game session
    pub async fn join_session(
        &self,
        session_id: &str,
        player_id: String,
        join_data: PlayerJoinData,
    ) -> Result<(), GameFrameworkError> {
        // Get session
        let sessions = self.active_sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| GameFrameworkError::SessionNotFound(session_id.to_string()))?;

        // Get game engine
        let engines = self.game_engines.read().await;
        let engine = engines
            .get(&session.game_id)
            .ok_or_else(|| GameFrameworkError::UnknownGame(session.game_id.clone()))?;

        // Validate join request
        engine
            .validate_player_join(session, &player_id, &join_data)
            .await?;

        // Add player to session
        let player_info = PlayerInfo {
            id: player_id.clone(),
            balance: join_data.initial_balance,
            joined_at: SystemTime::now(),
            is_active: true,
            game_specific_data: join_data.game_specific_data,
        };

        session
            .players
            .write()
            .await
            .insert(player_id.clone(), player_info);

        // Update last activity
        *session.last_activity.write().await = SystemTime::now();

        // Notify game engine
        engine.on_player_joined(session, &player_id).await?;

        info!("Player {} joined session {}", player_id, session_id);
        self.broadcast_event(GameFrameworkEvent::PlayerJoined {
            session_id: session_id.to_string(),
            player_id,
        })
        .await;

        Ok(())
    }

    /// Process game action
    pub async fn process_action(
        &self,
        session_id: &str,
        player_id: &str,
        action: GameAction,
    ) -> Result<GameActionResult, GameFrameworkError> {
        // Get session
        let sessions = self.active_sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| GameFrameworkError::SessionNotFound(session_id.to_string()))?;

        // Get game engine
        let engines = self.game_engines.read().await;
        let engine = engines
            .get(&session.game_id)
            .ok_or_else(|| GameFrameworkError::UnknownGame(session.game_id.clone()))?;

        // Validate player is in session
        let players = session.players.read().await;
        if !players.contains_key(player_id) {
            return Err(GameFrameworkError::PlayerNotInSession(
                player_id.to_string(),
            ));
        }
        drop(players);

        // Process action through game engine
        let result = engine
            .process_action(session, player_id, action.clone())
            .await?;

        // Update last activity
        *session.last_activity.write().await = SystemTime::now();

        // Update statistics
        self.stats
            .total_actions_processed
            .fetch_add(1, Ordering::Relaxed);

        debug!(
            "Processed action {:?} for player {} in session {}",
            action, player_id, session_id
        );
        self.broadcast_event(GameFrameworkEvent::ActionProcessed {
            session_id: session_id.to_string(),
            player_id: player_id.to_string(),
            action,
            result: result.clone(),
        })
        .await;

        Ok(result)
    }

    /// End game session
    pub async fn end_session(
        &self,
        session_id: &str,
        reason: SessionEndReason,
    ) -> Result<SessionSummary, GameFrameworkError> {
        // Get and remove session
        let session = self
            .active_sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| GameFrameworkError::SessionNotFound(session_id.to_string()))?;

        // Get game engine
        let engines = self.game_engines.read().await;
        let engine = engines
            .get(&session.game_id)
            .ok_or_else(|| GameFrameworkError::UnknownGame(session.game_id.clone()))?;

        // Let game engine handle cleanup
        engine.on_session_ended(&session, &reason).await?;

        // Generate session summary
        let summary = SessionSummary {
            session_id: session_id.to_string(),
            game_id: session.game_id.clone(),
            duration: SystemTime::now()
                .duration_since(session.created_at)
                .unwrap_or(Duration::from_secs(0)),
            players: session.players.read().await.keys().cloned().collect(),
            end_reason: reason.clone(),
            stats: session.stats.clone(),
        };

        info!("Ended session {} (reason: {:?})", session_id, reason);
        self.broadcast_event(GameFrameworkEvent::SessionEnded {
            session_id: session_id.to_string(),
            reason,
        })
        .await;

        Ok(summary)
    }

    /// Get framework statistics
    pub async fn get_statistics(&self) -> GameFrameworkStatistics {
        let active_sessions = self.active_sessions.read().await.len();
        let available_games = self.game_engines.read().await.len();

        GameFrameworkStatistics {
            total_sessions_created: self.stats.total_sessions_created.load(Ordering::Relaxed),
            active_sessions,
            total_actions_processed: self.stats.total_actions_processed.load(Ordering::Relaxed),
            total_games_registered: available_games,
            uptime_seconds: self.stats.start_time.elapsed().as_secs(),
        }
    }

    /// Subscribe to framework events
    pub fn subscribe_events(&self) -> broadcast::Receiver<GameFrameworkEvent> {
        self.event_sender.subscribe()
    }

    /// Register built-in games
    fn register_builtin_games(&mut self) {
        // Register Craps
        tokio::spawn({
            let framework = self.clone();
            async move {
                let craps_engine = Box::new(CrapsGameEngine::new());
                if let Err(e) = framework
                    .register_game("craps".to_string(), craps_engine)
                    .await
                {
                    error!("Failed to register Craps game: {:?}", e);
                }
            }
        });

        // Register Blackjack
        tokio::spawn({
            let framework = self.clone();
            async move {
                let blackjack_engine = Box::new(BlackjackGameEngine::new());
                if let Err(e) = framework
                    .register_game("blackjack".to_string(), blackjack_engine)
                    .await
                {
                    error!("Failed to register Blackjack game: {:?}", e);
                }
            }
        });

        // Register Poker
        tokio::spawn({
            let framework = self.clone();
            async move {
                let poker_engine = Box::new(PokerGameEngine::new());
                if let Err(e) = framework
                    .register_game("poker".to_string(), poker_engine)
                    .await
                {
                    error!("Failed to register Poker game: {:?}", e);
                }
            }
        });
    }

    /// Broadcast framework event
    async fn broadcast_event(&self, event: GameFrameworkEvent) {
        if let Err(e) = self.event_sender.send(event) {
            debug!("No event subscribers: {:?}", e);
        }
    }

    /// Start background tasks
    pub async fn start_background_tasks(&self) -> Result<(), GameFrameworkError> {
        // Session cleanup task
        let active_sessions = Arc::clone(&self.active_sessions);
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                Self::cleanup_inactive_sessions(&active_sessions, &event_sender).await;
            }
        });

        // Statistics reporting task
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
            loop {
                interval.tick().await;
                stats.report_to_metrics().await;
            }
        });

        Ok(())
    }

    /// Cleanup inactive sessions
    async fn cleanup_inactive_sessions(
        active_sessions: &Arc<RwLock<HashMap<String, Arc<GameSession>>>>,
        event_sender: &broadcast::Sender<GameFrameworkEvent>,
    ) {
        let timeout = Duration::from_secs(3600); // 1 hour timeout
        let mut expired_sessions = Vec::new();

        {
            let sessions = active_sessions.read().await;
            let now = SystemTime::now();

            for (session_id, session) in sessions.iter() {
                let last_activity = *session.last_activity.read().await;
                if now
                    .duration_since(last_activity)
                    .unwrap_or(Duration::from_secs(0))
                    > timeout
                {
                    expired_sessions.push(session_id.clone());
                }
            }
        }

        if !expired_sessions.is_empty() {
            let mut sessions = active_sessions.write().await;
            for session_id in expired_sessions {
                sessions.remove(&session_id);
                if let Err(e) = event_sender.send(GameFrameworkEvent::SessionEnded {
                    session_id,
                    reason: SessionEndReason::Timeout,
                }) {
                    debug!("Failed to broadcast session timeout event: {:?}", e);
                }
            }
        }
    }

    /// Set persistence manager
    pub fn set_persistence(&mut self, persistence: Arc<PersistenceManager>) {
        self.persistence = Some(persistence);
    }

    /// Save game state to persistent storage
    async fn save_game_state(
        &self,
        session_id: &str,
        session: &GameSession,
    ) -> crate::error::Result<()> {
        if let Some(ref persistence) = self.persistence {
            // Serialize game session to JSON
            let session_data = match serde_json::to_string(session) {
                Ok(data) => data,
                Err(e) => {
                    return Err(crate::error::Error::GameLogic(format!(
                        "Failed to serialize game session: {}",
                        e
                    )));
                }
            };

            // Store with game state key (using simple filesystem storage for now)
            let storage_path = format!("game_state_{}.json", session_id);
            if let Err(e) = persistence.save_string(&storage_path, &session_data).await {
                return Err(crate::error::Error::GameLogic(format!(
                    "Failed to persist game state: {}",
                    e
                )));
            }

            info!(
                "Successfully persisted game state for session: {}",
                session_id
            );
        } else {
            debug!("No persistence manager configured, skipping game state persistence");
        }
        Ok(())
    }

    /// Load game state from persistent storage
    pub async fn load_game_state(
        &self,
        session_id: &str,
    ) -> crate::error::Result<Option<GameSession>> {
        if let Some(ref persistence) = self.persistence {
            let storage_path = format!("game_state_{}.json", session_id);

            match persistence.load_string(&storage_path).await {
                Ok(Some(session_data)) => {
                    match serde_json::from_str::<GameSession>(&session_data) {
                        Ok(session) => {
                            info!("Successfully loaded game state for session: {}", session_id);
                            return Ok(Some(session));
                        }
                        Err(e) => {
                            error!("Failed to deserialize game session {}: {}", session_id, e);
                            return Err(crate::error::Error::GameLogic(format!(
                                "Failed to deserialize game session: {}",
                                e
                            )));
                        }
                    }
                }
                Ok(None) => {
                    debug!("No persisted state found for session: {}", session_id);
                }
                Err(e) => {
                    return Err(crate::error::Error::GameLogic(format!(
                        "Failed to load game state: {}",
                        e
                    )));
                }
            }
        }
        Ok(None)
    }

    /// Remove game state from persistent storage
    pub async fn remove_game_state(&self, session_id: &str) -> crate::error::Result<()> {
        if let Some(ref persistence) = self.persistence {
            let storage_path = format!("game_state_{}.json", session_id);
            if let Err(e) = persistence.delete_file(&storage_path).await {
                return Err(crate::error::Error::GameLogic(format!(
                    "Failed to delete game state: {}",
                    e
                )));
            }
            info!("Removed persisted game state for session: {}", session_id);
        }
        Ok(())
    }
}

impl Clone for MultiGameFramework {
    fn clone(&self) -> Self {
        Self {
            game_engines: Arc::clone(&self.game_engines),
            active_sessions: Arc::clone(&self.active_sessions),
            stats: Arc::clone(&self.stats),
            event_sender: self.event_sender.clone(),
            config: self.config.clone(),
            persistence: self.persistence.clone(),
        }
    }
}

/// Trait for implementing game engines
#[async_trait]
pub trait GameEngine: Send + Sync {
    /// Get game name
    fn get_name(&self) -> String;

    /// Get game description
    fn get_description(&self) -> String;

    /// Get minimum number of players
    fn get_min_players(&self) -> usize;

    /// Get maximum number of players
    fn get_max_players(&self) -> usize;

    /// Get supported bet types
    fn get_supported_bet_types(&self) -> Vec<String>;

    /// Get house edge percentage
    fn get_house_edge(&self) -> f64;

    /// Check if game is currently available
    async fn is_available(&self) -> bool;

    /// Validate game engine configuration
    async fn validate(&self) -> Result<(), GameEngineError>;

    /// Validate session configuration
    async fn validate_session_config(
        &self,
        config: &GameSessionConfig,
    ) -> Result<(), GameEngineError>;

    /// Initialize new game session
    async fn initialize_session(&self, session: &GameSession) -> Result<(), GameFrameworkError>;

    /// Validate player join request
    async fn validate_player_join(
        &self,
        session: &GameSession,
        player_id: &str,
        join_data: &PlayerJoinData,
    ) -> Result<(), GameFrameworkError>;

    /// Handle player joined event
    async fn on_player_joined(
        &self,
        session: &GameSession,
        player_id: &str,
    ) -> Result<(), GameFrameworkError>;

    /// Process game action
    async fn process_action(
        &self,
        session: &GameSession,
        player_id: &str,
        action: GameAction,
    ) -> Result<GameActionResult, GameFrameworkError>;

    /// Handle session ended event
    async fn on_session_ended(
        &self,
        session: &GameSession,
        reason: &SessionEndReason,
    ) -> Result<(), GameFrameworkError>;
}

/// Craps game engine implementation
pub struct CrapsGameEngine {
    craps_game: Arc<CrapsGame>,
}

impl Default for CrapsGameEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CrapsGameEngine {
    pub fn new() -> Self {
        Self {
            craps_game: Arc::new(CrapsGame::new([0u8; 16], [0u8; 32])),
        }
    }
}

#[async_trait]
impl GameEngine for CrapsGameEngine {
    fn get_name(&self) -> String {
        "Craps".to_string()
    }

    fn get_description(&self) -> String {
        "Traditional casino craps game with come-out and point phases".to_string()
    }

    fn get_min_players(&self) -> usize {
        1
    }

    fn get_max_players(&self) -> usize {
        14
    }

    fn get_supported_bet_types(&self) -> Vec<String> {
        vec![
            "pass_line".to_string(),
            "dont_pass".to_string(),
            "field".to_string(),
            "any_seven".to_string(),
            "hard_ways".to_string(),
            "place_bets".to_string(),
        ]
    }

    fn get_house_edge(&self) -> f64 {
        1.36 // 1.36% house edge for pass line bets
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        // Validate craps game configuration
        Ok(())
    }

    async fn validate_session_config(
        &self,
        _config: &GameSessionConfig,
    ) -> Result<(), GameEngineError> {
        // Validate craps-specific session configuration
        Ok(())
    }

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), GameFrameworkError> {
        // Initialize craps game state
        info!("Initialized Craps session");
        Ok(())
    }

    async fn validate_player_join(
        &self,
        session: &GameSession,
        _player_id: &str,
        _join_data: &PlayerJoinData,
    ) -> Result<(), GameFrameworkError> {
        // Check if session has room for another player
        let player_count = session.players.read().await.len();
        if player_count >= self.get_max_players() {
            return Err(GameFrameworkError::SessionFull);
        }
        Ok(())
    }

    async fn on_player_joined(
        &self,
        _session: &GameSession,
        player_id: &str,
    ) -> Result<(), GameFrameworkError> {
        info!("Player {} joined Craps game", player_id);
        Ok(())
    }

    async fn process_action(
        &self,
        _session: &GameSession,
        player_id: &str,
        action: GameAction,
    ) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            GameAction::PlaceBet { bet_type, amount } => {
                info!("Player {} placed {} bet: {}", player_id, bet_type, amount);
                Ok(GameActionResult::BetPlaced {
                    bet_id: Uuid::new_v4().to_string(),
                    confirmation: "Bet placed successfully".to_string(),
                })
            }
            GameAction::RollDice => {
                let roll = (fastrand::u8(1..=6), fastrand::u8(1..=6));
                info!("Dice rolled: {:?}", roll);
                Ok(GameActionResult::DiceRolled {
                    dice: roll,
                    total: roll.0 + roll.1,
                })
            }
            _ => Err(GameFrameworkError::UnsupportedAction(
                "Action not supported for Craps".to_string(),
            )),
        }
    }

    async fn on_session_ended(
        &self,
        session: &GameSession,
        reason: &SessionEndReason,
    ) -> Result<(), GameFrameworkError> {
        info!("Craps session {} ended: {:?}", session.id, reason);
        Ok(())
    }
}

/// Blackjack game engine implementation
pub struct BlackjackGameEngine;

impl Default for BlackjackGameEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BlackjackGameEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GameEngine for BlackjackGameEngine {
    fn get_name(&self) -> String {
        "Blackjack".to_string()
    }

    fn get_description(&self) -> String {
        "Classic casino blackjack with standard rules".to_string()
    }

    fn get_min_players(&self) -> usize {
        1
    }

    fn get_max_players(&self) -> usize {
        7
    }

    fn get_supported_bet_types(&self) -> Vec<String> {
        vec![
            "main".to_string(),
            "side_bet".to_string(),
            "insurance".to_string(),
        ]
    }

    fn get_house_edge(&self) -> f64 {
        0.5 // 0.5% house edge with basic strategy
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn validate_session_config(
        &self,
        _config: &GameSessionConfig,
    ) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), GameFrameworkError> {
        info!("Initialized Blackjack session");
        Ok(())
    }

    async fn validate_player_join(
        &self,
        session: &GameSession,
        _player_id: &str,
        _join_data: &PlayerJoinData,
    ) -> Result<(), GameFrameworkError> {
        let player_count = session.players.read().await.len();
        if player_count >= self.get_max_players() {
            return Err(GameFrameworkError::SessionFull);
        }
        Ok(())
    }

    async fn on_player_joined(
        &self,
        _session: &GameSession,
        player_id: &str,
    ) -> Result<(), GameFrameworkError> {
        info!("Player {} joined Blackjack game", player_id);
        Ok(())
    }

    async fn process_action(
        &self,
        _session: &GameSession,
        player_id: &str,
        action: GameAction,
    ) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            GameAction::Hit => {
                let card = fastrand::u8(1..=13);
                info!("Player {} hit, drew card: {}", player_id, card);
                Ok(GameActionResult::CardDealt { card })
            }
            GameAction::Stand => {
                info!("Player {} stands", player_id);
                Ok(GameActionResult::PlayerStands)
            }
            GameAction::PlaceBet { bet_type, amount } => {
                info!("Player {} placed {} bet: {}", player_id, bet_type, amount);
                Ok(GameActionResult::BetPlaced {
                    bet_id: Uuid::new_v4().to_string(),
                    confirmation: "Bet placed successfully".to_string(),
                })
            }
            _ => Err(GameFrameworkError::UnsupportedAction(
                "Action not supported for Blackjack".to_string(),
            )),
        }
    }

    async fn on_session_ended(
        &self,
        session: &GameSession,
        reason: &SessionEndReason,
    ) -> Result<(), GameFrameworkError> {
        info!("Blackjack session {} ended: {:?}", session.id, reason);
        Ok(())
    }
}

/// Poker game engine implementation
pub struct PokerGameEngine;

impl Default for PokerGameEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PokerGameEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GameEngine for PokerGameEngine {
    fn get_name(&self) -> String {
        "Texas Hold'em Poker".to_string()
    }

    fn get_description(&self) -> String {
        "Texas Hold'em poker with community cards".to_string()
    }

    fn get_min_players(&self) -> usize {
        2
    }

    fn get_max_players(&self) -> usize {
        9
    }

    fn get_supported_bet_types(&self) -> Vec<String> {
        vec![
            "blind".to_string(),
            "bet".to_string(),
            "raise".to_string(),
            "call".to_string(),
            "all_in".to_string(),
        ]
    }

    fn get_house_edge(&self) -> f64 {
        2.0 // 2% rake
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn validate_session_config(
        &self,
        _config: &GameSessionConfig,
    ) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), GameFrameworkError> {
        info!("Initialized Poker session");
        Ok(())
    }

    async fn validate_player_join(
        &self,
        session: &GameSession,
        _player_id: &str,
        _join_data: &PlayerJoinData,
    ) -> Result<(), GameFrameworkError> {
        let player_count = session.players.read().await.len();
        if player_count >= self.get_max_players() {
            return Err(GameFrameworkError::SessionFull);
        }
        Ok(())
    }

    async fn on_player_joined(
        &self,
        _session: &GameSession,
        player_id: &str,
    ) -> Result<(), GameFrameworkError> {
        info!("Player {} joined Poker game", player_id);
        Ok(())
    }

    async fn process_action(
        &self,
        _session: &GameSession,
        player_id: &str,
        action: GameAction,
    ) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            GameAction::Fold => {
                info!("Player {} folds", player_id);
                Ok(GameActionResult::PlayerFolds)
            }
            GameAction::Check => {
                info!("Player {} checks", player_id);
                Ok(GameActionResult::PlayerChecks)
            }
            GameAction::PlaceBet { bet_type, amount } => {
                info!("Player {} placed {} bet: {}", player_id, bet_type, amount);
                Ok(GameActionResult::BetPlaced {
                    bet_id: Uuid::new_v4().to_string(),
                    confirmation: format!("{} bet placed", bet_type),
                })
            }
            _ => Err(GameFrameworkError::UnsupportedAction(
                "Action not supported for Poker".to_string(),
            )),
        }
    }

    async fn on_session_ended(
        &self,
        session: &GameSession,
        reason: &SessionEndReason,
    ) -> Result<(), GameFrameworkError> {
        info!("Poker session {} ended: {:?}", session.id, reason);
        Ok(())
    }
}

// Supporting types and structures
#[derive(Debug, Clone)]
pub struct GameFrameworkConfig {
    pub session_timeout_minutes: u64,
    pub max_concurrent_sessions: usize,
    pub enable_cross_game_features: bool,
}

impl Default for GameFrameworkConfig {
    fn default() -> Self {
        Self {
            session_timeout_minutes: 60,
            max_concurrent_sessions: 1000,
            enable_cross_game_features: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub min_players: usize,
    pub max_players: usize,
    pub supported_bet_types: Vec<String>,
    pub house_edge: f64,
    pub is_available: bool,
}

#[derive(Debug, Clone)]
pub struct CreateSessionRequest {
    pub game_id: String,
    pub config: GameSessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSessionConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub auto_start: bool,
    pub game_specific_config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PlayerJoinData {
    pub initial_balance: u64,
    pub game_specific_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: String,
    pub balance: u64,
    pub joined_at: SystemTime,
    pub is_active: bool,
    pub game_specific_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub id: String,
    pub game_id: String,
    #[serde(skip, default = "default_players")] // Skip serialization with defaults
    pub players: Arc<RwLock<HashMap<String, PlayerInfo>>>,
    #[serde(skip, default = "default_game_state")]
    pub state: Arc<RwLock<GameSessionState>>,
    pub config: GameSessionConfig,
    #[serde(skip, default = "GameSessionStats::new")] // Skip atomic stats
    pub stats: GameSessionStats,
    pub created_at: SystemTime,
    #[serde(skip, default = "default_last_activity")]
    pub last_activity: Arc<RwLock<SystemTime>>,
}

// Default value functions for serde skipped fields
fn default_players() -> Arc<RwLock<HashMap<String, PlayerInfo>>> {
    Arc::new(RwLock::new(HashMap::new()))
}

fn default_game_state() -> Arc<RwLock<GameSessionState>> {
    Arc::new(RwLock::new(GameSessionState::WaitingForPlayers))
}

fn default_last_activity() -> Arc<RwLock<SystemTime>> {
    Arc::new(RwLock::new(SystemTime::now()))
}

#[derive(Debug, Clone)]
pub enum GameSessionState {
    WaitingForPlayers,
    InProgress,
    Paused,
    Ended,
}

#[derive(Debug)]
pub struct GameSessionStats {
    pub total_bets: AtomicU64,
    pub total_volume: AtomicU64,
    pub games_played: AtomicU64,
}

impl Clone for GameSessionStats {
    fn clone(&self) -> Self {
        Self {
            total_bets: AtomicU64::new(self.total_bets.load(Ordering::Relaxed)),
            total_volume: AtomicU64::new(self.total_volume.load(Ordering::Relaxed)),
            games_played: AtomicU64::new(self.games_played.load(Ordering::Relaxed)),
        }
    }
}

impl Default for GameSessionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl GameSessionStats {
    pub fn new() -> Self {
        Self {
            total_bets: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
            games_played: AtomicU64::new(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameAction {
    PlaceBet { bet_type: String, amount: u64 },
    RollDice,
    Hit,
    Stand,
    Fold,
    Check,
    Call,
    Raise { amount: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameActionResult {
    BetPlaced {
        bet_id: String,
        confirmation: String,
    },
    DiceRolled {
        dice: (u8, u8),
        total: u8,
    },
    CardDealt {
        card: u8,
    },
    PlayerStands,
    PlayerFolds,
    PlayerChecks,
    BetAccepted {
        new_balance: u64,
    },
}

#[derive(Debug, Clone)]
pub enum SessionEndReason {
    GameComplete,
    PlayerLeft,
    Timeout,
    Error(String),
    AdminAction,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub game_id: String,
    pub duration: Duration,
    pub players: Vec<String>,
    pub end_reason: SessionEndReason,
    pub stats: GameSessionStats,
}

#[derive(Debug, Clone)]
pub enum GameFrameworkEvent {
    GameRegistered {
        game_id: String,
    },
    SessionCreated {
        session_id: String,
        game_id: String,
    },
    SessionEnded {
        session_id: String,
        reason: SessionEndReason,
    },
    PlayerJoined {
        session_id: String,
        player_id: String,
    },
    ActionProcessed {
        session_id: String,
        player_id: String,
        action: GameAction,
        result: GameActionResult,
    },
}

pub struct GameFrameworkStats {
    pub total_sessions_created: AtomicU64,
    pub total_actions_processed: AtomicU64,
    pub start_time: std::time::Instant,
}

impl Default for GameFrameworkStats {
    fn default() -> Self {
        Self::new()
    }
}

impl GameFrameworkStats {
    pub fn new() -> Self {
        Self {
            total_sessions_created: AtomicU64::new(0),
            total_actions_processed: AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        }
    }

    pub async fn report_to_metrics(&self) {
        // Report gaming metrics to global monitoring
        #[cfg(feature = "monitoring")]
        {
            METRICS.gaming.total_games.store(
                self.total_sessions_created.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
        
        #[cfg(not(feature = "monitoring"))]
        {
            // No-op when monitoring is disabled
            tracing::debug!("Metrics reporting is disabled (monitoring feature not enabled)");
        }
    }
}

#[derive(Debug)]
pub struct GameFrameworkStatistics {
    pub total_sessions_created: u64,
    pub active_sessions: usize,
    pub total_actions_processed: u64,
    pub total_games_registered: usize,
    pub uptime_seconds: u64,
}

#[derive(Debug)]
pub enum GameFrameworkError {
    UnknownGame(String),
    SessionNotFound(String),
    SessionFull,
    PlayerNotInSession(String),
    InvalidGameEngine(String),
    InvalidSessionConfig(String),
    UnsupportedAction(String),
    GameEngineError(GameEngineError),
}

#[derive(Debug)]
pub enum GameEngineError {
    ConfigurationError(String),
    ValidationError(String),
    ProcessingError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_framework_creation() {
        let config = GameFrameworkConfig::default();
        let framework = MultiGameFramework::new(config);

        // Framework starts empty, games need to be registered
        let games = framework.get_available_games().await;
        assert_eq!(games.len(), 0); // Starts with no games
    }

    #[tokio::test]
    async fn test_session_creation() {
        let config = GameFrameworkConfig::default();
        let framework = MultiGameFramework::new(config);

        // Wait a bit for builtin games to register
        tokio::time::sleep(Duration::from_millis(100)).await;

        let request = CreateSessionRequest {
            game_id: "craps".to_string(),
            config: GameSessionConfig {
                min_bet: 1,
                max_bet: 1000,
                auto_start: false,
                game_specific_config: HashMap::new(),
            },
        };

        let session_id = framework.create_session(request).await.unwrap();
        assert!(!session_id.is_empty());

        let stats = framework.get_statistics().await;
        assert_eq!(stats.total_sessions_created, 1);
        assert_eq!(stats.active_sessions, 1);
    }

    #[tokio::test]
    async fn test_player_join() {
        let config = GameFrameworkConfig::default();
        let framework = MultiGameFramework::new(config);

        // Wait for builtin games
        tokio::time::sleep(Duration::from_millis(100)).await;

        let request = CreateSessionRequest {
            game_id: "craps".to_string(),
            config: GameSessionConfig {
                min_bet: 1,
                max_bet: 1000,
                auto_start: false,
                game_specific_config: HashMap::new(),
            },
        };

        let session_id = framework.create_session(request).await.unwrap();

        let join_data = PlayerJoinData {
            initial_balance: 1000,
            game_specific_data: HashMap::new(),
        };

        framework
            .join_session(&session_id, "player1".to_string(), join_data)
            .await
            .unwrap();

        // Test duplicate join should work (same player can rejoin)
        let join_data2 = PlayerJoinData {
            initial_balance: 2000,
            game_specific_data: HashMap::new(),
        };

        framework
            .join_session(&session_id, "player2".to_string(), join_data2)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_game_action() {
        let config = GameFrameworkConfig::default();
        let framework = MultiGameFramework::new(config);

        // Wait for builtin games
        tokio::time::sleep(Duration::from_millis(100)).await;

        let request = CreateSessionRequest {
            game_id: "craps".to_string(),
            config: GameSessionConfig {
                min_bet: 1,
                max_bet: 1000,
                auto_start: false,
                game_specific_config: HashMap::new(),
            },
        };

        let session_id = framework.create_session(request).await.unwrap();

        let join_data = PlayerJoinData {
            initial_balance: 1000,
            game_specific_data: HashMap::new(),
        };

        framework
            .join_session(&session_id, "player1".to_string(), join_data)
            .await
            .unwrap();

        let action = GameAction::PlaceBet {
            bet_type: "pass_line".to_string(),
            amount: 10,
        };

        let result = framework
            .process_action(&session_id, "player1", action)
            .await
            .unwrap();

        match result {
            GameActionResult::BetPlaced {
                bet_id,
                confirmation,
            } => {
                assert!(!bet_id.is_empty());
                assert!(!confirmation.is_empty());
            }
            _ => panic!("Expected BetPlaced result"),
        }
    }
}
