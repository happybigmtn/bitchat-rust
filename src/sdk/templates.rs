//! Game Templates for BitCraps SDK
//!
//! This module provides pre-built game templates that developers can use
//! as starting points for creating custom games.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Game template containing configuration and scaffolding
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

/// Game category classifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameCategory {
    CardGame,
    DiceGame,
    AuctionGame,
    StrategyGame,
    PuzzleGame,
}

/// Game rules configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRules {
    pub deck_size: Option<usize>,
    pub hand_size: Option<usize>,
    pub turn_time_limit: Option<u64>,
    pub betting_rounds: Option<usize>,
    pub custom_rules: HashMap<String, serde_json::Value>,
}

/// Schema definition for game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateSchema {
    pub fields: Vec<StateField>,
}

/// Individual field in the game state schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateField {
    pub name: String,
    pub field_type: String,
}

/// Template manager for loading and managing game templates
pub struct TemplateManager {
    templates: HashMap<String, GameTemplate>,
}

impl TemplateManager {
    /// Create new template manager with built-in templates
    pub fn new() -> Self {
        let mut manager = Self {
            templates: HashMap::new(),
        };
        manager.load_builtin_templates();
        manager
    }

    /// Get all available templates
    pub fn get_templates(&self) -> Vec<&GameTemplate> {
        self.templates.values().collect()
    }

    /// Get specific template by name
    pub fn get_template(&self, name: &str) -> Option<&GameTemplate> {
        self.templates.get(name)
    }

    /// Add custom template
    pub fn add_template(&mut self, template: GameTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Remove template
    pub fn remove_template(&mut self, name: &str) -> Option<GameTemplate> {
        self.templates.remove(name)
    }

    /// Load built-in game templates
    pub fn load_builtin_templates(&mut self) {
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

        // Strategy game template
        let strategy_game_template = GameTemplate {
            name: "strategy_game".to_string(),
            description: "Turn-based strategy game with board mechanics".to_string(),
            category: GameCategory::StrategyGame,
            min_players: 2,
            max_players: 4,
            default_rules: GameRules {
                deck_size: None,
                hand_size: None,
                turn_time_limit: Some(120),
                betting_rounds: None,
                custom_rules: {
                    let mut rules = HashMap::new();
                    rules.insert("board_width".to_string(), serde_json::Value::Number(8.into()));
                    rules.insert("board_height".to_string(), serde_json::Value::Number(8.into()));
                    rules.insert("units_per_player".to_string(), serde_json::Value::Number(6.into()));
                    rules
                },
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "board".to_string(), field_type: "GameBoard".to_string() },
                    StateField { name: "current_player".to_string(), field_type: "String".to_string() },
                    StateField { name: "turn_count".to_string(), field_type: "u32".to_string() },
                ],
            },
            required_actions: vec![
                "move_unit".to_string(),
                "attack".to_string(),
                "end_turn".to_string(),
            ],
        };

        // Puzzle game template
        let puzzle_game_template = GameTemplate {
            name: "puzzle_game".to_string(),
            description: "Logic puzzle game with configurable difficulty".to_string(),
            category: GameCategory::PuzzleGame,
            min_players: 1,
            max_players: 1,
            default_rules: GameRules {
                deck_size: None,
                hand_size: None,
                turn_time_limit: Some(300),
                betting_rounds: None,
                custom_rules: {
                    let mut rules = HashMap::new();
                    rules.insert("difficulty".to_string(), serde_json::Value::String("medium".to_string()));
                    rules.insert("max_hints".to_string(), serde_json::Value::Number(3.into()));
                    rules
                },
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "puzzle".to_string(), field_type: "Puzzle".to_string() },
                    StateField { name: "solution".to_string(), field_type: "Solution".to_string() },
                    StateField { name: "hints_used".to_string(), field_type: "u32".to_string() },
                ],
            },
            required_actions: vec![
                "submit_solution".to_string(),
                "request_hint".to_string(),
                "reset_puzzle".to_string(),
            ],
        };

        // Insert all templates
        self.templates.insert(card_game_template.name.clone(), card_game_template);
        self.templates.insert(dice_game_template.name.clone(), dice_game_template);
        self.templates.insert(auction_game_template.name.clone(), auction_game_template);
        self.templates.insert(strategy_game_template.name.clone(), strategy_game_template);
        self.templates.insert(puzzle_game_template.name.clone(), puzzle_game_template);
    }
}

impl Default for TemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_manager_creation() {
        let manager = TemplateManager::new();
        let templates = manager.get_templates();
        
        // Should have built-in templates
        assert!(templates.len() >= 5);
        
        // Check specific templates exist
        assert!(manager.get_template("card_game").is_some());
        assert!(manager.get_template("dice_game").is_some());
        assert!(manager.get_template("auction_game").is_some());
    }

    #[test]
    fn test_custom_template_management() {
        let mut manager = TemplateManager::new();
        
        let custom_template = GameTemplate {
            name: "test_game".to_string(),
            description: "Test game template".to_string(),
            category: GameCategory::CardGame,
            min_players: 1,
            max_players: 2,
            default_rules: GameRules {
                deck_size: Some(10),
                hand_size: Some(3),
                turn_time_limit: Some(60),
                betting_rounds: Some(1),
                custom_rules: HashMap::new(),
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "test_field".to_string(), field_type: "String".to_string() },
                ],
            },
            required_actions: vec!["test_action".to_string()],
        };

        // Add custom template
        manager.add_template(custom_template.clone());
        assert!(manager.get_template("test_game").is_some());

        // Remove template
        let removed = manager.remove_template("test_game");
        assert!(removed.is_some());
        assert!(manager.get_template("test_game").is_none());
    }
}