//! Game Builder Module for BitCraps SDK
//!
//! This module provides builder patterns for creating custom games
//! with a fluent, easy-to-use API.

use std::collections::HashMap;
use crate::sdk::game_types::*;
use crate::sdk::templates::*;
use crate::sdk::validation::{GameValidator, GameDevError, ValidationReport};
use crate::sdk::codegen::CodeGenerator;

/// Builder for creating custom games with a fluent API
pub struct GameBuilder {
    name: Option<String>,
    description: Option<String>,
    category: Option<GameCategory>,
    min_players: Option<usize>,
    max_players: Option<usize>,
    house_edge: Option<f64>,
    betting_limits: Option<BettingLimits>,
    time_limits: Option<TimeLimits>,
    payout_multipliers: HashMap<String, f64>,
    state_fields: Vec<StateField>,
    required_actions: Vec<String>,
    custom_rules: HashMap<String, serde_json::Value>,
}

impl GameBuilder {
    /// Create a new game builder
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            category: None,
            min_players: None,
            max_players: None,
            house_edge: None,
            betting_limits: None,
            time_limits: None,
            payout_multipliers: HashMap::new(),
            state_fields: Vec::new(),
            required_actions: Vec::new(),
            custom_rules: HashMap::new(),
        }
    }

    /// Start from an existing template
    pub fn from_template(template: &GameTemplate) -> Self {
        Self {
            name: None,
            description: Some(template.description.clone()),
            category: Some(template.category.clone()),
            min_players: Some(template.min_players),
            max_players: Some(template.max_players),
            house_edge: None,
            betting_limits: None,
            time_limits: None,
            payout_multipliers: HashMap::new(),
            state_fields: template.state_schema.fields.clone(),
            required_actions: template.required_actions.clone(),
            custom_rules: template.default_rules.custom_rules.clone(),
        }
    }

    /// Set the game name
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the game description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the game category
    pub fn category(mut self, category: GameCategory) -> Self {
        self.category = Some(category);
        self
    }

    /// Set the minimum number of players
    pub fn min_players(mut self, min: usize) -> Self {
        self.min_players = Some(min);
        self
    }

    /// Set the maximum number of players
    pub fn max_players(mut self, max: usize) -> Self {
        self.max_players = Some(max);
        self
    }

    /// Set the player range (min and max)
    pub fn player_range(mut self, min: usize, max: usize) -> Self {
        self.min_players = Some(min);
        self.max_players = Some(max);
        self
    }

    /// Set the house edge percentage
    pub fn house_edge(mut self, edge: f64) -> Self {
        self.house_edge = Some(edge);
        self
    }

    /// Set betting limits
    pub fn betting_limits(mut self, limits: BettingLimits) -> Self {
        self.betting_limits = Some(limits);
        self
    }

    /// Set betting limits with individual values
    pub fn betting_limits_values(mut self, min_bet: u64, max_bet: u64, max_total_bet: u64) -> Self {
        self.betting_limits = Some(BettingLimits {
            min_bet,
            max_bet,
            max_total_bet,
        });
        self
    }

    /// Set time limits
    pub fn time_limits(mut self, limits: TimeLimits) -> Self {
        self.time_limits = Some(limits);
        self
    }

    /// Set time limits with individual values
    pub fn time_limits_values(mut self, action_timeout: u64, game_timeout: u64) -> Self {
        self.time_limits = Some(TimeLimits {
            action_timeout_seconds: action_timeout,
            game_timeout_minutes: game_timeout,
        });
        self
    }

    /// Add a payout multiplier for a specific bet type
    pub fn payout_multiplier<S: Into<String>>(mut self, bet_type: S, multiplier: f64) -> Self {
        self.payout_multipliers.insert(bet_type.into(), multiplier);
        self
    }

    /// Add multiple payout multipliers
    pub fn payout_multipliers(mut self, multipliers: HashMap<String, f64>) -> Self {
        self.payout_multipliers.extend(multipliers);
        self
    }

    /// Add a state field to the game schema
    pub fn state_field<S: Into<String>>(mut self, name: S, field_type: S) -> Self {
        self.state_fields.push(StateField {
            name: name.into(),
            field_type: field_type.into(),
        });
        self
    }

    /// Add multiple state fields
    pub fn state_fields(mut self, fields: Vec<StateField>) -> Self {
        self.state_fields.extend(fields);
        self
    }

    /// Add a required action
    pub fn required_action<S: Into<String>>(mut self, action: S) -> Self {
        self.required_actions.push(action.into());
        self
    }

    /// Add multiple required actions
    pub fn required_actions<S: Into<String>>(mut self, actions: Vec<S>) -> Self {
        self.required_actions.extend(actions.into_iter().map(|a| a.into()));
        self
    }

    /// Add a custom rule
    pub fn custom_rule<S: Into<String>>(mut self, key: S, value: serde_json::Value) -> Self {
        self.custom_rules.insert(key.into(), value);
        self
    }

    /// Add multiple custom rules
    pub fn custom_rules(mut self, rules: HashMap<String, serde_json::Value>) -> Self {
        self.custom_rules.extend(rules);
        self
    }

    /// Build the custom game
    pub fn build(self) -> Result<CustomGame, GameDevError> {
        // Validate required fields
        let name = self.name.ok_or_else(|| GameDevError::InvalidConfiguration("Game name is required".to_string()))?;
        let description = self.description.unwrap_or_else(|| "Custom game".to_string());
        let category = self.category.unwrap_or(GameCategory::CardGame);
        let min_players = self.min_players.unwrap_or(1);
        let max_players = self.max_players.unwrap_or(8);

        // Create template
        let template = GameTemplate {
            name: format!("{}_template", name.to_lowercase().replace(' ', "_")),
            description: description.clone(),
            category: category.clone(),
            min_players,
            max_players,
            default_rules: GameRules {
                deck_size: match category {
                    GameCategory::CardGame => Some(52),
                    _ => None,
                },
                hand_size: match category {
                    GameCategory::CardGame => Some(5),
                    _ => None,
                },
                turn_time_limit: Some(30),
                betting_rounds: Some(1),
                custom_rules: self.custom_rules.clone(),
            },
            state_schema: GameStateSchema {
                fields: self.state_fields,
            },
            required_actions: self.required_actions,
        };

        // Create game configuration
        let config = GameConfig {
            house_edge: self.house_edge.unwrap_or(2.0),
            payout_multipliers: self.payout_multipliers,
            betting_limits: self.betting_limits.unwrap_or_default(),
            time_limits: self.time_limits.unwrap_or_default(),
        };

        // Create the custom game
        let game = CustomGame {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            template: template.clone(),
            config,
            rules: template.default_rules.clone(),
            state_schema: template.state_schema.clone(),
        };

        Ok(game)
    }

    /// Build and validate the custom game
    pub async fn build_and_validate(self) -> Result<(CustomGame, ValidationReport), GameDevError> {
        let game = self.build()?;
        let validator = GameValidator::new();
        let report = validator.validate(&game).await?;
        Ok((game, report))
    }

    /// Build, validate, and generate code
    pub async fn build_with_code(self, language: ProgrammingLanguage) -> Result<GameBuildResult, GameDevError> {
        let (game, report) = self.build_and_validate().await?;
        
        let generator = CodeGenerator::new();
        let code = generator.generate_boilerplate(&game, language)?;
        let documentation = generator.generate_documentation(&game)?;

        Ok(GameBuildResult {
            game,
            validation_report: report,
            generated_code: code,
            documentation,
            language,
        })
    }
}

impl Default for GameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a complete game build process
pub struct GameBuildResult {
    pub game: CustomGame,
    pub validation_report: ValidationReport,
    pub generated_code: String,
    pub documentation: String,
    pub language: ProgrammingLanguage,
}

impl GameBuildResult {
    /// Check if the build was successful (game is valid)
    pub fn is_successful(&self) -> bool {
        self.validation_report.is_valid
    }

    /// Get a summary of the build result
    pub fn summary(&self) -> String {
        format!(
            "Game: {} | Validation: {} | Code: {} ({}) | Docs: {}",
            self.game.name,
            if self.is_successful() { "✓" } else { "✗" },
            "Generated",
            self.language.name(),
            "Generated"
        )
    }

    /// Save all generated files to a directory
    pub async fn save_to_directory(&self, output_dir: &std::path::Path) -> Result<(), std::io::Error> {
        use std::fs;
        use std::path::Path;

        // Create output directory
        fs::create_dir_all(output_dir)?;

        // Save main code file
        let code_file = output_dir.join(format!("game.{}", self.language.file_extension()));
        fs::write(&code_file, &self.generated_code)?;

        // Save documentation
        let doc_file = output_dir.join("README.md");
        fs::write(&doc_file, &self.documentation)?;

        // Save validation report
        let report_file = output_dir.join("validation_report.txt");
        let report_content = format!(
            "Validation Report for {}\n{}

Errors:
{}

Warnings:
{}

Suggestions:
{}
",
            self.game.name,
            "=".repeat(40),
            if self.validation_report.errors.is_empty() {
                "None"
            } else {
                &self.validation_report.errors.join("\n")
            },
            if self.validation_report.warnings.is_empty() {
                "None"
            } else {
                &self.validation_report.warnings.join("\n")
            },
            if self.validation_report.suggestions.is_empty() {
                "None"
            } else {
                &self.validation_report.suggestions.join("\n")
            }
        );
        fs::write(&report_file, report_content)?;

        // Save game configuration as JSON
        let config_file = output_dir.join("game_config.json");
        let config_json = serde_json::to_string_pretty(&self.game.config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(&config_file, config_json)?;

        Ok(())
    }
}

/// Preset game builders for common game types
pub struct GamePresets;

impl GamePresets {
    /// Create a dice game builder with common settings
    pub fn dice_game() -> GameBuilder {
        GameBuilder::new()
            .category(GameCategory::DiceGame)
            .player_range(1, 12)
            .house_edge(1.4) // Typical craps house edge
            .betting_limits_values(1, 1000, 10000)
            .time_limits_values(15, 60)
            .state_field("dice", "Vec<u8>")
            .state_field("phase", "DicePhase")
            .state_field("bets", "Vec<Bet>")
            .required_action("roll_dice")
            .required_action("place_bet")
            .required_action("resolve_bets")
            .payout_multiplier("Pass", 1.0)
            .payout_multiplier("Don't Pass", 1.0)
            .payout_multiplier("Field", 2.0)
            .custom_rule("dice_count", serde_json::json!(2))
            .custom_rule("sides_per_die", serde_json::json!(6))
    }

    /// Create a card game builder with common settings
    pub fn card_game() -> GameBuilder {
        GameBuilder::new()
            .category(GameCategory::CardGame)
            .player_range(2, 8)
            .house_edge(2.5)
            .betting_limits_values(5, 500, 5000)
            .time_limits_values(30, 120)
            .state_field("deck", "Vec<Card>")
            .state_field("players", "HashMap<String, PlayerHand>")
            .state_field("current_turn", "String")
            .state_field("pot", "u64")
            .required_action("deal_cards")
            .required_action("play_card")
            .required_action("end_turn")
            .payout_multiplier("Player", 1.0)
            .payout_multiplier("Banker", 0.95) // Banker pays commission
            .payout_multiplier("Tie", 8.0)
            .custom_rule("deck_size", serde_json::json!(52))
            .custom_rule("hand_size", serde_json::json!(7))
    }

    /// Create an auction game builder with common settings
    pub fn auction_game() -> GameBuilder {
        GameBuilder::new()
            .category(GameCategory::AuctionGame)
            .player_range(2, 10)
            .house_edge(5.0) // House takes commission
            .betting_limits_values(10, 10000, 100000)
            .time_limits_values(60, 180)
            .state_field("current_item", "Option<AuctionItem>")
            .state_field("highest_bid", "Option<Bid>")
            .state_field("time_remaining", "u64")
            .state_field("participants", "HashMap<String, u64>")
            .required_action("place_bid")
            .required_action("pass")
            .required_action("end_auction")
            .custom_rule("starting_bid", serde_json::json!(10))
            .custom_rule("bid_increment", serde_json::json!(5))
            .custom_rule("auction_time_limit", serde_json::json!(120))
    }

    /// Create a strategy game builder with common settings
    pub fn strategy_game() -> GameBuilder {
        GameBuilder::new()
            .category(GameCategory::StrategyGame)
            .player_range(2, 4)
            .house_edge(0.0) // Usually no house edge for strategy games
            .betting_limits_values(0, 0, 0) // No betting in strategy games
            .time_limits_values(120, 300)
            .state_field("board", "GameBoard")
            .state_field("current_player", "String")
            .state_field("turn_count", "u32")
            .state_field("winner", "Option<String>")
            .required_action("move_unit")
            .required_action("attack")
            .required_action("end_turn")
            .custom_rule("board_width", serde_json::json!(8))
            .custom_rule("board_height", serde_json::json!(8))
            .custom_rule("units_per_player", serde_json::json!(6))
    }

    /// Create a puzzle game builder with common settings
    pub fn puzzle_game() -> GameBuilder {
        GameBuilder::new()
            .category(GameCategory::PuzzleGame)
            .player_range(1, 1) // Single player
            .house_edge(0.0)
            .betting_limits_values(0, 0, 0) // No betting in puzzle games
            .time_limits_values(300, 60) // 5 minutes per action, 1 hour total
            .state_field("puzzle", "Puzzle")
            .state_field("solution", "Option<Solution>")
            .state_field("hints_used", "u32")
            .state_field("score", "u64")
            .required_action("submit_solution")
            .required_action("request_hint")
            .required_action("reset_puzzle")
            .custom_rule("difficulty", serde_json::json!("medium"))
            .custom_rule("max_hints", serde_json::json!(3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_builder_basic() {
        let game = GameBuilder::new()
            .name("Test Game")
            .description("A test game")
            .category(GameCategory::DiceGame)
            .player_range(2, 6)
            .house_edge(2.5)
            .build()
            .unwrap();

        assert_eq!(game.name, "Test Game");
        assert_eq!(game.template.min_players, 2);
        assert_eq!(game.template.max_players, 6);
        assert_eq!(game.config.house_edge, 2.5);
    }

    #[test]
    fn test_game_builder_with_fields() {
        let game = GameBuilder::new()
            .name("Dice Game")
            .state_field("dice", "Vec<u8>")
            .state_field("phase", "GamePhase")
            .required_action("roll_dice")
            .required_action("place_bet")
            .build()
            .unwrap();

        assert_eq!(game.state_schema.fields.len(), 2);
        assert_eq!(game.template.required_actions.len(), 2);
    }

    #[tokio::test]
    async fn test_game_builder_validation() {
        let result = GameBuilder::new()
            .name("Test Game")
            .category(GameCategory::CardGame)
            .build_and_validate()
            .await
            .unwrap();

        let (game, report) = result;
        assert_eq!(game.name, "Test Game");
        // Should be valid with default values
        assert!(report.is_valid);
    }

    #[test]
    fn test_preset_dice_game() {
        let game = GamePresets::dice_game()
            .name("Craps")
            .description("Classic craps game")
            .build()
            .unwrap();

        assert_eq!(game.name, "Craps");
        assert!(matches!(game.template.category, GameCategory::DiceGame));
        assert_eq!(game.config.house_edge, 1.4);
        assert!(game.template.required_actions.contains(&"roll_dice".to_string()));
    }

    #[test]
    fn test_preset_card_game() {
        let game = GamePresets::card_game()
            .name("Poker")
            .description("Texas Hold'em Poker")
            .build()
            .unwrap();

        assert_eq!(game.name, "Poker");
        assert!(matches!(game.template.category, GameCategory::CardGame));
        assert!(game.template.required_actions.contains(&"deal_cards".to_string()));
    }

    #[test]
    fn test_required_name_validation() {
        let result = GameBuilder::new()
            .category(GameCategory::DiceGame)
            .build();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), GameDevError::InvalidConfiguration(_)));
    }

    #[tokio::test]
    async fn test_build_with_code() {
        let result = GamePresets::dice_game()
            .name("Test Dice Game")
            .build_with_code(ProgrammingLanguage::Rust)
            .await
            .unwrap();

        assert_eq!(result.game.name, "Test Dice Game");
        assert!(!result.generated_code.is_empty());
        assert!(!result.documentation.is_empty());
        assert!(matches!(result.language, ProgrammingLanguage::Rust));
    }
}