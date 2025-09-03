//! Core Gateway Node Implementation
//!
//! Provides the main gateway node functionality for bridging BLE mesh networks
//! to internet connectivity with load balancing, failover, and redundancy features.

use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, RwLock, Mutex, oneshot};
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::{PeerId, BitchatPacket};
use crate::protocol::versioning::ProtocolVersion;
use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::transport::TransportAddress;

/// Gateway node specific errors
#[derive(Error, Debug)]
pub enum GatewayNodeError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Bandwidth limit exceeded: {current} > {limit}")]
    BandwidthLimit { current: f64, limit: f64 },
    #[error("Maximum peers exceeded: {current} > {limit}")]
    PeerLimit { current: usize, limit: usize },
    #[error("Gateway unavailable: {0}")]
    Unavailable(String),
}

/// Gateway node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Local mesh interface configuration
    pub local_interface: GatewayInterface,
    /// Internet interface configuration
    pub internet_interface: GatewayInterface,
    /// Maximum peers per gateway
    pub max_peers: usize,
    /// Gateway discovery interval
    pub discovery_interval: Duration,
    /// Heartbeat interval for health checks
    pub heartbeat_interval: Duration,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Enable gateway relay rewards
    pub enable_relay_rewards: bool,
    /// Relay fee per message (in CrapTokens)
    pub relay_fee: u64,
    /// Load balancing configuration
    pub load_balancing: LoadBalancingConfig,
    /// Failover configuration
    pub failover: FailoverConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Interface configuration for gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayInterface {
    pub bind_address: SocketAddr,
    pub protocol: GatewayProtocol,
    pub max_bandwidth_mbps: f64,
    pub max_connections: usize,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub connection_timeout: Duration,
}

/// Supported gateway protocols
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum GatewayProtocol {
    Tcp,
    Udp,
    WebSocket,
    Quic,
    BleMesh,
}

/// Load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    pub algorithm: LoadBalancingAlgorithm,
    pub health_check_interval: Duration,
    pub failure_threshold: u32,
    pub recovery_time: Duration,
    pub sticky_sessions: bool,
}

/// Load balancing algorithms
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin,
    LeastResponseTime,
    IPHash,
    Random,
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    pub enable_failover: bool,
    pub detection_interval: Duration,
    pub failover_timeout: Duration,
    pub max_failures: u32,
    pub recovery_delay: Duration,
    pub enable_preemptive_failover: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub require_authentication: bool,
    pub rate_limiting: RateLimitConfig,
    pub ddos_protection: DDoSProtectionConfig,
    pub enable_firewall: bool,
    pub trusted_peers: Vec<PeerId>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_second: u32,
    pub burst_size: u32,
    pub ban_duration: Duration,
    pub enable_adaptive_limits: bool,
}

/// DDoS protection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DDoSProtectionConfig {
    pub enable_protection: bool,
    pub connection_limit_per_ip: u32,
    pub packet_rate_limit: u32,
    pub detection_threshold: f64,
    pub mitigation_timeout: Duration,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            local_interface: GatewayInterface {
                bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8333),
                protocol: GatewayProtocol::Tcp,
                max_bandwidth_mbps: 10.0,
                max_connections: 100,
                enable_compression: true,
                enable_encryption: true,
                connection_timeout: Duration::from_secs(30),
            },
            internet_interface: GatewayInterface {
                bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8334),
                protocol: GatewayProtocol::Tcp,
                max_bandwidth_mbps: 100.0,
                max_connections: 1000,
                enable_compression: true,
                enable_encryption: true,
                connection_timeout: Duration::from_secs(30),
            },
            max_peers: 1000,
            discovery_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            connection_timeout: Duration::from_secs(30),
            enable_relay_rewards: true,
            relay_fee: 1,
            load_balancing: LoadBalancingConfig {
                algorithm: LoadBalancingAlgorithm::LeastConnections,
                health_check_interval: Duration::from_secs(5),
                failure_threshold: 3,
                recovery_time: Duration::from_secs(30),
                sticky_sessions: false,
            },
            failover: FailoverConfig {
                enable_failover: true,
                detection_interval: Duration::from_secs(5),
                failover_timeout: Duration::from_secs(10),
                max_failures: 3,
                recovery_delay: Duration::from_secs(60),
                enable_preemptive_failover: true,
            },
            security: SecurityConfig {
                require_authentication: true,
                rate_limiting: RateLimitConfig {
                    requests_per_second: 100,
                    burst_size: 200,
                    ban_duration: Duration::from_secs(300),
                    enable_adaptive_limits: true,
                },
                ddos_protection: DDoSProtectionConfig {
                    enable_protection: true,
                    connection_limit_per_ip: 10,
                    packet_rate_limit: 1000,
                    detection_threshold: 0.8,
                    mitigation_timeout: Duration::from_secs(300),
                },
                enable_firewall: true,
                trusted_peers: Vec::new(),
            },
        }
    }
}

/// Gateway node that bridges local mesh to internet
pub struct GatewayNode {
    identity: Arc<BitchatIdentity>,
    config: GatewayConfig,
    mesh_service: Arc<MeshService>,

    // Peer management
    local_peers: Arc<RwLock<HashMap<PeerId, LocalPeer>>>,
    internet_peers: Arc<RwLock<HashMap<PeerId, InternetPeer>>>,

    // Gateway registry and routing
    gateway_registry: Arc<RwLock<HashMap<PeerId, GatewayInfo>>>,
    routing_table: Arc<RwLock<HashMap<PeerId, GatewayRoute>>>,

    // Monitoring and statistics
    bandwidth_monitor: Arc<BandwidthMonitor>,
    connection_manager: Arc<ConnectionManager>,
    relay_stats: Arc<RwLock<RelayStatistics>>,

    // Event handling
    event_sender: mpsc::UnboundedSender<GatewayEvent>,
    event_receiver: Arc<Mutex<mpsc::UnboundedReceiver<GatewayEvent>>>,

    // Control
    is_running: Arc<RwLock<bool>>,
    shutdown_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

/// Local mesh peer information
#[derive(Debug, Clone)]
pub struct LocalPeer {
    pub peer_id: PeerId,
    pub connected_at: Instant,
    pub last_seen: Instant,
    pub address: Option<TransportAddress>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_relayed: u64,
    pub reputation: f64,
    pub connection_quality: ConnectionQuality,
    pub last_activity: Instant,
}

/// Internet peer information
#[derive(Debug, Clone)]
pub struct InternetPeer {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub connected_at: Instant,
    pub last_ping: Instant,
    pub rtt: Duration,
    pub bandwidth_usage: f64,
    pub protocol_version: ProtocolVersion,
    pub bytes_relayed: u64,
    pub relay_fee_earned: u64,
    pub connection_quality: ConnectionQuality,
    pub last_activity: Instant,
}

/// Connection quality metrics
#[derive(Debug, Clone)]
pub struct ConnectionQuality {
    pub latency_ms: u32,
    pub packet_loss: f64,
    pub jitter_ms: u32,
    pub throughput_mbps: f64,
    pub reliability_score: f64,
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            packet_loss: 0.0,
            jitter_ms: 0,
            throughput_mbps: 0.0,
            reliability_score: 1.0,
        }
    }
}

/// Gateway information for registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayInfo {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub protocol: GatewayProtocol,
    pub max_bandwidth_mbps: f64,
    pub current_load: f64,
    pub relay_fee: u64,
    pub uptime_percentage: f64,
    pub protocol_version: ProtocolVersion,
    pub last_seen: SystemTime,
    pub capabilities: Vec<String>,
    pub reputation: f64,
}

/// Gateway routing information
#[derive(Debug, Clone)]
pub struct GatewayRoute {
    pub destination: PeerId,
    pub via_gateway: PeerId,
    pub hop_count: u8,
    pub bandwidth_cost: f64,
    pub latency_ms: u32,
    pub reliability: f64,
    pub last_updated: Instant,
    pub path_quality: f64,
}

/// Bandwidth monitoring system
pub struct BandwidthMonitor {
    local_usage: Arc<Mutex<f64>>,
    internet_usage: Arc<Mutex<f64>>,
    usage_history: Arc<RwLock<VecDeque<BandwidthSample>>>,
    limits: GatewayConfig,
    alert_threshold: f64,
}

/// Bandwidth usage sample
#[derive(Debug, Clone)]
pub struct BandwidthSample {
    pub timestamp: Instant,
    pub local_mbps: f64,
    pub internet_mbps: f64,
    pub total_connections: usize,
}

/// Connection management system
pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<PeerId, ConnectionInfo>>>,
    connection_pool: Arc<RwLock<ConnectionPool>>,
    rate_limiter: Arc<RateLimiter>,
}

/// Connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub bytes_transferred: u64,
    pub connection_state: ConnectionState,
}

/// Connection states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Degraded,
    Failed,
    Disconnected,
}

/// Connection pool for managing reusable connections
pub struct ConnectionPool {
    tcp_pool: HashMap<SocketAddr, Vec<TcpStream>>,
    udp_pool: HashMap<SocketAddr, Arc<UdpSocket>>,
    max_pool_size: usize,
    cleanup_interval: Duration,
}

/// Rate limiting system
pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<IpAddr, RateLimit>>>,
    global_limit: Arc<Mutex<TokenBucket>>,
}

/// Rate limit per IP address
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub tokens: TokenBucket,
    pub last_refill: Instant,
    pub violations: u32,
    pub banned_until: Option<Instant>,
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
pub struct TokenBucket {
    pub tokens: f64,
    pub capacity: f64,
    pub refill_rate: f64,
    pub last_refill: Instant,
}

/// Relay statistics for performance monitoring
#[derive(Debug, Default, Clone)]
pub struct RelayStatistics {
    pub messages_relayed: u64,
    pub bytes_relayed: u64,
    pub tokens_earned: u64,
    pub uptime_start: Option<Instant>,
    pub last_reward_claim: Option<Instant>,
    pub error_count: u64,
    pub peak_throughput: f64,
    pub average_latency: Duration,
}

/// Gateway events for monitoring and alerts
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    // Connection events
    LocalPeerConnected { peer_id: PeerId },
    LocalPeerDisconnected { peer_id: PeerId, reason: String },
    InternetPeerConnected { peer_id: PeerId, address: SocketAddr },
    InternetPeerDisconnected { peer_id: PeerId, reason: String },

    // Relay events
    MessageRelayed { from: PeerId, to: PeerId, bytes: usize },
    RelayRewardEarned { tokens: u64 },

    // System events
    BandwidthLimitReached { interface: String, limit_mbps: f64 },
    ConnectionLimitReached { current: usize, limit: usize },

    // Discovery events
    GatewayDiscovered { gateway: GatewayInfo },
    GatewayLost { peer_id: PeerId, reason: String },

    // Error events
    ProtocolError { peer_id: Option<PeerId>, error: String },
    SecurityViolation { source: SocketAddr, violation: String },
    FailoverTriggered { old_gateway: PeerId, new_gateway: PeerId },
}

/// Gateway statistics for monitoring
#[derive(Debug, Clone)]
pub struct GatewayStats {
    pub local_peers: usize,
    pub internet_peers: usize,
    pub known_gateways: usize,
    pub messages_relayed: u64,
    pub bytes_relayed: u64,
    pub tokens_earned: u64,
    pub bandwidth_usage: BandwidthUsage,
    pub uptime: Duration,
    pub success_rate: f64,
    pub average_latency: Duration,
    pub peak_throughput: f64,
    pub error_rate: f64,
}

/// Bandwidth usage information
#[derive(Debug, Clone)]
pub struct BandwidthUsage {
    pub local_mbps: f64,
    pub internet_mbps: f64,
    pub local_limit_mbps: f64,
    pub internet_limit_mbps: f64,
    pub utilization_percentage: f64,
}

impl GatewayNode {
    /// Create new gateway node
    pub fn new(
        identity: Arc<BitchatIdentity>,
        config: GatewayConfig,
        mesh_service: Arc<MeshService>,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::channel(1000); // Bounded gateway events

        Ok(Self {
            identity,
            mesh_service,
            local_peers: Arc::new(RwLock::new(HashMap::new())),
            internet_peers: Arc::new(RwLock::new(HashMap::new())),
            gateway_registry: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            bandwidth_monitor: Arc::new(BandwidthMonitor::new(config.clone())),
            connection_manager: Arc::new(ConnectionManager::new(config.clone())),
            relay_stats: Arc::new(RwLock::new(RelayStatistics::default())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            is_running: Arc::new(RwLock::new(false)),
            shutdown_sender: Arc::new(Mutex::new(None)),
            config,
        })
    }

    /// Start the gateway node
    pub async fn start(&self) -> Result<()> {
        *self.is_running.write().await = true;

        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        *self.shutdown_sender.lock().await = Some(shutdown_sender);

        // Initialize relay statistics
        {
            let mut stats = self.relay_stats.write().await;
            stats.uptime_start = Some(Instant::now());
        }

        // Start all components
        self.start_local_interface().await?;
        self.start_internet_interface().await?;
        self.start_gateway_discovery().await?;
        self.start_heartbeat_service().await?;
        self.start_bandwidth_monitoring().await?;
        self.start_connection_management().await?;

        if self.config.enable_relay_rewards {
            self.start_relay_rewards().await?;
        }

        log::info!(
            "Gateway node started - local:{}, internet:{}",
            self.config.local_interface.bind_address,
            self.config.internet_interface.bind_address
        );

        Ok(())
    }

    /// Stop the gateway node
    pub async fn stop(&self) {
        *self.is_running.write().await = false;

        // Trigger shutdown
        if let Some(sender) = self.shutdown_sender.lock().await.take() {
            let _ = sender.send(());
        }

        log::info!("Gateway node stopped");
    }

    /// Get gateway statistics
    pub async fn get_stats(&self) -> GatewayStats {
        let local_peers = self.local_peers.read().await;
        let internet_peers = self.internet_peers.read().await;
        let gateway_registry = self.gateway_registry.read().await;
        let relay_stats = self.relay_stats.read().await;

        let bandwidth_usage = self.bandwidth_monitor.get_usage().await;

        GatewayStats {
            local_peers: local_peers.len(),
            internet_peers: internet_peers.len(),
            known_gateways: gateway_registry.len(),
            messages_relayed: relay_stats.messages_relayed,
            bytes_relayed: relay_stats.bytes_relayed,
            tokens_earned: relay_stats.tokens_earned,
            bandwidth_usage,
            uptime: relay_stats.uptime_start.map(|start| start.elapsed()).unwrap_or(Duration::ZERO),
            success_rate: if relay_stats.messages_relayed > 0 {
                1.0 - (relay_stats.error_count as f64 / relay_stats.messages_relayed as f64)
            } else {
                1.0
            },
            average_latency: relay_stats.average_latency,
            peak_throughput: relay_stats.peak_throughput,
            error_rate: relay_stats.error_count as f64 / relay_stats.messages_relayed.max(1) as f64,
        }
    }

    /// Get next gateway event
    pub async fn next_event(&self) -> Option<GatewayEvent> {
        self.event_receiver.lock().await.recv().await
    }

    // Private implementation methods will be added here
    async fn start_local_interface(&self) -> Result<()> {
        // Implementation for starting local BLE mesh interface
        log::info!("Started local interface at {}", self.config.local_interface.bind_address);
        Ok(())
    }

    async fn start_internet_interface(&self) -> Result<()> {
        // Implementation for starting internet interface
        log::info!("Started internet interface at {}", self.config.internet_interface.bind_address);
        Ok(())
    }

    async fn start_gateway_discovery(&self) -> Result<()> {
        // Implementation for gateway discovery
        log::info!("Started gateway discovery service");
        Ok(())
    }

    async fn start_heartbeat_service(&self) -> Result<()> {
        // Implementation for heartbeat service
        log::info!("Started heartbeat service");
        Ok(())
    }

    async fn start_bandwidth_monitoring(&self) -> Result<()> {
        // Implementation for bandwidth monitoring
        log::info!("Started bandwidth monitoring");
        Ok(())
    }

    async fn start_connection_management(&self) -> Result<()> {
        // Implementation for connection management
        log::info!("Started connection management");
        Ok(())
    }

    async fn start_relay_rewards(&self) -> Result<()> {
        // Implementation for relay rewards
        log::info!("Started relay rewards system");
        Ok(())
    }
}

impl BandwidthMonitor {
    fn new(config: GatewayConfig) -> Self {
        Self {
            local_usage: Arc::new(Mutex::new(0.0)),
            internet_usage: Arc::new(Mutex::new(0.0)),
            usage_history: Arc::new(RwLock::new(VecDeque::new())),
            limits: config,
            alert_threshold: 0.8, // Alert at 80% of limit
        }
    }

    /// Get current bandwidth usage
    pub async fn get_usage(&self) -> BandwidthUsage {
        let local_usage = *self.local_usage.lock().await;
        let internet_usage = *self.internet_usage.lock().await;

        let total_usage = local_usage + internet_usage;
        let total_limit = self.limits.local_interface.max_bandwidth_mbps +
                         self.limits.internet_interface.max_bandwidth_mbps;

        BandwidthUsage {
            local_mbps: local_usage,
            internet_mbps: internet_usage,
            local_limit_mbps: self.limits.local_interface.max_bandwidth_mbps,
            internet_limit_mbps: self.limits.internet_interface.max_bandwidth_mbps,
            utilization_percentage: if total_limit > 0.0 {
                (total_usage / total_limit) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Update bandwidth usage
    pub async fn update_usage(&self, is_local: bool, bytes: usize) {
        let mbps = (bytes as f64 * 8.0) / 1_000_000.0;

        if is_local {
            let mut usage = self.local_usage.lock().await;
            *usage = (*usage * 0.9) + (mbps * 0.1); // Exponential moving average
        } else {
            let mut usage = self.internet_usage.lock().await;
            *usage = (*usage * 0.9) + (mbps * 0.1);
        }

        // Add sample to history
        let mut history = self.usage_history.write().await;
        let sample = BandwidthSample {
            timestamp: Instant::now(),
            local_mbps: *self.local_usage.lock().await,
            internet_mbps: *self.internet_usage.lock().await,
            total_connections: 0, // Would be updated with actual connection count
        };

        history.push_back(sample);

        // Keep only last hour of samples (assuming 1 sample per second)
        if history.len() > 3600 {
            history.pop_front();
        }
    }

    /// Check if bandwidth limit is reached
    pub async fn is_limit_reached(&self, is_local: bool) -> bool {
        if is_local {
            let usage = *self.local_usage.lock().await;
            usage >= self.limits.local_interface.max_bandwidth_mbps
        } else {
            let usage = *self.internet_usage.lock().await;
            usage >= self.limits.internet_interface.max_bandwidth_mbps
        }
    }
}

impl ConnectionManager {
    fn new(config: GatewayConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_pool: Arc::new(RwLock::new(ConnectionPool {
                tcp_pool: HashMap::new(),
                udp_pool: HashMap::new(),
                max_pool_size: 10,
                cleanup_interval: Duration::from_secs(60),
            })),
            rate_limiter: Arc::new(RateLimiter {
                limits: Arc::new(RwLock::new(HashMap::new())),
                global_limit: Arc::new(Mutex::new(TokenBucket {
                    tokens: config.security.rate_limiting.burst_size as f64,
                    capacity: config.security.rate_limiting.burst_size as f64,
                    refill_rate: config.security.rate_limiting.requests_per_second as f64,
                    last_refill: Instant::now(),
                })),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{BitchatKeypair, BitchatIdentity};
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_gateway_node_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh = Arc::new(MeshService::new(identity.clone(), transport));
        let config = GatewayConfig::default();

        let gateway = GatewayNode::new(identity, config, mesh).unwrap();

        let stats = gateway.get_stats().await;
        assert_eq!(stats.local_peers, 0);
        assert_eq!(stats.internet_peers, 0);
        assert_eq!(stats.known_gateways, 0);
    }

    #[tokio::test]
    async fn test_bandwidth_monitor() {
        let config = GatewayConfig::default();
        let monitor = BandwidthMonitor::new(config);

        // Test initial state
        let usage = monitor.get_usage().await;
        assert_eq!(usage.local_mbps, 0.0);
        assert_eq!(usage.internet_mbps, 0.0);

        // Test usage update
        monitor.update_usage(true, 1_000_000).await; // 1MB = 8Mbps instantaneous

        let usage = monitor.get_usage().await;
        assert!(usage.local_mbps > 0.0);
        assert!(usage.local_mbps < 8.0); // Should be less due to moving average
    }
}