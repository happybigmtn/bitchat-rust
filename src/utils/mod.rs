//! Utility modules for BitChat
//!
//! This module contains various utility functions and helpers used
//! throughout the BitChat application.

pub mod adaptive_interval;
pub mod growable_buffer;
pub mod loop_budget;

pub use adaptive_interval::{AdaptiveInterval, AdaptiveIntervalConfig};
pub use growable_buffer::GrowableBuffer;
pub use loop_budget::{LoopBudget, BoundedLoop, CircuitBreaker, LoadShedder, OverflowHandler};