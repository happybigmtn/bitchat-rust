//! Load Testing Module for BitCraps Production Hardening
//! 
//! This module provides comprehensive load testing capabilities for validating
//! the BitCraps platform under various load conditions:
//! 
//! - Baseline load testing (normal operations)
//! - Peak load testing (expected maximum traffic)
//! - Stress testing (beyond normal capacity)
//! - Endurance testing (sustained load over time)
//! - Spike testing (sudden traffic increases)
//!
//! Key Features:
//! - Support for 1000+ concurrent users
//! - Real-time performance monitoring
//! - Resource usage tracking
//! - Comprehensive reporting
//! - Automated pass/fail criteria

pub mod load_test_framework;
pub mod orchestrate_load_tests;

pub use load_test_framework::{
    LoadTestOrchestrator,
    LoadTestConfig, 
    LoadTestResults,
    LoadTestError,
    VirtualUser,
    ResourceLimits,
    ResourceMonitor,
    UserOperation,
};

pub use orchestrate_load_tests::{
    LoadTestSuite,
};

// Re-export common types for convenience
pub use load_test_framework::{
    LoadTestMetrics,
    ResourceUsage,
    VirtualUserError,
};