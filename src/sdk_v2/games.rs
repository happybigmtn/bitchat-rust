//! Game Management API
//!
//! High-level API for creating, joining, and managing games with builder patterns
//! and comprehensive error handling.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    types::*,
    rest::RestClient,
    SDKContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Game management API
#[derive(Debug)]
pub struct GameAPI {
    context: Arc<SDKContext>,
    rest_client: RestClient,
    active_sessions: Arc<RwLock<Vec<GameSession>>>,
}

impl GameAPI {
    /// Create a new game API instance
    pub fn new(context: Arc<SDKContext>) -> Self {
        let rest_client = RestClient::new(&context.config)
            .expect("Failed to create REST client");
        
        Self {
            context,
            rest_client,
            active_sessions: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Create a new game builder
    pub fn create(&self, name: &str) -> GameBuilder {
        GameBuilder::new(name.to_string(), self.rest_client.clone())
    }
    
    /// Join an existing game
    pub async fn join(&self, game_id: &GameId) -> SDKResult<GameSession> {
        let request = JoinGameRequest {
            player_id: self.get_current_player_id().await?,
        };
        
        let response: GameSession = self.rest_client
            .post(&format!("games/{}/join", game_id), request)
            .await?;
        
        // Add to active sessions
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.push(response.clone());
        }
        
        // Update metrics
        {
            let mut metrics = self.context.metrics.write().await;
            metrics.requests_made += 1;
        }
        
        Ok(response)
    }
    
    /// Leave a game
    pub async fn leave(&self, game_id: &GameId) -> SDKResult<()> {
        let player_id = self.get_current_player_id().await?;
        
        let _: serde_json::Value = self.rest_client
            .delete(&format!("games/{}/players/{}", game_id, player_id))
            .await?;
        
        // Remove from active sessions
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.retain(|s| &s.game_id != game_id);
        }
        
        Ok(())
    }
    
    /// List available games
    pub async fn list(&self, filters: Option<GameFilters>) -> SDKResult<Vec<GameInfo>> {
        let games: Vec<GameInfo> = if let Some(filters) = filters {
            self.rest_client
                .get_with_params("games", filters)
                .await?
        } else {
            self.rest_client
                .get("games")
                .await?
        };
        
        Ok(games)
    }
    
    /// Get detailed game information
    pub async fn get(&self, game_id: &GameId) -> SDKResult<GameDetails> {
        let game_details: GameDetails = self.rest_client
            .get(&format!("games/{}", game_id))
            .await?;
        
        Ok(game_details)
    }
    
    /// Get game state
    pub async fn get_state(&self, game_id: &GameId) -> SDKResult<GameState> {
        let game_state: GameState = self.rest_client
            .get(&format!("games/{}/state", game_id))
            .await?;
        
        Ok(game_state)
    }
    
    /// Place a bet in a game
    pub async fn place_bet(&self, game_id: &GameId, bet_type: String, amount: u64) -> SDKResult<BetResult> {
        let request = PlaceBetRequest {
            player_id: self.get_current_player_id().await?,
            bet_type,
            amount,
        };
        
        let result: BetResult = self.rest_client
            .post(&format!("games/{}/bets", game_id), request)
            .await?;
        
        Ok(result)
    }
    
    /// Get current player's active sessions
    pub async fn get_active_sessions(&self) -> Vec<GameSession> {
        self.active_sessions.read().await.clone()
    }
    
    /// Get game history for current player
    pub async fn get_history(&self, limit: Option<u32>) -> SDKResult<Vec<GameHistoryEntry>> {
        let player_id = self.get_current_player_id().await?;
        
        let mut path = format!("players/{}/games/history", player_id);
        if let Some(limit) = limit {
            path = format!("{}?limit={}", path, limit);
        }
        
        let history: Vec<GameHistoryEntry> = self.rest_client
            .get(&path)
            .await?;
        
        Ok(history)
    }
    
    /// Search for games
    pub async fn search(&self, query: &str) -> SDKResult<Vec<GameInfo>> {
        let request = SearchGamesRequest {
            query: query.to_string(),
            filters: None,
        };
        
        let games: Vec<GameInfo> = self.rest_client
            .post("games/search", request)
            .await?;
        
        Ok(games)
    }
    
    // Private helper methods
    
    async fn get_current_player_id(&self) -> SDKResult<PlayerId> {
        // In a real implementation, this would come from authentication context
        Ok("current_player_id".to_string())
    }
}

/// Game builder with fluent API
#[derive(Debug)]
pub struct GameBuilder {
    name: String,
    game_type: GameType,
    max_players: u32,
    min_players: u32,
    betting_limits: BettingLimits,
    time_limits: TimeLimits,
    is_private: bool,
    tags: Vec<String>,
    custom_rules: std::collections::HashMap<String, serde_json::Value>,
    rest_client: RestClient,
}

impl GameBuilder {
    /// Create a new game builder
    pub fn new(name: String, rest_client: RestClient) -> Self {
        Self {
            name,
            game_type: GameType::Craps,
            max_players: 8,
            min_players: 2,
            betting_limits: BettingLimits {
                min_bet: 1,
                max_bet: 1000,
                max_total_bet: None,
                bet_increment: None,
            },
            time_limits: TimeLimits {
                turn_timeout: Some(60),
                betting_timeout: Some(30),
                game_timeout: Some(3600),
                pause_timeout: Some(300),
            },
            is_private: false,
            tags: Vec::new(),
            custom_rules: std::collections::HashMap::new(),
            rest_client,
        }
    }
    
    /// Set game type
    pub fn game_type(mut self, game_type: GameType) -> Self {
        self.game_type = game_type;
        self
    }
    
    /// Set maximum number of players
    pub fn with_max_players(mut self, max_players: u32) -> Self {
        self.max_players = max_players;
        self
    }
    
    /// Set minimum number of players
    pub fn with_min_players(mut self, min_players: u32) -> Self {
        self.min_players = min_players;
        self
    }
    
    /// Set betting limits
    pub fn with_betting_limits(mut self, min_bet: u64, max_bet: u64) -> Self {
        self.betting_limits.min_bet = min_bet;
        self.betting_limits.max_bet = max_bet;
        self
    }
    
    /// Set maximum total bet per round
    pub fn with_max_total_bet(mut self, max_total_bet: u64) -> Self {
        self.betting_limits.max_total_bet = Some(max_total_bet);
        self
    }
    
    /// Set bet increment
    pub fn with_bet_increment(mut self, increment: u64) -> Self {
        self.betting_limits.bet_increment = Some(increment);
        self
    }
    
    /// Set turn timeout
    pub fn with_turn_timeout(mut self, seconds: u64) -> Self {
        self.time_limits.turn_timeout = Some(seconds);
        self
    }
    
    /// Set betting timeout
    pub fn with_betting_timeout(mut self, seconds: u64) -> Self {
        self.time_limits.betting_timeout = Some(seconds);
        self
    }
    
    /// Set game timeout
    pub fn with_game_timeout(mut self, seconds: u64) -> Self {
        self.time_limits.game_timeout = Some(seconds);
        self
    }
    
    /// Make game private
    pub fn private(mut self) -> Self {
        self.is_private = true;
        self
    }
    
    /// Make game public
    pub fn public(mut self) -> Self {
        self.is_private = false;
        self
    }
    
    /// Add tags to the game
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
    
    /// Add a single tag
    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }
    
    /// Add a custom rule
    pub fn with_custom_rule<T: serde::Serialize>(mut self, key: String, value: T) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.custom_rules.insert(key, json_value);
        }
        self
    }
    
    /// Build and create the game
    pub async fn build(self) -> SDKResult<GameInfo> {
        // Validate configuration
        self.validate()?;
        
        let request = CreateGameRequest {
            name: self.name,
            game_type: self.game_type,
            max_players: self.max_players,
            min_players: self.min_players,
            betting_limits: self.betting_limits,
            time_limits: self.time_limits,
            is_private: self.is_private,
            tags: self.tags,
            custom_rules: self.custom_rules,
        };
        
        let game_info: GameInfo = self.rest_client
            .post("games", request)
            .await?;
        
        Ok(game_info)
    }
    
    /// Validate the game configuration
    fn validate(&self) -> SDKResult<()> {
        if self.name.is_empty() {
            return Err(SDKError::ValidationError {
                message: "Game name cannot be empty".to_string(),
                field: Some("name".to_string()),
                invalid_value: Some(self.name.clone()),
            });
        }
        
        if self.max_players < self.min_players {
            return Err(SDKError::ValidationError {
                message: "Maximum players must be greater than or equal to minimum players".to_string(),
                field: Some("max_players".to_string()),
                invalid_value: Some(self.max_players.to_string()),
            });
        }
        
        if self.min_players == 0 {
            return Err(SDKError::ValidationError {
                message: "Minimum players must be at least 1".to_string(),
                field: Some("min_players".to_string()),
                invalid_value: Some(self.min_players.to_string()),
            });
        }
        
        if self.betting_limits.min_bet >= self.betting_limits.max_bet {
            return Err(SDKError::ValidationError {
                message: "Maximum bet must be greater than minimum bet".to_string(),
                field: Some("betting_limits".to_string()),
                invalid_value: Some(format!("min: {}, max: {}", self.betting_limits.min_bet, self.betting_limits.max_bet)),
            });
        }
        
        Ok(())
    }
}

/// Request/Response structures
#[derive(Debug, Serialize)]
struct CreateGameRequest {
    name: String,
    game_type: GameType,
    max_players: u32,
    min_players: u32,
    betting_limits: BettingLimits,
    time_limits: TimeLimits,
    is_private: bool,
    tags: Vec<String>,
    custom_rules: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JoinGameRequest {
    player_id: PlayerId,
}

#[derive(Debug, Serialize)]
struct PlaceBetRequest {
    player_id: PlayerId,
    bet_type: String,
    amount: u64,
}

#[derive(Debug, Deserialize)]
pub struct BetResult {
    pub success: bool,
    pub bet_id: String,
    pub remaining_balance: u64,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchGamesRequest {
    query: String,
    filters: Option<GameFilters>,
}

#[derive(Debug, Deserialize)]
pub struct GameHistoryEntry {
    pub game_id: GameId,
    pub game_name: String,
    pub game_type: GameType,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
    pub final_position: Option<u32>,
    pub total_winnings: i64,
    pub total_bets: u64,
}

/// Preset game configurations
pub struct GamePresets;

impl GamePresets {
    /// Create a quick casual craps game
    pub fn casual_craps() -> GameBuilder {
        GameBuilder::new("Casual Craps".to_string(), RestClient::new(&Default::default()).unwrap())
            .game_type(GameType::Craps)
            .with_max_players(6)
            .with_min_players(2)
            .with_betting_limits(1, 100)
            .with_turn_timeout(30)
            .with_tag("casual".to_string())
            .with_tag("quick".to_string())
    }
    
    /// Create a high-stakes craps game
    pub fn high_stakes_craps() -> GameBuilder {
        GameBuilder::new("High Stakes Craps".to_string(), RestClient::new(&Default::default()).unwrap())
            .game_type(GameType::Craps)
            .with_max_players(8)
            .with_min_players(3)
            .with_betting_limits(100, 10000)
            .with_turn_timeout(60)
            .with_betting_timeout(45)
            .with_tag("high-stakes".to_string())
            .with_tag("serious".to_string())
    }
    
    /// Create a tournament-style game
    pub fn tournament_game() -> GameBuilder {
        GameBuilder::new("Tournament Game".to_string(), RestClient::new(&Default::default()).unwrap())
            .game_type(GameType::Craps)
            .with_max_players(12)
            .with_min_players(8)
            .with_betting_limits(50, 5000)
            .with_game_timeout(7200) // 2 hours
            .with_turn_timeout(45)
            .with_tag("tournament".to_string())
            .with_tag("competitive".to_string())
    }
    
    /// Create a private game for friends
    pub fn private_friends_game() -> GameBuilder {
        GameBuilder::new("Friends Game".to_string(), RestClient::new(&Default::default()).unwrap())
            .game_type(GameType::Craps)
            .private()
            .with_max_players(6)
            .with_min_players(2)
            .with_betting_limits(1, 500)
            .with_turn_timeout(90) // More relaxed timing
            .with_tag("friends".to_string())
            .with_tag("private".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::{config::{Config, Environment}, init};
    
    #[tokio::test]
    async fn test_game_builder_validation() {
        let builder = GameBuilder::new("Test Game".to_string(), RestClient::new(&Default::default()).unwrap());
        assert!(builder.validate().is_ok());
        
        let invalid_builder = GameBuilder::new("".to_string(), RestClient::new(&Default::default()).unwrap());
        assert!(invalid_builder.validate().is_err());
        
        let invalid_players = GameBuilder::new("Test".to_string(), RestClient::new(&Default::default()).unwrap())
            .with_max_players(2)
            .with_min_players(5);
        assert!(invalid_players.validate().is_err());
    }
    
    #[test]
    fn test_game_presets() {
        let casual = GamePresets::casual_craps();
        assert_eq!(casual.max_players, 6);
        assert_eq!(casual.betting_limits.max_bet, 100);
        assert!(casual.tags.contains(&"casual".to_string()));
        
        let high_stakes = GamePresets::high_stakes_craps();
        assert_eq!(high_stakes.betting_limits.max_bet, 10000);
        assert!(high_stakes.tags.contains(&"high-stakes".to_string()));
    }
    
    #[tokio::test]
    async fn test_game_api_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build()
            .unwrap();
            
        let context = init(config).await.unwrap();
        let game_api = GameAPI::new(context);
        
        // Test that the API was created successfully
        assert_eq!(game_api.get_active_sessions().await.len(), 0);
    }
}