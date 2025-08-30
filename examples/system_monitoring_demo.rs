//! System monitoring demonstration
//!
//! This example shows how to use the real system monitoring capabilities
//! across different platforms (Linux, Android, iOS, macOS, Windows).

use bitcraps::monitoring::{global_system_monitor, metrics::METRICS};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🖥️ BitCraps Real System Monitoring Demo");
    println!("==========================================");

    // Get the system monitor
    let monitor = global_system_monitor();

    println!("Platform: {}", monitor.platform_name());
    println!("Real monitoring: {}", monitor.is_real_monitoring());
    println!("Supported metrics: {:?}", monitor.supported_metrics());
    println!();

    // Start automatic system monitoring updates
    println!("🔄 Starting periodic system monitoring...");
    let monitoring_task =
        bitcraps::monitoring::metrics::MetricsCollector::start_system_monitoring();

    // Collect and display metrics periodically
    for i in 1..=10 {
        println!("📊 Sample {} of 10", i);
        println!("-----------------");

        match monitor.collect_metrics() {
            Ok(metrics) => {
                println!("⏱️  Timestamp: {:?}", metrics.timestamp);
                println!("🧠 CPU Usage: {:.1}%", metrics.cpu_usage_percent);
                println!(
                    "💾 Memory: {:.1} MB used / {:.1} MB total ({:.1} MB available)",
                    metrics.used_memory_bytes as f64 / 1024.0 / 1024.0,
                    metrics.total_memory_bytes as f64 / 1024.0 / 1024.0,
                    metrics.available_memory_bytes as f64 / 1024.0 / 1024.0
                );

                if let Some(battery_level) = metrics.battery_level {
                    let charging_status = if let Some(charging) = metrics.battery_charging {
                        if charging {
                            "⚡ Charging"
                        } else {
                            "🔋 Discharging"
                        }
                    } else {
                        "❓ Unknown"
                    };
                    println!("🔋 Battery: {:.1}% ({})", battery_level, charging_status);
                }

                if let Some(temp) = metrics.temperature_celsius {
                    let temp_status = if metrics.thermal_throttling {
                        "🔥 Throttling"
                    } else if temp > 70.0 {
                        "🌡️  Hot"
                    } else if temp > 50.0 {
                        "☀️ Warm"
                    } else {
                        "❄️ Cool"
                    };
                    println!("🌡️ Temperature: {:.1}°C ({})", temp, temp_status);
                }

                println!("🧵 Threads: {}", metrics.thread_count);

                // Network interfaces summary
                let active_interfaces: Vec<_> = metrics
                    .network_interfaces
                    .iter()
                    .filter(|(_, interface)| interface.is_up)
                    .collect();

                println!("🌐 Network: {} active interfaces", active_interfaces.len());
                for (name, interface) in &active_interfaces {
                    println!(
                        "   {} - {} MB sent, {} MB received",
                        name,
                        interface.bytes_sent / 1024 / 1024,
                        interface.bytes_received / 1024 / 1024
                    );
                }

                // Show integration with main metrics system
                println!(
                    "📈 Global metrics updated: {}",
                    METRICS.is_real_system_monitoring()
                );
            }
            Err(e) => {
                println!("❌ Error collecting metrics: {}", e);
            }
        }

        println!();

        if i < 10 {
            println!("⏳ Waiting 3 seconds...\n");
            sleep(Duration::from_secs(3)).await;
        }
    }

    // Show Prometheus export with real system data
    println!("📊 Prometheus Metrics Export");
    println!("============================");
    let prometheus_output = METRICS.export_prometheus();

    // Show only system-related metrics from the export
    for line in prometheus_output.lines() {
        if line.contains("cpu_usage")
            || line.contains("memory_usage")
            || line.contains("battery")
            || line.contains("temperature")
            || line.contains("thermal_throttling")
        {
            println!("{}", line);
        }
    }

    println!("\n✅ System monitoring demo completed!");
    println!("💡 The metrics are now being updated in real-time in the background.");

    // Clean up the monitoring task
    monitoring_task.abort();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitoring_demo() {
        // Test that the demo can run without panicking
        let monitor = global_system_monitor();

        // Should be able to identify platform
        assert!(!monitor.platform_name().is_empty());

        // Should have some supported metrics
        assert!(!monitor.supported_metrics().is_empty());

        // Should be able to collect metrics (may be simulated)
        let _metrics = monitor.collect_metrics().unwrap();
    }
}
