//! Runtime configuration reload system for BitCraps
//!
//! This module provides hot-reloading of configuration without requiring application restart,
//! enabling dynamic adaptation to changing conditions and runtime tuning.

use crate::config::scalability::{ScalabilityConfig, ScalabilityManager};
use crate::performance::PerformanceMetrics;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;

#[derive(Debug, Error)]
pub enum ReloadError {
    #[error("Failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Invalid configuration: {0}")]
    Validation(String),
    #[error("Configuration is locked for critical operation")]
    Locked,
}

/// Configuration change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub timestamp: std::time::SystemTime,
    pub change_type: ConfigChangeType,
    pub source: ConfigSource,
    pub description: String,
}

/// Type of configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigChangeType {
    /// Manual reload from file
    ManualReload,
    /// Automatic adaptive change
    AutoAdapt,
    /// Environment variable change
    EnvChange,
    /// API-triggered change
    ApiChange,
    /// Emergency override
    Emergency,
}

/// Source of configuration change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigSource {
    /// Configuration file
    File(PathBuf),
    /// Environment variables
    Environment,
    /// API endpoint
    Api,
    /// Adaptive algorithms
    Adaptive,
    /// Emergency system
    Emergency,
}

/// Runtime configuration manager with hot-reload capability
pub struct RuntimeConfigManager {
    /// Current scalability configuration
    scalability_manager: Arc<ScalabilityManager>,

    /// Configuration file paths
    config_paths: Vec<PathBuf>,

    /// Last modified times for config files
    file_modified: Arc<RwLock<std::collections::HashMap<PathBuf, std::time::SystemTime>>>,

    /// Configuration change event broadcaster
    change_broadcaster: broadcast::Sender<ConfigChangeEvent>,

    /// Reload lock to prevent concurrent reloads
    reload_lock: Arc<tokio::sync::Mutex<()>>,

    /// Configuration validation rules
    validator: ConfigValidator,

    /// Hot reload settings
    settings: ReloadSettings,

    /// Performance metrics for adaptive changes
    last_metrics: Arc<RwLock<Option<PerformanceMetrics>>>,
}

/// Configuration validation rules
#[derive(Debug, Clone)]
pub struct ConfigValidator {
    /// Minimum values for safety
    pub min_connections: usize,
    pub min_memory_mb: usize,
    pub min_threads: usize,
    pub max_timeout_secs: u64,

    /// Maximum values to prevent resource exhaustion
    pub max_connections: usize,
    pub max_memory_mb: usize,
    pub max_threads: usize,
    pub min_timeout_millis: u64,
}

/// Hot reload settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadSettings {
    /// Enable automatic file watching
    pub enable_file_watching: bool,

    /// File check interval
    pub file_check_interval: Duration,

    /// Enable adaptive configuration
    pub enable_adaptive: bool,

    /// Adaptive check interval
    pub adaptive_interval: Duration,

    /// Maximum config changes per hour
    pub max_changes_per_hour: u32,

    /// Require confirmation for large changes
    pub require_confirmation: bool,

    /// Backup configurations before changes
    pub backup_configs: bool,

    /// Configuration history size
    pub history_size: usize,
}

impl Default for ReloadSettings {
    fn default() -> Self {
        Self {
            enable_file_watching: true,
            file_check_interval: Duration::from_secs(30),
            enable_adaptive: true,
            adaptive_interval: Duration::from_secs(60),
            max_changes_per_hour: 10,
            require_confirmation: true,
            backup_configs: true,
            history_size: 50,
        }
    }
}

impl RuntimeConfigManager {
    /// Create a new runtime configuration manager
    pub fn new(
        initial_config: ScalabilityConfig,
        config_paths: Vec<PathBuf>,
        settings: ReloadSettings,
    ) -> Self {
        let (tx, _) = broadcast::channel(100);

        Self {
            scalability_manager: Arc::new(ScalabilityManager::new(initial_config)),
            config_paths,
            file_modified: Arc::new(RwLock::new(std::collections::HashMap::new())),
            change_broadcaster: tx,
            reload_lock: Arc::new(tokio::sync::Mutex::new(())),
            validator: ConfigValidator::default(),
            settings,
            last_metrics: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the runtime configuration manager
    pub async fn start(&self) -> Result<(), ReloadError> {
        // Initialize file modification times
        self.init_file_times().await?;

        // Start file watcher if enabled
        if self.settings.enable_file_watching {
            self.start_file_watcher().await;
        }

        // Start adaptive configuration if enabled
        if self.settings.enable_adaptive {
            self.start_adaptive_monitor().await;
        }

        Ok(())
    }

    /// Get current scalability configuration
    pub fn get_config(&self) -> ScalabilityConfig {
        self.scalability_manager.get_config()
    }

    /// Subscribe to configuration changes
    pub fn subscribe_changes(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.change_broadcaster.subscribe()
    }

    /// Manually reload configuration from files
    pub async fn reload_from_files(&self) -> Result<(), ReloadError> {
        let _lock = self.reload_lock.lock().await;

        for path in &self.config_paths {
            if path.exists() {
                self.reload_from_file(path.clone()).await?;
            }
        }

        Ok(())
    }

    /// Update configuration with validation
    pub async fn update_config(
        &self,
        new_config: ScalabilityConfig,
        source: ConfigSource,
    ) -> Result<(), ReloadError> {
        let _lock = self.reload_lock.lock().await;

        // Validate configuration
        self.validator.validate(&new_config)?;

        // Backup current configuration if enabled
        if self.settings.backup_configs {
            self.backup_current_config().await?;
        }

        // Apply new configuration
        self.scalability_manager.update_config(new_config);

        // Broadcast change event
        let event = ConfigChangeEvent {
            timestamp: std::time::SystemTime::now(),
            change_type: ConfigChangeType::ApiChange,
            source,
            description: "Configuration updated via API".to_string(),
        };

        let _ = self.change_broadcaster.send(event);

        Ok(())
    }

    /// Update performance metrics for adaptive configuration
    pub async fn update_metrics(&self, metrics: PerformanceMetrics) {
        *self.last_metrics.write().await = Some(metrics.clone());

        if self.settings.enable_adaptive {
            self.scalability_manager.adapt_to_metrics(&metrics);
        }
    }

    /// Get configuration change history
    pub async fn get_change_history(&self) -> Vec<ConfigChangeEvent> {
        // In a real implementation, this would be stored persistently
        vec![]
    }

    /// Emergency configuration override
    pub async fn emergency_override(
        &self,
        emergency_config: ScalabilityConfig,
        reason: String,
    ) -> Result<(), ReloadError> {
        let _lock = self.reload_lock.lock().await;

        log::warn!("Emergency configuration override: {}", reason);

        // Apply emergency configuration without normal validation
        self.scalability_manager.update_config(emergency_config);

        let event = ConfigChangeEvent {
            timestamp: std::time::SystemTime::now(),
            change_type: ConfigChangeType::Emergency,
            source: ConfigSource::Emergency,
            description: format!("Emergency override: {}", reason),
        };

        let _ = self.change_broadcaster.send(event);

        Ok(())
    }

    /// Initialize file modification times
    async fn init_file_times(&self) -> Result<(), ReloadError> {
        let mut file_times = self.file_modified.write().await;

        for path in &self.config_paths {
            if let Ok(metadata) = tokio::fs::metadata(path).await {
                if let Ok(modified) = metadata.modified() {
                    file_times.insert(path.clone(), modified);
                }
            }
        }

        Ok(())
    }

    /// Start file watcher task
    async fn start_file_watcher(&self) {
        let config_paths = self.config_paths.clone();
        let file_modified = self.file_modified.clone();
        let settings = self.settings.clone();
        let change_broadcaster = self.change_broadcaster.clone();
        let scalability_manager = self.scalability_manager.clone();
        let validator = self.validator.clone();

        tokio::spawn(async move {
            let mut interval = interval(settings.file_check_interval);

            loop {
                interval.tick().await;

                for path in &config_paths {
                    if let Ok(metadata) = tokio::fs::metadata(path).await {
                        if let Ok(modified) = metadata.modified() {
                            let should_reload = {
                                let file_times = file_modified.read().await;
                                file_times
                                    .get(path)
                                    .map_or(true, |&last_modified| modified > last_modified)
                            };

                            if should_reload {
                                log::info!("Configuration file changed: {:?}", path);

                                match Self::load_config_from_file(path).await {
                                    Ok(new_config) => {
                                        if let Err(e) = validator.validate(&new_config) {
                                            log::error!(
                                                "Invalid configuration in {:?}: {}",
                                                path,
                                                e
                                            );
                                            continue;
                                        }

                                        scalability_manager.update_config(new_config);

                                        let event = ConfigChangeEvent {
                                            timestamp: std::time::SystemTime::now(),
                                            change_type: ConfigChangeType::ManualReload,
                                            source: ConfigSource::File(path.clone()),
                                            description: format!("Reloaded from {:?}", path),
                                        };

                                        let _ = change_broadcaster.send(event);

                                        // Update file modification time
                                        file_modified.write().await.insert(path.clone(), modified);
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Failed to reload config from {:?}: {}",
                                            path,
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// Start adaptive configuration monitor
    async fn start_adaptive_monitor(&self) {
        let settings = self.settings.clone();
        let scalability_manager = self.scalability_manager.clone();
        let change_broadcaster = self.change_broadcaster.clone();
        let last_metrics = self.last_metrics.clone();

        tokio::spawn(async move {
            let mut interval = interval(settings.adaptive_interval);
            let mut last_adaptation = Instant::now();

            loop {
                interval.tick().await;

                if let Some(metrics) = last_metrics.read().await.clone() {
                    let config_before = scalability_manager.get_config();
                    scalability_manager.adapt_to_metrics(&metrics);
                    let config_after = scalability_manager.get_config();

                    // Check if configuration actually changed
                    if !Self::configs_equal(&config_before, &config_after) {
                        let event = ConfigChangeEvent {
                            timestamp: std::time::SystemTime::now(),
                            change_type: ConfigChangeType::AutoAdapt,
                            source: ConfigSource::Adaptive,
                            description: "Adaptive configuration adjustment based on metrics"
                                .to_string(),
                        };

                        let _ = change_broadcaster.send(event);
                        last_adaptation = Instant::now();

                        log::info!("Adaptive configuration adjustment applied");
                    }
                }
            }
        });
    }

    /// Load configuration from a specific file
    async fn load_config_from_file(path: &PathBuf) -> Result<ScalabilityConfig, ReloadError> {
        let content = tokio::fs::read_to_string(path).await?;

        // Support multiple formats
        let config = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            toml::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
                .map_err(serde_json::Error::io)?
        } else {
            // Try JSON first, then TOML
            serde_json::from_str(&content).or_else(|_| {
                toml::from_str(&content)
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
                    })
                    .map_err(serde_json::Error::io)
            })?
        };

        Ok(config)
    }

    /// Reload from specific file
    async fn reload_from_file(&self, path: PathBuf) -> Result<(), ReloadError> {
        let new_config = Self::load_config_from_file(&path).await?;

        // Validate configuration
        self.validator.validate(&new_config)?;

        // Apply configuration
        self.scalability_manager.update_config(new_config);

        // Broadcast change event
        let event = ConfigChangeEvent {
            timestamp: std::time::SystemTime::now(),
            change_type: ConfigChangeType::ManualReload,
            source: ConfigSource::File(path.clone()),
            description: format!("Manual reload from {:?}", path),
        };

        let _ = self.change_broadcaster.send(event);

        Ok(())
    }

    /// Backup current configuration
    async fn backup_current_config(&self) -> Result<(), ReloadError> {
        let config = self.scalability_manager.get_config();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let backup_path = format!("config_backup_{}.json", timestamp);
        let content = serde_json::to_string_pretty(&config)?;

        tokio::fs::write(&backup_path, content).await?;
        log::info!("Configuration backed up to {}", backup_path);

        Ok(())
    }

    /// Compare two configurations for equality (simplified)
    fn configs_equal(a: &ScalabilityConfig, b: &ScalabilityConfig) -> bool {
        // This is a simplified comparison - in reality you'd want a more sophisticated diff
        a.network.max_connections == b.network.max_connections
            && a.memory.max_heap_mb == b.memory.max_heap_mb
            && a.cpu.worker_threads == b.cpu.worker_threads
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self {
            min_connections: 1,
            min_memory_mb: 16,
            min_threads: 1,
            max_timeout_secs: 3600, // 1 hour max
            max_connections: 10000,
            max_memory_mb: 16384, // 16GB max
            max_threads: 256,
            min_timeout_millis: 10,
        }
    }
}

impl ConfigValidator {
    /// Validate a configuration
    pub fn validate(&self, config: &ScalabilityConfig) -> Result<(), ReloadError> {
        // Network validation
        if config.network.max_connections < self.min_connections {
            return Err(ReloadError::Validation(format!(
                "max_connections {} is below minimum {}",
                config.network.max_connections, self.min_connections
            )));
        }

        if config.network.max_connections > self.max_connections {
            return Err(ReloadError::Validation(format!(
                "max_connections {} exceeds maximum {}",
                config.network.max_connections, self.max_connections
            )));
        }

        // Memory validation
        if config.memory.max_heap_mb < self.min_memory_mb {
            return Err(ReloadError::Validation(format!(
                "max_heap_mb {} is below minimum {}",
                config.memory.max_heap_mb, self.min_memory_mb
            )));
        }

        if config.memory.max_heap_mb > self.max_memory_mb {
            return Err(ReloadError::Validation(format!(
                "max_heap_mb {} exceeds maximum {}",
                config.memory.max_heap_mb, self.max_memory_mb
            )));
        }

        // CPU validation
        if config.cpu.worker_threads < self.min_threads {
            return Err(ReloadError::Validation(format!(
                "worker_threads {} is below minimum {}",
                config.cpu.worker_threads, self.min_threads
            )));
        }

        if config.cpu.worker_threads > self.max_threads {
            return Err(ReloadError::Validation(format!(
                "worker_threads {} exceeds maximum {}",
                config.cpu.worker_threads, self.max_threads
            )));
        }

        // Timeout validation
        if config.timeouts.consensus.as_secs() > self.max_timeout_secs {
            return Err(ReloadError::Validation(format!(
                "consensus timeout {}s exceeds maximum {}s",
                config.timeouts.consensus.as_secs(),
                self.max_timeout_secs
            )));
        }

        if config.timeouts.critical_fast.as_millis() < self.min_timeout_millis as u128 {
            return Err(ReloadError::Validation(format!(
                "critical_fast timeout {}ms is below minimum {}ms",
                config.timeouts.critical_fast.as_millis(),
                self.min_timeout_millis
            )));
        }

        // Cross-validation
        if config.network.connection_pool_size > config.network.max_connections {
            return Err(ReloadError::Validation(
                "connection_pool_size cannot exceed max_connections".to_string(),
            ));
        }

        if config.memory.cache_size_mb > config.memory.max_heap_mb {
            return Err(ReloadError::Validation(
                "cache_size_mb cannot exceed max_heap_mb".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::scalability::{PerformanceProfile, PlatformType};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_config_validation() {
        let validator = ConfigValidator::default();

        // Valid config should pass
        let valid_config =
            ScalabilityConfig::for_platform(PlatformType::Desktop, PerformanceProfile::Balanced);
        assert!(validator.validate(&valid_config).is_ok());

        // Invalid config should fail
        let mut invalid_config = valid_config.clone();
        invalid_config.network.max_connections = 0; // Below minimum
        assert!(validator.validate(&invalid_config).is_err());

        invalid_config.network.max_connections = 20000; // Above maximum
        assert!(validator.validate(&invalid_config).is_err());
    }

    #[tokio::test]
    async fn test_file_based_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");

        // Create initial config file
        let initial_config =
            ScalabilityConfig::for_platform(PlatformType::Mobile, PerformanceProfile::PowerSaver);
        let content = serde_json::to_string_pretty(&initial_config).unwrap();
        tokio::fs::write(&config_path, content).await.unwrap();

        // Create manager
        let manager = RuntimeConfigManager::new(
            initial_config.clone(),
            vec![config_path.clone()],
            ReloadSettings::default(),
        );

        // Test manual reload
        let result = manager.reload_from_files().await;
        assert!(result.is_ok());

        // Verify configuration
        let loaded_config = manager.get_config();
        assert_eq!(loaded_config.platform, PlatformType::Mobile);
    }

    #[tokio::test]
    async fn test_adaptive_config_updates() {
        let initial_config =
            ScalabilityConfig::for_platform(PlatformType::Desktop, PerformanceProfile::Balanced);
        let manager =
            RuntimeConfigManager::new(initial_config.clone(), vec![], ReloadSettings::default());

        // Create mock metrics indicating high CPU usage
        let metrics = crate::performance::PerformanceMetrics {
            mesh_performance: crate::performance::MeshMetrics {
                peer_discovery_time_ms: 500.0,
                connection_establishment_time_ms: 200.0,
                message_propagation_time_ms: 100.0,
                network_diameter: 4,
                average_hop_count: 2.5,
                connected_peers: 8,
                active_connections: 6,
                bytes_sent: 1024000,
                bytes_received: 2048000,
                ..Default::default()
            },
            mobile_metrics: None,
            timestamp: std::time::SystemTime::now(),
            collection_time_ms: 50.0,
            cpu_usage: crate::performance::CpuMetrics {
                utilization_percent: 95.0,
                system_time_percent: 60.0,
                user_time_percent: 35.0,
                thread_count: 8,
                core_count: 4,
                frequency_mhz: 3000,
                per_core_usage: vec![90.0, 95.0, 85.0, 100.0],
                load_average: (2.0, 1.8, 1.6),
            },
            memory_usage: crate::performance::MemoryMetrics {
                heap_allocated_mb: 1024.0,
                heap_used_mb: 512.0,
                cache_size_mb: 256.0,
                buffer_pool_size_mb: 128.0,
                total_memory_gb: 16.0,
                available_memory_gb: 8.0,
                swap_used_mb: 256.0,
                virtual_memory_mb: 2048.0,
            },
            network_latency: crate::performance::LatencyMetrics {
                p50_ms: 45.0,
                p95_ms: 100.0,
                p99_ms: 200.0,
                max_ms: 500.0,
                ..Default::default()
            },
            consensus_performance: crate::performance::ConsensusMetrics {
                proposal_time_ms: 50.0,
                vote_time_ms: 30.0,
                finalization_time_ms: 100.0,
                fork_detection_time_ms: 20.0,
                throughput_ops_per_sec: 25.0,
                active_games: 3,
                total_operations_processed: 1000,
                consensus_failures: 10,
                average_round_time_ms: 100.0,
                validator_count: 5,
                byzantine_threshold: 0.33,
            },
        };

        // Update metrics should trigger adaptation
        manager.update_metrics(metrics).await;

        // Configuration should have adapted
        let adapted_config = manager.get_config();
        // In high CPU scenario, batch sizes should be reduced
        assert!(adapted_config.cpu.task_batch_size <= initial_config.cpu.task_batch_size);
    }
}
