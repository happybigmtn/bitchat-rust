//! Database models and data transfer objects
//!
//! This module provides comprehensive database models for the BitCraps system,
//! including enhanced validation, business logic, and serialization support.

use crate::error::{Error, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Enhanced user model with validation and business logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub public_key: Vec<u8>,
    pub reputation: f64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_active: bool,
    pub last_seen_at: Option<i64>,
}

impl User {
    /// Create a new user with validation
    pub fn new(id: String, username: String, public_key: Vec<u8>) -> Result<Self> {
        if username.is_empty() || username.len() > 50 {
            return Err(Error::ValidationError(
                "Username must be 1-50 characters".into(),
            ));
        }

        if public_key.is_empty() || public_key.len() != 32 {
            return Err(Error::ValidationError("Public key must be 32 bytes".into()));
        }

        let now = Utc::now().timestamp();
        Ok(Self {
            id,
            username,
            public_key,
            reputation: 0.0,
            created_at: now,
            updated_at: now,
            is_active: true,
            last_seen_at: Some(now),
        })
    }

    /// Update user's reputation with bounds checking
    pub fn update_reputation(&mut self, delta: f64) -> Result<()> {
        let new_reputation = self.reputation + delta;
        if new_reputation < -100.0 || new_reputation > 100.0 {
            return Err(Error::ValidationError(
                "Reputation must be between -100 and 100".into(),
            ));
        }
        self.reputation = new_reputation;
        self.updated_at = Utc::now().timestamp();
        Ok(())
    }

    /// Check if user is in good standing
    pub fn is_good_standing(&self) -> bool {
        self.is_active && self.reputation >= -10.0
    }
}

/// Enhanced game model with state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub state: GameState,
    pub pot_size: i64,
    pub phase: GamePhase,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub winner_id: Option<String>,
    pub game_type: GameType,
    pub metadata: HashMap<String, serde_json::Value>,
    pub max_players: i32,
    pub min_bet: i64,
    pub players: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameState {
    Waiting,
    Playing,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GamePhase {
    Betting,
    Rolling,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameType {
    Craps,
    Poker,
    Blackjack,
}

impl Game {
    /// Create a new game
    pub fn new(id: String, game_type: GameType) -> Self {
        Self {
            id,
            state: GameState::Waiting,
            pot_size: 0,
            phase: GamePhase::Betting,
            created_at: Utc::now().timestamp(),
            completed_at: None,
            winner_id: None,
            game_type,
            metadata: HashMap::new(),
            max_players: 8,
            min_bet: 10,
            players: Vec::new(),
        }
    }

    /// Add a player to the game
    pub fn add_player(&mut self, player_id: String) -> Result<()> {
        if self.state != GameState::Waiting {
            return Err(Error::ValidationError(
                "Cannot join game in progress".into(),
            ));
        }

        if self.players.len() >= self.max_players as usize {
            return Err(Error::ValidationError("Game is full".into()));
        }

        if self.players.contains(&player_id) {
            return Err(Error::ValidationError("Player already in game".into()));
        }

        self.players.push(player_id);
        Ok(())
    }

    /// Start the game
    pub fn start(&mut self) -> Result<()> {
        if self.players.len() < 2 {
            return Err(Error::ValidationError(
                "Need at least 2 players to start".into(),
            ));
        }

        self.state = GameState::Playing;
        self.phase = GamePhase::Betting;
        Ok(())
    }

    /// Complete the game
    pub fn complete(&mut self, winner_id: Option<String>) -> Result<()> {
        if self.state != GameState::Playing {
            return Err(Error::ValidationError("Game is not in progress".into()));
        }

        self.state = GameState::Completed;
        self.phase = GamePhase::Resolved;
        self.completed_at = Some(Utc::now().timestamp());
        self.winner_id = winner_id;
        Ok(())
    }

    /// Get game duration in seconds
    pub fn duration_seconds(&self) -> Option<i64> {
        self.completed_at
            .map(|completed| completed - self.created_at)
    }
}

/// Enhanced bet model with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub id: Vec<u8>,
    pub game_id: String,
    pub player_id: String,
    pub bet_type: BetType,
    pub amount: i64,
    pub odds_multiplier: f64,
    pub outcome: Option<BetOutcome>,
    pub payout: i64,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BetType {
    PassLine,
    DontPass,
    Come,
    DontCome,
    Field,
    Place6,
    Place8,
    Any7,
    AnyCraps,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BetOutcome {
    Win,
    Lose,
    Push,
}

impl Bet {
    /// Create a new bet with validation
    pub fn new(
        id: Vec<u8>,
        game_id: String,
        player_id: String,
        bet_type: BetType,
        amount: i64,
    ) -> Result<Self> {
        if amount <= 0 {
            return Err(Error::ValidationError("Bet amount must be positive".into()));
        }

        let odds_multiplier = Self::get_odds_multiplier(&bet_type);

        Ok(Self {
            id,
            game_id,
            player_id,
            bet_type,
            amount,
            odds_multiplier,
            outcome: None,
            payout: 0,
            created_at: Utc::now().timestamp(),
            resolved_at: None,
        })
    }

    /// Get odds multiplier for bet type
    fn get_odds_multiplier(bet_type: &BetType) -> f64 {
        match bet_type {
            BetType::PassLine => 1.0,
            BetType::DontPass => 1.0,
            BetType::Come => 1.0,
            BetType::DontCome => 1.0,
            BetType::Field => 1.0,
            BetType::Place6 => 7.0 / 6.0,
            BetType::Place8 => 7.0 / 6.0,
            BetType::Any7 => 4.0,
            BetType::AnyCraps => 7.0,
        }
    }

    /// Resolve the bet
    pub fn resolve(&mut self, outcome: BetOutcome) -> Result<()> {
        if self.outcome.is_some() {
            return Err(Error::ValidationError("Bet already resolved".into()));
        }

        self.outcome = Some(outcome.clone());
        self.resolved_at = Some(Utc::now().timestamp());

        self.payout = match outcome {
            BetOutcome::Win => (self.amount as f64 * self.odds_multiplier) as i64,
            BetOutcome::Lose => 0,
            BetOutcome::Push => self.amount,
        };

        Ok(())
    }
}

/// Enhanced transaction model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub from_user_id: Option<String>,
    pub to_user_id: Option<String>,
    pub amount: i64,
    pub transaction_type: TransactionType,
    pub status: TransactionStatus,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
    pub block_height: Option<i64>,
    pub tx_hash: Option<String>,
    pub fee: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Transfer,
    BetPlaced,
    BetPayout,
    Deposit,
    Withdrawal,
    Fee,
    Reward,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Cancelled,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(
        id: String,
        from_user_id: Option<String>,
        to_user_id: Option<String>,
        amount: i64,
        transaction_type: TransactionType,
    ) -> Result<Self> {
        if amount <= 0 {
            return Err(Error::ValidationError(
                "Transaction amount must be positive".into(),
            ));
        }

        Ok(Self {
            id,
            from_user_id,
            to_user_id,
            amount,
            transaction_type,
            status: TransactionStatus::Pending,
            created_at: Utc::now().timestamp(),
            confirmed_at: None,
            block_height: None,
            tx_hash: None,
            fee: 0,
        })
    }

    /// Confirm the transaction
    pub fn confirm(&mut self, block_height: Option<i64>, tx_hash: Option<String>) -> Result<()> {
        if self.status != TransactionStatus::Pending {
            return Err(Error::ValidationError("Transaction is not pending".into()));
        }

        self.status = TransactionStatus::Confirmed;
        self.confirmed_at = Some(Utc::now().timestamp());
        self.block_height = block_height;
        self.tx_hash = tx_hash;
        Ok(())
    }

    /// Fail the transaction
    pub fn fail(&mut self) -> Result<()> {
        if self.status == TransactionStatus::Confirmed {
            return Err(Error::ValidationError(
                "Cannot fail confirmed transaction".into(),
            ));
        }

        self.status = TransactionStatus::Failed;
        Ok(())
    }
}

/// Peer connection tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConnection {
    pub id: String,
    pub peer_id: String,
    pub connection_type: ConnectionType,
    pub transport_layer: Option<TransportLayer>,
    pub signal_strength: Option<i32>,
    pub latency_ms: Option<i32>,
    pub connected_at: i64,
    pub disconnected_at: Option<i64>,
    pub data_sent_bytes: i64,
    pub data_received_bytes: i64,
    pub connection_quality: f64,
    pub error_count: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionType {
    Bluetooth,
    Tcp,
    WebSocket,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransportLayer {
    Ble,
    ClassicBt,
    WifiDirect,
    Internet,
}

/// Consensus round tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusRound {
    pub round_number: i64,
    pub game_id: Option<String>,
    pub proposer_id: String,
    pub proposal_hash: Vec<u8>,
    pub proposal_data: Option<String>,
    pub vote_threshold: i32,
    pub vote_count: i32,
    pub positive_votes: i32,
    pub negative_votes: i32,
    pub finalized: bool,
    pub consensus_type: ConsensusType,
    pub created_at: i64,
    pub voting_deadline: Option<i64>,
    pub finalized_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusType {
    GameState,
    BetResolution,
    Payout,
    Dispute,
}

/// Consensus vote
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusVote {
    pub id: String,
    pub round_number: i64,
    pub voter_id: String,
    pub vote_type: VoteType,
    pub vote_hash: Vec<u8>,
    pub signature: Vec<u8>,
    pub vote_weight: f64,
    pub reasoning: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VoteType {
    Approve,
    Reject,
    Abstain,
}

/// Game statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub game_id: String,
    pub total_bets: i32,
    pub total_wagered: i64,
    pub total_won: i64,
    pub house_edge: Option<f64>,
    pub duration_seconds: Option<i32>,
    pub player_count: i32,
    pub max_pot_size: i64,
    pub average_bet_size: Option<f64>,
    pub volatility_index: Option<f64>,
    pub fairness_score: f64,
    pub created_at: i64,
}

/// Player statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStats {
    pub player_id: String,
    pub games_played: i32,
    pub games_won: i32,
    pub games_lost: i32,
    pub total_wagered: i64,
    pub total_won: i64,
    pub net_profit: i64,
    pub win_rate: f64,
    pub avg_bet_size: i64,
    pub biggest_win: i64,
    pub biggest_loss: i64,
    pub longest_winning_streak: i32,
    pub longest_losing_streak: i32,
    pub current_streak: i32,
    pub current_streak_type: Option<StreakType>,
    pub risk_tolerance: f64,
    pub play_style: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StreakType {
    Win,
    Loss,
    None,
}

impl PlayerStats {
    /// Create new player statistics
    pub fn new(player_id: String) -> Self {
        Self {
            player_id,
            games_played: 0,
            games_won: 0,
            games_lost: 0,
            total_wagered: 0,
            total_won: 0,
            net_profit: 0,
            win_rate: 0.0,
            avg_bet_size: 0,
            biggest_win: 0,
            biggest_loss: 0,
            longest_winning_streak: 0,
            longest_losing_streak: 0,
            current_streak: 0,
            current_streak_type: Some(StreakType::None),
            risk_tolerance: 0.5,
            play_style: "balanced".to_string(),
            updated_at: Utc::now().timestamp(),
        }
    }

    /// Update statistics after a game
    pub fn update_game_result(&mut self, won: bool, amount_wagered: i64, amount_won: i64) {
        self.games_played += 1;
        self.total_wagered += amount_wagered;
        self.total_won += amount_won;
        self.net_profit = self.total_won - self.total_wagered;

        if won {
            self.games_won += 1;
            if self.current_streak_type == Some(StreakType::Win) {
                self.current_streak += 1;
            } else {
                self.current_streak = 1;
                self.current_streak_type = Some(StreakType::Win);
            }
            self.longest_winning_streak = self.longest_winning_streak.max(self.current_streak);

            if amount_won > self.biggest_win {
                self.biggest_win = amount_won;
            }
        } else {
            self.games_lost += 1;
            if self.current_streak_type == Some(StreakType::Loss) {
                self.current_streak += 1;
            } else {
                self.current_streak = 1;
                self.current_streak_type = Some(StreakType::Loss);
            }
            self.longest_losing_streak = self.longest_losing_streak.max(self.current_streak);

            let loss = amount_wagered - amount_won;
            if loss > self.biggest_loss {
                self.biggest_loss = loss;
            }
        }

        // Recalculate derived statistics
        self.win_rate = if self.games_played > 0 {
            self.games_won as f64 / self.games_played as f64
        } else {
            0.0
        };

        self.avg_bet_size = if self.games_played > 0 {
            self.total_wagered / self.games_played as i64
        } else {
            0
        };

        self.updated_at = Utc::now().timestamp();
    }
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub username: String,
    pub games_won: i32,
    pub total_won: i64,
    pub win_rate: f64,
    pub reputation: f64,
    pub rank: i32,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub event_type: AuditEventType,
    pub entity_type: Option<EntityType>,
    pub entity_id: Option<String>,
    pub user_id: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub risk_score: f64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditEventType {
    UserLogin,
    UserLogout,
    UserCreate,
    GameCreate,
    BetPlace,
    TransactionCreate,
    SecurityViolation,
    AdminAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityType {
    User,
    Game,
    Bet,
    Transaction,
    Consensus,
    System,
}

/// Performance metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    pub id: i64,
    pub metric_name: String,
    pub metric_value: f64,
    pub metric_unit: Option<String>,
    pub component: String,
    pub tags: Option<HashMap<String, String>>,
    pub created_at: i64,
}

/// System health snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub id: i64,
    pub cpu_usage: Option<f64>,
    pub memory_usage: Option<f64>,
    pub disk_usage: Option<f64>,
    pub network_in_bytes: Option<i64>,
    pub network_out_bytes: Option<i64>,
    pub active_connections: Option<i32>,
    pub error_count: i32,
    pub created_at: i64,
}
