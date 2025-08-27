//! Real system metrics collection replacing simulated values
//!
//! This module provides actual system monitoring capabilities
//! instead of the placeholder implementations.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use sysinfo::{System, SystemExt, ProcessExt, CpuExt};

#[cfg(target_os = "linux")]
use procfs::process::Process;

#[cfg(target_os = "android")]
use android_system::BatteryManager;

/// Real system metrics collector
pub struct RealMetricsCollector {
    system: Arc<RwLock<System>>,
    process_id: u32,
    last_update: Arc<RwLock<Instant>>,
    cache_duration: Duration,
}

impl RealMetricsCollector {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        Self {
            system: Arc::new(RwLock::new(system)),
            process_id: std::process::id(),
            last_update: Arc::new(RwLock::new(Instant::now())),
            cache_duration: Duration::from_millis(100),
        }
    }
    
    /// Get actual CPU usage percentage
    pub async fn get_cpu_usage(&self) -> f64 {
        self.refresh_if_needed().await;
        
        let system = self.system.read().await;
        
        // Get process CPU usage
        if let Some(process) = system.process(self.process_id.into()) {
            process.cpu_usage() as f64
        } else {
            // Fallback to global CPU usage
            system.global_cpu_info().cpu_usage() as f64
        }
    }
    
    /// Get actual memory usage in MB
    pub async fn get_memory_usage(&self) -> f64 {
        self.refresh_if_needed().await;
        
        let system = self.system.read().await;
        
        if let Some(process) = system.process(self.process_id.into()) {
            // Convert from KB to MB
            (process.memory() as f64) / 1024.0
        } else {
            0.0
        }
    }
    
    /// Get system temperature (if available)
    #[cfg(target_os = "linux")]
    pub async fn get_temperature(&self) -> Option<f64> {
        // Try to read from thermal zone
        if let Ok(temp_str) = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
            if let Ok(temp_millidegree) = temp_str.trim().parse::<f64>() {
                return Some(temp_millidegree / 1000.0);
            }
        }
        None
    }
    
    #[cfg(not(target_os = "linux"))]
    pub async fn get_temperature(&self) -> Option<f64> {
        None // Not supported on this platform
    }
    
    /// Get battery level and drain rate
    #[cfg(target_os = "android")]
    pub async fn get_battery_info(&self) -> (f64, f64) {
        if let Ok(battery) = BatteryManager::new() {
            let level = battery.get_level() as f64;
            let drain_rate = battery.get_instantaneous_current() as f64 / 1000.0; // mA to A
            (level, drain_rate)
        } else {
            (100.0, 0.0) // Default values
        }
    }
    
    #[cfg(not(target_os = "android"))]
    pub async fn get_battery_info(&self) -> (f64, f64) {
        // Simulate for non-Android platforms
        (100.0, 0.0)
    }
    
    /// Get network latency in milliseconds
    pub async fn get_network_latency(&self, target: &str) -> u64 {
        let start = Instant::now();
        
        // Simple TCP connect test
        match tokio::time::timeout(
            Duration::from_secs(1),
            tokio::net::TcpStream::connect(target)
        ).await {
            Ok(Ok(_)) => start.elapsed().as_millis() as u64,
            _ => 999, // Timeout or error
        }
    }
    
    /// Refresh system data if cache expired
    async fn refresh_if_needed(&self) {
        let mut last_update = self.last_update.write().await;
        
        if last_update.elapsed() > self.cache_duration {
            let mut system = self.system.write().await;
            system.refresh_cpu();
            system.refresh_memory();
            system.refresh_processes();
            *last_update = Instant::now();
        }
    }
}

/// Platform-specific memory profiler
pub struct MemoryProfiler {
    #[cfg(target_os = "linux")]
    process: Process,
}

impl MemoryProfiler {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            process: Process::myself().expect("Failed to access process info"),
        }
    }
    
    /// Get detailed memory statistics
    pub fn get_memory_stats(&self) -> MemoryStats {
        #[cfg(target_os = "linux")]
        {
            if let Ok(stat) = self.process.statm() {
                return MemoryStats {
                    total_mb: (stat.size * 4096) / 1_048_576, // Pages to MB
                    resident_mb: (stat.resident * 4096) / 1_048_576,
                    shared_mb: (stat.shared * 4096) / 1_048_576,
                    text_mb: (stat.text * 4096) / 1_048_576,
                    data_mb: (stat.data * 4096) / 1_048_576,
                };
            }
        }
        
        // Fallback for other platforms
        MemoryStats::default()
    }
}

#[derive(Debug, Default, Clone)]
pub struct MemoryStats {
    pub total_mb: u64,
    pub resident_mb: u64,
    pub shared_mb: u64,
    pub text_mb: u64,
    pub data_mb: u64,
}

/// Real compression implementation using actual algorithms
pub mod compression {
    use flate2::Compression;
    use flate2::write::{GzEncoder, GzDecoder};
    use std::io::Write;
    
    /// Compress data using gzip
    pub fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        encoder.finish()
    }
    
    /// Decompress gzip data
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut decoder = GzDecoder::new(Vec::new());
        decoder.write_all(data)?;
        decoder.finish()
    }
    
    /// Calculate actual compression ratio
    pub fn compression_ratio(original: &[u8], compressed: &[u8]) -> f64 {
        if original.is_empty() {
            return 1.0;
        }
        compressed.len() as f64 / original.len() as f64
    }
    
    /// LZ4 compression for real-time data
    pub fn compress_lz4(data: &[u8]) -> Result<Vec<u8>, lz4::block::Error> {
        lz4::block::compress(data, Some(lz4::block::CompressionMode::DEFAULT), true)
    }
    
    /// LZ4 decompression
    pub fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>, lz4::block::Error> {
        lz4::block::decompress(data, None)
    }
}

/// Benchmark actual performance metrics
pub struct PerformanceBenchmark {
    start_time: Instant,
    operations: u64,
}

impl PerformanceBenchmark {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            operations: 0,
        }
    }
    
    /// Record an operation
    pub fn record_operation(&mut self) {
        self.operations += 1;
    }
    
    /// Get operations per second
    pub fn get_ops_per_second(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.operations as f64 / elapsed
        } else {
            0.0
        }
    }
    
    /// Reset benchmark
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.operations = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_real_cpu_usage() {
        let collector = RealMetricsCollector::new();
        let cpu_usage = collector.get_cpu_usage().await;
        
        // CPU usage should be between 0 and 100
        assert!(cpu_usage >= 0.0);
        assert!(cpu_usage <= 100.0);
    }
    
    #[tokio::test]
    async fn test_real_memory_usage() {
        let collector = RealMetricsCollector::new();
        let memory_mb = collector.get_memory_usage().await;
        
        // This process should use at least some memory
        assert!(memory_mb > 0.0);
        // But less than 10GB for a test
        assert!(memory_mb < 10_000.0);
    }
    
    #[test]
    fn test_real_compression() {
        let original = b"Hello, world! This is a test of real compression.";
        let compressed = compression::compress(original).unwrap();
        let decompressed = compression::decompress(&compressed).unwrap();
        
        assert_eq!(original, &decompressed[..]);
        
        // Compression should reduce size for repetitive data
        let repetitive = vec![b'A'; 1000];
        let compressed = compression::compress(&repetitive).unwrap();
        assert!(compressed.len() < repetitive.len());
        
        let ratio = compression::compression_ratio(&repetitive, &compressed);
        assert!(ratio < 0.5); // Should compress well
    }
    
    #[test]
    fn test_performance_benchmark() {
        let mut bench = PerformanceBenchmark::new();
        
        for _ in 0..1000 {
            bench.record_operation();
        }
        
        std::thread::sleep(Duration::from_millis(100));
        
        let ops_per_sec = bench.get_ops_per_second();
        assert!(ops_per_sec > 0.0);
        assert!(ops_per_sec < 100_000.0); // Reasonable upper bound
    }
}