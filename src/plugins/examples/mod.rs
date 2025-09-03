//! Example Plugin Implementations for BitCraps
//!
//! This module provides reference implementations of various casino game
//! plugins demonstrating the plugin API and best practices.

pub mod blackjack;
pub mod poker;
pub mod roulette;
pub mod slot_machine;

// Re-export example plugins
pub use blackjack::BlackjackPlugin;
pub use poker::PokerPlugin;
pub use roulette::RoulettePlugin;
pub use slot_machine::SlotMachinePlugin;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::core::*;
use crate::gaming::GameAction;

/// Common plugin utilities and helpers
pub struct PluginUtils;

impl PluginUtils {
    /// Generate plugin ID from name and version
    pub fn generate_plugin_id(name: &str, version: &str) -> String {
        format!("{}_{}", name.to_lowercase().replace(' ', "_"), version.replace('.', "_"))
    }

    /// Create default plugin info template
    pub fn create_plugin_info_template(
        name: &str,
        version: &str,
        game_type: GameType,
        description: &str,
    ) -> PluginInfo {
        PluginInfo {
            id: Self::generate_plugin_id(name, version),
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            author: "BitCraps Team".to_string(),
            license: "MIT".to_string(),
            website: Some("https://bitcraps.com".to_string()),
            api_version: "1.0".to_string(),
            minimum_platform_version: "1.0.0".to_string(),
            game_type,
            supported_features: vec![
                "network".to_string(),
                "storage".to_string(),
                "crypto".to_string(),
            ],
            dependencies: vec![],
        }
    }

    /// Validate game action against allowed actions
    pub fn validate_action_type(action: &GameAction, allowed_actions: &[&str]) -> bool {
        let action_name = match action {
            GameAction::PlaceBet { .. } => "place_bet",
            GameAction::RollDice => "roll_dice",
            GameAction::Hit => "hit",
            GameAction::Stand => "stand",
            GameAction::Fold => "fold",
            GameAction::Check => "check",
            GameAction::Call => "call",
            GameAction::Raise { .. } => "raise",
        };

        allowed_actions.contains(&action_name)
    }

    /// Create standard plugin health status
    pub fn create_health_status(
        state: PluginState,
        memory_mb: u64,
        cpu_percent: f32,
        error_count: u64,
        warnings: Vec<String>,
    ) -> PluginHealth {
        PluginHealth {
            state,
            memory_usage_mb: memory_mb,
            cpu_usage_percent: cpu_percent,
            last_heartbeat: std::time::SystemTime::now(),
            error_count,
            warnings,
        }
    }

    /// Generate session configuration with defaults
    pub fn create_session_config() -> HashMap<String, serde_json::Value> {
        let mut config = HashMap::new();
        config.insert("min_bet".to_string(), serde_json::json!(1));
        config.insert("max_bet".to_string(), serde_json::json!(1000));
        config.insert("auto_start".to_string(), serde_json::json!(false));
        config.insert("max_players".to_string(), serde_json::json!(8));
        config
    }
}

/// Common game state structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonGameState {
    pub session_id: String,
    pub players: HashMap<String, PlayerState>,
    pub current_phase: String,
    pub game_data: serde_json::Value,
    pub created_at: std::time::SystemTime,
    pub last_updated: std::time::SystemTime,
}

/// Player state within a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub player_id: String,
    pub balance: u64,
    pub is_active: bool,
    pub joined_at: std::time::SystemTime,
    pub player_data: serde_json::Value,
}

/// Plugin error handling utilities
#[derive(Debug, Clone)]
pub struct PluginErrorHandler {
    error_count: std::sync::atomic::AtomicU64,
    warnings: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl PluginErrorHandler {
    pub fn new() -> Self {
        Self {
            error_count: std::sync::atomic::AtomicU64::new(0),
            warnings: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn record_error(&self, error: &str) {
        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tracing::error!("Plugin error: {}", error);
    }

    pub fn record_warning(&self, warning: &str) {
        if let Ok(mut warnings) = self.warnings.lock() {
            warnings.push(warning.to_string());
            // Keep only recent warnings
            if warnings.len() > 100 {
                warnings.drain(0..50);
            }
        }
        tracing::warn!("Plugin warning: {}", warning);
    }

    pub fn get_error_count(&self) -> u64 {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_warnings(&self) -> Vec<String> {
        self.warnings.lock().unwrap_or_else(|_| std::sync::MutexGuard::new(Vec::new())).clone()
    }
}

impl Default for PluginErrorHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Base plugin statistics tracker
#[derive(Debug, Clone)]
pub struct BasePluginStatistics {
    pub sessions_created: std::sync::atomic::AtomicU64,
    pub actions_processed: std::sync::atomic::AtomicU64,
    pub errors_encountered: std::sync::atomic::AtomicU64,
    pub messages_sent: std::sync::atomic::AtomicU64,
    pub messages_received: std::sync::atomic::AtomicU64,
    pub start_time: std::time::Instant,
}

impl BasePluginStatistics {
    pub fn new() -> Self {
        Self {
            sessions_created: std::sync::atomic::AtomicU64::new(0),
            actions_processed: std::sync::atomic::AtomicU64::new(0),
            errors_encountered: std::sync::atomic::AtomicU64::new(0),
            messages_sent: std::sync::atomic::AtomicU64::new(0),
            messages_received: std::sync::atomic::AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn to_plugin_statistics(&self) -> PluginStatistics {
        let uptime = self.start_time.elapsed().as_secs();
        let total_actions = self.actions_processed.load(std::sync::atomic::Ordering::Relaxed);
        let avg_response_time = if total_actions > 0 {
            // Simplified calculation - in practice would track actual response times
            50.0 // 50ms average
        } else {
            0.0
        };

        PluginStatistics {
            sessions_created: self.sessions_created.load(std::sync::atomic::Ordering::Relaxed),
            actions_processed: total_actions,
            errors_encountered: self.errors_encountered.load(std::sync::atomic::Ordering::Relaxed),
            messages_sent: self.messages_sent.load(std::sync::atomic::Ordering::Relaxed),
            messages_received: self.messages_received.load(std::sync::atomic::Ordering::Relaxed),
            uptime_seconds: uptime,
            average_response_time_ms: avg_response_time,
        }
    }
}

impl Default for BasePluginStatistics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id_generation() {
        let id = PluginUtils::generate_plugin_id("Blackjack Game", "1.2.3");
        assert_eq!(id, "blackjack_game_1_2_3");
    }

    #[test]
    fn test_action_validation() {
        let allowed = &["place_bet", "hit", "stand"];
        
        assert!(PluginUtils::validate_action_type(&GameAction::Hit, allowed));
        assert!(PluginUtils::validate_action_type(&GameAction::Stand, allowed));
        assert!(!PluginUtils::validate_action_type(&GameAction::Fold, allowed));
    }

    #[test]
    fn test_plugin_info_template() {
        let info = PluginUtils::create_plugin_info_template(
            "Test Game",
            "1.0.0",
            GameType::Blackjack,
            "A test game plugin"
        );
        
        assert_eq!(info.name, "Test Game");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.game_type, GameType::Blackjack);
        assert_eq!(info.id, "test_game_1_0_0");
    }

    #[test]
    fn test_error_handler() {
        let handler = PluginErrorHandler::new();
        
        handler.record_error("Test error");
        handler.record_warning("Test warning");
        
        assert_eq!(handler.get_error_count(), 1);
        assert_eq!(handler.get_warnings().len(), 1);
        assert_eq!(handler.get_warnings()[0], "Test warning");
    }

    #[test]
    fn test_base_statistics() {
        let stats = BasePluginStatistics::new();
        
        stats.sessions_created.fetch_add(5, std::sync::atomic::Ordering::Relaxed);
        stats.actions_processed.fetch_add(100, std::sync::atomic::Ordering::Relaxed);
        
        let plugin_stats = stats.to_plugin_statistics();
        assert_eq!(plugin_stats.sessions_created, 5);
        assert_eq!(plugin_stats.actions_processed, 100);
        assert!(plugin_stats.uptime_seconds >= 0);
    }
}