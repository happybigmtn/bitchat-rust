//! BitCraps - A decentralized, peer-to-peer casino protocol
#![allow(dead_code)]  // Allow dead code during development
#![allow(unused_variables)]  // Allow unused variables during development
#![allow(unused_assignments)]  // Allow unused assignments during development
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
pub mod logging;      // Production logging and observability
pub mod resilience;   // Network resilience and fault tolerance
pub mod keystore;     // Secure key management
pub mod memory_pool;  // Memory pooling for performance optimization
pub mod protocol;     // Core protocol and binary serialization
pub mod crypto;       // Cryptographic foundations
pub mod transport;    // Network transport layer (Bluetooth mesh)
pub mod mesh;         // Mesh networking coordination
pub mod discovery;    // Peer discovery (Bluetooth, DHT)
pub mod coordinator;  // Network coordination and monitoring
pub mod gaming;       // Gaming interfaces and session management
pub mod session;      // Session management with Noise protocol
pub mod token;        // Token economics and CRAP tokens
pub mod ui;           // User interface (CLI and TUI)
pub mod platform;     // Platform-specific integrations (Android, iOS)
pub mod monitoring;   // Production monitoring and metrics
pub mod optimization; // Performance optimizations
pub mod persistence;  // Data persistence layer
pub mod cache;        // Multi-tier caching system
pub mod mobile;       // Mobile platform bindings and UniFFI interface
pub mod performance;  // Performance benchmarking and analysis
pub mod profiling;    // Performance profiling and analysis
pub mod economics;    // Advanced token economics and supply management
pub mod treasury;     // Treasury management and automated market making
pub mod contracts;    // Smart contract integration and cross-chain bridges
pub mod app;          // Main application coordinator
pub mod security;     // Security hardening and input validation

// UniFFI type tag (required by generated code)
#[cfg(feature = "uniffi")]
pub struct UniFfiTag;


// Re-export commonly used types for easy access
pub use error::{Error, Result};
pub use protocol::{
    PeerId, GameId, BetType, DiceRoll, CrapTokens,
};
pub use protocol::versioning::{
    ProtocolVersion, ProtocolFeature, ProtocolCompatibility, VersionedMessage,
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
pub use economics::{
    TokenEconomics, EconomicsConfig, AdvancedStakingPosition, EconomicsStats,
};
pub use app::{
    BitCrapsApp, ApplicationConfig,
};
pub use treasury::{
    TreasuryManager, TreasuryConfig, TreasuryWallet, AutomatedMarketMaker, TreasuryStats,
};
pub use contracts::{
    ContractManager, BlockchainNetwork, TokenContract, StakingContract, BridgeContract,
};
pub use security::{
    SecurityManager, SecurityConfig, SecurityLimits, InputValidator, RateLimiter, 
    DosProtection, SecurityEventLogger, SecurityEvent, SecurityLevel,
};

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
