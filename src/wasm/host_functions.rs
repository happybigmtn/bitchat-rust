//! Host function bindings for WASM modules
//!
//! This module provides the interface between WASM modules and the BitCraps host environment,
//! allowing WASM code to interact with game state, network operations, and system resources
//! in a secure and controlled manner.

use crate::error::{Error, Result};
use crate::gaming::{CrapsGameEngine, GameAction, GameStateSnapshot, PlayerJoinData};
use crate::protocol::{BitchatPacket, PeerId};
use crate::wasm::WasmValue;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Host function registry and execution environment
pub struct WasmHostFunctions {
    /// Function registry
    functions: HashMap<String, HostFunction>,
    /// Game state access
    game_states: Arc<DashMap<Uuid, GameStateSnapshot>>,
    /// PlayerJoinData registry
    players: Arc<DashMap<PeerId, PlayerJoinData>>,
    /// Call statistics
    call_count: AtomicU64,
    /// Security context
    security_context: Arc<RwLock<SecurityContext>>,
    /// Resource limits
    resource_limits: ResourceLimits,
}

/// Host function definition
pub struct HostFunction {
    pub name: String,
    pub signature: HostFunctionSignature,
    pub handler: Box<dyn Fn(&HostFunctionContext, Vec<WasmValue>) -> Result<WasmValue> + Send + Sync>,
    pub permission_required: Option<String>,
    pub resource_cost: ResourceCost,
}

/// Host function signature
#[derive(Debug, Clone)]
pub struct HostFunctionSignature {
    pub parameters: Vec<WasmValueType>,
    pub returns: WasmValueType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmValueType {
    I32,
    I64,
    F32,
    F64,
    String,
    Bytes,
    Void,
}

/// Host function execution context
#[derive(Debug)]
pub struct HostFunctionContext {
    pub caller_id: Uuid,
    pub caller_peer: PeerId,
    pub game_id: Option<Uuid>,
    pub timestamp: Instant,
    pub permissions: Vec<String>,
}

/// Security context for host function access
#[derive(Debug, Clone, Default)]
pub struct SecurityContext {
    pub allowed_permissions: HashMap<Uuid, Vec<String>>,
    pub denied_functions: HashMap<Uuid, Vec<String>>,
    pub resource_quotas: HashMap<Uuid, ResourceQuota>,
}

/// Resource quota for WASM instances
#[derive(Debug, Clone)]
pub struct ResourceQuota {
    pub max_calls_per_second: u64,
    pub max_memory_access: usize,
    pub max_network_requests: u64,
    pub max_game_state_modifications: u64,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_calls_per_second: 100,
            max_memory_access: 1024 * 1024, // 1MB
            max_network_requests: 10,
            max_game_state_modifications: 50,
        }
    }
}

/// Resource cost for function execution
#[derive(Debug, Clone, Default)]
pub struct ResourceCost {
    pub cpu_cost: u64,
    pub memory_cost: usize,
    pub network_cost: u64,
}

/// Resource limits configuration
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_cpu_per_call: u64,
    pub max_memory_per_call: usize,
    pub max_network_per_call: u64,
    pub max_execution_time: Duration,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_per_call: 10_000,
            max_memory_per_call: 1024 * 1024,
            max_network_per_call: 100,
            max_execution_time: Duration::from_millis(100),
        }
    }
}

impl WasmHostFunctions {
    /// Create a new host functions registry
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        
        // Register standard host functions
        Self::register_standard_functions(&mut functions);
        
        Self {
            functions,
            game_states: Arc::new(DashMap::new()),
            players: Arc::new(DashMap::new()),
            call_count: AtomicU64::new(0),
            security_context: Arc::new(RwLock::new(SecurityContext::default())),
            resource_limits: ResourceLimits::default(),
        }
    }

    /// Register standard host functions
    fn register_standard_functions(functions: &mut HashMap<String, HostFunction>) {
        // Game state functions
        functions.insert("get_game_state".to_string(), HostFunction {
            name: "get_game_state".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::String], // game_id
                returns: WasmValueType::String, // JSON game state
            },
            handler: Box::new(|ctx, args| {
                Self::host_get_game_state(ctx, args)
            }),
            permission_required: Some("game_read".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 100,
                memory_cost: 1024,
                network_cost: 0,
            },
        });

        functions.insert("update_game_state".to_string(), HostFunction {
            name: "update_game_state".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::String, WasmValueType::String], // game_id, new_state
                returns: WasmValueType::I32, // success boolean
            },
            handler: Box::new(|ctx, args| {
                Self::host_update_game_state(ctx, args)
            }),
            permission_required: Some("game_write".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 500,
                memory_cost: 2048,
                network_cost: 0,
            },
        });

        // PlayerJoinData functions
        functions.insert("get_player_info".to_string(), HostFunction {
            name: "get_player_info".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::String], // peer_id
                returns: WasmValueType::String, // JSON player info
            },
            handler: Box::new(|ctx, args| {
                Self::host_get_player_info(ctx, args)
            }),
            permission_required: Some("player_read".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 50,
                memory_cost: 512,
                network_cost: 0,
            },
        });

        // Utility functions
        functions.insert("log".to_string(), HostFunction {
            name: "log".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::String], // message
                returns: WasmValueType::Void,
            },
            handler: Box::new(|ctx, args| {
                Self::host_log(ctx, args)
            }),
            permission_required: None,
            resource_cost: ResourceCost {
                cpu_cost: 10,
                memory_cost: 0,
                network_cost: 0,
            },
        });

        functions.insert("get_time".to_string(), HostFunction {
            name: "get_time".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![],
                returns: WasmValueType::I64, // timestamp
            },
            handler: Box::new(|ctx, args| {
                Self::host_get_time(ctx, args)
            }),
            permission_required: None,
            resource_cost: ResourceCost {
                cpu_cost: 5,
                memory_cost: 0,
                network_cost: 0,
            },
        });

        // Cryptographic functions
        functions.insert("random_bytes".to_string(), HostFunction {
            name: "random_bytes".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::I32], // length
                returns: WasmValueType::Bytes,
            },
            handler: Box::new(|ctx, args| {
                Self::host_random_bytes(ctx, args)
            }),
            permission_required: Some("crypto".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 100,
                memory_cost: 1024,
                network_cost: 0,
            },
        });

        functions.insert("hash_data".to_string(), HostFunction {
            name: "hash_data".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::Bytes], // data
                returns: WasmValueType::Bytes, // hash
            },
            handler: Box::new(|ctx, args| {
                Self::host_hash_data(ctx, args)
            }),
            permission_required: Some("crypto".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 200,
                memory_cost: 512,
                network_cost: 0,
            },
        });

        // Network functions
        functions.insert("send_message".to_string(), HostFunction {
            name: "send_message".to_string(),
            signature: HostFunctionSignature {
                parameters: vec![WasmValueType::String, WasmValueType::Bytes], // peer_id, message
                returns: WasmValueType::I32, // success boolean
            },
            handler: Box::new(|ctx, args| {
                Self::host_send_message(ctx, args)
            }),
            permission_required: Some("network_send".to_string()),
            resource_cost: ResourceCost {
                cpu_cost: 1000,
                memory_cost: 4096,
                network_cost: 1,
            },
        });
    }

    /// Call a host function
    pub async fn call_function(
        &self,
        context: &HostFunctionContext,
        function_name: &str,
        args: Vec<WasmValue>,
    ) -> Result<WasmValue> {
        // Get function
        let function = self.functions.get(function_name)
            .ok_or_else(|| Error::Wasm(format!("Host function '{}' not found", function_name)))?;

        // Check permissions
        if let Some(ref permission) = function.permission_required {
            if !context.permissions.contains(permission) {
                return Err(Error::Wasm(format!(
                    "Permission '{}' required for function '{}'",
                    permission, function_name
                )));
            }
        }

        // Check resource limits
        self.check_resource_limits(context, &function.resource_cost).await?;

        // Validate arguments
        if args.len() != function.signature.parameters.len() {
            return Err(Error::Wasm(format!(
                "Function '{}' expects {} arguments, got {}",
                function_name,
                function.signature.parameters.len(),
                args.len()
            )));
        }

        // Execute function
        let start_time = Instant::now();
        let result = (function.handler)(context, args);
        let execution_time = start_time.elapsed();

        // Update statistics
        self.call_count.fetch_add(1, Ordering::Relaxed);

        // Log slow functions
        if execution_time > Duration::from_millis(10) {
            log::warn!("Slow host function '{}' took {:?}", function_name, execution_time);
        }

        result
    }

    /// Check resource limits for function execution
    async fn check_resource_limits(
        &self,
        context: &HostFunctionContext,
        cost: &ResourceCost,
    ) -> Result<()> {
        // Check CPU cost
        if cost.cpu_cost > self.resource_limits.max_cpu_per_call {
            return Err(Error::Wasm("Function exceeds CPU limit".to_string()));
        }

        // Check memory cost
        if cost.memory_cost > self.resource_limits.max_memory_per_call {
            return Err(Error::Wasm("Function exceeds memory limit".to_string()));
        }

        // Check network cost
        if cost.network_cost > self.resource_limits.max_network_per_call {
            return Err(Error::Wasm("Function exceeds network limit".to_string()));
        }

        // Check instance-specific quotas
        let security_ctx = self.security_context.read().await;
        if let Some(quota) = security_ctx.resource_quotas.get(&context.caller_id) {
            // In a real implementation, would check rate limits and quotas
            log::trace!("Checking resource quota for instance {}", context.caller_id);
        }

        Ok(())
    }

    /// Register a custom host function
    pub fn register_function(&mut self, function: HostFunction) {
        self.functions.insert(function.name.clone(), function);
    }

    /// Set permissions for a WASM instance
    pub async fn set_permissions(&self, instance_id: Uuid, permissions: Vec<String>) {
        let mut security_ctx = self.security_context.write().await;
        security_ctx.allowed_permissions.insert(instance_id, permissions);
    }

    /// Set resource quota for a WASM instance
    pub async fn set_resource_quota(&self, instance_id: Uuid, quota: ResourceQuota) {
        let mut security_ctx = self.security_context.write().await;
        security_ctx.resource_quotas.insert(instance_id, quota);
    }

    /// Get function call statistics
    pub fn get_call_count(&self) -> u64 {
        self.call_count.load(Ordering::Relaxed)
    }

    /// List available host functions
    pub fn list_functions(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    // Host function implementations

    fn host_get_game_state(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if let Some(WasmValue::String(game_id_str)) = args.first() {
            // In a real implementation, would look up actual game state
            let mock_state = serde_json::json!({
                "game_id": game_id_str,
                "phase": "come_out",
                "point": null,
                "players": [],
                "bets": {}
            });
            
            Ok(WasmValue::String(mock_state.to_string()))
        } else {
            Err(Error::Wasm("Invalid game_id parameter".to_string()))
        }
    }

    fn host_update_game_state(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if args.len() >= 2 {
            if let (Some(WasmValue::String(_game_id)), Some(WasmValue::String(_new_state))) = 
                (args.get(0), args.get(1)) {
                // In a real implementation, would validate and update game state
                Ok(WasmValue::I32(1)) // success
            } else {
                Err(Error::Wasm("Invalid parameters for update_game_state".to_string()))
            }
        } else {
            Err(Error::Wasm("update_game_state requires 2 parameters".to_string()))
        }
    }

    fn host_get_player_info(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if let Some(WasmValue::String(peer_id_str)) = args.first() {
            // Mock player info
            let mock_player = serde_json::json!({
                "peer_id": peer_id_str,
                "balance": 1000,
                "reputation": 100,
                "games_played": 42
            });
            
            Ok(WasmValue::String(mock_player.to_string()))
        } else {
            Err(Error::Wasm("Invalid peer_id parameter".to_string()))
        }
    }

    fn host_log(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if let Some(WasmValue::String(message)) = args.first() {
            log::info!("[WASM] {}", message);
            Ok(WasmValue::Null)
        } else {
            Err(Error::Wasm("Invalid message parameter".to_string()))
        }
    }

    fn host_get_time(_context: &HostFunctionContext, _args: Vec<WasmValue>) -> Result<WasmValue> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        Ok(WasmValue::I64(timestamp))
    }

    fn host_random_bytes(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if let Some(WasmValue::I32(length)) = args.first() {
            if *length < 0 || *length > 1024 {
                return Err(Error::Wasm("Invalid length for random_bytes".to_string()));
            }
            
            use rand::{rngs::OsRng, RngCore};
            let mut bytes = vec![0u8; *length as usize];
            OsRng.fill_bytes(&mut bytes);
            
            Ok(WasmValue::Bytes(bytes))
        } else {
            Err(Error::Wasm("Invalid length parameter".to_string()))
        }
    }

    fn host_hash_data(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if let Some(WasmValue::Bytes(data)) = args.first() {
            let hash = blake3::hash(data);
            Ok(WasmValue::Bytes(hash.as_bytes().to_vec()))
        } else {
            Err(Error::Wasm("Invalid data parameter".to_string()))
        }
    }

    fn host_send_message(_context: &HostFunctionContext, args: Vec<WasmValue>) -> Result<WasmValue> {
        if args.len() >= 2 {
            if let (Some(WasmValue::String(_peer_id)), Some(WasmValue::Bytes(_message))) = 
                (args.get(0), args.get(1)) {
                // In a real implementation, would send message through transport layer
                log::debug!("WASM module sending message to peer");
                Ok(WasmValue::I32(1)) // success
            } else {
                Err(Error::Wasm("Invalid parameters for send_message".to_string()))
            }
        } else {
            Err(Error::Wasm("send_message requires 2 parameters".to_string()))
        }
    }

    /// Update game state cache (called by runtime)
    pub fn update_game_state_cache(&self, game_id: Uuid, state: GameStateSnapshot) {
        self.game_states.insert(game_id, state);
    }

    /// Update player cache (called by runtime)
    pub fn update_player_cache(&self, peer_id: PeerId, player: PlayerJoinData) {
        self.players.insert(peer_id, player);
    }

    /// Clear caches
    pub fn clear_caches(&self) {
        self.game_states.clear();
        self.players.clear();
    }
}

impl Default for WasmHostFunctions {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating host function contexts
pub struct HostFunctionContextBuilder {
    caller_id: Option<Uuid>,
    caller_peer: Option<PeerId>,
    game_id: Option<Uuid>,
    permissions: Vec<String>,
}

impl HostFunctionContextBuilder {
    pub fn new() -> Self {
        Self {
            caller_id: None,
            caller_peer: None,
            game_id: None,
            permissions: Vec::new(),
        }
    }

    pub fn caller_id(mut self, id: Uuid) -> Self {
        self.caller_id = Some(id);
        self
    }

    pub fn caller_peer(mut self, peer: PeerId) -> Self {
        self.caller_peer = Some(peer);
        self
    }

    pub fn game_id(mut self, id: Uuid) -> Self {
        self.game_id = Some(id);
        self
    }

    pub fn permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn build(self) -> Result<HostFunctionContext> {
        Ok(HostFunctionContext {
            caller_id: self.caller_id.ok_or_else(|| Error::Wasm("caller_id required".to_string()))?,
            caller_peer: self.caller_peer.ok_or_else(|| Error::Wasm("caller_peer required".to_string()))?,
            game_id: self.game_id,
            timestamp: Instant::now(),
            permissions: self.permissions,
        })
    }
}

impl Default for HostFunctionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_host_functions_creation() {
        let host_functions = WasmHostFunctions::new();
        
        assert!(host_functions.functions.contains_key("get_game_state"));
        assert!(host_functions.functions.contains_key("update_game_state"));
        assert!(host_functions.functions.contains_key("log"));
        assert!(host_functions.functions.contains_key("get_time"));
    }

    #[tokio::test]
    async fn test_host_function_call() {
        let host_functions = WasmHostFunctions::new();
        let context = HostFunctionContextBuilder::new()
            .caller_id(Uuid::new_v4())
            .caller_peer(PeerId::new([1u8; 32]))
            .permissions(vec!["game_read".to_string()])
            .build()
            .unwrap();

        let result = host_functions.call_function(
            &context,
            "get_game_state",
            vec![WasmValue::String("test_game".to_string())],
        ).await;

        assert!(result.is_ok());
        match result.unwrap() {
            WasmValue::String(json) => assert!(json.contains("test_game")),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let host_functions = WasmHostFunctions::new();
        let context = HostFunctionContextBuilder::new()
            .caller_id(Uuid::new_v4())
            .caller_peer(PeerId::new([1u8; 32]))
            .permissions(vec![]) // No permissions
            .build()
            .unwrap();

        let result = host_functions.call_function(
            &context,
            "get_game_state",
            vec![WasmValue::String("test_game".to_string())],
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Permission"));
    }

    #[tokio::test]
    async fn test_resource_limits() {
        let mut host_functions = WasmHostFunctions::new();
        host_functions.resource_limits.max_cpu_per_call = 50; // Very low limit

        let context = HostFunctionContextBuilder::new()
            .caller_id(Uuid::new_v4())
            .caller_peer(PeerId::new([1u8; 32]))
            .permissions(vec!["game_write".to_string()])
            .build()
            .unwrap();

        let result = host_functions.call_function(
            &context,
            "update_game_state",
            vec![
                WasmValue::String("test_game".to_string()),
                WasmValue::String("{}".to_string()),
            ],
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CPU limit"));
    }

    #[tokio::test]
    async fn test_utility_functions() {
        let host_functions = WasmHostFunctions::new();
        let context = HostFunctionContextBuilder::new()
            .caller_id(Uuid::new_v4())
            .caller_peer(PeerId::new([1u8; 32]))
            .build()
            .unwrap();

        // Test log function (no permissions required)
        let result = host_functions.call_function(
            &context,
            "log",
            vec![WasmValue::String("test message".to_string())],
        ).await;
        assert!(result.is_ok());

        // Test get_time function
        let result = host_functions.call_function(&context, "get_time", vec![]).await;
        assert!(result.is_ok());
        match result.unwrap() {
            WasmValue::I64(timestamp) => assert!(timestamp > 0),
            _ => panic!("Expected I64 timestamp"),
        }
    }

    #[tokio::test]
    async fn test_crypto_functions() {
        let host_functions = WasmHostFunctions::new();
        let context = HostFunctionContextBuilder::new()
            .caller_id(Uuid::new_v4())
            .caller_peer(PeerId::new([1u8; 32]))
            .permissions(vec!["crypto".to_string()])
            .build()
            .unwrap();

        // Test random_bytes
        let result = host_functions.call_function(
            &context,
            "random_bytes",
            vec![WasmValue::I32(32)],
        ).await;
        assert!(result.is_ok());
        match result.unwrap() {
            WasmValue::Bytes(bytes) => assert_eq!(bytes.len(), 32),
            _ => panic!("Expected bytes result"),
        }

        // Test hash_data
        let test_data = b"hello world";
        let result = host_functions.call_function(
            &context,
            "hash_data",
            vec![WasmValue::Bytes(test_data.to_vec())],
        ).await;
        assert!(result.is_ok());
        match result.unwrap() {
            WasmValue::Bytes(hash) => assert_eq!(hash.len(), 32), // blake3 hash size
            _ => panic!("Expected bytes result"),
        }
    }
}