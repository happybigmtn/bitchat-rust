//! BitCraps Developer SDK
//!
//! This module provides a comprehensive SDK for developers to build applications
//! and integrations with the BitCraps platform:
//!
//! ## Core Features
//! - High-level API client
//! - Game development tools
//! - Testing utilities
//! - Code generation tools
//! - Performance profiling
//! - Integration helpers

pub mod client;
pub mod game_dev_kit;
pub mod testing;
pub mod codegen;
pub mod profiler;
pub mod integration;

pub use client::{BitCrapsClient, ClientConfig, ClientError};
pub use game_dev_kit::{GameDevKit, GameTemplate, GameValidator};
pub use testing::{TestFramework, MockEnvironment, TestScenario};
pub use codegen::{CodeGenerator, TemplateEngine, SchemaGenerator};
pub use profiler::{PerformanceProfiler, ProfileReport, Benchmark};
pub use integration::{IntegrationHelper, WebhookManager, EventBridge};