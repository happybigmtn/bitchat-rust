//! Performance monitoring and optimization module
//!
//! This module provides comprehensive performance analysis, benchmarking,
//! and optimization tools for BitCraps.

pub mod benchmarking;
pub mod optimizer;
pub mod soak_test;

pub use benchmarking::*;
pub use optimizer::{
    // M8 Performance additions
    AdaptiveIntervalTuning,
    AdaptiveMetrics,
    ConsensusMetrics,
    CpuMetrics,
    LatencyMetrics,
    MemoryMetrics,
    MeshMetrics,
    MetricsCollectionStats,
    MobileMetrics,
    OptimizationStrategy,
    PerformanceMetrics,
    PerformanceOptimizer,
    ThermalState,
};
pub use soak_test::{
    MemoryAnalysis, PerformanceAnalysis, SoakTestConfig, SoakTestMonitor, SoakTestProgress,
    SoakTestResult, StabilityAnalysis,
};
