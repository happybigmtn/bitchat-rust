//! Real system monitoring for mobile and desktop platforms
//!
//! This module provides actual system metrics instead of simulated values.
//! It implements platform-specific monitoring for:
//! - CPU usage monitoring
//! - Battery level and status
//! - Thermal/temperature monitoring
//! - Memory usage tracking

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// System metrics collected from real platform APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: f32,
    /// Available memory in bytes
    pub available_memory_bytes: u64,
    /// Used memory in bytes
    pub used_memory_bytes: u64,
    /// Total memory in bytes
    pub total_memory_bytes: u64,
    /// Battery level (0-100) if available
    pub battery_level: Option<f32>,
    /// Battery charging status
    pub battery_charging: Option<bool>,
    /// Device temperature in Celsius if available
    pub temperature_celsius: Option<f32>,
    /// Thermal throttling state
    pub thermal_throttling: bool,
    /// Number of active threads
    pub thread_count: u32,
    /// Network interfaces status
    pub network_interfaces: HashMap<String, NetworkInterface>,
    /// Timestamp when metrics were collected
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub is_up: bool,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

/// Platform-specific system monitor
pub trait SystemMonitor: Send + Sync {
    /// Collect current system metrics
    fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError>;

    /// Get platform identifier
    fn platform_name(&self) -> &str;

    /// Check if real monitoring is available (vs simulation)
    fn is_real_monitoring(&self) -> bool;

    /// Get supported metrics for this platform
    fn supported_metrics(&self) -> Vec<MetricType>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    CpuUsage,
    Memory,
    Battery,
    Temperature,
    Network,
    Threads,
}

#[derive(Debug, thiserror::Error)]
pub enum SystemMonitorError {
    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),
    #[error("Permission denied accessing system metrics: {0}")]
    PermissionDenied(String),
    #[error("System API error: {0}")]
    SystemApiError(String),
    #[error("Metric not available on this platform: {metric:?}")]
    MetricNotAvailable { metric: MetricType },
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Cached metrics collector to avoid excessive system calls
pub struct CachedSystemMonitor {
    inner: Box<dyn SystemMonitor>,
    cache: Arc<Mutex<Option<CachedMetrics>>>,
    cache_duration: Duration,
}

#[derive(Debug, Clone)]
struct CachedMetrics {
    metrics: SystemMetrics,
    collected_at: Instant,
}

impl CachedSystemMonitor {
    pub fn new(monitor: Box<dyn SystemMonitor>) -> Self {
        Self::with_cache_duration(monitor, Duration::from_secs(5))
    }

    pub fn with_cache_duration(monitor: Box<dyn SystemMonitor>, cache_duration: Duration) -> Self {
        Self {
            inner: monitor,
            cache: Arc::new(Mutex::new(None)),
            cache_duration,
        }
    }

    pub fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();

        // Check if cache is valid
        if let Some(ref cached) = *cache {
            if now.duration_since(cached.collected_at) < self.cache_duration {
                return Ok(cached.metrics.clone());
            }
        }

        // Cache expired or doesn't exist, collect new metrics
        let metrics = self.inner.collect_metrics()?;
        *cache = Some(CachedMetrics {
            metrics: metrics.clone(),
            collected_at: now,
        });

        Ok(metrics)
    }

    pub fn platform_name(&self) -> &str {
        self.inner.platform_name()
    }

    pub fn is_real_monitoring(&self) -> bool {
        self.inner.is_real_monitoring()
    }

    pub fn supported_metrics(&self) -> Vec<MetricType> {
        self.inner.supported_metrics()
    }

    /// Force cache refresh
    pub fn refresh_cache(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let mut cache = self.cache.lock().unwrap();
        let metrics = self.inner.collect_metrics()?;
        *cache = Some(CachedMetrics {
            metrics: metrics.clone(),
            collected_at: Instant::now(),
        });
        Ok(metrics)
    }
}

/// Platform detection and monitor factory
pub struct SystemMonitorFactory;

impl SystemMonitorFactory {
    /// Create the appropriate system monitor for the current platform
    pub fn create_monitor() -> Box<dyn SystemMonitor> {
        #[cfg(target_os = "linux")]
        {
            Box::new(linux::LinuxSystemMonitor::new())
        }

        #[cfg(target_os = "android")]
        {
            Box::new(android::AndroidSystemMonitor::new())
        }

        #[cfg(target_os = "ios")]
        {
            Box::new(ios::IOSSystemMonitor::new())
        }

        #[cfg(target_os = "macos")]
        {
            Box::new(macos::MacOSSystemMonitor::new())
        }

        #[cfg(target_os = "windows")]
        {
            Box::new(windows::WindowsSystemMonitor::new())
        }

        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_os = "windows"
        )))]
        {
            Box::new(fallback::FallbackSystemMonitor::new())
        }
    }

    /// Create a cached monitor with default settings
    pub fn create_cached_monitor() -> CachedSystemMonitor {
        CachedSystemMonitor::new(Self::create_monitor())
    }

    /// Get the current platform name
    pub fn current_platform() -> &'static str {
        #[cfg(target_os = "linux")]
        return "linux";

        #[cfg(target_os = "android")]
        return "android";

        #[cfg(target_os = "ios")]
        return "ios";

        #[cfg(target_os = "macos")]
        return "macos";

        #[cfg(target_os = "windows")]
        return "windows";

        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_os = "windows"
        )))]
        return "unknown";
    }
}

/// Global system monitor instance
static SYSTEM_MONITOR: std::sync::OnceLock<CachedSystemMonitor> = std::sync::OnceLock::new();

/// Get the global system monitor instance
pub fn global_system_monitor() -> &'static CachedSystemMonitor {
    SYSTEM_MONITOR.get_or_init(|| SystemMonitorFactory::create_cached_monitor())
}

// Platform-specific implementations
pub mod android;
pub mod fallback;
pub mod ios;
pub mod linux;
pub mod macos;
pub mod windows;

// Re-export platform monitors for direct access if needed
pub use android::AndroidSystemMonitor;
pub use fallback::FallbackSystemMonitor;
pub use ios::IOSSystemMonitor;
pub use linux::LinuxSystemMonitor;
pub use macos::MacOSSystemMonitor;
pub use windows::WindowsSystemMonitor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_monitor_creation() {
        let monitor = SystemMonitorFactory::create_monitor();
        let platform = monitor.platform_name();
        println!("Platform: {}", platform);

        // Should be able to get supported metrics
        let metrics = monitor.supported_metrics();
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_cached_monitor() {
        let monitor = SystemMonitorFactory::create_cached_monitor();

        // First call should hit the system
        let start = Instant::now();
        let metrics1 = monitor.collect_metrics().unwrap();
        let first_duration = start.elapsed();

        // Second call should use cache
        let start = Instant::now();
        let metrics2 = monitor.collect_metrics().unwrap();
        let second_duration = start.elapsed();

        // Cache should make it faster (though this is a rough test)
        assert!(second_duration <= first_duration);

        // Metrics should be the same (from cache)
        assert_eq!(metrics1.timestamp, metrics2.timestamp);
    }
}
