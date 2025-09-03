//! Game Engine Service Types
//!
//! Defines the data structures used by the game engine service.

use crate::protocol::craps::{BetType, CrapTokens, DiceRoll, GamePhase};
use crate::protocol::{GameId, PeerId};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Game actions that players can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameAction {
    PlaceBet { bet_type: BetType, amount: u64 },
    RollDice,
    CashOut,
}

/// Results of game actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameActionResult {
    BetPlaced {
        player: PeerId,
        bet_type: BetType,
        amount: u64,
    },
    DiceRolled {
        roller: PeerId,
        roll: DiceRoll,
        new_phase: GamePhase,
    },
    CashOut {
        player: PeerId,
        amount: u64,
    },
}

/// Game session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSessionInfo {
    pub game_id: GameId,
    pub players: Vec<PeerId>,
    pub phase: GamePhase,
    pub created_at: u64,
    pub last_activity: u64,
    pub is_active: bool,
}

impl GameSessionInfo {
    pub fn new(game_id: GameId, players: Vec<PeerId>, phase: GamePhase) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            game_id,
            players,
            phase,
            created_at: now,
            last_activity: now,
            is_active: true,
        }
    }
    
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Game statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub total_games: u64,
    pub active_games: u64,
    pub completed_games: u64,
    pub total_players: u64,
    pub total_bets: u64,
    pub total_volume: u64,
}

impl Default for GameStats {
    fn default() -> Self {
        Self {
            total_games: 0,
            active_games: 0,
            completed_games: 0,
            total_players: 0,
            total_bets: 0,
            total_volume: 0,
        }
    }
}

/// Game creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGameRequest {
    pub players: Vec<PeerId>,
    pub game_type: String,
    pub config: Option<serde_json::Value>,
}

/// Game creation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGameResponse {
    pub game_id: GameId,
    pub session_info: GameSessionInfo,
}

/// Process action request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessActionRequest {
    pub game_id: GameId,
    pub player_id: PeerId,
    pub action: GameAction,
}

/// Process action response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessActionResponse {
    pub result: GameActionResult,
    pub updated_session: GameSessionInfo,
}

/// Get game state request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGameStateRequest {
    pub game_id: GameId,
}

/// Get game state response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGameStateResponse {
    pub session_info: GameSessionInfo,
    pub valid_actions: std::collections::HashMap<PeerId, Vec<GameAction>>,
}

/// List games request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGamesRequest {
    pub player_id: Option<PeerId>,
    pub active_only: bool,
    pub limit: Option<usize>,
}

/// List games response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGamesResponse {
    pub games: Vec<GameSessionInfo>,
    pub total_count: usize,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub stats: GameStats,
}

/// Service error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceError {
    GameNotFound(GameId),
    PlayerNotFound(PeerId),
    InvalidAction(String),
    ServiceUnavailable(String),
    InternalError(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::GameNotFound(id) => write!(f, "Game not found: {:?}", id),
            ServiceError::PlayerNotFound(id) => write!(f, "Player not found: {:?}", id),
            ServiceError::InvalidAction(msg) => write!(f, "Invalid action: {}", msg),
            ServiceError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
            ServiceError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ServiceError {}