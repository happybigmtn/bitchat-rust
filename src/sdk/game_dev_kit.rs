//! Game Development Kit for BitCraps
//!
//! This is the legacy interface for the BitCraps Game Development Kit.
//! It provides backwards compatibility while using the new modular architecture.

use std::path::Path;
use std::collections::HashMap;

// Re-export all the modular components
pub use crate::sdk::templates::*;
pub use crate::sdk::game_types::*;
pub use crate::sdk::validation::*;
pub use crate::sdk::codegen::*;
pub use crate::sdk::builder::*;
pub use crate::sdk::custom_engine::*;

/// Game development kit for creating custom games (Legacy Interface)
pub struct GameDevKit {
    template_manager: TemplateManager,
    validator: GameValidator,
    code_generator: CodeGenerator,
}

impl GameDevKit {
    /// Create new game development kit
    pub fn new() -> Self {
        Self {
            template_manager: TemplateManager::new(),
            validator: GameValidator::new(),
            code_generator: CodeGenerator::new(),
        }
    }

    /// Create a new game from template
    pub fn create_game_from_template(
        &self, 
        template_name: &str, 
        game_name: &str, 
        config: GameConfig
    ) -> Result<CustomGame, GameDevError> {
        let template = self.template_manager.get_template(template_name)
            .ok_or_else(|| GameDevError::TemplateNotFound(template_name.to_string()))?
            .clone();

        let game = CustomGame {
            id: uuid::Uuid::new_v4().to_string(),
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
        self.code_generator.generate_boilerplate(game, language)
    }

    /// Export game as SDK package
    pub async fn export_game(&self, game: &CustomGame, output_path: &Path) -> Result<(), GameDevError> {
        // Generate documentation
        let documentation = self.code_generator.generate_documentation(game)?;

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
            documentation,
        };

        // Write package to filesystem
        self.write_game_package(&package, output_path).await?;

        Ok(())
    }

    /// Get available templates
    pub fn get_templates(&self) -> Vec<&GameTemplate> {
        self.template_manager.get_templates()
    }

    /// Add custom template
    pub fn add_template(&mut self, template: GameTemplate) {
        self.template_manager.add_template(template);
    }

    /// Write game package to filesystem
    async fn write_game_package(&self, package: &GamePackage, output_path: &Path) -> Result<(), GameDevError> {
        use tokio::fs;

        // Create output directory
        fs::create_dir_all(output_path).await
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to create directory: {}", e)))?;

        // Write package metadata
        let metadata_json = serde_json::to_string_pretty(&package.metadata)
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(output_path.join("package.json"), metadata_json).await
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to write metadata: {}", e)))?;

        // Write documentation
        fs::write(output_path.join("README.md"), &package.documentation).await
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to write documentation: {}", e)))?;

        // Write engine configuration (as JSON for portability)
        let engine_config = serde_json::json!({
            "name": package.engine.get_name(),
            "description": package.engine.get_description(),
            "min_players": package.engine.get_min_players(),
            "max_players": package.engine.get_max_players(),
            "supported_bet_types": package.engine.get_supported_bet_types(),
            "house_edge": package.engine.get_house_edge()
        });
        let engine_json = serde_json::to_string_pretty(&engine_config)
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to serialize engine config: {}", e)))?;
        fs::write(output_path.join("engine.json"), engine_json).await
            .map_err(|e| GameDevError::ExportFailed(format!("Failed to write engine config: {}", e)))?;

        Ok(())
    }

    /// Create a game builder (recommended modern interface)
    pub fn builder() -> GameBuilder {
        GameBuilder::new()
    }

    /// Create a preset game builder
    pub fn preset(preset_type: &str) -> GameBuilder {
        match preset_type {
            "dice" => GamePresets::dice_game(),
            "card" => GamePresets::card_game(),
            "auction" => GamePresets::auction_game(),
            "strategy" => GamePresets::strategy_game(),
            "puzzle" => GamePresets::puzzle_game(),
            _ => GameBuilder::new(),
        }
    }
}

impl Default for GameDevKit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_dev_kit_creation() {
        let kit = GameDevKit::new();
        let templates = kit.get_templates();
        assert!(templates.len() >= 5); // Should have built-in templates
    }

    #[test]
    fn test_create_game_from_template() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        
        let result = kit.create_game_from_template("dice_game", "My Dice Game", config);
        assert!(result.is_ok());
        
        let game = result.unwrap();
        assert_eq!(game.name, "My Dice Game");
        assert!(matches!(game.template.category, GameCategory::DiceGame));
    }

    #[tokio::test]
    async fn test_validate_game() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        let game = kit.create_game_from_template("card_game", "Test Game", config).unwrap();
        
        let report = kit.validate_game(&game).await.unwrap();
        assert!(report.is_valid);
    }

    #[test]
    fn test_generate_boilerplate_rust() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        let game = kit.create_game_from_template("dice_game", "Test Game", config).unwrap();
        
        let result = kit.generate_boilerplate(&game, ProgrammingLanguage::Rust);
        assert!(result.is_ok());
        
        let code = result.unwrap();
        assert!(code.contains("TestGameEngine"));
        assert!(code.contains("impl GameEngine"));
    }

    #[test]
    fn test_builder_interface() {
        let game = GameDevKit::builder()
            .name("Test Game")
            .category(GameCategory::DiceGame)
            .player_range(2, 6)
            .build()
            .unwrap();

        assert_eq!(game.name, "Test Game");
        assert!(matches!(game.template.category, GameCategory::DiceGame));
    }

    #[test]
    fn test_preset_builders() {
        let dice_game = GameDevKit::preset("dice")
            .name("Craps")
            .build()
            .unwrap();
        
        assert!(matches!(dice_game.template.category, GameCategory::DiceGame));
        assert_eq!(dice_game.config.house_edge, 1.4);

        let card_game = GameDevKit::preset("card")
            .name("Poker")
            .build()
            .unwrap();
        
        assert!(matches!(card_game.template.category, GameCategory::CardGame));
    }

    #[tokio::test]
    async fn test_export_game() {
        let kit = GameDevKit::new();
        let config = GameConfig::default();
        let game = kit.create_game_from_template("dice_game", "Export Test", config).unwrap();
        
        let temp_dir = std::env::temp_dir().join("bitcraps_export_test");
        
        let result = kit.export_game(&game, &temp_dir).await;
        assert!(result.is_ok());
        
        // Check that files were created
        assert!(temp_dir.join("package.json").exists());
        assert!(temp_dir.join("README.md").exists());
        assert!(temp_dir.join("engine.json").exists());
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}