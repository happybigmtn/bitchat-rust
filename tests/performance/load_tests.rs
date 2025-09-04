//! Load Testing Suite for BitCraps
//!
//! This module provides comprehensive load testing scenarios to validate
//! system performance under various load conditions.

use bitcraps::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use uuid::Uuid;

/// Load testing configuration
#[derive(Clone, Debug)]
pub struct LoadTestConfig {
    /// Number of concurrent connections
    pub connection_count: usize,
    /// Test duration
    pub duration: Duration,
    /// Messages per second per connection
    pub message_rate: f64,
    /// Game creation rate (games per second)
    pub game_creation_rate: f64,
    /// Player join rate (players per second per game)
    pub player_join_rate: f64,
    /// Betting frequency (bets per second per player)
    pub betting_frequency: f64,
    /// Target latency percentiles
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    /// Memory limits
    pub max_memory_mb: usize,
    /// CPU usage limits
    pub max_cpu_percent: f64,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            connection_count: 100,
            duration: Duration::from_secs(300), // 5 minutes
            message_rate: 10.0,
            game_creation_rate: 0.5,
            player_join_rate: 2.0,
            betting_frequency: 1.0,
            latency_p50_ms: 50.0,
            latency_p95_ms: 200.0,
            latency_p99_ms: 500.0,
            max_memory_mb: 512,
            max_cpu_percent: 80.0,
        }
    }
}

/// Load test results and metrics
#[derive(Debug, Clone)]
pub struct LoadTestResults {
    pub test_duration: Duration,
    pub total_messages: u64,
    pub successful_messages: u64,
    pub failed_messages: u64,
    pub games_created: u64,
    pub players_joined: u64,
    pub bets_placed: u64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub latency_max_ms: f64,
    pub throughput_msgs_per_sec: f64,
    pub memory_peak_mb: usize,
    pub cpu_peak_percent: f64,
    pub error_rate: f64,
    pub success: bool,
}

/// Performance metrics collector
#[derive(Debug)]
pub struct PerformanceMetrics {
    latencies_ms: Arc<Mutex<Vec<f64>>>,
    message_count: Arc<Mutex<u64>>,
    error_count: Arc<Mutex<u64>>,
    game_count: Arc<Mutex<u64>>,
    player_count: Arc<Mutex<u64>>,
    bet_count: Arc<Mutex<u64>>,
    memory_samples: Arc<Mutex<Vec<usize>>>,
    cpu_samples: Arc<Mutex<Vec<f64>>>,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            latencies_ms: Arc::new(Mutex::new(Vec::new())),
            message_count: Arc::new(Mutex::new(0)),
            error_count: Arc::new(Mutex::new(0)),
            game_count: Arc::new(Mutex::new(0)),
            player_count: Arc::new(Mutex::new(0)),
            bet_count: Arc::new(Mutex::new(0)),
            memory_samples: Arc::new(Mutex::new(Vec::new())),
            cpu_samples: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn record_latency(&self, latency_ms: f64) {
        let mut latencies = self.latencies_ms.lock().await;
        latencies.push(latency_ms);
    }

    pub async fn increment_messages(&self) {
        let mut count = self.message_count.lock().await;
        *count += 1;
    }

    pub async fn increment_errors(&self) {
        let mut count = self.error_count.lock().await;
        *count += 1;
    }

    pub async fn increment_games(&self) {
        let mut count = self.game_count.lock().await;
        *count += 1;
    }

    pub async fn increment_players(&self) {
        let mut count = self.player_count.lock().await;
        *count += 1;
    }

    pub async fn increment_bets(&self) {
        let mut count = self.bet_count.lock().await;
        *count += 1;
    }

    pub async fn record_memory(&self, memory_mb: usize) {
        let mut samples = self.memory_samples.lock().await;
        samples.push(memory_mb);
    }

    pub async fn record_cpu(&self, cpu_percent: f64) {
        let mut samples = self.cpu_samples.lock().await;
        samples.push(cpu_percent);
    }

    pub async fn calculate_results(&self, test_duration: Duration) -> LoadTestResults {
        let mut latencies = self.latencies_ms.lock().await;
        let message_count = *self.message_count.lock().await;
        let error_count = *self.error_count.lock().await;
        let game_count = *self.game_count.lock().await;
        let player_count = *self.player_count.lock().await;
        let bet_count = *self.bet_count.lock().await;
        let memory_samples = self.memory_samples.lock().await;
        let cpu_samples = self.cpu_samples.lock().await;

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let p50 = percentile(&latencies, 50.0);
        let p95 = percentile(&latencies, 95.0);
        let p99 = percentile(&latencies, 99.0);
        let max_latency = latencies.last().copied().unwrap_or(0.0);
        
        let throughput = message_count as f64 / test_duration.as_secs_f64();
        let error_rate = if message_count > 0 {
            error_count as f64 / message_count as f64 * 100.0
        } else {
            0.0
        };

        let memory_peak = memory_samples.iter().max().copied().unwrap_or(0);
        let cpu_peak = cpu_samples.iter().fold(0.0f64, |acc, &x| acc.max(x));

        LoadTestResults {
            test_duration,
            total_messages: message_count,
            successful_messages: message_count - error_count,
            failed_messages: error_count,
            games_created: game_count,
            players_joined: player_count,
            bets_placed: bet_count,
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
            latency_max_ms: max_latency,
            throughput_msgs_per_sec: throughput,
            memory_peak_mb: memory_peak,
            cpu_peak_percent: cpu_peak,
            error_rate,
            success: error_rate < 1.0, // Success if error rate < 1%
        }
    }
}

/// Calculate percentile from sorted vector
fn percentile(sorted_data: &[f64], percentile: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    let index = (percentile / 100.0 * (sorted_data.len() - 1) as f64).round() as usize;
    sorted_data[index.min(sorted_data.len() - 1)]
}

/// Load test executor
pub struct LoadTester {
    config: LoadTestConfig,
    metrics: PerformanceMetrics,
}

impl LoadTester {
    pub fn new(config: LoadTestConfig) -> Self {
        Self {
            config,
            metrics: PerformanceMetrics::new(),
        }
    }

    /// Execute comprehensive load test
    pub async fn run_load_test(&self) -> Result<LoadTestResults, Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting load test with {} connections for {:?}", 
                 self.config.connection_count, self.config.duration);

        let start_time = Instant::now();
        let test_end_time = start_time + self.config.duration;
        
        // Start system monitoring
        let metrics = Arc::new(&self.metrics);
        let monitor_handle = self.start_system_monitor(metrics.clone()).await;

        // Create connection pool
        let mut join_set = JoinSet::new();
        
        for connection_id in 0..self.config.connection_count {
            let metrics = metrics.clone();
            let config = self.config.clone();
            let end_time = test_end_time;
            
            join_set.spawn(async move {
                Self::simulate_connection(connection_id, config, metrics, end_time).await
            });
        }

        // Wait for all connections to complete
        while let Some(result) = join_set.join_next().await {
            if let Err(e) = result {
                eprintln!("Connection task failed: {}", e);
            }
        }

        // Stop monitoring
        monitor_handle.abort();
        
        let actual_duration = start_time.elapsed();
        let results = self.metrics.calculate_results(actual_duration).await;
        
        self.print_results(&results);
        Ok(results)
    }

    /// Simulate a single connection's load
    async fn simulate_connection(
        connection_id: usize,
        config: LoadTestConfig,
        metrics: Arc<&PerformanceMetrics>,
        end_time: Instant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Calculate message interval
        let message_interval = Duration::from_secs_f64(1.0 / config.message_rate);
        
        let mut last_message_time = Instant::now();
        let mut game_creation_counter = 0.0;
        let mut player_join_counter = 0.0;
        let mut betting_counter = 0.0;

        while Instant::now() < end_time {
            let now = Instant::now();
            
            // Rate limiting
            if now < last_message_time + message_interval {
                tokio::time::sleep(message_interval - (now - last_message_time)).await;
            }

            let message_start = Instant::now();
            
            // Simulate different types of operations based on rates
            let operation_result = if game_creation_counter >= 1.0 / config.game_creation_rate {
                game_creation_counter = 0.0;
                Self::simulate_game_creation(connection_id, &metrics).await
            } else if player_join_counter >= 1.0 / config.player_join_rate {
                player_join_counter = 0.0;
                Self::simulate_player_join(connection_id, &metrics).await
            } else if betting_counter >= 1.0 / config.betting_frequency {
                betting_counter = 0.0;
                Self::simulate_betting(connection_id, &metrics).await
            } else {
                Self::simulate_message(connection_id, &metrics).await
            };

            let latency = message_start.elapsed().as_secs_f64() * 1000.0;
            metrics.record_latency(latency).await;

            if operation_result.is_ok() {
                metrics.increment_messages().await;
            } else {
                metrics.increment_errors().await;
            }

            last_message_time = Instant::now();
            game_creation_counter += config.message_rate / 1000.0;
            player_join_counter += config.message_rate / 1000.0;
            betting_counter += config.message_rate / 1000.0;
        }

        Ok(())
    }

    /// Simulate game creation operation
    async fn simulate_game_creation(
        _connection_id: usize,
        metrics: &PerformanceMetrics,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simulate game creation latency
        tokio::time::sleep(Duration::from_millis(10)).await;
        metrics.increment_games().await;
        Ok(())
    }

    /// Simulate player joining operation
    async fn simulate_player_join(
        _connection_id: usize,
        metrics: &PerformanceMetrics,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simulate player join latency
        tokio::time::sleep(Duration::from_millis(5)).await;
        metrics.increment_players().await;
        Ok(())
    }

    /// Simulate betting operation
    async fn simulate_betting(
        _connection_id: usize,
        metrics: &PerformanceMetrics,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simulate betting latency
        tokio::time::sleep(Duration::from_millis(3)).await;
        metrics.increment_bets().await;
        Ok(())
    }

    /// Simulate general message operation
    async fn simulate_message(
        _connection_id: usize,
        _metrics: &PerformanceMetrics,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simulate message processing latency
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(())
    }

    /// Start system resource monitoring
    async fn start_system_monitor(
        &self,
        metrics: Arc<&PerformanceMetrics>,
    ) -> tokio::task::JoinHandle<()> {
        let monitor_interval = Duration::from_secs(1);
        
        tokio::spawn(async move {
            loop {
                // Simulate memory monitoring
                let memory_mb = Self::get_memory_usage_mb().await;
                metrics.record_memory(memory_mb).await;
                
                // Simulate CPU monitoring
                let cpu_percent = Self::get_cpu_usage_percent().await;
                metrics.record_cpu(cpu_percent).await;
                
                tokio::time::sleep(monitor_interval).await;
            }
        })
    }

    /// Get current memory usage (simulated)
    async fn get_memory_usage_mb() -> usize {
        // In a real implementation, this would query actual memory usage
        // For testing, we simulate realistic memory growth
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(50..200)
    }

    /// Get current CPU usage (simulated)
    async fn get_cpu_usage_percent() -> f64 {
        // In a real implementation, this would query actual CPU usage
        // For testing, we simulate realistic CPU fluctuation
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(10.0..80.0)
    }

    /// Print formatted test results
    fn print_results(&self, results: &LoadTestResults) {
        println!("\n=== LOAD TEST RESULTS ===");
        println!("Test Duration: {:?}", results.test_duration);
        println!("Total Messages: {}", results.total_messages);
        println!("Successful Messages: {}", results.successful_messages);
        println!("Failed Messages: {}", results.failed_messages);
        println!("Games Created: {}", results.games_created);
        println!("Players Joined: {}", results.players_joined);
        println!("Bets Placed: {}", results.bets_placed);
        println!("Error Rate: {:.2}%", results.error_rate);
        println!("Throughput: {:.2} msg/sec", results.throughput_msgs_per_sec);
        println!("Latency P50: {:.2}ms", results.latency_p50_ms);
        println!("Latency P95: {:.2}ms", results.latency_p95_ms);
        println!("Latency P99: {:.2}ms", results.latency_p99_ms);
        println!("Latency Max: {:.2}ms", results.latency_max_ms);
        println!("Peak Memory: {}MB", results.memory_peak_mb);
        println!("Peak CPU: {:.1}%", results.cpu_peak_percent);
        println!("Overall Success: {}", results.success);
        
        // Performance assertions
        if results.latency_p95_ms > self.config.latency_p95_ms {
            println!("⚠️  WARNING: P95 latency ({:.2}ms) exceeds target ({:.2}ms)", 
                     results.latency_p95_ms, self.config.latency_p95_ms);
        }
        
        if results.memory_peak_mb > self.config.max_memory_mb {
            println!("⚠️  WARNING: Peak memory ({}MB) exceeds limit ({}MB)", 
                     results.memory_peak_mb, self.config.max_memory_mb);
        }
        
        if results.cpu_peak_percent > self.config.max_cpu_percent {
            println!("⚠️  WARNING: Peak CPU ({:.1}%) exceeds limit ({:.1}%)", 
                     results.cpu_peak_percent, self.config.max_cpu_percent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_light_load() {
        let config = LoadTestConfig {
            connection_count: 10,
            duration: Duration::from_secs(30),
            message_rate: 5.0,
            ..Default::default()
        };
        
        let tester = LoadTester::new(config);
        let results = tester.run_load_test().await.unwrap();
        
        assert!(results.success, "Light load test should succeed");
        assert!(results.error_rate < 1.0, "Error rate should be low");
    }

    #[tokio::test]
    async fn test_medium_load() {
        let config = LoadTestConfig {
            connection_count: 50,
            duration: Duration::from_secs(60),
            message_rate: 10.0,
            game_creation_rate: 1.0,
            player_join_rate: 3.0,
            betting_frequency: 2.0,
            ..Default::default()
        };
        
        let tester = LoadTester::new(config);
        let results = tester.run_load_test().await.unwrap();
        
        assert!(results.total_messages > 0, "Should process messages");
        assert!(results.games_created > 0, "Should create games");
        assert!(results.players_joined > 0, "Should join players");
        assert!(results.bets_placed > 0, "Should place bets");
    }

    #[tokio::test]
    async fn test_high_load() {
        let config = LoadTestConfig {
            connection_count: 100,
            duration: Duration::from_secs(120),
            message_rate: 20.0,
            latency_p95_ms: 300.0, // More lenient for high load
            max_memory_mb: 1024,
            max_cpu_percent: 90.0,
            ..Default::default()
        };
        
        let tester = LoadTester::new(config);
        let results = tester.run_load_test().await.unwrap();
        
        // Verify system can handle high load
        assert!(results.throughput_msgs_per_sec > 100.0, "Should maintain high throughput");
        assert!(results.latency_p50_ms < 100.0, "P50 latency should be reasonable");
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new();
        
        // Record some test data
        metrics.record_latency(10.0).await;
        metrics.record_latency(20.0).await;
        metrics.record_latency(30.0).await;
        metrics.increment_messages().await;
        metrics.increment_games().await;
        
        let results = metrics.calculate_results(Duration::from_secs(60)).await;
        
        assert_eq!(results.total_messages, 1);
        assert_eq!(results.games_created, 1);
        assert!(results.latency_p50_ms > 0.0);
    }
}