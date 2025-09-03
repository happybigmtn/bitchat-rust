//! Core Plugin Architecture for BitCraps
//!
//! This module defines the fundamental plugin system components including
//! the plugin trait, plugin manager, and core plugin lifecycle management.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::gaming::{GameAction, GameActionResult, GameSession};
use rand::{CryptoRng, RngCore};

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Core trait that all game plugins must implement
#[async_trait]
pub trait GamePlugin: Send + Sync {
    /// Get plugin information
    fn get_info(&self) -> PluginInfo;

    /// Get plugin capabilities
    fn get_capabilities(&self) -> Vec<PluginCapability>;

    /// Initialize the plugin with given configuration
    async fn initialize(&mut self, config: HashMap<String, serde_json::Value>) -> PluginResult<()>;

    /// Start the plugin
    async fn start(&mut self) -> PluginResult<()>;

    /// Stop the plugin
    async fn stop(&mut self) -> PluginResult<()>;

    /// Handle plugin events
    async fn handle_event(&mut self, event: PluginEvent) -> PluginResult<()>;

    /// Process game action from player
    async fn process_game_action(
        &mut self,
        session_id: &str,
        player_id: &str,
        action: GameAction,
    ) -> PluginResult<GameActionResult>;

    /// Get current game state for session
    async fn get_game_state(&self, session_id: &str) -> PluginResult<serde_json::Value>;

    /// Synchronize game state with peers
    async fn sync_state(
        &mut self,
        session_id: &str,
        peer_states: Vec<serde_json::Value>,
    ) -> PluginResult<serde_json::Value>;

    /// Validate player action before processing
    async fn validate_action(
        &self,
        session_id: &str,
        player_id: &str,
        action: &GameAction,
    ) -> PluginResult<bool>;

    /// Handle player joining game session
    async fn on_player_join(
        &mut self,
        session_id: &str,
        player_id: &str,
        initial_balance: u64,
    ) -> PluginResult<()>;

    /// Handle player leaving game session
    async fn on_player_leave(&mut self, session_id: &str, player_id: &str) -> PluginResult<()>;

    /// Handle game session creation
    async fn on_session_create(&mut self, session_id: &str, config: HashMap<String, serde_json::Value>) -> PluginResult<()>;

    /// Handle game session ending
    async fn on_session_end(&mut self, session_id: &str) -> PluginResult<()>;

    /// Get plugin health status
    async fn health_check(&self) -> PluginResult<PluginHealth>;

    /// Get plugin statistics
    async fn get_statistics(&self) -> PluginStatistics;

    /// Handle shutdown gracefully
    async fn shutdown(&mut self) -> PluginResult<()>;
}

/// Plugin information metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub website: Option<String>,
    pub api_version: String,
    pub minimum_platform_version: String,
    pub game_type: GameType,
    pub supported_features: Vec<String>,
    pub dependencies: Vec<PluginDependency>,
}

/// Plugin dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub minimum_version: String,
    pub required: bool,
}

/// Types of casino games supported
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameType {
    Blackjack,
    Poker,
    Roulette,
    Slots,
    Baccarat,
    Craps,
    Other(String),
}

/// Plugin capabilities that can be enabled/disabled
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginCapability {
    /// Can process real money bets
    RealMoneyGaming,
    /// Can access network resources
    NetworkAccess,
    /// Can store persistent data
    DataStorage,
    /// Can use cryptographic functions
    Cryptography,
    /// Can interact with other plugins
    InterPluginCommunication,
    /// Can access player information
    PlayerDataAccess,
    /// Can generate random numbers
    RandomNumberGeneration,
    /// Can send push notifications
    Notifications,
    /// Can access device sensors (mobile)
    SensorAccess,
    /// Custom capability
    Custom(String),
}

/// Plugin lifecycle states
#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Uninitialized,
    Initializing,
    Initialized,
    Starting,
    Running,
    Stopping,
    Stopped,
    Error(String),
}

/// Plugin health status
#[derive(Debug, Clone)]
pub struct PluginHealth {
    pub state: PluginState,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
    pub last_heartbeat: SystemTime,
    pub error_count: u64,
    pub warnings: Vec<String>,
}

/// Plugin statistics
#[derive(Debug, Clone)]
pub struct PluginStatistics {
    pub sessions_created: u64,
    pub actions_processed: u64,
    pub errors_encountered: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub uptime_seconds: u64,
    pub average_response_time_ms: f64,
}

/// Events that can be sent to plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// System is starting up
    SystemStartup,
    /// System is shutting down
    SystemShutdown,
    /// Configuration has changed
    ConfigurationUpdated(HashMap<String, serde_json::Value>),
    /// New player connected to platform
    PlayerConnected { player_id: String },
    /// Player disconnected from platform
    PlayerDisconnected { player_id: String },
    /// Game session created
    SessionCreated { session_id: String, game_type: GameType },
    /// Game session ended
    SessionEnded { session_id: String },
    /// Network event occurred
    NetworkEvent { event_type: String, data: serde_json::Value },
    /// Custom event from another plugin
    CustomEvent { from_plugin: String, event_type: String, data: serde_json::Value },
}

/// Plugin manager that handles all loaded plugins
pub struct PluginManager {
    /// Configuration
    config: PluginManagerConfig,
    /// Loaded plugins
    plugins: Arc<RwLock<HashMap<String, Box<dyn GamePlugin>>>>,
    /// Plugin states
    plugin_states: Arc<RwLock<HashMap<String, PluginState>>>,
    /// Event broadcaster
    event_sender: broadcast::Sender<PluginEvent>,
    /// Statistics
    stats: Arc<PluginManagerStats>,
    /// Random number generator
    rng: Arc<dyn CryptoRng + RngCore + Send + Sync>,
}

impl PluginManager {
    /// Create new plugin manager
    pub fn new(config: PluginManagerConfig) -> PluginResult<Self> {
        let (event_sender, _) = broadcast::channel(1000);
        let rng = Arc::new(rand::rngs::OsRng);

        Ok(Self {
            config,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_states: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            stats: Arc::new(PluginManagerStats::new()),
            rng,
        })
    }

    /// Initialize a plugin with given data
    pub async fn initialize_plugin(
        &self,
        plugin_id: &str,
        plugin_data: PluginLoadData,
    ) -> PluginResult<()> {
        // Set state to initializing
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Initializing);
        }

        // Create plugin instance (this would typically involve loading from shared library)
        let mut plugin = plugin_data.create_instance()?;

        // Initialize plugin
        plugin.initialize(plugin_data.config).await?;

        // Store plugin
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(plugin_id.to_string(), plugin);
        }

        // Update state
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Initialized);
        }

        self.stats.plugins_initialized.fetch_add(1, Ordering::Relaxed);
        tracing::info!("Initialized plugin: {}", plugin_id);

        Ok(())
    }

    /// Start a plugin
    pub async fn start_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Set state to starting
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Starting);
        }

        // Start plugin
        {
            let mut plugins = self.plugins.write().await;
            let plugin = plugins
                .get_mut(plugin_id)
                .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;
            
            plugin.start().await?;
        }

        // Update state
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Running);
        }

        // Broadcast startup event
        let event = PluginEvent::SystemStartup;
        if let Err(e) = self.event_sender.send(event) {
            tracing::debug!("No event subscribers: {:?}", e);
        }

        self.stats.plugins_started.fetch_add(1, Ordering::Relaxed);
        tracing::info!("Started plugin: {}", plugin_id);

        Ok(())
    }

    /// Stop a plugin
    pub async fn stop_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Set state to stopping
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Stopping);
        }

        // Stop plugin
        {
            let mut plugins = self.plugins.write().await;
            let plugin = plugins
                .get_mut(plugin_id)
                .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;
            
            plugin.stop().await?;
        }

        // Update state
        {
            let mut states = self.plugin_states.write().await;
            states.insert(plugin_id.to_string(), PluginState::Stopped);
        }

        self.stats.plugins_stopped.fetch_add(1, Ordering::Relaxed);
        tracing::info!("Stopped plugin: {}", plugin_id);

        Ok(())
    }

    /// Remove a plugin completely
    pub async fn remove_plugin(&self, plugin_id: &str) -> PluginResult<()> {
        // Stop plugin first
        self.stop_plugin(plugin_id).await?;

        // Remove from collections
        {
            let mut plugins = self.plugins.write().await;
            plugins.remove(plugin_id);
        }
        {
            let mut states = self.plugin_states.write().await;
            states.remove(plugin_id);
        }

        tracing::info!("Removed plugin: {}", plugin_id);
        Ok(())
    }

    /// Get plugin state
    pub async fn get_plugin_state(&self, plugin_id: &str) -> Option<PluginState> {
        let states = self.plugin_states.read().await;
        states.get(plugin_id).cloned()
    }

    /// List all plugins
    pub async fn list_plugins(&self) -> Vec<String> {
        let plugins = self.plugins.read().await;
        plugins.keys().cloned().collect()
    }

    /// Broadcast event to all plugins
    pub async fn broadcast_event(&self, event: PluginEvent) -> PluginResult<()> {
        let plugins = self.plugins.read().await;
        
        for (plugin_id, plugin) in plugins.iter() {
            let plugin_ptr = plugin.as_ref() as *const dyn GamePlugin;
            // SAFETY: We're holding a read lock on plugins, so the plugin won't be removed
            let plugin = unsafe { &*plugin_ptr };
            
            if let Err(e) = plugin.handle_event(event.clone()).await {
                tracing::warn!("Plugin {} failed to handle event: {:?}", plugin_id, e);
                self.stats.event_errors.fetch_add(1, Ordering::Relaxed);
            }
        }

        self.stats.events_broadcast.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Perform health check on all plugins
    pub async fn health_check(&self) -> PluginResult<Vec<(String, PluginHealth)>> {
        let plugins = self.plugins.read().await;
        let mut health_reports = Vec::new();

        for (plugin_id, plugin) in plugins.iter() {
            let plugin_ptr = plugin.as_ref() as *const dyn GamePlugin;
            // SAFETY: We're holding a read lock on plugins
            let plugin = unsafe { &*plugin_ptr };
            
            match plugin.health_check().await {
                Ok(health) => health_reports.push((plugin_id.clone(), health)),
                Err(e) => {
                    tracing::warn!("Health check failed for plugin {}: {:?}", plugin_id, e);
                    let error_health = PluginHealth {
                        state: PluginState::Error(format!("Health check failed: {:?}", e)),
                        memory_usage_mb: 0,
                        cpu_usage_percent: 0.0,
                        last_heartbeat: SystemTime::UNIX_EPOCH,
                        error_count: 1,
                        warnings: vec![format!("Health check failed: {:?}", e)],
                    };
                    health_reports.push((plugin_id.clone(), error_health));
                }
            }
        }

        Ok(health_reports)
    }

    /// Get manager statistics
    pub async fn get_statistics(&self) -> PluginManagerStatistics {
        let plugins = self.plugins.read().await;
        let active_plugins = plugins.len();
        drop(plugins);

        PluginManagerStatistics {
            active_plugins,
            plugins_initialized: self.stats.plugins_initialized.load(Ordering::Relaxed),
            plugins_started: self.stats.plugins_started.load(Ordering::Relaxed),
            plugins_stopped: self.stats.plugins_stopped.load(Ordering::Relaxed),
            events_broadcast: self.stats.events_broadcast.load(Ordering::Relaxed),
            event_errors: self.stats.event_errors.load(Ordering::Relaxed),
            uptime_seconds: self.stats.start_time.elapsed().as_secs(),
        }
    }

    /// Subscribe to plugin events
    pub fn subscribe_events(&self) -> broadcast::Receiver<PluginEvent> {
        self.event_sender.subscribe()
    }
}

/// Plugin manager configuration
#[derive(Debug, Clone)]
pub struct PluginManagerConfig {
    pub max_plugins: usize,
    pub event_buffer_size: usize,
    pub health_check_interval_seconds: u64,
    pub plugin_timeout_seconds: u64,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            max_plugins: 100,
            event_buffer_size: 1000,
            health_check_interval_seconds: 30,
            plugin_timeout_seconds: 60,
        }
    }
}

/// Plugin manager statistics
pub struct PluginManagerStats {
    pub plugins_initialized: AtomicU64,
    pub plugins_started: AtomicU64,
    pub plugins_stopped: AtomicU64,
    pub events_broadcast: AtomicU64,
    pub event_errors: AtomicU64,
    pub start_time: std::time::Instant,
}

impl PluginManagerStats {
    pub fn new() -> Self {
        Self {
            plugins_initialized: AtomicU64::new(0),
            plugins_started: AtomicU64::new(0),
            plugins_stopped: AtomicU64::new(0),
            events_broadcast: AtomicU64::new(0),
            event_errors: AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        }
    }
}

/// Plugin manager statistics snapshot
#[derive(Debug, Clone)]
pub struct PluginManagerStatistics {
    pub active_plugins: usize,
    pub plugins_initialized: u64,
    pub plugins_started: u64,
    pub plugins_stopped: u64,
    pub events_broadcast: u64,
    pub event_errors: u64,
    pub uptime_seconds: u64,
}

/// Plugin load data used during initialization
pub struct PluginLoadData {
    pub metadata: PluginInfo,
    pub config: HashMap<String, serde_json::Value>,
    pub factory: Box<dyn PluginFactory>,
}

impl PluginLoadData {
    /// Create plugin instance
    pub fn create_instance(self) -> PluginResult<Box<dyn GamePlugin>> {
        self.factory.create()
    }
}

/// Plugin factory trait for creating instances
pub trait PluginFactory: Send + Sync {
    fn create(&self) -> PluginResult<Box<dyn GamePlugin>>;
}

/// Plugin error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Plugin configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Plugin validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Plugin security violation: {0}")]
    SecurityViolation(String),
    
    #[error("Plugin communication error: {0}")]
    CommunicationError(String),
    
    #[error("Plugin resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),
    
    #[error("Plugin dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
    
    #[error("Plugin version incompatible: {0}")]
    VersionIncompatible(String),
    
    #[error("Plugin runtime error: {0}")]
    RuntimeError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let config = PluginManagerConfig::default();
        let manager = PluginManager::new(config).unwrap();
        
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 0);
        
        let stats = manager.get_statistics().await;
        assert_eq!(stats.active_plugins, 0);
    }

    #[tokio::test]
    async fn test_plugin_states() {
        let config = PluginManagerConfig::default();
        let manager = PluginManager::new(config).unwrap();
        
        let state = manager.get_plugin_state("nonexistent").await;
        assert!(state.is_none());
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let config = PluginManagerConfig::default();
        let manager = PluginManager::new(config).unwrap();
        
        let _receiver = manager.subscribe_events();
        // Test would send events and verify reception
    }
}