//! Performance monitoring and optimization module
//!
//! This module provides comprehensive performance analysis, benchmarking,
//! and optimization tools for BitCraps.

pub mod benchmarking;
pub mod optimizer;

pub use benchmarking::*;
pub use optimizer::{OptimizationStrategy, PerformanceMetrics, PerformanceOptimizer};
