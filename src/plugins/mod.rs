#![cfg(feature = "plugins")]

//! Plugin System for BitCraps Casino Games
//!
//! This module provides a comprehensive plugin architecture for extending
//! the BitCraps platform with additional casino games through:
//!
//! ## Core Plugin Features
//! - Dynamic plugin loading and unloading
//! - Sandboxed execution environment
//! - Plugin lifecycle management
//! - Inter-plugin communication
//! - Security isolation and validation
//!
//! ## Game Plugin Architecture
//! - Standardized game plugin API
//! - State synchronization across peers
//! - Event-driven plugin communication
//! - Configuration management
//! - Version compatibility checks
//!
//! ## Security & Safety
//! - Resource quotas and limits
//! - Capability-based security model
//! - Plugin validation and signing
//! - Runtime monitoring and controls

pub mod core;
pub mod loader;
pub mod registry;
pub mod sandbox;
pub mod communication;
pub mod examples;

// Re-export core plugin types
pub use core::{
    GamePlugin, PluginCapability, PluginInfo, PluginManager, PluginManagerConfig,
    PluginResult, PluginError, PluginEvent, PluginState, PluginStatistics
};

// Re-export plugin loader
pub use loader::{
    PluginLoader, PluginLoadResult, LoaderConfig, PluginSignature
};

// Re-export plugin registry
pub use registry::{
    PluginRegistry, RegistryConfig, PluginEntry, PluginVersion, RegistryError
};

// Re-export sandbox
pub use sandbox::{
    PluginSandbox, SandboxConfig, ResourceQuota, SecurityPolicy, SandboxViolation
};

// Re-export communication
pub use communication::{
    PluginCommunicator, MessageBus, PluginMessage, MessageType, CommunicationError
};

// Re-export example plugins
pub use examples::{
    BlackjackPlugin, PokerPlugin, RoulettePlugin, SlotMachinePlugin
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global plugin system instance
static mut PLUGIN_SYSTEM: Option<Arc<PluginSystem>> = None;

/// Main plugin system coordinator
pub struct PluginSystem {
    manager: Arc<PluginManager>,
    loader: Arc<PluginLoader>,
    registry: Arc<RwLock<PluginRegistry>>,
    sandbox: Arc<PluginSandbox>,
    communicator: Arc<PluginCommunicator>,
}

impl PluginSystem {
    /// Initialize the global plugin system
    pub async fn initialize(config: PluginSystemConfig) -> PluginResult<Arc<Self>> {
        let manager = Arc::new(PluginManager::new(config.manager)?);
        let loader = Arc::new(PluginLoader::new(config.loader)?);
        let registry = Arc::new(RwLock::new(PluginRegistry::new(config.registry)?));
        let sandbox = Arc::new(PluginSandbox::new(config.sandbox)?);
        let communicator = Arc::new(PluginCommunicator::new(config.communication)?);

        let system = Arc::new(Self {
            manager,
            loader,
            registry,
            sandbox,
            communicator,
        });

        // Store globally
        unsafe {
            PLUGIN_SYSTEM = Some(Arc::clone(&system));
        }

        // Initialize subsystems
        system.start_background_tasks().await?;

        Ok(system)
    }

    /// Get global plugin system instance
    pub fn instance() -> Option<Arc<Self>> {
        unsafe { PLUGIN_SYSTEM.as_ref().cloned() }
    }

    /// Load a plugin from file
    pub async fn load_plugin(&self, path: &str) -> PluginResult<String> {
        // Load and validate plugin
        let plugin_data = self.loader.load(path).await?;
        
        // Register in registry
        let plugin_id = {
            let mut registry = self.registry.write().await;
            registry.register(plugin_data.metadata.clone())?
        };

        // Create sandbox for plugin
        self.sandbox.create_environment(&plugin_id).await?;

        // Initialize plugin in manager
        self.manager.initialize_plugin(&plugin_id, plugin_data).await?;

        // Set up communication channels
        self.communicator.setup_plugin_channels(&plugin_id).await?;

        Ok(plugin_id)
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Stop plugin
        self.manager.stop_plugin(plugin_id).await?;

        // Cleanup sandbox
        self.sandbox.destroy_environment(plugin_id).await?;

        // Remove from registry
        let mut registry = self.registry.write().await;
        registry.unregister(plugin_id)?;

        // Cleanup communication
        self.communicator.cleanup_plugin_channels(plugin_id).await?;

        Ok(())
    }

    /// Get list of available plugins
    pub async fn list_plugins(&self) -> Vec<PluginInfo> {
        let registry = self.registry.read().await;
        registry.list_plugins()
    }

    /// Get plugin statistics
    pub async fn get_statistics(&self) -> PluginSystemStatistics {
        let registry = self.registry.read().await;
        let manager_stats = self.manager.get_statistics().await;
        let sandbox_stats = self.sandbox.get_statistics().await;

        PluginSystemStatistics {
            total_plugins: registry.count(),
            active_plugins: manager_stats.active_plugins,
            total_messages: self.communicator.get_message_count().await,
            sandbox_violations: sandbox_stats.total_violations,
            uptime_seconds: manager_stats.uptime_seconds,
        }
    }

    /// Start background maintenance tasks
    async fn start_background_tasks(&self) -> PluginResult<()> {
        // Plugin health monitoring
        let manager = Arc::clone(&self.manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = manager.health_check().await {
                    tracing::warn!("Plugin health check failed: {:?}", e);
                }
            }
        });

        // Sandbox monitoring
        let sandbox = Arc::clone(&self.sandbox);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = sandbox.monitor_resources().await {
                    tracing::warn!("Sandbox monitoring failed: {:?}", e);
                }
            }
        });

        // Communication cleanup
        let communicator = Arc::clone(&self.communicator);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                communicator.cleanup_stale_channels().await;
            }
        });

        Ok(())
    }
}

/// Plugin system configuration
#[derive(Debug, Clone)]
pub struct PluginSystemConfig {
    pub manager: PluginManagerConfig,
    pub loader: LoaderConfig,
    pub registry: RegistryConfig,
    pub sandbox: SandboxConfig,
    pub communication: communication::Config,
}

impl Default for PluginSystemConfig {
    fn default() -> Self {
        Self {
            manager: PluginManagerConfig::default(),
            loader: LoaderConfig::default(),
            registry: RegistryConfig::default(),
            sandbox: SandboxConfig::default(),
            communication: communication::Config::default(),
        }
    }
}

/// Plugin system statistics
#[derive(Debug, Clone)]
pub struct PluginSystemStatistics {
    pub total_plugins: usize,
    pub active_plugins: usize,
    pub total_messages: u64,
    pub sandbox_violations: u64,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_system_initialization() {
        let config = PluginSystemConfig::default();
        let system = PluginSystem::initialize(config).await.unwrap();
        
        let stats = system.get_statistics().await;
        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.active_plugins, 0);
    }

    #[tokio::test]
    async fn test_plugin_lifecycle() {
        let config = PluginSystemConfig::default();
        let system = PluginSystem::initialize(config).await.unwrap();
        
        // Test loading a plugin (would need actual plugin file)
        // let plugin_id = system.load_plugin("test_plugin.so").await.unwrap();
        // assert!(!plugin_id.is_empty());
        
        let plugins = system.list_plugins().await;
        assert_eq!(plugins.len(), 0); // No plugins loaded yet
    }
}