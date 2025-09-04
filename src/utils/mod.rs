//! Utility modules for BitChat
//!
//! This module contains various utility functions and helpers used
//! throughout the BitChat application.

pub mod adaptive_interval;
pub mod clone_reducer;
pub mod correlation;
pub mod growable_buffer;
pub mod lock_ordering;
pub mod loop_budget;
pub mod task;
pub mod task_tracker;
pub mod timeout;

pub use adaptive_interval::{AdaptiveInterval, AdaptiveIntervalConfig};
pub use correlation::{CorrelationId, RequestContext, CorrelationManager, CorrelationMiddleware, CorrelationSpanExt};
pub use growable_buffer::GrowableBuffer;
pub use loop_budget::{BoundedLoop, CircuitBreaker, LoadShedder, LoopBudget, OverflowHandler};
pub use task_tracker::{TaskTracker, spawn_tracked, TaskType, TaskInfo};
pub use timeout::{TimeoutConfig, TimeoutDefaults, TimeoutError, TimeoutExt, TimeoutGuard};
