//! Custom Game Engine Implementation for BitCraps SDK
//!
//! This module provides a generic custom game engine that can be configured
//! to run games created with the BitCraps SDK.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::gaming::{
    GameEngine, GameEngineError, GameSession, PlayerJoinData, 
    GameAction, GameActionResult, SessionEndReason, GameFrameworkError
};
use crate::sdk::game_types::*;

/// Custom game engine that can run user-defined games
pub struct CustomGameEngine {
    game: CustomGame,
    sessions: Arc<RwLock<std::collections::HashMap<String, CustomGameSession>>>,
}

impl CustomGameEngine {
    /// Create a new custom game engine from a game definition
    pub fn from_game(game: CustomGame) -> Self {
        Self {
            game,
            sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Get the underlying game definition
    pub fn game(&self) -> &CustomGame {
        &self.game
    }

    /// Update the game configuration
    pub fn update_config(&mut self, config: GameConfig) {
        // Create a new game with updated config
        let mut updated_game = self.game.clone();
        updated_game.config = config;
        self.game = updated_game;
    }
}

#[async_trait]
impl GameEngine for CustomGameEngine {
    fn get_name(&self) -> String {
        self.game.name.clone()
    }

    fn get_description(&self) -> String {
        self.game.template.description.clone()
    }

    fn get_min_players(&self) -> usize {
        self.game.template.min_players
    }

    fn get_max_players(&self) -> usize {
        self.game.template.max_players
    }

    fn get_supported_bet_types(&self) -> Vec<String> {
        // Generate bet types based on game category and payout multipliers
        let mut bet_types = match self.game.template.category {
            crate::sdk::templates::GameCategory::DiceGame => {
                vec!["Pass", "Don't Pass", "Come", "Field"]
            }
            crate::sdk::templates::GameCategory::CardGame => {
                vec!["Player", "Banker", "Tie"]
            }
            crate::sdk::templates::GameCategory::AuctionGame => {
                vec!["Bid"]
            }
            _ => vec!["Standard"]
        }.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();

        // Add custom bet types from payout multipliers
        bet_types.extend(self.game.config.payout_multipliers.keys().cloned());

        // Remove duplicates and return
        bet_types.sort();
        bet_types.dedup();
        bet_types
    }

    fn get_house_edge(&self) -> f64 {
        self.game.config.house_edge
    }

    async fn is_available(&self) -> bool {
        // Check if game is properly configured and ready to run
        !self.game.name.is_empty() 
            && self.game.template.min_players > 0 
            && self.game.template.min_players <= self.game.template.max_players
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        // Validate game configuration
        if self.game.name.is_empty() {
            return Err(GameEngineError::InvalidConfiguration("Game name is empty".to_string()));
        }

        if self.game.template.min_players == 0 {
            return Err(GameEngineError::InvalidConfiguration("Minimum players must be at least 1".to_string()));
        }

        if self.game.template.min_players > self.game.template.max_players {
            return Err(GameEngineError::InvalidConfiguration("Minimum players cannot exceed maximum players".to_string()));
        }

        if self.game.config.house_edge < 0.0 || self.game.config.house_edge > 100.0 {
            return Err(GameEngineError::InvalidConfiguration("House edge must be between 0% and 100%".to_string()));
        }

        if self.game.config.betting_limits.min_bet > self.game.config.betting_limits.max_bet {
            return Err(GameEngineError::InvalidConfiguration("Minimum bet cannot exceed maximum bet".to_string()));
        }

        Ok(())
    }

    async fn validate_session_config(&self, config: &crate::gaming::GameSessionConfig) -> Result<(), GameEngineError> {
        // Validate that session configuration is compatible with this game
        if config.max_players > self.game.template.max_players {
            return Err(GameEngineError::InvalidConfiguration(
                format!("Session allows {} players but game maximum is {}", 
                    config.max_players, self.game.template.max_players)
            ));
        }

        if config.min_players < self.game.template.min_players {
            return Err(GameEngineError::InvalidConfiguration(
                format!("Session requires {} players but game minimum is {}", 
                    config.min_players, self.game.template.min_players)
            ));
        }

        Ok(())
    }

    async fn initialize_session(&self, session: &GameSession) -> Result<(), GameFrameworkError> {
        let mut sessions = self.sessions.write().await;
        
        // Create initial game state based on game category
        let initial_state = match self.game.template.category {
            crate::sdk::templates::GameCategory::DiceGame => {
                CustomGameState::Dice(DiceGameState {
                    dice: vec![1, 1], // Initial dice values
                    phase: DicePhase::WaitingForBets,
                    bets: Vec::new(),
                    roll_result: None,
                    current_player: None,
                })
            }
            crate::sdk::templates::GameCategory::CardGame => {
                CustomGameState::Card(CardGameState {
                    deck: generate_standard_deck(),
                    players: std::collections::HashMap::new(),
                    current_turn: String::new(),
                    pot: 0,
                    betting_round: 0,
                    community_cards: Vec::new(),
                })
            }
            crate::sdk::templates::GameCategory::AuctionGame => {
                CustomGameState::Auction(AuctionGameState {
                    current_item: None,
                    highest_bid: None,
                    participants: std::collections::HashMap::new(),
                    auction_queue: Vec::new(),
                    completed_auctions: Vec::new(),
                    time_remaining: 0,
                })
            }
            _ => {
                CustomGameState::Generic(std::collections::HashMap::new())
            }
        };

        sessions.insert(session.id.clone(), CustomGameSession {
            session_id: session.id.clone(),
            game_state: initial_state,
            players: Vec::new(),
            started_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
        });

        Ok(())
    }

    async fn validate_player_join(&self, session: &GameSession, player_id: &str, _join_data: &PlayerJoinData) -> Result<(), GameFrameworkError> {
        let sessions = self.sessions.read().await;
        
        if let Some(custom_session) = sessions.get(&session.id) {
            // Check if player is already in the session
            if custom_session.players.iter().any(|p| p.id == player_id) {
                return Err(GameFrameworkError::PlayerAlreadyInSession(player_id.to_string()));
            }

            // Check if session is full
            if custom_session.players.len() >= self.game.template.max_players {
                return Err(GameFrameworkError::SessionFull);
            }

            Ok(())
        } else {
            Err(GameFrameworkError::SessionNotFound(session.id.clone()))
        }
    }

    async fn on_player_joined(&self, session: &GameSession, player_id: &str) -> Result<(), GameFrameworkError> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(custom_session) = sessions.get_mut(&session.id) {
            custom_session.players.push(CustomPlayer {
                id: player_id.to_string(),
                name: format!("Player {}", player_id),
                balance: 1000, // Default starting balance
                status: PlayerStatus::Active,
                joined_at: std::time::SystemTime::now(),
            });

            custom_session.last_activity = std::time::SystemTime::now();
            Ok(())
        } else {
            Err(GameFrameworkError::SessionNotFound(session.id.clone()))
        }
    }

    async fn process_action(&self, session: &GameSession, player_id: &str, action: GameAction) -> Result<GameActionResult, GameFrameworkError> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(custom_session) = sessions.get_mut(&session.id) {
            // Update last activity
            custom_session.last_activity = std::time::SystemTime::now();

            // Process action based on game category and current state
            match &mut custom_session.game_state {
                CustomGameState::Dice(dice_state) => {
                    self.process_dice_action(dice_state, player_id, action).await
                }
                CustomGameState::Card(card_state) => {
                    self.process_card_action(card_state, player_id, action).await
                }
                CustomGameState::Auction(auction_state) => {
                    self.process_auction_action(auction_state, player_id, action).await
                }
                CustomGameState::Generic(_) => {
                    // Generic action processing
                    Ok(GameActionResult::Success {
                        message: "Action processed".to_string(),
                        state_changes: std::collections::HashMap::new(),
                    })
                }
            }
        } else {
            Err(GameFrameworkError::SessionNotFound(session.id.clone()))
        }
    }

    async fn on_session_ended(&self, session: &GameSession, _reason: &SessionEndReason) -> Result<(), GameFrameworkError> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&session.id);
        Ok(())
    }
}

impl CustomGameEngine {
    async fn process_dice_action(&self, _dice_state: &mut DiceGameState, _player_id: &str, action: GameAction) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            // Handle dice-specific actions
            _ => Ok(GameActionResult::Success {
                message: format!("Dice action processed: {:?}", action),
                state_changes: std::collections::HashMap::new(),
            })
        }
    }

    async fn process_card_action(&self, _card_state: &mut CardGameState, _player_id: &str, action: GameAction) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            // Handle card-specific actions
            _ => Ok(GameActionResult::Success {
                message: format!("Card action processed: {:?}", action),
                state_changes: std::collections::HashMap::new(),
            })
        }
    }

    async fn process_auction_action(&self, _auction_state: &mut AuctionGameState, _player_id: &str, action: GameAction) -> Result<GameActionResult, GameFrameworkError> {
        match action {
            // Handle auction-specific actions
            _ => Ok(GameActionResult::Success {
                message: format!("Auction action processed: {:?}", action),
                state_changes: std::collections::HashMap::new(),
            })
        }
    }
}

/// Custom game session state
struct CustomGameSession {
    session_id: String,
    game_state: CustomGameState,
    players: Vec<CustomPlayer>,
    started_at: std::time::SystemTime,
    last_activity: std::time::SystemTime,
}

/// Game state variants for different game types
enum CustomGameState {
    Dice(DiceGameState),
    Card(CardGameState),
    Auction(AuctionGameState),
    Generic(std::collections::HashMap<String, serde_json::Value>),
}

/// Generate a standard 52-card deck
fn generate_standard_deck() -> Vec<Card> {
    let mut deck = Vec::new();
    let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
    let ranks = [
        Rank::Ace, Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six,
        Rank::Seven, Rank::Eight, Rank::Nine, Rank::Ten, Rank::Jack, Rank::Queen, Rank::King
    ];

    for suit in &suits {
        for rank in &ranks {
            deck.push(Card {
                suit: suit.clone(),
                rank: rank.clone(),
            });
        }
    }

    deck
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::builder::GamePresets;

    #[tokio::test]
    async fn test_custom_engine_creation() {
        let game = GamePresets::dice_game()
            .name("Test Dice Game")
            .build()
            .unwrap();

        let engine = CustomGameEngine::from_game(game);
        
        assert_eq!(engine.get_name(), "Test Dice Game");
        assert!(engine.is_available().await);
        assert!(engine.validate().await.is_ok());
    }

    #[tokio::test]
    async fn test_session_management() {
        let game = GamePresets::card_game()
            .name("Test Card Game")
            .build()
            .unwrap();

        let engine = CustomGameEngine::from_game(game);
        
        // Create a mock session
        let session = GameSession {
            id: "test-session".to_string(),
            game_id: "test-game".to_string(),
            created_at: std::time::SystemTime::now(),
            started_at: None,
            ended_at: None,
            status: crate::gaming::SessionStatus::WaitingForPlayers,
            players: Vec::new(),
            max_players: 4,
            config: crate::gaming::GameSessionConfig {
                min_players: 2,
                max_players: 4,
                betting_enabled: true,
                spectators_allowed: false,
                private_session: false,
            },
        };

        // Initialize session
        assert!(engine.initialize_session(&session).await.is_ok());

        // Test player join
        let join_data = PlayerJoinData::default();
        assert!(engine.validate_player_join(&session, "player1", &join_data).await.is_ok());
        assert!(engine.on_player_joined(&session, "player1").await.is_ok());

        // Test duplicate player join
        assert!(engine.validate_player_join(&session, "player1", &join_data).await.is_err());
    }

    #[test]
    fn test_bet_types_generation() {
        let dice_game = GamePresets::dice_game()
            .name("Dice Game")
            .build()
            .unwrap();

        let dice_engine = CustomGameEngine::from_game(dice_game);
        let bet_types = dice_engine.get_supported_bet_types();
        
        assert!(bet_types.contains(&"Pass".to_string()));
        assert!(bet_types.contains(&"Don't Pass".to_string()));
    }

    #[test]
    fn test_standard_deck_generation() {
        let deck = generate_standard_deck();
        assert_eq!(deck.len(), 52);
        
        // Check that we have all suits and ranks
        let mut hearts = 0;
        let mut aces = 0;
        
        for card in &deck {
            if matches!(card.suit, Suit::Hearts) {
                hearts += 1;
            }
            if matches!(card.rank, Rank::Ace) {
                aces += 1;
            }
        }
        
        assert_eq!(hearts, 13); // 13 hearts
        assert_eq!(aces, 4);    // 4 aces
    }
}