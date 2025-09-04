#![cfg(feature = "wasm")]

//! WebAssembly runtime integration for BitCraps
//!
//! This module provides WebAssembly support for extending BitCraps functionality including:
//! - WASM module loading and execution
//! - Host function bindings for game logic
//! - Plugin system for custom games and strategies
//! - Sandboxed execution environment
//! - Memory management and security isolation

use crate::error::{Error, Result};
use crate::gaming::{CrapsGameEngine, GameAction, GameStateSnapshot, PlayerJoinData};
use crate::protocol::craps::GameState;
use crate::protocol::PeerId;
use crate::utils::{spawn_tracked, TaskType};
use bytes::Bytes;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{mpsc, RwLock, Mutex};
use uuid::Uuid;

pub mod runtime;
pub mod plugin_system;
pub mod host_functions;
pub mod memory_manager;

// Web bindings for browser integration
#[cfg(feature = "wasm")]
pub mod web_bindings;

pub use runtime::*;
pub use plugin_system::*;
pub use host_functions::*;
pub use memory_manager::*;

/// WASM runtime configuration
#[derive(Debug, Clone)]
pub struct WasmConfig {
    /// Maximum memory per WASM module (in bytes)
    pub max_memory: usize,
    /// Maximum execution time per call
    pub max_execution_time: Duration,
    /// Maximum stack size
    pub max_stack_size: usize,
    /// Enable debug information
    pub debug_mode: bool,
    /// Enable host function access
    pub allow_host_functions: bool,
    /// Plugin directory path
    pub plugin_directory: PathBuf,
    /// Fuel limit for execution metering
    pub fuel_limit: u64,
    /// Enable compilation cache
    pub enable_cache: bool,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory: 16 * 1024 * 1024,     // 16MB
            max_execution_time: Duration::from_secs(5),
            max_stack_size: 1024 * 1024,      // 1MB
            debug_mode: false,
            allow_host_functions: true,
            plugin_directory: PathBuf::from("plugins"),
            fuel_limit: 1_000_000,
            enable_cache: true,
        }
    }
}

/// WASM module metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleInfo {
    /// Module name
    pub name: String,
    /// Module version
    pub version: String,
    /// Module description
    pub description: String,
    /// Module author
    pub author: String,
    /// Required permissions
    pub permissions: Vec<String>,
    /// Supported game types
    pub game_types: Vec<String>,
    /// Host function dependencies
    pub host_functions: Vec<String>,
    /// Module entry points
    pub entry_points: HashMap<String, String>,
    /// Metadata checksum for integrity
    pub checksum: String,
}

/// WASM execution context
#[derive(Debug, Clone)]
pub struct WasmExecutionContext {
    /// Module instance ID
    pub instance_id: Uuid,
    /// Player who initiated the execution
    pub player_id: PeerId,
    /// Game context if applicable
    pub game_id: Option<Uuid>,
    /// Execution start time
    pub start_time: Instant,
    /// Fuel consumed
    pub fuel_consumed: u64,
    /// Memory usage
    pub memory_used: usize,
    /// Execution result
    pub result: Option<WasmExecutionResult>,
}

/// WASM execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmExecutionResult {
    /// Successful execution with return value
    Success { 
        value: WasmValue,
        fuel_consumed: u64,
        execution_time: Duration,
    },
    /// Execution error
    Error { 
        message: String,
        error_type: WasmErrorType,
    },
    /// Execution timeout
    Timeout,
    /// Out of fuel
    OutOfFuel,
    /// Memory limit exceeded
    OutOfMemory,
}

/// WASM value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Null,
}

/// WASM error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmErrorType {
    CompileError,
    RuntimeError,
    TrapError,
    SecurityViolation,
    InvalidInput,
    HostFunctionError,
}

/// Game plugin interface
pub trait GamePlugin {
    /// Get plugin metadata
    fn metadata(&self) -> &WasmModuleInfo;
    
    /// Initialize the plugin
    fn initialize(&mut self, context: &WasmExecutionContext) -> Result<()>;
    
    /// Execute game logic
    fn execute_game_action(&mut self, action: GameAction, state: &GameState) -> Result<GameState>;
    
    /// Validate move legality
    fn validate_move(&self, action: &GameAction, state: &GameState) -> Result<bool>;
    
    /// Calculate game outcome
    fn calculate_outcome(&self, state: &GameState) -> Result<HashMap<PeerId, f64>>;
    
    /// Cleanup plugin resources
    fn cleanup(&mut self) -> Result<()>;
}

/// WASM runtime statistics
#[derive(Debug, Clone, Default)]
pub struct WasmRuntimeStats {
    pub modules_loaded: usize,
    pub active_instances: usize,
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub timeout_executions: u64,
    pub total_fuel_consumed: u64,
    pub total_execution_time: Duration,
    pub memory_usage: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Main WASM runtime
pub struct WasmRuntime {
    config: WasmConfig,
    modules: Arc<DashMap<String, Arc<WasmModule>>>,
    instances: Arc<DashMap<Uuid, Arc<RwLock<WasmInstance>>>>,
    plugins: Arc<DashMap<String, Box<dyn GamePlugin + Send + Sync>>>,
    host_functions: Arc<WasmHostFunctions>,
    memory_manager: Arc<WasmMemoryManager>,
    stats: Arc<RwLock<WasmRuntimeStats>>,
    execution_contexts: Arc<DashMap<Uuid, WasmExecutionContext>>,
    running: Arc<RwLock<bool>>,
}

impl WasmRuntime {
    /// Create a new WASM runtime
    pub fn new(config: WasmConfig) -> Self {
        let host_functions = Arc::new(WasmHostFunctions::new());
        let memory_manager = Arc::new(WasmMemoryManager::new(config.max_memory));

        Self {
            config,
            modules: Arc::new(DashMap::new()),
            instances: Arc::new(DashMap::new()),
            plugins: Arc::new(DashMap::new()),
            host_functions,
            memory_manager,
            stats: Arc::new(RwLock::new(WasmRuntimeStats::default())),
            execution_contexts: Arc::new(DashMap::new()),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the WASM runtime
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Load plugins from plugin directory
        self.load_plugins().await?;

        // Start cleanup task
        self.start_cleanup_task().await;

        log::info!("WASM runtime started");
        Ok(())
    }

    /// Stop the WASM runtime
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;

        // Cleanup all instances
        for entry in self.instances.iter() {
            let instance_id = *entry.key();
            let _ = self.cleanup_instance(instance_id).await;
        }

        // Cleanup all modules
        self.modules.clear();
        self.plugins.clear();

        log::info!("WASM runtime stopped");
        Ok(())
    }

    /// Load a WASM module from bytes
    pub async fn load_module(&self, name: String, wasm_bytes: Bytes) -> Result<Arc<WasmModule>> {
        let module = WasmModule::new(name.clone(), wasm_bytes, self.config.clone()).await?;
        let module_arc = Arc::new(module);
        
        self.modules.insert(name.clone(), module_arc.clone());
        
        let mut stats = self.stats.write().await;
        stats.modules_loaded += 1;

        log::info!("Loaded WASM module: {}", name);
        Ok(module_arc)
    }

    /// Load a WASM module from file
    pub async fn load_module_from_file<P: AsRef<Path>>(&self, name: String, path: P) -> Result<Arc<WasmModule>> {
        let wasm_bytes = fs::read(path).await
            .map_err(|e| Error::Wasm(format!("Failed to read WASM file: {}", e)))?;
        
        self.load_module(name, Bytes::from(wasm_bytes)).await
    }

    /// Create a new WASM instance
    pub async fn create_instance(&self, module_name: &str, player_id: PeerId) -> Result<Uuid> {
        let module = self.modules.get(module_name)
            .ok_or_else(|| Error::Wasm(format!("Module '{}' not found", module_name)))?
            .clone();

        let instance_id = Uuid::new_v4();
        let instance = WasmInstance::new(
            instance_id,
            module,
            self.host_functions.clone(),
            self.memory_manager.clone(),
        ).await?;

        let instance_arc = Arc::new(RwLock::new(instance));
        self.instances.insert(instance_id, instance_arc);

        // Create execution context
        let context = WasmExecutionContext {
            instance_id,
            player_id,
            game_id: None,
            start_time: Instant::now(),
            fuel_consumed: 0,
            memory_used: 0,
            result: None,
        };
        self.execution_contexts.insert(instance_id, context);

        let mut stats = self.stats.write().await;
        stats.active_instances += 1;

        log::debug!("Created WASM instance {} for module {}", instance_id, module_name);
        Ok(instance_id)
    }

    /// Execute a function in a WASM instance
    pub async fn execute_function(
        &self,
        instance_id: Uuid,
        function_name: &str,
        args: Vec<WasmValue>,
    ) -> Result<WasmExecutionResult> {
        let instance = self.instances.get(&instance_id)
            .ok_or_else(|| Error::Wasm(format!("Instance {} not found", instance_id)))?
            .clone();

        let start_time = Instant::now();
        let timeout = self.config.max_execution_time;

        // Update execution context
        if let Some(mut context) = self.execution_contexts.get_mut(&instance_id) {
            context.start_time = start_time;
        }

        let result = match tokio::time::timeout(timeout, async {
            let mut instance_guard = instance.write().await;
            instance_guard.execute_function(function_name, args).await
        }).await {
            Ok(Ok(value)) => {
                let execution_time = start_time.elapsed();
                WasmExecutionResult::Success {
                    value,
                    fuel_consumed: 0, // Would be tracked in real implementation
                    execution_time,
                }
            }
            Ok(Err(e)) => {
                WasmExecutionResult::Error {
                    message: e.to_string(),
                    error_type: WasmErrorType::RuntimeError,
                }
            }
            Err(_) => WasmExecutionResult::Timeout,
        };

        // Update execution context with result
        if let Some(mut context) = self.execution_contexts.get_mut(&instance_id) {
            context.result = Some(result.clone());
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_executions += 1;
        match &result {
            WasmExecutionResult::Success { execution_time, .. } => {
                stats.successful_executions += 1;
                stats.total_execution_time += *execution_time;
            }
            WasmExecutionResult::Timeout => {
                stats.timeout_executions += 1;
            }
            _ => {
                stats.failed_executions += 1;
            }
        }

        Ok(result)
    }

    /// Execute game logic through a plugin
    pub async fn execute_game_plugin(
        &self,
        plugin_name: &str,
        action: GameAction,
        state: &GameState,
        player_id: PeerId,
    ) -> Result<GameState> {
        // Create a temporary instance for plugin execution
        let instance_id = self.create_instance(plugin_name, player_id).await?;

        // Set game context
        if let Some(mut context) = self.execution_contexts.get_mut(&instance_id) {
            context.game_id = Some(Uuid::new_v4()); // Would use actual game ID
        }

        // Execute plugin logic
        let args = vec![
            WasmValue::String(serde_json::to_string(&action).unwrap_or_default()),
            WasmValue::String(serde_json::to_string(&state).unwrap_or_default()),
        ];

        let result = self.execute_function(instance_id, "execute_game_action", args).await?;

        match result {
            WasmExecutionResult::Success { value, .. } => {
                match value {
                    WasmValue::String(json_str) => {
                        let new_state: GameState = serde_json::from_str(&json_str)
                            .map_err(|e| Error::Wasm(format!("Failed to parse game state: {}", e)))?;
                        
                        // Cleanup instance
                        self.cleanup_instance(instance_id).await?;
                        
                        Ok(new_state)
                    }
                    _ => Err(Error::Wasm("Plugin returned invalid result type".to_string())),
                }
            }
            WasmExecutionResult::Error { message, .. } => {
                self.cleanup_instance(instance_id).await?;
                Err(Error::Wasm(format!("Plugin execution failed: {}", message)))
            }
            WasmExecutionResult::Timeout => {
                self.cleanup_instance(instance_id).await?;
                Err(Error::Wasm("Plugin execution timed out".to_string()))
            }
            _ => {
                self.cleanup_instance(instance_id).await?;
                Err(Error::Wasm("Plugin execution failed".to_string()))
            }
        }
    }

    /// Load plugins from the plugin directory
    async fn load_plugins(&self) -> Result<()> {
        let plugin_dir = &self.config.plugin_directory;
        
        if !plugin_dir.exists() {
            log::info!("Plugin directory does not exist: {:?}", plugin_dir);
            return Ok(());
        }

        let mut entries = fs::read_dir(plugin_dir).await
            .map_err(|e| Error::Wasm(format!("Failed to read plugin directory: {}", e)))?;

        let mut loaded_count = 0;
        
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| Error::Wasm(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_module_from_file(name.to_string(), &path).await {
                        Ok(_) => {
                            loaded_count += 1;
                            log::info!("Loaded plugin: {} from {:?}", name, path);
                        }
                        Err(e) => {
                            log::error!("Failed to load plugin {}: {}", name, e);
                        }
                    }
                }
            }
        }

        log::info!("Loaded {} WASM plugins", loaded_count);
        Ok(())
    }

    /// Cleanup a WASM instance
    async fn cleanup_instance(&self, instance_id: Uuid) -> Result<()> {
        if let Some((_, instance)) = self.instances.remove(&instance_id) {
            let mut instance_guard = instance.write().await;
            instance_guard.cleanup().await?;
        }

        self.execution_contexts.remove(&instance_id);

        let mut stats = self.stats.write().await;
        stats.active_instances = stats.active_instances.saturating_sub(1);

        Ok(())
    }

    /// Start background cleanup task
    async fn start_cleanup_task(&self) {
        let instances = self.instances.clone();
        let execution_contexts = self.execution_contexts.clone();
        let running = self.running.clone();
        let config = self.config.clone();

        spawn_tracked("wasm_cleanup", TaskType::Background, async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            while *running.read().await {
                interval.tick().await;

                let mut to_cleanup = Vec::new();
                let now = Instant::now();

                // Find stale instances
                for entry in execution_contexts.iter() {
                    let instance_id = *entry.key();
                    let context = entry.value();

                    if now.duration_since(context.start_time) > config.max_execution_time * 2 {
                        to_cleanup.push(instance_id);
                    }
                }

                // Cleanup stale instances
                for instance_id in to_cleanup {
                    if let Some((_, instance)) = instances.remove(&instance_id) {
                        if let Ok(mut instance_guard) = instance.try_write() {
                            let _ = instance_guard.cleanup().await;
                        }
                    }
                    execution_contexts.remove(&instance_id);
                    log::debug!("Cleaned up stale WASM instance: {}", instance_id);
                }
            }
        }).await;
    }

    /// Get runtime statistics
    pub async fn get_stats(&self) -> WasmRuntimeStats {
        let mut stats = self.stats.read().await.clone();
        stats.active_instances = self.instances.len();
        stats.modules_loaded = self.modules.len();
        stats.memory_usage = self.memory_manager.get_total_usage().await;
        stats
    }

    /// Register a native plugin
    pub fn register_plugin(&self, name: String, plugin: Box<dyn GamePlugin + Send + Sync>) {
        self.plugins.insert(name.clone(), plugin);
        log::info!("Registered native plugin: {}", name);
    }

    /// Validate a WASM module before loading
    pub async fn validate_module(&self, wasm_bytes: &[u8]) -> Result<WasmModuleInfo> {
        // In a real implementation, this would:
        // 1. Parse WASM module
        // 2. Validate structure
        // 3. Check for malicious patterns
        // 4. Extract metadata
        // 5. Verify checksums

        // For now, return a placeholder
        Ok(WasmModuleInfo {
            name: "unknown".to_string(),
            version: "1.0.0".to_string(),
            description: "WASM module".to_string(),
            author: "unknown".to_string(),
            permissions: vec![],
            game_types: vec!["craps".to_string()],
            host_functions: vec![],
            entry_points: HashMap::new(),
            checksum: "placeholder".to_string(),
        })
    }

    /// List available modules
    pub fn list_modules(&self) -> Vec<String> {
        self.modules.iter().map(|entry| entry.key().clone()).collect()
    }

    /// List active instances
    pub fn list_instances(&self) -> Vec<Uuid> {
        self.instances.iter().map(|entry| *entry.key()).collect()
    }

    /// Get execution context for an instance
    pub fn get_execution_context(&self, instance_id: Uuid) -> Option<WasmExecutionContext> {
        self.execution_contexts.get(&instance_id).map(|entry| entry.value().clone())
    }
}

/// WASM runtime builder for easy configuration
pub struct WasmRuntimeBuilder {
    config: WasmConfig,
}

impl WasmRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            config: WasmConfig::default(),
        }
    }

    pub fn max_memory(mut self, bytes: usize) -> Self {
        self.config.max_memory = bytes;
        self
    }

    pub fn max_execution_time(mut self, duration: Duration) -> Self {
        self.config.max_execution_time = duration;
        self
    }

    pub fn debug_mode(mut self, enable: bool) -> Self {
        self.config.debug_mode = enable;
        self
    }

    pub fn plugin_directory<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config.plugin_directory = path.as_ref().to_path_buf();
        self
    }

    pub fn fuel_limit(mut self, limit: u64) -> Self {
        self.config.fuel_limit = limit;
        self
    }

    pub fn enable_cache(mut self, enable: bool) -> Self {
        self.config.enable_cache = enable;
        self
    }

    pub fn build(self) -> WasmRuntime {
        WasmRuntime::new(self.config)
    }
}

impl Default for WasmRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasm_runtime_creation() {
        let runtime = WasmRuntimeBuilder::new()
            .max_memory(8 * 1024 * 1024)
            .max_execution_time(Duration::from_secs(10))
            .debug_mode(true)
            .build();

        assert_eq!(runtime.config.max_memory, 8 * 1024 * 1024);
        assert_eq!(runtime.config.max_execution_time, Duration::from_secs(10));
        assert!(runtime.config.debug_mode);
    }

    #[tokio::test]
    async fn test_execution_context() {
        let peer_id: PeerId = [1u8; 32];
        let instance_id = Uuid::new_v4();
        
        let context = WasmExecutionContext {
            instance_id,
            player_id: peer_id,
            game_id: None,
            start_time: Instant::now(),
            fuel_consumed: 0,
            memory_used: 0,
            result: None,
        };

        assert_eq!(context.instance_id, instance_id);
        assert_eq!(context.player_id, peer_id);
        assert!(context.game_id.is_none());
        assert!(context.result.is_none());
    }

    #[tokio::test]
    async fn test_wasm_value_serialization() {
        let value = WasmValue::String("test".to_string());
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: WasmValue = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            WasmValue::String(s) => assert_eq!(s, "test"),
            _ => panic!("Unexpected value type"),
        }
    }
}