//! Utility modules for BitChat
//!
//! This module contains various utility functions and helpers used
//! throughout the BitChat application.

pub mod adaptive_interval;
pub mod correlation;
pub mod growable_buffer;
pub mod loop_budget;
pub mod timeout;

pub use adaptive_interval::{AdaptiveInterval, AdaptiveIntervalConfig};
pub use correlation::{CorrelationId, RequestContext, CorrelationManager, CorrelationMiddleware, CorrelationSpanExt};
pub use growable_buffer::GrowableBuffer;
pub use loop_budget::{BoundedLoop, CircuitBreaker, LoadShedder, LoopBudget, OverflowHandler};
pub use timeout::{TimeoutConfig, TimeoutDefaults, TimeoutError, TimeoutExt, TimeoutGuard};
