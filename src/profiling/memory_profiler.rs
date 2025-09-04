use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(feature = "monitoring")]
use sysinfo::{System, SystemExt};

use crate::error::BitCrapsError;

/// Memory profiler with allocation tracking and leak detection
pub struct MemoryProfiler {
    #[cfg(feature = "monitoring")]
    system: Arc<RwLock<System>>,
    metrics: Arc<RwLock<MemoryMetrics>>,
    allocation_tracker: Arc<RwLock<AllocationTracker>>,
    profiling_active: Arc<RwLock<bool>>,
    sample_interval: Duration,
}

impl MemoryProfiler {
    pub fn new() -> Result<Self, BitCrapsError> {
        #[cfg(feature = "monitoring")]
        let mut system = System::new();
        #[cfg(feature = "monitoring")]
        system.refresh_memory();

        Ok(Self {
            #[cfg(feature = "monitoring")]
            system: Arc::new(RwLock::new(system)),
            metrics: Arc::new(RwLock::new(MemoryMetrics::new())),
            allocation_tracker: Arc::new(RwLock::new(AllocationTracker::new())),
            profiling_active: Arc::new(RwLock::new(false)),
            sample_interval: Duration::from_millis(250), // 4 samples per second
        })
    }

    pub async fn start(&mut self) -> Result<(), BitCrapsError> {
        *self.profiling_active.write() = true;

        let system = Arc::clone(&self.system);
        let metrics = Arc::clone(&self.metrics);
        let profiling_active = Arc::clone(&self.profiling_active);
        let sample_interval = self.sample_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(sample_interval);

            while *profiling_active.read() {
                interval.tick().await;

                // Refresh memory information
                {
                    let mut sys = system.write();
                    sys.refresh_memory();
                }

                // Collect memory metrics
                let (used_memory, total_memory) = {
                    let sys = system.read();
                    (sys.used_memory(), sys.total_memory())
                };

                let usage_mb = (used_memory / 1024 / 1024) as u32;
                let total_mb = (total_memory / 1024 / 1024) as u32;
                let usage_percent = if total_mb > 0 {
                    (usage_mb as f32 / total_mb as f32) * 100.0
                } else {
                    0.0
                };

                {
                    let mut metrics = metrics.write();
                    metrics.add_sample(usage_mb, usage_percent, Instant::now());
                }
            }
        });

        tracing::debug!("Memory profiling started");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<MemoryProfile, BitCrapsError> {
        *self.profiling_active.write() = false;

        // Wait for final samples
        tokio::time::sleep(Duration::from_millis(300)).await;

        let metrics = self.metrics.read().clone();
        let allocation_stats = self.allocation_tracker.read().get_statistics();

        // Reset for next session
        {
            let mut metrics_guard = self.metrics.write();
            *metrics_guard = MemoryMetrics::new();
        }
        {
            let mut tracker = self.allocation_tracker.write();
            tracker.reset();
        }

        Ok(MemoryProfile {
            total_samples: metrics.samples.len(),
            average_usage_mb: metrics.average_usage_mb(),
            peak_usage_mb: metrics.peak_usage_mb(),
            usage_trend: metrics.calculate_trend(),
            allocation_rate: allocation_stats.allocations_per_second,
            deallocation_rate: allocation_stats.deallocations_per_second,
            leak_suspects: allocation_stats.potential_leaks,
            fragmentation_level: self.estimate_fragmentation(&metrics),
            profiling_duration: metrics.profiling_duration(),
        })
    }

    pub async fn current_metrics(&self) -> Result<MemoryMetrics, BitCrapsError> {
        Ok(self.metrics.read().clone())
    }

    /// Track an allocation (call this when allocating large objects)
    pub fn track_allocation(&self, size: usize, type_name: &str, location: &str) -> AllocationId {
        let mut tracker = self.allocation_tracker.write();
        tracker.track_allocation(size, type_name, location)
    }

    /// Track a deallocation
    pub fn track_deallocation(&self, allocation_id: AllocationId) {
        let mut tracker = self.allocation_tracker.write();
        tracker.track_deallocation(allocation_id);
    }

    /// Profile memory usage of a specific function
    pub async fn profile_memory_usage<F, R>(
        &mut self,
        name: &str,
        func: F,
    ) -> Result<(R, MemoryUsageProfile), BitCrapsError>
    where
        F: std::future::Future<Output = R>,
    {
        let start_memory = self.get_current_memory_usage().await?;
        let start_time = Instant::now();

        let result = func.await;

        let end_memory = self.get_current_memory_usage().await?;
        let duration = start_time.elapsed();

        let profile = MemoryUsageProfile {
            function_name: name.to_string(),
            duration,
            memory_before_mb: start_memory,
            memory_after_mb: end_memory,
            memory_delta_mb: end_memory as i32 - start_memory as i32,
            peak_memory_mb: end_memory.max(start_memory), // Simplified - real implementation would track peak
        };

        Ok((result, profile))
    }

    /// Get current memory usage in MB
    async fn get_current_memory_usage(&self) -> Result<u32, BitCrapsError> {
        {
            let mut system = self.system.write();
            system.refresh_memory();
        }

        let usage_mb = {
            let system = self.system.read();
            (system.used_memory() / 1024 / 1024) as u32
        };

        Ok(usage_mb)
    }

    /// Estimate memory fragmentation based on allocation patterns
    fn estimate_fragmentation(&self, metrics: &MemoryMetrics) -> f32 {
        let allocation_stats = self.allocation_tracker.read().get_statistics();

        // Simple heuristic: high allocation/deallocation rate with stable usage suggests fragmentation
        if metrics.samples.len() < 10 {
            return 0.0;
        }

        let usage_variance = metrics.calculate_usage_variance();
        let allocation_ratio = if allocation_stats.total_deallocations > 0 {
            allocation_stats.total_allocations as f32 / allocation_stats.total_deallocations as f32
        } else {
            1.0
        };

        // Higher variance with high allocation activity suggests fragmentation
        (usage_variance * allocation_ratio).min(100.0)
    }
}

/// Memory performance metrics
#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    pub samples: Vec<MemorySample>,
    pub start_time: Option<Instant>,
}

impl MemoryMetrics {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            start_time: None,
        }
    }

    pub fn add_sample(&mut self, usage_mb: u32, usage_percent: f32, timestamp: Instant) {
        if self.start_time.is_none() {
            self.start_time = Some(timestamp);
        }

        self.samples.push(MemorySample {
            usage_mb,
            usage_percent,
            timestamp,
        });

        // Keep only last 5000 samples
        if self.samples.len() > 5000 {
            self.samples.remove(0);
        }
    }

    pub fn average_usage_mb(&self) -> u32 {
        if self.samples.is_empty() {
            0
        } else {
            (self.samples.iter().map(|s| s.usage_mb as u64).sum::<u64>()
                / self.samples.len() as u64) as u32
        }
    }

    pub fn peak_usage_mb(&self) -> u32 {
        self.samples.iter().map(|s| s.usage_mb).max().unwrap_or(0)
    }

    pub fn calculate_trend(&self) -> MemoryTrend {
        if self.samples.len() < 3 {
            return MemoryTrend::Stable;
        }

        let recent_samples = &self.samples[self.samples.len().saturating_sub(10)..];
        let first_avg = recent_samples
            .iter()
            .take(recent_samples.len() / 2)
            .map(|s| s.usage_mb as f32)
            .sum::<f32>()
            / (recent_samples.len() / 2) as f32;

        let second_avg = recent_samples
            .iter()
            .skip(recent_samples.len() / 2)
            .map(|s| s.usage_mb as f32)
            .sum::<f32>()
            / (recent_samples.len() - recent_samples.len() / 2) as f32;

        let change_percent = ((second_avg - first_avg) / first_avg) * 100.0;

        match change_percent {
            x if x > 10.0 => MemoryTrend::Increasing,
            x if x < -10.0 => MemoryTrend::Decreasing,
            _ => MemoryTrend::Stable,
        }
    }

    pub fn calculate_usage_variance(&self) -> f32 {
        if self.samples.len() < 2 {
            return 0.0;
        }

        let mean = self.average_usage_mb() as f32;
        let variance = self
            .samples
            .iter()
            .map(|s| {
                let diff = s.usage_mb as f32 - mean;
                diff * diff
            })
            .sum::<f32>()
            / self.samples.len() as f32;

        variance.sqrt()
    }

    pub fn profiling_duration(&self) -> Duration {
        if let (Some(start), Some(last)) = (self.start_time, self.samples.last()) {
            last.timestamp - start
        } else {
            Duration::from_nanos(0)
        }
    }
}

/// Individual memory usage sample
#[derive(Debug, Clone)]
pub struct MemorySample {
    pub usage_mb: u32,
    pub usage_percent: f32,
    pub timestamp: Instant,
}

/// Memory usage trend analysis
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryTrend {
    Increasing,
    Stable,
    Decreasing,
}

/// Complete memory profiling results
#[derive(Debug, Clone)]
pub struct MemoryProfile {
    pub total_samples: usize,
    pub average_usage_mb: u32,
    pub peak_usage_mb: u32,
    pub usage_trend: MemoryTrend,
    pub allocation_rate: f64,   // allocations per second
    pub deallocation_rate: f64, // deallocations per second
    pub leak_suspects: Vec<LeakSuspect>,
    pub fragmentation_level: f32, // 0-100 estimate
    pub profiling_duration: Duration,
}

/// Memory usage profile for a specific function
#[derive(Debug, Clone)]
pub struct MemoryUsageProfile {
    pub function_name: String,
    pub duration: Duration,
    pub memory_before_mb: u32,
    pub memory_after_mb: u32,
    pub memory_delta_mb: i32,
    pub peak_memory_mb: u32,
}

/// Allocation tracking for leak detection
pub struct AllocationTracker {
    active_allocations: FxHashMap<AllocationId, AllocationInfo>,
    next_id: u64,
    total_allocations: u64,
    total_deallocations: u64,
    total_bytes_allocated: u64,
    allocation_history: Vec<AllocationEvent>,
    start_time: Instant,
}

impl AllocationTracker {
    pub fn new() -> Self {
        Self {
            active_allocations: FxHashMap::default(),
            next_id: 1,
            total_allocations: 0,
            total_deallocations: 0,
            total_bytes_allocated: 0,
            allocation_history: Vec::new(),
            start_time: Instant::now(),
        }
    }

    pub fn track_allocation(
        &mut self,
        size: usize,
        type_name: &str,
        location: &str,
    ) -> AllocationId {
        let id = AllocationId(self.next_id);
        self.next_id += 1;

        let info = AllocationInfo {
            id,
            size,
            type_name: type_name.to_string(),
            location: location.to_string(),
            allocated_at: Instant::now(),
        };

        self.active_allocations.insert(id, info.clone());
        self.total_allocations += 1;
        self.total_bytes_allocated += size as u64;

        // Record allocation event
        self.allocation_history.push(AllocationEvent {
            allocation_id: id,
            event_type: AllocationEventType::Allocated,
            size,
            timestamp: Instant::now(),
        });

        // Keep history bounded
        if self.allocation_history.len() > 10000 {
            self.allocation_history.remove(0);
        }

        id
    }

    pub fn track_deallocation(&mut self, allocation_id: AllocationId) {
        if let Some(info) = self.active_allocations.remove(&allocation_id) {
            self.total_deallocations += 1;

            self.allocation_history.push(AllocationEvent {
                allocation_id,
                event_type: AllocationEventType::Deallocated,
                size: info.size,
                timestamp: Instant::now(),
            });
        }
    }

    pub fn get_statistics(&self) -> AllocationStatistics {
        let duration = self.start_time.elapsed();
        let duration_seconds = duration.as_secs_f64();

        let allocations_per_second = if duration_seconds > 0.0 {
            self.total_allocations as f64 / duration_seconds
        } else {
            0.0
        };

        let deallocations_per_second = if duration_seconds > 0.0 {
            self.total_deallocations as f64 / duration_seconds
        } else {
            0.0
        };

        // Find potential leaks (allocations that have been active for a long time)
        let now = Instant::now();
        let leak_threshold = Duration::from_secs(300); // 5 minutes

        let potential_leaks = self
            .active_allocations
            .values()
            .filter(|info| now.duration_since(info.allocated_at) > leak_threshold)
            .map(|info| LeakSuspect {
                allocation_id: info.id,
                size: info.size,
                type_name: info.type_name.clone(),
                location: info.location.clone(),
                age: now.duration_since(info.allocated_at),
            })
            .collect();

        AllocationStatistics {
            total_allocations: self.total_allocations,
            total_deallocations: self.total_deallocations,
            active_allocations: self.active_allocations.len() as u64,
            total_bytes_allocated: self.total_bytes_allocated,
            allocations_per_second,
            deallocations_per_second,
            potential_leaks,
        }
    }

    pub fn reset(&mut self) {
        self.active_allocations.clear();
        self.allocation_history.clear();
        self.total_allocations = 0;
        self.total_deallocations = 0;
        self.total_bytes_allocated = 0;
        self.start_time = Instant::now();
    }
}

/// Unique identifier for tracked allocations
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct AllocationId(pub u64);

/// Information about a tracked allocation
#[derive(Debug, Clone)]
struct AllocationInfo {
    id: AllocationId,
    size: usize,
    type_name: String,
    location: String,
    allocated_at: Instant,
}

/// Allocation or deallocation event
#[derive(Debug, Clone)]
struct AllocationEvent {
    allocation_id: AllocationId,
    event_type: AllocationEventType,
    size: usize,
    timestamp: Instant,
}

#[derive(Debug, Clone, Copy)]
enum AllocationEventType {
    Allocated,
    Deallocated,
}

/// Statistics about allocations
#[derive(Debug, Clone)]
pub struct AllocationStatistics {
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub active_allocations: u64,
    pub total_bytes_allocated: u64,
    pub allocations_per_second: f64,
    pub deallocations_per_second: f64,
    pub potential_leaks: Vec<LeakSuspect>,
}

/// Potential memory leak
#[derive(Debug, Clone)]
pub struct LeakSuspect {
    pub allocation_id: AllocationId,
    pub size: usize,
    pub type_name: String,
    pub location: String,
    pub age: Duration,
}
