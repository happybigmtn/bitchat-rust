//! Game Engine Service Implementation
//!
//! Core service that manages game sessions and coordinates with other microservices.

use super::types::*;
use super::{GameEngine, GameEngineConfig, CrapsGameEngine};
use crate::error::{Error, Result};
use crate::protocol::craps::CrapsGame;
use crate::protocol::{GameId, PeerId};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use uuid::Uuid;

/// Game Engine Service
pub struct GameEngineService {
    config: GameEngineConfig,
    engine: Arc<CrapsGameEngine>,
    sessions: Arc<DashMap<GameId, Arc<RwLock<GameSessionData>>>>,
    stats: Arc<GameServiceStats>,
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
}

/// Internal session data
#[derive(Debug)]
struct GameSessionData {
    info: GameSessionInfo,
    game_state: CrapsGame,
    last_activity: Instant,
}

/// Service statistics
#[derive(Debug)]
struct GameServiceStats {
    total_games: AtomicU64,
    active_games: AtomicU64,
    completed_games: AtomicU64,
    total_players: AtomicU64,
    total_bets: AtomicU64,
    total_volume: AtomicU64,
    start_time: Instant,
}

impl GameServiceStats {
    fn new() -> Self {
        Self {
            total_games: AtomicU64::new(0),
            active_games: AtomicU64::new(0),
            completed_games: AtomicU64::new(0),
            total_players: AtomicU64::new(0),
            total_bets: AtomicU64::new(0),
            total_volume: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
    
    fn to_game_stats(&self) -> GameStats {
        GameStats {
            total_games: self.total_games.load(Ordering::Relaxed),
            active_games: self.active_games.load(Ordering::Relaxed),
            completed_games: self.completed_games.load(Ordering::Relaxed),
            total_players: self.total_players.load(Ordering::Relaxed),
            total_bets: self.total_bets.load(Ordering::Relaxed),
            total_volume: self.total_volume.load(Ordering::Relaxed),
        }
    }
}

impl GameEngineService {
    /// Create a new game engine service
    pub fn new(config: GameEngineConfig) -> Self {
        let engine = Arc::new(CrapsGameEngine::new(config.clone()));
        let sessions = Arc::new(DashMap::new());
        let stats = Arc::new(GameServiceStats::new());
        
        Self {
            config,
            engine,
            sessions,
            stats,
            shutdown_tx: None,
        }
    }
    
    /// Start the service
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start background tasks
        let sessions = self.sessions.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));
            
            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        Self::cleanup_inactive_sessions(&sessions, &config, &stats).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        log::info!("Game Engine Service started");
        Ok(())
    }
    
    /// Stop the service
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        log::info!("Game Engine Service stopped");
        Ok(())
    }
    
    /// Create a new game
    pub async fn create_game(&self, request: CreateGameRequest) -> Result<CreateGameResponse> {
        if request.players.is_empty() {
            return Err(Error::GameError("At least one player required".to_string()));
        }
        
        let (game_id, game_state) = self.engine.create_game(request.players.clone()).await?;
        let session_info = GameSessionInfo::new(game_id, request.players, game_state.phase);
        
        let session_data = GameSessionData {
            info: session_info.clone(),
            game_state,
            last_activity: Instant::now(),
        };
        
        self.sessions.insert(game_id, Arc::new(RwLock::new(session_data)));
        
        // Update statistics
        self.stats.total_games.fetch_add(1, Ordering::Relaxed);
        self.stats.active_games.fetch_add(1, Ordering::Relaxed);
        self.stats.total_players.fetch_add(session_info.players.len() as u64, Ordering::Relaxed);
        
        Ok(CreateGameResponse {
            game_id,
            session_info,
        })
    }
    
    /// Process a game action
    pub async fn process_action(&self, request: ProcessActionRequest) -> Result<ProcessActionResponse> {
        let session = self.sessions.get(&request.game_id)
            .ok_or_else(|| Error::GameError("Game not found".to_string()))?;
        
        let mut session_data = session.write().await;
        
        if !session_data.info.is_active {
            return Err(Error::GameError("Game is not active".to_string()));
        }
        
        let result = self.engine.process_action(
            &request.game_id,
            &request.player_id,
            request.action.clone(),
            &mut session_data.game_state,
        ).await?;
        
        // Update session info
        session_data.info.phase = session_data.game_state.phase;
        session_data.info.update_activity();
        session_data.last_activity = Instant::now();
        
        // Check if game is complete
        if self.engine.is_game_complete(&session_data.game_state) {
            session_data.info.is_active = false;
            self.stats.active_games.fetch_sub(1, Ordering::Relaxed);
            self.stats.completed_games.fetch_add(1, Ordering::Relaxed);
        }
        
        // Update statistics
        match &request.action {
            GameAction::PlaceBet { amount, .. } => {
                self.stats.total_bets.fetch_add(1, Ordering::Relaxed);
                self.stats.total_volume.fetch_add(*amount, Ordering::Relaxed);
            },
            _ => {}
        }
        
        Ok(ProcessActionResponse {
            result,
            updated_session: session_data.info.clone(),
        })
    }
    
    /// Get game state
    pub async fn get_game_state(&self, request: GetGameStateRequest) -> Result<GetGameStateResponse> {
        let session = self.sessions.get(&request.game_id)
            .ok_or_else(|| Error::GameError("Game not found".to_string()))?;
        
        let session_data = session.read().await;
        
        let mut valid_actions = std::collections::HashMap::new();
        for player_id in &session_data.info.players {
            let actions = self.engine.get_valid_actions(&session_data.game_state, player_id);
            valid_actions.insert(*player_id, actions);
        }
        
        Ok(GetGameStateResponse {
            session_info: session_data.info.clone(),
            valid_actions,
        })
    }
    
    /// List games
    pub async fn list_games(&self, request: ListGamesRequest) -> Result<ListGamesResponse> {
        let mut games = Vec::new();
        
        for entry in self.sessions.iter() {
            let session_data = entry.value().read().await;
            
            // Filter by active status
            if request.active_only && !session_data.info.is_active {
                continue;
            }
            
            // Filter by player
            if let Some(player_id) = request.player_id {
                if !session_data.info.players.contains(&player_id) {
                    continue;
                }
            }
            
            games.push(session_data.info.clone());
        }
        
        // Sort by last activity (most recent first)
        games.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        
        // Apply limit
        let total_count = games.len();
        if let Some(limit) = request.limit {
            games.truncate(limit);
        }
        
        Ok(ListGamesResponse {
            games,
            total_count,
        })
    }
    
    /// Health check
    pub async fn health_check(&self) -> Result<HealthCheckResponse> {
        let uptime = self.stats.start_time.elapsed().as_secs();
        
        Ok(HealthCheckResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime,
            stats: self.stats.to_game_stats(),
        })
    }
    
    /// Cleanup inactive sessions
    async fn cleanup_inactive_sessions(
        sessions: &DashMap<GameId, Arc<RwLock<GameSessionData>>>,
        config: &GameEngineConfig,
        stats: &GameServiceStats,
    ) {
        let mut to_remove = Vec::new();
        let timeout_threshold = Instant::now() - config.game_timeout;
        
        for entry in sessions.iter() {
            let session_data = entry.value().read().await;
            
            if !session_data.info.is_active && session_data.last_activity < timeout_threshold {
                to_remove.push(*entry.key());
            }
        }
        
        for game_id in to_remove {
            if sessions.remove(&game_id).is_some() {
                log::info!("Cleaned up inactive game: {:?}", game_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::craps::BetType;
    
    #[tokio::test]
    async fn test_create_game() {
        let mut service = GameEngineService::new(GameEngineConfig::default());
        service.start().await.unwrap();
        
        let players = vec![PeerId::new(), PeerId::new()];
        let request = CreateGameRequest {
            players: players.clone(),
            game_type: "craps".to_string(),
            config: None,
        };
        
        let response = service.create_game(request).await.unwrap();
        assert_eq!(response.session_info.players, players);
        assert!(response.session_info.is_active);
        
        service.stop().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_process_action() {
        let mut service = GameEngineService::new(GameEngineConfig::default());
        service.start().await.unwrap();
        
        // Create a game
        let players = vec![PeerId::new(), PeerId::new()];
        let create_request = CreateGameRequest {
            players: players.clone(),
            game_type: "craps".to_string(),
            config: None,
        };
        let create_response = service.create_game(create_request).await.unwrap();
        
        // Place a bet
        let action_request = ProcessActionRequest {
            game_id: create_response.game_id,
            player_id: players[0],
            action: GameAction::PlaceBet {
                bet_type: BetType::Pass,
                amount: 100,
            },
        };
        
        let action_response = service.process_action(action_request).await.unwrap();
        match action_response.result {
            GameActionResult::BetPlaced { player, amount, .. } => {
                assert_eq!(player, players[0]);
                assert_eq!(amount, 100);
            },
            _ => panic!("Expected BetPlaced result"),
        }
        
        service.stop().await.unwrap();
    }
}