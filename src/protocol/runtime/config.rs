//! Runtime Configuration
//!
//! Centralized configuration for the game runtime.

use crate::protocol::consensus::ConsensusConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Gaming runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRuntimeConfig {
    /// Maximum concurrent games allowed
    pub max_concurrent_games: usize,

    /// Minimum players required to start a game
    pub min_players: usize,

    /// Maximum players allowed in a game
    pub max_players: usize,

    /// Game timeout for inactivity
    pub game_timeout: Duration,

    /// Timeout for dice rolls
    pub roll_timeout: Duration,

    /// Minimum bet amount
    pub min_bet: u64,

    /// Maximum bet amount
    pub max_bet: u64,

    /// Treasury rake percentage (e.g., 0.01 for 1%)
    pub treasury_rake: f32,

    /// Enable anti-cheat monitoring
    pub enable_anti_cheat: bool,

    /// Enable consensus for game state
    pub enable_consensus: bool,

    /// Consensus configuration
    pub consensus_config: ConsensusConfig,

    /// Player session timeout
    pub session_timeout: Duration,

    /// Enable detailed logging
    pub enable_debug_logging: bool,

    /// Maximum bet types allowed per game
    pub max_bet_types: usize,

    /// Enable automatic game suspension on errors
    pub auto_suspend_on_error: bool,
}

impl Default for GameRuntimeConfig {
    fn default() -> Self {
        Self {
            max_concurrent_games: 100,
            min_players: 2,
            max_players: 10,
            game_timeout: Duration::from_secs(3600), // 1 hour
            roll_timeout: Duration::from_secs(30),
            min_bet: 100,
            max_bet: 1_000_000,
            treasury_rake: 0.01, // 1% rake
            enable_anti_cheat: true,
            enable_consensus: true,
            consensus_config: ConsensusConfig::default(),
            session_timeout: Duration::from_secs(7200), // 2 hours
            enable_debug_logging: false,
            max_bet_types: 20,
            auto_suspend_on_error: true,
        }
    }
}

impl GameRuntimeConfig {
    /// Create a configuration for testing
    pub fn for_testing() -> Self {
        Self {
            max_concurrent_games: 10,
            min_players: 1, // Allow single player for testing
            max_players: 4,
            game_timeout: Duration::from_secs(300), // 5 minutes
            roll_timeout: Duration::from_secs(10),
            min_bet: 10,
            max_bet: 10_000,
            treasury_rake: 0.0, // No rake for testing
            enable_anti_cheat: false,
            enable_consensus: false,
            consensus_config: ConsensusConfig::default(),
            session_timeout: Duration::from_secs(600), // 10 minutes
            enable_debug_logging: true,
            max_bet_types: 10,
            auto_suspend_on_error: false,
        }
    }

    /// Create a production configuration
    pub fn for_production() -> Self {
        Self {
            max_concurrent_games: 1000,
            min_players: 2,
            max_players: 20,
            game_timeout: Duration::from_secs(7200), // 2 hours
            roll_timeout: Duration::from_secs(60),
            min_bet: 1000,
            max_bet: 10_000_000,
            treasury_rake: 0.02, // 2% rake
            enable_anti_cheat: true,
            enable_consensus: true,
            consensus_config: ConsensusConfig::default(),
            session_timeout: Duration::from_secs(14400), // 4 hours
            enable_debug_logging: false,
            max_bet_types: 30,
            auto_suspend_on_error: true,
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.min_players < 1 {
            return Err("min_players must be at least 1".to_string());
        }

        if self.max_players < self.min_players {
            return Err("max_players must be >= min_players".to_string());
        }

        if self.min_bet >= self.max_bet {
            return Err("min_bet must be < max_bet".to_string());
        }

        if self.treasury_rake < 0.0 || self.treasury_rake > 1.0 {
            return Err("treasury_rake must be between 0.0 and 1.0".to_string());
        }

        Ok(())
    }
}
