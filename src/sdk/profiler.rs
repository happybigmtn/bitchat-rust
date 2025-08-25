//! Performance Profiler for BitCraps SDK

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Performance profiler for analyzing game and network performance
pub struct PerformanceProfiler {
    active_measurements: HashMap<String, Instant>,
    completed_measurements: Vec<Measurement>,
    benchmarks: Vec<Benchmark>,
}

impl PerformanceProfiler {
    pub fn new() -> Self {
        Self {
            active_measurements: HashMap::new(),
            completed_measurements: Vec::new(),
            benchmarks: Vec::new(),
        }
    }

    /// Start measuring performance for a named operation
    pub fn start_measurement(&mut self, name: &str) {
        self.active_measurements.insert(name.to_string(), Instant::now());
    }

    /// End measurement and record duration
    pub fn end_measurement(&mut self, name: &str) -> Option<Duration> {
        if let Some(start_time) = self.active_measurements.remove(name) {
            let duration = start_time.elapsed();
            self.completed_measurements.push(Measurement {
                name: name.to_string(),
                duration,
                timestamp: start_time,
            });
            Some(duration)
        } else {
            None
        }
    }

    /// Run performance benchmark
    pub fn run_benchmark(&mut self, benchmark: Benchmark) -> BenchmarkResult {
        let start_time = Instant::now();
        
        // Run benchmark iterations
        let mut durations = Vec::new();
        for _ in 0..benchmark.iterations {
            let iter_start = Instant::now();
            // Benchmark operation would be executed here
            durations.push(iter_start.elapsed());
        }

        let total_duration = start_time.elapsed();
        let average_duration = Duration::from_nanos(
            durations.iter().map(|d| d.as_nanos()).sum::<u128>() / durations.len() as u128
        );

        BenchmarkResult {
            benchmark_name: benchmark.name,
            iterations: benchmark.iterations,
            total_duration,
            average_duration,
            min_duration: durations.iter().min().copied().unwrap_or(Duration::ZERO),
            max_duration: durations.iter().max().copied().unwrap_or(Duration::ZERO),
        }
    }

    /// Generate performance report
    pub fn generate_report(&self) -> ProfileReport {
        ProfileReport {
            total_measurements: self.completed_measurements.len(),
            measurements: self.completed_measurements.clone(),
            performance_summary: self.calculate_summary(),
        }
    }

    fn calculate_summary(&self) -> PerformanceSummary {
        if self.completed_measurements.is_empty() {
            return PerformanceSummary::default();
        }

        let total_duration: Duration = self.completed_measurements.iter()
            .map(|m| m.duration)
            .sum();

        let average_duration = total_duration / self.completed_measurements.len() as u32;

        PerformanceSummary {
            total_operations: self.completed_measurements.len(),
            total_time: total_duration,
            average_time: average_duration,
            fastest_operation: self.completed_measurements.iter()
                .min_by_key(|m| m.duration)
                .map(|m| m.name.clone()),
            slowest_operation: self.completed_measurements.iter()
                .max_by_key(|m| m.duration)
                .map(|m| m.name.clone()),
        }
    }
}

/// Individual performance measurement
#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub duration: Duration,
    pub timestamp: Instant,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct Benchmark {
    pub name: String,
    pub iterations: usize,
    pub operation: BenchmarkOperation,
}

#[derive(Debug, Clone)]
pub enum BenchmarkOperation {
    GameAction,
    NetworkOperation,
    CryptoOperation,
    StorageOperation,
}

/// Benchmark execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub benchmark_name: String,
    pub iterations: usize,
    pub total_duration: Duration,
    pub average_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
}

/// Complete performance report
#[derive(Debug, Clone)]
pub struct ProfileReport {
    pub total_measurements: usize,
    pub measurements: Vec<Measurement>,
    pub performance_summary: PerformanceSummary,
}

/// Performance statistics summary
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_operations: usize,
    pub total_time: Duration,
    pub average_time: Duration,
    pub fastest_operation: Option<String>,
    pub slowest_operation: Option<String>,
}

impl Default for PerformanceSummary {
    fn default() -> Self {
        Self {
            total_operations: 0,
            total_time: Duration::ZERO,
            average_time: Duration::ZERO,
            fastest_operation: None,
            slowest_operation: None,
        }
    }
}