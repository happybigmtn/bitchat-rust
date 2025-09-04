//! Runtime Profiler for BitCraps
//!
//! Provides comprehensive runtime profiling capabilities including
//! CPU usage, memory allocation tracking, and performance bottleneck detection.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// Profiler configuration
#[derive(Clone, Debug)]
pub struct ProfilerConfig {
    /// Enable CPU profiling
    pub enable_cpu_profiling: bool,
    /// Enable memory profiling
    pub enable_memory_profiling: bool,
    /// Enable I/O profiling
    pub enable_io_profiling: bool,
    /// Sampling interval for metrics collection
    pub sampling_interval: Duration,
    /// Maximum number of samples to retain
    pub max_samples: usize,
    /// Threshold for slow operation detection (milliseconds)
    pub slow_operation_threshold_ms: f64,
    /// Enable stack trace collection for slow operations
    pub collect_stack_traces: bool,
    /// Profile data retention duration
    pub retention_duration: Duration,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            enable_cpu_profiling: true,
            enable_memory_profiling: true,
            enable_io_profiling: true,
            sampling_interval: Duration::from_millis(100),
            max_samples: 10000,
            slow_operation_threshold_ms: 100.0,
            collect_stack_traces: false, // Expensive operation
            retention_duration: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Performance profile data
#[derive(Debug, Clone)]
pub struct ProfileData {
    pub timestamp: Instant,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub heap_size_mb: f64,
    pub gc_pressure: f64,
    pub active_connections: usize,
    pub pending_operations: usize,
    pub network_io_bytes_per_sec: u64,
    pub disk_io_ops_per_sec: u64,
    pub operation_latencies: HashMap<String, f64>,
    pub slow_operations: Vec<SlowOperation>,
}

/// Slow operation tracking
#[derive(Debug, Clone)]
pub struct SlowOperation {
    pub operation_name: String,
    pub duration_ms: f64,
    pub timestamp: Instant,
    pub thread_id: String,
    pub stack_trace: Option<String>,
    pub context: HashMap<String, String>,
}

/// Operation timing context
#[derive(Debug)]
pub struct OperationTimer {
    operation_name: String,
    start_time: Instant,
    profiler: Arc<RuntimeProfiler>,
    context: HashMap<String, String>,
}

impl OperationTimer {
    pub fn new(operation_name: String, profiler: Arc<RuntimeProfiler>) -> Self {
        Self {
            operation_name,
            start_time: Instant::now(),
            profiler,
            context: HashMap::new(),
        }
    }

    pub fn add_context<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.context.insert(key.into(), value.into());
    }

    pub async fn finish(self) {
        let duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
        self.profiler
            .record_operation_timing(&self.operation_name, duration_ms, self.context)
            .await;
    }
}

/// Memory allocation tracking
#[derive(Debug, Clone)]
pub struct MemoryAllocation {
    pub timestamp: Instant,
    pub size_bytes: usize,
    pub allocation_type: AllocationType,
    pub call_site: String,
    pub thread_id: String,
}

#[derive(Debug, Clone)]
pub enum AllocationType {
    Heap,
    Stack,
    Mmap,
    Network,
    Database,
    Cache,
}

/// CPU profiling data
#[derive(Debug, Clone)]
pub struct CpuProfile {
    pub timestamp: Instant,
    pub total_cpu_percent: f64,
    pub user_cpu_percent: f64,
    pub system_cpu_percent: f64,
    pub idle_cpu_percent: f64,
    pub per_core_usage: Vec<f64>,
    pub context_switches_per_sec: u64,
    pub interrupts_per_sec: u64,
}

/// I/O profiling data
#[derive(Debug, Clone)]
pub struct IoProfile {
    pub timestamp: Instant,
    pub network_bytes_read: u64,
    pub network_bytes_written: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,
    pub database_queries_per_sec: f64,
    pub cache_hit_rate: f64,
    pub active_file_descriptors: usize,
}

/// Comprehensive profiler statistics
#[derive(Debug, Clone)]
pub struct ProfilerStatistics {
    pub uptime: Duration,
    pub total_operations: u64,
    pub slow_operations: u64,
    pub average_cpu_usage: f64,
    pub peak_memory_usage_mb: f64,
    pub total_allocations: u64,
    pub total_bytes_allocated: u64,
    pub gc_collections: u64,
    pub gc_time_ms: f64,
    pub top_operations_by_time: Vec<(String, f64)>,
    pub top_operations_by_count: Vec<(String, u64)>,
    pub bottlenecks: Vec<PerformanceBottleneck>,
}

/// Performance bottleneck detection
#[derive(Debug, Clone)]
pub struct PerformanceBottleneck {
    pub bottleneck_type: BottleneckType,
    pub severity: BottleneckSeverity,
    pub description: String,
    pub affected_operations: Vec<String>,
    pub suggested_fix: String,
    pub impact_estimate: ImpactEstimate,
}

#[derive(Debug, Clone)]
pub enum BottleneckType {
    CpuBound,
    MemoryBound,
    IoBound,
    NetworkBound,
    DatabaseBound,
    LockContention,
    GarbageCollection,
}

#[derive(Debug, Clone)]
pub enum BottleneckSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ImpactEstimate {
    pub performance_gain_percent: f64,
    pub effort_level: EffortLevel,
    pub priority_score: f64,
}

#[derive(Debug, Clone)]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Runtime profiler implementation
pub struct RuntimeProfiler {
    config: ProfilerConfig,
    profiles: Arc<RwLock<Vec<ProfileData>>>,
    operation_timings: Arc<RwLock<HashMap<String, Vec<f64>>>>,
    operation_counts: Arc<RwLock<HashMap<String, AtomicU64>>>,
    memory_allocations: Arc<RwLock<Vec<MemoryAllocation>>>,
    cpu_profiles: Arc<RwLock<Vec<CpuProfile>>>,
    io_profiles: Arc<RwLock<Vec<IoProfile>>>,
    slow_operations: Arc<RwLock<Vec<SlowOperation>>>,
    active_timers: Arc<RwLock<HashMap<Uuid, OperationTimer>>>,
    start_time: Instant,
    is_profiling: Arc<std::sync::atomic::AtomicBool>,
}

impl RuntimeProfiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            config,
            profiles: Arc::new(RwLock::new(Vec::new())),
            operation_timings: Arc::new(RwLock::new(HashMap::new())),
            operation_counts: Arc::new(RwLock::new(HashMap::new())),
            memory_allocations: Arc::new(RwLock::new(Vec::new())),
            cpu_profiles: Arc::new(RwLock::new(Vec::new())),
            io_profiles: Arc::new(RwLock::new(Vec::new())),
            slow_operations: Arc::new(RwLock::new(Vec::new())),
            active_timers: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            is_profiling: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Start profiling
    pub async fn start(&self) {
        if self.is_profiling.swap(true, Ordering::Relaxed) {
            return; // Already running
        }

        println!("Starting runtime profiler with config: {:?}", self.config);
        
        if self.config.enable_cpu_profiling {
            self.start_cpu_profiling().await;
        }
        
        if self.config.enable_memory_profiling {
            self.start_memory_profiling().await;
        }
        
        if self.config.enable_io_profiling {
            self.start_io_profiling().await;
        }
        
        self.start_profile_collection().await;
        self.start_cleanup_task().await;
    }

    /// Stop profiling
    pub async fn stop(&self) {
        self.is_profiling.store(false, Ordering::Relaxed);
        println!("Stopping runtime profiler");
    }

    /// Time an operation
    pub async fn time_operation(&self, operation_name: &str) -> OperationTimer {
        OperationTimer::new(operation_name.to_string(), Arc::new(self.clone()))
    }

    /// Record operation timing manually
    pub async fn record_operation_timing(
        &self,
        operation_name: &str,
        duration_ms: f64,
        context: HashMap<String, String>,
    ) {
        // Record timing
        {
            let mut timings = self.operation_timings.write().await;
            timings
                .entry(operation_name.to_string())
                .or_insert_with(Vec::new)
                .push(duration_ms);
        }

        // Increment operation count
        {
            let mut counts = self.operation_counts.write().await;
            counts
                .entry(operation_name.to_string())
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }

        // Check for slow operation
        if duration_ms > self.config.slow_operation_threshold_ms {
            let slow_op = SlowOperation {
                operation_name: operation_name.to_string(),
                duration_ms,
                timestamp: Instant::now(),
                thread_id: format!("{:?}", std::thread::current().id()),
                stack_trace: if self.config.collect_stack_traces {
                    Some(self.collect_stack_trace())
                } else {
                    None
                },
                context,
            };

            let mut slow_ops = self.slow_operations.write().await;
            slow_ops.push(slow_op);
            
            // Keep only recent slow operations
            if slow_ops.len() > self.config.max_samples / 10 {
                slow_ops.drain(0..slow_ops.len() / 2);
            }
        }
    }

    /// Record memory allocation
    pub async fn record_memory_allocation(
        &self,
        size_bytes: usize,
        allocation_type: AllocationType,
        call_site: &str,
    ) {
        if !self.config.enable_memory_profiling {
            return;
        }

        let allocation = MemoryAllocation {
            timestamp: Instant::now(),
            size_bytes,
            allocation_type,
            call_site: call_site.to_string(),
            thread_id: format!("{:?}", std::thread::current().id()),
        };

        let mut allocations = self.memory_allocations.write().await;
        allocations.push(allocation);
        
        // Keep memory allocation history bounded
        if allocations.len() > self.config.max_samples {
            allocations.drain(0..allocations.len() / 2);
        }
    }

    /// Get comprehensive profiler statistics
    pub async fn get_statistics(&self) -> ProfilerStatistics {
        let uptime = self.start_time.elapsed();
        
        // Calculate operation statistics
        let timings = self.operation_timings.read().await;
        let counts = self.operation_counts.read().await;
        let slow_ops = self.slow_operations.read().await;
        let profiles = self.profiles.read().await;
        let allocations = self.memory_allocations.read().await;

        let total_operations: u64 = counts.values()
            .map(|count| count.load(Ordering::Relaxed))
            .sum();

        let slow_operations = slow_ops.len() as u64;

        // Calculate average CPU usage
        let average_cpu_usage = if !profiles.is_empty() {
            profiles.iter().map(|p| p.cpu_usage_percent).sum::<f64>() / profiles.len() as f64
        } else {
            0.0
        };

        // Calculate peak memory usage
        let peak_memory_usage_mb = profiles.iter()
            .map(|p| p.memory_usage_mb)
            .fold(0.0f64, |acc, x| acc.max(x));

        // Calculate allocation statistics
        let total_allocations = allocations.len() as u64;
        let total_bytes_allocated: u64 = allocations.iter()
            .map(|a| a.size_bytes as u64)
            .sum();

        // Get top operations by time
        let mut top_operations_by_time: Vec<(String, f64)> = timings.iter()
            .map(|(name, times)| {
                let avg_time = times.iter().sum::<f64>() / times.len() as f64;
                (name.clone(), avg_time)
            })
            .collect();
        top_operations_by_time.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        top_operations_by_time.truncate(10);

        // Get top operations by count
        let mut top_operations_by_count: Vec<(String, u64)> = counts.iter()
            .map(|(name, count)| (name.clone(), count.load(Ordering::Relaxed)))
            .collect();
        top_operations_by_count.sort_by(|a, b| b.1.cmp(&a.1));
        top_operations_by_count.truncate(10);

        // Detect bottlenecks
        let bottlenecks = self.detect_bottlenecks(&profiles, &timings, &slow_ops).await;

        ProfilerStatistics {
            uptime,
            total_operations,
            slow_operations,
            average_cpu_usage,
            peak_memory_usage_mb,
            total_allocations,
            total_bytes_allocated,
            gc_collections: 0, // Would be collected from actual GC stats
            gc_time_ms: 0.0,  // Would be collected from actual GC stats
            top_operations_by_time,
            top_operations_by_count,
            bottlenecks,
        }
    }

    /// Generate profiling report
    pub async fn generate_report(&self) -> String {
        let stats = self.get_statistics().await;
        let mut report = String::new();
        
        report.push_str("=== RUNTIME PROFILER REPORT ===\n");
        report.push_str(&format!("Uptime: {:?}\n", stats.uptime));
        report.push_str(&format!("Total Operations: {}\n", stats.total_operations));
        report.push_str(&format!("Slow Operations: {} ({:.2}%)\n", 
                                stats.slow_operations,
                                if stats.total_operations > 0 {
                                    stats.slow_operations as f64 / stats.total_operations as f64 * 100.0
                                } else { 0.0 }));
        
        report.push_str("\n--- Performance Metrics ---\n");
        report.push_str(&format!("Average CPU Usage: {:.1}%\n", stats.average_cpu_usage));
        report.push_str(&format!("Peak Memory Usage: {:.1}MB\n", stats.peak_memory_usage_mb));
        report.push_str(&format!("Total Allocations: {}\n", stats.total_allocations));
        report.push_str(&format!("Total Bytes Allocated: {}MB\n", stats.total_bytes_allocated / 1024 / 1024));
        
        report.push_str("\n--- Top Operations by Average Time ---\n");
        for (i, (name, time)) in stats.top_operations_by_time.iter().enumerate() {
            report.push_str(&format!("{}. {}: {:.2}ms\n", i + 1, name, time));
        }
        
        report.push_str("\n--- Top Operations by Count ---\n");
        for (i, (name, count)) in stats.top_operations_by_count.iter().enumerate() {
            report.push_str(&format!("{}. {}: {} calls\n", i + 1, name, count));
        }
        
        if !stats.bottlenecks.is_empty() {
            report.push_str("\n--- Performance Bottlenecks ---\n");
            for (i, bottleneck) in stats.bottlenecks.iter().enumerate() {
                report.push_str(&format!("{}. {:?} ({:?}): {}\n", 
                                        i + 1, bottleneck.bottleneck_type, 
                                        bottleneck.severity, bottleneck.description));
                report.push_str(&format!("   Suggested Fix: {}\n", bottleneck.suggested_fix));
                report.push_str(&format!("   Impact: {:.1}% improvement ({:?} effort)\n", 
                                        bottleneck.impact_estimate.performance_gain_percent,
                                        bottleneck.impact_estimate.effort_level));
            }
        }
        
        report
    }

    /// Export profiling data for external analysis
    pub async fn export_profile_data(&self) -> HashMap<String, serde_json::Value> {
        let mut export = HashMap::new();
        
        let profiles = self.profiles.read().await;
        let timings = self.operation_timings.read().await;
        let slow_ops = self.slow_operations.read().await;
        let stats = self.get_statistics().await;
        
        export.insert("statistics".to_string(), serde_json::to_value(&stats).unwrap_or_default());
        export.insert("profile_count".to_string(), profiles.len().into());
        export.insert("slow_operations_count".to_string(), slow_ops.len().into());
        export.insert("operation_types".to_string(), timings.keys().len().into());
        
        export
    }

    /// Start CPU profiling task
    async fn start_cpu_profiling(&self) {
        let profiler = self.clone();
        tokio::spawn(async move {
            while profiler.is_profiling.load(Ordering::Relaxed) {
                let cpu_profile = profiler.collect_cpu_profile().await;
                {
                    let mut profiles = profiler.cpu_profiles.write().await;
                    profiles.push(cpu_profile);
                    
                    // Keep bounded
                    if profiles.len() > profiler.config.max_samples {
                        profiles.drain(0..profiles.len() / 2);
                    }
                }
                
                tokio::time::sleep(profiler.config.sampling_interval).await;
            }
        });
    }

    /// Start memory profiling task
    async fn start_memory_profiling(&self) {
        // Memory profiling is primarily event-driven (allocation tracking)
        // This task could monitor overall memory statistics
        let profiler = self.clone();
        tokio::spawn(async move {
            while profiler.is_profiling.load(Ordering::Relaxed) {
                // Could collect memory statistics here
                tokio::time::sleep(profiler.config.sampling_interval * 2).await;
            }
        });
    }

    /// Start I/O profiling task
    async fn start_io_profiling(&self) {
        let profiler = self.clone();
        tokio::spawn(async move {
            while profiler.is_profiling.load(Ordering::Relaxed) {
                let io_profile = profiler.collect_io_profile().await;
                {
                    let mut profiles = profiler.io_profiles.write().await;
                    profiles.push(io_profile);
                    
                    // Keep bounded
                    if profiles.len() > profiler.config.max_samples {
                        profiles.drain(0..profiles.len() / 2);
                    }
                }
                
                tokio::time::sleep(profiler.config.sampling_interval).await;
            }
        });
    }

    /// Start main profile collection task
    async fn start_profile_collection(&self) {
        let profiler = self.clone();
        tokio::spawn(async move {
            while profiler.is_profiling.load(Ordering::Relaxed) {
                let profile = profiler.collect_profile_data().await;
                {
                    let mut profiles = profiler.profiles.write().await;
                    profiles.push(profile);
                    
                    // Keep bounded
                    if profiles.len() > profiler.config.max_samples {
                        profiles.drain(0..profiles.len() / 2);
                    }
                }
                
                tokio::time::sleep(profiler.config.sampling_interval).await;
            }
        });
    }

    /// Start cleanup task to remove old data
    async fn start_cleanup_task(&self) {
        let profiler = self.clone();
        tokio::spawn(async move {
            while profiler.is_profiling.load(Ordering::Relaxed) {
                profiler.cleanup_old_data().await;
                tokio::time::sleep(Duration::from_secs(300)).await; // Every 5 minutes
            }
        });
    }

    /// Collect current profile data snapshot
    async fn collect_profile_data(&self) -> ProfileData {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // In a real implementation, these would be actual system metrics
        ProfileData {
            timestamp: Instant::now(),
            cpu_usage_percent: rng.gen_range(20.0..80.0),
            memory_usage_mb: rng.gen_range(100.0..500.0),
            heap_size_mb: rng.gen_range(80.0..400.0),
            gc_pressure: rng.gen_range(0.0..1.0),
            active_connections: rng.gen_range(10..100),
            pending_operations: rng.gen_range(5..50),
            network_io_bytes_per_sec: rng.gen_range(1000..100000),
            disk_io_ops_per_sec: rng.gen_range(10..1000),
            operation_latencies: HashMap::new(), // Would be populated with actual data
            slow_operations: Vec::new(),
        }
    }

    /// Collect CPU profile data
    async fn collect_cpu_profile(&self) -> CpuProfile {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        let total_cpu = rng.gen_range(20.0..80.0);
        let user_cpu = total_cpu * rng.gen_range(0.6..0.8);
        let system_cpu = total_cpu * rng.gen_range(0.1..0.3);
        let idle_cpu = 100.0 - total_cpu;
        
        CpuProfile {
            timestamp: Instant::now(),
            total_cpu_percent: total_cpu,
            user_cpu_percent: user_cpu,
            system_cpu_percent: system_cpu,
            idle_cpu_percent: idle_cpu,
            per_core_usage: (0..num_cpus::get())
                .map(|_| rng.gen_range(10.0..90.0))
                .collect(),
            context_switches_per_sec: rng.gen_range(1000..10000),
            interrupts_per_sec: rng.gen_range(500..5000),
        }
    }

    /// Collect I/O profile data
    async fn collect_io_profile(&self) -> IoProfile {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        IoProfile {
            timestamp: Instant::now(),
            network_bytes_read: rng.gen_range(1000..100000),
            network_bytes_written: rng.gen_range(1000..100000),
            disk_bytes_read: rng.gen_range(0..50000),
            disk_bytes_written: rng.gen_range(0..50000),
            database_queries_per_sec: rng.gen_range(10.0..100.0),
            cache_hit_rate: rng.gen_range(0.7..0.95),
            active_file_descriptors: rng.gen_range(50..500),
        }
    }

    /// Detect performance bottlenecks
    async fn detect_bottlenecks(
        &self,
        _profiles: &[ProfileData],
        timings: &HashMap<String, Vec<f64>>,
        slow_ops: &[SlowOperation],
    ) -> Vec<PerformanceBottleneck> {
        let mut bottlenecks = Vec::new();

        // Check for CPU-bound operations
        for (operation, times) in timings.iter() {
            let avg_time = times.iter().sum::<f64>() / times.len() as f64;
            if avg_time > 500.0 && times.len() > 100 {
                bottlenecks.push(PerformanceBottleneck {
                    bottleneck_type: BottleneckType::CpuBound,
                    severity: if avg_time > 1000.0 { BottleneckSeverity::High } else { BottleneckSeverity::Medium },
                    description: format!("Operation '{}' shows high CPU usage (avg: {:.1}ms)", operation, avg_time),
                    affected_operations: vec![operation.clone()],
                    suggested_fix: "Consider async processing, caching, or algorithm optimization".to_string(),
                    impact_estimate: ImpactEstimate {
                        performance_gain_percent: 25.0,
                        effort_level: EffortLevel::Medium,
                        priority_score: avg_time / 100.0,
                    },
                });
            }
        }

        // Check for excessive slow operations
        if slow_ops.len() > 50 {
            bottlenecks.push(PerformanceBottleneck {
                bottleneck_type: BottleneckType::IoBound,
                severity: BottleneckSeverity::High,
                description: format!("High number of slow operations detected ({})", slow_ops.len()),
                affected_operations: slow_ops.iter().map(|op| op.operation_name.clone()).collect(),
                suggested_fix: "Review I/O operations, add connection pooling, implement caching".to_string(),
                impact_estimate: ImpactEstimate {
                    performance_gain_percent: 40.0,
                    effort_level: EffortLevel::High,
                    priority_score: slow_ops.len() as f64 / 10.0,
                },
            });
        }

        bottlenecks
    }

    /// Collect stack trace (simplified)
    fn collect_stack_trace(&self) -> String {
        // In a real implementation, this would collect actual stack traces
        format!("Stack trace collection not implemented in example")
    }

    /// Clean up old profiling data
    async fn cleanup_old_data(&self) {
        let cutoff_time = Instant::now() - self.config.retention_duration;
        
        // Clean up profiles
        {
            let mut profiles = self.profiles.write().await;
            profiles.retain(|p| p.timestamp > cutoff_time);
        }
        
        // Clean up slow operations
        {
            let mut slow_ops = self.slow_operations.write().await;
            slow_ops.retain(|op| op.timestamp > cutoff_time);
        }
        
        // Clean up memory allocations
        {
            let mut allocations = self.memory_allocations.write().await;
            allocations.retain(|a| a.timestamp > cutoff_time);
        }
    }
}

impl Clone for RuntimeProfiler {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            profiles: Arc::clone(&self.profiles),
            operation_timings: Arc::clone(&self.operation_timings),
            operation_counts: Arc::clone(&self.operation_counts),
            memory_allocations: Arc::clone(&self.memory_allocations),
            cpu_profiles: Arc::clone(&self.cpu_profiles),
            io_profiles: Arc::clone(&self.io_profiles),
            slow_operations: Arc::clone(&self.slow_operations),
            active_timers: Arc::clone(&self.active_timers),
            start_time: self.start_time,
            is_profiling: Arc::clone(&self.is_profiling),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_profiler_basic_operations() {
        let config = ProfilerConfig::default();
        let profiler = RuntimeProfiler::new(config);
        
        // Test operation timing
        let timer = profiler.time_operation("test_operation").await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        timer.finish().await;
        
        // Test memory allocation tracking
        profiler.record_memory_allocation(1024, AllocationType::Heap, "test_site").await;
        
        let stats = profiler.get_statistics().await;
        assert!(stats.total_operations > 0);
        assert!(stats.total_allocations > 0);
    }

    #[tokio::test]
    async fn test_profiler_start_stop() {
        let config = ProfilerConfig {
            sampling_interval: Duration::from_millis(50),
            ..Default::default()
        };
        let profiler = RuntimeProfiler::new(config);
        
        profiler.start().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        profiler.stop().await;
        
        let stats = profiler.get_statistics().await;
        assert!(stats.uptime > Duration::from_millis(150));
    }

    #[tokio::test]
    async fn test_slow_operation_detection() {
        let config = ProfilerConfig {
            slow_operation_threshold_ms: 50.0,
            ..Default::default()
        };
        let profiler = RuntimeProfiler::new(config);
        
        // Record a slow operation
        profiler.record_operation_timing("slow_op", 100.0, HashMap::new()).await;
        
        let stats = profiler.get_statistics().await;
        assert!(stats.slow_operations > 0);
    }

    #[tokio::test]
    async fn test_bottleneck_detection() {
        let config = ProfilerConfig::default();
        let profiler = RuntimeProfiler::new(config);
        
        // Create pattern that should trigger bottleneck detection
        for _ in 0..150 {
            profiler.record_operation_timing("cpu_heavy_op", 600.0, HashMap::new()).await;
        }
        
        let stats = profiler.get_statistics().await;
        assert!(!stats.bottlenecks.is_empty());
        
        let cpu_bottleneck = stats.bottlenecks.iter()
            .find(|b| matches!(b.bottleneck_type, BottleneckType::CpuBound));
        assert!(cpu_bottleneck.is_some());
    }

    #[tokio::test]
    async fn test_report_generation() {
        let config = ProfilerConfig::default();
        let profiler = RuntimeProfiler::new(config);
        
        // Add some test data
        profiler.record_operation_timing("op1", 25.0, HashMap::new()).await;
        profiler.record_operation_timing("op2", 75.0, HashMap::new()).await;
        profiler.record_memory_allocation(2048, AllocationType::Network, "network_buffer").await;
        
        let report = profiler.generate_report().await;
        assert!(report.contains("RUNTIME PROFILER REPORT"));
        assert!(report.contains("Performance Metrics"));
        assert!(report.contains("Top Operations"));
    }
}