//! SDK Type Definitions
//!
//! Core data structures and types used throughout the BitCraps SDK.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Unique identifier types
pub type GameId = String;
pub type PlayerId = String;
pub type PeerId = String;
pub type ProposalId = String;
pub type TransactionId = String;
pub type SessionId = String;

/// Game information structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameInfo {
    pub id: GameId,
    pub name: String,
    pub game_type: GameType,
    pub status: GameStatus,
    pub current_players: u32,
    pub max_players: u32,
    pub min_bet: u64,
    pub max_bet: u64,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub host: PlayerId,
    pub is_private: bool,
    pub tags: Vec<String>,
}

/// Detailed game information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetails {
    #[serde(flatten)]
    pub info: GameInfo,
    pub players: Vec<PlayerInfo>,
    pub game_state: GameState,
    pub rules: GameRules,
    pub statistics: GameStatistics,
    pub chat_enabled: bool,
    pub spectators_allowed: bool,
    pub replay_available: bool,
}

/// Game types supported by the platform
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameType {
    Craps,
    Poker,
    Blackjack,
    Roulette,
    Custom,
}

impl GameType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameType::Craps => "craps",
            GameType::Poker => "poker",
            GameType::Blackjack => "blackjack",
            GameType::Roulette => "roulette",
            GameType::Custom => "custom",
        }
    }
}

/// Game status enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameStatus {
    Waiting,
    InProgress,
    Paused,
    Finished,
    Cancelled,
    Error,
}

/// Game state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub current_round: u32,
    pub current_player: Option<PlayerId>,
    pub phase: GamePhase,
    pub pot_size: u64,
    pub last_action: Option<GameAction>,
    pub dice_state: Option<DiceState>,
    pub betting_round: Option<BettingRound>,
    pub time_remaining: Option<u64>, // seconds
}

/// Game phases
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GamePhase {
    Setup,
    Betting,
    Action,
    Resolution,
    Payout,
    GameOver,
}

/// Dice state for craps games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceState {
    pub dice1: Option<u8>,
    pub dice2: Option<u8>,
    pub roll_count: u32,
    pub point: Option<u8>,
    pub come_out_roll: bool,
}

/// Betting round information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettingRound {
    pub round_number: u32,
    pub min_bet: u64,
    pub max_bet: u64,
    pub current_bets: HashMap<PlayerId, u64>,
    pub time_limit: Option<u64>,
}

/// Player information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub username: String,
    pub balance: u64,
    pub status: PlayerStatus,
    pub position: Option<u8>,
    pub avatar_url: Option<String>,
    pub join_time: DateTime<Utc>,
    pub stats: PlayerStats,
}

/// Player status in game
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlayerStatus {
    Active,
    Inactive,
    Spectating,
    Disconnected,
    Eliminated,
}

/// Player statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlayerStats {
    pub games_played: u32,
    pub games_won: u32,
    pub total_winnings: i64,
    pub current_streak: i32,
    pub best_streak: u32,
    pub average_bet: u64,
    pub reputation_score: f64,
}

/// Game rules configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRules {
    pub game_type: GameType,
    pub max_players: u32,
    pub min_players: u32,
    pub betting_limits: BettingLimits,
    pub time_limits: TimeLimits,
    pub house_edge: f64,
    pub custom_rules: HashMap<String, serde_json::Value>,
}

/// Betting limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettingLimits {
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_total_bet: Option<u64>,
    pub bet_increment: Option<u64>,
}

/// Time limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLimits {
    pub turn_timeout: Option<u64>,
    pub betting_timeout: Option<u64>,
    pub game_timeout: Option<u64>,
    pub pause_timeout: Option<u64>,
}

/// Game statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameStatistics {
    pub total_rounds: u32,
    pub total_bets: u64,
    pub total_payout: u64,
    pub average_round_time: f64,
    pub players_joined: u32,
    pub players_left: u32,
    pub dice_rolls: Vec<(u8, u8)>,
    pub biggest_win: u64,
    pub biggest_loss: u64,
}

/// Game actions that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameAction {
    JoinGame,
    LeaveGame,
    PlaceBet { bet_type: String, amount: u64 },
    RollDice,
    Pass,
    Fold,
    Call { amount: u64 },
    Raise { amount: u64 },
    AllIn,
    Chat { message: String },
    Custom { action_type: String, data: serde_json::Value },
}

/// Game session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub session_id: SessionId,
    pub game_id: GameId,
    pub player_id: PlayerId,
    pub joined_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub is_active: bool,
    pub connection_status: ConnectionStatus,
}

/// Connection status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    TimedOut,
}

/// Game filters for listing/searching
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameFilters {
    pub game_type: Option<GameType>,
    pub status: Option<GameStatus>,
    pub min_players: Option<u32>,
    pub max_players: Option<u32>,
    pub min_bet: Option<u64>,
    pub max_bet: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub host: Option<PlayerId>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

/// Consensus proposal types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusProposal {
    pub id: ProposalId,
    pub proposer: PlayerId,
    pub action: GameAction,
    pub game_id: GameId,
    pub proposed_at: DateTime<Utc>,
    pub votes: HashMap<PlayerId, Vote>,
    pub status: ProposalStatus,
    pub required_votes: u32,
    pub timeout: DateTime<Utc>,
}

/// Vote types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Vote {
    Approve,
    Reject,
    Abstain,
}

/// Proposal status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    TimedOut,
    Cancelled,
}

/// Peer information for networking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub address: String,
    pub status: PeerStatus,
    pub last_seen: DateTime<Utc>,
    pub version: String,
    pub capabilities: Vec<String>,
    pub latency: Option<u64>,
}

/// Peer connection status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerStatus {
    Connected,
    Connecting,
    Disconnected,
    Banned,
    Unknown,
}

/// Event types for subscriptions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    GameCreated,
    GameStarted,
    GameEnded,
    PlayerJoined,
    PlayerLeft,
    BetPlaced,
    DiceRolled,
    ConsensusProposal,
    ConsensusResult,
    PeerConnected,
    PeerDisconnected,
    ChatMessage,
    SystemAnnouncement,
}

/// Event stream for real-time updates
pub struct EventStream<T> {
    receiver: tokio::sync::mpsc::UnboundedReceiver<T>,
}

impl<T> EventStream<T> {
    pub fn new(receiver: tokio::sync::mpsc::UnboundedReceiver<T>) -> Self {
        Self { receiver }
    }
    
    pub async fn next(&mut self) -> Option<T> {
        self.receiver.recv().await
    }
}

/// Health status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub overall: ServiceHealth,
    pub api: ComponentHealth,
    pub websocket: ComponentHealth,
    pub consensus: ComponentHealth,
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Service health status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServiceHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub healthy: bool,
    pub response_time_ms: u64,
    pub last_error: Option<String>,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
    pub pagination: Option<PaginationInfo>,
}

/// Pagination information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub total_items: u64,
    pub has_next: bool,
    pub has_previous: bool,
}

/// Transaction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: TransactionId,
    pub from: PlayerId,
    pub to: PlayerId,
    pub amount: u64,
    pub transaction_type: TransactionType,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub game_id: Option<GameId>,
    pub description: Option<String>,
    pub fee: u64,
}

/// Transaction types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Bet,
    Payout,
    Transfer,
    Fee,
    Refund,
}

/// Transaction status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Cancelled,
    Reversed,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WebSocketMessage {
    GameUpdate { game_id: GameId, update: GameState },
    PlayerAction { game_id: GameId, player_id: PlayerId, action: GameAction },
    ChatMessage { game_id: GameId, player_id: PlayerId, message: String, timestamp: DateTime<Utc> },
    ConsensusProposal { proposal: ConsensusProposal },
    ConsensusVote { proposal_id: ProposalId, voter: PlayerId, vote: Vote },
    PeerUpdate { peer_id: PeerId, status: PeerStatus },
    SystemNotification { message: String, level: NotificationLevel },
    Error { error: String, code: String },
    Heartbeat,
}

/// Notification levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
    Critical,
}

/// SDK metrics shared type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SDKMetrics {
    pub requests_made: u64,
    pub games_created: u64,
    pub consensus_operations: u64,
    pub websocket_connections: u64,
    pub errors_encountered: u64,
    pub average_response_time_ms: f64,
}

impl GameInfo {
    /// Check if game can accept new players
    pub fn can_accept_players(&self) -> bool {
        matches!(self.status, GameStatus::Waiting) && self.current_players < self.max_players
    }
    
    /// Check if game is in progress
    pub fn is_active(&self) -> bool {
        matches!(self.status, GameStatus::InProgress | GameStatus::Paused)
    }
}

impl PlayerInfo {
    /// Check if player can make moves
    pub fn can_act(&self) -> bool {
        matches!(self.status, PlayerStatus::Active)
    }
    
    /// Get win rate as percentage
    pub fn win_rate(&self) -> f64 {
        if self.stats.games_played == 0 {
            0.0
        } else {
            (self.stats.games_won as f64 / self.stats.games_played as f64) * 100.0
        }
    }
}

impl ConsensusProposal {
    /// Check if proposal has enough votes to pass
    pub fn has_sufficient_votes(&self) -> bool {
        let approve_votes = self.votes.values().filter(|&&v| v == Vote::Approve).count() as u32;
        approve_votes >= self.required_votes
    }
    
    /// Check if proposal has timed out
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.timeout
    }
}