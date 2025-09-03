//! 8-Hour Soak Test Monitoring for M8 Performance Requirements
//!
//! This module implements continuous monitoring and validation for long-running
//! performance tests to ensure system stability and resource management.

use crate::performance::{AdaptiveMetrics, PerformanceMetrics, PerformanceOptimizer};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Configuration for 8-hour soak test
#[derive(Debug, Clone)]
pub struct SoakTestConfig {
    /// Total test duration (default: 8 hours)
    pub duration: Duration,
    /// Sample interval for metrics collection
    pub sample_interval: Duration,
    /// Memory leak detection threshold (MB growth per hour)
    pub memory_leak_threshold_mb_per_hour: f64,
    /// CPU utilization warning threshold
    pub cpu_warning_threshold: f64,
    /// Network latency failure threshold (p95)
    pub latency_failure_threshold_ms: f64,
    /// Maximum allowed system downtime during test
    pub max_downtime_seconds: u64,
    /// Consensus throughput minimum requirement
    pub min_consensus_throughput_ops_per_sec: f64,
}

impl Default for SoakTestConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(8 * 60 * 60), // 8 hours
            sample_interval: Duration::from_secs(30),   // 30 seconds
            memory_leak_threshold_mb_per_hour: 50.0,    // 50MB/hour leak threshold
            cpu_warning_threshold: 80.0,                // 80% CPU warning
            latency_failure_threshold_ms: 500.0,        // 500ms p95 failure
            max_downtime_seconds: 300,                  // 5 minutes max downtime
            min_consensus_throughput_ops_per_sec: 50.0, // 50 ops/sec minimum
        }
    }
}

/// Results from 8-hour soak test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoakTestResult {
    pub start_time: SystemTime,
    pub duration: Duration,
    pub total_samples: usize,
    pub memory_analysis: MemoryAnalysis,
    pub performance_analysis: PerformanceAnalysis,
    pub stability_analysis: StabilityAnalysis,
    pub pass: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAnalysis {
    pub initial_memory_mb: f64,
    pub final_memory_mb: f64,
    pub peak_memory_mb: f64,
    pub memory_growth_rate_mb_per_hour: f64,
    pub suspected_leak: bool,
    pub gc_efficiency: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub average_cpu_utilization: f64,
    pub peak_cpu_utilization: f64,
    pub average_p95_latency_ms: f64,
    pub peak_p95_latency_ms: f64,
    pub consensus_throughput_avg: f64,
    pub consensus_throughput_min: f64,
    pub adaptive_interval_effectiveness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StabilityAnalysis {
    pub total_downtime_seconds: u64,
    pub crash_count: u32,
    pub error_count: u64,
    pub consensus_failures: u64,
    pub network_partitions: u32,
    pub recovery_time_avg_seconds: f64,
}

/// 8-hour soak test monitor
pub struct SoakTestMonitor {
    config: SoakTestConfig,
    performance_optimizer: Arc<PerformanceOptimizer>,
    is_running: Arc<AtomicBool>,
    start_time: Arc<RwLock<Option<Instant>>>,
    samples: Arc<RwLock<VecDeque<SoakTestSample>>>,
    error_count: Arc<AtomicU64>,
    downtime_tracker: Arc<RwLock<DowntimeTracker>>,
}

#[derive(Debug, Clone)]
struct SoakTestSample {
    timestamp: Instant,
    metrics: PerformanceMetrics,
    adaptive_metrics: AdaptiveMetrics,
}

#[derive(Debug, Clone)]
struct DowntimeTracker {
    current_downtime_start: Option<Instant>,
    total_downtime: Duration,
    downtime_events: Vec<(Instant, Duration)>,
}

impl SoakTestMonitor {
    pub fn new(config: SoakTestConfig, performance_optimizer: Arc<PerformanceOptimizer>) -> Self {
        Self {
            config,
            performance_optimizer,
            is_running: Arc::new(AtomicBool::new(false)),
            start_time: Arc::new(RwLock::new(None)),
            samples: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            error_count: Arc::new(AtomicU64::new(0)),
            downtime_tracker: Arc::new(RwLock::new(DowntimeTracker {
                current_downtime_start: None,
                total_downtime: Duration::ZERO,
                downtime_events: Vec::new(),
            })),
        }
    }

    /// Start the 8-hour soak test
    pub async fn start_test(&self) -> Result<SoakTestResult, String> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err("Soak test is already running".to_string());
        }

        self.is_running.store(true, Ordering::Relaxed);
        *self.start_time.write().await = Some(Instant::now());
        self.samples.write().await.clear();
        self.error_count.store(0, Ordering::Relaxed);

        info!("Starting 8-hour soak test with config: {:?}", self.config);

        // Start monitoring task
        let monitor_handle = self.start_monitoring_task().await;

        // Wait for test completion or manual stop
        let test_start = Instant::now();
        while self.is_running.load(Ordering::Relaxed) && test_start.elapsed() < self.config.duration
        {
            sleep(Duration::from_secs(60)).await; // Check every minute

            // Periodic health checks
            if let Err(e) = self.perform_health_check().await {
                error!("Health check failed: {}", e);
                self.error_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Stop monitoring and analyze results
        self.is_running.store(false, Ordering::Relaxed);
        monitor_handle.abort();

        let result = self.analyze_results().await;

        info!("8-hour soak test completed. Pass: {}", result.pass);
        if !result.pass {
            warn!("Soak test failures: {:?}", result.failure_reasons);
        }

        Ok(result)
    }

    /// Start the background monitoring task
    async fn start_monitoring_task(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let optimizer = Arc::clone(&self.performance_optimizer);
        let is_running = Arc::clone(&self.is_running);
        let samples = Arc::clone(&self.samples);
        let error_count = Arc::clone(&self.error_count);
        let downtime_tracker = Arc::clone(&self.downtime_tracker);

        tokio::spawn(async move {
            let mut sample_interval = tokio::time::interval(config.sample_interval);

            while is_running.load(Ordering::Relaxed) {
                sample_interval.tick().await;

                // Collect performance metrics
                match Self::collect_sample(&optimizer).await {
                    Ok(sample) => {
                        // Store sample
                        let mut samples_guard = samples.write().await;
                        samples_guard.push_back(sample.clone());

                        // Keep only last 1000 samples to manage memory
                        if samples_guard.len() > 1000 {
                            samples_guard.pop_front();
                        }

                        // Update downtime tracking
                        Self::update_downtime_tracking(&downtime_tracker, &sample).await;

                        // Real-time alerts
                        Self::check_real_time_alerts(&config, &sample, &error_count).await;
                    }
                    Err(e) => {
                        error!("Failed to collect soak test sample: {}", e);
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        })
    }

    async fn collect_sample(
        optimizer: &Arc<PerformanceOptimizer>,
    ) -> Result<SoakTestSample, String> {
        let metrics = optimizer.get_metrics().await;
        let adaptive_metrics = optimizer.get_adaptive_metrics().await;

        Ok(SoakTestSample {
            timestamp: Instant::now(),
            metrics,
            adaptive_metrics,
        })
    }

    async fn update_downtime_tracking(
        downtime_tracker: &Arc<RwLock<DowntimeTracker>>,
        sample: &SoakTestSample,
    ) {
        let mut tracker = downtime_tracker.write().await;

        // Simple heuristic: high CPU + high latency = potential downtime
        let is_system_down = sample.metrics.cpu_usage.utilization_percent > 95.0
            && sample.metrics.network_latency.p95_ms > 2000.0;

        match (tracker.current_downtime_start, is_system_down) {
            (None, true) => {
                // Downtime started
                tracker.current_downtime_start = Some(sample.timestamp);
            }
            (Some(start), false) => {
                // Downtime ended
                let downtime_duration = sample.timestamp - start;
                tracker.total_downtime += downtime_duration;
                tracker.downtime_events.push((start, downtime_duration));
                tracker.current_downtime_start = None;

                info!("System recovered from downtime: {:?}", downtime_duration);
            }
            _ => {
                // No state change
            }
        }
    }

    async fn check_real_time_alerts(
        config: &SoakTestConfig,
        sample: &SoakTestSample,
        error_count: &Arc<AtomicU64>,
    ) {
        // CPU utilization alert
        if sample.metrics.cpu_usage.utilization_percent > config.cpu_warning_threshold {
            warn!(
                "High CPU utilization detected: {:.1}%",
                sample.metrics.cpu_usage.utilization_percent
            );
        }

        // Latency alert
        if sample.metrics.network_latency.p95_ms > config.latency_failure_threshold_ms {
            warn!(
                "High latency detected: p95 = {:.1}ms",
                sample.metrics.network_latency.p95_ms
            );
            error_count.fetch_add(1, Ordering::Relaxed);
        }

        // Consensus throughput alert
        if sample.metrics.consensus_performance.throughput_ops_per_sec
            < config.min_consensus_throughput_ops_per_sec
        {
            warn!(
                "Low consensus throughput: {:.1} ops/sec",
                sample.metrics.consensus_performance.throughput_ops_per_sec
            );
        }
    }

    async fn perform_health_check(&self) -> Result<(), String> {
        // Basic health checks
        let metrics = self.performance_optimizer.get_metrics().await;

        // Check for memory leaks (simplified)
        if metrics.memory_usage.heap_used_mb > 4096.0 {
            // 4GB warning
            return Err("Memory usage exceeds warning threshold".to_string());
        }

        // Check system responsiveness
        let start = Instant::now();
        self.performance_optimizer.get_adaptive_metrics().await;
        let response_time = start.elapsed();

        if response_time > Duration::from_millis(1000) {
            return Err(format!(
                "System response time too slow: {:?}",
                response_time
            ));
        }

        Ok(())
    }

    async fn analyze_results(&self) -> SoakTestResult {
        let samples = self.samples.read().await;
        let downtime_tracker = self.downtime_tracker.read().await;
        let error_count = self.error_count.load(Ordering::Relaxed);

        if samples.is_empty() {
            return SoakTestResult {
                start_time: SystemTime::now(),
                duration: Duration::ZERO,
                total_samples: 0,
                memory_analysis: MemoryAnalysis {
                    initial_memory_mb: 0.0,
                    final_memory_mb: 0.0,
                    peak_memory_mb: 0.0,
                    memory_growth_rate_mb_per_hour: 0.0,
                    suspected_leak: false,
                    gc_efficiency: 0.0,
                },
                performance_analysis: PerformanceAnalysis {
                    average_cpu_utilization: 0.0,
                    peak_cpu_utilization: 0.0,
                    average_p95_latency_ms: 0.0,
                    peak_p95_latency_ms: 0.0,
                    consensus_throughput_avg: 0.0,
                    consensus_throughput_min: 0.0,
                    adaptive_interval_effectiveness: 0.0,
                },
                stability_analysis: StabilityAnalysis {
                    total_downtime_seconds: 0,
                    crash_count: 0,
                    error_count: 0,
                    consensus_failures: 0,
                    network_partitions: 0,
                    recovery_time_avg_seconds: 0.0,
                },
                pass: false,
                failure_reasons: vec!["No samples collected".to_string()],
            };
        }

        let total_samples = samples.len();
        let test_duration = samples.back().unwrap().timestamp - samples.front().unwrap().timestamp;

        // Memory analysis
        let initial_memory = samples.front().unwrap().metrics.memory_usage.heap_used_mb;
        let final_memory = samples.back().unwrap().metrics.memory_usage.heap_used_mb;
        let peak_memory = samples
            .iter()
            .map(|s| s.metrics.memory_usage.heap_used_mb)
            .fold(0.0, f64::max);

        let hours = test_duration.as_secs_f64() / 3600.0;
        let memory_growth_rate = (final_memory - initial_memory) / hours;
        let suspected_leak = memory_growth_rate > self.config.memory_leak_threshold_mb_per_hour;

        // Performance analysis
        let avg_cpu = samples
            .iter()
            .map(|s| s.metrics.cpu_usage.utilization_percent)
            .sum::<f64>()
            / total_samples as f64;
        let peak_cpu = samples
            .iter()
            .map(|s| s.metrics.cpu_usage.utilization_percent)
            .fold(0.0, f64::max);

        let avg_latency = samples
            .iter()
            .map(|s| s.metrics.network_latency.p95_ms)
            .sum::<f64>()
            / total_samples as f64;
        let peak_latency = samples
            .iter()
            .map(|s| s.metrics.network_latency.p95_ms)
            .fold(0.0, f64::max);

        let avg_throughput = samples
            .iter()
            .map(|s| s.metrics.consensus_performance.throughput_ops_per_sec)
            .sum::<f64>()
            / total_samples as f64;
        let min_throughput = samples
            .iter()
            .map(|s| s.metrics.consensus_performance.throughput_ops_per_sec)
            .fold(f64::INFINITY, f64::min);

        // Adaptive interval effectiveness
        let avg_efficiency = samples
            .iter()
            .map(|s| s.adaptive_metrics.efficiency_score)
            .sum::<f64>()
            / total_samples as f64;

        // Stability analysis
        let total_downtime = downtime_tracker.total_downtime.as_secs();
        let recovery_times: Vec<f64> = downtime_tracker
            .downtime_events
            .iter()
            .map(|(_, duration)| duration.as_secs_f64())
            .collect();
        let avg_recovery = if recovery_times.is_empty() {
            0.0
        } else {
            recovery_times.iter().sum::<f64>() / recovery_times.len() as f64
        };

        // Determine if test passes
        let mut failure_reasons = Vec::new();

        if suspected_leak {
            failure_reasons.push(format!(
                "Memory leak detected: {:.2} MB/hour growth",
                memory_growth_rate
            ));
        }

        if total_downtime > self.config.max_downtime_seconds {
            failure_reasons.push(format!(
                "Excessive downtime: {}s > {}s limit",
                total_downtime, self.config.max_downtime_seconds
            ));
        }

        if avg_latency > self.config.latency_failure_threshold_ms {
            failure_reasons.push(format!("High average latency: {:.1}ms", avg_latency));
        }

        if min_throughput < self.config.min_consensus_throughput_ops_per_sec {
            failure_reasons.push(format!(
                "Low consensus throughput: {:.1} ops/sec",
                min_throughput
            ));
        }

        let pass = failure_reasons.is_empty();

        SoakTestResult {
            start_time: SystemTime::now() - test_duration,
            duration: test_duration,
            total_samples,
            memory_analysis: MemoryAnalysis {
                initial_memory_mb: initial_memory,
                final_memory_mb: final_memory,
                peak_memory_mb: peak_memory,
                memory_growth_rate_mb_per_hour: memory_growth_rate,
                suspected_leak,
                gc_efficiency: 0.95, // Placeholder
            },
            performance_analysis: PerformanceAnalysis {
                average_cpu_utilization: avg_cpu,
                peak_cpu_utilization: peak_cpu,
                average_p95_latency_ms: avg_latency,
                peak_p95_latency_ms: peak_latency,
                consensus_throughput_avg: avg_throughput,
                consensus_throughput_min: min_throughput,
                adaptive_interval_effectiveness: avg_efficiency,
            },
            stability_analysis: StabilityAnalysis {
                total_downtime_seconds: total_downtime,
                crash_count: 0, // Would need crash detection
                error_count,
                consensus_failures: samples
                    .iter()
                    .map(|s| s.metrics.consensus_performance.consensus_failures)
                    .max()
                    .unwrap_or(0),
                network_partitions: 0, // Would need partition detection
                recovery_time_avg_seconds: avg_recovery,
            },
            pass,
            failure_reasons,
        }
    }

    /// Stop the soak test early
    pub async fn stop_test(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        info!("Soak test stopped by user request");
    }

    /// Get current test progress
    pub async fn get_progress(&self) -> Option<SoakTestProgress> {
        if !self.is_running.load(Ordering::Relaxed) {
            return None;
        }

        let start_time = self.start_time.read().await;
        if let Some(start) = *start_time {
            let elapsed = start.elapsed();
            let progress =
                (elapsed.as_secs_f64() / self.config.duration.as_secs_f64() * 100.0).min(100.0);

            Some(SoakTestProgress {
                elapsed,
                total_duration: self.config.duration,
                progress_percent: progress,
                samples_collected: self.samples.read().await.len(),
                errors_detected: self.error_count.load(Ordering::Relaxed),
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct SoakTestProgress {
    pub elapsed: Duration,
    pub total_duration: Duration,
    pub progress_percent: f64,
    pub samples_collected: usize,
    pub errors_detected: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance::PerformanceOptimizer;

    #[tokio::test]
    async fn test_soak_test_monitor_creation() {
        let config = SoakTestConfig {
            duration: Duration::from_secs(60), // 1 minute for testing
            ..Default::default()
        };

        let optimizer = Arc::new(PerformanceOptimizer::new());
        let monitor = SoakTestMonitor::new(config, optimizer);

        assert!(!monitor.is_running.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_soak_test_progress_tracking() {
        let config = SoakTestConfig {
            duration: Duration::from_secs(10),
            sample_interval: Duration::from_millis(100),
            ..Default::default()
        };

        let optimizer = Arc::new(PerformanceOptimizer::new());
        let monitor = SoakTestMonitor::new(config, optimizer);

        // Start monitoring in background
        monitor.is_running.store(true, Ordering::Relaxed);
        *monitor.start_time.write().await = Some(Instant::now());

        // Check progress
        tokio::time::sleep(Duration::from_millis(50)).await;
        let progress = monitor.get_progress().await;
        assert!(progress.is_some());

        let progress = progress.unwrap();
        assert!(progress.progress_percent >= 0.0);
        assert!(progress.progress_percent <= 100.0);

        monitor.stop_test().await;
    }
}
