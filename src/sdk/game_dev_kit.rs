//! Game Development Kit for BitCraps
//!
//! Tools and utilities for developing custom games on the BitCraps platform

use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use uuid::Uuid;

use crate::gaming::{GameEngine, GameEngineError};
use crate::gaming::{GameSession, PlayerJoinData, GameAction, GameActionResult, SessionEndReason};

/// Game development kit for creating custom games
pub struct GameDevKit {
    templates: HashMap<String, GameTemplate>,
    validator: GameValidator,
}

impl GameDevKit {
    /// Create new game development kit
    pub fn new() -> Self {
        let mut kit = Self {
            templates: HashMap::new(),
            validator: GameValidator::new(),
        };

        // Load built-in templates
        kit.load_builtin_templates();
        kit
    }

    /// Create a new game from template
    pub fn create_game_from_template(&self, template_name: &str, game_name: &str, config: GameConfig) -> Result<CustomGame, GameDevError> {
        let template = self.templates.get(template_name)
            .ok_or_else(|| GameDevError::TemplateNotFound(template_name.to_string()))?;

        let game = CustomGame {
            id: Uuid::new_v4().to_string(),
            name: game_name.to_string(),
            template: template.clone(),
            config,
            rules: template.default_rules.clone(),
            state_schema: template.state_schema.clone(),
        };

        Ok(game)
    }

    /// Validate a custom game
    pub async fn validate_game(&self, game: &CustomGame) -> Result<ValidationReport, GameDevError> {
        self.validator.validate(game).await
    }

    /// Generate game boilerplate code
    pub fn generate_boilerplate(&self, game: &CustomGame, language: ProgrammingLanguage) -> Result<String, GameDevError> {
        match language {
            ProgrammingLanguage::Rust => self.generate_rust_boilerplate(game),
            ProgrammingLanguage::TypeScript => self.generate_typescript_boilerplate(game),
            ProgrammingLanguage::Python => self.generate_python_boilerplate(game),
        }
    }

    /// Export game as SDK package
    pub async fn export_game(&self, game: &CustomGame, output_path: &Path) -> Result<(), GameDevError> {
        // Create game package structure
        let package = GamePackage {
            metadata: GameMetadata {
                id: game.id.clone(),
                name: game.name.clone(),
                version: "1.0.0".to_string(),
                author: "Developer".to_string(),
                description: game.template.description.clone(),
            },
            engine: Box::new(CustomGameEngine::from_game(game.clone())),
            assets: vec![], // Would include game assets
            documentation: self.generate_documentation(game)?,
        };

        // Write package to filesystem
        self.write_game_package(&package, output_path).await?;

        Ok(())
    }

    /// Get available templates
    pub fn get_templates(&self) -> Vec<&GameTemplate> {
        self.templates.values().collect()
    }

    /// Add custom template
    pub fn add_template(&mut self, template: GameTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Load built-in game templates
    fn load_builtin_templates(&mut self) {
        // Card game template
        let card_game_template = GameTemplate {
            name: "card_game".to_string(),
            description: "Basic card game with deck management".to_string(),
            category: GameCategory::CardGame,
            min_players: 2,
            max_players: 8,
            default_rules: GameRules {
                deck_size: Some(52),
                hand_size: Some(7),
                turn_time_limit: Some(30),
                betting_rounds: Some(3),
                custom_rules: HashMap::new(),
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "deck".to_string(), field_type: "Vec<Card>".to_string() },
                    StateField { name: "players".to_string(), field_type: "HashMap<String, Player>".to_string() },
                    StateField { name: "current_turn".to_string(), field_type: "String".to_string() },
                ],
            },
            required_actions: vec![
                "deal_cards".to_string(),
                "play_card".to_string(),
                "end_turn".to_string(),
            ],
        };

        // Dice game template
        let dice_game_template = GameTemplate {
            name: "dice_game".to_string(),
            description: "Dice-based game with configurable rules".to_string(),
            category: GameCategory::DiceGame,
            min_players: 1,
            max_players: 12,
            default_rules: GameRules {
                deck_size: None,
                hand_size: None,
                turn_time_limit: Some(15),
                betting_rounds: Some(1),
                custom_rules: {
                    let mut rules = HashMap::new();
                    rules.insert("dice_count".to_string(), serde_json::Value::Number(2.into()));
                    rules.insert("sides_per_die".to_string(), serde_json::Value::Number(6.into()));
                    rules
                },
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "dice".to_string(), field_type: "Vec<u8>".to_string() },
                    StateField { name: "phase".to_string(), field_type: "GamePhase".to_string() },
                    StateField { name: "bets".to_string(), field_type: "Vec<Bet>".to_string() },
                ],
            },
            required_actions: vec![
                "roll_dice".to_string(),
                "place_bet".to_string(),
                "resolve_bets".to_string(),
            ],
        };

        // Auction game template
        let auction_game_template = GameTemplate {
            name: "auction_game".to_string(),
            description: "Auction-style game with bidding mechanics".to_string(),
            category: GameCategory::AuctionGame,
            min_players: 2,
            max_players: 10,
            default_rules: GameRules {
                deck_size: None,
                hand_size: None,
                turn_time_limit: Some(60),
                betting_rounds: None,
                custom_rules: {
                    let mut rules = HashMap::new();
                    rules.insert("starting_bid".to_string(), serde_json::Value::Number(10.into()));
                    rules.insert("bid_increment".to_string(), serde_json::Value::Number(5.into()));
                    rules.insert("auction_time_limit".to_string(), serde_json::Value::Number(120.into()));
                    rules
                },
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "current_item".to_string(), field_type: "AuctionItem".to_string() },
                    StateField { name: "highest_bid".to_string(), field_type: "Bid".to_string() },
                    StateField { name: "time_remaining".to_string(), field_type: "Duration".to_string() },
                ],
            },
            required_actions: vec![
                "place_bid".to_string(),
                "pass".to_string(),
                "end_auction".to_string(),
            ],
        };

        self.templates.insert(card_game_template.name.clone(), card_game_template);
        self.templates.insert(dice_game_template.name.clone(), dice_game_template);
        self.templates.insert(auction_game_template.name.clone(), auction_game_template);
    }

    fn generate_rust_boilerplate(&self, game: &CustomGame) -> Result<String, GameDevError> {
        let code = format!(r#"
//! Custom game: {}
//! Generated by BitCraps Game Development Kit

use async_trait::async_trait;
use crate::gaming::{{GameEngine, GameEngineError, GameSession, PlayerJoinData, GameAction, GameActionResult, SessionEndReason}};

pub struct {}GameEngine {{
    // Game state and configuration
}}

impl {}GameEngine {{
    pub fn new() -> Self {{
        Self {{
            // Initialize game engine
        }}
    }}
}}

#[async_trait]
impl GameEngine for {}GameEngine {{
    fn get_name(&self) -> String {{
        "{}".to_string()
    }}

    fn get_description(&self) -> String {{
        "{}".to_string()
    }}

    fn get_min_players(&self) -> usize {{
        {}
    }}

    fn get_max_players(&self) -> usize {{
        {}
    }}

    fn get_supported_bet_types(&self) -> Vec<String> {{
        // Return bet types appropriate for this game
        {}
    }}

    fn get_house_edge(&self) -> f64 {{
        {}  // Real house edge based on game type
    }}

    async fn is_available(&self) -> bool {{
        true
    }}

    async fn validate(&self) -> Result<(), GameEngineError> {{
        // Validate game engine configuration and state
        Ok(())
    }}

    async fn validate_session_config(&self, _config: &crate::gaming::GameSessionConfig) -> Result<(), GameEngineError> {{
        // Validate session configuration parameters
        Ok(())
    }}

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), crate::gaming::GameFrameworkError> {{
        // Initialize game session state
        Ok(())
    }}

    async fn validate_player_join(&self, _session: &GameSession, _player_id: &str, _join_data: &PlayerJoinData) -> Result<(), crate::gaming::GameFrameworkError> {{
        // Validate player can join this session
        Ok(())
    }}

    async fn on_player_joined(&self, _session: &GameSession, _player_id: &str) -> Result<(), crate::gaming::GameFrameworkError> {{
        // Handle new player joining the game
        Ok(())
    }}

    async fn process_action(&self, _session: &GameSession, _player_id: &str, action: GameAction) -> Result<GameActionResult, crate::gaming::GameFrameworkError> {{
        // Process game actions based on current state and rules
        match action {{
            // Handle different action types
            _ => Err(crate::gaming::GameFrameworkError::UnsupportedAction("Action not implemented".to_string())),
        }}
    }}

    async fn on_session_ended(&self, _session: &GameSession, _reason: &SessionEndReason) -> Result<(), crate::gaming::GameFrameworkError> {{
        // Clean up when session ends
        Ok(())
    }}
}}

// Game-specific types and structures
{}

#[cfg(test)]
mod tests {{
    use super::*;

    #[tokio::test]
    async fn test_{}_engine_creation() {{
        let engine = {}GameEngine::new();
        assert_eq!(engine.get_name(), "{}");
    }}
}}
"#,
            game.name,                                    // Custom game: {}
            to_pascal_case(&game.name),                   // {}GameEngine
            to_pascal_case(&game.name),                   // {}GameEngine
            to_pascal_case(&game.name),                   // {}GameEngine
            game.name,                                    // get_name()
            game.template.description,                    // get_description()
            game.template.min_players,                    // get_min_players()
            game.template.max_players,                    // get_max_players()
            game.config.house_edge,                       // get_house_edge()
            self.generate_game_types(game),               // Game-specific types
            to_snake_case(&game.name),                    // test_{}_engine_creation
            to_pascal_case(&game.name),                   // {}GameEngine::new()
            game.name,                                    // assert_eq!
        );

        Ok(code)
    }

    fn generate_typescript_boilerplate(&self, game: &CustomGame) -> Result<String, GameDevError> {
        let code = format!(r#"
/**
 * Custom game: {}
 * Generated by BitCraps Game Development Kit
 */

import {{ GameEngine, GameSession, PlayerJoinData, GameAction, GameActionResult, SessionEndReason }} from '@bitcraps/sdk';

export class {}GameEngine implements GameEngine {{
    constructor() {{
        // Initialize game engine
    }}

    getName(): string {{
        return '{}';
    }}

    getDescription(): string {{
        return '{}';
    }}

    getMinPlayers(): number {{
        return {};
    }}

    getMaxPlayers(): number {{
        return {};
    }}

    getSupportedBetTypes(): string[] {{
        return ["Standard"];  // Implement game-specific bet types
    }}

    getHouseEdge(): number {{
        return {};  // Real house edge based on game type
    }}

    async isAvailable(): Promise<boolean> {{
        return true;
    }}

    async validate(): Promise<void> {{
        // Validate game engine configuration
    }}

    async validateSessionConfig(config: any): Promise<void> {{
        // Validate session configuration
    }}

    async initializeSession(session: GameSession): Promise<void> {{
        // Initialize game session state
    }}

    async validatePlayerJoin(session: GameSession, playerId: string, joinData: PlayerJoinData): Promise<void> {{
        // Validate player can join this session
    }}

    async onPlayerJoined(session: GameSession, playerId: string): Promise<void> {{
        // Handle new player joining the game
    }}

    async processAction(session: GameSession, playerId: string, action: GameAction): Promise<GameActionResult> {{
        // Process player actions
        throw new Error('Action not implemented');
    }}

    async onSessionEnded(session: GameSession, reason: SessionEndReason): Promise<void> {{
        // Clean up when session ends
    }}
}}

// Game-specific types and interfaces
{}

// Export game engine
export default {}GameEngine;
"#,
            game.name,
            to_pascal_case(&game.name),
            game.name,
            game.template.description,
            game.template.min_players,
            game.template.max_players,
            game.config.house_edge,
            self.generate_typescript_types(game),
            to_pascal_case(&game.name),
        );

        Ok(code)
    }

    fn generate_python_boilerplate(&self, game: &CustomGame) -> Result<String, GameDevError> {
        let code = format!(r#"
"""
Custom game: {}
Generated by BitCraps Game Development Kit
"""

from abc import ABC, abstractmethod
from typing import List, Dict, Any
from bitcraps_sdk import GameEngine, GameSession, PlayerJoinData, GameAction, GameActionResult, SessionEndReason

class {}GameEngine(GameEngine):
    """Custom game engine for {}"""

    def __init__(self):
        """Initialize game engine"""
        super().__init__()

    def get_name(self) -> str:
        return "{}"

    def get_description(self) -> str:
        return "{}"

    def get_min_players(self) -> int:
        return {}

    def get_max_players(self) -> int:
        return {}

    def get_supported_bet_types(self) -> List[str]:
        return ["Standard"]  # Implement game-specific bet types

    def get_house_edge(self) -> float:
        return {}  # Real house edge based on game type

    async def is_available(self) -> bool:
        return True

    async def validate(self) -> None:
        """Validate game engine configuration"""
        # Validate game engine configuration
        pass

    async def validate_session_config(self, config: Dict[str, Any]) -> None:
        """Validate session configuration"""
        # Validate session configuration
        pass

    async def initialize_session(self, session: GameSession) -> None:
        """Initialize game session"""
        # Initialize game session state
        pass

    async def validate_player_join(self, session: GameSession, player_id: str, join_data: PlayerJoinData) -> None:
        """Validate player join request"""
        # Validate player can join this session
        pass

    async def on_player_joined(self, session: GameSession, player_id: str) -> None:
        """Handle player joined event"""
        # Handle new player joining the game
        pass

    async def process_action(self, session: GameSession, player_id: str, action: GameAction) -> GameActionResult:
        """Process game action"""
        # Process player actions
        raise NotImplementedError("Action processing not implemented")

    async def on_session_ended(self, session: GameSession, reason: SessionEndReason) -> None:
        """Handle session ended event"""
        # Clean up when session ends
        pass

# Game-specific classes and functions
{}

# Example usage
if __name__ == "__main__":
    engine = {}GameEngine()
    print(f"Game: {{engine.get_name()}}")
    print(f"Description: {{engine.get_description()}}")
"#,
            game.name,
            to_pascal_case(&game.name),
            game.name,
            game.name,
            game.template.description,
            game.template.min_players,
            game.template.max_players,
            game.config.house_edge,
            self.generate_python_types(game),
            to_pascal_case(&game.name),
        );

        Ok(code)
    }

    fn generate_game_types(&self, game: &CustomGame) -> String {
        // Generate comprehensive Rust type definitions based on game template
        match &game.template.category {
            GameCategory::DiceGame => {
                format!(r#"
// Dice Game Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceGameState {{
    pub dice: Vec<u8>,
    pub phase: DicePhase,
    pub bets: HashMap<String, Vec<DiceBet>>,
    pub pot: u64,
    pub round_number: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DicePhase {{
    ComeOut,
    Point(u8),
    Resolved,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceBet {{
    pub player_id: String,
    pub bet_type: DiceBetType,
    pub amount: u64,
    pub placed_at: u64,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiceBetType {{
    PassLine,
    DontPassLine,
    Come,
    DontCome,
    Field,
    Place(u8), // Place bet on specific number
    Hard(u8),  // Hard way bet
    Any7,
    Custom(String),
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceRollResult {{
    pub dice: Vec<u8>,
    pub total: u8,
    pub winning_bets: Vec<String>,
    pub losing_bets: Vec<String>,
    pub payouts: HashMap<String, u64>,
}}"#)
            },
            GameCategory::CardGame => {
                format!(r#"
// Card Game Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardGameState {{
    pub deck: Vec<Card>,
    pub players: HashMap<String, PlayerHand>,
    pub community_cards: Vec<Card>,
    pub current_turn: Option<String>,
    pub pot: u64,
    pub betting_round: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {{
    pub suit: Suit,
    pub rank: Rank,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Suit {{
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rank {{
    Ace,
    Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten,
    Jack, Queen, King,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHand {{
    pub cards: Vec<Card>,
    pub total_value: u32,
    pub is_folded: bool,
    pub current_bet: u64,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardAction {{
    Deal,
    Hit,
    Stand,
    Fold,
    Raise(u64),
    Call,
    Check,
}}"#)
            },
            GameCategory::AuctionGame => {
                format!(r#"
// Auction Game Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionGameState {{
    pub current_item: Option<AuctionItem>,
    pub highest_bid: Option<Bid>,
    pub time_remaining: Duration,
    pub bidding_history: Vec<Bid>,
    pub completed_auctions: Vec<CompletedAuction>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuctionItem {{
    pub id: String,
    pub name: String,
    pub description: String,
    pub starting_price: u64,
    pub reserve_price: Option<u64>,
    pub category: String,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bid {{
    pub bidder_id: String,
    pub amount: u64,
    pub timestamp: u64,
    pub is_reserve_met: bool,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedAuction {{
    pub item: AuctionItem,
    pub winning_bid: Option<Bid>,
    pub total_bids: u32,
    pub completed_at: u64,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuctionAction {{
    PlaceBid(u64),
    Pass,
    EndAuction,
    AddItem(AuctionItem),
}}"#)
            },
            GameCategory::StrategyGame => {
                format!(r#"
// Strategy Game Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyGameState {{
    pub board: GameBoard,
    pub players: HashMap<String, StrategyPlayer>,
    pub current_turn: String,
    pub move_history: Vec<GameMove>,
    pub turn_number: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameBoard {{
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Vec<Tile>>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {{
    pub position: (u32, u32),
    pub tile_type: TileType,
    pub occupant: Option<String>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileType {{
    Empty,
    Wall,
    Resource(String),
    Special(String),
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPlayer {{
    pub resources: HashMap<String, u64>,
    pub units: Vec<GameUnit>,
    pub score: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameUnit {{
    pub id: String,
    pub position: (u32, u32),
    pub unit_type: String,
    pub health: u32,
    pub can_move: bool,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMove {{
    pub player_id: String,
    pub move_type: MoveType,
    pub timestamp: u64,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MoveType {{
    PlaceUnit {{ unit_type: String, position: (u32, u32) }},
    MoveUnit {{ unit_id: String, to_position: (u32, u32) }},
    UseResource {{ resource_type: String, amount: u64 }},
    EndTurn,
}}"#)
            },
            GameCategory::PuzzleGame => {
                format!(r#"
// Puzzle Game Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PuzzleGameState {{
    pub puzzle: Puzzle,
    pub player_solutions: HashMap<String, Solution>,
    pub time_started: u64,
    pub time_limit: Option<u64>,
    pub hint_count: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Puzzle {{
    pub id: String,
    pub difficulty: Difficulty,
    pub puzzle_type: PuzzleType,
    pub data: serde_json::Value,
    pub solution_hash: String,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Difficulty {{
    Easy,
    Medium,
    Hard,
    Expert,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuzzleType {{
    Mathematical,
    Logical,
    Spatial,
    WordBased,
    Custom(String),
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {{
    pub player_id: String,
    pub answer: serde_json::Value,
    pub submitted_at: u64,
    pub is_correct: Option<bool>,
    pub score: u32,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuzzleAction {{
    SubmitSolution(serde_json::Value),
    RequestHint,
    Skip,
    Reset,
}}"#)
            },
            GameCategory::Custom(category_name) => {
                format!(r#"
// Custom Game Types for {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {}GameState {{
    pub custom_data: serde_json::Value,
    pub players: HashMap<String, CustomPlayer>,
    pub game_phase: CustomPhase,
    pub metadata: HashMap<String, String>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPlayer {{
    pub id: String,
    pub score: u64,
    pub data: serde_json::Value,
    pub last_action: Option<u64>,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CustomPhase {{
    Setup,
    Playing,
    Scoring,
    Finished,
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAction {{
    pub action_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub timestamp: u64,
}}

impl Default for {}GameState {{
    fn default() -> Self {{
        Self {{
            custom_data: serde_json::Value::Null,
            players: HashMap::new(),
            game_phase: CustomPhase::Setup,
            metadata: HashMap::new(),
        }}
    }}
}}"#, category_name, to_pascal_case(category_name), to_pascal_case(category_name))
            }
        }
    }

    fn generate_typescript_types(&self, game: &CustomGame) -> String {
        // Generate comprehensive TypeScript interface definitions based on game template
        match &game.template.category {
            GameCategory::DiceGame => {
                format!(r#"
// Dice Game TypeScript Interfaces
export interface DiceGameState {{
    dice: number[];
    phase: DicePhase;
    bets: {{ [playerId: string]: DiceBet[] }};
    pot: number;
    roundNumber: number;
}}

export enum DicePhase {{
    ComeOut = "come-out",
    Point = "point",
    Resolved = "resolved",
}}

export interface DiceBet {{
    playerId: string;
    betType: DiceBetType;
    amount: number;
    placedAt: number;
}}

export enum DiceBetType {{
    PassLine = "pass-line",
    DontPassLine = "dont-pass-line",
    Come = "come",
    DontCome = "dont-come",
    Field = "field",
    Place4 = "place-4",
    Place5 = "place-5",
    Place6 = "place-6",
    Place8 = "place-8",
    Place9 = "place-9",
    Place10 = "place-10",
    Hard4 = "hard-4",
    Hard6 = "hard-6",
    Hard8 = "hard-8",
    Hard10 = "hard-10",
    Any7 = "any-7",
}}

export interface DiceRollResult {{
    dice: number[];
    total: number;
    winningBets: string[];
    losingBets: string[];
    payouts: {{ [playerId: string]: number }};
}}

export type DiceGameActions =
    | {{ type: 'roll-dice' }}
    | {{ type: 'place-bet'; betType: DiceBetType; amount: number }}
    | {{ type: 'resolve-bets'; rollResult: DiceRollResult }};"#)
            },
            GameCategory::CardGame => {
                format!(r#"
// Card Game TypeScript Interfaces
export interface CardGameState {{
    deck: Card[];
    players: {{ [playerId: string]: PlayerHand }};
    communityCards: Card[];
    currentTurn: string | null;
    pot: number;
    bettingRound: number;
}}

export interface Card {{
    suit: Suit;
    rank: Rank;
}}

export enum Suit {{
    Hearts = "hearts",
    Diamonds = "diamonds",
    Clubs = "clubs",
    Spades = "spades",
}}

export enum Rank {{
    Ace = "ace",
    Two = "two", Three = "three", Four = "four", Five = "five",
    Six = "six", Seven = "seven", Eight = "eight", Nine = "nine", Ten = "ten",
    Jack = "jack", Queen = "queen", King = "king",
}}

export interface PlayerHand {{
    cards: Card[];
    totalValue: number;
    isFolded: boolean;
    currentBet: number;
}}

export type CardGameActions =
    | {{ type: 'deal' }}
    | {{ type: 'hit' }}
    | {{ type: 'stand' }}
    | {{ type: 'fold' }}
    | {{ type: 'raise'; amount: number }}
    | {{ type: 'call' }}
    | {{ type: 'check' }};"#)
            },
            GameCategory::AuctionGame => {
                format!(r#"
// Auction Game TypeScript Interfaces
export interface AuctionGameState {{
    currentItem: AuctionItem | null;
    highestBid: Bid | null;
    timeRemaining: number; // seconds
    biddingHistory: Bid[];
    completedAuctions: CompletedAuction[];
}}

export interface AuctionItem {{
    id: string;
    name: string;
    description: string;
    startingPrice: number;
    reservePrice?: number;
    category: string;
}}

export interface Bid {{
    bidderId: string;
    amount: number;
    timestamp: number;
    isReserveMet: boolean;
}}

export interface CompletedAuction {{
    item: AuctionItem;
    winningBid: Bid | null;
    totalBids: number;
    completedAt: number;
}}

export type AuctionGameActions =
    | {{ type: 'place-bid'; amount: number }}
    | {{ type: 'pass' }}
    | {{ type: 'end-auction' }}
    | {{ type: 'add-item'; item: AuctionItem }};"#)
            },
            GameCategory::StrategyGame => {
                format!(r#"
// Strategy Game TypeScript Interfaces
export interface StrategyGameState {{
    board: GameBoard;
    players: {{ [playerId: string]: StrategyPlayer }};
    currentTurn: string;
    moveHistory: GameMove[];
    turnNumber: number;
}}

export interface GameBoard {{
    width: number;
    height: number;
    tiles: Tile[][];
}}

export interface Tile {{
    position: [number, number];
    tileType: TileType;
    occupant: string | null;
}}

export enum TileType {{
    Empty = "empty",
    Wall = "wall",
    Resource = "resource",
    Special = "special",
}}

export interface StrategyPlayer {{
    resources: {{ [resourceType: string]: number }};
    units: GameUnit[];
    score: number;
}}

export interface GameUnit {{
    id: string;
    position: [number, number];
    unitType: string;
    health: number;
    canMove: boolean;
}}

export interface GameMove {{
    playerId: string;
    moveType: MoveType;
    timestamp: number;
}}

export type MoveType =
    | {{ type: 'place-unit'; unitType: string; position: [number, number] }}
    | {{ type: 'move-unit'; unitId: string; toPosition: [number, number] }}
    | {{ type: 'use-resource'; resourceType: string; amount: number }}
    | {{ type: 'end-turn' }};"#)
            },
            GameCategory::PuzzleGame => {
                format!(r#"
// Puzzle Game TypeScript Interfaces
export interface PuzzleGameState {{
    puzzle: Puzzle;
    playerSolutions: {{ [playerId: string]: Solution }};
    timeStarted: number;
    timeLimit?: number;
    hintCount: number;
}}

export interface Puzzle {{
    id: string;
    difficulty: Difficulty;
    puzzleType: PuzzleType;
    data: any; // JSON data
    solutionHash: string;
}}

export enum Difficulty {{
    Easy = "easy",
    Medium = "medium",
    Hard = "hard",
    Expert = "expert",
}}

export enum PuzzleType {{
    Mathematical = "mathematical",
    Logical = "logical",
    Spatial = "spatial",
    WordBased = "word-based",
}}

export interface Solution {{
    playerId: string;
    answer: any; // JSON data
    submittedAt: number;
    isCorrect?: boolean;
    score: number;
}}

export type PuzzleGameActions =
    | {{ type: 'submit-solution'; answer: any }}
    | {{ type: 'request-hint' }}
    | {{ type: 'skip' }}
    | {{ type: 'reset' }};"#)
            },
            GameCategory::Custom(category_name) => {
                format!(r#"
// Custom Game TypeScript Interfaces for {}
export interface {}GameState {{
    customData: any; // JSON data
    players: {{ [playerId: string]: CustomPlayer }};
    gamePhase: CustomPhase;
    metadata: {{ [key: string]: string }};
}}

export interface CustomPlayer {{
    id: string;
    score: number;
    data: any; // JSON data
    lastAction?: number;
}}

export enum CustomPhase {{
    Setup = "setup",
    Playing = "playing",
    Scoring = "scoring",
    Finished = "finished",
}}

export interface CustomAction {{
    actionType: string;
    parameters: {{ [key: string]: any }};
    timestamp: number;
}}

export const default{}GameState: {}GameState = {{
    customData: null,
    players: {{}},
    gamePhase: CustomPhase.Setup,
    metadata: {{}},
}};"#, category_name, to_pascal_case(category_name), to_pascal_case(category_name), to_pascal_case(category_name))
            }
        }
    }

    fn generate_python_types(&self, game: &CustomGame) -> String {
        // Generate comprehensive Python class definitions based on game template
        match &game.template.category {
            GameCategory::DiceGame => {
                format!(r#"
# Dice Game Python Classes
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union
import time

@dataclass
class DiceGameState:
    """State for dice-based games like Craps"""
    dice: List[int] = field(default_factory=list)
    phase: 'DicePhase' = DicePhase.COME_OUT
    bets: Dict[str, List['DiceBet']] = field(default_factory=dict)
    pot: int = 0
    round_number: int = 1

class DicePhase(Enum):
    """Different phases of a dice game"""
    COME_OUT = "come-out"
    POINT = "point"
    RESOLVED = "resolved"

@dataclass
class DiceBet:
    """Individual dice bet"""
    player_id: str
    bet_type: 'DiceBetType'
    amount: int
    placed_at: int = field(default_factory=lambda: int(time.time()))

class DiceBetType(Enum):
    """Types of dice bets available"""
    PASS_LINE = "pass-line"
    DONT_PASS_LINE = "dont-pass-line"
    COME = "come"
    DONT_COME = "dont-come"
    FIELD = "field"
    PLACE_4 = "place-4"
    PLACE_5 = "place-5"
    PLACE_6 = "place-6"
    PLACE_8 = "place-8"
    PLACE_9 = "place-9"
    PLACE_10 = "place-10"
    HARD_4 = "hard-4"
    HARD_6 = "hard-6"
    HARD_8 = "hard-8"
    HARD_10 = "hard-10"
    ANY_7 = "any-7"

@dataclass
class DiceRollResult:
    """Result of a dice roll"""
    dice: List[int]
    total: int
    winning_bets: List[str] = field(default_factory=list)
    losing_bets: List[str] = field(default_factory=list)
    payouts: Dict[str, int] = field(default_factory=dict)

class DiceGameActions:
    """Available actions for dice games"""

    @staticmethod
    def roll_dice() -> Dict[str, str]:
        return {{"type": "roll-dice"}}

    @staticmethod
    def place_bet(bet_type: DiceBetType, amount: int) -> Dict[str, Union[str, int]]:
        return {{"type": "place-bet", "bet_type": bet_type.value, "amount": amount}}

    @staticmethod
    def resolve_bets(roll_result: DiceRollResult) -> Dict[str, Union[str, dict]]:
        return {{"type": "resolve-bets", "roll_result": roll_result.__dict__}}"#)
            },
            GameCategory::CardGame => {
                format!(r#"
# Card Game Python Classes
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union
import random

@dataclass
class CardGameState:
    """State for card-based games"""
    deck: List['Card'] = field(default_factory=list)
    players: Dict[str, 'PlayerHand'] = field(default_factory=dict)
    community_cards: List['Card'] = field(default_factory=list)
    current_turn: Optional[str] = None
    pot: int = 0
    betting_round: int = 1

@dataclass
class Card:
    """Playing card"""
    suit: 'Suit'
    rank: 'Rank'

    def __str__(self) -> str:
        return f"{{self.rank.value}} of {{self.suit.value}}"

class Suit(Enum):
    """Card suits"""
    HEARTS = "hearts"
    DIAMONDS = "diamonds"
    CLUBS = "clubs"
    SPADES = "spades"

class Rank(Enum):
    """Card ranks"""
    ACE = "ace"
    TWO = "two"
    THREE = "three"
    FOUR = "four"
    FIVE = "five"
    SIX = "six"
    SEVEN = "seven"
    EIGHT = "eight"
    NINE = "nine"
    TEN = "ten"
    JACK = "jack"
    QUEEN = "queen"
    KING = "king"

@dataclass
class PlayerHand:
    """Player's cards and status"""
    cards: List[Card] = field(default_factory=list)
    total_value: int = 0
    is_folded: bool = False
    current_bet: int = 0

    def add_card(self, card: Card) -> None:
        """Add card to hand"""
        self.cards.append(card)
        self.calculate_value()

    def calculate_value(self) -> None:
        """Calculate hand value (basic implementation)"""
        value = 0
        aces = 0

        for card in self.cards:
            if card.rank in [Rank.JACK, Rank.QUEEN, Rank.KING]:
                value += 10
            elif card.rank == Rank.ACE:
                aces += 1
                value += 11
            else:
                value += int(card.rank.value) if card.rank.value.isdigit() else 1

        # Handle aces
        while value > 21 and aces > 0:
            value -= 10
            aces -= 1

        self.total_value = value

class CardGameActions:
    """Available actions for card games"""

    @staticmethod
    def deal() -> Dict[str, str]:
        return {{"type": "deal"}}

    @staticmethod
    def hit() -> Dict[str, str]:
        return {{"type": "hit"}}

    @staticmethod
    def stand() -> Dict[str, str]:
        return {{"type": "stand"}}

    @staticmethod
    def fold() -> Dict[str, str]:
        return {{"type": "fold"}}

    @staticmethod
    def raise_bet(amount: int) -> Dict[str, Union[str, int]]:
        return {{"type": "raise", "amount": amount}}

    @staticmethod
    def call() -> Dict[str, str]:
        return {{"type": "call"}}

    @staticmethod
    def check() -> Dict[str, str]:
        return {{"type": "check"}}"#)
            },
            GameCategory::AuctionGame => {
                format!(r#"
# Auction Game Python Classes
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union
import time

@dataclass
class AuctionGameState:
    """State for auction-based games"""
    current_item: Optional['AuctionItem'] = None
    highest_bid: Optional['Bid'] = None
    time_remaining: int = 0  # seconds
    bidding_history: List['Bid'] = field(default_factory=list)
    completed_auctions: List['CompletedAuction'] = field(default_factory=list)

@dataclass
class AuctionItem:
    """Item being auctioned"""
    id: str
    name: str
    description: str
    starting_price: int
    reserve_price: Optional[int] = None
    category: str = "general"

@dataclass
class Bid:
    """Auction bid"""
    bidder_id: str
    amount: int
    timestamp: int = field(default_factory=lambda: int(time.time()))
    is_reserve_met: bool = False

    def meets_reserve(self, reserve_price: Optional[int]) -> bool:
        """Check if bid meets reserve price"""
        if reserve_price is None:
            return True
        return self.amount >= reserve_price

@dataclass
class CompletedAuction:
    """Completed auction results"""
    item: AuctionItem
    winning_bid: Optional[Bid]
    total_bids: int
    completed_at: int = field(default_factory=lambda: int(time.time()))

    @property
    def was_successful(self) -> bool:
        """Check if auction had a winner"""
        return self.winning_bid is not None

class AuctionGameActions:
    """Available actions for auction games"""

    @staticmethod
    def place_bid(amount: int) -> Dict[str, Union[str, int]]:
        return {{"type": "place-bid", "amount": amount}}

    @staticmethod
    def pass_auction() -> Dict[str, str]:
        return {{"type": "pass"}}

    @staticmethod
    def end_auction() -> Dict[str, str]:
        return {{"type": "end-auction"}}

    @staticmethod
    def add_item(item: AuctionItem) -> Dict[str, Union[str, dict]]:
        return {{"type": "add-item", "item": item.__dict__}}"#)
            },
            GameCategory::StrategyGame => {
                format!(r#"
# Strategy Game Python Classes
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union, Tuple
import time

@dataclass
class StrategyGameState:
    """State for strategy games"""
    board: 'GameBoard'
    players: Dict[str, 'StrategyPlayer'] = field(default_factory=dict)
    current_turn: str = ""
    move_history: List['GameMove'] = field(default_factory=list)
    turn_number: int = 1

@dataclass
class GameBoard:
    """Game board representation"""
    width: int
    height: int
    tiles: List[List['Tile']] = field(default_factory=list)

    def __post_init__(self):
        """Initialize empty board"""
        if not self.tiles:
            self.tiles = [[Tile((x, y), TileType.EMPTY)
                          for x in range(self.width)]
                         for y in range(self.height)]

    def get_tile(self, x: int, y: int) -> Optional['Tile']:
        """Get tile at position"""
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.tiles[y][x]
        return None

    def set_tile(self, x: int, y: int, tile: 'Tile') -> bool:
        """Set tile at position"""
        if 0 <= x < self.width and 0 <= y < self.height:
            self.tiles[y][x] = tile
            return True
        return False

@dataclass
class Tile:
    """Individual board tile"""
    position: Tuple[int, int]
    tile_type: 'TileType'
    occupant: Optional[str] = None

class TileType(Enum):
    """Types of board tiles"""
    EMPTY = "empty"
    WALL = "wall"
    RESOURCE = "resource"
    SPECIAL = "special"

@dataclass
class StrategyPlayer:
    """Strategy game player"""
    resources: Dict[str, int] = field(default_factory=dict)
    units: List['GameUnit'] = field(default_factory=list)
    score: int = 0

    def add_resource(self, resource_type: str, amount: int) -> None:
        """Add resources to player"""
        self.resources[resource_type] = self.resources.get(resource_type, 0) + amount

    def spend_resource(self, resource_type: str, amount: int) -> bool:
        """Try to spend resources"""
        current = self.resources.get(resource_type, 0)
        if current >= amount:
            self.resources[resource_type] = current - amount
            return True
        return False

@dataclass
class GameUnit:
    """Game unit/piece"""
    id: str
    position: Tuple[int, int]
    unit_type: str
    health: int = 100
    can_move: bool = True

    def move_to(self, new_position: Tuple[int, int]) -> bool:
        """Move unit to new position"""
        if self.can_move and self.health > 0:
            self.position = new_position
            return True
        return False

@dataclass
class GameMove:
    """Player move/action"""
    player_id: str
    move_type: 'MoveType'
    timestamp: int = field(default_factory=lambda: int(time.time()))

class MoveType(Enum):
    """Types of moves"""
    PLACE_UNIT = "place-unit"
    MOVE_UNIT = "move-unit"
    USE_RESOURCE = "use-resource"
    END_TURN = "end-turn"

class StrategyGameActions:
    """Available actions for strategy games"""

    @staticmethod
    def place_unit(unit_type: str, position: Tuple[int, int]) -> Dict[str, Union[str, list]]:
        return {{"type": "place-unit", "unit_type": unit_type, "position": list(position)}}

    @staticmethod
    def move_unit(unit_id: str, to_position: Tuple[int, int]) -> Dict[str, Union[str, list]]:
        return {{"type": "move-unit", "unit_id": unit_id, "to_position": list(to_position)}}

    @staticmethod
    def use_resource(resource_type: str, amount: int) -> Dict[str, Union[str, int]]:
        return {{"type": "use-resource", "resource_type": resource_type, "amount": amount}}

    @staticmethod
    def end_turn() -> Dict[str, str]:
        return {{"type": "end-turn"}}"#)
            },
            GameCategory::PuzzleGame => {
                format!(r#"
# Puzzle Game Python Classes
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union, Any
import time
import hashlib
import json

@dataclass
class PuzzleGameState:
    """State for puzzle games"""
    puzzle: 'Puzzle'
    player_solutions: Dict[str, 'Solution'] = field(default_factory=dict)
    time_started: int = field(default_factory=lambda: int(time.time()))
    time_limit: Optional[int] = None
    hint_count: int = 0

@dataclass
class Puzzle:
    """Puzzle definition"""
    id: str
    difficulty: 'Difficulty'
    puzzle_type: 'PuzzleType'
    data: Any  # JSON-serializable data
    solution_hash: str

    @classmethod
    def create(cls, id: str, difficulty: 'Difficulty', puzzle_type: 'PuzzleType',
               data: Any, solution: Any) -> 'Puzzle':
        """Create puzzle with solution hash"""
        solution_str = json.dumps(solution, sort_keys=True)
        solution_hash = hashlib.sha256(solution_str.encode()).hexdigest()
        return cls(id, difficulty, puzzle_type, data, solution_hash)

    def check_solution(self, answer: Any) -> bool:
        """Check if answer is correct"""
        answer_str = json.dumps(answer, sort_keys=True)
        answer_hash = hashlib.sha256(answer_str.encode()).hexdigest()
        return answer_hash == self.solution_hash

class Difficulty(Enum):
    """Puzzle difficulty levels"""
    EASY = "easy"
    MEDIUM = "medium"
    HARD = "hard"
    EXPERT = "expert"

class PuzzleType(Enum):
    """Types of puzzles"""
    MATHEMATICAL = "mathematical"
    LOGICAL = "logical"
    SPATIAL = "spatial"
    WORD_BASED = "word-based"

@dataclass
class Solution:
    """Player solution submission"""
    player_id: str
    answer: Any  # JSON-serializable answer
    submitted_at: int = field(default_factory=lambda: int(time.time()))
    is_correct: Optional[bool] = None
    score: int = 0

    def calculate_score(self, puzzle: Puzzle, time_taken: int) -> int:
        """Calculate score based on correctness and time"""
        if not self.is_correct:
            return 0

        base_score = {{
            Difficulty.EASY: 100,
            Difficulty.MEDIUM: 200,
            Difficulty.HARD: 400,
            Difficulty.EXPERT: 800,
        }}[puzzle.difficulty]

        # Time bonus (faster = higher score)
        time_bonus = max(0, 300 - time_taken)  # 5 minute max bonus

        return base_score + time_bonus

class PuzzleGameActions:
    """Available actions for puzzle games"""

    @staticmethod
    def submit_solution(answer: Any) -> Dict[str, Any]:
        return {{"type": "submit-solution", "answer": answer}}

    @staticmethod
    def request_hint() -> Dict[str, str]:
        return {{"type": "request-hint"}}

    @staticmethod
    def skip_puzzle() -> Dict[str, str]:
        return {{"type": "skip"}}

    @staticmethod
    def reset_puzzle() -> Dict[str, str]:
        return {{"type": "reset"}}"#)
            },
            GameCategory::Custom(category_name) => {
                format!(r#"
# Custom Game Python Classes for {}
from enum import Enum
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Union, Any
import time

@dataclass
class {}GameState:
    """State for custom {} game"""
    custom_data: Any = None  # JSON-serializable data
    players: Dict[str, 'CustomPlayer'] = field(default_factory=dict)
    game_phase: 'CustomPhase' = CustomPhase.SETUP
    metadata: Dict[str, str] = field(default_factory=dict)

@dataclass
class CustomPlayer:
    """Custom game player"""
    id: str
    score: int = 0
    data: Any = None  # JSON-serializable data
    last_action: Optional[int] = None

    def update_last_action(self) -> None:
        """Update last action timestamp"""
        self.last_action = int(time.time())

class CustomPhase(Enum):
    """Custom game phases"""
    SETUP = "setup"
    PLAYING = "playing"
    SCORING = "scoring"
    FINISHED = "finished"

@dataclass
class CustomAction:
    """Custom game action"""
    action_type: str
    parameters: Dict[str, Any] = field(default_factory=dict)
    timestamp: int = field(default_factory=lambda: int(time.time()))

class {}GameManager:
    """Manager for {} game logic"""

    def __init__(self):
        self.state = {}GameState()

    def add_player(self, player_id: str) -> bool:
        """Add player to game"""
        if player_id not in self.state.players:
            self.state.players[player_id] = CustomPlayer(player_id)
            return True
        return False

    def remove_player(self, player_id: str) -> bool:
        """Remove player from game"""
        if player_id in self.state.players:
            del self.state.players[player_id]
            return True
        return False

    def process_action(self, player_id: str, action: CustomAction) -> bool:
        """Process custom action"""
        if player_id in self.state.players:
            player = self.state.players[player_id]
            player.update_last_action()

            # Custom action processing logic would go here
            return True
        return False

    def advance_phase(self) -> None:
        """Advance to next game phase"""
        phases = [CustomPhase.SETUP, CustomPhase.PLAYING, CustomPhase.SCORING, CustomPhase.FINISHED]
        current_index = phases.index(self.state.game_phase)
        if current_index < len(phases) - 1:
            self.state.game_phase = phases[current_index + 1]

# Default game state factory
def create_default_{}_game_state() -> {}GameState:
    """Create default game state"""
    return {}GameState(
        custom_data=None,
        players={{}},
        game_phase=CustomPhase.SETUP,
        metadata={{}}
    )"#,
                category_name,
                to_pascal_case(category_name), category_name,
                to_pascal_case(category_name), category_name,
                to_pascal_case(category_name),
                to_snake_case(category_name), to_pascal_case(category_name),
                to_pascal_case(category_name))
            }
        }
    }

    fn generate_documentation(&self, game: &CustomGame) -> Result<String, GameDevError> {
        let doc = format!(r#"
# {} Game Documentation

## Overview
{}

## Game Rules
- Minimum Players: {}
- Maximum Players: {}
- House Edge: {:.2}%

## Configuration
{}

## State Schema
{}

## Required Actions
{}

## Implementation Guide

### Getting Started
1. Install the BitCraps SDK
2. Import the game engine
3. Implement required methods
4. Test your implementation

### Example Usage
```rust
// Rust example
let engine = {}GameEngine::new();
```

```typescript
// TypeScript example
const engine = new {}GameEngine();
```

```python
# Python example
engine = {}GameEngine()
```

## Testing
Use the BitCraps testing framework to validate your game implementation.

## Deployment
Package your game using the SDK export functionality.
"#,
            game.name,
            game.template.description,
            game.template.min_players,
            game.template.max_players,
            game.config.house_edge,
            serde_json::to_string_pretty(&game.config).unwrap_or_default(),
            serde_json::to_string_pretty(&game.state_schema).unwrap_or_default(),
            game.template.required_actions.join(", "),
            to_pascal_case(&game.name),
            to_pascal_case(&game.name),
            to_pascal_case(&game.name),
        );

        Ok(doc)
    }

    async fn write_game_package(&self, _package: &GamePackage, _output_path: &Path) -> Result<(), GameDevError> {
        // Write game package to filesystem
        // This would create the directory structure and files
        Ok(())
    }
}

/// Game validator for checking game implementations
pub struct GameValidator;

impl GameValidator {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate(&self, game: &CustomGame) -> Result<ValidationReport, GameDevError> {
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        };

        // Validate basic game properties
        if game.name.is_empty() {
            report.errors.push("Game name cannot be empty".to_string());
            report.is_valid = false;
        }

        if game.template.min_players > game.template.max_players {
            report.errors.push("Minimum players cannot be greater than maximum players".to_string());
            report.is_valid = false;
        }

        if game.config.house_edge < 0.0 || game.config.house_edge > 50.0 {
            report.warnings.push("House edge should typically be between 0% and 50%".to_string());
        }

        // Validate state schema
        if game.state_schema.fields.is_empty() {
            report.warnings.push("Game state schema has no fields defined".to_string());
        }

        // Validate required actions
        if game.template.required_actions.is_empty() {
            report.warnings.push("No required actions defined for game".to_string());
        }

        // Add suggestions
        if game.config.house_edge > 5.0 {
            report.suggestions.push("Consider lowering house edge for better player retention".to_string());
        }

        if game.template.max_players > 10 {
            report.suggestions.push("Large player counts may impact performance".to_string());
        }

        Ok(report)
    }
}

// Supporting types and structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTemplate {
    pub name: String,
    pub description: String,
    pub category: GameCategory,
    pub min_players: usize,
    pub max_players: usize,
    pub default_rules: GameRules,
    pub state_schema: GameStateSchema,
    pub required_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameCategory {
    CardGame,
    DiceGame,
    AuctionGame,
    StrategyGame,
    PuzzleGame,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRules {
    pub deck_size: Option<usize>,
    pub hand_size: Option<usize>,
    pub turn_time_limit: Option<u64>,
    pub betting_rounds: Option<usize>,
    pub custom_rules: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSchema {
    pub fields: Vec<StateField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateField {
    pub name: String,
    pub field_type: String,
}

#[derive(Debug, Clone)]
pub struct CustomGame {
    pub id: String,
    pub name: String,
    pub template: GameTemplate,
    pub config: GameConfig,
    pub rules: GameRules,
    pub state_schema: GameStateSchema,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeLimits {
    pub action_timeout_seconds: u64,
    pub game_timeout_minutes: u64,
    pub idle_timeout_minutes: u64,
}

impl Default for TimeLimits {
    fn default() -> Self {
        Self {
            action_timeout_seconds: 30,
            game_timeout_minutes: 60,
            idle_timeout_minutes: 5,
        }
    }
}

#[derive(Debug)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug)]
pub enum ProgrammingLanguage {
    Rust,
    TypeScript,
    Python,
}

struct GamePackage {
    metadata: GameMetadata,
    engine: Box<dyn GameEngine>,
    assets: Vec<GameAsset>,
    documentation: String,
}

struct GameMetadata {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
}

struct GameAsset {
    name: String,
    content: Vec<u8>,
    asset_type: AssetType,
}

enum AssetType {
    Image,
    Audio,
    Data,
    Other,
}

// Custom game engine implementation
struct CustomGameEngine {
    game: CustomGame,
}

impl CustomGameEngine {
    fn from_game(game: CustomGame) -> Self {
        Self { game }
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
        // Return comprehensive bet types based on game template
        match self.game.template.category {
            GameCategory::DiceGame => vec![
                "PassLine".to_string(),
                "DontPassLine".to_string(),
                "Come".to_string(),
                "DontCome".to_string(),
                "Field".to_string(),
                "Place4".to_string(),
                "Place5".to_string(),
                "Place6".to_string(),
                "Place8".to_string(),
                "Place9".to_string(),
                "Place10".to_string(),
                "Any7".to_string(),
                "Hard4".to_string(),
                "Hard6".to_string(),
                "Hard8".to_string(),
                "Hard10".to_string(),
            ],
            GameCategory::CardGame => vec![
                "Ante".to_string(),
                "Raise".to_string(),
                "Side".to_string(),
                "Insurance".to_string(),
            ],
            GameCategory::AuctionGame => vec![
                "Bid".to_string(),
                "ReserveBid".to_string(),
            ],
            GameCategory::StrategyGame => vec![
                "Move".to_string(),
                "Strategy".to_string(),
            ],
            GameCategory::PuzzleGame => vec![
                "Solution".to_string(),
            ],
            GameCategory::Custom(_) => {
                // Extract from game configuration
                self.game.config.payout_multipliers.keys().cloned().collect()
            },
        }
    }

    fn get_house_edge(&self) -> f64 {
        self.game.config.house_edge
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        // Validate game configuration
        if self.game.name.is_empty() {
            return Err(GameEngineError::InvalidConfiguration("Game name cannot be empty".to_string()));
        }

        if self.game.template.min_players > self.game.template.max_players {
            return Err(GameEngineError::InvalidConfiguration("Min players cannot exceed max players".to_string()));
        }

        if self.game.config.house_edge < 0.0 || self.game.config.house_edge > 50.0 {
            return Err(GameEngineError::InvalidConfiguration("House edge must be between 0% and 50%".to_string()));
        }

        // Validate betting limits
        if self.game.config.betting_limits.min_bet > self.game.config.betting_limits.max_bet {
            return Err(GameEngineError::InvalidConfiguration("Min bet cannot exceed max bet".to_string()));
        }

        // Validate payout multipliers
        for (bet_type, multiplier) in &self.game.config.payout_multipliers {
            if *multiplier <= 0.0 {
                return Err(GameEngineError::InvalidConfiguration(
                    format!("Payout multiplier for {} must be positive", bet_type)
                ));
            }
        }

        Ok(())
    }

    async fn validate_session_config(&self, config: &crate::gaming::GameSessionConfig) -> Result<(), GameEngineError> {
        // Validate session betting limits
        if config.min_bet < self.game.config.betting_limits.min_bet {
            return Err(GameEngineError::InvalidConfiguration(
                format!("Session min bet {} is below game minimum {}",
                    config.min_bet, self.game.config.betting_limits.min_bet)
            ));
        }

        if config.max_bet > self.game.config.betting_limits.max_bet {
            return Err(GameEngineError::InvalidConfiguration(
                format!("Session max bet {} exceeds game maximum {}",
                    config.max_bet, self.game.config.betting_limits.max_bet)
            ));
        }

        // Validate game-specific configuration
        for (key, value) in &config.game_specific_config {
            match self.game.template.category {
                GameCategory::DiceGame => {
                    if key == "dice_count" {
                        if let Ok(count) = value.parse::<u8>() {
                            if count < 1 || count > 10 {
                                return Err(GameEngineError::InvalidConfiguration(
                                    "Dice count must be between 1 and 10".to_string()
                                ));
                            }
                        }
                    }
                },
                GameCategory::CardGame => {
                    if key == "deck_size" {
                        if let Ok(size) = value.parse::<u8>() {
                            if size < 20 || size > 104 { // 20 minimum, 2 decks maximum
                                return Err(GameEngineError::InvalidConfiguration(
                                    "Deck size must be between 20 and 104 cards".to_string()
                                ));
                            }
                        }
                    }
                },
                _ => {}
            }
        }

        Ok(())
    }

    async fn initialize_session(&self, session: &GameSession) -> Result<(), crate::gaming::GameFrameworkError> {
        // Initialize game state based on template
        match self.game.template.category {
            GameCategory::DiceGame => {
                // Initialize dice game state
                // Set up initial phase, dice, and betting areas
                tracing::info!("Initialized dice game session: {}", session.id);
            },
            GameCategory::CardGame => {
                // Initialize card game state
                // Shuffle deck, deal initial cards if needed
                tracing::info!("Initialized card game session: {}", session.id);
            },
            GameCategory::AuctionGame => {
                // Initialize auction state
                // Set up auction items and bidding parameters
                tracing::info!("Initialized auction game session: {}", session.id);
            },
            _ => {
                tracing::info!("Initialized generic game session: {}", session.id);
            }
        }

        // Log initialization metrics
        tracing::debug!("Session {} initialized with {} players",
            session.id, session.players.len());

        Ok(())
    }

    async fn validate_player_join(&self, session: &GameSession, player_id: &str, join_data: &PlayerJoinData) -> Result<(), crate::gaming::GameFrameworkError> {
        // Check if session is full
        if session.players.len() >= self.game.template.max_players {
            return Err(crate::gaming::GameFrameworkError::SessionFull);
        }

        // Check if player already in session
        if session.players.contains_key(player_id) {
            return Err(crate::gaming::GameFrameworkError::PlayerAlreadyInSession);
        }

        // Validate initial balance
        let min_balance = (self.game.config.betting_limits.min_bet * 10) as u64; // 10x min bet
        if join_data.initial_balance < min_balance {
            return Err(crate::gaming::GameFrameworkError::InsufficientBalance {
                required: min_balance,
                available: join_data.initial_balance,
            });
        }

        // Validate game-specific join requirements
        match self.game.template.category {
            GameCategory::DiceGame => {
                // No additional requirements for dice games
            },
            GameCategory::CardGame => {
                // Might require specific player experience level
                if let Some(experience) = join_data.game_specific_data.get("experience_level") {
                    if let Ok(level) = experience.parse::<u32>() {
                        if level < 1 {
                            return Err(crate::gaming::GameFrameworkError::InvalidJoinData(
                                "Card games require experience level >= 1".to_string()
                            ));
                        }
                    }
                }
            },
            GameCategory::AuctionGame => {
                // Might require higher minimum balance for auctions
                let auction_min = min_balance * 5; // 5x higher for auctions
                if join_data.initial_balance < auction_min {
                    return Err(crate::gaming::GameFrameworkError::InsufficientBalance {
                        required: auction_min,
                        available: join_data.initial_balance,
                    });
                }
            },
            _ => {}
        }

        Ok(())
    }

    async fn on_player_joined(&self, session: &GameSession, player_id: &str) -> Result<(), crate::gaming::GameFrameworkError> {
        tracing::info!("Player {} joined session {} (game: {})",
            player_id, session.id, self.game.name);

        // Check if we can auto-start the session
        if session.players.len() >= self.game.template.min_players {
            if let Some(config) = session.config.as_ref() {
                if config.auto_start {
                    tracing::info!("Auto-starting session {} with {} players",
                        session.id, session.players.len());
                    // In a real implementation, this would trigger session start
                }
            }
        }

        // Send welcome message or game rules to new player
        tracing::debug!("Sending game rules to player {}", player_id);

        Ok(())
    }

    async fn process_action(&self, session: &GameSession, player_id: &str, action: GameAction) -> Result<GameActionResult, crate::gaming::GameFrameworkError> {
        // Validate player is in session
        if !session.players.contains_key(player_id) {
            return Err(crate::gaming::GameFrameworkError::PlayerNotInSession);
        }

        // Process actions based on game type and current action
        match (&self.game.template.category, &action) {
            (GameCategory::DiceGame, GameAction::PlaceBet { bet_type, amount }) => {
                // Validate bet type is supported
                let supported_types = self.get_supported_bet_types();
                if !supported_types.contains(bet_type) {
                    return Err(crate::gaming::GameFrameworkError::UnsupportedAction(
                        format!("Bet type {} not supported", bet_type)
                    ));
                }

                // Validate bet amount
                if *amount < self.game.config.betting_limits.min_bet as i64 ||
                   *amount > self.game.config.betting_limits.max_bet as i64 {
                    return Err(crate::gaming::GameFrameworkError::InvalidBetAmount);
                }

                Ok(GameActionResult::BetPlaced {
                    bet_id: uuid::Uuid::new_v4().to_string(),
                    amount: *amount,
                })
            },
            (GameCategory::DiceGame, GameAction::RollDice) => {
                // Simulate dice roll (in real implementation, this would use secure randomness)
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                player_id.hash(&mut hasher);
                session.id.hash(&mut hasher);
                let hash = hasher.finish();

                let die1 = (hash % 6 + 1) as u8;
                let die2 = ((hash >> 8) % 6 + 1) as u8;

                Ok(GameActionResult::DiceRolled {
                    dice: vec![die1, die2],
                    total: die1 + die2,
                })
            },
            (GameCategory::CardGame, GameAction::Hit) => {
                // Deal a card (simplified implementation)
                Ok(GameActionResult::CardDealt {
                    card: "Ace of Spades".to_string(), // Placeholder
                })
            },
            (GameCategory::CardGame, GameAction::Stand) => {
                Ok(GameActionResult::PlayerStands)
            },
            (GameCategory::AuctionGame, GameAction::PlaceBet { amount, .. }) => {
                // Treat as auction bid
                Ok(GameActionResult::BetPlaced {
                    bet_id: uuid::Uuid::new_v4().to_string(),
                    amount: *amount,
                })
            },
            _ => {
                Err(crate::gaming::GameFrameworkError::UnsupportedAction(
                    format!("Action {:?} not supported for game type {:?}",
                        action, self.game.template.category)
                ))
            }
        }
    }

    async fn on_session_ended(&self, session: &GameSession, reason: &SessionEndReason) -> Result<(), crate::gaming::GameFrameworkError> {
        tracing::info!("Session {} ended: {:?} (game: {})",
            session.id, reason, self.game.name);

        // Log final statistics
        let total_bets = session.players.values()
            .map(|player| player.total_bets_placed)
            .sum::<u64>();
        let total_payouts = session.players.values()
            .map(|player| player.total_winnings)
            .sum::<u64>();

        tracing::info!("Session {} statistics: {} players, {} total bets, {} total payouts",
            session.id, session.players.len(), total_bets, total_payouts);

        // Calculate house edge performance
        if total_bets > 0 {
            let actual_house_take = if total_bets > total_payouts {
                total_bets - total_payouts
            } else {
                0
            };
            let actual_house_edge = (actual_house_take as f64 / total_bets as f64) * 100.0;

            tracing::info!("Actual house edge for session {}: {:.2}% (expected: {:.2}%)",
                session.id, actual_house_edge, self.game.config.house_edge);
        }

        // Clean up any game-specific resources
        match self.game.template.category {
            GameCategory::DiceGame => {
                // Clean up dice game state
                tracing::debug!("Cleaned up dice game resources for session {}", session.id);
            },
            GameCategory::CardGame => {
                // Return cards to deck, clean up game state
                tracing::debug!("Cleaned up card game resources for session {}", session.id);
            },
            GameCategory::AuctionGame => {
                // Finalize auction results, transfer items
                tracing::debug!("Cleaned up auction resources for session {}", session.id);
            },
            _ => {}
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum GameDevError {
    TemplateNotFound(String),
    ValidationFailed(String),
    CodeGenerationFailed(String),
    ExportFailed(String),
    InvalidConfiguration(String),
}

// Utility functions
fn to_pascal_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars: Vec<char> = word.chars().collect();
            if !chars.is_empty() {
                chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
            }
            chars.into_iter().collect()
        })
        .collect::<Vec<String>>()
        .join("")
}

fn to_snake_case(s: &str) -> String {
    s.to_lowercase().replace(' ', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_dev_kit_creation() {
        let kit = GameDevKit::new();
        let templates = kit.get_templates();
        assert!(templates.len() >= 3); // Should have at least 3 built-in templates
    }

    #[test]
    fn test_game_creation_from_template() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();

        let game = kit.create_game_from_template("card_game", "My Card Game", config).unwrap();
        assert_eq!(game.name, "My Card Game");
        assert_eq!(game.template.name, "card_game");
    }

    #[tokio::test]
    async fn test_game_validation() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        let game = kit.create_game_from_template("dice_game", "Test Dice Game", config).unwrap();

        let report = kit.validate_game(&game).await.unwrap();
        assert!(report.is_valid);
    }

    #[test]
    fn test_code_generation() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        let game = kit.create_game_from_template("card_game", "Test Game", config).unwrap();

        let rust_code = kit.generate_boilerplate(&game, ProgrammingLanguage::Rust).unwrap();
        assert!(rust_code.contains("TestGameEngine"));

        let ts_code = kit.generate_boilerplate(&game, ProgrammingLanguage::TypeScript).unwrap();
        assert!(ts_code.contains("TestGameEngine"));

        let py_code = kit.generate_boilerplate(&game, ProgrammingLanguage::Python).unwrap();
        assert!(py_code.contains("TestGameEngine"));
    }

    #[test]
    fn test_utility_functions() {
        assert_eq!(to_pascal_case("hello world"), "HelloWorld");
        assert_eq!(to_pascal_case("test game"), "TestGame");

        assert_eq!(to_snake_case("Hello World"), "hello_world");
        assert_eq!(to_snake_case("Test Game"), "test_game");
    }
}