//! Game Engine Service
//!
//! Microservice responsible for game logic, state management, and game sessions.
//! Provides gRPC and REST APIs for game operations.

pub mod api;
pub mod engine;
pub mod service;
pub mod types;

pub use service::GameEngineService;
pub use types::*;

use crate::error::{Error, Result};
use crate::protocol::craps::{CrapsGame, BetType, CrapTokens, GamePhase, DiceRoll};
use crate::protocol::{GameId, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Game engine configuration
#[derive(Debug, Clone)]
pub struct GameEngineConfig {
    pub max_concurrent_games: usize,
    pub max_players_per_game: usize,
    pub min_bet_amount: u64,
    pub max_bet_amount: u64,
    pub game_timeout: std::time::Duration,
}

impl Default for GameEngineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_games: 100,
            max_players_per_game: 8,
            min_bet_amount: 1,
            max_bet_amount: 10000,
            game_timeout: std::time::Duration::from_mins(30),
        }
    }
}

/// Game engine trait for different game types
pub trait GameEngine: Send + Sync {
    type GameState: Clone + Send + Sync;
    type GameAction: Clone + Send + Sync;
    type GameResult: Clone + Send + Sync;

    /// Create a new game instance
    async fn create_game(&self, players: Vec<PeerId>) -> Result<(GameId, Self::GameState)>;
    
    /// Process a game action
    async fn process_action(
        &self,
        game_id: &GameId,
        player_id: &PeerId,
        action: Self::GameAction,
        state: &mut Self::GameState,
    ) -> Result<Self::GameResult>;
    
    /// Check if game is complete
    fn is_game_complete(&self, state: &Self::GameState) -> bool;
    
    /// Get valid actions for a player
    fn get_valid_actions(&self, state: &Self::GameState, player_id: &PeerId) -> Vec<Self::GameAction>;
}

/// Craps game engine implementation
pub struct CrapsGameEngine {
    config: GameEngineConfig,
}

impl CrapsGameEngine {
    pub fn new(config: GameEngineConfig) -> Self {
        Self { config }
    }
}

impl GameEngine for CrapsGameEngine {
    type GameState = CrapsGame;
    type GameAction = GameAction;
    type GameResult = GameActionResult;

    async fn create_game(&self, players: Vec<PeerId>) -> Result<(GameId, Self::GameState)> {
        if players.len() > self.config.max_players_per_game {
            return Err(Error::GameError("Too many players".to_string()));
        }
        
        let game_id = GameId::new();
        let game = CrapsGame::new(players);
        Ok((game_id, game))
    }
    
    async fn process_action(
        &self,
        game_id: &GameId,
        player_id: &PeerId,
        action: Self::GameAction,
        state: &mut Self::GameState,
    ) -> Result<Self::GameResult> {
        match action {
            GameAction::PlaceBet { bet_type, amount } => {
                if amount < self.config.min_bet_amount || amount > self.config.max_bet_amount {
                    return Err(Error::GameError("Invalid bet amount".to_string()));
                }
                
                state.place_bet(*player_id, bet_type, amount)?;
                Ok(GameActionResult::BetPlaced {
                    player: *player_id,
                    bet_type,
                    amount,
                })
            },
            GameAction::RollDice => {
                if !state.can_roll_dice(*player_id) {
                    return Err(Error::GameError("Cannot roll dice now".to_string()));
                }
                
                let roll = state.roll_dice()?;
                Ok(GameActionResult::DiceRolled {
                    roller: *player_id,
                    roll,
                    new_phase: state.phase,
                })
            },
            GameAction::CashOut => {
                let winnings = state.cash_out(*player_id)?;
                Ok(GameActionResult::CashOut {
                    player: *player_id,
                    amount: winnings,
                })
            }
        }
    }
    
    fn is_game_complete(&self, state: &Self::GameState) -> bool {
        matches!(state.phase, GamePhase::GameOver)
    }
    
    fn get_valid_actions(&self, state: &Self::GameState, player_id: &PeerId) -> Vec<Self::GameAction> {
        let mut actions = Vec::new();
        
        if state.can_place_bet(*player_id) {
            // Add betting actions based on current phase
            match state.phase {
                GamePhase::ComeOut => {
                    actions.push(GameAction::PlaceBet { 
                        bet_type: BetType::Pass, 
                        amount: self.config.min_bet_amount 
                    });
                    actions.push(GameAction::PlaceBet { 
                        bet_type: BetType::DontPass, 
                        amount: self.config.min_bet_amount 
                    });
                },
                GamePhase::Point(_) => {
                    actions.push(GameAction::PlaceBet { 
                        bet_type: BetType::Come, 
                        amount: self.config.min_bet_amount 
                    });
                    actions.push(GameAction::PlaceBet { 
                        bet_type: BetType::DontCome, 
                        amount: self.config.min_bet_amount 
                    });
                },
                _ => {}
            }
        }
        
        if state.can_roll_dice(*player_id) {
            actions.push(GameAction::RollDice);
        }
        
        if state.can_cash_out(*player_id) {
            actions.push(GameAction::CashOut);
        }
        
        actions
    }
}