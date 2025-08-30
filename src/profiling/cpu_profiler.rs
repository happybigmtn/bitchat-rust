use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{CpuExt, System, SystemExt};
use tokio::time::interval;

use crate::error::BitCrapsError;

/// CPU performance profiler with thermal monitoring
pub struct CpuProfiler {
    system: Arc<RwLock<System>>,
    metrics: Arc<RwLock<CpuMetrics>>,
    profiling_active: Arc<RwLock<bool>>,
    sample_interval: Duration,
    hotspot_tracker: HotspotTracker,
}

impl CpuProfiler {
    pub fn new() -> Result<Self, BitCrapsError> {
        let mut system = System::new();
        system.refresh_cpu();

        Ok(Self {
            system: Arc::new(RwLock::new(system)),
            metrics: Arc::new(RwLock::new(CpuMetrics::new())),
            profiling_active: Arc::new(RwLock::new(false)),
            sample_interval: Duration::from_millis(100), // 10 samples per second
            hotspot_tracker: HotspotTracker::new(),
        })
    }

    /// Start CPU profiling
    pub async fn start(&mut self) -> Result<(), BitCrapsError> {
        *self.profiling_active.write() = true;

        // Spawn background sampling task
        let system = Arc::clone(&self.system);
        let metrics = Arc::clone(&self.metrics);
        let profiling_active = Arc::clone(&self.profiling_active);
        let sample_interval = self.sample_interval;

        tokio::spawn(async move {
            let mut interval = interval(sample_interval);

            while *profiling_active.read() {
                interval.tick().await;

                // Refresh system information
                {
                    let mut sys = system.write();
                    sys.refresh_cpu();
                }

                // Collect CPU metrics
                let cpu_usage = {
                    let sys = system.read();
                    sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
                        / sys.cpus().len() as f32
                };

                // Update metrics
                {
                    let mut metrics = metrics.write();
                    metrics.add_sample(cpu_usage, Instant::now());
                }
            }
        });

        tracing::debug!(
            "CPU profiling started with {}ms sample interval",
            sample_interval.as_millis()
        );
        Ok(())
    }

    /// Stop CPU profiling and return profile
    pub async fn stop(&mut self) -> Result<CpuProfile, BitCrapsError> {
        *self.profiling_active.write() = false;

        // Wait a bit for final samples
        tokio::time::sleep(Duration::from_millis(200)).await;

        let metrics = self.metrics.read().clone();
        let hotspots = self.hotspot_tracker.get_hotspots();

        // Reset for next session
        {
            let mut metrics_guard = self.metrics.write();
            *metrics_guard = CpuMetrics::new();
        }
        self.hotspot_tracker.reset();

        Ok(CpuProfile {
            total_samples: metrics.samples.len(),
            average_usage: metrics.average_usage(),
            peak_usage: metrics.peak_usage(),
            usage_distribution: metrics.usage_distribution(),
            thermal_throttling_detected: self.detect_thermal_throttling(&metrics),
            hotspots,
            profiling_duration: metrics.profiling_duration(),
        })
    }

    /// Get current CPU metrics without stopping profiling
    pub async fn current_metrics(&self) -> Result<CpuMetrics, BitCrapsError> {
        Ok(self.metrics.read().clone())
    }

    /// Profile a specific function or code block
    pub async fn profile_function<F, R>(
        &mut self,
        name: &str,
        func: F,
    ) -> Result<(R, FunctionProfile), BitCrapsError>
    where
        F: std::future::Future<Output = R>,
    {
        let start_time = Instant::now();
        let start_cpu = self.get_current_cpu_usage().await?;

        let result = func.await;

        let end_time = Instant::now();
        let end_cpu = self.get_current_cpu_usage().await?;

        let profile = FunctionProfile {
            name: name.to_string(),
            duration: end_time - start_time,
            cpu_usage_before: start_cpu,
            cpu_usage_after: end_cpu,
            cpu_usage_delta: end_cpu - start_cpu,
        };

        // Track as potential hotspot
        self.hotspot_tracker.record_function_call(&profile);

        Ok((result, profile))
    }

    /// Detect if thermal throttling is occurring
    fn detect_thermal_throttling(&self, metrics: &CpuMetrics) -> bool {
        // Look for patterns indicating thermal throttling:
        // 1. Sudden drops in CPU usage despite high load
        // 2. Oscillating usage patterns
        // 3. Usage capping below maximum

        if metrics.samples.len() < 10 {
            return false;
        }

        // Check for sudden drops (>20% drop in usage)
        let mut throttling_events = 0;
        for window in metrics.samples.windows(2) {
            if let [prev, curr] = window {
                if prev.usage > 80.0 && curr.usage < prev.usage - 20.0 {
                    throttling_events += 1;
                }
            }
        }

        // If we see multiple throttling events, likely thermal throttling
        throttling_events > 3
    }

    /// Get current CPU usage
    async fn get_current_cpu_usage(&self) -> Result<f32, BitCrapsError> {
        {
            let mut system = self.system.write();
            system.refresh_cpu();
        }

        let usage = {
            let system = self.system.read();
            system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
                / system.cpus().len() as f32
        };

        Ok(usage)
    }
}

/// CPU performance metrics collected during profiling
#[derive(Debug, Clone)]
pub struct CpuMetrics {
    pub samples: Vec<CpuSample>,
    pub start_time: Option<Instant>,
}

impl CpuMetrics {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            start_time: None,
        }
    }

    pub fn add_sample(&mut self, usage: f32, timestamp: Instant) {
        if self.start_time.is_none() {
            self.start_time = Some(timestamp);
        }

        self.samples.push(CpuSample { usage, timestamp });

        // Keep only last 10000 samples to prevent unbounded growth
        if self.samples.len() > 10000 {
            self.samples.remove(0);
        }
    }

    pub fn average_usage(&self) -> f32 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.samples.iter().map(|s| s.usage).sum::<f32>() / self.samples.len() as f32
        }
    }

    pub fn peak_usage(&self) -> f32 {
        self.samples.iter().map(|s| s.usage).fold(0.0, f32::max)
    }

    pub fn usage_distribution(&self) -> UsageDistribution {
        if self.samples.is_empty() {
            return UsageDistribution::default();
        }

        let mut low = 0;
        let mut medium = 0;
        let mut high = 0;
        let mut critical = 0;

        for sample in &self.samples {
            match sample.usage {
                usage if usage < 25.0 => low += 1,
                usage if usage < 50.0 => medium += 1,
                usage if usage < 80.0 => high += 1,
                _ => critical += 1,
            }
        }

        let total = self.samples.len() as f32;
        UsageDistribution {
            low_usage_percent: (low as f32 / total) * 100.0,
            medium_usage_percent: (medium as f32 / total) * 100.0,
            high_usage_percent: (high as f32 / total) * 100.0,
            critical_usage_percent: (critical as f32 / total) * 100.0,
        }
    }

    pub fn profiling_duration(&self) -> Duration {
        if let (Some(start), Some(last)) = (self.start_time, self.samples.last()) {
            last.timestamp - start
        } else {
            Duration::from_nanos(0)
        }
    }
}

/// Individual CPU usage sample
#[derive(Debug, Clone)]
pub struct CpuSample {
    pub usage: f32,
    pub timestamp: Instant,
}

/// CPU usage distribution across different levels
#[derive(Debug, Clone, Default)]
pub struct UsageDistribution {
    pub low_usage_percent: f32,      // 0-25%
    pub medium_usage_percent: f32,   // 25-50%
    pub high_usage_percent: f32,     // 50-80%
    pub critical_usage_percent: f32, // 80-100%
}

/// Complete CPU profiling results
#[derive(Debug, Clone)]
pub struct CpuProfile {
    pub total_samples: usize,
    pub average_usage: f32,
    pub peak_usage: f32,
    pub usage_distribution: UsageDistribution,
    pub thermal_throttling_detected: bool,
    pub hotspots: Vec<FunctionHotspot>,
    pub profiling_duration: Duration,
}

/// Profile of a specific function call
#[derive(Debug, Clone)]
pub struct FunctionProfile {
    pub name: String,
    pub duration: Duration,
    pub cpu_usage_before: f32,
    pub cpu_usage_after: f32,
    pub cpu_usage_delta: f32,
}

/// Hotspot detection for performance-critical functions
pub struct HotspotTracker {
    function_calls: FxHashMap<String, FunctionStats>,
}

impl HotspotTracker {
    pub fn new() -> Self {
        Self {
            function_calls: FxHashMap::default(),
        }
    }

    pub fn record_function_call(&mut self, profile: &FunctionProfile) {
        let stats = self
            .function_calls
            .entry(profile.name.clone())
            .or_insert_with(FunctionStats::new);

        stats.call_count += 1;
        stats.total_duration += profile.duration;
        stats.total_cpu_usage += profile.cpu_usage_delta;
        stats.peak_duration = stats.peak_duration.max(profile.duration);
        stats.peak_cpu_usage = stats.peak_cpu_usage.max(profile.cpu_usage_delta);
    }

    pub fn get_hotspots(&self) -> Vec<FunctionHotspot> {
        let mut hotspots: Vec<_> = self
            .function_calls
            .iter()
            .map(|(name, stats)| FunctionHotspot {
                function_name: name.clone(),
                call_count: stats.call_count,
                total_duration: stats.total_duration,
                average_duration: if stats.call_count > 0 {
                    stats.total_duration / stats.call_count as u32
                } else {
                    Duration::from_nanos(0)
                },
                peak_duration: stats.peak_duration,
                average_cpu_usage: if stats.call_count > 0 {
                    stats.total_cpu_usage / stats.call_count as f32
                } else {
                    0.0
                },
                peak_cpu_usage: stats.peak_cpu_usage,
                hotspot_score: stats.calculate_hotspot_score(),
            })
            .collect();

        // Sort by hotspot score (highest first)
        hotspots.sort_by(|a, b| {
            b.hotspot_score
                .partial_cmp(&a.hotspot_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return top 20 hotspots
        hotspots.truncate(20);
        hotspots
    }

    pub fn reset(&mut self) {
        self.function_calls.clear();
    }
}

#[derive(Debug, Clone)]
struct FunctionStats {
    call_count: u64,
    total_duration: Duration,
    total_cpu_usage: f32,
    peak_duration: Duration,
    peak_cpu_usage: f32,
}

impl FunctionStats {
    fn new() -> Self {
        Self {
            call_count: 0,
            total_duration: Duration::from_nanos(0),
            total_cpu_usage: 0.0,
            peak_duration: Duration::from_nanos(0),
            peak_cpu_usage: 0.0,
        }
    }

    fn calculate_hotspot_score(&self) -> f64 {
        if self.call_count == 0 {
            return 0.0;
        }

        let avg_duration_ms = self.total_duration.as_millis() as f64 / self.call_count as f64;
        let avg_cpu_usage = self.total_cpu_usage / self.call_count as f32;
        let call_frequency = self.call_count as f64;

        // Hotspot score combines duration, CPU usage, and frequency
        (avg_duration_ms * avg_cpu_usage as f64 * call_frequency.sqrt()) / 1000.0
    }
}

/// Function identified as a performance hotspot
#[derive(Debug, Clone)]
pub struct FunctionHotspot {
    pub function_name: String,
    pub call_count: u64,
    pub total_duration: Duration,
    pub average_duration: Duration,
    pub peak_duration: Duration,
    pub average_cpu_usage: f32,
    pub peak_cpu_usage: f32,
    pub hotspot_score: f64,
}
