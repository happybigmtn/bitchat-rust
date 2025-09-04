#[cfg(feature = "profiling")]
pub mod cpu_profiler;
#[cfg(feature = "profiling")]
pub mod memory_profiler;
#[cfg(feature = "profiling")]
pub mod mobile_profiler;
#[cfg(feature = "profiling")]
pub mod network_profiler;

#[cfg(feature = "profiling")]
pub use cpu_profiler::{CpuMetrics, CpuProfile, CpuProfiler};
#[cfg(feature = "profiling")]
pub use memory_profiler::{AllocationTracker, MemoryMetrics, MemoryProfile, MemoryProfiler};
#[cfg(feature = "profiling")]
pub use mobile_profiler::{BatteryMetrics, MobileProfile, MobileProfiler, ThermalMetrics};
#[cfg(feature = "profiling")]
pub use network_profiler::{LatencyTracker, NetworkMetrics, NetworkProfile, NetworkProfiler};

use crate::error::BitCrapsError;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Comprehensive performance profiling system
#[cfg(feature = "profiling")]
pub struct PerformanceProfiler {
    cpu_profiler: CpuProfiler,
    memory_profiler: MemoryProfiler,
    network_profiler: NetworkProfiler,
    mobile_profiler: MobileProfiler,
    session_start: Instant,
    profiling_overhead: Arc<RwLock<Duration>>,
}

#[cfg(feature = "profiling")]
impl PerformanceProfiler {
    pub fn new() -> Result<Self, BitCrapsError> {
        Ok(Self {
            cpu_profiler: CpuProfiler::new()?,
            memory_profiler: MemoryProfiler::new()?,
            network_profiler: NetworkProfiler::new()?,
            mobile_profiler: MobileProfiler::new()?,
            session_start: Instant::now(),
            profiling_overhead: Arc::new(RwLock::new(Duration::from_nanos(0))),
        })
    }

    /// Start comprehensive profiling session
    pub async fn start_profiling(&mut self) -> Result<(), BitCrapsError> {
        let start_time = Instant::now();

        self.cpu_profiler.start().await?;
        self.memory_profiler.start().await?;
        self.network_profiler.start().await?;
        self.mobile_profiler.start().await?;

        // Track profiling overhead
        let overhead = start_time.elapsed();
        *self.profiling_overhead.write() += overhead;

        tracing::info!("Started comprehensive performance profiling");
        Ok(())
    }

    /// Stop profiling and generate comprehensive report
    pub async fn stop_profiling(&mut self) -> Result<ComprehensiveReport, BitCrapsError> {
        let start_time = Instant::now();

        let cpu_profile = self.cpu_profiler.stop().await?;
        let memory_profile = self.memory_profiler.stop().await?;
        let network_profile = self.network_profiler.stop().await?;
        let mobile_profile = self.mobile_profiler.stop().await?;

        // Track profiling overhead
        let overhead = start_time.elapsed();
        *self.profiling_overhead.write() += overhead;

        let total_overhead = *self.profiling_overhead.read();

        let recommendations = self.generate_recommendations(
            &cpu_profile,
            &memory_profile,
            &network_profile,
            &mobile_profile,
        );

        Ok(ComprehensiveReport {
            session_duration: self.session_start.elapsed(),
            profiling_overhead: total_overhead,
            cpu_profile,
            memory_profile,
            network_profile,
            mobile_profile,
            recommendations,
        })
    }

    /// Take a snapshot of current performance metrics
    pub async fn snapshot(&self) -> Result<PerformanceSnapshot, BitCrapsError> {
        let start_time = Instant::now();

        let cpu_metrics = self.cpu_profiler.current_metrics().await?;
        let memory_metrics = self.memory_profiler.current_metrics().await?;
        let network_metrics = self.network_profiler.current_metrics().await?;
        let mobile_metrics = self.mobile_profiler.current_metrics().await?;

        let overhead = start_time.elapsed();

        Ok(PerformanceSnapshot {
            timestamp: Instant::now(),
            profiling_overhead: overhead,
            cpu_metrics,
            memory_metrics,
            network_metrics,
            mobile_metrics,
        })
    }

    /// Generate optimization recommendations based on profiling data
    fn generate_recommendations(
        &self,
        cpu_profile: &CpuProfile,
        memory_profile: &MemoryProfile,
        network_profile: &NetworkProfile,
        mobile_profile: &MobileProfile,
    ) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // CPU recommendations
        if cpu_profile.average_usage > 80.0 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Cpu,
                priority: RecommendationPriority::High,
                title: "High CPU Usage Detected".to_string(),
                description: format!(
                    "CPU usage averaging {:.1}% is above recommended threshold of 80%",
                    cpu_profile.average_usage
                ),
                suggested_actions: vec![
                    "Enable CPU optimization profiles".to_string(),
                    "Reduce background task frequency".to_string(),
                    "Consider SIMD optimizations for hot paths".to_string(),
                ],
                estimated_impact: "15-30% CPU usage reduction".to_string(),
            });
        }

        if cpu_profile.thermal_throttling_detected {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Cpu,
                priority: RecommendationPriority::Critical,
                title: "Thermal Throttling Detected".to_string(),
                description: "CPU is being throttled due to high temperature".to_string(),
                suggested_actions: vec![
                    "Enable aggressive thermal management".to_string(),
                    "Reduce CPU frequency scaling".to_string(),
                    "Pause non-essential operations".to_string(),
                ],
                estimated_impact: "Prevent performance degradation".to_string(),
            });
        }

        // Memory recommendations
        if memory_profile.peak_usage_mb > 512 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Memory,
                priority: RecommendationPriority::Medium,
                title: "High Memory Usage".to_string(),
                description: format!(
                    "Peak memory usage of {} MB may cause issues on low-memory devices",
                    memory_profile.peak_usage_mb
                ),
                suggested_actions: vec![
                    "Enable memory pooling for frequent allocations".to_string(),
                    "Implement more aggressive caching eviction".to_string(),
                    "Use memory-mapped files for large data".to_string(),
                ],
                estimated_impact: "20-40% memory usage reduction".to_string(),
            });
        }

        if memory_profile.allocation_rate > 10_000_000.0 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Memory,
                priority: RecommendationPriority::High,
                title: "High Allocation Rate".to_string(),
                description: format!(
                    "Allocation rate of {:.1}M allocations/sec may cause GC pressure",
                    memory_profile.allocation_rate as f64 / 1_000_000.0
                ),
                suggested_actions: vec![
                    "Implement object pooling".to_string(),
                    "Use zero-copy message passing".to_string(),
                    "Pre-allocate frequently used data structures".to_string(),
                ],
                estimated_impact: "50-70% allocation reduction".to_string(),
            });
        }

        // Network recommendations
        if network_profile.average_latency > Duration::from_millis(200) {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Network,
                priority: RecommendationPriority::Medium,
                title: "High Network Latency".to_string(),
                description: format!(
                    "Average latency of {:?} may impact user experience",
                    network_profile.average_latency
                ),
                suggested_actions: vec![
                    "Enable message batching".to_string(),
                    "Implement connection pooling".to_string(),
                    "Use compression for large messages".to_string(),
                ],
                estimated_impact: "30-50% latency reduction".to_string(),
            });
        }

        if network_profile.packet_loss_rate > 0.05 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Network,
                priority: RecommendationPriority::High,
                title: "High Packet Loss".to_string(),
                description: format!(
                    "Packet loss rate of {:.1}% indicates network issues",
                    network_profile.packet_loss_rate * 100.0
                ),
                suggested_actions: vec![
                    "Implement adaptive retry mechanisms".to_string(),
                    "Enable forward error correction".to_string(),
                    "Use redundant connections".to_string(),
                ],
                estimated_impact: "Improved connection reliability".to_string(),
            });
        }

        // Mobile-specific recommendations
        if mobile_profile.battery_drain_rate > 5.0 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Mobile,
                priority: RecommendationPriority::High,
                title: "High Battery Drain".to_string(),
                description: format!(
                    "Battery drain rate of {:.1}%/hour is above acceptable levels",
                    mobile_profile.battery_drain_rate
                ),
                suggested_actions: vec![
                    "Enable power-saving profiles".to_string(),
                    "Reduce BLE advertising frequency".to_string(),
                    "Optimize background task scheduling".to_string(),
                ],
                estimated_impact: "40-60% battery life improvement".to_string(),
            });
        }

        if mobile_profile.thermal_events > 0 {
            recommendations.push(OptimizationRecommendation {
                category: RecommendationCategory::Mobile,
                priority: RecommendationPriority::Critical,
                title: "Thermal Events Detected".to_string(),
                description: format!(
                    "{} thermal events may lead to forced shutdowns",
                    mobile_profile.thermal_events
                ),
                suggested_actions: vec![
                    "Enable emergency thermal management".to_string(),
                    "Reduce computational intensity".to_string(),
                    "Pause intensive operations when overheating".to_string(),
                ],
                estimated_impact: "Prevent thermal shutdowns".to_string(),
            });
        }

        recommendations
    }

    /// Get current profiling overhead
    pub fn get_profiling_overhead(&self) -> Duration {
        *self.profiling_overhead.read()
    }
}

/// Comprehensive performance report
#[cfg(feature = "profiling")]
#[derive(Debug, Clone)]
pub struct ComprehensiveReport {
    pub session_duration: Duration,
    pub profiling_overhead: Duration,
    pub cpu_profile: CpuProfile,
    pub memory_profile: MemoryProfile,
    pub network_profile: NetworkProfile,
    pub mobile_profile: MobileProfile,
    pub recommendations: Vec<OptimizationRecommendation>,
}

#[cfg(feature = "profiling")]
impl ComprehensiveReport {
    /// Generate a summary score (0-100) for overall performance
    pub fn performance_score(&self) -> u8 {
        let mut score: f64 = 100.0;

        // CPU score (25% weight)
        let cpu_score = if self.cpu_profile.average_usage > 90.0 {
            0.0
        } else if self.cpu_profile.average_usage > 80.0 {
            50.0
        } else if self.cpu_profile.average_usage > 60.0 {
            75.0
        } else {
            100.0
        };
        score = score - (25.0 * (100.0 - cpu_score) / 100.0);

        // Memory score (25% weight)
        let memory_score = if self.memory_profile.peak_usage_mb > 1024 {
            0.0
        } else if self.memory_profile.peak_usage_mb > 512 {
            50.0
        } else if self.memory_profile.peak_usage_mb > 256 {
            75.0
        } else {
            100.0
        };
        score = score - (25.0 * (100.0 - memory_score) / 100.0);

        // Network score (25% weight)
        let network_score = if self.network_profile.average_latency > Duration::from_millis(500) {
            0.0
        } else if self.network_profile.average_latency > Duration::from_millis(200) {
            50.0
        } else if self.network_profile.average_latency > Duration::from_millis(100) {
            75.0
        } else {
            100.0
        };
        score = score - (25.0 * (100.0 - network_score) / 100.0);

        // Mobile score (25% weight)
        let mobile_score = if self.mobile_profile.battery_drain_rate > 10.0 {
            0.0
        } else if self.mobile_profile.battery_drain_rate > 5.0 {
            50.0
        } else if self.mobile_profile.battery_drain_rate > 2.0 {
            75.0
        } else {
            100.0
        };
        score = score - (25.0 * (100.0 - mobile_score) / 100.0);

        score.max(0.0).min(100.0) as u8
    }

    /// Get critical recommendations that need immediate attention
    pub fn critical_recommendations(&self) -> Vec<&OptimizationRecommendation> {
        self.recommendations
            .iter()
            .filter(|r| r.priority == RecommendationPriority::Critical)
            .collect()
    }
}

/// Point-in-time performance snapshot
#[cfg(feature = "profiling")]
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub timestamp: Instant,
    pub profiling_overhead: Duration,
    pub cpu_metrics: CpuMetrics,
    pub memory_metrics: MemoryMetrics,
    pub network_metrics: NetworkMetrics,
    pub mobile_metrics: BatteryMetrics,
}

/// Optimization recommendation based on profiling data
#[cfg(feature = "profiling")]
#[derive(Debug, Clone)]
pub struct OptimizationRecommendation {
    pub category: RecommendationCategory,
    pub priority: RecommendationPriority,
    pub title: String,
    pub description: String,
    pub suggested_actions: Vec<String>,
    pub estimated_impact: String,
}

#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationCategory {
    Cpu,
    Memory,
    Network,
    Mobile,
    Integration,
}

#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}
