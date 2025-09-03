//! Game Validation Module for BitCraps SDK
//!
//! This module provides comprehensive validation logic for custom games,
//! ensuring they meet platform requirements and best practices.

use crate::sdk::game_types::*;

/// Game validator for checking game implementations
pub struct GameValidator {
    strict_mode: bool,
}

impl GameValidator {
    /// Create a new validator with default settings
    pub fn new() -> Self {
        Self {
            strict_mode: false,
        }
    }

    /// Create a new validator with strict validation enabled
    pub fn new_strict() -> Self {
        Self {
            strict_mode: true,
        }
    }

    /// Enable or disable strict validation mode
    pub fn set_strict_mode(&mut self, strict: bool) {
        self.strict_mode = strict;
    }

    /// Validate a custom game implementation
    pub async fn validate(&self, game: &CustomGame) -> Result<ValidationReport, GameDevError> {
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        };

        // Validate basic game properties
        self.validate_basic_properties(game, &mut report);
        
        // Validate game configuration
        self.validate_configuration(game, &mut report);
        
        // Validate state schema
        self.validate_state_schema(game, &mut report);
        
        // Validate required actions
        self.validate_required_actions(game, &mut report);
        
        // Validate betting mechanics
        self.validate_betting_mechanics(game, &mut report);
        
        // Validate time limits
        self.validate_time_limits(game, &mut report);

        // Add performance suggestions
        self.add_performance_suggestions(game, &mut report);

        Ok(report)
    }

    fn validate_basic_properties(&self, game: &CustomGame, report: &mut ValidationReport) {
        // Game name validation
        if game.name.is_empty() {
            report.errors.push("Game name cannot be empty".to_string());
            report.is_valid = false;
        } else if game.name.len() < 3 {
            report.warnings.push("Game name should be at least 3 characters long".to_string());
        } else if game.name.len() > 50 {
            report.warnings.push("Game name should be less than 50 characters".to_string());
        }

        // Player count validation
        if game.template.min_players == 0 {
            report.errors.push("Minimum players must be at least 1".to_string());
            report.is_valid = false;
        }

        if game.template.min_players > game.template.max_players {
            report.errors.push("Minimum players cannot be greater than maximum players".to_string());
            report.is_valid = false;
        }

        if self.strict_mode && game.template.max_players > 20 {
            report.errors.push("Maximum players cannot exceed 20 in strict mode".to_string());
            report.is_valid = false;
        } else if game.template.max_players > 50 {
            report.warnings.push("Very large player counts may cause performance issues".to_string());
        }

        // Game ID validation
        if game.id.is_empty() {
            report.errors.push("Game ID cannot be empty".to_string());
            report.is_valid = false;
        }
    }

    fn validate_configuration(&self, game: &CustomGame, report: &mut ValidationReport) {
        // House edge validation
        if game.config.house_edge < 0.0 {
            report.errors.push("House edge cannot be negative".to_string());
            report.is_valid = false;
        } else if game.config.house_edge > 50.0 {
            if self.strict_mode {
                report.errors.push("House edge cannot exceed 50% in strict mode".to_string());
                report.is_valid = false;
            } else {
                report.warnings.push("House edge above 50% may be considered unfair".to_string());
            }
        }

        // Optimal house edge suggestions
        if game.config.house_edge > 10.0 {
            report.suggestions.push("Consider lowering house edge below 10% for better player retention".to_string());
        } else if game.config.house_edge > 5.0 {
            report.suggestions.push("House edge above 5% may impact player satisfaction".to_string());
        }

        // Payout multiplier validation
        for (bet_type, multiplier) in &game.config.payout_multipliers {
            if *multiplier <= 0.0 {
                report.errors.push(format!("Payout multiplier for '{}' must be positive", bet_type));
                report.is_valid = false;
            } else if *multiplier > 1000.0 {
                report.warnings.push(format!("Very high payout multiplier for '{}' may cause balance issues", bet_type));
            }
        }
    }

    fn validate_state_schema(&self, game: &CustomGame, report: &mut ValidationReport) {
        if game.state_schema.fields.is_empty() {
            if self.strict_mode {
                report.errors.push("Game state schema cannot be empty in strict mode".to_string());
                report.is_valid = false;
            } else {
                report.warnings.push("Game state schema has no fields defined".to_string());
            }
        }

        // Validate field names and types
        for field in &game.state_schema.fields {
            if field.name.is_empty() {
                report.errors.push("State field name cannot be empty".to_string());
                report.is_valid = false;
            }

            if field.field_type.is_empty() {
                report.errors.push(format!("Field '{}' has no type specified", field.name));
                report.is_valid = false;
            }

            // Check for reserved field names
            if ["id", "session_id", "created_at", "updated_at"].contains(&field.name.as_str()) {
                report.warnings.push(format!("Field name '{}' may conflict with system fields", field.name));
            }
        }

        // Check for duplicate field names
        let mut field_names = std::collections::HashSet::new();
        for field in &game.state_schema.fields {
            if !field_names.insert(&field.name) {
                report.errors.push(format!("Duplicate field name: '{}'", field.name));
                report.is_valid = false;
            }
        }
    }

    fn validate_required_actions(&self, game: &CustomGame, report: &mut ValidationReport) {
        if game.template.required_actions.is_empty() {
            if self.strict_mode {
                report.errors.push("No required actions defined for game in strict mode".to_string());
                report.is_valid = false;
            } else {
                report.warnings.push("No required actions defined for game".to_string());
            }
        }

        // Check for common required actions based on game category
        match game.template.category {
            crate::sdk::templates::GameCategory::CardGame => {
                if !game.template.required_actions.contains(&"deal_cards".to_string()) {
                    report.suggestions.push("Card games typically require a 'deal_cards' action".to_string());
                }
            },
            crate::sdk::templates::GameCategory::DiceGame => {
                if !game.template.required_actions.contains(&"roll_dice".to_string()) {
                    report.suggestions.push("Dice games typically require a 'roll_dice' action".to_string());
                }
            },
            _ => {}
        }

        // Check for duplicate actions
        let mut action_names = std::collections::HashSet::new();
        for action in &game.template.required_actions {
            if !action_names.insert(action) {
                report.warnings.push(format!("Duplicate required action: '{}'", action));
            }
        }
    }

    fn validate_betting_mechanics(&self, game: &CustomGame, report: &mut ValidationReport) {
        let limits = &game.config.betting_limits;

        // Basic betting limit validation
        if limits.min_bet == 0 {
            report.warnings.push("Minimum bet of 0 may allow free play".to_string());
        }

        if limits.min_bet > limits.max_bet {
            report.errors.push("Minimum bet cannot be greater than maximum bet".to_string());
            report.is_valid = false;
        }

        if limits.max_bet > limits.max_total_bet {
            report.errors.push("Maximum bet cannot be greater than maximum total bet".to_string());
            report.is_valid = false;
        }

        // Reasonable limit suggestions
        if limits.max_bet > 100000 {
            report.suggestions.push("Very high maximum bet may require special risk management".to_string());
        }

        if limits.min_bet > 100 {
            report.suggestions.push("High minimum bet may exclude casual players".to_string());
        }
    }

    fn validate_time_limits(&self, game: &CustomGame, report: &mut ValidationReport) {
        let limits = &game.config.time_limits;

        // Action timeout validation
        if limits.action_timeout_seconds == 0 {
            report.warnings.push("No action timeout may allow games to stall indefinitely".to_string());
        } else if limits.action_timeout_seconds > 300 {
            report.suggestions.push("Very long action timeouts may slow gameplay".to_string());
        } else if limits.action_timeout_seconds < 5 {
            report.warnings.push("Very short action timeouts may frustrate players".to_string());
        }

        // Game timeout validation
        if limits.game_timeout_minutes == 0 {
            report.warnings.push("No game timeout may allow sessions to run indefinitely".to_string());
        } else if limits.game_timeout_minutes > 480 { // 8 hours
            report.suggestions.push("Very long game timeouts may consume excessive resources".to_string());
        }
    }

    fn add_performance_suggestions(&self, game: &CustomGame, report: &mut ValidationReport) {
        // Large player count performance warnings
        if game.template.max_players > 10 {
            report.suggestions.push("Consider implementing player batching for large games".to_string());
        }

        if game.template.max_players > 20 {
            report.suggestions.push("Large player counts may require load balancing strategies".to_string());
        }

        // Complex state schema warnings
        if game.state_schema.fields.len() > 20 {
            report.suggestions.push("Complex state schemas may benefit from caching strategies".to_string());
        }

        // Many required actions
        if game.template.required_actions.len() > 15 {
            report.suggestions.push("Many required actions may complicate game logic - consider grouping".to_string());
        }
    }
}

impl Default for GameValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation report containing errors, warnings, and suggestions
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

impl ValidationReport {
    /// Create a new empty validation report
    pub fn new() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Check if the report has any issues
    pub fn has_issues(&self) -> bool {
        !self.errors.is_empty() || !self.warnings.is_empty()
    }

    /// Get total number of issues (errors + warnings)
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }

    /// Generate a human-readable summary
    pub fn summary(&self) -> String {
        let mut summary = String::new();
        
        if self.is_valid {
            summary.push_str("✓ Validation passed");
        } else {
            summary.push_str("✗ Validation failed");
        }

        if !self.errors.is_empty() {
            summary.push_str(&format!(" ({} errors)", self.errors.len()));
        }

        if !self.warnings.is_empty() {
            summary.push_str(&format!(" ({} warnings)", self.warnings.len()));
        }

        if !self.suggestions.is_empty() {
            summary.push_str(&format!(" ({} suggestions)", self.suggestions.len()));
        }

        summary
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during game development
#[derive(Debug, thiserror::Error)]
pub enum GameDevError {
    #[error("Template not found: {0}")]
    TemplateNotFound(String),
    
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Code generation failed: {0}")]
    CodeGenerationFailed(String),
    
    #[error("Export failed: {0}")]
    ExportFailed(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::templates::*;

    fn create_test_game() -> CustomGame {
        CustomGame {
            id: "test-game-123".to_string(),
            name: "Test Game".to_string(),
            template: GameTemplate {
                name: "test_template".to_string(),
                description: "Test template".to_string(),
                category: GameCategory::DiceGame,
                min_players: 2,
                max_players: 6,
                default_rules: GameRules {
                    deck_size: None,
                    hand_size: None,
                    turn_time_limit: Some(30),
                    betting_rounds: Some(1),
                    custom_rules: std::collections::HashMap::new(),
                },
                state_schema: GameStateSchema {
                    fields: vec![
                        StateField { name: "dice".to_string(), field_type: "Vec<u8>".to_string() },
                        StateField { name: "phase".to_string(), field_type: "GamePhase".to_string() },
                    ],
                },
                required_actions: vec!["roll_dice".to_string(), "place_bet".to_string()],
            },
            config: GameConfig::default(),
            rules: GameRules {
                deck_size: None,
                hand_size: None,
                turn_time_limit: Some(30),
                betting_rounds: Some(1),
                custom_rules: std::collections::HashMap::new(),
            },
            state_schema: GameStateSchema {
                fields: vec![
                    StateField { name: "dice".to_string(), field_type: "Vec<u8>".to_string() },
                ],
            },
        }
    }

    #[tokio::test]
    async fn test_valid_game_validation() {
        let validator = GameValidator::new();
        let game = create_test_game();
        
        let report = validator.validate(&game).await.unwrap();
        assert!(report.is_valid);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_game_name() {
        let validator = GameValidator::new();
        let mut game = create_test_game();
        game.name = "".to_string(); // Empty name
        
        let report = validator.validate(&game).await.unwrap();
        assert!(!report.is_valid);
        assert!(!report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_invalid_player_counts() {
        let validator = GameValidator::new();
        let mut game = create_test_game();
        game.template.min_players = 5;
        game.template.max_players = 3; // Min > Max
        
        let report = validator.validate(&game).await.unwrap();
        assert!(!report.is_valid);
        assert!(report.errors.iter().any(|e| e.contains("Minimum players cannot be greater")));
    }

    #[tokio::test]
    async fn test_strict_mode_validation() {
        let validator = GameValidator::new_strict();
        let mut game = create_test_game();
        game.state_schema.fields.clear(); // Empty schema
        
        let report = validator.validate(&game).await.unwrap();
        assert!(!report.is_valid);
        assert!(report.errors.iter().any(|e| e.contains("Game state schema cannot be empty")));
    }

    #[test]
    fn test_validation_report_summary() {
        let mut report = ValidationReport::new();
        report.errors.push("Test error".to_string());
        report.warnings.push("Test warning".to_string());
        report.is_valid = false;
        
        let summary = report.summary();
        assert!(summary.contains("✗ Validation failed"));
        assert!(summary.contains("1 errors"));
        assert!(summary.contains("1 warnings"));
    }
}