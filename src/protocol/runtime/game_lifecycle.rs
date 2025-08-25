//! Game Lifecycle Manager
//! 
//! Handles game creation, joining, leaving, and lifecycle transitions.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use crate::protocol::{PeerId, GameId, new_game_id, DiceRoll};
use crate::protocol::craps::CrapsGame;
use crate::protocol::{Bet, BetType};
use crate::error::{Error, Result};
use super::config::GameRuntimeConfig;

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
}

/// Commands for game lifecycle
#[derive(Debug)]
pub enum GameCommand {
    CreateGame { creator: PeerId, config: GameConfig },
    JoinGame { game_id: GameId, player: PeerId, buy_in: u64 },
    PlaceBet { game_id: GameId, player: PeerId, bet: Bet },
    RollDice { game_id: GameId, shooter: PeerId },
    LeaveGame { game_id: GameId, player: PeerId },
    SuspendGame { game_id: GameId, reason: String },
    ResumeGame { game_id: GameId },
}

/// Manages game lifecycles
pub struct GameLifecycleManager {
    config: Arc<GameRuntimeConfig>,
    games: Arc<RwLock<HashMap<GameId, ActiveGame>>>,
    game_timeouts: Arc<RwLock<HashMap<GameId, Instant>>>,
}

impl GameLifecycleManager {
    /// Create a new game lifecycle manager
    pub fn new(config: Arc<GameRuntimeConfig>) -> Self {
        Self {
            config,
            games: Arc::new(RwLock::new(HashMap::new())),
            game_timeouts: Arc::new(RwLock::new(HashMap::new())),
        }
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
        };
        
        games.insert(game_id, active_game);
        
        // Set timeout
        let mut timeouts = self.game_timeouts.write().await;
        timeouts.insert(game_id, Instant::now() + self.config.game_timeout);
        
        Ok(game_id)
    }
    
    /// Add a player to a game
    pub async fn add_player_to_game(&self, game_id: GameId, player: PeerId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
        // Check if game is full
        if game.game.participants.len() >= self.config.max_players {
            return Err(Error::GameError("Game is full".into()));
        }
        
        // Check if game is suspended
        if game.is_suspended {
            return Err(Error::GameError("Game is suspended".into()));
        }
        
        // Add player
        if !game.game.add_player(player) {
            return Err(Error::GameError("Player already in game".into()));
        }
        
        // Update activity
        game.last_activity = Instant::now();
        
        Ok(())
    }
    
    /// Remove a player from a game
    pub async fn remove_player_from_game(&self, game_id: GameId, player: PeerId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
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
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
        // Validate bet is allowed
        if !game.config.allowed_bets.is_empty() && 
           !game.config.allowed_bets.contains(&bet.bet_type) {
            return Err(Error::InvalidBet("Bet type not allowed in this game".into()));
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
    
    /// Process a dice roll
    pub async fn process_dice_roll(&self, game_id: GameId, shooter: PeerId) -> Result<DiceRoll> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
        // Verify shooter
        if game.game.get_shooter() != shooter {
            return Err(Error::GameError("Not the current shooter".into()));
        }
        
        // Generate secure dice roll
        let roll = CrapsGame::roll_dice_secure()?;
        
        // Process roll
        let _resolutions = game.game.process_roll(roll);
        
        // Update stats
        game.rounds_played += 1;
        game.last_activity = Instant::now();
        
        Ok(roll)
    }
    
    /// Suspend a game
    pub async fn suspend_game(&self, game_id: GameId, _reason: String) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
        game.is_suspended = true;
        Ok(())
    }
    
    /// Resume a game
    pub async fn resume_game(&self, game_id: GameId) -> Result<()> {
        let mut games = self.games.write().await;
        let game = games.get_mut(&game_id)
            .ok_or(Error::GameNotFound)?;
        
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
}