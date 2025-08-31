//! Mobile platform bindings and UniFFI interface implementation
//!
//! This module provides the cross-platform interface for mobile applications
//! using UniFFI to generate bindings for Android (Kotlin) and iOS (Swift).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

// Android-specific modules
#[cfg(target_os = "android")]
pub mod android;

// iOS-specific modules
#[cfg(target_os = "ios")]
pub mod ios;

// Performance optimization modules
pub mod battery_thermal;
pub mod ble_optimizer;
pub mod compression;
pub mod cpu_optimizer;
pub mod memory_manager;
pub mod network_optimizer;
pub mod performance;
pub mod power_manager;

// Legacy mobile platform modules
mod android_keystore;
pub mod battery_optimization;
mod biometric_auth;
mod ffi;
mod ios_keychain;
mod jni_bindings;
mod key_derivation;
mod permissions;
mod platform_adaptations;
mod platform_config;
pub mod power_management;
mod secure_storage;
mod security_integration;
mod uniffi_impl;

// Re-export all types from modules
pub use battery_optimization::*;
pub use ffi::*;
pub use jni_bindings::*;
pub use platform_adaptations::*;
pub use platform_config::*;
pub use power_management::*;
pub use secure_storage::*;
// Don't re-export from android_keystore and ios_keychain to avoid SecurityLevel conflict
// pub use android_keystore::*;
// pub use ios_keychain::*;
pub use biometric_auth::*;
pub use key_derivation::*;
pub use permissions::*;
pub use security_integration::*;

// Export specific types from android/ios modules
pub use android_keystore::{AndroidKeystore, SecurityLevel};
pub use ios_keychain::IOSKeychain;

// UniFFI scaffolding for mobile bindings
#[cfg(feature = "uniffi")]
uniffi::include_scaffolding!("bitcraps");

/// Main BitCraps node for mobile platforms
pub struct BitCrapsNode {
    inner: Arc<crate::mesh::MeshService>,
    event_queue: Arc<Mutex<VecDeque<GameEvent>>>,
    event_sender: mpsc::Sender<GameEvent>,
    power_manager: Arc<PowerManager>,
    config: BitCrapsConfig,
    status: Arc<Mutex<NodeStatus>>,
    current_game: Arc<Mutex<Option<Arc<GameHandle>>>>,
}

/// Game handle for managing individual games
pub struct GameHandle {
    game_id: String,
    node: Arc<BitCrapsNode>,
    state: Arc<Mutex<GameState>>,
    history: Arc<Mutex<Vec<GameEvent>>>,
    last_roll: Arc<Mutex<Option<DiceRoll>>>,
}

/// Configuration for BitCraps node initialization
#[derive(Clone)]
pub struct BitCrapsConfig {
    pub data_dir: String,
    pub pow_difficulty: u32,
    pub protocol_version: u16,
    pub power_mode: PowerMode,
    pub platform_config: Option<PlatformConfig>,
    pub enable_logging: bool,
    pub log_level: LogLevel,
}

/// Configuration for individual games
#[derive(Clone)]
pub struct GameConfig {
    pub game_name: Option<String>,
    pub min_bet: u64,
    pub max_bet: u64,
    pub max_players: u32,
    pub timeout_seconds: u32,
}

/// Platform-specific configuration
#[derive(Clone)]
pub struct PlatformConfig {
    pub platform: PlatformType,
    pub background_scanning: bool,
    pub scan_window_ms: u32,
    pub scan_interval_ms: u32,
    pub low_power_mode: bool,
    pub service_uuids: Vec<String>,
}

/// Information about a discovered peer
#[derive(Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub display_name: Option<String>,
    pub signal_strength: u32,
    pub last_seen: u64, // timestamp
    pub is_connected: bool,
}

/// Network statistics for monitoring
#[derive(Clone)]
pub struct NetworkStats {
    pub peers_discovered: u32,
    pub active_connections: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_dropped: u32,
    pub average_latency_ms: f64,
}

/// Current node status information
#[derive(Clone)]
pub struct NodeStatus {
    pub state: NodeState,
    pub bluetooth_enabled: bool,
    pub discovery_active: bool,
    pub current_game_id: Option<String>,
    pub active_connections: u32,
    pub current_power_mode: PowerMode,
}

/// Dice roll result
#[derive(Clone)]
pub struct DiceRoll {
    pub die1: u8,
    pub die2: u8,
    pub roll_time: u64, // timestamp
    pub roller_peer_id: String,
}

/// Game events for mobile UI updates
#[derive(Clone)]
pub enum GameEvent {
    PeerDiscovered {
        peer: PeerInfo,
    },
    PeerConnected {
        peer_id: String,
    },
    PeerDisconnected {
        peer_id: String,
    },
    GameCreated {
        game_id: String,
    },
    GameJoined {
        game_id: String,
        peer_id: String,
    },
    GameLeft {
        game_id: String,
        peer_id: String,
    },
    GameStarted {
        game_id: String,
    },
    BetPlaced {
        peer_id: String,
        bet_type: BetType,
        amount: u64,
    },
    DiceRolled {
        roll: DiceRoll,
    },
    GameEnded {
        game_id: String,
        winner_id: Option<String>,
        payout: u64,
    },
    ErrorOccurred {
        error: BitCrapsError,
    },
    BatteryOptimizationDetected {
        reason: String,
    },
    NetworkStateChanged {
        new_state: NetworkState,
    },
}

/// Different types of bets in craps
#[derive(Debug, Clone, Copy)]
pub enum BetType {
    Pass,
    DontPass,
    Field,
    Any7,
    AnyCraps,
    Hardway { number: u8 },
    PlaceBet { number: u8 },
}

/// Current state of a game
#[derive(Clone)]
pub enum GameState {
    Waiting,
    ComeOut,
    Point { point: u8 },
    Resolved,
    Error { reason: String },
}

/// Node operational states
#[derive(Debug, Clone)]
pub enum NodeState {
    Initializing,
    Ready,
    Discovering,
    Connected,
    InGame,
    Error { reason: String },
}

/// Power management modes for battery optimization
#[derive(Debug, Clone, Copy)]
pub enum PowerMode {
    HighPerformance,
    Balanced,
    BatterySaver,
    UltraLowPower,
}

// PlatformType is defined in platform_adaptations module

/// Network connection states
#[derive(Clone)]
pub enum NetworkState {
    Offline,
    Scanning,
    Connected,
    Optimized,
}

/// Logging levels
#[derive(Clone)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Error types for mobile interface
#[derive(thiserror::Error, Debug, Clone)]
pub enum BitCrapsError {
    #[error("Initialization failed: {reason}")]
    InitializationError { reason: String },
    #[error("Bluetooth error: {reason}")]
    BluetoothError { reason: String },
    #[error("Network error: {reason}")]
    NetworkError { reason: String },
    #[error("Game error: {reason}")]
    GameError { reason: String },
    #[error("Crypto error: {reason}")]
    CryptoError { reason: String },
    #[error("Invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("Operation timed out")]
    Timeout,
    #[error("Item not found: {item}")]
    NotFound { item: String },
    #[error("Game logic error: {reason}")]
    GameLogic { reason: String },
    #[error("Consensus error: {reason}")]
    ConsensusError { reason: String },
}

impl Default for BitCrapsConfig {
    fn default() -> Self {
        Self {
            data_dir: String::from("./data"),
            pow_difficulty: 4,
            protocol_version: 1,
            power_mode: PowerMode::Balanced,
            platform_config: None,
            enable_logging: true,
            log_level: LogLevel::Info,
        }
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            game_name: None,
            min_bet: 1,
            max_bet: 1000,
            max_players: 8,
            timeout_seconds: 300,
        }
    }
}

/// Get current timestamp in seconds since Unix epoch
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Create a new BitCraps node with the given configuration
pub fn create_node(config: BitCrapsConfig) -> Result<Arc<BitCrapsNode>, BitCrapsError> {
    // Initialize logging if enabled
    if config.enable_logging {
        match config.log_level {
            LogLevel::Error => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"))
                    .init()
            }
            LogLevel::Warn => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
                    .init()
            }
            LogLevel::Info => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                    .init()
            }
            LogLevel::Debug => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
                    .init()
            }
            LogLevel::Trace => {
                env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
                    .init()
            }
        }
    }

    // Create event channel with bounded size to prevent memory exhaustion
    let (event_sender, mut event_receiver) = mpsc::channel(1000); // Moderate traffic for mobile events
    let event_queue = Arc::new(Mutex::new(VecDeque::new()));

    // Clone for the receiver task
    let event_queue_clone = Arc::clone(&event_queue);
    tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            if let Ok(mut queue) = event_queue_clone.lock() {
                queue.push_back(event);
                // Limit queue size to prevent memory issues
                if queue.len() > 1000 {
                    queue.pop_front();
                }
            }
        }
    });

    // Initialize power manager
    let power_manager = Arc::new(PowerManager::new(config.power_mode));

    // Create initial node status
    let status = Arc::new(Mutex::new(NodeStatus {
        state: NodeState::Initializing,
        bluetooth_enabled: false,
        discovery_active: false,
        current_game_id: None,
        active_connections: 0,
        current_power_mode: config.power_mode,
    }));

    // TODO: Initialize actual mesh service
    // For now, create a placeholder with dummy identity and transport
    let identity = Arc::new(crate::crypto::BitchatIdentity::generate_with_pow(8));
    let transport = Arc::new(crate::transport::TransportCoordinator::new());
    let mesh_service = Arc::new(crate::mesh::MeshService::new(identity, transport));

    let node = Arc::new(BitCrapsNode {
        inner: mesh_service,
        event_queue,
        event_sender,
        power_manager,
        config,
        status,
        current_game: Arc::new(Mutex::new(None)),
    });

    // Update status to ready
    if let Ok(mut status) = node.status.lock() {
        status.state = NodeState::Ready;
    }

    Ok(node)
}

/// Get list of available Bluetooth adapters
pub fn get_available_bluetooth_adapters() -> Result<Vec<String>, BitCrapsError> {
    // TODO: Implement actual adapter discovery
    Ok(vec!["default".to_string()])
}
