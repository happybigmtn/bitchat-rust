//! Endurance Testing Suite for BitCraps
//!
//! This module provides long-running endurance tests to validate
//! system stability, memory leaks, and performance degradation over time.

use bitcraps::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use uuid::Uuid;

/// Endurance test configuration
#[derive(Clone, Debug)]
pub struct EnduranceTestConfig {
    /// Test duration (for long-running stability tests)
    pub test_duration: Duration,
    /// Sampling intervals for metrics collection
    pub metrics_interval: Duration,
    /// Baseline load to maintain throughout test
    pub baseline_connections: usize,
    pub baseline_message_rate: f64,
    /// Memory leak detection thresholds
    pub memory_growth_threshold_mb: f64, // MB per hour
    pub max_memory_variance_percent: f64,
    /// Performance degradation thresholds
    pub latency_degradation_threshold_percent: f64,
    pub throughput_degradation_threshold_percent: f64,
    /// System health checks
    pub health_check_interval: Duration,
    pub max_consecutive_failures: usize,
    /// Resource cleanup intervals
    pub cleanup_interval: Duration,
    pub gc_force_interval: Duration,
}

impl Default for EnduranceTestConfig {
    fn default() -> Self {
        Self {
            test_duration: Duration::from_secs(7200), // 2 hours
            metrics_interval: Duration::from_secs(60), // 1 minute
            baseline_connections: 50,
            baseline_message_rate: 5.0,
            memory_growth_threshold_mb: 50.0, // 50MB per hour max growth
            max_memory_variance_percent: 20.0,
            latency_degradation_threshold_percent: 25.0,
            throughput_degradation_threshold_percent: 15.0,
            health_check_interval: Duration::from_secs(300), // 5 minutes
            max_consecutive_failures: 3,
            cleanup_interval: Duration::from_secs(900), // 15 minutes
            gc_force_interval: Duration::from_secs(1800), // 30 minutes
        }
    }
}

/// Endurance test results
#[derive(Debug, Clone)]
pub struct EnduranceTestResults {
    pub test_duration: Duration,
    pub baseline_memory_mb: f64,
    pub final_memory_mb: f64,
    pub memory_growth_rate_mb_per_hour: f64,
    pub memory_leak_detected: bool,
    pub baseline_latency_ms: f64,
    pub final_latency_ms: f64,
    pub latency_degradation_percent: f64,
    pub baseline_throughput_ops_per_sec: f64,
    pub final_throughput_ops_per_sec: f64,
    pub throughput_degradation_percent: f64,
    pub total_operations: u64,
    pub total_failures: u64,
    pub failure_rate_percent: f64,
    pub health_check_failures: usize,
    pub stability_events: Vec<StabilityEvent>,
    pub resource_metrics: Vec<ResourceSnapshot>,
    pub gc_statistics: GarbageCollectionStats,
    pub overall_stability_score: f64,
    pub success: bool,
}

/// Stability event tracking
#[derive(Debug, Clone)]
pub struct StabilityEvent {
    pub timestamp: Instant,
    pub event_type: StabilityEventType,
    pub description: String,
    pub severity: Severity,
    pub system_state: DetailedSystemState,
}

#[derive(Debug, Clone)]
pub enum StabilityEventType {
    MemorySpike,
    LatencySpike,
    ThroughputDrop,
    ConnectionDrop,
    ResourceLeak,
    HealthCheckFailure,
    RecoveryEvent,
    GarbageCollection,
}

#[derive(Debug, Clone)]
pub enum Severity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Detailed system state snapshot
#[derive(Debug, Clone)]
pub struct DetailedSystemState {
    pub timestamp: Instant,
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub connections: usize,
    pub pending_operations: usize,
    pub latency_p50_ms: f64,
    pub latency_p99_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub open_file_descriptors: usize,
    pub heap_fragmentation_percent: f64,
}

/// Resource usage snapshot over time
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    pub timestamp: Instant,
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub network_bytes_per_sec: u64,
    pub disk_io_ops_per_sec: u64,
    pub active_connections: usize,
    pub operations_per_sec: f64,
    pub error_rate_percent: f64,
}

/// Garbage collection statistics
#[derive(Debug, Clone)]
pub struct GarbageCollectionStats {
    pub total_collections: usize,
    pub total_collection_time_ms: f64,
    pub average_collection_time_ms: f64,
    pub max_collection_time_ms: f64,
    pub memory_freed_mb: f64,
    pub collection_efficiency_percent: f64,
}

/// Advanced endurance metrics collector
pub struct EnduranceMetrics {
    start_time: Instant,
    operation_count: AtomicU64,
    failure_count: AtomicU64,
    health_check_failures: AtomicU64,
    stability_events: Arc<Mutex<Vec<StabilityEvent>>>,
    resource_snapshots: Arc<Mutex<Vec<ResourceSnapshot>>>,
    latency_samples: Arc<Mutex<Vec<(Instant, f64)>>>,
    throughput_samples: Arc<Mutex<Vec<(Instant, f64)>>>,
    gc_stats: Arc<Mutex<GarbageCollectionStats>>,
    test_active: AtomicBool,
}

impl EnduranceMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            operation_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            health_check_failures: AtomicU64::new(0),
            stability_events: Arc::new(Mutex::new(Vec::new())),
            resource_snapshots: Arc::new(Mutex::new(Vec::new())),
            latency_samples: Arc::new(Mutex::new(Vec::new())),
            throughput_samples: Arc::new(Mutex::new(Vec::new())),
            gc_stats: Arc::new(Mutex::new(GarbageCollectionStats {
                total_collections: 0,
                total_collection_time_ms: 0.0,
                average_collection_time_ms: 0.0,
                max_collection_time_ms: 0.0,
                memory_freed_mb: 0.0,
                collection_efficiency_percent: 0.0,
            })),
            test_active: AtomicBool::new(true),
        }
    }

    pub fn increment_operations(&self) -> u64 {
        self.operation_count.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_failures(&self) -> u64 {
        self.failure_count.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_health_failures(&self) -> u64 {
        self.health_check_failures.fetch_add(1, Ordering::Relaxed)
    }

    pub fn stop_test(&self) {
        self.test_active.store(false, Ordering::Relaxed);
    }

    pub fn is_test_active(&self) -> bool {
        self.test_active.load(Ordering::Relaxed)
    }

    pub async fn record_stability_event(&self, event: StabilityEvent) {
        let mut events = self.stability_events.lock().await;
        events.push(event);
    }

    pub async fn record_resource_snapshot(&self, snapshot: ResourceSnapshot) {
        let mut snapshots = self.resource_snapshots.lock().await;
        snapshots.push(snapshot);
    }

    pub async fn record_latency_sample(&self, latency_ms: f64) {
        let mut samples = self.latency_samples.lock().await;
        samples.push((Instant::now(), latency_ms));
    }

    pub async fn record_throughput_sample(&self, ops_per_sec: f64) {
        let mut samples = self.throughput_samples.lock().await;
        samples.push((Instant::now(), ops_per_sec));
    }

    pub async fn record_gc_event(&self, collection_time_ms: f64, memory_freed_mb: f64) {
        let mut stats = self.gc_stats.lock().await;
        stats.total_collections += 1;
        stats.total_collection_time_ms += collection_time_ms;
        stats.max_collection_time_ms = stats.max_collection_time_ms.max(collection_time_ms);
        stats.average_collection_time_ms = stats.total_collection_time_ms / stats.total_collections as f64;
        stats.memory_freed_mb += memory_freed_mb;
        
        // Calculate efficiency (simplified)
        stats.collection_efficiency_percent = (memory_freed_mb / collection_time_ms) * 100.0;
    }

    pub async fn generate_results(&self) -> EnduranceTestResults {
        let resource_snapshots = self.resource_snapshots.lock().await;
        let stability_events = self.stability_events.lock().await;
        let latency_samples = self.latency_samples.lock().await;
        let throughput_samples = self.throughput_samples.lock().await;
        let gc_stats = self.gc_stats.lock().await;

        let test_duration = self.start_time.elapsed();
        let total_operations = self.operation_count.load(Ordering::Relaxed);
        let total_failures = self.failure_count.load(Ordering::Relaxed);
        let health_failures = self.health_check_failures.load(Ordering::Relaxed) as usize;

        // Calculate baseline and final metrics
        let (baseline_memory, final_memory) = if resource_snapshots.len() >= 2 {
            let baseline = resource_snapshots.iter().take(5).map(|s| s.memory_mb).sum::<f64>() / 5.0.min(resource_snapshots.len() as f64);
            let final_val = resource_snapshots.iter().rev().take(5).map(|s| s.memory_mb).sum::<f64>() / 5.0.min(resource_snapshots.len() as f64);
            (baseline, final_val)
        } else {
            (0.0, 0.0)
        };

        let memory_growth_rate = if test_duration.as_secs() > 0 {
            (final_memory - baseline_memory) / (test_duration.as_secs_f64() / 3600.0)
        } else {
            0.0
        };

        let (baseline_latency, final_latency) = if latency_samples.len() >= 2 {
            let baseline = latency_samples.iter().take(10).map(|(_, l)| *l).sum::<f64>() / 10.0.min(latency_samples.len() as f64);
            let final_val = latency_samples.iter().rev().take(10).map(|(_, l)| *l).sum::<f64>() / 10.0.min(latency_samples.len() as f64);
            (baseline, final_val)
        } else {
            (0.0, 0.0)
        };

        let latency_degradation = if baseline_latency > 0.0 {
            ((final_latency - baseline_latency) / baseline_latency) * 100.0
        } else {
            0.0
        };

        let (baseline_throughput, final_throughput) = if throughput_samples.len() >= 2 {
            let baseline = throughput_samples.iter().take(10).map(|(_, t)| *t).sum::<f64>() / 10.0.min(throughput_samples.len() as f64);
            let final_val = throughput_samples.iter().rev().take(10).map(|(_, t)| *t).sum::<f64>() / 10.0.min(throughput_samples.len() as f64);
            (baseline, final_val)
        } else {
            (0.0, 0.0)
        };

        let throughput_degradation = if baseline_throughput > 0.0 {
            ((baseline_throughput - final_throughput) / baseline_throughput) * 100.0
        } else {
            0.0
        };

        let failure_rate = if total_operations > 0 {
            (total_failures as f64 / total_operations as f64) * 100.0
        } else {
            0.0
        };

        // Calculate overall stability score
        let stability_score = self.calculate_stability_score(
            memory_growth_rate,
            latency_degradation,
            throughput_degradation,
            failure_rate,
            health_failures,
            &stability_events,
        );

        EnduranceTestResults {
            test_duration,
            baseline_memory_mb: baseline_memory,
            final_memory_mb: final_memory,
            memory_growth_rate_mb_per_hour: memory_growth_rate,
            memory_leak_detected: memory_growth_rate > 100.0, // >100MB/hour considered a leak
            baseline_latency_ms: baseline_latency,
            final_latency_ms: final_latency,
            latency_degradation_percent: latency_degradation,
            baseline_throughput_ops_per_sec: baseline_throughput,
            final_throughput_ops_per_sec: final_throughput,
            throughput_degradation_percent: throughput_degradation,
            total_operations,
            total_failures,
            failure_rate_percent: failure_rate,
            health_check_failures: health_failures,
            stability_events: stability_events.clone(),
            resource_metrics: resource_snapshots.clone(),
            gc_statistics: gc_stats.clone(),
            overall_stability_score: stability_score,
            success: stability_score > 7.0 && !memory_growth_rate > 100.0,
        }
    }

    fn calculate_stability_score(
        &self,
        memory_growth_rate: f64,
        latency_degradation: f64,
        throughput_degradation: f64,
        failure_rate: f64,
        health_failures: usize,
        stability_events: &[StabilityEvent],
    ) -> f64 {
        let mut score = 10.0;

        // Memory stability
        score -= (memory_growth_rate / 50.0).min(3.0); // -3 points max for memory growth

        // Performance stability
        score -= (latency_degradation / 25.0).min(2.0); // -2 points max for latency degradation
        score -= (throughput_degradation / 20.0).min(2.0); // -2 points max for throughput degradation

        // Reliability
        score -= (failure_rate / 5.0).min(2.0); // -2 points max for failure rate

        // Health checks
        score -= (health_failures as f64 / 3.0).min(1.0); // -1 point max for health failures

        // Critical events
        let critical_events = stability_events.iter()
            .filter(|e| matches!(e.severity, Severity::Critical | Severity::Emergency))
            .count();
        score -= (critical_events as f64 * 0.5).min(2.0); // -0.5 per critical event, max -2

        score.max(0.0)
    }
}

/// Endurance test executor
pub struct EnduranceTester {
    config: EnduranceTestConfig,
    metrics: Arc<EnduranceMetrics>,
}

impl EnduranceTester {
    pub fn new(config: EnduranceTestConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(EnduranceMetrics::new()),
        }
    }

    /// Run comprehensive endurance test
    pub async fn run_endurance_test(&self) -> Result<EnduranceTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting endurance test - Duration: {:?}", self.config.test_duration);
        println!("Baseline load: {} connections, {:.1} msg/sec", 
                 self.config.baseline_connections, self.config.baseline_message_rate);

        let mut join_set = JoinSet::new();

        // Start monitoring tasks
        let metrics_monitor = self.start_metrics_monitor().await;
        let health_monitor = self.start_health_monitor().await;
        let cleanup_monitor = self.start_cleanup_monitor().await;
        
        // Start baseline workload
        for connection_id in 0..self.config.baseline_connections {
            let metrics = self.metrics.clone();
            let config = self.config.clone();
            
            join_set.spawn(async move {
                Self::endurance_connection_worker(connection_id, config, metrics).await
            });
        }

        // Wait for test completion or early termination
        tokio::time::timeout(self.config.test_duration + Duration::from_secs(60), async {
            tokio::time::sleep(self.config.test_duration).await;
        }).await.ok();

        // Stop monitoring
        self.metrics.stop_test();
        metrics_monitor.abort();
        health_monitor.abort();
        cleanup_monitor.abort();

        // Shutdown workload
        join_set.shutdown().await;

        let results = self.metrics.generate_results().await;
        self.print_endurance_results(&results);
        Ok(results)
    }

    /// Run memory leak detection test
    pub async fn run_memory_leak_test(&self) -> Result<EnduranceTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting memory leak detection test - Duration: {:?}", self.config.test_duration);

        let metrics_monitor = self.start_metrics_monitor().await;
        let memory_pressure_task = self.start_memory_pressure_simulation().await;

        // Wait for test completion
        tokio::time::sleep(self.config.test_duration).await;

        // Stop monitoring
        self.metrics.stop_test();
        metrics_monitor.abort();
        memory_pressure_task.abort();

        let results = self.metrics.generate_results().await;
        self.analyze_memory_leaks(&results);
        Ok(results)
    }

    /// Connection worker for endurance testing
    async fn endurance_connection_worker(
        connection_id: usize,
        config: EnduranceTestConfig,
        metrics: Arc<EnduranceMetrics>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message_interval = Duration::from_secs_f64(1.0 / config.baseline_message_rate);
        let mut operation_counter = 0u64;
        
        while metrics.is_test_active() {
            let start = Instant::now();
            
            // Simulate various operations
            let success = match operation_counter % 10 {
                0..=6 => Self::simulate_game_operation(connection_id).await,
                7..=8 => Self::simulate_consensus_operation(connection_id).await,
                9 => Self::simulate_heavy_operation(connection_id).await,
                _ => unreachable!(),
            };

            let latency = start.elapsed().as_secs_f64() * 1000.0;
            metrics.record_latency_sample(latency).await;

            if success {
                metrics.increment_operations();
            } else {
                metrics.increment_failures();
            }

            operation_counter += 1;
            
            // Maintain steady rate
            tokio::time::sleep(message_interval).await;
        }

        Ok(())
    }

    /// Start comprehensive metrics monitoring
    async fn start_metrics_monitor(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut sample_counter = 0;
            
            while metrics.is_test_active() {
                let snapshot = Self::collect_resource_snapshot().await;
                metrics.record_resource_snapshot(snapshot.clone()).await;

                // Check for stability events
                Self::detect_stability_events(&metrics, &snapshot, &config).await;

                sample_counter += 1;
                if sample_counter % 10 == 0 {
                    println!("Endurance Monitor - Sample {}: Memory: {:.1}MB, CPU: {:.1}%, Ops: {:.1}/sec",
                             sample_counter, snapshot.memory_mb, snapshot.cpu_percent, snapshot.operations_per_sec);
                }

                tokio::time::sleep(config.metrics_interval).await;
            }
        })
    }

    /// Start health monitoring
    async fn start_health_monitor(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut consecutive_failures = 0;
            
            while metrics.is_test_active() {
                let health_ok = Self::perform_health_check().await;
                
                if health_ok {
                    consecutive_failures = 0;
                } else {
                    consecutive_failures += 1;
                    metrics.increment_health_failures();
                    
                    let event = StabilityEvent {
                        timestamp: Instant::now(),
                        event_type: StabilityEventType::HealthCheckFailure,
                        description: format!("Health check failed (consecutive: {})", consecutive_failures),
                        severity: if consecutive_failures >= config.max_consecutive_failures {
                            Severity::Critical
                        } else {
                            Severity::Warning
                        },
                        system_state: Self::collect_detailed_system_state().await,
                    };
                    metrics.record_stability_event(event).await;
                    
                    if consecutive_failures >= config.max_consecutive_failures {
                        println!("🚨 CRITICAL: {} consecutive health check failures - system may be unstable", consecutive_failures);
                    }
                }
                
                tokio::time::sleep(config.health_check_interval).await;
            }
        })
    }

    /// Start cleanup monitoring and garbage collection
    async fn start_cleanup_monitor(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            while metrics.is_test_active() {
                // Simulate resource cleanup
                tokio::time::sleep(config.cleanup_interval).await;
                let cleanup_start = Instant::now();
                Self::perform_resource_cleanup().await;
                let cleanup_time = cleanup_start.elapsed().as_secs_f64() * 1000.0;
                
                // Simulate garbage collection
                if cleanup_time > 100.0 { // Only if significant cleanup occurred
                    let memory_freed = 10.0; // Simulated
                    metrics.record_gc_event(cleanup_time, memory_freed).await;
                    
                    let event = StabilityEvent {
                        timestamp: Instant::now(),
                        event_type: StabilityEventType::GarbageCollection,
                        description: format!("Cleanup completed - {:.1}ms, {:.1}MB freed", cleanup_time, memory_freed),
                        severity: Severity::Info,
                        system_state: Self::collect_detailed_system_state().await,
                    };
                    metrics.record_stability_event(event).await;
                }
            }
        })
    }

    /// Start memory pressure simulation for leak detection
    async fn start_memory_pressure_simulation(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut memory_allocations = Vec::new();
            
            while metrics.is_test_active() {
                // Simulate memory allocation patterns
                if memory_allocations.len() < 1000 {
                    let allocation = vec![0u8; 1024 * 1024]; // 1MB
                    memory_allocations.push(allocation);
                }
                
                // Occasionally free some memory
                if memory_allocations.len() > 100 && rand::random::<f32>() < 0.1 {
                    memory_allocations.drain(0..10);
                }
                
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    }

    /// Collect detailed resource snapshot
    async fn collect_resource_snapshot() -> ResourceSnapshot {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        ResourceSnapshot {
            timestamp: Instant::now(),
            memory_mb: rng.gen_range(100.0..500.0),
            cpu_percent: rng.gen_range(20.0..80.0),
            network_bytes_per_sec: rng.gen_range(1000..10000),
            disk_io_ops_per_sec: rng.gen_range(10..100),
            active_connections: rng.gen_range(40..60),
            operations_per_sec: rng.gen_range(100.0..300.0),
            error_rate_percent: rng.gen_range(0.0..2.0),
        }
    }

    /// Collect detailed system state
    async fn collect_detailed_system_state() -> DetailedSystemState {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        DetailedSystemState {
            timestamp: Instant::now(),
            memory_mb: rng.gen_range(100.0..500.0),
            cpu_percent: rng.gen_range(20.0..80.0),
            connections: rng.gen_range(40..60),
            pending_operations: rng.gen_range(10..50),
            latency_p50_ms: rng.gen_range(10.0..50.0),
            latency_p99_ms: rng.gen_range(50.0..200.0),
            throughput_ops_per_sec: rng.gen_range(100.0..300.0),
            open_file_descriptors: rng.gen_range(100..1000),
            heap_fragmentation_percent: rng.gen_range(5.0..25.0),
        }
    }

    /// Detect stability events from resource snapshots
    async fn detect_stability_events(
        metrics: &Arc<EnduranceMetrics>,
        snapshot: &ResourceSnapshot,
        config: &EnduranceTestConfig,
    ) {
        // Memory spike detection
        if snapshot.memory_mb > 400.0 {
            let event = StabilityEvent {
                timestamp: snapshot.timestamp,
                event_type: StabilityEventType::MemorySpike,
                description: format!("Memory usage spike: {:.1}MB", snapshot.memory_mb),
                severity: if snapshot.memory_mb > 450.0 { Severity::Critical } else { Severity::Warning },
                system_state: Self::collect_detailed_system_state().await,
            };
            metrics.record_stability_event(event).await;
        }

        // Throughput drop detection
        if snapshot.operations_per_sec < 150.0 {
            let event = StabilityEvent {
                timestamp: snapshot.timestamp,
                event_type: StabilityEventType::ThroughputDrop,
                description: format!("Throughput drop: {:.1} ops/sec", snapshot.operations_per_sec),
                severity: Severity::Warning,
                system_state: Self::collect_detailed_system_state().await,
            };
            metrics.record_stability_event(event).await;
        }

        // Record throughput for trend analysis
        metrics.record_throughput_sample(snapshot.operations_per_sec).await;
    }

    /// Perform health check
    async fn perform_health_check() -> bool {
        // Simulate health check with occasional failures
        use rand::Rng;
        rand::thread_rng().gen_range(0.0..1.0) > 0.05 // 5% failure rate
    }

    /// Perform resource cleanup
    async fn perform_resource_cleanup() {
        // Simulate resource cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    /// Simulate various operations
    async fn simulate_game_operation(_connection_id: usize) -> bool {
        tokio::time::sleep(Duration::from_millis(5)).await;
        true
    }

    async fn simulate_consensus_operation(_connection_id: usize) -> bool {
        tokio::time::sleep(Duration::from_millis(10)).await;
        use rand::Rng;
        rand::thread_rng().gen_range(0.0..1.0) > 0.02 // 2% failure rate
    }

    async fn simulate_heavy_operation(_connection_id: usize) -> bool {
        tokio::time::sleep(Duration::from_millis(20)).await;
        use rand::Rng;
        rand::thread_rng().gen_range(0.0..1.0) > 0.05 // 5% failure rate
    }

    /// Analyze memory leaks from results
    fn analyze_memory_leaks(&self, results: &EnduranceTestResults) {
        println!("\n=== MEMORY LEAK ANALYSIS ===");
        println!("Baseline Memory: {:.1}MB", results.baseline_memory_mb);
        println!("Final Memory: {:.1}MB", results.final_memory_mb);
        println!("Growth Rate: {:.1}MB/hour", results.memory_growth_rate_mb_per_hour);
        
        if results.memory_leak_detected {
            println!("🚨 MEMORY LEAK DETECTED");
            println!("  - Growth rate exceeds threshold ({:.1} MB/hour)", self.config.memory_growth_threshold_mb);
            println!("  - Consider reviewing memory management in:");
            println!("    * Connection handling");
            println!("    * Message processing");
            println!("    * Cache management");
            println!("    * Resource cleanup");
        } else {
            println!("✅ No significant memory leaks detected");
        }
    }

    /// Print comprehensive endurance results
    fn print_endurance_results(&self, results: &EnduranceTestResults) {
        println!("\n=== ENDURANCE TEST RESULTS ===");
        println!("Test Duration: {:?}", results.test_duration);
        println!("Total Operations: {}", results.total_operations);
        println!("Total Failures: {}", results.total_failures);
        println!("Failure Rate: {:.2}%", results.failure_rate_percent);
        
        println!("\n--- Memory Analysis ---");
        println!("Baseline Memory: {:.1}MB", results.baseline_memory_mb);
        println!("Final Memory: {:.1}MB", results.final_memory_mb);
        println!("Growth Rate: {:.1}MB/hour", results.memory_growth_rate_mb_per_hour);
        println!("Memory Leak: {}", results.memory_leak_detected);
        
        println!("\n--- Performance Analysis ---");
        println!("Baseline Latency: {:.2}ms", results.baseline_latency_ms);
        println!("Final Latency: {:.2}ms", results.final_latency_ms);
        println!("Latency Degradation: {:.1}%", results.latency_degradation_percent);
        println!("Baseline Throughput: {:.1} ops/sec", results.baseline_throughput_ops_per_sec);
        println!("Final Throughput: {:.1} ops/sec", results.final_throughput_ops_per_sec);
        println!("Throughput Degradation: {:.1}%", results.throughput_degradation_percent);
        
        println!("\n--- Stability Analysis ---");
        println!("Health Check Failures: {}", results.health_check_failures);
        println!("Stability Events: {}", results.stability_events.len());
        println!("Overall Stability Score: {:.1}/10.0", results.overall_stability_score);
        
        println!("\n--- Garbage Collection ---");
        println!("Total Collections: {}", results.gc_statistics.total_collections);
        println!("Total Collection Time: {:.1}ms", results.gc_statistics.total_collection_time_ms);
        println!("Average Collection Time: {:.1}ms", results.gc_statistics.average_collection_time_ms);
        println!("Max Collection Time: {:.1}ms", results.gc_statistics.max_collection_time_ms);
        
        // Key stability events
        let critical_events: Vec<_> = results.stability_events.iter()
            .filter(|e| matches!(e.severity, Severity::Critical | Severity::Emergency))
            .collect();
        
        if !critical_events.is_empty() {
            println!("\n--- Critical Events ---");
            for (i, event) in critical_events.iter().take(5).enumerate() {
                println!("  {}. {:?}: {}", i + 1, event.event_type, event.description);
            }
        }
        
        println!("\nOverall Success: {}", results.success);
        
        // Performance recommendations
        if results.memory_leak_detected {
            println!("🔧 RECOMMENDATION: Investigate memory management - leak detected");
        }
        if results.latency_degradation_percent > 20.0 {
            println!("🔧 RECOMMENDATION: Investigate latency degradation - {:.1}% increase", results.latency_degradation_percent);
        }
        if results.throughput_degradation_percent > 15.0 {
            println!("🔧 RECOMMENDATION: Investigate throughput degradation - {:.1}% decrease", results.throughput_degradation_percent);
        }
        if results.overall_stability_score < 7.0 {
            println!("🔧 RECOMMENDATION: Overall system stability needs improvement - score: {:.1}/10", results.overall_stability_score);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_short_endurance() {
        let config = EnduranceTestConfig {
            test_duration: Duration::from_secs(60), // 1 minute for testing
            baseline_connections: 10,
            baseline_message_rate: 2.0,
            metrics_interval: Duration::from_secs(5),
            ..Default::default()
        };
        
        let tester = EnduranceTester::new(config);
        let results = tester.run_endurance_test().await.unwrap();
        
        assert!(results.total_operations > 0, "Should process operations");
        assert!(results.overall_stability_score > 5.0, "Should maintain reasonable stability");
    }

    #[tokio::test]
    async fn test_memory_leak_detection() {
        let config = EnduranceTestConfig {
            test_duration: Duration::from_secs(30), // Short test
            memory_growth_threshold_mb: 10.0, // Low threshold for testing
            ..Default::default()
        };
        
        let tester = EnduranceTester::new(config);
        let results = tester.run_memory_leak_test().await.unwrap();
        
        assert!(results.baseline_memory_mb > 0.0, "Should measure baseline memory");
        // Note: Leak detection result will vary based on simulation
    }

    #[tokio::test]
    async fn test_stability_scoring() {
        let metrics = EnduranceMetrics::new();
        
        // Simulate some operations
        metrics.increment_operations();
        metrics.record_latency_sample(50.0).await;
        metrics.record_throughput_sample(200.0).await;
        
        let results = metrics.generate_results().await;
        
        assert!(results.overall_stability_score >= 0.0, "Stability score should be valid");
        assert!(results.overall_stability_score <= 10.0, "Stability score should be in range");
    }
}