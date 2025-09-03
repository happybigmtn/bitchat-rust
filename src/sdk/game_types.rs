//! Game Type Definitions for BitCraps SDK
//!
//! This module contains all the data structures and types used
//! for defining custom games in the BitCraps ecosystem.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::sdk::templates::{GameTemplate, GameRules, GameStateSchema};

/// Custom game definition
#[derive(Debug, Clone)]
pub struct CustomGame {
    pub id: String,
    pub name: String,
    pub template: GameTemplate,
    pub config: GameConfig,
    pub rules: GameRules,
    pub state_schema: GameStateSchema,
}

/// Game configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub house_edge: f64,
    pub payout_multipliers: HashMap<String, f64>,
    pub betting_limits: BettingLimits,
    pub time_limits: TimeLimits,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            house_edge: 2.0,
            payout_multipliers: HashMap::new(),
            betting_limits: BettingLimits::default(),
            time_limits: TimeLimits::default(),
        }
    }
}

/// Betting limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettingLimits {
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_total_bet: u64,
}

impl Default for BettingLimits {
    fn default() -> Self {
        Self {
            min_bet: 1,
            max_bet: 1000,
            max_total_bet: 10000,
        }
    }
}

/// Time limits configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLimits {
    pub action_timeout_seconds: u64,
    pub game_timeout_minutes: u64,
}

impl Default for TimeLimits {
    fn default() -> Self {
        Self {
            action_timeout_seconds: 30,
            game_timeout_minutes: 120,
        }
    }
}

/// Game package for distribution
pub struct GamePackage {
    pub metadata: GameMetadata,
    pub engine: Box<dyn crate::gaming::GameEngine + Send + Sync>,
    pub assets: Vec<GameAsset>,
    pub documentation: String,
}

/// Game metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

/// Game asset (textures, sounds, etc.)
#[derive(Debug, Clone)]
pub struct GameAsset {
    pub name: String,
    pub asset_type: AssetType,
    pub data: Vec<u8>,
}

/// Asset type enumeration
#[derive(Debug, Clone)]
pub enum AssetType {
    Texture,
    Sound,
    Model,
    Config,
    Other(String),
}

// Game-specific state structures

/// Dice game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceGameState {
    pub dice: Vec<u8>,
    pub phase: DicePhase,
    pub bets: Vec<DiceBet>,
    pub roll_result: Option<DiceRollResult>,
    pub current_player: Option<String>,
}

/// Phases in a dice game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DicePhase {
    WaitingForBets,
    Rolling,
    PayingOut,
    GameOver,
}

/// Bet placed in a dice game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceBet {
    pub player_id: String,
    pub amount: u64,
    pub bet_type: DiceBetType,
}

/// Types of bets in dice games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiceBetType {
    Pass,
    DontPass,
    Come,
    DontCome,
    Field,
    Place(u8),
    Buy(u8),
    Lay(u8),
    HardWay(u8),
    PropBet(String),
}

/// Result of a dice roll
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceRollResult {
    pub dice_values: Vec<u8>,
    pub total: u8,
    pub is_natural: bool,
    pub is_craps: bool,
    pub winning_bets: Vec<String>,
    pub losing_bets: Vec<String>,
}

/// Card game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGameState {
    pub deck: Vec<Card>,
    pub players: HashMap<String, PlayerHand>,
    pub current_turn: String,
    pub pot: u64,
    pub betting_round: u32,
    pub community_cards: Vec<Card>,
}

/// Playing card
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

/// Card suits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

/// Card ranks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rank {
    Ace = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
}

/// Player's hand in a card game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHand {
    pub cards: Vec<Card>,
    pub current_bet: u64,
    pub total_bet: u64,
    pub is_folded: bool,
}

/// Actions in a card game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardAction {
    Deal,
    Hit,
    Stand,
    Bet(u64),
    Raise(u64),
    Call,
    Fold,
    Check,
    AllIn,
}

/// Auction game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionGameState {
    pub current_item: Option<AuctionItem>,
    pub highest_bid: Option<Bid>,
    pub participants: HashMap<String, u64>, // player_id -> balance
    pub auction_queue: Vec<AuctionItem>,
    pub completed_auctions: Vec<CompletedAuction>,
    pub time_remaining: u64,
}

/// Item being auctioned
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub starting_bid: u64,
    pub reserve_price: Option<u64>,
    pub estimated_value: u64,
}

/// Bid on an auction item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bid {
    pub bidder_id: String,
    pub amount: u64,
    pub timestamp: std::time::SystemTime,
}

/// Completed auction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedAuction {
    pub item: AuctionItem,
    pub winning_bid: Bid,
    pub total_bids: u32,
    pub duration_seconds: u64,
}

/// Actions in an auction game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuctionAction {
    PlaceBid(u64),
    Pass,
    StartAuction(AuctionItem),
    EndAuction,
    WithdrawBid,
}

/// Strategy game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyGameState {
    pub board: GameBoard,
    pub current_player: String,
    pub turn_count: u32,
    pub players: HashMap<String, StrategyPlayer>,
    pub winner: Option<String>,
    pub game_phase: GamePhase,
}

/// Game board for strategy games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameBoard {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Vec<Tile>>,
}

/// Individual tile on the game board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub tile_type: TileType,
    pub occupant: Option<String>, // unit id
}

/// Types of tiles on the board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileType {
    Empty,
    Wall,
    Water,
    Forest,
    Mountain,
    City,
    Resource(String),
}

/// Player in a strategy game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPlayer {
    pub id: String,
    pub name: String,
    pub units: Vec<GameUnit>,
    pub resources: HashMap<String, u64>,
    pub score: u64,
}

/// Game unit (piece) in strategy games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameUnit {
    pub id: String,
    pub unit_type: String,
    pub x: u32,
    pub y: u32,
    pub health: u32,
    pub attack: u32,
    pub movement: u32,
}

/// Move in a strategy game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMove {
    pub unit_id: String,
    pub move_type: MoveType,
    pub from_x: u32,
    pub from_y: u32,
    pub to_x: u32,
    pub to_y: u32,
}

/// Types of moves in strategy games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveType {
    Move,
    Attack,
    Defend,
    Build(String),
    Collect,
    Special(String),
}

/// Puzzle game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PuzzleGameState {
    pub puzzle: Puzzle,
    pub solution: Option<Solution>,
    pub hints_used: u32,
    pub max_hints: u32,
    pub time_remaining: u64,
    pub attempts: u32,
    pub score: u64,
}

/// Puzzle definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Puzzle {
    pub id: String,
    pub puzzle_type: PuzzleType,
    pub difficulty: Difficulty,
    pub data: serde_json::Value,
    pub constraints: Vec<String>,
}

/// Difficulty levels for puzzles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Expert,
    Master,
}

/// Types of puzzles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuzzleType {
    Logic,
    Math,
    Pattern,
    Word,
    Visual,
    Memory,
    Custom(String),
}

/// Solution to a puzzle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub data: serde_json::Value,
    pub explanation: Option<String>,
    pub steps: Vec<String>,
}

/// Actions in puzzle games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuzzleAction {
    SubmitSolution(Solution),
    RequestHint,
    ResetPuzzle,
    SkipPuzzle,
    SaveProgress,
}

/// Generic game phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    Setup,
    Playing,
    Paused,
    Finished,
}

/// Custom player in a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPlayer {
    pub id: String,
    pub name: String,
    pub balance: u64,
    pub status: PlayerStatus,
    pub joined_at: std::time::SystemTime,
}

/// Player status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerStatus {
    Active,
    Inactive,
    Disconnected,
    Eliminated,
}

/// Programming languages for code generation
#[derive(Debug, Clone, Copy)]
pub enum ProgrammingLanguage {
    Rust,
    TypeScript,
    Python,
    JavaScript,
    Go,
    CSharp,
}

impl ProgrammingLanguage {
    /// Get file extension for the language
    pub fn file_extension(&self) -> &'static str {
        match self {
            ProgrammingLanguage::Rust => "rs",
            ProgrammingLanguage::TypeScript => "ts",
            ProgrammingLanguage::Python => "py",
            ProgrammingLanguage::JavaScript => "js",
            ProgrammingLanguage::Go => "go",
            ProgrammingLanguage::CSharp => "cs",
        }
    }

    /// Get the language name as string
    pub fn name(&self) -> &'static str {
        match self {
            ProgrammingLanguage::Rust => "Rust",
            ProgrammingLanguage::TypeScript => "TypeScript",
            ProgrammingLanguage::Python => "Python",
            ProgrammingLanguage::JavaScript => "JavaScript",
            ProgrammingLanguage::Go => "Go",
            ProgrammingLanguage::CSharp => "C#",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_config_defaults() {
        let config = GameConfig::default();
        assert_eq!(config.house_edge, 2.0);
        assert_eq!(config.betting_limits.min_bet, 1);
        assert_eq!(config.betting_limits.max_bet, 1000);
        assert_eq!(config.time_limits.action_timeout_seconds, 30);
    }

    #[test]
    fn test_programming_language_methods() {
        let rust = ProgrammingLanguage::Rust;
        assert_eq!(rust.file_extension(), "rs");
        assert_eq!(rust.name(), "Rust");

        let ts = ProgrammingLanguage::TypeScript;
        assert_eq!(ts.file_extension(), "ts");
        assert_eq!(ts.name(), "TypeScript");
    }

    #[test]
    fn test_card_creation() {
        let card = Card {
            suit: Suit::Hearts,
            rank: Rank::Ace,
        };
        
        // Test serialization
        let json = serde_json::to_string(&card).unwrap();
        let deserialized: Card = serde_json::from_str(&json).unwrap();
        
        assert!(matches!(deserialized.suit, Suit::Hearts));
        assert!(matches!(deserialized.rank, Rank::Ace));
    }

    #[test]
    fn test_dice_game_state() {
        let mut state = DiceGameState {
            dice: vec![1, 2],
            phase: DicePhase::WaitingForBets,
            bets: Vec::new(),
            roll_result: None,
            current_player: None,
        };

        // Add a bet
        state.bets.push(DiceBet {
            player_id: "player1".to_string(),
            amount: 100,
            bet_type: DiceBetType::Pass,
        });

        assert_eq!(state.bets.len(), 1);
        assert!(matches!(state.phase, DicePhase::WaitingForBets));
    }

    #[test]
    fn test_auction_item() {
        let item = AuctionItem {
            id: "item1".to_string(),
            name: "Rare Card".to_string(),
            description: "A very rare collectible card".to_string(),
            starting_bid: 50,
            reserve_price: Some(100),
            estimated_value: 200,
        };

        assert_eq!(item.starting_bid, 50);
        assert_eq!(item.reserve_price, Some(100));
        assert_eq!(item.estimated_value, 200);
    }
}