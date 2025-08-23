//! BitCraps - A decentralized, peer-to-peer casino protocol
//! 
//! Feynman Explanation: This is the "master blueprint" for our decentralized casino.
//! Think of it as a city plan where each module is a different district:
//! - protocol: The "language" everyone speaks (like traffic laws)
//! - crypto: The "locks and keys" for security
//! - transport: The "roads and highways" for communication
//! - mesh: The "network coordinator" managing peer connections
//! - gaming: The "casino floor" with all the games
//! - session: The "secure phone lines" for encrypted communication
//! - token: The "bank" managing CRAP tokens
//! 
//! All modules are now implemented and working together to create a complete
//! decentralized craps casino over Bluetooth mesh networks.

pub mod error;
pub mod config;
pub mod database;
pub mod validation;
pub mod protocol;     // Core protocol and binary serialization
pub mod crypto;       // Cryptographic foundations
pub mod transport;    // Network transport layer (Bluetooth mesh)
pub mod mesh;         // Mesh networking coordination
pub mod discovery;    // Peer discovery (Bluetooth, DHT)
pub mod coordinator;  // Network coordination and monitoring
// Gaming is now integrated into protocol module (protocol/craps.rs and protocol/runtime.rs)
pub mod session;      // Session management with Noise protocol
pub mod token;        // Token economics and CRAP tokens
pub mod ui;           // User interface (CLI and TUI)
pub mod platform;     // Platform-specific integrations (Android, iOS)
pub mod monitoring;   // Production monitoring and metrics
pub mod optimization; // Performance optimizations
pub mod persistence;  // Data persistence layer
pub mod cache;        // Multi-tier caching system

// Re-export commonly used types for easy access
pub use error::{Error, Result};
pub use protocol::{
    PeerId, GameId, BitchatPacket, BetType, DiceRoll, CrapTokens, 
    PacketUtils, PACKET_TYPE_PING, PACKET_TYPE_GAME_CREATE,
};
pub use crypto::{
    BitchatKeypair, BitchatIdentity, GameCrypto, ProofOfWork,
};
pub use transport::{
    TransportCoordinator, BluetoothTransport, TransportAddress,
};
pub use mesh::{
    MeshService, MeshPeer,
};
pub use discovery::{
    BluetoothDiscovery, DhtDiscovery, DiscoveredPeer, DhtPeer,
};
pub use coordinator::{
    MultiTransportCoordinator, NetworkMonitor, NetworkTopology, HealthMetrics,
};
pub use protocol::craps::{
    CrapsGame, GamePhase,
};
pub use protocol::runtime::{
    GameRuntime,
};
pub const TREASURY_ADDRESS: PeerId = [0xFFu8; 32];
pub use session::{
    SessionManager, BitchatSession, SessionLimits,
};
pub use token::{
    TokenLedger, ProofOfRelay, Account, TransactionType,
};
pub use ui::{
    Cli, Commands,
};
pub use monitoring::{
    NetworkDashboard, NetworkMetrics, HealthCheck,
};
pub use persistence::PersistenceManager;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: String,
    pub nickname: Option<String>,
    pub pow_difficulty: u32,
    pub max_connections: usize,
    pub enable_treasury: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_dir: "~/.bitcraps".to_string(),
            pow_difficulty: 16,
            max_connections: 50,
            enable_treasury: true,
            nickname: None,
        }
    }
}