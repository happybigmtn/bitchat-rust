//! BitCraps - A decentralized, peer-to-peer casino protocol
#![allow(dead_code)] // Allow dead code during development
#![allow(unused_variables)] // Allow unused variables during development
#![allow(unused_assignments)] // Allow unused assignments during development
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

pub mod app; // Main application coordinator
pub mod cache; // Multi-tier caching system
pub mod config;
pub mod contracts; // Smart contract integration and cross-chain bridges
pub mod coordinator; // Network coordination and monitoring
pub mod crypto; // Cryptographic foundations
pub mod database;
pub mod discovery; // Peer discovery (Bluetooth, DHT)
pub mod economics; // Advanced token economics and supply management
pub mod error;
pub mod gaming; // Gaming interfaces and session management
pub mod keystore; // Secure key management
pub mod logging; // Production logging and observability
pub mod memory_pool; // Memory pooling for performance optimization
pub mod mesh; // Mesh networking coordination
pub mod mobile; // Mobile platform bindings and UniFFI interface
pub mod monitoring; // Production monitoring and metrics
pub mod optimization; // Performance optimizations
pub mod performance; // Performance benchmarking and analysis
pub mod persistence; // Data persistence layer
pub mod platform; // Platform-specific integrations (Android, iOS)
pub mod profiling; // Performance profiling and analysis
pub mod protocol; // Core protocol and binary serialization
pub mod resilience; // Network resilience and fault tolerance
pub mod security;
pub mod session; // Session management with Noise protocol
pub mod token; // Token economics and CRAP tokens
pub mod transport; // Network transport layer (Bluetooth mesh)
pub mod treasury; // Treasury management and automated market making
pub mod ui; // User interface (CLI and TUI)
pub mod utils; // Utility functions and helpers
pub mod validation; // Security hardening and input validation

// UniFFI type tag (required by generated code)
#[cfg(feature = "uniffi")]
pub struct UniFfiTag;

// Re-export commonly used types for easy access
pub use coordinator::{HealthMetrics, MultiTransportCoordinator, NetworkMonitor, NetworkTopology};
pub use crypto::{BitchatIdentity, BitchatKeypair, GameCrypto, ProofOfWork};
pub use discovery::{BluetoothDiscovery, DhtDiscovery, DhtPeer, DiscoveredPeer};
pub use error::{Error, Result};
pub use mesh::{MeshPeer, MeshService};
pub use protocol::craps::{CrapsGame, GamePhase};
pub use protocol::runtime::GameRuntime;
pub use protocol::versioning::{
    ProtocolCompatibility, ProtocolFeature, ProtocolVersion, VersionedMessage,
};
pub use protocol::{BetType, CrapTokens, DiceRoll, GameId, PeerId};
pub use transport::{BluetoothTransport, TransportAddress, TransportCoordinator};
pub const TREASURY_ADDRESS: PeerId = [0xFFu8; 32];
pub use app::{ApplicationConfig, BitCrapsApp};
pub use contracts::{
    BlockchainNetwork, BridgeContract, ContractManager, StakingContract, TokenContract,
};
pub use economics::{AdvancedStakingPosition, EconomicsConfig, EconomicsStats, TokenEconomics};
#[cfg(not(feature = "mvp"))]
pub use monitoring::{HealthCheck, NetworkDashboard, NetworkMetrics};
#[cfg(feature = "mvp")]
pub use monitoring::metrics::NetworkMetrics;
pub use persistence::PersistenceManager;
pub use security::{
    DosProtection, InputValidator, RateLimiter, SecurityConfig, SecurityEvent, SecurityEventLogger,
    SecurityLevel, SecurityLimits, SecurityManager,
};
pub use session::{BitchatSession, SessionLimits, SessionManager};
pub use token::{Account, ProofOfRelay, TokenLedger, TransactionType};
pub use treasury::{
    AutomatedMarketMaker, TreasuryConfig, TreasuryManager, TreasuryStats, TreasuryWallet,
};
pub use ui::{Cli, Commands};
pub use utils::{AdaptiveInterval, AdaptiveIntervalConfig};

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: String,
    pub nickname: Option<String>,
    pub pow_difficulty: u32,
    pub max_connections: usize,
    pub enable_treasury: bool,
    // MVP networking options
    pub listen_tcp: Option<String>,
    pub connect_tcp: Vec<String>,
    pub enable_ble: bool,
    // Monitoring options
    pub prometheus_port: Option<u16>,
    pub dashboard_port: Option<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            data_dir: "~/.bitcraps".to_string(),
            pow_difficulty: 16,
            max_connections: 50,
            enable_treasury: true,
            nickname: None,
            listen_tcp: None,
            connect_tcp: Vec::new(),
            enable_ble: false,
            prometheus_port: Some(9090),
            dashboard_port: Some(8080),
        }
    }
}
