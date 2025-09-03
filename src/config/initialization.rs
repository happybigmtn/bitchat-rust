//! Configuration initialization and setup for BitCraps
//!
//! This module provides convenient initialization functions that set up
//! the complete configuration system with scalability, performance tuning,
//! and runtime reload capabilities.

use crate::config::{
    runtime_reload::{ReloadSettings, RuntimeConfigManager},
    scalability::{PerformanceProfile, PlatformType, ScalabilityConfig, ScalabilityManager},
};
use crate::performance::PerformanceMetrics;
use crate::utils::timeout::TimeoutConfig;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// Complete configuration manager combining all configuration subsystems
#[derive(Clone)]
pub struct ConfigurationManager {
    /// Scalability configuration manager
    scalability_manager: Arc<ScalabilityManager>,

    /// Runtime reload manager
    runtime_manager: Arc<RuntimeConfigManager>,

    /// Platform type for optimizations
    platform: PlatformType,

    /// Performance profile
    profile: PerformanceProfile,

    /// Shutdown signal for background tasks
    shutdown_tx: watch::Sender<bool>,

    /// Background task handles for cleanup
    task_handles: Arc<parking_lot::Mutex<Vec<JoinHandle<()>>>>,
}

impl ConfigurationManager {
    /// Initialize configuration for a specific platform and performance profile
    pub async fn initialize(
        platform: PlatformType,
        profile: PerformanceProfile,
        config_paths: Vec<PathBuf>,
    ) -> Result<Self, crate::error::Error> {
        // Create scalability configuration
        let scalability_config = ScalabilityConfig::for_platform(platform, profile);

        // Initialize global timeout configuration
        let timeout_config = TimeoutConfig::from_scalability_config(&scalability_config);
        TimeoutConfig::init_global(timeout_config);

        // Create scalability manager
        let scalability_manager = Arc::new(ScalabilityManager::new(scalability_config.clone()));

        // Create runtime reload manager
        let reload_settings = ReloadSettings::for_platform(platform);
        let runtime_manager = Arc::new(RuntimeConfigManager::new(
            scalability_config,
            config_paths,
            reload_settings,
        ));

        // Start runtime manager
        runtime_manager.start().await.map_err(|e| {
            crate::error::Error::Config(format!("Failed to start runtime manager: {}", e))
        })?;

        let (shutdown_tx, _shutdown_rx) = watch::channel(false);

        Ok(Self {
            scalability_manager,
            runtime_manager,
            platform,
            profile,
            shutdown_tx,
            task_handles: Arc::new(parking_lot::Mutex::new(Vec::new())),
        })
    }

    /// Initialize with automatic platform detection
    pub async fn auto_initialize(config_paths: Vec<PathBuf>) -> Result<Self, crate::error::Error> {
        let platform = detect_platform();
        let profile = detect_performance_profile(platform);

        Self::initialize(platform, profile, config_paths).await
    }

    /// Initialize for mobile platform with battery optimization
    pub async fn initialize_mobile(
        config_paths: Vec<PathBuf>,
    ) -> Result<Self, crate::error::Error> {
        Self::initialize(
            PlatformType::Mobile,
            PerformanceProfile::PowerSaver,
            config_paths,
        )
        .await
    }

    /// Initialize for desktop platform with balanced performance
    pub async fn initialize_desktop(
        config_paths: Vec<PathBuf>,
    ) -> Result<Self, crate::error::Error> {
        Self::initialize(
            PlatformType::Desktop,
            PerformanceProfile::Balanced,
            config_paths,
        )
        .await
    }

    /// Initialize for server platform with maximum performance
    pub async fn initialize_server(
        config_paths: Vec<PathBuf>,
    ) -> Result<Self, crate::error::Error> {
        Self::initialize(
            PlatformType::Server,
            PerformanceProfile::HighPerformance,
            config_paths,
        )
        .await
    }

    /// Get current scalability configuration
    pub fn get_scalability_config(&self) -> ScalabilityConfig {
        self.scalability_manager.get_config()
    }

    /// Update performance metrics for adaptive configuration
    pub async fn update_metrics(&self, metrics: PerformanceMetrics) {
        self.runtime_manager.update_metrics(metrics.clone()).await;
        self.scalability_manager.adapt_to_metrics(&metrics);
    }

    /// Register a background task handle for cleanup
    pub fn register_task(&self, handle: JoinHandle<()>) {
        self.task_handles.lock().push(handle);
    }

    /// Shutdown all background tasks gracefully
    pub async fn shutdown(&self) {
        log::info!("Shutting down configuration manager background tasks");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);

        // Wait for all tasks to complete
        let handles = self.task_handles.lock().drain(..).collect::<Vec<_>>();
        for handle in handles {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        }

        log::info!("Configuration manager shutdown complete");
    }

    /// Reload configuration from files
    pub async fn reload_configuration(&self) -> Result<(), crate::error::Error> {
        self.runtime_manager
            .reload_from_files()
            .await
            .map_err(|e| crate::error::Error::Config(format!("Reload failed: {}", e)))
    }

    /// Get configuration change events
    pub fn subscribe_to_changes(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::config::runtime_reload::ConfigChangeEvent> {
        self.runtime_manager.subscribe_changes()
    }

    /// Emergency configuration override
    pub async fn emergency_override(&self, reason: String) -> Result<(), crate::error::Error> {
        // Create emergency configuration with conservative settings
        let emergency_config = ScalabilityConfig::for_platform(
            PlatformType::Embedded, // Most conservative settings
            PerformanceProfile::PowerSaver,
        );

        self.runtime_manager
            .emergency_override(emergency_config, reason)
            .await
            .map_err(|e| crate::error::Error::Config(format!("Emergency override failed: {}", e)))
    }

    /// Get platform type
    pub fn platform(&self) -> PlatformType {
        self.platform
    }

    /// Get performance profile
    pub fn profile(&self) -> PerformanceProfile {
        self.profile
    }
}

impl Drop for ConfigurationManager {
    fn drop(&mut self) {
        // Send shutdown signal when dropping
        let _ = self.shutdown_tx.send(true);

        // Abort all background tasks
        let handles = self.task_handles.lock().drain(..).collect::<Vec<_>>();
        for handle in handles {
            handle.abort();
        }
    }
}

/// Detect the current platform
fn detect_platform() -> PlatformType {
    #[cfg(target_os = "android")]
    return PlatformType::Mobile;

    #[cfg(target_os = "ios")]
    return PlatformType::Mobile;

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        // Check if running on embedded hardware
        if is_embedded_system() {
            return PlatformType::Embedded;
        }

        // Check if running as server
        if is_server_environment() {
            return PlatformType::Server;
        }

        PlatformType::Desktop
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )))]
    PlatformType::Embedded
}

/// Detect performance profile based on platform and resources
fn detect_performance_profile(platform: PlatformType) -> PerformanceProfile {
    match platform {
        PlatformType::Mobile => {
            if is_charging() {
                PerformanceProfile::Balanced
            } else {
                PerformanceProfile::PowerSaver
            }
        }
        PlatformType::Desktop => PerformanceProfile::Balanced,
        PlatformType::Server => PerformanceProfile::HighPerformance,
        PlatformType::Embedded => PerformanceProfile::PowerSaver,
    }
}

/// Check if running on embedded hardware
fn is_embedded_system() -> bool {
    // Check for Raspberry Pi
    if std::path::Path::new("/proc/device-tree/model").exists() {
        if let Ok(model) = std::fs::read_to_string("/proc/device-tree/model") {
            if model.to_lowercase().contains("raspberry pi") {
                return true;
            }
        }
    }

    // Check for limited memory (< 1GB suggests embedded)
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        let mb = kb / 1024;
                        return mb < 1024; // Less than 1GB
                    }
                }
            }
        }
    }

    false
}

/// Check if running in server environment
fn is_server_environment() -> bool {
    // Check for server-specific environment variables
    std::env::var("SERVER_MODE").is_ok()
        || std::env::var("BITCRAPS_SERVER").is_ok()
        || std::env::var("PRODUCTION").is_ok()
        || {
            // Check if running without GUI
            std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err()
        }
}

/// Check if device is charging (mobile platforms)
fn is_charging() -> bool {
    #[cfg(target_os = "android")]
    {
        // On Android, check battery status via system properties
        // This is simplified - actual implementation would use JNI
        false // Default to not charging for conservative behavior
    }

    #[cfg(target_os = "ios")]
    {
        // On iOS, would use iOS-specific APIs
        // This is simplified - actual implementation would use FFI
        false // Default to not charging for conservative behavior
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // On desktop platforms, assume always "charging" (plugged in)
        true
    }
}

impl ReloadSettings {
    /// Create platform-specific reload settings
    pub fn for_platform(platform: PlatformType) -> Self {
        match platform {
            PlatformType::Mobile => Self {
                enable_file_watching: false, // Battery optimization
                file_check_interval: std::time::Duration::from_secs(300), // 5 minutes
                enable_adaptive: true,
                adaptive_interval: std::time::Duration::from_secs(120), // 2 minutes
                max_changes_per_hour: 5,                                // Conservative
                require_confirmation: true,
                backup_configs: true,
                history_size: 20,
            },

            PlatformType::Desktop => Self::default(),

            PlatformType::Server => Self {
                enable_file_watching: true,
                file_check_interval: std::time::Duration::from_secs(10), // Fast response
                enable_adaptive: true,
                adaptive_interval: std::time::Duration::from_secs(30), // 30 seconds
                max_changes_per_hour: 50,                              // High frequency for servers
                require_confirmation: false,                           // Automated environments
                backup_configs: true,
                history_size: 100,
            },

            PlatformType::Embedded => Self {
                enable_file_watching: false, // Resource conservation
                file_check_interval: std::time::Duration::from_secs(600), // 10 minutes
                enable_adaptive: false,      // Too expensive
                adaptive_interval: std::time::Duration::from_secs(300), // 5 minutes
                max_changes_per_hour: 2,     // Very conservative
                require_confirmation: true,
                backup_configs: false, // Limited storage
                history_size: 5,
            },
        }
    }
}

/// Example configuration usage
pub async fn example_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration manager
    let config_manager = ConfigurationManager::auto_initialize(vec![
        PathBuf::from("config/scalability.json"),
        PathBuf::from("config/network.toml"),
    ])
    .await?;

    // Subscribe to configuration changes
    let mut changes = config_manager.subscribe_to_changes();

    // Spawn task to handle configuration changes
    tokio::spawn(async move {
        while let Ok(change_event) = changes.recv().await {
            log::info!("Configuration changed: {:?}", change_event);

            // React to configuration changes
            match change_event.change_type {
                crate::config::runtime_reload::ConfigChangeType::AutoAdapt => {
                    log::info!("Adaptive configuration adjustment applied");
                }
                crate::config::runtime_reload::ConfigChangeType::Emergency => {
                    log::warn!(
                        "Emergency configuration override: {}",
                        change_event.description
                    );
                }
                _ => {}
            }
        }
    });

    // Example of updating metrics for adaptive configuration
    let config_manager_clone = config_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            // Get current system metrics (this would be actual metrics)
            let metrics = PerformanceMetrics {
                network_latency: crate::performance::LatencyMetrics {
                    p50_ms: 20.0,
                    p95_ms: 50.0,
                    p99_ms: 100.0,
                    max_ms: 200.0,
                    ..Default::default()
                },
                consensus_performance: crate::performance::ConsensusMetrics {
                    proposal_time_ms: 25.0,
                    vote_time_ms: 15.0,
                    finalization_time_ms: 10.0,
                    fork_detection_time_ms: 5.0,
                    throughput_ops_per_sec: 75.0,
                    active_games: 3,
                    total_operations_processed: 12345,
                    consensus_failures: 5,
                    average_round_time_ms: 50.0,
                    validator_count: 8,
                    byzantine_threshold: 0.33,
                },
                memory_usage: crate::performance::MemoryMetrics {
                    heap_allocated_mb: 256.0,
                    heap_used_mb: 128.0,
                    cache_size_mb: 64.0,
                    buffer_pool_size_mb: 32.0,
                    total_memory_gb: 8.0,
                    available_memory_gb: 6.5,
                    swap_used_mb: 100.0,
                    virtual_memory_mb: 512.0,
                },
                cpu_usage: crate::performance::CpuMetrics {
                    utilization_percent: 45.0,
                    system_time_percent: 20.0,
                    user_time_percent: 25.0,
                    thread_count: 8,
                    core_count: 4,
                    frequency_mhz: 2800,
                    per_core_usage: vec![45.0, 47.0, 43.0, 46.0],
                    load_average: (1.2, 1.1, 1.0),
                },
                mesh_performance: crate::performance::MeshMetrics {
                    peer_discovery_time_ms: 500.0,
                    connection_establishment_time_ms: 200.0,
                    message_propagation_time_ms: 25.0,
                    network_diameter: 5,
                    average_hop_count: 2.5,
                    connected_peers: 12,
                    active_connections: 15,
                    bytes_sent: 1024000,
                    bytes_received: 890000,
                    messages_sent: 1500,
                    messages_received: 1350,
                    packet_loss_rate: 0.005,
                    bandwidth_utilization_percent: 65.0,
                },
                mobile_metrics: None,
                timestamp: std::time::SystemTime::now(),
                collection_time_ms: 50.0,
            };

            // Update metrics for adaptive configuration
            config_manager_clone.update_metrics(metrics).await;
        }
    });

    // Manual configuration reload example
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    config_manager.reload_configuration().await?;

    // Emergency override example
    if false {
        // Enable for testing
        config_manager
            .emergency_override("High memory pressure detected".to_string())
            .await?;
    }

    // Get current configuration
    let config = config_manager.get_scalability_config();
    log::info!(
        "Current max connections: {}",
        config.network.max_connections
    );
    log::info!("Current BLE MTU: {}", config.network.ble_max_payload_size);
    log::info!(
        "Platform: {:?}, Profile: {:?}",
        config_manager.platform(),
        config_manager.profile()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = detect_platform();
        assert!(matches!(
            platform,
            PlatformType::Mobile
                | PlatformType::Desktop
                | PlatformType::Server
                | PlatformType::Embedded
        ));
    }

    #[test]
    fn test_performance_profile_detection() {
        for platform in [
            PlatformType::Mobile,
            PlatformType::Desktop,
            PlatformType::Server,
            PlatformType::Embedded,
        ] {
            let profile = detect_performance_profile(platform);
            assert!(matches!(
                profile,
                PerformanceProfile::PowerSaver
                    | PerformanceProfile::Balanced
                    | PerformanceProfile::HighPerformance
            ));
        }
    }

    #[tokio::test]
    async fn test_configuration_manager_initialization() {
        let result = ConfigurationManager::initialize(
            PlatformType::Desktop,
            PerformanceProfile::Balanced,
            vec![],
        )
        .await;

        assert!(result.is_ok());
        let manager = result.unwrap();
        assert_eq!(manager.platform(), PlatformType::Desktop);
        assert_eq!(manager.profile(), PerformanceProfile::Balanced);
    }
}
