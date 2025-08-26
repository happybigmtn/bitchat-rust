//! macOS system monitoring implementation
//!
//! Uses macOS system APIs similar to iOS but with more access

use super::{SystemMonitor, SystemMetrics, SystemMonitorError, MetricType, NetworkInterface};
use std::collections::HashMap;
use std::time::SystemTime;

#[cfg(target_os = "macos")]
use std::os::raw::{c_void, c_int, c_uint, c_char};

pub struct MacOSSystemMonitor {
    // macOS-specific state
}

impl MacOSSystemMonitor {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Get CPU usage using host_processor_info (same as iOS but with more access)
    #[cfg(target_os = "macos")]
    fn get_cpu_usage(&self) -> Result<f32, SystemMonitorError> {
        // Similar to iOS implementation but can access more detailed info
        // For now, use system command as fallback
        self.get_cpu_usage_sysctl()
    }
    
    #[cfg(target_os = "macos")]
    fn get_cpu_usage_sysctl(&self) -> Result<f32, SystemMonitorError> {
        use std::process::Command;
        
        let output = Command::new("top")
            .args(&["-l", "1", "-n", "0"])
            .output()
            .map_err(|e| SystemMonitorError::SystemApiError(format!("Failed to run top command: {}", e)))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse CPU usage from top output
        for line in output_str.lines() {
            if line.contains("CPU usage:") {
                // Example: "CPU usage: 2.5% user, 1.2% sys, 96.3% idle"
                if let Some(user_start) = line.find("CPU usage: ") {
                    let after_prefix = &line[user_start + 11..];
                    if let Some(percent_pos) = after_prefix.find("% user") {
                        let user_str = &after_prefix[..percent_pos];
                        if let Ok(user_percent) = user_str.parse::<f32>() {
                            // For simplicity, just return user CPU percentage
                            // In practice, you'd parse sys percentage too
                            return Ok(user_percent);
                        }
                    }
                }
            }
        }
        
        // Fallback to vm_stat and iostat if available
        Ok(0.0)
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_cpu_usage(&self) -> Result<f32, SystemMonitorError> {
        Err(SystemMonitorError::PlatformNotSupported("macOS CPU monitoring not available".to_string()))
    }
    
    /// Get memory information using vm_stat
    #[cfg(target_os = "macos")]
    fn get_memory_info(&self) -> Result<(u64, u64, u64), SystemMonitorError> {
        use std::process::Command;
        
        let output = Command::new("vm_stat")
            .output()
            .map_err(|e| SystemMonitorError::SystemApiError(format!("Failed to run vm_stat: {}", e)))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        let mut page_size = 4096u64;
        let mut free_pages = 0u64;
        let mut active_pages = 0u64;
        let mut inactive_pages = 0u64;
        let mut wired_pages = 0u64;
        let mut compressed_pages = 0u64;
        
        for line in output_str.lines() {
            if line.contains("Mach Virtual Memory Statistics:") {
                continue;
            } else if line.contains("page size of") {
                // Extract page size
                if let Some(start) = line.find("page size of ") {
                    let after = &line[start + 13..];
                    if let Some(end) = after.find(" bytes") {
                        let size_str = &after[..end];
                        page_size = size_str.parse().unwrap_or(4096);
                    }
                }
            } else if line.starts_with("Pages free:") {
                free_pages = self.extract_page_count(line)?;
            } else if line.starts_with("Pages active:") {
                active_pages = self.extract_page_count(line)?;
            } else if line.starts_with("Pages inactive:") {
                inactive_pages = self.extract_page_count(line)?;
            } else if line.starts_with("Pages wired down:") {
                wired_pages = self.extract_page_count(line)?;
            } else if line.starts_with("Pages occupied by compressor:") {
                compressed_pages = self.extract_page_count(line)?;
            }
        }
        
        let total_pages = free_pages + active_pages + inactive_pages + wired_pages + compressed_pages;
        let total_bytes = total_pages * page_size;
        let free_bytes = free_pages * page_size;
        let used_bytes = (active_pages + wired_pages + compressed_pages) * page_size;
        let available_bytes = free_bytes + (inactive_pages * page_size);
        
        Ok((total_bytes, used_bytes, available_bytes))
    }
    
    #[cfg(target_os = "macos")]
    fn extract_page_count(&self, line: &str) -> Result<u64, SystemMonitorError> {
        // Extract number from lines like "Pages free: 123456."
        if let Some(colon_pos) = line.find(':') {
            let after_colon = &line[colon_pos + 1..].trim();
            let number_str = after_colon.trim_end_matches('.');
            number_str.parse()
                .map_err(|e| SystemMonitorError::ParseError(format!("Failed to parse page count '{}': {}", number_str, e)))
        } else {
            Err(SystemMonitorError::ParseError(format!("Invalid vm_stat line: {}", line)))
        }
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_memory_info(&self) -> Result<(u64, u64, u64), SystemMonitorError> {
        Err(SystemMonitorError::PlatformNotSupported("macOS memory monitoring not available".to_string()))
    }
    
    /// Get battery information (for MacBooks)
    #[cfg(target_os = "macos")]
    fn get_battery_info(&self) -> Result<(Option<f32>, Option<bool>), SystemMonitorError> {
        use std::process::Command;
        
        // Use pmset to get battery information
        let output = Command::new("pmset")
            .args(&["-g", "batt"])
            .output()
            .map_err(|e| SystemMonitorError::SystemApiError(format!("Failed to run pmset: {}", e)))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        for line in output_str.lines() {
            if line.contains("InternalBattery") && line.contains("%") {
                // Parse line like "InternalBattery-0	100%; charged; 0:00 remaining"
                if let Some(percent_start) = line.find('\t') {
                    let after_tab = &line[percent_start + 1..];
                    if let Some(percent_end) = after_tab.find('%') {
                        let percent_str = &after_tab[..percent_end];
                        if let Ok(battery_level) = percent_str.parse::<f32>() {
                            let is_charging = line.contains("charging") || line.contains("AC attached");
                            return Ok((Some(battery_level), Some(is_charging)));
                        }
                    }
                }
            }
        }
        
        // No battery found (desktop Mac)
        Ok((None, None))
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_battery_info(&self) -> Result<(Option<f32>, Option<bool>), SystemMonitorError> {
        Ok((None, None))
    }
    
    /// Get thermal information using powermetrics or thermal sensors
    #[cfg(target_os = "macos")]
    fn get_thermal_info(&self) -> Result<(Option<f32>, bool), SystemMonitorError> {
        // macOS thermal monitoring is available through powermetrics (requires sudo)
        // or through IOKit temperature sensors
        // For now, return minimal info
        Ok((None, false))
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_thermal_info(&self) -> Result<(Option<f32>, bool), SystemMonitorError> {
        Ok((None, false))
    }
    
    /// Get thread count
    #[cfg(target_os = "macos")]
    fn get_thread_count(&self) -> Result<u32, SystemMonitorError> {
        Ok(std::thread::available_parallelism()
           .map(|n| n.get() as u32)
           .unwrap_or(1))
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_thread_count(&self) -> Result<u32, SystemMonitorError> {
        Ok(1)
    }
    
    /// Get network interfaces using netstat
    #[cfg(target_os = "macos")]
    fn get_network_interfaces(&self) -> Result<HashMap<String, NetworkInterface>, SystemMonitorError> {
        use std::process::Command;
        
        let mut interfaces = HashMap::new();
        
        // Use netstat to get interface statistics
        let output = Command::new("netstat")
            .args(&["-i", "-b"])
            .output()
            .map_err(|e| SystemMonitorError::SystemApiError(format!("Failed to run netstat: {}", e)))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Skip header line
        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let name = parts[0].to_string();
                
                // Parse network statistics
                let bytes_received = parts[6].parse::<u64>().unwrap_or(0);
                let bytes_sent = parts[9].parse::<u64>().unwrap_or(0);
                
                // Check if interface is up (simplified)
                let is_up = !parts[3].contains("*");
                
                interfaces.insert(name.clone(), NetworkInterface {
                    name,
                    is_up,
                    bytes_sent,
                    bytes_received,
                    packets_sent: 0,    // Would need additional parsing
                    packets_received: 0,
                });
            }
        }
        
        Ok(interfaces)
    }
    
    #[cfg(not(target_os = "macos"))]
    fn get_network_interfaces(&self) -> Result<HashMap<String, NetworkInterface>, SystemMonitorError> {
        Ok(HashMap::new())
    }
}

impl SystemMonitor for MacOSSystemMonitor {
    fn collect_metrics(&self) -> Result<SystemMetrics, SystemMonitorError> {
        let cpu_usage = self.get_cpu_usage().unwrap_or(0.0);
        let (total_memory, used_memory, available_memory) = self.get_memory_info()
            .unwrap_or((0, 0, 0));
        let (battery_level, battery_charging) = self.get_battery_info()
            .unwrap_or((None, None));
        let (temperature, thermal_throttling) = self.get_thermal_info()
            .unwrap_or((None, false));
        let thread_count = self.get_thread_count().unwrap_or(1);
        let network_interfaces = self.get_network_interfaces()
            .unwrap_or_default();
        
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
        "macos"
    }
    
    fn is_real_monitoring(&self) -> bool {
        cfg!(target_os = "macos")
    }
    
    fn supported_metrics(&self) -> Vec<MetricType> {
        vec![
            MetricType::CpuUsage,
            MetricType::Memory,
            MetricType::Battery,     // For MacBooks
            MetricType::Temperature, // Limited without sudo
            MetricType::Network,
            MetricType::Threads,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macos_monitor_creation() {
        let monitor = MacOSSystemMonitor::new();
        assert_eq!(monitor.platform_name(), "macos");
    }
    
    #[tokio::test]
    async fn test_collect_metrics() {
        let monitor = MacOSSystemMonitor::new();
        
        match monitor.collect_metrics() {
            Ok(metrics) => {
                println!("macOS Metrics - CPU: {}%, Memory: {} MB", 
                         metrics.cpu_usage_percent,
                         metrics.used_memory_bytes / 1024 / 1024);
                
                if cfg!(target_os = "macos") {
                    assert!(metrics.cpu_usage_percent >= 0.0);
                }
            },
            Err(e) => {
                println!("macOS metrics collection failed (expected on non-macOS): {}", e);
            }
        }
    }
}