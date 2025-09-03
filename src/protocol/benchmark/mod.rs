//! Comprehensive benchmarks for efficient BitCraps game logic
//!
//! This module provides extensive performance benchmarks for all the optimized
//! data structures and algorithms, measuring both memory usage and CPU cycles
//! to validate the efficiency improvements.

#[cfg(feature = "benchmarks")]
pub mod crypto_benchmarks;
#[cfg(feature = "benchmarks")]
pub mod game_benchmarks;
#[cfg(feature = "benchmarks")]
pub mod network_benchmarks;
#[cfg(feature = "benchmarks")]
pub mod state_benchmarks;

#[cfg(feature = "benchmarks")]
use criterion::Criterion;
use std::time::Duration;

/// Benchmark results with detailed metrics
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Test name
    pub name: String,

    /// Average execution time
    pub avg_time: Duration,

    /// Minimum execution time
    pub min_time: Duration,

    /// Maximum execution time
    pub max_time: Duration,

    /// Throughput (operations per second)
    pub throughput: f64,

    /// Memory usage statistics
    pub memory_stats: MemoryBenchmarkStats,

    /// Improvement factor over baseline
    pub improvement_factor: f64,
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryBenchmarkStats {
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,

    /// Average memory usage in bytes
    pub avg_memory_bytes: usize,

    /// Memory allocations count
    pub allocations: u64,

    /// Memory deallocations count
    pub deallocations: u64,

    /// Cache efficiency metrics
    pub cache_hit_rate: f64,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations for each test
    pub iterations: usize,

    /// Number of warmup iterations
    pub warmup_iterations: usize,

    /// Enable memory profiling
    pub profile_memory: bool,

    /// Enable CPU profiling
    pub profile_cpu: bool,

    /// Test data size scaling factors
    pub small_dataset: usize,
    pub medium_dataset: usize,
    pub large_dataset: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 1000,
            warmup_iterations: 100,
            profile_memory: true,
            profile_cpu: true,
            small_dataset: 100,
            medium_dataset: 1000,
            large_dataset: 10000,
        }
    }
}

/// Comprehensive benchmark suite for BitCraps optimizations
pub struct BitCrapsBenchmarks;

impl BitCrapsBenchmarks {
    /// Run all benchmark suites
    pub fn run_all_benchmarks() -> Vec<BenchmarkResults> {
        let config = BenchmarkConfig::default();
        let mut results = Vec::new();

        println!("🎲 Starting BitCraps Performance Benchmarks");
        println!("============================================");

        // Game state benchmarks
        results.extend(game_benchmarks::benchmark_compact_game_state(&config));

        // Bet resolution benchmarks
        results.extend(game_benchmarks::benchmark_bet_resolution(&config));

        // Consensus benchmarks
        results.extend(crypto_benchmarks::benchmark_consensus(&config));

        // History storage benchmarks
        results.extend(state_benchmarks::benchmark_history_storage(&config));

        // State synchronization benchmarks
        results.extend(network_benchmarks::benchmark_state_sync(&config));

        // Memory efficiency benchmarks
        results.extend(state_benchmarks::benchmark_memory_efficiency(&config));

        // Overall system benchmarks
        results.extend(game_benchmarks::benchmark_full_system(&config));

        println!("\n📊 Benchmark Summary");
        println!("===================");
        Self::print_summary(&results);

        results
    }

    /// Print benchmark summary
    fn print_summary(results: &[BenchmarkResults]) {
        let total_tests = results.len();
        let avg_throughput: f64 =
            results.iter().map(|r| r.throughput).sum::<f64>() / total_tests as f64;
        let total_memory_bytes: usize = results
            .iter()
            .map(|r| r.memory_stats.peak_memory_bytes)
            .sum();

        println!("Total tests executed: {}", total_tests);
        println!("Average throughput: {:.2} ops/sec", avg_throughput);
        println!(
            "Peak memory usage: {:.2} MB",
            total_memory_bytes as f64 / 1_048_576.0
        );

        println!("\nTop performing benchmarks:");
        let mut sorted_results = results.to_vec();
        sorted_results.sort_by(|a, b| b.throughput.partial_cmp(&a.throughput).unwrap());

        for (i, result) in sorted_results.iter().take(5).enumerate() {
            println!(
                "  {}. {} - {:.2} ops/sec",
                i + 1,
                result.name,
                result.throughput
            );
        }

        println!("\nImprovements over baseline:");
        for result in results.iter() {
            if result.improvement_factor > 1.0 {
                println!(
                    "  {} - {:.2}x improvement",
                    result.name, result.improvement_factor
                );
            }
        }
    }
}

/// Helper function to run benchmarks in a criterion context
pub fn criterion_benchmarks(c: &mut Criterion) {
    let config = BenchmarkConfig::default();

    game_benchmarks::criterion_game_benchmarks(c, &config);
    crypto_benchmarks::criterion_crypto_benchmarks(c, &config);
    network_benchmarks::criterion_network_benchmarks(c, &config);
    state_benchmarks::criterion_state_benchmarks(c, &config);
}
