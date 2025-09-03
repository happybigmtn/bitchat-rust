//! BitCraps Developer SDK
//!
//! This module provides a comprehensive SDK for developers to build applications
//! and integrations with the BitCraps platform:
//!
//! ## Core Features
//! - High-level API client
//! - Game development tools and templates
//! - Code generation for multiple languages
//! - Game validation and testing utilities
//! - Builder patterns for easy game creation
//! - Custom game engine implementation
//! - Performance profiling
//! - Integration helpers

// Core SDK modules
pub mod client;
pub mod testing;
pub mod profiler;
pub mod integration;

// Game development modules
pub mod templates;
pub mod game_types;
pub mod validation;
pub mod codegen;
pub mod builder;
pub mod custom_engine;

// Legacy game_dev_kit (now re-exports from modular components)
pub mod game_dev_kit;

// Public API exports
pub use client::{BitCrapsClient, ClientConfig, ClientError};
pub use testing::{TestFramework, MockEnvironment, TestScenario};
pub use profiler::{PerformanceProfiler, ProfileReport, Benchmark};
pub use integration::{IntegrationHelper, WebhookManager, EventBridge};

// Game development exports
pub use templates::{GameTemplate, GameCategory, GameRules, GameStateSchema, StateField, TemplateManager};
pub use game_types::{CustomGame, GameConfig, BettingLimits, TimeLimits, ProgrammingLanguage, GamePackage};
pub use validation::{GameValidator, ValidationReport, GameDevError};
pub use codegen::{CodeGenerator, to_pascal_case, to_snake_case, to_camel_case};
pub use builder::{GameBuilder, GamePresets, GameBuildResult};
pub use custom_engine::CustomGameEngine;

// Re-export main game dev kit for backwards compatibility
pub use game_dev_kit::GameDevKit;