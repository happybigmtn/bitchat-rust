//! Gateway node implementation for bridging local mesh networks to the internet
//!
//! Gateway nodes provide internet connectivity for isolated mesh networks,
//! enabling global BitCraps gameplay while maintaining local mesh efficiency.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;

use crate::crypto::BitchatIdentity;
use crate::error::{Error, Result};
use crate::mesh::MeshService;
use crate::protocol::versioning::ProtocolVersion;
use crate::protocol::PeerId;
use crate::utils::GrowableBuffer;

/// Gateway node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Local mesh interface (Bluetooth, WiFi Direct)
    pub local_interface: GatewayInterface,
    /// Internet interface (TCP, UDP)
    pub internet_interface: GatewayInterface,
    /// Maximum peers per gateway
    pub max_peers: usize,
    /// Gateway discovery interval
    pub discovery_interval: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Enable gateway relay rewards
    pub enable_relay_rewards: bool,
    /// Relay fee per message (in CrapTokens)
    pub relay_fee: u64,
}

/// Gateway interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayInterface {
    pub bind_address: SocketAddr,
    pub protocol: GatewayProtocol,
    pub max_bandwidth_mbps: f64,
    pub max_connections: usize,
}

/// Gateway protocol types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GatewayProtocol {
    Tcp,
    Udp,
    WebSocket,
    Quic,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            local_interface: GatewayInterface {
                bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8333),
                protocol: GatewayProtocol::Tcp,
                max_bandwidth_mbps: 10.0,
                max_connections: 50,
            },
            internet_interface: GatewayInterface {
                bind_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8334),
                protocol: GatewayProtocol::Tcp,
                max_bandwidth_mbps: 100.0,
                max_connections: 500,
            },
            max_peers: 1000,
            discovery_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            connection_timeout: Duration::from_secs(30),
            enable_relay_rewards: true,
            relay_fee: 1, // 1 CrapToken per relay
        }
    }
}

/// Gateway node that bridges local mesh to internet
#[allow(dead_code)]
pub struct GatewayNode {
    identity: Arc<BitchatIdentity>,
    config: GatewayConfig,
    mesh_service: Arc<MeshService>,
    local_peers: Arc<RwLock<HashMap<PeerId, LocalPeer>>>,
    internet_peers: Arc<RwLock<HashMap<PeerId, InternetPeer>>>,
    gateway_registry: Arc<RwLock<HashMap<PeerId, GatewayInfo>>>,
    routing_table: Arc<RwLock<HashMap<PeerId, GatewayRoute>>>,
    bandwidth_monitor: Arc<BandwidthMonitor>,
    relay_stats: Arc<RwLock<RelayStatistics>>,
    event_sender: mpsc::Sender<GatewayEvent>,
    is_running: Arc<RwLock<bool>>,
}

/// Local mesh peer information
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct LocalPeer {
    peer_id: PeerId,
    connected_at: Instant,
    last_seen: Instant,
    bytes_relayed: u64,
    messages_relayed: u64,
    reputation: f64,
    address: Option<SocketAddr>,
    last_activity: Instant,
    bytes_sent: u64,
    bytes_received: u64,
}

/// Internet peer information
#[derive(Debug, Clone)]
struct InternetPeer {
    peer_id: PeerId,
    address: SocketAddr,
    connected_at: Instant,
    last_ping: Instant,
    rtt: Duration,
    bandwidth_usage: f64,
    protocol_version: ProtocolVersion,
    last_activity: Instant,
    bytes_relayed: u64,
    relay_fee_earned: u64,
}

/// Gateway information from registry
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
}

/// Gateway routing information
#[derive(Debug, Clone)]
struct GatewayRoute {
    destination: PeerId,
    via_gateway: PeerId,
    hop_count: u8,
    bandwidth_cost: f64,
    latency_ms: u32,
    reliability: f64,
    last_updated: Instant,
}

/// Bandwidth monitoring for QoS
pub struct BandwidthMonitor {
    local_usage: Arc<Mutex<f64>>,    // Mbps
    internet_usage: Arc<Mutex<f64>>, // Mbps
    usage_history: Arc<RwLock<Vec<(Instant, f64, f64)>>>,
    limits: GatewayConfig,
}

/// Relay statistics for mining rewards
#[derive(Debug, Default, Clone)]
struct RelayStatistics {
    messages_relayed: u64,
    bytes_relayed: u64,
    tokens_earned: u64,
    uptime_start: Option<Instant>,
    last_reward_claim: Option<Instant>,
}

/// Gateway events
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    LocalPeerConnected {
        peer_id: PeerId,
    },
    LocalPeerDisconnected {
        peer_id: PeerId,
        reason: String,
    },
    InternetPeerConnected {
        peer_id: PeerId,
        address: SocketAddr,
    },
    InternetPeerDisconnected {
        peer_id: PeerId,
        reason: String,
    },
    PeerDisconnected {
        peer_id: PeerId,
        reason: String,
    },
    MessageRelayed {
        from: PeerId,
        to: PeerId,
        bytes: usize,
    },
    BandwidthLimitReached {
        interface: String,
        limit_mbps: f64,
    },
    GatewayDiscovered {
        gateway: GatewayInfo,
    },
    RelayRewardEarned {
        tokens: u64,
    },
}

impl GatewayNode {
    /// Create new gateway node
    pub fn new(
        identity: Arc<BitchatIdentity>,
        config: GatewayConfig,
        mesh_service: Arc<MeshService>,
    ) -> Self {
        let (event_sender, _) = mpsc::channel(1000); // Moderate traffic for gateway events

        Self {
            identity,
            mesh_service,
            local_peers: Arc::new(RwLock::new(HashMap::new())),
            internet_peers: Arc::new(RwLock::new(HashMap::new())),
            gateway_registry: Arc::new(RwLock::new(HashMap::new())),
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            bandwidth_monitor: Arc::new(BandwidthMonitor::new(config.clone())),
            relay_stats: Arc::new(RwLock::new(RelayStatistics::default())),
            event_sender,
            is_running: Arc::new(RwLock::new(false)),
            config,
        }
    }

    /// Start gateway node
    pub async fn start(&self) -> Result<()> {
        *self.is_running.write().await = true;

        // Start local interface (mesh)
        self.start_local_interface().await?;

        // Start internet interface
        self.start_internet_interface().await?;

        // Start gateway discovery
        self.start_gateway_discovery().await;

        // Start heartbeat service
        self.start_heartbeat_service().await;

        // Start bandwidth monitoring
        self.start_bandwidth_monitoring().await;

        // Start relay reward system
        if self.config.enable_relay_rewards {
            self.start_relay_rewards().await;
        }

        log::info!(
            "Gateway node started at local:{} internet:{}",
            self.config.local_interface.bind_address,
            self.config.internet_interface.bind_address
        );

        Ok(())
    }

    /// Stop gateway node
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        log::info!("Gateway node stopped");
    }

    /// Start local interface (mesh side)
    async fn start_local_interface(&self) -> Result<()> {
        match self.config.local_interface.protocol {
            GatewayProtocol::Tcp => {
                self.start_tcp_server(self.config.local_interface.bind_address, true)
                    .await
            }
            GatewayProtocol::Udp => {
                self.start_udp_server(self.config.local_interface.bind_address, true)
                    .await
            }
            _ => Err(Error::Transport(
                "Protocol not supported for local interface".to_string(),
            )),
        }
    }

    /// Start internet interface
    async fn start_internet_interface(&self) -> Result<()> {
        match self.config.internet_interface.protocol {
            GatewayProtocol::Tcp => {
                self.start_tcp_server(self.config.internet_interface.bind_address, false)
                    .await
            }
            GatewayProtocol::Udp => {
                self.start_udp_server(self.config.internet_interface.bind_address, false)
                    .await
            }
            GatewayProtocol::WebSocket => self.start_websocket_server().await,
            GatewayProtocol::Quic => self.start_quic_server().await,
        }
    }

    /// Start TCP server
    async fn start_tcp_server(&self, addr: SocketAddr, is_local: bool) -> Result<()> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Transport(format!("Failed to bind TCP listener: {}", e)))?;

        let local_peers = self.local_peers.clone();
        let internet_peers = self.internet_peers.clone();
        let identity = self.identity.clone();
        let event_sender = self.event_sender.clone();
        let is_running = self.is_running.clone();
        let bandwidth_monitor = self.bandwidth_monitor.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        log::info!("New TCP connection from {}", peer_addr);

                        // Handle connection in separate task
                        let local_peers_task = local_peers.clone();
                        let internet_peers_task = internet_peers.clone();
                        let identity_task = identity.clone();
                        let event_sender_task = event_sender.clone();
                        let bandwidth_monitor_task = bandwidth_monitor.clone();

                        tokio::spawn(async move {
                            Self::handle_tcp_connection(
                                stream,
                                peer_addr,
                                is_local,
                                local_peers_task,
                                internet_peers_task,
                                identity_task,
                                event_sender_task,
                                bandwidth_monitor_task,
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to accept TCP connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start UDP server
    async fn start_udp_server(&self, addr: SocketAddr, _is_local: bool) -> Result<()> {
        let socket = UdpSocket::bind(addr)
            .await
            .map_err(|e| Error::Transport(format!("Failed to bind UDP socket: {}", e)))?;

        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 65536];

            while *is_running.read().await {
                match socket.recv_from(&mut buffer).await {
                    Ok((len, peer_addr)) => {
                        log::debug!("Received {} bytes from {}", len, peer_addr);
                        // Handle UDP message
                    }
                    Err(e) => {
                        log::error!("UDP receive error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start WebSocket server (placeholder)
    async fn start_websocket_server(&self) -> Result<()> {
        // WebSocket implementation would go here
        log::warn!("WebSocket server not yet implemented");
        Ok(())
    }

    /// Start QUIC server (placeholder)
    async fn start_quic_server(&self) -> Result<()> {
        // QUIC implementation would go here
        log::warn!("QUIC server not yet implemented");
        Ok(())
    }

    /// Handle TCP connection
    async fn handle_tcp_connection(
        mut stream: TcpStream,
        peer_addr: SocketAddr,
        is_local: bool,
        local_peers: Arc<RwLock<HashMap<PeerId, LocalPeer>>>,
        internet_peers: Arc<RwLock<HashMap<PeerId, InternetPeer>>>,
        identity: Arc<BitchatIdentity>,
        event_sender: mpsc::Sender<GatewayEvent>,
        bandwidth_monitor: Arc<BandwidthMonitor>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Generate temporary peer ID from address
        let peer_id = Self::peer_id_from_addr(peer_addr);

        // Perform handshake
        let handshake_data = identity.public_key().to_vec();
        if let Err(e) = stream.write_all(&handshake_data).await {
            log::error!("Failed to send handshake: {}", e);
            return;
        }

        // Read peer handshake
        let mut peer_handshake = vec![0u8; 32];
        match stream.read_exact(&mut peer_handshake).await {
            Ok(_) => {
                // Store peer information
                if is_local {
                    let mut peers = local_peers.write().await;
                    peers.insert(
                        peer_id,
                        LocalPeer {
                            peer_id,
                            address: Some(peer_addr),
                            connected_at: Instant::now(),
                            last_activity: Instant::now(),
                            last_seen: Instant::now(),
                            bytes_sent: 0,
                            bytes_received: 0,
                            bytes_relayed: 0,
                            messages_relayed: 0,
                            reputation: 1.0,
                        },
                    );
                    let _ = event_sender.send(GatewayEvent::LocalPeerConnected { peer_id });
                } else {
                    let mut peers = internet_peers.write().await;
                    peers.insert(
                        peer_id,
                        InternetPeer {
                            peer_id,
                            address: peer_addr,
                            connected_at: Instant::now(),
                            last_activity: Instant::now(),
                            last_ping: Instant::now(),
                            rtt: Duration::from_millis(0),
                            bandwidth_usage: 0.0,
                            protocol_version: ProtocolVersion::CURRENT,
                            bytes_relayed: 0,
                            relay_fee_earned: 0,
                        },
                    );
                    let _ = event_sender.send(GatewayEvent::InternetPeerConnected {
                        peer_id,
                        address: peer_addr,
                    });
                }

                log::info!(
                    "Successfully connected to peer {} at {} (local: {})",
                    hex::encode(&peer_id[..8]),
                    peer_addr,
                    is_local
                );

                // Start message relay loop
                let mut buffer = GrowableBuffer::new();
                loop {
                    let buffer_slice = match buffer.get_mut(GrowableBuffer::MTU_SIZE) {
                        Ok(slice) => slice,
                        Err(e) => {
                            log::error!("Failed to get buffer: {}", e);
                            break;
                        }
                    };
                    match stream.read(buffer_slice).await {
                        Ok(0) => {
                            // Connection closed
                            break;
                        }
                        Ok(n) => {
                            // Mark buffer usage for memory optimization
                            buffer.mark_used(n);

                            // Update bandwidth monitoring
                            bandwidth_monitor.update_usage(is_local, n).await;

                            // Relay message to appropriate destination
                            if is_local {
                                // Local to internet relay
                                if let Some(peers) = local_peers.write().await.get_mut(&peer_id) {
                                    peers.bytes_received += n as u64;
                                    peers.last_activity = Instant::now();
                                }
                            } else {
                                // Internet to local relay
                                if let Some(peers) = internet_peers.write().await.get_mut(&peer_id)
                                {
                                    peers.bytes_relayed += n as u64;
                                    peers.last_activity = Instant::now();
                                    peers.relay_fee_earned += 1; // Simple fee calculation
                                }
                            }

                            // Process message (would include actual routing logic)
                            log::trace!(
                                "Relayed {} bytes from peer {}",
                                n,
                                hex::encode(&peer_id[..8])
                            );
                        }
                        Err(e) => {
                            log::error!(
                                "Read error from peer {}: {}",
                                hex::encode(&peer_id[..8]),
                                e
                            );
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to read peer handshake: {}", e);
            }
        }

        // Clean up on disconnect
        if is_local {
            local_peers.write().await.remove(&peer_id);
            let _ = event_sender.send(GatewayEvent::LocalPeerDisconnected {
                peer_id,
                reason: "Connection closed".to_string(),
            });
        } else {
            internet_peers.write().await.remove(&peer_id);
            let _ = event_sender.send(GatewayEvent::InternetPeerDisconnected {
                peer_id,
                reason: "Connection closed".to_string(),
            });
        }

        log::info!("Peer {} disconnected", hex::encode(&peer_id[..8]));
    }

    /// Generate peer ID from socket address (temporary solution)
    fn peer_id_from_addr(addr: SocketAddr) -> PeerId {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(addr.to_string().as_bytes());
        let hash = hasher.finalize();

        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(&hash);
        peer_id
    }

    /// Start gateway discovery
    async fn start_gateway_discovery(&self) {
        let gateway_registry = self.gateway_registry.clone();
        let is_running = self.is_running.clone();
        let discovery_interval = self.config.discovery_interval;

        tokio::spawn(async move {
            let mut interval = interval(discovery_interval);

            while *is_running.read().await {
                interval.tick().await;

                // Discovery logic - broadcast gateway announcement
                log::debug!("Running gateway discovery");

                // In a real implementation, this would:
                // 1. Broadcast gateway announcement to known peers
                // 2. Listen for other gateway announcements
                // 3. Update gateway registry
                // 4. Calculate optimal routing paths

                let registry = gateway_registry.read().await;
                log::debug!("Known gateways: {}", registry.len());
            }
        });
    }

    /// Start heartbeat service
    async fn start_heartbeat_service(&self) {
        let local_peers = self.local_peers.clone();
        let internet_peers = self.internet_peers.clone();
        let is_running = self.is_running.clone();
        let heartbeat_interval = self.config.heartbeat_interval;

        tokio::spawn(async move {
            let mut interval = interval(heartbeat_interval);

            while *is_running.read().await {
                interval.tick().await;

                // Send heartbeats to all peers
                let local_count = local_peers.read().await.len();
                let internet_count = internet_peers.read().await.len();

                log::debug!(
                    "Heartbeat: {} local peers, {} internet peers",
                    local_count,
                    internet_count
                );
            }
        });
    }

    /// Start bandwidth monitoring
    async fn start_bandwidth_monitoring(&self) {
        let bandwidth_monitor = self.bandwidth_monitor.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));

            while *is_running.read().await {
                interval.tick().await;

                // Update bandwidth usage statistics
                let local_usage = *bandwidth_monitor.local_usage.lock().await;
                let internet_usage = *bandwidth_monitor.internet_usage.lock().await;

                // Add to history
                {
                    let mut history = bandwidth_monitor.usage_history.write().await;
                    history.push((Instant::now(), local_usage, internet_usage));

                    // Keep only last 3600 entries (1 hour at 1 second intervals)
                    if history.len() > 3600 {
                        let drain_count = history.len() - 3600;
                        history.drain(0..drain_count);
                    }
                }

                log::trace!(
                    "Bandwidth usage: local={:.2}Mbps, internet={:.2}Mbps",
                    local_usage,
                    internet_usage
                );
            }
        });
    }

    /// Start relay reward system
    async fn start_relay_rewards(&self) {
        let relay_stats = self.relay_stats.clone();
        let is_running = self.is_running.clone();
        let event_sender = self.event_sender.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Check every minute

            while *is_running.read().await {
                interval.tick().await;

                // Calculate relay rewards
                let mut stats = relay_stats.write().await;

                // Simple reward calculation: 1 token per message relayed
                let tokens_earned = stats.messages_relayed - stats.tokens_earned;

                if tokens_earned > 0 {
                    stats.tokens_earned += tokens_earned;
                    let _ = event_sender.send(GatewayEvent::RelayRewardEarned {
                        tokens: tokens_earned,
                    });

                    log::info!("Relay rewards earned: {} tokens", tokens_earned);
                }
            }
        });
    }

    /// Get gateway statistics
    pub async fn get_stats(&self) -> GatewayStats {
        let local_peers = self.local_peers.read().await;
        let internet_peers = self.internet_peers.read().await;
        let gateway_registry = self.gateway_registry.read().await;
        let relay_stats = self.relay_stats.read().await;

        let bandwidth_usage = {
            let local_usage = *self.bandwidth_monitor.local_usage.lock().await;
            let internet_usage = *self.bandwidth_monitor.internet_usage.lock().await;
            BandwidthUsage {
                local_mbps: local_usage,
                internet_mbps: internet_usage,
                local_limit_mbps: self.config.local_interface.max_bandwidth_mbps,
                internet_limit_mbps: self.config.internet_interface.max_bandwidth_mbps,
            }
        };

        GatewayStats {
            local_peers: local_peers.len(),
            internet_peers: internet_peers.len(),
            known_gateways: gateway_registry.len(),
            messages_relayed: relay_stats.messages_relayed,
            bytes_relayed: relay_stats.bytes_relayed,
            tokens_earned: relay_stats.tokens_earned,
            bandwidth_usage,
            uptime: relay_stats
                .uptime_start
                .map(|start| start.elapsed())
                .unwrap_or(Duration::ZERO),
        }
    }
}

impl BandwidthMonitor {
    fn new(config: GatewayConfig) -> Self {
        Self {
            local_usage: Arc::new(Mutex::new(0.0)),
            internet_usage: Arc::new(Mutex::new(0.0)),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            limits: config,
        }
    }

    /// Update bandwidth usage
    pub async fn update_usage(&self, is_local: bool, bytes: usize) {
        let mbps = (bytes as f64 * 8.0) / 1_000_000.0; // Convert to Mbps

        if is_local {
            let mut usage = self.local_usage.lock().await;
            *usage = (*usage * 0.9) + (mbps * 0.1); // Exponential moving average
        } else {
            let mut usage = self.internet_usage.lock().await;
            *usage = (*usage * 0.9) + (mbps * 0.1);
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

/// Gateway statistics
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
}

/// Bandwidth usage information
#[derive(Debug, Clone)]
pub struct BandwidthUsage {
    pub local_mbps: f64,
    pub internet_mbps: f64,
    pub local_limit_mbps: f64,
    pub internet_limit_mbps: f64,
}

/// Gateway discovery service for finding optimal gateways
pub struct GatewayDiscovery {
    known_gateways: Arc<RwLock<HashMap<PeerId, GatewayInfo>>>,
    discovery_peers: HashSet<PeerId>,
}

impl Default for GatewayDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayDiscovery {
    pub fn new() -> Self {
        Self {
            known_gateways: Arc::new(RwLock::new(HashMap::new())),
            discovery_peers: HashSet::new(),
        }
    }

    /// Discover gateways in the network
    pub async fn discover_gateways(&mut self) -> Result<Vec<GatewayInfo>> {
        // Implementation would broadcast discovery messages
        // and collect responses from gateway nodes

        let gateways = self.known_gateways.read().await;
        Ok(gateways.values().cloned().collect())
    }

    /// Select best gateway based on criteria
    pub async fn select_best_gateway(
        &self,
        criteria: GatewaySelectionCriteria,
    ) -> Option<GatewayInfo> {
        let gateways = self.known_gateways.read().await;

        let mut candidates: Vec<_> = gateways.values().collect();

        // Filter by criteria
        candidates.retain(|gw| {
            gw.current_load < criteria.max_load
                && gw.uptime_percentage > criteria.min_uptime
                && gw.relay_fee <= criteria.max_relay_fee
        });

        // Sort by preference
        match criteria.preference {
            GatewayPreference::LowestLatency => {
                // Would need latency measurements
                candidates.first().cloned().cloned()
            }
            GatewayPreference::LowestCost => {
                candidates.sort_by_key(|gw| gw.relay_fee);
                candidates.first().cloned().cloned()
            }
            GatewayPreference::HighestBandwidth => {
                candidates.sort_by(|a, b| {
                    b.max_bandwidth_mbps
                        .partial_cmp(&a.max_bandwidth_mbps)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                candidates.first().cloned().cloned()
            }
            GatewayPreference::MostReliable => {
                candidates.sort_by(|a, b| {
                    b.uptime_percentage
                        .partial_cmp(&a.uptime_percentage)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                candidates.first().cloned().cloned()
            }
        }
    }
}

/// Gateway selection criteria
#[derive(Debug, Clone)]
pub struct GatewaySelectionCriteria {
    pub max_load: f64,
    pub min_uptime: f64,
    pub max_relay_fee: u64,
    pub preference: GatewayPreference,
}

/// Gateway selection preferences
#[derive(Debug, Clone, Copy)]
pub enum GatewayPreference {
    LowestLatency,
    LowestCost,
    HighestBandwidth,
    MostReliable,
}

impl Default for GatewaySelectionCriteria {
    fn default() -> Self {
        Self {
            max_load: 0.8,     // 80% maximum load
            min_uptime: 0.95,  // 95% minimum uptime
            max_relay_fee: 10, // Maximum 10 tokens per relay
            preference: GatewayPreference::MostReliable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{BitchatIdentity, BitchatKeypair};
    use crate::mesh::MeshService;
    use crate::transport::TransportCoordinator;

    #[tokio::test]
    async fn test_gateway_node_creation() {
        let keypair = BitchatKeypair::generate();
        let identity = Arc::new(BitchatIdentity::from_keypair_with_pow(keypair, 8));
        let transport = Arc::new(TransportCoordinator::new());
        let mesh = Arc::new(MeshService::new(identity.clone(), transport));
        let config = GatewayConfig::default();

        let gateway = GatewayNode::new(identity, config, mesh);

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
        assert!(!monitor.is_limit_reached(true).await);
        assert!(!monitor.is_limit_reached(false).await);

        // Update usage
        monitor.update_usage(true, 1_000_000).await; // 1MB = 8Mbps instantaneous

        // Should still be under limit due to moving average
        assert!(!monitor.is_limit_reached(true).await);
    }

    #[test]
    fn test_gateway_info_serialization() {
        let gateway_info = GatewayInfo {
            peer_id: [0u8; 32],
            address: "127.0.0.1:8333".parse().expect("Valid test address"),
            protocol: GatewayProtocol::Tcp,
            max_bandwidth_mbps: 100.0,
            current_load: 0.5,
            relay_fee: 5,
            uptime_percentage: 0.99,
            protocol_version: ProtocolVersion::CURRENT,
            last_seen: SystemTime::now(),
        };

        // Test serialization/deserialization
        let serialized = serde_json::to_string(&gateway_info).expect("Serialization should work");
        let deserialized: GatewayInfo =
            serde_json::from_str(&serialized).expect("Deserialization should work");

        assert_eq!(gateway_info.peer_id, deserialized.peer_id);
        assert_eq!(
            gateway_info.max_bandwidth_mbps,
            deserialized.max_bandwidth_mbps
        );
    }
}
