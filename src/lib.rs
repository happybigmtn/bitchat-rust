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
pub mod bridges; // Cross-chain bridge infrastructure for multi-blockchain interoperability
pub mod cache; // Multi-tier caching system
pub mod compliance; // Regulatory compliance and KYC/AML systems
pub mod config;
pub mod contracts; // Smart contract integration and cross-chain bridges
pub mod coordinator; // Network coordination and monitoring
pub mod crypto; // Cryptographic foundations
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub mod database;
#[cfg(feature = "bluetooth")]
pub mod discovery; // Peer discovery (Bluetooth, DHT)
pub mod economics; // Advanced token economics and supply management
pub mod edge; // Edge computing and CDN integration
pub mod error;
pub mod gaming; // Gaming interfaces and session management
pub mod governance; // Decentralized autonomous organization and governance
#[cfg(feature = "gpu")]
pub mod gpu; // GPU acceleration for physics, crypto, and ML
pub mod keystore; // Secure key management
pub mod logging; // Production logging and observability
pub mod memory_pool; // Memory pooling for performance optimization
pub mod mesh; // Mesh networking coordination
#[cfg(any(feature = "android", feature = "uniffi"))]
pub mod mobile; // Mobile platform bindings and UniFFI interface
#[cfg(feature = "monitoring")]
pub mod monitoring; // Production monitoring and metrics
pub mod optimization; // Performance optimizations
pub mod performance; // Performance benchmarking and analysis
pub mod persistence; // Data persistence layer
pub mod platform; // Platform-specific integrations (Android, iOS)
pub mod plugins; // Plugin system for additional casino games
pub mod profiling; // Performance profiling and analysis
pub mod protocol; // Core protocol and binary serialization
pub mod resilience; // Network resilience and fault tolerance
pub mod security;
pub mod session; // Session management with Noise protocol
pub mod token; // Token economics and CRAP tokens
pub mod transport; // Network transport layer (Bluetooth mesh)
pub mod treasury; // Treasury management and automated market making
#[cfg(feature = "ui")]
pub mod ui; // User interface (CLI and TUI)
pub mod utils; // Utility functions and helpers
pub mod validation; // Security hardening and input validation
pub mod services; // Microservices architecture
pub mod wasm; // WebAssembly runtime and plugin system

// UniFFI type tag (required by generated code)
#[cfg(feature = "uniffi")]
pub struct UniFfiTag;

// Re-export commonly used types for easy access
pub use coordinator::{HealthMetrics, MultiTransportCoordinator, NetworkMonitor, NetworkTopology};
pub use crypto::{BitchatIdentity, BitchatKeypair, GameCrypto, ProofOfWork};
#[cfg(feature = "bluetooth")]
pub use discovery::{BluetoothDiscovery, DhtDiscovery, DhtPeer, DiscoveredPeer};
pub use error::{Error, Result};
pub use mesh::{MeshPeer, MeshService};
pub use protocol::craps::{CrapsGame, GamePhase};
pub use protocol::runtime::GameRuntime;
pub use protocol::versioning::{
    ProtocolCompatibility, ProtocolFeature, ProtocolVersion, VersionedMessage,
};
pub use protocol::{BetType, CrapTokens, DiceRoll, GameId, PeerId};
pub use transport::{TransportAddress, TransportCoordinator};
#[cfg(feature = "bluetooth")]
pub use transport::BluetoothTransport;
pub const TREASURY_ADDRESS: PeerId = [0xFFu8; 32];
pub use app::{ApplicationConfig, BitCrapsApp};
pub use contracts::{
    BlockchainNetwork, BridgeContract, ContractManager, StakingContract, TokenContract,
};
pub use economics::{AdvancedStakingPosition, EconomicsConfig, EconomicsStats, TokenEconomics};
#[cfg(feature = "monitoring")]
pub use monitoring::{HealthCheck, NetworkDashboard, NetworkMetrics};
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
#[cfg(feature = "ui")]
pub use ui::{Cli, Commands};
pub use utils::{AdaptiveInterval, AdaptiveIntervalConfig};
#[cfg(feature = "compliance")]
pub use compliance::{
    ComplianceConfig, ComplianceLevel, ComplianceManager, ComplianceStatus, ComplianceRestriction,
    KycProvider, KycStatus, AmlMonitor, RiskScore, SanctionsScreening, SanctionsResult,
    AuditLogger, AuditEvent, ComplianceAudit,
};
#[cfg(feature = "governance")]
pub use governance::{
    GovernanceConfig, GovernanceCoordinator, Dao, DaoMember, MembershipTier,
    Proposal, ProposalType, VotingMechanism, VotingPower,
};
pub use edge::{
    EdgeRuntime, EdgeRuntimeConfig, EdgeNode, EdgeNodeId, EdgeWorkload, WorkloadType,
    GeoLocation, EdgeCapabilities, EdgeMetrics, EdgeNodeStatus,
    CdnManager, CdnConfig, CdnProvider, EdgeWorker,
    EdgeOrchestrator, OrchestratorConfig, EdgeCluster, AutoScalingConfig,
    MecManager, MecConfig, MecPlatform, NetworkSlice, QosClass,
    EdgeCacheManager, EdgeCacheConfig, CacheTier, CacheMetrics,
};
pub use bridges::{
    Bridge, BridgeConfig, BridgeTransaction, BridgeTransactionStatus, BridgeEvent,
    ValidatorSignature, ChainId, BridgeSecurityManager, BridgeStateManager, BridgeEventMonitor,
    BridgeStatistics, FraudDetectionRule,
};
pub use bridges::ethereum::{
    EthereumBridge, EthereumBridgeConfig, EthereumTransaction, EthereumTxStatus,
    ContractInterface, GasSettings,
};
pub use bridges::bitcoin::{
    BitcoinBridge, BitcoinBridgeConfig, BitcoinNetwork, MultisigWallet, PartiallySignedTransaction,
    LightningConfig, LightningPayment, LightningPaymentStatus, AtomicSwap, AtomicSwapStatus,
    AtomicSwapConfig, BitcoinUTXO,
};
pub use bridges::universal::{
    UniversalBridge, UniversalBridgeConfig, NetworkConfig, BridgeProtocol, IBCConfig,
    CrossChainMessage, MessageStatus, CrossChainRoute, RouteHop, MessagingConfig,
    LiquidityConfig, LiquidityPool, RoutingConfig, RoutingAlgorithm, BridgePlugin,
};

/// Application configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    Validator,
    Gateway,
    Client,
}

#[derive(Clone)]
pub struct AppConfig {
    pub data_dir: String,
    pub nickname: Option<String>,
    pub pow_difficulty: u32,
    pub max_connections: usize,
    pub enable_treasury: bool,
    /// Node role for tiered architecture
    pub role: NodeRole,
    /// PBFT tuning: optional batch size override
    pub pbft_batch_size: Option<usize>,
    /// PBFT tuning: optional pipeline depth override
    pub pbft_pipeline_depth: Option<usize>,
    /// PBFT tuning: optional base timeout (ms)
    pub pbft_base_timeout_ms: Option<u64>,
    /// PBFT tuning: optional view change timeout (ms)
    pub pbft_view_timeout_ms: Option<u64>,
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
            role: NodeRole::Client,
            pbft_batch_size: None,
            pbft_pipeline_depth: None,
            pbft_base_timeout_ms: None,
            pbft_view_timeout_ms: None,
            listen_tcp: None,
            connect_tcp: Vec::new(),
            enable_ble: false,
            prometheus_port: Some(9090),
            dashboard_port: Some(8080),
        }
    }
}
