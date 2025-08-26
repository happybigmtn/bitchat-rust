//! Gaming Module for BitCraps Multi-Game Platform
//! 
//! This module provides comprehensive gaming capabilities including:
//! 
//! ## Multi-Game Framework
//! - Support for multiple casino games (Craps, Blackjack, Poker, etc.)
//! - Plugin-based game engine architecture
//! - Unified game session management
//! - Cross-game interoperability
//! - Flexible betting systems
//! 
//! ## Game Features
//! - Real-time multiplayer gameplay
//! - Consensus-based game state
//! - Anti-cheat mechanisms
//! - Fair random number generation
//! - Comprehensive game statistics
//! 
//! ## Integration
//! - Seamless mesh network integration
//! - Mobile-optimized protocols
//! - Production monitoring and alerting

pub mod multi_game_framework;
pub mod consensus_game_manager;

pub use multi_game_framework::{
    MultiGameFramework,
    GameEngine,
    GameFrameworkConfig,
    GameInfo,
    CreateSessionRequest,
    GameSessionConfig,
    PlayerJoinData,
    GameSession,
    GameAction,
    GameActionResult,
    SessionSummary,
    SessionEndReason,
    GameFrameworkEvent,
    GameFrameworkStatistics,
    GameFrameworkError,
    GameEngineError,
};

// Re-export game engines
pub use multi_game_framework::{
    CrapsGameEngine,
    BlackjackGameEngine,
    PokerGameEngine,
};

// Re-export consensus game management
pub use consensus_game_manager::{
    ConsensusGameManager,
    ConsensusGameConfig,
    ConsensusGameSession,
    GameEvent,
    GameManagerStats,
};