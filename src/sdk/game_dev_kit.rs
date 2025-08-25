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
        vec![]  // TODO: Implement supported bet types
    }}

    fn get_house_edge(&self) -> f64 {{
        {:.2}  // TODO: Calculate actual house edge
    }}

    async fn is_available(&self) -> bool {{
        true
    }}

    async fn validate(&self) -> Result<(), GameEngineError> {{
        // TODO: Implement validation logic
        Ok(())
    }}

    async fn validate_session_config(&self, _config: &crate::gaming::GameSessionConfig) -> Result<(), GameEngineError> {{
        // TODO: Implement session config validation
        Ok(())
    }}

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), crate::gaming::GameFrameworkError> {{
        // TODO: Initialize game session
        Ok(())
    }}

    async fn validate_player_join(&self, _session: &GameSession, _player_id: &str, _join_data: &PlayerJoinData) -> Result<(), crate::gaming::GameFrameworkError> {{
        // TODO: Validate player join
        Ok(())
    }}

    async fn on_player_joined(&self, _session: &GameSession, _player_id: &str) -> Result<(), crate::gaming::GameFrameworkError> {{
        // TODO: Handle player joined event
        Ok(())
    }}

    async fn process_action(&self, _session: &GameSession, _player_id: &str, action: GameAction) -> Result<GameActionResult, crate::gaming::GameFrameworkError> {{
        // TODO: Implement action processing
        match action {{
            // Handle different action types
            _ => Err(crate::gaming::GameFrameworkError::UnsupportedAction("Action not implemented".to_string())),
        }}
    }}

    async fn on_session_ended(&self, _session: &GameSession, _reason: &SessionEndReason) -> Result<(), crate::gaming::GameFrameworkError> {{
        // TODO: Handle session ended event
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
        return [];  // TODO: Implement supported bet types
    }}

    getHouseEdge(): number {{
        return {};  // TODO: Calculate actual house edge
    }}

    async isAvailable(): Promise<boolean> {{
        return true;
    }}

    async validate(): Promise<void> {{
        // TODO: Implement validation logic
    }}

    async validateSessionConfig(config: any): Promise<void> {{
        // TODO: Implement session config validation
    }}

    async initializeSession(session: GameSession): Promise<void> {{
        // TODO: Initialize game session
    }}

    async validatePlayerJoin(session: GameSession, playerId: string, joinData: PlayerJoinData): Promise<void> {{
        // TODO: Validate player join
    }}

    async onPlayerJoined(session: GameSession, playerId: string): Promise<void> {{
        // TODO: Handle player joined event
    }}

    async processAction(session: GameSession, playerId: string, action: GameAction): Promise<GameActionResult> {{
        // TODO: Implement action processing
        throw new Error('Action not implemented');
    }}

    async onSessionEnded(session: GameSession, reason: SessionEndReason): Promise<void> {{
        // TODO: Handle session ended event
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
        return []  # TODO: Implement supported bet types
    
    def get_house_edge(self) -> float:
        return {}  # TODO: Calculate actual house edge
    
    async def is_available(self) -> bool:
        return True
    
    async def validate(self) -> None:
        """Validate game engine configuration"""
        # TODO: Implement validation logic
        pass
    
    async def validate_session_config(self, config: Dict[str, Any]) -> None:
        """Validate session configuration"""
        # TODO: Implement session config validation
        pass
    
    async def initialize_session(self, session: GameSession) -> None:
        """Initialize game session"""
        # TODO: Initialize game session
        pass
    
    async def validate_player_join(self, session: GameSession, player_id: str, join_data: PlayerJoinData) -> None:
        """Validate player join request"""
        # TODO: Validate player join
        pass
    
    async def on_player_joined(self, session: GameSession, player_id: str) -> None:
        """Handle player joined event"""
        # TODO: Handle player joined event
        pass
    
    async def process_action(self, session: GameSession, player_id: str, action: GameAction) -> GameActionResult:
        """Process game action"""
        # TODO: Implement action processing
        raise NotImplementedError("Action processing not implemented")
    
    async def on_session_ended(self, session: GameSession, reason: SessionEndReason) -> None:
        """Handle session ended event"""
        # TODO: Handle session ended event
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

    fn generate_game_types(&self, _game: &CustomGame) -> String {
        // Generate Rust type definitions
        "// TODO: Add game-specific types here".to_string()
    }

    fn generate_typescript_types(&self, _game: &CustomGame) -> String {
        // Generate TypeScript interface definitions
        "// TODO: Add game-specific interfaces here".to_string()
    }

    fn generate_python_types(&self, _game: &CustomGame) -> String {
        // Generate Python class definitions
        "# TODO: Add game-specific classes here".to_string()
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
        // Return bet types based on game configuration
        vec!["main".to_string()] // Default
    }

    fn get_house_edge(&self) -> f64 {
        self.game.config.house_edge
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn validate(&self) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn validate_session_config(&self, _config: &crate::gaming::GameSessionConfig) -> Result<(), GameEngineError> {
        Ok(())
    }

    async fn initialize_session(&self, _session: &GameSession) -> Result<(), crate::gaming::GameFrameworkError> {
        Ok(())
    }

    async fn validate_player_join(&self, _session: &GameSession, _player_id: &str, _join_data: &PlayerJoinData) -> Result<(), crate::gaming::GameFrameworkError> {
        Ok(())
    }

    async fn on_player_joined(&self, _session: &GameSession, _player_id: &str) -> Result<(), crate::gaming::GameFrameworkError> {
        Ok(())
    }

    async fn process_action(&self, _session: &GameSession, _player_id: &str, _action: GameAction) -> Result<GameActionResult, crate::gaming::GameFrameworkError> {
        // Default implementation - should be customized
        Err(crate::gaming::GameFrameworkError::UnsupportedAction("Custom action processing not implemented".to_string()))
    }

    async fn on_session_ended(&self, _session: &GameSession, _reason: &SessionEndReason) -> Result<(), crate::gaming::GameFrameworkError> {
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