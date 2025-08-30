//! Fallback system monitoring implementation
//!
//! Provides simulated metrics for unsupported platforms or when real monitoring fails

use super::{MetricType, NetworkInterface, SystemMetrics, SystemMonitor, SystemMonitorError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

pub struct FallbackSystemMonitor {
    start_time: Instant,
    random_state: Arc<Mutex<FallbackRandomState>>,
}

struct FallbackRandomState {
    cpu_base: f32,
    memory_base: u64,
    last_update: Instant,
    trend: f32,
}

impl FallbackSystemMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            random_state: Arc::new(Mutex::new(FallbackRandomState {
                cpu_base: 15.0,                  // Start with moderate CPU usage
                memory_base: 1024 * 1024 * 1024, // 1GB base memory usage
                last_update: Instant::now(),
                trend: 1.0,
            })),
        }
    }

    /// Generate realistic-looking simulated metrics
    fn generate_simulated_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let mut state = self.random_state.lock().unwrap();
        let now = Instant::now();

        // Update trend periodically
        if now.duration_since(state.last_update).as_secs() > 10 {
            state.trend = (fastrand::f32() - 0.5) * 0.2 + 0.9; // Slight trending
            state.last_update = now;
        }

        // Generate CPU usage with some variability
        let cpu_variation = (fastrand::f32() - 0.5) * 10.0; // ±5%
        let cpu_usage = (state.cpu_base + cpu_variation * state.trend)
            .max(0.0)
            .min(100.0);
        state.cpu_base = (state.cpu_base * 0.98 + cpu_usage * 0.02)
            .max(2.0)
            .min(80.0); // Smooth trending

        // Generate memory usage
        let total_memory = 8 * 1024 * 1024 * 1024u64; // 8GB total
        let memory_variation = (fastrand::f32() - 0.5) * 0.1; // ±5%
        let used_memory =
            ((state.memory_base as f32) * (1.0 + memory_variation * state.trend)) as u64;
        let used_memory = used_memory.min(total_memory * 9 / 10); // Max 90% usage
        let available_memory = total_memory - used_memory;

        state.memory_base = (state.memory_base * 99 / 100) + (used_memory / 100); // Slow trending

        // Simulate battery (50-50 chance of having battery)
        let (battery_level, battery_charging) = if fastrand::bool() {
            let level = 20.0 + fastrand::f32() * 70.0; // 20-90%
            let charging = fastrand::bool();
            (Some(level), Some(charging))
        } else {
            (None, None) // No battery (desktop)
        };

        // Simulate temperature (mobile devices more likely to report)
        let temperature = if fastrand::f32() < 0.7 {
            // 70% chance
            Some(25.0 + fastrand::f32() * 35.0) // 25-60°C
        } else {
            None
        };

        // Thermal throttling based on temperature
        let thermal_throttling = temperature.map_or(false, |t| t > 55.0);

        // Simulate thread count
        let thread_count = (1 + fastrand::usize(..15)) as u32; // 1-16 threads

        // Simulate network interfaces
        let mut network_interfaces = HashMap::new();

        // Common interface names by platform
        let interface_names = match self.platform_name() {
            "android" => vec!["wlan0", "rmnet_data0", "lo"],
            "ios" => vec!["en0", "pdp_ip0", "lo0"],
            "linux" => vec!["eth0", "wlan0", "lo"],
            "windows" => vec!["Ethernet", "Wi-Fi", "Loopback"],
            "macos" => vec!["en0", "en1", "lo0"],
            _ => vec!["eth0", "wlan0", "lo"],
        };

        for (i, &name) in interface_names.iter().enumerate() {
            let is_active = i < 2; // First two interfaces are active
            let base_traffic = if is_active { 1024 * 1024 } else { 0 }; // 1MB base
            let variation = fastrand::u64(..base_traffic);

            network_interfaces.insert(
                name.to_string(),
                NetworkInterface {
                    name: name.to_string(),
                    is_up: is_active,
                    bytes_sent: base_traffic + variation,
                    bytes_received: base_traffic + variation * 3, // More download than upload
                    packets_sent: (base_traffic + variation) / 1024, // Approximate packets
                    packets_received: (base_traffic + variation * 3) / 1024,
                },
            );
        }

        Ok(SystemMetrics {
            cpu_usage_percent: cpu_usage,
            available_memory_bytes: available_memory,
            used_memory_bytes: used_memory,
            total_memory_bytes: total_memory,
            battery_level,
            battery_charging,
            temperature_celsius: temperature,
            thermal_throttling,
            thread_count,
            network_interfaces,
            timestamp: SystemTime::now(),
        })
    }
}

impl SystemMonitor for FallbackSystemMonitor {
    fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        // Always succeeds with simulated data
        self.generate_simulated_metrics()
    }

    fn platform_name(&self) -> &str {
        // Return the actual platform even though we're using fallback
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

    fn is_real_monitoring(&self) -> bool {
        false // This is simulated data
    }

    fn supported_metrics(&self) -> Vec<MetricType> {
        // Fallback supports all metrics (simulated)
        vec![
            MetricType::CpuUsage,
            MetricType::Memory,
            MetricType::Battery,
            MetricType::Temperature,
            MetricType::Network,
            MetricType::Threads,
        ]
    }
}

impl Default for FallbackSystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_monitor_creation() {
        let monitor = FallbackSystemMonitor::new();
        assert!(!monitor.is_real_monitoring());

        // Should support all metric types
        let metrics = monitor.supported_metrics();
        assert_eq!(metrics.len(), 6);
        assert!(metrics.contains(&MetricType::CpuUsage));
        assert!(metrics.contains(&MetricType::Battery));
    }

    #[tokio::test]
    async fn test_collect_simulated_metrics() {
        let monitor = FallbackSystemMonitor::new();

        // Should always succeed
        let metrics1 = monitor.collect_metrics().unwrap();
        let metrics2 = monitor.collect_metrics().unwrap();

        // Check basic validity
        assert!(metrics1.cpu_usage_percent >= 0.0 && metrics1.cpu_usage_percent <= 100.0);
        assert!(metrics1.total_memory_bytes > 0);
        assert!(metrics1.used_memory_bytes <= metrics1.total_memory_bytes);
        assert!(metrics1.thread_count > 0);
        assert!(!metrics1.network_interfaces.is_empty());

        // Metrics should change between calls (due to randomization)
        // Allow some tolerance for trending
        println!(
            "Metrics 1 - CPU: {}%, Memory: {} MB",
            metrics1.cpu_usage_percent,
            metrics1.used_memory_bytes / 1024 / 1024
        );
        println!(
            "Metrics 2 - CPU: {}%, Memory: {} MB",
            metrics2.cpu_usage_percent,
            metrics2.used_memory_bytes / 1024 / 1024
        );
    }

    #[test]
    fn test_realistic_simulation() {
        let monitor = FallbackSystemMonitor::new();

        // Collect multiple samples to test trends
        let mut cpu_values = Vec::new();
        let mut memory_values = Vec::new();

        for _ in 0..10 {
            let metrics = monitor.collect_metrics().unwrap();
            cpu_values.push(metrics.cpu_usage_percent);
            memory_values.push(metrics.used_memory_bytes);
        }

        // Should have realistic ranges
        assert!(cpu_values.iter().all(|&cpu| cpu >= 0.0 && cpu <= 100.0));
        assert!(memory_values
            .iter()
            .all(|&mem| mem > 0 && mem <= 8 * 1024 * 1024 * 1024));

        // Values should vary (not all identical)
        let cpu_variance = cpu_values
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            - cpu_values
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
        assert!(cpu_variance > 0.0, "CPU values should vary");

        println!(
            "CPU range: {:.1}% - {:.1}%",
            cpu_values
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap(),
            cpu_values
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap()
        );
    }
}
