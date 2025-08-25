//! Production Load Testing Framework for BitCraps
//! Supports 1000+ concurrent users with comprehensive metrics

use std::sync::{Arc, atomic::{AtomicUsize, AtomicU64, Ordering}};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use tokio::time::{sleep, interval};
use tokio::sync::{RwLock, Semaphore, mpsc};
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use futures::future::join_all;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::mesh::service::MeshService;
use crate::protocol::craps::CrapsGame;
use crate::transport::connection_pool::ConnectionPool;
use crate::monitoring::metrics::METRICS;

/// Load testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestConfig {
    /// Number of concurrent users
    pub concurrent_users: usize,
    /// Test duration in seconds
    pub duration_seconds: u64,
    /// Ramp-up time in seconds
    pub ramp_up_seconds: u64,
    /// Target operations per second
    pub target_ops_per_second: u64,
    /// Maximum latency threshold (ms)
    pub max_latency_ms: u64,
    /// Error rate threshold (%)
    pub max_error_rate: f64,
    /// Resource limits
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in MB
    pub max_memory_mb: u64,
    /// Maximum CPU usage percentage
    pub max_cpu_percent: f64,
    /// Maximum open connections
    pub max_connections: usize,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            concurrent_users: 1000,
            duration_seconds: 300, // 5 minutes
            ramp_up_seconds: 60,    // 1 minute ramp-up
            target_ops_per_second: 10000,
            max_latency_ms: 500,
            max_error_rate: 1.0,
            resource_limits: ResourceLimits {
                max_memory_mb: 2048,
                max_cpu_percent: 80.0,
                max_connections: 5000,
            },
        }
    }
}

/// Load test orchestrator
pub struct LoadTestOrchestrator {
    config: LoadTestConfig,
    mesh_service: Arc<MeshService>,
    connection_pool: Arc<ConnectionPool>,
    active_users: AtomicUsize,
    total_operations: AtomicU64,
    total_errors: AtomicU64,
    latency_samples: Arc<RwLock<Vec<u64>>>,
    resource_monitor: Arc<ResourceMonitor>,
    test_start_time: Instant,
    results: Arc<RwLock<LoadTestResults>>,
}

impl LoadTestOrchestrator {
    pub fn new(
        config: LoadTestConfig,
        mesh_service: Arc<MeshService>,
        connection_pool: Arc<ConnectionPool>,
    ) -> Self {
        Self {
            config,
            mesh_service,
            connection_pool,
            active_users: AtomicUsize::new(0),
            total_operations: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            latency_samples: Arc::new(RwLock::new(Vec::new())),
            resource_monitor: Arc::new(ResourceMonitor::new()),
            test_start_time: Instant::now(),
            results: Arc::new(RwLock::new(LoadTestResults::new())),
        }
    }

    /// Execute comprehensive load test
    pub async fn execute_load_test(&self) -> Result<LoadTestResults, LoadTestError> {
        tracing::info!("Starting load test with {} concurrent users", self.config.concurrent_users);
        
        // Start resource monitoring
        let resource_monitor = Arc::clone(&self.resource_monitor);
        let resource_task = tokio::spawn(async move {
            resource_monitor.start_monitoring().await;
        });

        // Start metrics collection
        let metrics_task = self.start_metrics_collection();

        // Execute load test phases
        let load_test_result = self.run_load_test_phases().await;

        // Stop monitoring
        resource_monitor.stop_monitoring();
        resource_task.abort();
        metrics_task.abort();

        // Compile results
        let final_results = self.compile_results().await;
        
        match load_test_result {
            Ok(_) => Ok(final_results),
            Err(e) => {
                tracing::error!("Load test failed: {:?}", e);
                Err(e)
            }
        }
    }

    /// Run load test in phases: ramp-up, steady-state, ramp-down
    async fn run_load_test_phases(&self) -> Result<(), LoadTestError> {
        // Phase 1: Ramp-up
        tracing::info!("Phase 1: Ramp-up ({} seconds)", self.config.ramp_up_seconds);
        self.ramp_up_phase().await?;

        // Phase 2: Steady-state
        let steady_duration = self.config.duration_seconds - self.config.ramp_up_seconds - 30;
        tracing::info!("Phase 2: Steady-state ({} seconds)", steady_duration);
        self.steady_state_phase(steady_duration).await?;

        // Phase 3: Ramp-down
        tracing::info!("Phase 3: Ramp-down (30 seconds)");
        self.ramp_down_phase().await?;

        Ok(())
    }

    /// Ramp-up phase: gradually increase load
    async fn ramp_up_phase(&self) -> Result<(), LoadTestError> {
        let ramp_interval = Duration::from_secs(self.config.ramp_up_seconds) / self.config.concurrent_users as u32;
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_users));

        for user_id in 0..self.config.concurrent_users {
            let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();
            let user_sim = VirtualUser::new(
                user_id,
                Arc::clone(&self.mesh_service),
                Arc::clone(&self.connection_pool),
                Arc::clone(&self.latency_samples),
            );

            let total_ops = Arc::clone(&self.total_operations);
            let total_errors = Arc::clone(&self.total_errors);
            let active_users = Arc::clone(&self.active_users);

            tokio::spawn(async move {
                active_users.fetch_add(1, Ordering::Relaxed);
                
                if let Err(e) = user_sim.simulate_user_behavior().await {
                    total_errors.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!("User {} simulation error: {:?}", user_id, e);
                } else {
                    total_ops.fetch_add(1, Ordering::Relaxed);
                }

                active_users.fetch_sub(1, Ordering::Relaxed);
                drop(permit);
            });

            sleep(ramp_interval).await;
        }

        Ok(())
    }

    /// Steady-state phase: maintain constant load
    async fn steady_state_phase(&self, duration_seconds: u64) -> Result<(), LoadTestError> {
        let start_time = Instant::now();
        let end_time = start_time + Duration::from_secs(duration_seconds);

        // Monitor performance during steady state
        let mut interval = interval(Duration::from_secs(10));

        while Instant::now() < end_time {
            interval.tick().await;
            
            // Check resource limits
            if let Err(e) = self.check_resource_limits().await {
                tracing::error!("Resource limit exceeded: {:?}", e);
                return Err(e);
            }

            // Check performance thresholds
            if let Err(e) = self.check_performance_thresholds().await {
                tracing::error!("Performance threshold exceeded: {:?}", e);
                return Err(e);
            }

            tracing::info!(
                "Steady-state: {} active users, {} ops/sec, avg latency: {:.2}ms",
                self.active_users.load(Ordering::Relaxed),
                self.calculate_ops_per_second(),
                self.calculate_average_latency().await
            );
        }

        Ok(())
    }

    /// Ramp-down phase: gradually reduce load
    async fn ramp_down_phase(&self) -> Result<(), LoadTestError> {
        // Wait for all users to complete their operations
        let mut check_interval = interval(Duration::from_secs(1));
        let timeout = Instant::now() + Duration::from_secs(30);

        while self.active_users.load(Ordering::Relaxed) > 0 && Instant::now() < timeout {
            check_interval.tick().await;
            tracing::info!(
                "Ramp-down: {} users remaining",
                self.active_users.load(Ordering::Relaxed)
            );
        }

        if self.active_users.load(Ordering::Relaxed) > 0 {
            tracing::warn!("Some users did not complete within timeout");
        }

        Ok(())
    }

    /// Check if resource limits are exceeded
    async fn check_resource_limits(&self) -> Result<(), LoadTestError> {
        let usage = self.resource_monitor.get_current_usage().await;

        if usage.memory_mb > self.config.resource_limits.max_memory_mb {
            return Err(LoadTestError::ResourceLimitExceeded(
                format!("Memory usage {}MB exceeds limit {}MB", 
                    usage.memory_mb, self.config.resource_limits.max_memory_mb)
            ));
        }

        if usage.cpu_percent > self.config.resource_limits.max_cpu_percent {
            return Err(LoadTestError::ResourceLimitExceeded(
                format!("CPU usage {:.1}% exceeds limit {:.1}%", 
                    usage.cpu_percent, self.config.resource_limits.max_cpu_percent)
            ));
        }

        if usage.connection_count > self.config.resource_limits.max_connections {
            return Err(LoadTestError::ResourceLimitExceeded(
                format!("Connection count {} exceeds limit {}", 
                    usage.connection_count, self.config.resource_limits.max_connections)
            ));
        }

        Ok(())
    }

    /// Check if performance thresholds are exceeded
    async fn check_performance_thresholds(&self) -> Result<(), LoadTestError> {
        let avg_latency = self.calculate_average_latency().await;
        if avg_latency > self.config.max_latency_ms as f64 {
            return Err(LoadTestError::PerformanceThresholdExceeded(
                format!("Average latency {:.2}ms exceeds limit {}ms", 
                    avg_latency, self.config.max_latency_ms)
            ));
        }

        let error_rate = self.calculate_error_rate();
        if error_rate > self.config.max_error_rate {
            return Err(LoadTestError::PerformanceThresholdExceeded(
                format!("Error rate {:.2}% exceeds limit {:.2}%", 
                    error_rate, self.config.max_error_rate)
            ));
        }

        Ok(())
    }

    /// Calculate operations per second
    fn calculate_ops_per_second(&self) -> f64 {
        let elapsed = self.test_start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_operations.load(Ordering::Relaxed) as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Calculate average latency
    async fn calculate_average_latency(&self) -> f64 {
        let samples = self.latency_samples.read().await;
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<u64>() as f64 / samples.len() as f64
        }
    }

    /// Calculate error rate percentage
    fn calculate_error_rate(&self) -> f64 {
        let total_ops = self.total_operations.load(Ordering::Relaxed);
        let total_errors = self.total_errors.load(Ordering::Relaxed);
        if total_ops > 0 {
            (total_errors as f64 / total_ops as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Start metrics collection task
    fn start_metrics_collection(&self) -> tokio::task::JoinHandle<()> {
        let results = Arc::clone(&self.results);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                // Collect current metrics
                let snapshot = LoadTestMetrics {
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    active_connections: METRICS.network.active_connections.load(Ordering::Relaxed),
                    messages_per_second: METRICS.network.messages_sent.load(Ordering::Relaxed), // Simplified
                    average_latency_ms: METRICS.consensus.average_latency_ms(),
                    memory_usage_mb: METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024,
                    cpu_usage_percent: METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64,
                };

                results.write().await.add_metric_snapshot(snapshot);
            }
        })
    }

    /// Compile final test results
    async fn compile_results(&self) -> LoadTestResults {
        let mut results = self.results.write().await;
        results.test_duration_seconds = self.test_start_time.elapsed().as_secs();
        results.total_operations = self.total_operations.load(Ordering::Relaxed);
        results.total_errors = self.total_errors.load(Ordering::Relaxed);
        results.final_ops_per_second = self.calculate_ops_per_second();
        results.final_error_rate = self.calculate_error_rate();
        results.average_latency_ms = self.calculate_average_latency().await;
        
        // Calculate percentiles
        let samples = self.latency_samples.read().await;
        if !samples.is_empty() {
            let mut sorted_samples = samples.clone();
            sorted_samples.sort();
            results.latency_p95_ms = sorted_samples[(sorted_samples.len() as f64 * 0.95) as usize] as f64;
            results.latency_p99_ms = sorted_samples[(sorted_samples.len() as f64 * 0.99) as usize] as f64;
        }

        results.clone()
    }
}

/// Virtual user simulator
pub struct VirtualUser {
    id: usize,
    mesh_service: Arc<MeshService>,
    connection_pool: Arc<ConnectionPool>,
    latency_samples: Arc<RwLock<Vec<u64>>>,
    rng: ChaCha8Rng,
}

impl VirtualUser {
    pub fn new(
        id: usize,
        mesh_service: Arc<MeshService>,
        connection_pool: Arc<ConnectionPool>,
        latency_samples: Arc<RwLock<Vec<u64>>>,
    ) -> Self {
        Self {
            id,
            mesh_service,
            connection_pool,
            latency_samples,
            rng: ChaCha8Rng::seed_from_u64(id as u64),
        }
    }

    /// Simulate realistic user behavior
    pub async fn simulate_user_behavior(&mut self) -> Result<(), VirtualUserError> {
        let operations = vec![
            UserOperation::Connect,
            UserOperation::JoinGame,
            UserOperation::PlaceBet(self.rng.gen_range(10..=1000)),
            UserOperation::PlayGame,
            UserOperation::LeaveGame,
            UserOperation::Disconnect,
        ];

        for operation in operations {
            let start_time = Instant::now();
            
            match self.execute_operation(operation).await {
                Ok(_) => {
                    let latency_ms = start_time.elapsed().as_millis() as u64;
                    self.latency_samples.write().await.push(latency_ms);
                    
                    // Random think time between operations
                    let think_time = Duration::from_millis(self.rng.gen_range(100..=500));
                    sleep(think_time).await;
                },
                Err(e) => {
                    tracing::warn!("User {} operation failed: {:?}", self.id, e);
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Execute a specific user operation
    async fn execute_operation(&mut self, operation: UserOperation) -> Result<(), VirtualUserError> {
        match operation {
            UserOperation::Connect => {
                // Simulate connection to mesh network
                self.connection_pool.get_connection().await
                    .map_err(|_| VirtualUserError::ConnectionFailed)?;
                Ok(())
            },
            UserOperation::JoinGame => {
                // Simulate joining a game
                sleep(Duration::from_millis(50)).await; // Simulate processing time
                Ok(())
            },
            UserOperation::PlaceBet(amount) => {
                // Simulate placing a bet
                sleep(Duration::from_millis(30)).await;
                tracing::debug!("User {} placed bet: {}", self.id, amount);
                Ok(())
            },
            UserOperation::PlayGame => {
                // Simulate playing the game
                sleep(Duration::from_millis(100)).await;
                Ok(())
            },
            UserOperation::LeaveGame => {
                // Simulate leaving the game
                sleep(Duration::from_millis(20)).await;
                Ok(())
            },
            UserOperation::Disconnect => {
                // Simulate disconnection
                Ok(())
            },
        }
    }
}

/// User operations for simulation
#[derive(Debug, Clone)]
pub enum UserOperation {
    Connect,
    JoinGame,
    PlaceBet(u64),
    PlayGame,
    LeaveGame,
    Disconnect,
}

/// Resource monitoring
pub struct ResourceMonitor {
    monitoring: Arc<AtomicUsize>,
    current_usage: Arc<RwLock<ResourceUsage>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            monitoring: Arc::new(AtomicUsize::new(0)),
            current_usage: Arc::new(RwLock::new(ResourceUsage::default())),
        }
    }

    pub async fn start_monitoring(&self) {
        self.monitoring.store(1, Ordering::Relaxed);
        let mut interval = interval(Duration::from_secs(1));

        while self.monitoring.load(Ordering::Relaxed) == 1 {
            interval.tick().await;
            
            let usage = self.collect_system_metrics().await;
            *self.current_usage.write().await = usage;
        }
    }

    pub fn stop_monitoring(&self) {
        self.monitoring.store(0, Ordering::Relaxed);
    }

    pub async fn get_current_usage(&self) -> ResourceUsage {
        self.current_usage.read().await.clone()
    }

    async fn collect_system_metrics(&self) -> ResourceUsage {
        // In a real implementation, you'd collect actual system metrics
        // For now, we'll use the global metrics
        ResourceUsage {
            memory_mb: METRICS.resources.memory_usage_bytes.load(Ordering::Relaxed) / 1024 / 1024,
            cpu_percent: METRICS.resources.cpu_usage_percent.load(Ordering::Relaxed) as f64,
            connection_count: METRICS.network.active_connections.load(Ordering::Relaxed),
        }
    }
}

/// Resource usage data
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    pub memory_mb: u64,
    pub cpu_percent: f64,
    pub connection_count: usize,
}

/// Load test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResults {
    pub test_duration_seconds: u64,
    pub total_operations: u64,
    pub total_errors: u64,
    pub final_ops_per_second: f64,
    pub final_error_rate: f64,
    pub average_latency_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub max_memory_usage_mb: u64,
    pub max_cpu_usage_percent: f64,
    pub max_connections: usize,
    pub metric_snapshots: Vec<LoadTestMetrics>,
    pub success: bool,
    pub failure_reason: Option<String>,
}

impl LoadTestResults {
    pub fn new() -> Self {
        Self {
            test_duration_seconds: 0,
            total_operations: 0,
            total_errors: 0,
            final_ops_per_second: 0.0,
            final_error_rate: 0.0,
            average_latency_ms: 0.0,
            latency_p95_ms: 0.0,
            latency_p99_ms: 0.0,
            max_memory_usage_mb: 0,
            max_cpu_usage_percent: 0.0,
            max_connections: 0,
            metric_snapshots: Vec::new(),
            success: true,
            failure_reason: None,
        }
    }

    pub fn add_metric_snapshot(&mut self, snapshot: LoadTestMetrics) {
        self.max_memory_usage_mb = self.max_memory_usage_mb.max(snapshot.memory_usage_mb);
        self.max_cpu_usage_percent = self.max_cpu_usage_percent.max(snapshot.cpu_usage_percent);
        self.max_connections = self.max_connections.max(snapshot.active_connections);
        self.metric_snapshots.push(snapshot);
    }
}

/// Load test metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestMetrics {
    pub timestamp: u64,
    pub active_connections: usize,
    pub messages_per_second: u64,
    pub average_latency_ms: f64,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f64,
}

/// Load test errors
#[derive(Debug, Clone)]
pub enum LoadTestError {
    ResourceLimitExceeded(String),
    PerformanceThresholdExceeded(String),
    TestSetupFailed(String),
    NetworkError(String),
}

/// Virtual user errors
#[derive(Debug, Clone)]
pub enum VirtualUserError {
    ConnectionFailed,
    OperationTimeout,
    NetworkError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_load_test_config_default() {
        let config = LoadTestConfig::default();
        assert_eq!(config.concurrent_users, 1000);
        assert_eq!(config.duration_seconds, 300);
    }

    #[tokio::test]
    async fn test_virtual_user_creation() {
        let mesh_service = Arc::new(MeshService::new().await.unwrap());
        let connection_pool = Arc::new(ConnectionPool::new(100));
        let latency_samples = Arc::new(RwLock::new(Vec::new()));

        let user = VirtualUser::new(1, mesh_service, connection_pool, latency_samples);
        assert_eq!(user.id, 1);
    }

    #[tokio::test]
    async fn test_resource_monitor() {
        let monitor = ResourceMonitor::new();
        let usage = monitor.get_current_usage().await;
        assert_eq!(usage.memory_mb, 0);
    }

    #[tokio::test]
    async fn test_load_test_results() {
        let mut results = LoadTestResults::new();
        assert!(results.success);
        
        let snapshot = LoadTestMetrics {
            timestamp: 12345,
            active_connections: 100,
            messages_per_second: 500,
            average_latency_ms: 25.0,
            memory_usage_mb: 512,
            cpu_usage_percent: 45.0,
        };
        
        results.add_metric_snapshot(snapshot);
        assert_eq!(results.max_memory_usage_mb, 512);
        assert_eq!(results.metric_snapshots.len(), 1);
    }
}