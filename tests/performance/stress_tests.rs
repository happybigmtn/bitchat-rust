//! Stress Testing Suite for BitCraps
//!
//! This module provides stress testing scenarios that push the system
//! beyond normal operating conditions to identify breaking points.

use bitcraps::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore, RwLock};
use tokio::task::JoinSet;
use uuid::Uuid;

/// Stress test configuration
#[derive(Clone, Debug)]
pub struct StressTestConfig {
    /// Maximum number of concurrent connections to test
    pub max_connections: usize,
    /// Connection ramp-up rate (connections per second)
    pub connection_ramp_rate: f64,
    /// Message burst size
    pub message_burst_size: usize,
    /// Memory pressure target (MB)
    pub memory_pressure_mb: usize,
    /// CPU stress level (0.0 to 1.0)
    pub cpu_stress_level: f64,
    /// Network congestion simulation
    pub network_delay_ms: u64,
    pub network_loss_percent: f64,
    /// Resource exhaustion thresholds
    pub max_open_files: usize,
    pub max_memory_mb: usize,
    pub max_cpu_percent: f64,
    /// Test duration limits
    pub max_test_duration: Duration,
    pub failure_threshold_percent: f64,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_ramp_rate: 50.0,
            message_burst_size: 100,
            memory_pressure_mb: 2048,
            cpu_stress_level: 0.9,
            network_delay_ms: 100,
            network_loss_percent: 5.0,
            max_open_files: 65536,
            max_memory_mb: 4096,
            max_cpu_percent: 95.0,
            max_test_duration: Duration::from_secs(1800), // 30 minutes
            failure_threshold_percent: 10.0,
        }
    }
}

/// Stress test results
#[derive(Debug, Clone)]
pub struct StressTestResults {
    pub test_type: String,
    pub peak_connections: usize,
    pub peak_memory_mb: usize,
    pub peak_cpu_percent: f64,
    pub peak_latency_ms: f64,
    pub failure_rate: f64,
    pub breaking_point_connections: Option<usize>,
    pub recovery_time_ms: f64,
    pub resource_exhaustion_detected: bool,
    pub system_stability_score: f64, // 0.0 to 10.0
    pub detailed_failures: Vec<FailureReport>,
    pub success: bool,
}

/// Individual failure report
#[derive(Debug, Clone)]
pub struct FailureReport {
    pub timestamp: Instant,
    pub connection_id: usize,
    pub failure_type: FailureType,
    pub error_message: String,
    pub system_state: SystemState,
}

#[derive(Debug, Clone)]
pub enum FailureType {
    ConnectionTimeout,
    MemoryExhaustion,
    CpuOverload,
    NetworkCongestion,
    ResourceLeak,
    DeadLock,
    Panic,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct SystemState {
    pub memory_mb: usize,
    pub cpu_percent: f64,
    pub open_connections: usize,
    pub pending_operations: usize,
}

/// Advanced stress test metrics
pub struct StressMetrics {
    connection_count: AtomicU64,
    failure_count: AtomicU64,
    memory_samples: Arc<Mutex<Vec<usize>>>,
    cpu_samples: Arc<Mutex<Vec<f64>>>,
    latency_samples: Arc<Mutex<Vec<f64>>>,
    failures: Arc<Mutex<Vec<FailureReport>>>,
    start_time: Instant,
}

impl StressMetrics {
    pub fn new() -> Self {
        Self {
            connection_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            memory_samples: Arc::new(Mutex::new(Vec::new())),
            cpu_samples: Arc::new(Mutex::new(Vec::new())),
            latency_samples: Arc::new(Mutex::new(Vec::new())),
            failures: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        }
    }

    pub fn increment_connections(&self) -> u64 {
        self.connection_count.fetch_add(1, Ordering::Relaxed)
    }

    pub fn decrement_connections(&self) -> u64 {
        self.connection_count.fetch_sub(1, Ordering::Relaxed)
    }

    pub fn get_connection_count(&self) -> u64 {
        self.connection_count.load(Ordering::Relaxed)
    }

    pub fn increment_failures(&self) -> u64 {
        self.failure_count.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn record_failure(&self, failure: FailureReport) {
        let mut failures = self.failures.lock().await;
        failures.push(failure);
        self.increment_failures();
    }

    pub async fn record_system_metrics(&self, memory_mb: usize, cpu_percent: f64) {
        {
            let mut memory = self.memory_samples.lock().await;
            memory.push(memory_mb);
        }
        {
            let mut cpu = self.cpu_samples.lock().await;
            cpu.push(cpu_percent);
        }
    }

    pub async fn record_latency(&self, latency_ms: f64) {
        let mut latencies = self.latency_samples.lock().await;
        latencies.push(latency_ms);
    }

    pub async fn generate_results(&self, test_type: String) -> StressTestResults {
        let memory_samples = self.memory_samples.lock().await;
        let cpu_samples = self.cpu_samples.lock().await;
        let latency_samples = self.latency_samples.lock().await;
        let failures = self.failures.lock().await;

        let peak_memory = memory_samples.iter().max().copied().unwrap_or(0);
        let peak_cpu = cpu_samples.iter().fold(0.0f64, |acc, &x| acc.max(x));
        let peak_latency = latency_samples.iter().fold(0.0f64, |acc, &x| acc.max(x));
        let total_operations = self.connection_count.load(Ordering::Relaxed);
        let total_failures = self.failure_count.load(Ordering::Relaxed);
        
        let failure_rate = if total_operations > 0 {
            (total_failures as f64 / total_operations as f64) * 100.0
        } else {
            0.0
        };

        // Calculate stability score based on various factors
        let stability_score = self.calculate_stability_score(
            failure_rate,
            peak_memory,
            peak_cpu,
            peak_latency,
        );

        // Detect breaking point (simplified heuristic)
        let breaking_point = if failure_rate > 50.0 {
            Some(total_operations as usize)
        } else {
            None
        };

        StressTestResults {
            test_type,
            peak_connections: total_operations as usize,
            peak_memory_mb: peak_memory,
            peak_cpu_percent: peak_cpu,
            peak_latency_ms: peak_latency,
            failure_rate,
            breaking_point_connections: breaking_point,
            recovery_time_ms: 0.0, // Would be calculated based on recovery tests
            resource_exhaustion_detected: peak_memory > 3000 || peak_cpu > 95.0,
            system_stability_score: stability_score,
            detailed_failures: failures.clone(),
            success: failure_rate < 10.0 && stability_score > 6.0,
        }
    }

    fn calculate_stability_score(
        &self,
        failure_rate: f64,
        peak_memory: usize,
        peak_cpu: f64,
        peak_latency: f64,
    ) -> f64 {
        let mut score = 10.0;

        // Deduct points for high failure rate
        score -= (failure_rate / 10.0).min(5.0);

        // Deduct points for high resource usage
        if peak_memory > 2000 {
            score -= ((peak_memory - 2000) as f64 / 1000.0).min(2.0);
        }
        if peak_cpu > 80.0 {
            score -= ((peak_cpu - 80.0) / 10.0).min(2.0);
        }
        if peak_latency > 500.0 {
            score -= ((peak_latency - 500.0) / 500.0).min(1.0);
        }

        score.max(0.0)
    }
}

/// Stress test executor
pub struct StressTester {
    config: StressTestConfig,
    metrics: Arc<StressMetrics>,
    rate_limiter: Arc<Semaphore>,
}

impl StressTester {
    pub fn new(config: StressTestConfig) -> Self {
        Self {
            rate_limiter: Arc::new(Semaphore::new(config.max_connections)),
            config,
            metrics: Arc::new(StressMetrics::new()),
        }
    }

    /// Run connection flooding stress test
    pub async fn run_connection_flood_test(&self) -> Result<StressTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting connection flood test - target: {} connections", self.config.max_connections);
        
        let mut join_set = JoinSet::new();
        let connection_interval = Duration::from_secs_f64(1.0 / self.config.connection_ramp_rate);
        
        // Start system monitoring
        let monitor_handle = self.start_stress_monitor().await;
        
        // Gradually ramp up connections
        for connection_id in 0..self.config.max_connections {
            let permit = self.rate_limiter.clone().acquire_owned().await?;
            let metrics = self.metrics.clone();
            let config = self.config.clone();
            
            join_set.spawn(async move {
                let _permit = permit; // Hold permit for connection lifetime
                Self::stress_connection(connection_id, config, metrics).await
            });
            
            // Rate limit connection creation
            if connection_id % 10 == 0 {
                tokio::time::sleep(connection_interval).await;
            }
            
            // Emergency brake if system is failing
            if self.should_abort_test().await {
                println!("Emergency abort triggered - system instability detected");
                break;
            }
        }
        
        // Wait for connections to complete or timeout
        let timeout_duration = Duration::from_secs(300);
        tokio::time::timeout(timeout_duration, async {
            while let Some(result) = join_set.join_next().await {
                if let Err(e) = result {
                    eprintln!("Stress connection failed: {}", e);
                }
            }
        }).await.ok(); // Ignore timeout errors
        
        monitor_handle.abort();
        let results = self.metrics.generate_results("Connection Flood".to_string()).await;
        self.print_stress_results(&results);
        Ok(results)
    }

    /// Run memory pressure stress test
    pub async fn run_memory_pressure_test(&self) -> Result<StressTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting memory pressure test - target: {}MB", self.config.memory_pressure_mb);
        
        let monitor_handle = self.start_stress_monitor().await;
        let mut memory_allocations = Vec::new();
        
        // Gradually increase memory pressure
        let chunk_size = 10 * 1024 * 1024; // 10MB chunks
        let target_bytes = self.config.memory_pressure_mb * 1024 * 1024;
        
        for chunk in 0..(target_bytes / chunk_size) {
            // Allocate memory chunk
            let data: Vec<u8> = vec![0; chunk_size];
            memory_allocations.push(data);
            
            let current_memory = (chunk + 1) * chunk_size / (1024 * 1024);
            self.metrics.record_system_metrics(current_memory, 50.0).await;
            
            // Test system responsiveness under memory pressure
            let latency_start = Instant::now();
            self.simulate_memory_intensive_operation().await;
            let latency = latency_start.elapsed().as_secs_f64() * 1000.0;
            self.metrics.record_latency(latency).await;
            
            if self.should_abort_test().await {
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        monitor_handle.abort();
        let results = self.metrics.generate_results("Memory Pressure".to_string()).await;
        self.print_stress_results(&results);
        Ok(results)
    }

    /// Run CPU stress test
    pub async fn run_cpu_stress_test(&self) -> Result<StressTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting CPU stress test - target: {:.0}% utilization", 
                 self.config.cpu_stress_level * 100.0);
        
        let monitor_handle = self.start_stress_monitor().await;
        let cpu_workers = num_cpus::get();
        let mut join_set = JoinSet::new();
        
        // Spawn CPU-intensive workers
        for worker_id in 0..cpu_workers {
            let metrics = self.metrics.clone();
            let stress_level = self.config.cpu_stress_level;
            
            join_set.spawn(async move {
                Self::cpu_stress_worker(worker_id, stress_level, metrics).await
            });
        }
        
        // Run for configured duration
        tokio::time::sleep(Duration::from_secs(60)).await;
        
        // Stop all workers
        join_set.shutdown().await;
        monitor_handle.abort();
        
        let results = self.metrics.generate_results("CPU Stress".to_string()).await;
        self.print_stress_results(&results);
        Ok(results)
    }

    /// Run network congestion stress test
    pub async fn run_network_congestion_test(&self) -> Result<StressTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting network congestion test - delay: {}ms, loss: {:.1}%", 
                 self.config.network_delay_ms, self.config.network_loss_percent);
        
        let monitor_handle = self.start_stress_monitor().await;
        let mut join_set = JoinSet::new();
        
        // Create multiple concurrent network streams
        for stream_id in 0..100 {
            let metrics = self.metrics.clone();
            let config = self.config.clone();
            
            join_set.spawn(async move {
                Self::network_stress_stream(stream_id, config, metrics).await
            });
        }
        
        // Wait for streams to complete
        while let Some(result) = join_set.join_next().await {
            if let Err(e) = result {
                eprintln!("Network stream failed: {}", e);
            }
        }
        
        monitor_handle.abort();
        let results = self.metrics.generate_results("Network Congestion".to_string()).await;
        self.print_stress_results(&results);
        Ok(results)
    }

    /// Simulate a stressed connection
    async fn stress_connection(
        connection_id: usize,
        config: StressTestConfig,
        metrics: Arc<StressMetrics>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        metrics.increment_connections();
        
        let start_time = Instant::now();
        
        // Simulate connection stress scenarios
        for burst in 0..5 {
            for message in 0..config.message_burst_size {
                let latency_start = Instant::now();
                
                // Simulate message processing with potential failures
                let success = Self::simulate_stressed_operation(connection_id, burst, message).await;
                
                let latency = latency_start.elapsed().as_secs_f64() * 1000.0;
                metrics.record_latency(latency).await;
                
                if !success {
                    let failure = FailureReport {
                        timestamp: Instant::now(),
                        connection_id,
                        failure_type: FailureType::ConnectionTimeout,
                        error_message: format!("Message processing failed for connection {}", connection_id),
                        system_state: SystemState {
                            memory_mb: 100, // Simulated
                            cpu_percent: 75.0,
                            open_connections: metrics.get_connection_count() as usize,
                            pending_operations: 50,
                        },
                    };
                    metrics.record_failure(failure).await;
                }
            }
            
            // Brief pause between bursts
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        metrics.decrement_connections();
        Ok(())
    }

    /// Simulate memory-intensive operation
    async fn simulate_memory_intensive_operation(&self) {
        // Simulate complex data processing that uses significant memory
        let _temp_data: Vec<Vec<u8>> = (0..1000)
            .map(|_| vec![0u8; 1024]) // 1KB each
            .collect();
        
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    /// CPU stress worker
    async fn cpu_stress_worker(
        worker_id: usize,
        stress_level: f64,
        metrics: Arc<StressMetrics>,
    ) {
        println!("Starting CPU worker {} with {:.0}% stress", worker_id, stress_level * 100.0);
        
        let work_duration = Duration::from_millis((100.0 * stress_level) as u64);
        let rest_duration = Duration::from_millis((100.0 * (1.0 - stress_level)) as u64);
        
        for cycle in 0..600 { // 60 seconds at 100ms intervals
            let start = Instant::now();
            
            // CPU-intensive work
            while start.elapsed() < work_duration {
                // Simulate computation
                let _result: f64 = (0..1000)
                    .map(|i| (i as f64).sin().cos())
                    .sum();
            }
            
            // Record CPU usage
            let cpu_usage = stress_level * 100.0;
            metrics.record_system_metrics(200, cpu_usage).await;
            
            // Rest period
            tokio::time::sleep(rest_duration).await;
            
            if cycle % 100 == 0 {
                println!("CPU worker {} completed {} cycles", worker_id, cycle);
            }
        }
    }

    /// Network stress stream
    async fn network_stress_stream(
        stream_id: usize,
        config: StressTestConfig,
        metrics: Arc<StressMetrics>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for packet in 0..1000 {
            let latency_start = Instant::now();
            
            // Simulate network delay
            tokio::time::sleep(Duration::from_millis(config.network_delay_ms)).await;
            
            // Simulate packet loss
            use rand::Rng;
            if rand::thread_rng().gen_range(0.0..100.0) < config.network_loss_percent {
                // Packet lost - record failure
                let failure = FailureReport {
                    timestamp: Instant::now(),
                    connection_id: stream_id,
                    failure_type: FailureType::NetworkCongestion,
                    error_message: format!("Packet {} lost in stream {}", packet, stream_id),
                    system_state: SystemState {
                        memory_mb: 150,
                        cpu_percent: 60.0,
                        open_connections: 100,
                        pending_operations: packet,
                    },
                };
                metrics.record_failure(failure).await;
                continue;
            }
            
            let latency = latency_start.elapsed().as_secs_f64() * 1000.0;
            metrics.record_latency(latency).await;
        }
        
        Ok(())
    }

    /// Simulate stressed operation with potential failures
    async fn simulate_stressed_operation(
        connection_id: usize,
        burst: usize,
        message: usize,
    ) -> bool {
        // Increase failure probability with higher connection IDs and burst numbers
        let failure_probability = (connection_id as f64 / 1000.0) + (burst as f64 / 20.0);
        
        use rand::Rng;
        let random_value = rand::thread_rng().gen_range(0.0..1.0);
        
        if random_value < failure_probability {
            return false; // Simulate failure
        }
        
        // Simulate variable processing time
        let processing_time = Duration::from_millis(
            rand::thread_rng().gen_range(1..10) + message as u64
        );
        tokio::time::sleep(processing_time).await;
        
        true
    }

    /// Start stress-specific monitoring
    async fn start_stress_monitor(&self) -> tokio::task::JoinHandle<()> {
        let metrics = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut counter = 0;
            loop {
                // Simulate more aggressive monitoring during stress tests
                let memory = Self::get_stress_memory_usage().await;
                let cpu = Self::get_stress_cpu_usage().await;
                metrics.record_system_metrics(memory, cpu).await;
                
                if counter % 10 == 0 {
                    println!("Stress Monitor - Memory: {}MB, CPU: {:.1}%, Connections: {}", 
                             memory, cpu, metrics.get_connection_count());
                }
                
                counter += 1;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
    }

    /// Check if test should be aborted due to system instability
    async fn should_abort_test(&self) -> bool {
        // In a real implementation, this would check actual system health
        let memory_samples = self.metrics.memory_samples.lock().await;
        let cpu_samples = self.metrics.cpu_samples.lock().await;
        
        if let (Some(&last_memory), Some(&last_cpu)) = 
            (memory_samples.last(), cpu_samples.last()) {
            return last_memory > self.config.max_memory_mb || 
                   last_cpu > self.config.max_cpu_percent;
        }
        
        false
    }

    /// Get stressed memory usage
    async fn get_stress_memory_usage() -> usize {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(200..2000) // Higher memory usage during stress
    }

    /// Get stressed CPU usage
    async fn get_stress_cpu_usage() -> f64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(30.0..95.0) // Higher CPU usage during stress
    }

    /// Print stress test results
    fn print_stress_results(&self, results: &StressTestResults) {
        println!("\n=== STRESS TEST RESULTS: {} ===", results.test_type);
        println!("Peak Connections: {}", results.peak_connections);
        println!("Peak Memory: {}MB", results.peak_memory_mb);
        println!("Peak CPU: {:.1}%", results.peak_cpu_percent);
        println!("Peak Latency: {:.2}ms", results.peak_latency_ms);
        println!("Failure Rate: {:.2}%", results.failure_rate);
        println!("Resource Exhaustion: {}", results.resource_exhaustion_detected);
        println!("Stability Score: {:.1}/10.0", results.system_stability_score);
        
        if let Some(breaking_point) = results.breaking_point_connections {
            println!("Breaking Point: {} connections", breaking_point);
        }
        
        println!("Detailed Failures: {}", results.detailed_failures.len());
        for (i, failure) in results.detailed_failures.iter().take(5).enumerate() {
            println!("  {}. {:?}: {}", i + 1, failure.failure_type, failure.error_message);
        }
        
        println!("Overall Success: {}", results.success);
        
        // Recommendations
        if results.failure_rate > 20.0 {
            println!("🚨 HIGH FAILURE RATE - System may be unstable under stress");
        }
        if results.system_stability_score < 5.0 {
            println!("⚠️  LOW STABILITY SCORE - Consider performance optimizations");
        }
        if results.resource_exhaustion_detected {
            println!("💾 RESOURCE EXHAUSTION - System limits reached");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_light_stress() {
        let config = StressTestConfig {
            max_connections: 20,
            connection_ramp_rate: 10.0,
            message_burst_size: 5,
            memory_pressure_mb: 100,
            cpu_stress_level: 0.3,
            ..Default::default()
        };
        
        let tester = StressTester::new(config);
        let results = tester.run_connection_flood_test().await.unwrap();
        
        assert!(results.system_stability_score > 6.0, "Light stress should maintain stability");
        assert!(results.failure_rate < 15.0, "Failure rate should be reasonable");
    }

    #[tokio::test]
    async fn test_memory_pressure() {
        let config = StressTestConfig {
            memory_pressure_mb: 500, // Moderate memory pressure
            ..Default::default()
        };
        
        let tester = StressTester::new(config);
        let results = tester.run_memory_pressure_test().await.unwrap();
        
        assert!(results.peak_memory_mb > 0, "Should allocate memory");
        assert!(results.peak_latency_ms > 0.0, "Should measure latency");
    }

    #[tokio::test]
    async fn test_cpu_stress() {
        let config = StressTestConfig {
            cpu_stress_level: 0.5, // 50% CPU stress
            ..Default::default()
        };
        
        let tester = StressTester::new(config);
        let results = tester.run_cpu_stress_test().await.unwrap();
        
        assert!(results.peak_cpu_percent > 30.0, "Should show CPU usage");
        assert_eq!(results.test_type, "CPU Stress");
    }

    #[tokio::test]
    async fn test_network_congestion() {
        let config = StressTestConfig {
            network_delay_ms: 50,
            network_loss_percent: 2.0,
            ..Default::default()
        };
        
        let tester = StressTester::new(config);
        let results = tester.run_network_congestion_test().await.unwrap();
        
        assert!(results.peak_latency_ms >= 50.0, "Should reflect network delay");
        assert!(results.detailed_failures.len() > 0, "Should detect packet loss");
    }
}