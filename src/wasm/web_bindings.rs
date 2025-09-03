//! Web bindings for BitCraps WASM integration
//!
//! This module provides JavaScript/TypeScript bindings for browser integration,
//! allowing BitCraps to run in web browsers using WebAssembly.

use crate::error::{Error, Result};
use crate::gaming::{CrapsGame, GameAction, GameState};
use crate::protocol::PeerId;
use crate::wasm::{WasmRuntime, WasmValue, WasmConfig};
use js_sys::{Array, Object, Promise, Uint8Array};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys::{console, window, Location, Navigator};

/// JavaScript interface for BitCraps WASM runtime
#[wasm_bindgen]
pub struct BitCrapsWasm {
    runtime: Arc<WasmRuntime>,
    peer_id: PeerId,
    games: Arc<RwLock<HashMap<String, CrapsGame>>>,
}

/// JavaScript-compatible game state
#[wasm_bindgen]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsGameState {
    game_id: String,
    phase: String,
    point: Option<u8>,
    players: Array,
    bets: Object,
    dice_result: Option<Array>,
}

/// JavaScript-compatible game action
#[wasm_bindgen]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsGameAction {
    action_type: String,
    player_id: String,
    amount: Option<f64>,
    bet_type: Option<String>,
    data: Option<Object>,
}

/// JavaScript-compatible peer information
#[wasm_bindgen]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsPeerInfo {
    peer_id: String,
    connected: bool,
    transport_type: String,
    connection_time: Option<f64>,
    last_activity: Option<f64>,
}

/// Configuration for browser environment
#[wasm_bindgen]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserConfig {
    enable_webrtc: bool,
    signaling_server: Option<String>,
    stun_servers: Array,
    max_peers: u32,
    auto_connect: bool,
    debug_mode: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enable_webrtc: true,
            signaling_server: Some("wss://signal.bitcraps.io".to_string()),
            stun_servers: Array::new(),
            max_peers: 8,
            auto_connect: true,
            debug_mode: false,
        }
    }
}

#[wasm_bindgen]
impl BitCrapsWasm {
    /// Create a new BitCraps WASM instance
    #[wasm_bindgen(constructor)]
    pub fn new(config: Option<BrowserConfig>) -> Result<BitCrapsWasm, JsValue> {
        // Set panic hook for better error reporting
        console_error_panic_hook::set_once();
        
        // Enable logging
        wasm_logger::init(wasm_logger::Config::default());

        let config = config.unwrap_or_default();
        
        // Create WASM runtime configuration
        let wasm_config = WasmConfig {
            max_memory: 16 * 1024 * 1024, // 16MB for browser
            max_execution_time: std::time::Duration::from_secs(2),
            debug_mode: config.debug_mode,
            allow_host_functions: true,
            plugin_directory: std::path::PathBuf::from("plugins"), // Virtual in browser
            fuel_limit: 100_000, // Conservative for browser
            enable_cache: true,
            ..Default::default()
        };

        let runtime = Arc::new(WasmRuntime::new(wasm_config));
        
        // Generate a peer ID for this browser instance
        let peer_id = Self::generate_browser_peer_id()?;

        log::info!("Initialized BitCraps WASM runtime for peer {:?}", peer_id);

        Ok(BitCrapsWasm {
            runtime,
            peer_id,
            games: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Initialize the runtime and start services
    #[wasm_bindgen]
    pub fn initialize(&self) -> Promise {
        let runtime = self.runtime.clone();
        
        future_to_promise(async move {
            runtime.start().await
                .map_err(|e| JsValue::from_str(&format!("Initialization failed: {}", e)))?;
            
            log::info!("BitCraps WASM runtime initialized successfully");
            Ok(JsValue::from_str("initialized"))
        })
    }

    /// Get the peer ID for this browser instance
    #[wasm_bindgen(getter)]
    pub fn peer_id(&self) -> String {
        format!("{:?}", self.peer_id)
    }

    /// Create a new craps game
    #[wasm_bindgen]
    pub fn create_game(&self, game_id: String) -> Promise {
        let games = self.games.clone();
        let peer_id = self.peer_id;
        
        future_to_promise(async move {
            let mut games = games.write().await;
            
            if games.contains_key(&game_id) {
                return Err(JsValue::from_str("Game already exists"));
            }

            let game = CrapsGame::new(game_id.clone(), peer_id);
            games.insert(game_id.clone(), game);
            
            log::info!("Created game: {}", game_id);
            Ok(JsValue::from_str(&game_id))
        })
    }

    /// Join an existing game
    #[wasm_bindgen]
    pub fn join_game(&self, game_id: String) -> Promise {
        let games = self.games.clone();
        let peer_id = self.peer_id;

        future_to_promise(async move {
            let mut games = games.write().await;
            
            let game = games.get_mut(&game_id)
                .ok_or_else(|| JsValue::from_str("Game not found"))?;

            // In a real implementation, would join through network protocol
            log::info!("Player {:?} joined game: {}", peer_id, game_id);
            Ok(JsValue::from_str("joined"))
        })
    }

    /// Execute a game action
    #[wasm_bindgen]
    pub fn execute_action(&self, game_id: String, action: JsGameAction) -> Promise {
        let games = self.games.clone();
        let runtime = self.runtime.clone();

        future_to_promise(async move {
            let games = games.read().await;
            let game = games.get(&game_id)
                .ok_or_else(|| JsValue::from_str("Game not found"))?;

            // Convert JS action to Rust action
            let rust_action = Self::js_action_to_rust(&action)?;
            
            // Execute action through WASM plugin if available
            let result = runtime.execute_game_plugin(
                "craps_game",
                rust_action,
                &game.get_state(),
                action.player_id.parse::<PeerId>()
                    .map_err(|_| JsValue::from_str("Invalid player ID"))?
            ).await;

            match result {
                Ok(new_state) => {
                    let js_state = Self::rust_state_to_js(&new_state)?;
                    Ok(js_state.into())
                }
                Err(e) => Err(JsValue::from_str(&format!("Action failed: {}", e)))
            }
        })
    }

    /// Get current game state
    #[wasm_bindgen]
    pub fn get_game_state(&self, game_id: String) -> Promise {
        let games = self.games.clone();

        future_to_promise(async move {
            let games = games.read().await;
            let game = games.get(&game_id)
                .ok_or_else(|| JsValue::from_str("Game not found"))?;

            let js_state = Self::rust_state_to_js(&game.get_state())?;
            Ok(js_state.into())
        })
    }

    /// Get list of available games
    #[wasm_bindgen]
    pub fn list_games(&self) -> Promise {
        let games = self.games.clone();

        future_to_promise(async move {
            let games = games.read().await;
            let game_ids = games.keys().cloned().collect::<Vec<_>>();
            
            let js_array = Array::new();
            for game_id in game_ids {
                js_array.push(&JsValue::from_str(&game_id));
            }
            
            Ok(js_array.into())
        })
    }

    /// Connect to peers via WebRTC
    #[wasm_bindgen]
    pub fn connect_peers(&self, signaling_server: Option<String>) -> Promise {
        let _server = signaling_server.unwrap_or_else(|| "wss://signal.bitcraps.io".to_string());

        future_to_promise(async move {
            // In a real implementation, would:
            // 1. Connect to signaling server
            // 2. Exchange offers/answers with peers
            // 3. Establish WebRTC data channels
            // 4. Start game synchronization
            
            log::info!("WebRTC peer connection initiated");
            Ok(JsValue::from_str("connected"))
        })
    }

    /// Get connected peers
    #[wasm_bindgen]
    pub fn get_peers(&self) -> Promise {
        future_to_promise(async move {
            // Mock peer data
            let peers = Array::new();
            
            let peer_info = Object::new();
            js_sys::Reflect::set(&peer_info, &"peer_id".into(), &"peer_123".into()).unwrap();
            js_sys::Reflect::set(&peer_info, &"connected".into(), &true.into()).unwrap();
            js_sys::Reflect::set(&peer_info, &"transport_type".into(), &"webrtc".into()).unwrap();
            
            peers.push(&peer_info);
            Ok(peers.into())
        })
    }

    /// Send message to peer
    #[wasm_bindgen]
    pub fn send_message(&self, peer_id: String, message: Uint8Array) -> Promise {
        future_to_promise(async move {
            let _data = message.to_vec();
            
            // In a real implementation, would send through WebRTC data channel
            log::info!("Sending message to peer: {}", peer_id);
            Ok(JsValue::from_bool(true))
        })
    }

    /// Load a WASM plugin
    #[wasm_bindgen]
    pub fn load_plugin(&self, name: String, wasm_bytes: Uint8Array) -> Promise {
        let runtime = self.runtime.clone();

        future_to_promise(async move {
            let bytes = bytes::Bytes::from(wasm_bytes.to_vec());
            
            match runtime.load_module(name.clone(), bytes).await {
                Ok(_) => {
                    log::info!("Loaded plugin: {}", name);
                    Ok(JsValue::from_str(&name))
                }
                Err(e) => Err(JsValue::from_str(&format!("Plugin load failed: {}", e)))
            }
        })
    }

    /// Get runtime statistics
    #[wasm_bindgen]
    pub fn get_stats(&self) -> Promise {
        let runtime = self.runtime.clone();

        future_to_promise(async move {
            let stats = runtime.get_stats().await;
            
            let js_stats = Object::new();
            js_sys::Reflect::set(&js_stats, &"modules_loaded".into(), &(stats.modules_loaded as f64).into()).unwrap();
            js_sys::Reflect::set(&js_stats, &"active_instances".into(), &(stats.active_instances as f64).into()).unwrap();
            js_sys::Reflect::set(&js_stats, &"total_executions".into(), &(stats.total_executions as f64).into()).unwrap();
            js_sys::Reflect::set(&js_stats, &"memory_usage".into(), &(stats.memory_usage as f64).into()).unwrap();
            
            Ok(js_stats.into())
        })
    }

    /// Enable debug logging
    #[wasm_bindgen]
    pub fn enable_debug(&self) {
        console::log_1(&"Debug mode enabled".into());
        log::set_max_level(log::LevelFilter::Debug);
    }

    /// Get browser information
    #[wasm_bindgen]
    pub fn get_browser_info(&self) -> Object {
        let info = Object::new();
        
        if let Some(window) = window() {
            if let Some(navigator) = window.navigator() {
                js_sys::Reflect::set(&info, &"user_agent".into(), &navigator.user_agent().unwrap_or_default().into()).unwrap();
                js_sys::Reflect::set(&info, &"platform".into(), &navigator.platform().unwrap_or_default().into()).unwrap();
                js_sys::Reflect::set(&info, &"language".into(), &navigator.language().unwrap_or_default().into()).unwrap();
            }

            if let Ok(location) = window.location() {
                js_sys::Reflect::set(&info, &"origin".into(), &location.origin().unwrap_or_default().into()).unwrap();
                js_sys::Reflect::set(&info, &"protocol".into(), &location.protocol().unwrap_or_default().into()).unwrap();
            }
        }

        // Add WASM-specific info
        js_sys::Reflect::set(&info, &"wasm_supported".into(), &true.into()).unwrap();
        js_sys::Reflect::set(&info, &"webrtc_supported".into(), &Self::check_webrtc_support().into()).unwrap();
        
        info
    }

    // Helper methods

    /// Generate a browser-specific peer ID
    fn generate_browser_peer_id() -> Result<PeerId, JsValue> {
        use rand::{rngs::OsRng, RngCore};
        
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        
        Ok(PeerId::new(bytes))
    }

    /// Convert JavaScript action to Rust action
    fn js_action_to_rust(js_action: &JsGameAction) -> Result<GameAction, JsValue> {
        // This is a simplified conversion
        match js_action.action_type.as_str() {
            "place_bet" => {
                let amount = js_action.amount.ok_or_else(|| JsValue::from_str("Amount required for bet"))?;
                let bet_type = js_action.bet_type.as_ref().ok_or_else(|| JsValue::from_str("Bet type required"))?;
                
                // Create a GameAction based on bet type
                Ok(GameAction::PlaceBet {
                    player: js_action.player_id.parse::<PeerId>()
                        .map_err(|_| JsValue::from_str("Invalid player ID"))?,
                    amount,
                    bet_type: bet_type.clone(),
                })
            }
            "roll_dice" => {
                Ok(GameAction::RollDice {
                    player: js_action.player_id.parse::<PeerId>()
                        .map_err(|_| JsValue::from_str("Invalid player ID"))?,
                })
            }
            _ => Err(JsValue::from_str("Unknown action type"))
        }
    }

    /// Convert Rust game state to JavaScript state
    fn rust_state_to_js(rust_state: &GameState) -> Result<JsGameState, JsValue> {
        let players = Array::new();
        // In a real implementation, would convert actual player data
        
        let bets = Object::new();
        // In a real implementation, would convert actual bet data

        Ok(JsGameState {
            game_id: "unknown".to_string(), // Would extract from actual state
            phase: format!("{:?}", rust_state.phase),
            point: rust_state.point,
            players,
            bets,
            dice_result: None, // Would extract from actual state
        })
    }

    /// Check if WebRTC is supported in this browser
    fn check_webrtc_support() -> bool {
        // Check for RTCPeerConnection availability
        if let Some(window) = window() {
            js_sys::Reflect::has(&window, &"RTCPeerConnection".into()).unwrap_or(false)
        } else {
            false
        }
    }
}

/// JavaScript bindings for WASM values
#[wasm_bindgen]
impl JsGameState {
    #[wasm_bindgen(constructor)]
    pub fn new(game_id: String, phase: String) -> JsGameState {
        JsGameState {
            game_id,
            phase,
            point: None,
            players: Array::new(),
            bets: Object::new(),
            dice_result: None,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn game_id(&self) -> String {
        self.game_id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn phase(&self) -> String {
        self.phase.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn point(&self) -> Option<u8> {
        self.point
    }

    #[wasm_bindgen(getter)]
    pub fn players(&self) -> Array {
        self.players.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn bets(&self) -> Object {
        self.bets.clone()
    }
}

#[wasm_bindgen]
impl JsGameAction {
    #[wasm_bindgen(constructor)]
    pub fn new(action_type: String, player_id: String) -> JsGameAction {
        JsGameAction {
            action_type,
            player_id,
            amount: None,
            bet_type: None,
            data: None,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_amount(&mut self, amount: f64) {
        self.amount = Some(amount);
    }

    #[wasm_bindgen(setter)]
    pub fn set_bet_type(&mut self, bet_type: String) {
        self.bet_type = Some(bet_type);
    }

    #[wasm_bindgen(getter)]
    pub fn action_type(&self) -> String {
        self.action_type.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn player_id(&self) -> String {
        self.player_id.clone()
    }
}

/// Global initialization function for the WASM module
#[wasm_bindgen(start)]
pub fn initialize_bitcraps_wasm() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    
    console::log_1(&"BitCraps WASM module loaded successfully".into());
}

/// JavaScript utility functions
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_many(a: &str, b: &str);
}

// Macro for easier console logging
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;

/// Export the WASM memory for JavaScript access
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}

/// Get WASM module version
#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_wasm_initialization() {
        let config = BrowserConfig::default();
        let bitcraps = BitCrapsWasm::new(Some(config));
        assert!(bitcraps.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_peer_id_generation() {
        let peer_id = BitCrapsWasm::generate_browser_peer_id();
        assert!(peer_id.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_js_game_state_creation() {
        let state = JsGameState::new("test_game".to_string(), "come_out".to_string());
        assert_eq!(state.game_id(), "test_game");
        assert_eq!(state.phase(), "come_out");
    }

    #[wasm_bindgen_test]
    fn test_js_game_action_creation() {
        let mut action = JsGameAction::new("place_bet".to_string(), "player_123".to_string());
        action.set_amount(100.0);
        action.set_bet_type("pass_line".to_string());
        
        assert_eq!(action.action_type(), "place_bet");
        assert_eq!(action.player_id(), "player_123");
        assert_eq!(action.amount, Some(100.0));
    }

    #[wasm_bindgen_test]
    fn test_webrtc_support_check() {
        let supported = BitCrapsWasm::check_webrtc_support();
        // In a real browser environment, this would check actual WebRTC support
        // For now, just ensure the function runs without panic
        let _ = supported;
    }

    #[wasm_bindgen_test]
    fn test_version_export() {
        let version = get_version();
        assert!(!version.is_empty());
    }
}