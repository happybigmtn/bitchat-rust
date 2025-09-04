//! WASM plugin system for BitCraps
//!
//! This module provides a plugin architecture that allows custom game logic,
//! strategies, and extensions to be loaded as WASM modules with secure
//! sandboxing and resource management.

use crate::error::{Error, Result};
use crate::gaming::{CrapsGameEngine, GameAction, GameStateSnapshot, PlayerJoinData};
use crate::protocol::PeerId;
use crate::wasm::{
    WasmConfig, WasmExecutionContext, WasmExecutionResult, WasmModule, WasmModuleInfo,
    WasmRuntime, WasmValue,
};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Plugin registry for managing WASM plugins
pub struct PluginRegistry {
    /// Registered plugins
    plugins: Arc<DashMap<String, Arc<PluginInfo>>>,
    /// Plugin categories
    categories: Arc<DashMap<PluginCategory, Vec<String>>>,
    /// Plugin dependencies
    dependencies: Arc<DashMap<String, Vec<String>>>,
    /// Active plugin instances
    active_instances: Arc<DashMap<Uuid, Arc<RwLock<PluginInstance>>>>,
    /// WASM runtime
    runtime: Arc<WasmRuntime>,
    /// Plugin lifecycle hooks
    lifecycle_hooks: Arc<RwLock<PluginLifecycleHooks>>,
}

/// Plugin information and metadata
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin metadata
    pub metadata: WasmModuleInfo,
    /// Plugin bytecode
    pub bytecode: Bytes,
    /// Plugin configuration
    pub config: PluginConfig,
    /// Plugin category
    pub category: PluginCategory,
    /// Load time
    pub loaded_at: Instant,
    /// Usage statistics
    pub stats: PluginStats,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub name: String,
    /// Enable plugin
    pub enabled: bool,
    /// Resource limits
    pub resource_limits: PluginResourceLimits,
    /// Permissions granted to plugin
    pub permissions: Vec<String>,
    /// Plugin-specific settings
    pub settings: HashMap<String, serde_json::Value>,
    /// Auto-load on startup
    pub auto_load: bool,
    /// Priority for loading order
    pub priority: u32,
}

/// Plugin resource limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResourceLimits {
    pub max_memory: usize,
    pub max_execution_time: Duration,
    pub max_fuel: u64,
    pub max_instances: usize,
}

impl Default for PluginResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 8 * 1024 * 1024, // 8MB
            max_execution_time: Duration::from_secs(5),
            max_fuel: 500_000,
            max_instances: 10,
        }
    }
}

/// Plugin categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCategory {
    /// Game logic plugins
    GameLogic,
    /// Betting strategy plugins
    Strategy,
    /// User interface plugins
    UserInterface,
    /// Network transport plugins
    Transport,
    /// Authentication plugins
    Authentication,
    /// Analytics and statistics plugins
    Analytics,
    /// Security and anti-cheat plugins
    Security,
    /// Utility plugins
    Utility,
}

/// Plugin statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginStats {
    pub load_count: u64,
    pub execution_count: u64,
    #[serde(with = "humantime_serde")]
    pub total_execution_time: Duration,
    pub error_count: u64,
    #[serde(skip)]
    pub last_used: Option<Instant>,
    pub memory_usage: usize,
}

/// Plugin lifecycle hooks
pub struct PluginLifecycleHooks {
    pub on_load: Vec<Box<dyn Fn(&PluginInfo) -> Result<()> + Send + Sync>>,
    pub on_unload: Vec<Box<dyn Fn(&str) -> Result<()> + Send + Sync>>,
    pub on_execute: Vec<Box<dyn Fn(&str, &WasmExecutionContext) -> Result<()> + Send + Sync>>,
    pub on_error: Vec<Box<dyn Fn(&str, &Error) -> Result<()> + Send + Sync>>,
}

impl Default for PluginLifecycleHooks {
    fn default() -> Self {
        Self {
            on_load: Vec::new(),
            on_unload: Vec::new(),
            on_execute: Vec::new(),
            on_error: Vec::new(),
        }
    }
}

/// Active plugin instance
pub struct PluginInstance {
    /// Instance ID
    pub id: Uuid,
    /// Plugin name
    pub plugin_name: String,
    /// WASM instance ID
    pub wasm_instance_id: Uuid,
    /// Player who loaded the plugin
    pub loaded_by: PeerId,
    /// Instance configuration
    pub config: PluginInstanceConfig,
    /// Creation time
    pub created_at: Instant,
    /// Last activity
    pub last_activity: Instant,
    /// Instance state
    pub state: PluginInstanceState,
}

/// Plugin instance configuration
#[derive(Debug, Clone)]
pub struct PluginInstanceConfig {
    pub game_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub resource_limits: PluginResourceLimits,
    pub context_data: HashMap<String, serde_json::Value>,
}

/// Plugin instance state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PluginInstanceState {
    Loading,
    Ready,
    Running,
    Suspended,
    Error,
    Terminated,
}

/// Plugin execution context
#[derive(Debug, Clone)]
pub struct PluginExecutionContext {
    pub instance_id: Uuid,
    pub plugin_name: String,
    pub player_id: PeerId,
    pub game_id: Option<Uuid>,
    pub function_name: String,
    pub start_time: Instant,
}

/// Plugin API trait for game logic plugins
#[async_trait]
pub trait GameLogicPlugin {
    /// Initialize the plugin
    async fn initialize(&mut self, context: &PluginExecutionContext) -> Result<()>;

    /// Execute a game action
    async fn execute_action(
        &mut self,
        action: &GameAction,
        state: &GameStateSnapshot,
        context: &PluginExecutionContext,
    ) -> Result<GameStateSnapshot>;

    /// Validate a game action
    async fn validate_action(
        &self,
        action: &GameAction,
        state: &GameStateSnapshot,
        context: &PluginExecutionContext,
    ) -> Result<bool>;

    /// Calculate game outcomes
    async fn calculate_outcomes(
        &self,
        state: &GameStateSnapshot,
        context: &PluginExecutionContext,
    ) -> Result<HashMap<PeerId, f64>>;

    /// Handle plugin events
    async fn handle_event(
        &mut self,
        event: &PluginEvent,
        context: &PluginExecutionContext,
    ) -> Result<()>;

    /// Cleanup plugin resources
    async fn cleanup(&mut self) -> Result<()>;
}

/// Plugin events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    GameStarted { game_id: Uuid },
    GameEnded { game_id: Uuid, outcome: HashMap<PeerId, f64> },
    PlayerJoined { game_id: Uuid, player_id: PeerId },
    PlayerLeft { game_id: Uuid, player_id: PeerId },
    BetPlaced { game_id: Uuid, player_id: PeerId, amount: f64 },
    DiceRolled { game_id: Uuid, result: [u8; 2] },
    Custom { event_type: String, data: serde_json::Value },
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new(runtime: Arc<WasmRuntime>) -> Self {
        Self {
            plugins: Arc::new(DashMap::new()),
            categories: Arc::new(DashMap::new()),
            dependencies: Arc::new(DashMap::new()),
            active_instances: Arc::new(DashMap::new()),
            runtime,
            lifecycle_hooks: Arc::new(RwLock::new(PluginLifecycleHooks::default())),
        }
    }

    /// Load a plugin from file
    pub async fn load_plugin_from_file<P: AsRef<Path>>(&self, plugin_path: P) -> Result<String> {
        let path = plugin_path.as_ref();
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::Wasm("Invalid plugin filename".to_string()))?
            .to_string();

        // Read plugin bytecode
        let bytecode = fs::read(path)
            .await
            .map_err(|e| Error::Wasm(format!("Failed to read plugin file: {}", e)))?;

        // Read plugin metadata if exists
        let metadata_path = path.with_extension("toml");
        let config = if metadata_path.exists() {
            let config_content = fs::read_to_string(&metadata_path)
                .await
                .map_err(|e| Error::Wasm(format!("Failed to read plugin config: {}", e)))?;
            toml::from_str::<PluginConfig>(&config_content)
                .map_err(|e| Error::Wasm(format!("Invalid plugin config: {}", e)))?
        } else {
            PluginConfig {
                name: plugin_name.clone(),
                enabled: true,
                resource_limits: PluginResourceLimits::default(),
                permissions: vec![],
                settings: HashMap::new(),
                auto_load: false,
                priority: 100,
            }
        };

        self.load_plugin(plugin_name, Bytes::from(bytecode), config).await
    }

    /// Load a plugin from bytecode
    pub async fn load_plugin(
        &self,
        name: String,
        bytecode: Bytes,
        config: PluginConfig,
    ) -> Result<String> {
        // Validate plugin bytecode
        let metadata = self.runtime.validate_module(&bytecode).await?;

        // Determine plugin category
        let category = self.determine_plugin_category(&metadata)?;

        // Load WASM module
        let module = self.runtime.load_module(name.clone(), bytecode.clone()).await?;

        // Create plugin info
        let plugin_info = Arc::new(PluginInfo {
            metadata,
            bytecode,
            config,
            category,
            loaded_at: Instant::now(),
            stats: PluginStats::default(),
        });

        // Register plugin
        self.plugins.insert(name.clone(), plugin_info.clone());

        // Update category index
        self.categories
            .entry(category)
            .or_insert_with(Vec::new)
            .push(name.clone());

        // Run lifecycle hooks
        let hooks = self.lifecycle_hooks.read().await;
        for hook in &hooks.on_load {
            if let Err(e) = hook(&plugin_info) {
                log::warn!("Plugin load hook failed for '{}': {}", name, e);
            }
        }

        log::info!("Loaded plugin '{}' in category {:?}", name, category);
        Ok(name)
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, name: &str) -> Result<()> {
        // Remove from registry
        let plugin_info = self
            .plugins
            .remove(name)
            .ok_or_else(|| Error::Wasm(format!("Plugin '{}' not found", name)))?
            .1;

        // Remove from category index
        if let Some(mut plugins) = self.categories.get_mut(&plugin_info.category) {
            plugins.retain(|p| p != name);
        }

        // Terminate active instances
        let instances_to_remove: Vec<Uuid> = self
            .active_instances
            .iter()
            .filter(|entry| {
                if let Ok(instance) = entry.value().try_read() {
                    instance.plugin_name == name
                } else {
                    false
                }
            })
            .map(|entry| *entry.key())
            .collect();

        for instance_id in instances_to_remove {
            self.terminate_instance(instance_id).await?;
        }

        // Run lifecycle hooks
        let hooks = self.lifecycle_hooks.read().await;
        for hook in &hooks.on_unload {
            if let Err(e) = hook(name) {
                log::warn!("Plugin unload hook failed for '{}': {}", name, e);
            }
        }

        log::info!("Unloaded plugin '{}'", name);
        Ok(())
    }

    /// Create a plugin instance
    pub async fn create_instance(
        &self,
        plugin_name: &str,
        player_id: PeerId,
        config: PluginInstanceConfig,
    ) -> Result<Uuid> {
        let plugin_info = self
            .plugins
            .get(plugin_name)
            .ok_or_else(|| Error::Wasm(format!("Plugin '{}' not found", plugin_name)))?;

        if !plugin_info.config.enabled {
            return Err(Error::Wasm(format!("Plugin '{}' is disabled", plugin_name)));
        }

        // Check instance limits
        let active_instances: usize = self
            .active_instances
            .iter()
            .filter(|entry| {
                if let Ok(instance) = entry.value().try_read() {
                    instance.plugin_name == plugin_name
                } else {
                    false
                }
            })
            .count();

        if active_instances >= plugin_info.config.resource_limits.max_instances {
            return Err(Error::Wasm(format!(
                "Maximum instances reached for plugin '{}'",
                plugin_name
            )));
        }

        // Create WASM instance
        let wasm_instance_id = self.runtime.create_instance(plugin_name, player_id).await?;

        // Set permissions for WASM instance
        self.runtime
            .set_permissions(wasm_instance_id, config.permissions.clone())
            .await;

        // Create plugin instance
        let instance_id = Uuid::new_v4();
        let instance = PluginInstance {
            id: instance_id,
            plugin_name: plugin_name.to_string(),
            wasm_instance_id,
            loaded_by: player_id,
            config,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            state: PluginInstanceState::Loading,
        };

        let instance_arc = Arc::new(RwLock::new(instance));
        self.active_instances.insert(instance_id, instance_arc.clone());

        // Initialize instance
        {
            let mut instance_guard = instance_arc.write().await;
            instance_guard.state = PluginInstanceState::Ready;
        }

        log::debug!(
            "Created plugin instance {} for plugin '{}'",
            instance_id,
            plugin_name
        );
        Ok(instance_id)
    }

    /// Execute a function in a plugin instance
    pub async fn execute_plugin_function(
        &self,
        instance_id: Uuid,
        function_name: &str,
        args: Vec<WasmValue>,
    ) -> Result<WasmExecutionResult> {
        let instance = self
            .active_instances
            .get(&instance_id)
            .ok_or_else(|| Error::Wasm(format!("Plugin instance {} not found", instance_id)))?;

        let (wasm_instance_id, plugin_name) = {
            let mut instance_guard = instance.write().await;
            instance_guard.last_activity = Instant::now();
            instance_guard.state = PluginInstanceState::Running;
            (instance_guard.wasm_instance_id, instance_guard.plugin_name.clone())
        };

        // Create execution context
        let exec_context = PluginExecutionContext {
            instance_id,
            plugin_name: plugin_name.clone(),
            player_id: {
                let instance_guard = instance.read().await;
                instance_guard.loaded_by
            },
            game_id: {
                let instance_guard = instance.read().await;
                instance_guard.config.game_id
            },
            function_name: function_name.to_string(),
            start_time: Instant::now(),
        };

        // Run execution hooks
        let hooks = self.lifecycle_hooks.read().await;
        for hook in &hooks.on_execute {
            if let Err(e) = hook(&plugin_name, &WasmExecutionContext {
                instance_id: wasm_instance_id,
                player_id: exec_context.player_id,
                game_id: exec_context.game_id,
                start_time: exec_context.start_time,
                fuel_consumed: 0,
                memory_used: 0,
                result: None,
            }) {
                log::warn!("Plugin execution hook failed: {}", e);
            }
        }
        drop(hooks);

        // Execute function
        let result = self
            .runtime
            .execute_function(wasm_instance_id, function_name, args)
            .await?;

        // Update instance state
        {
            let mut instance_guard = instance.write().await;
            instance_guard.state = match &result {
                WasmExecutionResult::Success { .. } => PluginInstanceState::Ready,
                _ => PluginInstanceState::Error,
            };
        }

        // Update plugin statistics
        if let Some(mut plugin_info) = self.plugins.get_mut(&plugin_name) {
            let mut stats = &mut Arc::make_mut(&mut plugin_info).stats;
            stats.execution_count += 1;
            if let WasmExecutionResult::Success { execution_time, .. } = &result {
                stats.total_execution_time += *execution_time;
            } else {
                stats.error_count += 1;
            }
            stats.last_used = Some(Instant::now());
        }

        Ok(result)
    }

    /// Terminate a plugin instance
    pub async fn terminate_instance(&self, instance_id: Uuid) -> Result<()> {
        if let Some((_, instance)) = self.active_instances.remove(&instance_id) {
            let wasm_instance_id = {
                let mut instance_guard = instance.write().await;
                instance_guard.state = PluginInstanceState::Terminated;
                instance_guard.wasm_instance_id
            };

            // Cleanup WASM instance
            self.runtime.cleanup_instance(wasm_instance_id).await?;

            log::debug!("Terminated plugin instance {}", instance_id);
        }

        Ok(())
    }

    /// Get plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<Arc<PluginInfo>> {
        self.plugins.get(name).map(|entry| entry.value().clone())
    }

    /// List plugins by category
    pub fn list_plugins_by_category(&self, category: PluginCategory) -> Vec<String> {
        self.categories
            .get(&category)
            .map(|entry| entry.value().clone())
            .unwrap_or_default()
    }

    /// List all plugins
    pub fn list_all_plugins(&self) -> Vec<String> {
        self.plugins.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Get active instances for a plugin
    pub fn get_active_instances(&self, plugin_name: &str) -> Vec<Uuid> {
        self.active_instances
            .iter()
            .filter_map(|entry| {
                if let Ok(instance) = entry.value().try_read() {
                    if instance.plugin_name == plugin_name {
                        Some(*entry.key())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Add lifecycle hook
    pub async fn add_lifecycle_hook<F>(&self, hook_type: LifecycleHookType, hook: F)
    where
        F: Fn(&str) -> Result<()> + Send + Sync + 'static,
    {
        let mut hooks = self.lifecycle_hooks.write().await;
        match hook_type {
            LifecycleHookType::OnUnload => hooks.on_unload.push(Box::new(hook)),
            // Other hook types would be handled similarly
            _ => {
                log::warn!("Hook type {:?} not yet implemented", hook_type);
            }
        }
    }

    /// Determine plugin category from metadata
    fn determine_plugin_category(&self, metadata: &WasmModuleInfo) -> Result<PluginCategory> {
        // Check game types to determine category
        for game_type in &metadata.game_types {
            match game_type.as_str() {
                "craps" | "game_logic" => return Ok(PluginCategory::GameLogic),
                "strategy" | "betting" => return Ok(PluginCategory::Strategy),
                "ui" | "interface" => return Ok(PluginCategory::UserInterface),
                "transport" | "network" => return Ok(PluginCategory::Transport),
                "auth" | "authentication" => return Ok(PluginCategory::Authentication),
                "analytics" | "stats" => return Ok(PluginCategory::Analytics),
                "security" | "anticheat" => return Ok(PluginCategory::Security),
                _ => continue,
            }
        }

        // Default to utility category
        Ok(PluginCategory::Utility)
    }

    /// Get registry statistics
    pub async fn get_stats(&self) -> PluginRegistryStats {
        let mut stats = PluginRegistryStats::default();

        stats.total_plugins = self.plugins.len();
        stats.active_instances = self.active_instances.len();

        // Count plugins by category
        for entry in self.categories.iter() {
            let count = entry.value().len();
            stats.plugins_by_category.insert(*entry.key(), count);
        }

        // Sum plugin statistics
        for entry in self.plugins.iter() {
            let plugin_stats = &entry.value().stats;
            stats.total_executions += plugin_stats.execution_count;
            stats.total_errors += plugin_stats.error_count;
            stats.total_execution_time += plugin_stats.total_execution_time;
        }

        stats
    }
}

/// Lifecycle hook types
#[derive(Debug, Clone, Copy)]
pub enum LifecycleHookType {
    OnLoad,
    OnUnload,
    OnExecute,
    OnError,
}

/// Plugin registry statistics
#[derive(Debug, Clone, Default)]
pub struct PluginRegistryStats {
    pub total_plugins: usize,
    pub active_instances: usize,
    pub plugins_by_category: HashMap<PluginCategory, usize>,
    pub total_executions: u64,
    pub total_errors: u64,
    pub total_execution_time: Duration,
}

/// Plugin manager for high-level plugin operations
pub struct PluginManager {
    registry: Arc<PluginRegistry>,
    auto_load_plugins: Vec<String>,
    plugin_directory: PathBuf,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(registry: Arc<PluginRegistry>, plugin_directory: PathBuf) -> Self {
        Self {
            registry,
            auto_load_plugins: Vec::new(),
            plugin_directory,
        }
    }

    /// Initialize plugin manager and load auto-load plugins
    pub async fn initialize(&mut self) -> Result<()> {
        // Scan plugin directory
        self.scan_plugin_directory().await?;

        // Load auto-load plugins
        self.load_auto_load_plugins().await?;

        log::info!("Plugin manager initialized");
        Ok(())
    }

    /// Scan plugin directory for plugins
    async fn scan_plugin_directory(&self) -> Result<()> {
        if !self.plugin_directory.exists() {
            log::info!("Plugin directory does not exist: {:?}", self.plugin_directory);
            return Ok(());
        }

        let mut entries = fs::read_dir(&self.plugin_directory)
            .await
            .map_err(|e| Error::Wasm(format!("Failed to read plugin directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::Wasm(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    log::debug!("Found plugin: {} at {:?}", name, path);
                }
            }
        }

        Ok(())
    }

    /// Load auto-load plugins
    async fn load_auto_load_plugins(&mut self) -> Result<()> {
        for plugin_name in &self.auto_load_plugins.clone() {
            let plugin_path = self.plugin_directory.join(format!("{}.wasm", plugin_name));
            
            if plugin_path.exists() {
                match self.registry.load_plugin_from_file(&plugin_path).await {
                    Ok(_) => {
                        log::info!("Auto-loaded plugin: {}", plugin_name);
                    }
                    Err(e) => {
                        log::error!("Failed to auto-load plugin {}: {}", plugin_name, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Add plugin to auto-load list
    pub fn add_auto_load_plugin(&mut self, plugin_name: String) {
        if !self.auto_load_plugins.contains(&plugin_name) {
            self.auto_load_plugins.push(plugin_name);
        }
    }

    /// Remove plugin from auto-load list
    pub fn remove_auto_load_plugin(&mut self, plugin_name: &str) {
        self.auto_load_plugins.retain(|p| p != plugin_name);
    }

    /// Get registry reference
    pub fn registry(&self) -> &Arc<PluginRegistry> {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::WasmRuntimeBuilder;

    #[tokio::test]
    async fn test_plugin_registry_creation() {
        let runtime = Arc::new(WasmRuntimeBuilder::new().build());
        let registry = PluginRegistry::new(runtime);

        assert_eq!(registry.list_all_plugins().len(), 0);
        let stats = registry.get_stats().await;
        assert_eq!(stats.total_plugins, 0);
    }

    #[tokio::test]
    async fn test_plugin_config() {
        let config = PluginConfig {
            name: "test_plugin".to_string(),
            enabled: true,
            resource_limits: PluginResourceLimits::default(),
            permissions: vec!["game_read".to_string()],
            settings: HashMap::new(),
            auto_load: false,
            priority: 100,
        };

        assert_eq!(config.name, "test_plugin");
        assert!(config.enabled);
        assert_eq!(config.permissions.len(), 1);
    }

    #[tokio::test]
    async fn test_plugin_categories() {
        let runtime = Arc::new(WasmRuntimeBuilder::new().build());
        let registry = PluginRegistry::new(runtime);

        // Test category determination
        let metadata = WasmModuleInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test Author".to_string(),
            permissions: vec![],
            game_types: vec!["craps".to_string()],
            host_functions: vec![],
            entry_points: HashMap::new(),
            checksum: "test".to_string(),
        };

        let category = registry.determine_plugin_category(&metadata).unwrap();
        assert_eq!(category, PluginCategory::GameLogic);
    }

    #[tokio::test]
    async fn test_plugin_instance_lifecycle() {
        let peer_id: PeerId = [1u8; 32];
        let instance = PluginInstance {
            id: Uuid::new_v4(),
            plugin_name: "test_plugin".to_string(),
            wasm_instance_id: Uuid::new_v4(),
            loaded_by: peer_id,
            config: PluginInstanceConfig {
                game_id: None,
                permissions: vec![],
                resource_limits: PluginResourceLimits::default(),
                context_data: HashMap::new(),
            },
            created_at: Instant::now(),
            last_activity: Instant::now(),
            state: PluginInstanceState::Loading,
        };

        assert_eq!(instance.state, PluginInstanceState::Loading);
        assert_eq!(instance.plugin_name, "test_plugin");
        assert_eq!(instance.loaded_by, peer_id);
    }
}