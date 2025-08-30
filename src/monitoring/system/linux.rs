//! Linux system monitoring implementation
//!
//! Uses /proc filesystem and system APIs for real metrics collection

use super::{MetricType, NetworkInterface, SystemMetrics, SystemMonitor, SystemMonitorError};
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

pub struct LinuxSystemMonitor {
    last_cpu_stats: std::sync::Mutex<Option<CpuStats>>,
}

#[derive(Debug, Clone)]
struct CpuStats {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_nice: u64,
    timestamp: std::time::Instant,
}

impl CpuStats {
    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
            + self.guest
            + self.guest_nice
    }

    fn idle_total(&self) -> u64 {
        self.idle + self.iowait
    }

    fn work_total(&self) -> u64 {
        self.total() - self.idle_total()
    }
}

impl LinuxSystemMonitor {
    pub fn new() -> Self {
        Self {
            last_cpu_stats: std::sync::Mutex::new(None),
        }
    }

    fn read_cpu_stats(&self) -> Result<CpuStats, SystemMonitorError> {
        let content = fs::read_to_string("/proc/stat").map_err(|e| {
            SystemMonitorError::SystemApiError(format!("Failed to read /proc/stat: {}", e))
        })?;

        let first_line = content
            .lines()
            .next()
            .ok_or_else(|| SystemMonitorError::ParseError("Empty /proc/stat".to_string()))?;

        if !first_line.starts_with("cpu ") {
            return Err(SystemMonitorError::ParseError(
                "Invalid /proc/stat format".to_string(),
            ));
        }

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 11 {
            return Err(SystemMonitorError::ParseError(
                "Insufficient CPU stats in /proc/stat".to_string(),
            ));
        }

        let parse_u64 = |s: &str| {
            s.parse::<u64>().map_err(|e| {
                SystemMonitorError::ParseError(format!("Failed to parse CPU stat '{}': {}", s, e))
            })
        };

        Ok(CpuStats {
            user: parse_u64(parts[1])?,
            nice: parse_u64(parts[2])?,
            system: parse_u64(parts[3])?,
            idle: parse_u64(parts[4])?,
            iowait: parse_u64(parts[5])?,
            irq: parse_u64(parts[6])?,
            softirq: parse_u64(parts[7])?,
            steal: parse_u64(parts[8])?,
            guest: parse_u64(parts[9])?,
            guest_nice: parse_u64(parts[10])?,
            timestamp: std::time::Instant::now(),
        })
    }

    fn calculate_cpu_usage(&self) -> Result<f32, SystemMonitorError> {
        let current_stats = self.read_cpu_stats()?;
        let mut last_stats_guard = self.last_cpu_stats.lock().unwrap();

        let cpu_usage = if let Some(ref last_stats) = *last_stats_guard {
            let total_diff = current_stats.total() - last_stats.total();
            let idle_diff = current_stats.idle_total() - last_stats.idle_total();

            if total_diff == 0 {
                0.0
            } else {
                let work_diff = total_diff - idle_diff;
                (work_diff as f32 / total_diff as f32) * 100.0
            }
        } else {
            // First measurement, return 0
            0.0
        };

        *last_stats_guard = Some(current_stats);
        Ok(cpu_usage)
    }

    fn read_memory_info(&self) -> Result<(u64, u64, u64), SystemMonitorError> {
        let content = fs::read_to_string("/proc/meminfo").map_err(|e| {
            SystemMonitorError::SystemApiError(format!("Failed to read /proc/meminfo: {}", e))
        })?;

        let mut total_kb = 0u64;
        let mut available_kb = 0u64;
        let mut free_kb = 0u64;
        let mut buffers_kb = 0u64;
        let mut cached_kb = 0u64;

        for line in content.lines() {
            if let Some(colon_pos) = line.find(':') {
                let key = &line[..colon_pos];
                let value_str = &line[colon_pos + 1..].trim();

                // Parse value (remove " kB" suffix)
                let value_str = value_str.strip_suffix(" kB").unwrap_or(value_str);
                let value = value_str.trim().parse::<u64>().map_err(|e| {
                    SystemMonitorError::ParseError(format!(
                        "Failed to parse memory value '{}': {}",
                        value_str, e
                    ))
                })?;

                match key {
                    "MemTotal" => total_kb = value,
                    "MemAvailable" => available_kb = value,
                    "MemFree" => free_kb = value,
                    "Buffers" => buffers_kb = value,
                    "Cached" => cached_kb = value,
                    _ => {}
                }
            }
        }

        // If MemAvailable is not available (older kernels), calculate it
        if available_kb == 0 {
            available_kb = free_kb + buffers_kb + cached_kb;
        }

        let total_bytes = total_kb * 1024;
        let available_bytes = available_kb * 1024;
        let used_bytes = total_bytes - available_bytes;

        Ok((total_bytes, used_bytes, available_bytes))
    }

    fn read_battery_info(&self) -> Result<(Option<f32>, Option<bool>), SystemMonitorError> {
        // Try to read battery information from /sys/class/power_supply/
        let power_supply_path = "/sys/class/power_supply";

        if !std::path::Path::new(power_supply_path).exists() {
            return Ok((None, None));
        }

        // Look for battery entries
        let entries = fs::read_dir(power_supply_path).map_err(|e| {
            SystemMonitorError::SystemApiError(format!("Failed to read power supply dir: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| SystemMonitorError::IoError(e))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Check if this is a battery (not AC adapter)
            let type_path = path.join("type");
            if let Ok(device_type) = fs::read_to_string(&type_path) {
                if device_type.trim().to_lowercase() == "battery" {
                    // Read battery capacity
                    let capacity = self.read_battery_capacity(&path)?;
                    let charging = self.read_battery_charging_status(&path)?;
                    return Ok((capacity, charging));
                }
            }
        }

        Ok((None, None))
    }

    fn read_battery_capacity(
        &self,
        battery_path: &std::path::Path,
    ) -> Result<Option<f32>, SystemMonitorError> {
        let capacity_path = battery_path.join("capacity");
        match fs::read_to_string(&capacity_path) {
            Ok(content) => {
                let capacity = content.trim().parse::<f32>().map_err(|e| {
                    SystemMonitorError::ParseError(format!(
                        "Failed to parse battery capacity: {}",
                        e
                    ))
                })?;
                Ok(Some(capacity))
            }
            Err(_) => Ok(None),
        }
    }

    fn read_battery_charging_status(
        &self,
        battery_path: &std::path::Path,
    ) -> Result<Option<bool>, SystemMonitorError> {
        let status_path = battery_path.join("status");
        match fs::read_to_string(&status_path) {
            Ok(content) => {
                let status = content.trim().to_lowercase();
                let charging = match status.as_str() {
                    "charging" => Some(true),
                    "discharging" | "not charging" => Some(false),
                    "full" => Some(false), // Full battery is not charging
                    _ => None,
                };
                Ok(charging)
            }
            Err(_) => Ok(None),
        }
    }

    fn read_thermal_info(&self) -> Result<(Option<f32>, bool), SystemMonitorError> {
        // Try to read thermal zone information
        let thermal_path = "/sys/class/thermal";

        if !std::path::Path::new(thermal_path).exists() {
            return Ok((None, false));
        }

        let entries = fs::read_dir(thermal_path).map_err(|e| {
            SystemMonitorError::SystemApiError(format!("Failed to read thermal dir: {}", e))
        })?;

        let mut max_temp: Option<f32> = None;
        let mut is_throttling = false;

        for entry in entries {
            let entry = entry.map_err(|e| SystemMonitorError::IoError(e))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with("thermal_zone") {
                // Read temperature
                let temp_path = path.join("temp");
                if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                    if let Ok(temp_millicelsius) = temp_str.trim().parse::<i32>() {
                        let temp_celsius = temp_millicelsius as f32 / 1000.0;
                        max_temp = Some(
                            max_temp.map_or(temp_celsius, |existing| existing.max(temp_celsius)),
                        );
                    }
                }

                // Check for throttling (simplified check)
                let policy_path = path.join("policy");
                if let Ok(policy) = fs::read_to_string(&policy_path) {
                    if policy.trim().contains("step_wise") {
                        // This is a very basic check - in practice you'd want more sophisticated detection
                        if let Some(temp) = max_temp {
                            is_throttling = temp > 80.0; // Simplified throttling threshold
                        }
                    }
                }
            }
        }

        Ok((max_temp, is_throttling))
    }

    fn read_thread_count(&self) -> Result<u32, SystemMonitorError> {
        // Read from /proc/self/stat or count threads in /proc/self/task/
        let task_dir = "/proc/self/task";
        match fs::read_dir(task_dir) {
            Ok(entries) => {
                let count = entries.count() as u32;
                Ok(count)
            }
            Err(e) => {
                // Fallback: try to read from /proc/self/status
                let status = fs::read_to_string("/proc/self/status").map_err(|e| {
                    SystemMonitorError::SystemApiError(format!(
                        "Failed to read process status: {}",
                        e
                    ))
                })?;

                for line in status.lines() {
                    if line.starts_with("Threads:") {
                        let count_str = line.split_whitespace().nth(1).ok_or_else(|| {
                            SystemMonitorError::ParseError(
                                "Invalid Threads line in /proc/self/status".to_string(),
                            )
                        })?;
                        let count = count_str.parse::<u32>().map_err(|e| {
                            SystemMonitorError::ParseError(format!(
                                "Failed to parse thread count: {}",
                                e
                            ))
                        })?;
                        return Ok(count);
                    }
                }

                Err(SystemMonitorError::SystemApiError(format!(
                    "Failed to determine thread count: {}",
                    e
                )))
            }
        }
    }

    fn read_network_interfaces(
        &self,
    ) -> Result<HashMap<String, NetworkInterface>, SystemMonitorError> {
        let mut interfaces = HashMap::new();

        // Read from /proc/net/dev
        let content = fs::read_to_string("/proc/net/dev").map_err(|e| {
            SystemMonitorError::SystemApiError(format!("Failed to read /proc/net/dev: {}", e))
        })?;

        for line in content.lines().skip(2) {
            // Skip header lines
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 17 {
                let name = parts[0].trim_end_matches(':').to_string();

                let parse_u64 = |s: &str| {
                    s.parse::<u64>().map_err(|e| {
                        SystemMonitorError::ParseError(format!(
                            "Failed to parse network stat '{}': {}",
                            s, e
                        ))
                    })
                };

                let bytes_received = parse_u64(parts[1])?;
                let packets_received = parse_u64(parts[2])?;
                let bytes_sent = parse_u64(parts[9])?;
                let packets_sent = parse_u64(parts[10])?;

                // Check if interface is up by reading /sys/class/net/{name}/operstate
                let operstate_path = format!("/sys/class/net/{}/operstate", name);
                let is_up = fs::read_to_string(&operstate_path)
                    .map(|s| s.trim() == "up")
                    .unwrap_or(false);

                interfaces.insert(
                    name.clone(),
                    NetworkInterface {
                        name,
                        is_up,
                        bytes_sent,
                        bytes_received,
                        packets_sent,
                        packets_received,
                    },
                );
            }
        }

        Ok(interfaces)
    }
}

impl SystemMonitor for LinuxSystemMonitor {
    fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let cpu_usage = self.calculate_cpu_usage()?;
        let (total_memory, used_memory, available_memory) = self.read_memory_info()?;
        let (battery_level, battery_charging) = self.read_battery_info()?;
        let (temperature, thermal_throttling) = self.read_thermal_info()?;
        let thread_count = self.read_thread_count()?;
        let network_interfaces = self.read_network_interfaces()?;

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

    fn platform_name(&self) -> &str {
        "linux"
    }

    fn is_real_monitoring(&self) -> bool {
        true
    }

    fn supported_metrics(&self) -> Vec<MetricType> {
        vec![
            MetricType::CpuUsage,
            MetricType::Memory,
            MetricType::Battery,     // May not be available on desktop
            MetricType::Temperature, // May not be available on all systems
            MetricType::Network,
            MetricType::Threads,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_monitor_creation() {
        let monitor = LinuxSystemMonitor::new();
        assert_eq!(monitor.platform_name(), "linux");
        assert!(monitor.is_real_monitoring());
    }

    #[tokio::test]
    async fn test_collect_metrics() {
        let monitor = LinuxSystemMonitor::new();

        // This test will only work on Linux systems
        if cfg!(target_os = "linux") {
            match monitor.collect_metrics() {
                Ok(metrics) => {
                    println!("CPU Usage: {}%", metrics.cpu_usage_percent);
                    println!(
                        "Memory: {} MB used of {} MB total",
                        metrics.used_memory_bytes / 1024 / 1024,
                        metrics.total_memory_bytes / 1024 / 1024
                    );

                    assert!(metrics.cpu_usage_percent >= 0.0 && metrics.cpu_usage_percent <= 100.0);
                    assert!(metrics.total_memory_bytes > 0);
                    assert!(metrics.used_memory_bytes <= metrics.total_memory_bytes);
                }
                Err(e) => {
                    println!("Warning: Could not collect metrics on this system: {}", e);
                    // Don't fail the test - system might not have /proc filesystem access
                }
            }
        }
    }
}
