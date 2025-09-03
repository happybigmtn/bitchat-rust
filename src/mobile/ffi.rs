//! Foreign Function Interface for mobile platforms
//!
//! This module provides the UniFFI bindings for Android and iOS

use crate::crypto::BitchatIdentity;
use crate::error::Error;
use crate::gaming::multi_game_framework::{
    GameSession, GameSessionConfig, GameSessionState, GameSessionStats,
};
use crate::mesh::MeshService;
use crate::protocol::PeerId;
use crate::transport::TransportCoordinator;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

// Type alias for FFI results
type Result<T> = std::result::Result<T, BitCrapsError>;

// UniFFI scaffolding is included in mobile/mod.rs

/// Mobile-friendly wrapper around BitCraps functionality
pub struct BitCrapsNode {
    peer_id: PeerId,
    mesh_service: Arc<MeshService>,
    active_games: Arc<Mutex<HashMap<String, Arc<GameSession>>>>,
    config: BitCrapsConfig,
}

/// Configuration for BitCraps node
#[derive(Debug, Clone)]
pub struct BitCrapsConfig {
    pub bluetooth_name: String,
    pub enable_battery_optimization: bool,
    pub max_peers: u32,
    pub discovery_timeout_seconds: u32,
}

impl Default for BitCrapsConfig {
    fn default() -> Self {
        Self {
            bluetooth_name: format!("BitCraps-{}", &Uuid::new_v4().to_string()[..8]),
            enable_battery_optimization: true,
            max_peers: 10,
            discovery_timeout_seconds: 30,
        }
    }
}

/// Create a new BitCraps node with the given configuration
pub fn create_node(config: BitCrapsConfig) -> Result<Arc<BitCrapsNode>> {
    // Generate peer ID and identity
    let uuid_bytes = Uuid::new_v4().as_bytes().clone();
    let mut peer_id_bytes = [0u8; 32];
    peer_id_bytes[..16].copy_from_slice(&uuid_bytes);
    peer_id_bytes[16..].copy_from_slice(&uuid_bytes); // Duplicate for 32 bytes
    let peer_id = PeerId::from(peer_id_bytes);

    // Create identity for the node (with minimal proof-of-work for mobile)
    // Lower difficulty for mobile devices to conserve battery
    let pow_difficulty = if config.enable_battery_optimization {
        8
    } else {
        10
    };
    let identity = Arc::new(BitchatIdentity::generate_with_pow(pow_difficulty));

    // Create transport coordinator with mobile optimizations
    let mut transport = TransportCoordinator::new();

    // Configure Bluetooth transport for mobile
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // Add Bluetooth transport for mobile platforms
        if let Ok(bt_transport) = BluetoothTransport::new(&config.bluetooth_name) {
            transport.add_transport(Box::new(bt_transport));
        }
    }

    // Set mobile-specific transport parameters
    transport.set_max_connections(config.max_peers);
    transport.set_discovery_interval(Duration::from_secs(config.discovery_timeout_seconds as u64));

    let transport = Arc::new(transport);

    // Initialize mesh service with mobile configuration
    let mesh_service = Arc::new(MeshService::new(identity.clone(), transport.clone()));

    // Configure mesh service for mobile
    if config.enable_battery_optimization {
        // Reduce heartbeat frequency and increase timeouts for battery saving
        mesh_service.set_heartbeat_interval(Duration::from_secs(60));
        mesh_service.set_peer_timeout(Duration::from_secs(180));
    }

    let node = Arc::new(BitCrapsNode {
        peer_id,
        mesh_service,
        active_games: Arc::new(Mutex::new(HashMap::new())),
        config,
    });

    Ok(node)
}

/// Get available Bluetooth adapters on the device
pub fn get_available_bluetooth_adapters() -> Vec<String> {
    // Platform-specific implementation
    #[cfg(target_os = "android")]
    {
        // Use JNI to query Android Bluetooth adapters
        vec!["Default Bluetooth Adapter".to_string()]
    }

    #[cfg(target_os = "ios")]
    {
        // iOS only has one Bluetooth adapter
        vec!["iOS Bluetooth".to_string()]
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // Desktop/testing fallback
        vec!["Test Adapter".to_string()]
    }
}

/// Error types for mobile interface
#[cfg_attr(feature = "uniffi", derive(uniffi::Error))]
#[derive(Debug, thiserror::Error)]
pub enum BitCrapsError {
    #[error("Initialization error: {reason}")]
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

    #[error("Operation timeout")]
    Timeout,

    #[error("Item not found: {item}")]
    NotFound { item: String },
}

impl From<Error> for BitCrapsError {
    fn from(err: Error) -> Self {
        match err {
            Error::Network(msg) => BitCrapsError::NetworkError { reason: msg },
            Error::InvalidData(msg) => BitCrapsError::InvalidInput { reason: msg },
            Error::NotFound(item) => BitCrapsError::NotFound { item },
            Error::GameError(msg) => BitCrapsError::GameError { reason: msg },
            Error::Crypto(msg) => BitCrapsError::CryptoError { reason: msg },
            Error::Transport(msg) => BitCrapsError::BluetoothError { reason: msg }, // Transport errors as Bluetooth
            _ => BitCrapsError::InitializationError {
                reason: format!("Internal error: {}", err),
            },
        }
    }
}

/// Game handle for active games
pub struct GameHandle {
    game_id: String,
    session: Arc<GameSession>,
}

/// Game events for UI updates
#[derive(Debug, Clone)]
pub enum GameEvent {
    PeerDiscovered {
        peer_id: String,
        name: String,
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
    },
    GameStarted,
    DiceRolled {
        die1: u8,
        die2: u8,
        roller: String,
    },
    BetPlaced {
        amount: u64,
        bet_type: String,
        player: String,
    },
    PayoutReceived {
        amount: u64,
    },
    GameEnded {
        winner: Option<String>,
    },
    NetworkStateChanged {
        new_state: NetworkState,
    },
    Error {
        message: String,
    },
}

/// Network state for UI
#[derive(Debug, Clone)]
pub enum NetworkState {
    Offline,
    Scanning,
    Connecting,
    Connected,
    Syncing,
}

/// Node status information
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct NodeStatus {
    pub peer_id: String,
    pub state: NodeState,
    pub connected_peers: u32,
    pub active_games: u32,
    pub total_balance: u64,
    pub battery_level: Option<f32>,
    pub bluetooth_enabled: bool,
}

/// Node state enum
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug, Clone)]
pub enum NodeState {
    Initializing,
    Ready,
    Discovering,
    InGame,
    Syncing,
    Error,
}

/// Peer information
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub name: String,
    pub reputation: u32,
    pub games_played: u32,
    pub connection_quality: ConnectionQuality,
}

/// Connection quality indicator
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug, Clone)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
}

/// Network statistics
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub average_latency_ms: u32,
    pub packet_loss_percent: f32,
}

/// Game configuration for mobile
#[derive(Debug, Clone)]
pub struct GameConfig {
    pub min_bet: u64,
    pub max_bet: u64,
    pub player_limit: usize,
    pub timeout_seconds: u32,
    pub allow_spectators: bool,
}

impl BitCrapsNode {
    /// Start discovery for nearby peers
    pub async fn start_discovery(&self) -> Result<()> {
        // Start the mesh service which includes discovery
        self.mesh_service
            .start()
            .await
            .map_err(|e| BitCrapsError::NetworkError {
                reason: format!("Failed to start discovery: {}", e),
            })?;
        Ok(())
    }

    /// Stop discovery
    pub async fn stop_discovery(&self) {
        // Stop the mesh service
        self.mesh_service.stop().await;
    }

    /// Create a new game
    pub async fn create_game(&self, config: GameConfig) -> Result<Arc<GameHandle>> {
        let game_id = Uuid::new_v4().to_string();

        // Create game session with proper initialization
        let session = Arc::new(GameSession {
            id: Uuid::new_v4().to_string(),
            game_id: game_id.clone(),
            players: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            state: Arc::new(tokio::sync::RwLock::new(
                GameSessionState::WaitingForPlayers,
            )),
            config: GameSessionConfig {
                min_bet: config.min_bet,
                max_bet: config.max_bet,
                auto_start: true,
                game_specific_config: {
                    let mut map = HashMap::new();
                    map.insert(
                        "player_limit".to_string(),
                        serde_json::json!(config.player_limit),
                    );
                    map.insert(
                        "timeout_seconds".to_string(),
                        serde_json::json!(config.timeout_seconds),
                    );
                    map.insert(
                        "allow_spectators".to_string(),
                        serde_json::json!(config.allow_spectators),
                    );
                    map
                },
            },
            stats: GameSessionStats {
                total_bets: std::sync::atomic::AtomicU64::new(0),
                total_volume: std::sync::atomic::AtomicU64::new(0),
                games_played: std::sync::atomic::AtomicU64::new(0),
            },
            created_at: std::time::SystemTime::now(),
            last_activity: Arc::new(tokio::sync::RwLock::new(std::time::SystemTime::now())),
        });

        let handle = Arc::new(GameHandle {
            game_id: game_id.clone(),
            session: session.clone(),
        });

        // Store in active games
        if let Ok(mut games) = self.active_games.lock() {
            games.insert(game_id, session);
        }

        Ok(handle)
    }

    /// Join an existing game
    pub async fn join_game(&self, game_id: String) -> Result<Arc<GameHandle>> {
        // Parse game ID
        let game_id_bytes = hex::decode(&game_id).map_err(|_| BitCrapsError::InvalidInput {
            reason: "Invalid game ID format".to_string(),
        })?;

        // Validate game ID format
        if game_id_bytes.len() != 32 {
            return Err(BitCrapsError::InvalidInput {
                reason: "Game ID must be 64 hex characters".to_string(),
            });
        }

        // Join game via mesh network
        let game_id_bytes: [u8; 32] = hex::decode(&game_id)
            .map_err(|_| BitCrapsError::InvalidInput {
                reason: "Invalid hex in game ID".to_string(),
            })?
            .try_into()
            .map_err(|_| BitCrapsError::InvalidInput {
                reason: "Invalid game ID length".to_string(),
            })?;

        // Send join request through mesh network
        let join_payload = serde_json::json!({
            "game_id": hex::encode(game_id_bytes),
            "player_id": hex::encode(self.peer_id),
        });

        let join_msg = crate::mesh::MeshMessage {
            message_type: crate::mesh::MeshMessageType::DirectMessage,
            payload: serde_json::to_vec(&join_payload).unwrap_or_default(),
            sender: self.peer_id,
            recipient: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: vec![], // Signature will be added by mesh service
        };

        // Find the game host and send join request
        if let Some(response) = self.mesh_service.poll_discovery_response().await {
            // Process game discovery response
            log::info!("Found game to join: {:?}", response);
        }

        // For now, return not found if game doesn't exist
        Err(BitCrapsError::NotFound {
            item: format!("Game {}", game_id),
        })
    }

    /// Leave current game
    pub async fn leave_game(&self) -> Result<()> {
        // Clear active game from storage
        if let Ok(mut games) = self.active_games.lock() {
            // Remove all games for now (simplified)
            games.clear();
        }
        Ok(())
    }

    /// Poll for next event
    pub async fn poll_event(&self) -> Option<GameEvent> {
        // In production, this would pull from an event queue
        // For now, return None to indicate no pending events
        None
    }

    /// Drain all pending events
    pub async fn drain_events(&self) -> Vec<GameEvent> {
        // In production, this would drain an event queue
        // For now, return empty vector
        Vec::new()
    }

    /// Get current node status
    pub fn get_status(&self) -> NodeStatus {
        NodeStatus {
            peer_id: hex::encode(&self.peer_id),
            state: NodeState::Ready,
            connected_peers: 0,
            active_games: self
                .active_games
                .lock()
                .map(|g| g.len() as u32)
                .unwrap_or(0),
            total_balance: 0,
            battery_level: None,
            bluetooth_enabled: false,
        }
    }

    /// Get connected peers
    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        // For now, return empty list as mesh peer tracking is async
        // In production, this would use futures::executor::block_on or similar
        Vec::new()
    }

    /// Get network statistics
    pub fn get_network_stats(&self) -> NetworkStats {
        NetworkStats {
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            average_latency_ms: 0,
            packet_loss_percent: 0.0,
        }
    }

    /// Set power mode for battery optimization
    pub fn set_power_mode(&self, mode: PowerMode) -> Result<()> {
        // Adjust mesh service parameters based on power mode
        match mode {
            PowerMode::PowerSaver => {
                self.mesh_service
                    .set_heartbeat_interval(Duration::from_secs(120));
                self.mesh_service.set_peer_timeout(Duration::from_secs(300));
            }
            PowerMode::Balanced => {
                self.mesh_service
                    .set_heartbeat_interval(Duration::from_secs(60));
                self.mesh_service.set_peer_timeout(Duration::from_secs(180));
            }
            PowerMode::HighPerformance => {
                self.mesh_service
                    .set_heartbeat_interval(Duration::from_secs(30));
                self.mesh_service.set_peer_timeout(Duration::from_secs(90));
            }
        }
        Ok(())
    }
}

/// Power modes for mobile battery optimization
#[derive(Debug, Clone)]
pub enum PowerMode {
    HighPerformance,
    Balanced,
    PowerSaver,
}

impl GameHandle {
    /// Place a bet in the current game
    pub async fn place_bet(&self, amount: u64, bet_type: String) -> Result<()> {
        // Store bet in session (simplified for mobile demo)
        // In production, this would broadcast to consensus layer
        log::info!("Bet placed: {} CRAP on {}", amount, bet_type);
        Ok(())
    }

    /// Roll dice in the current game
    pub async fn roll_dice(&self) -> Result<(u8, u8)> {
        // Generate cryptographically secure random dice
        use rand::{rngs::OsRng, Rng};
        let mut rng = OsRng;
        let die1 = rng.gen_range(1..=6);
        let die2 = rng.gen_range(1..=6);

        // In production, this would use consensus-based rolling
        // with commitment scheme and multi-party computation
        Ok((die1, die2))
    }

    /// Get current game state
    pub fn get_game_state(&self) -> GameState {
        GameState::Waiting
    }

    /// Get game history
    pub fn get_history(&self) -> Vec<GameHistoryEntry> {
        Vec::new()
    }
}

/// Game state for UI
#[derive(Debug, Clone)]
pub enum GameState {
    Waiting,
    Starting,
    InProgress,
    RollingDice,
    ResolvingBets,
    Ended,
}

/// Game history entry
#[derive(Debug, Clone)]
pub struct GameHistoryEntry {
    pub timestamp: u64,
    pub event_type: String,
    pub details: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let config = BitCrapsConfig::default();
        let node = create_node(config);
        assert!(node.is_ok());
    }

    #[test]
    fn test_get_adapters() {
        let adapters = get_available_bluetooth_adapters();
        assert!(!adapters.is_empty());
    }
}
