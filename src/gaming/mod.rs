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

pub mod consensus_game_manager;
pub mod game_orchestrator;
pub mod multi_game_framework;
pub mod payout_engine;

pub use multi_game_framework::{
    CreateSessionRequest, GameAction, GameActionResult, GameEngine, GameEngineError,
    GameFrameworkConfig, GameFrameworkError, GameFrameworkEvent, GameFrameworkStatistics, GameInfo,
    GameSession, GameSessionConfig, MultiGameFramework, PlayerJoinData, SessionEndReason,
    SessionSummary,
};

// Re-export game engines
pub use multi_game_framework::{BlackjackGameEngine, CrapsGameEngine, PokerGameEngine};

// Re-export consensus game management
pub use consensus_game_manager::{
    ConsensusGameConfig, ConsensusGameManager, ConsensusGameSession, GameEvent, GameManagerStats,
};

// Re-export game orchestrator
pub use game_orchestrator::{
    BetRecord, DiceCommit, DiceReveal, GameAdvertisement, GameConfig, GameDiscoveryRequest,
    GameDiscoveryResponse, GameJoinRequest, GameJoinResponse, GameOrchestrator, GameStateSnapshot,
    OrchestratorCommand, OrchestratorConfig, OrchestratorEvent, OrchestratorStats,
    PlayerCapabilities, TurnManager,
};

// Re-export payout engine
pub use payout_engine::{
    BetValidationRequest, BetValidationResponse, BetValidationRules, InvalidBetReason,
    InvalidBetSeverity, LosingBet, PayoutEngine, PayoutEngineStats, PayoutResult, PayoutSignature,
    PlayerPayout, WinningBet,
};
