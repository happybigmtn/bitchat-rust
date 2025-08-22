# Week 7: Network Coordinator and Peer Discovery

## Overview

**Feynman Explanation**: Week 7 is about building the "casino district manager" - the system that helps all the individual casinos find each other and stay connected. Imagine trying to organize thousands of casinos spread across the world, where casinos can appear and disappear at any time. The network coordinator is like a combination of a phone directory, a GPS system, and a neighborhood watch that helps casinos discover nearby peers, maintain connections, and route messages efficiently through the mesh network.

This week implements the critical infrastructure that transforms isolated nodes into a cohesive, self-organizing network capable of discovering peers, maintaining connections, and ensuring messages reach their destinations even when direct paths don't exist.

---

## Day 1: Bluetooth Mesh Discovery

### Goals
- Implement BLE advertisement-based discovery
- Create proximity-based peer detection
- Build automatic connection management
- Handle connection handoffs between devices

### Bluetooth Discovery Implementation

```rust
// src/discovery/bluetooth_discovery.rs
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};

use crate::protocol::PeerId;
use crate::crypto::BitchatIdentity;

/// Bluetooth mesh discovery service
/// 
/// Feynman: This is like having a radar that constantly scans for
/// other casinos. When your phone's Bluetooth sees another phone
/// running BitCraps, they automatically shake hands and exchange
/// business cards (peer IDs). It's completely automatic - you just
/// walk near someone and your casinos connect.
pub struct BluetoothDiscovery {
    identity: Arc<BitchatIdentity>,
    adapter: Arc<Adapter>,
    discovered_peers: Arc<RwLock<HashMap<PeerId, DiscoveredPeer>>>,
    active_connections: Arc<RwLock<HashSet<PeerId>>>,
    discovery_events: mpsc::UnboundedSender<DiscoveryEvent>,
    scan_interval: Duration,
    connection_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub device_address: String,
    pub rssi: i16, // Signal strength
    pub distance_estimate: f32, // Estimated distance in meters
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub connection_attempts: u32,
    pub is_connected: bool,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerDiscovered { peer: DiscoveredPeer },
    PeerConnected { peer_id: PeerId },
    PeerDisconnected { peer_id: PeerId },
    PeerRangeChanged { peer_id: PeerId, distance: f32 },
    DiscoveryError { error: String },
}

impl BluetoothDiscovery {
    pub async fn new(
        identity: Arc<BitchatIdentity>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next()
            .ok_or("No Bluetooth adapter found")?;
        
        let (discovery_events, _) = mpsc::unbounded_channel();
        
        Ok(Self {
            identity,
            adapter: Arc::new(adapter),
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(RwLock::new(HashSet::new())),
            discovery_events,
            scan_interval: Duration::from_secs(5),
            connection_timeout: Duration::from_secs(30),
        })
    }
    
    /// Start discovery process
    /// 
    /// Feynman: Like turning on a lighthouse that both shines its light
    /// (advertising) and looks for other lights (scanning). Every few
    /// seconds, we sweep the area looking for new casinos and telling
    /// others we're here.
    pub async fn start_discovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Start advertising our presence
        self.start_advertising().await?;
        
        // Start scanning for peers
        self.start_scanning().await?;
        
        // Start connection manager
        self.start_connection_manager().await?;
        
        Ok(())
    }
    
    /// Advertise our presence via BLE
    async fn start_advertising(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create BitCraps service advertisement
        let service_data = self.create_service_advertisement();
        
        // In production, would use platform-specific BLE advertising APIs
        // This is simplified for illustration
        
        println!("Starting BitCraps BLE advertisement");
        println!("Service UUID: {}", BITCRAPS_SERVICE_UUID);
        println!("Peer ID: {:?}", self.identity.peer_id);
        
        Ok(())
    }
    
    /// Scan for other BitCraps nodes
    async fn start_scanning(&self) -> Result<(), Box<dyn std::error::Error>> {
        let adapter = self.adapter.clone();
        let discovered_peers = self.discovered_peers.clone();
        let discovery_events = self.discovery_events.clone();
        
        tokio::spawn(async move {
            let mut scan_interval = interval(Duration::from_secs(5));
            
            loop {
                scan_interval.tick().await;
                
                // Start scan with filter for BitCraps service
                if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
                    eprintln!("Scan error: {}", e);
                    continue;
                }
                
                // Scan for 4 seconds
                tokio::time::sleep(Duration::from_secs(4)).await;
                
                // Get discovered peripherals
                let peripherals = adapter.peripherals().await.unwrap_or_default();
                
                for peripheral in peripherals {
                    // Check if this is a BitCraps node
                    if let Ok(properties) = peripheral.properties().await {
                        if let Some(properties) = properties {
                            // Parse advertisement data
                            if Self::is_bitcraps_device(&properties.local_name) {
                                let peer_id = Self::extract_peer_id(&properties.manufacturer_data);
                                let rssi = properties.rssi.unwrap_or(-100);
                                let distance = Self::estimate_distance(rssi);
                                
                                let discovered_peer = DiscoveredPeer {
                                    peer_id,
                                    device_address: properties.address.to_string(),
                                    rssi,
                                    distance_estimate: distance,
                                    first_seen: Instant::now(),
                                    last_seen: Instant::now(),
                                    connection_attempts: 0,
                                    is_connected: false,
                                };
                                
                                // Update or insert peer
                                let mut peers = discovered_peers.write().await;
                                let is_new = !peers.contains_key(&peer_id);
                                peers.insert(peer_id, discovered_peer.clone());
                                
                                if is_new {
                                    discovery_events.send(DiscoveryEvent::PeerDiscovered {
                                        peer: discovered_peer,
                                    }).ok();
                                }
                            }
                        }
                    }
                }
                
                // Stop scan
                let _ = adapter.stop_scan().await;
            }
        });
        
        Ok(())
    }
    
    /// Manage connections to discovered peers
    async fn start_connection_manager(&self) -> Result<(), Box<dyn std::error::Error>> {
        let discovered_peers = self.discovered_peers.clone();
        let active_connections = self.active_connections.clone();
        let discovery_events = self.discovery_events.clone();
        let adapter = self.adapter.clone();
        
        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(10));
            
            loop {
                check_interval.tick().await;
                
                let peers = discovered_peers.read().await;
                let connections = active_connections.read().await;
                
                // Try to connect to nearby unconnected peers
                for (peer_id, peer) in peers.iter() {
                    if !peer.is_connected && !connections.contains(peer_id) {
                        // Only connect if close enough (within 10 meters)
                        if peer.distance_estimate < 10.0 {
                            println!("Attempting to connect to peer {:?} at ~{:.1}m", 
                                     peer_id, peer.distance_estimate);
                            
                            // Attempt connection
                            // In production, would implement actual BLE connection
                            
                            discovery_events.send(DiscoveryEvent::PeerConnected {
                                peer_id: *peer_id,
                            }).ok();
                        }
                    }
                }
                
                // Check for disconnected peers (haven't seen in 30 seconds)
                let now = Instant::now();
                for (peer_id, peer) in peers.iter() {
                    if peer.is_connected && 
                       now.duration_since(peer.last_seen) > Duration::from_secs(30) {
                        discovery_events.send(DiscoveryEvent::PeerDisconnected {
                            peer_id: *peer_id,
                        }).ok();
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Create service advertisement data
    fn create_service_advertisement(&self) -> Vec<u8> {
        let mut data = Vec::new();
        
        // Service UUID (16 bytes)
        data.extend_from_slice(&BITCRAPS_SERVICE_UUID.as_bytes());
        
        // Peer ID (32 bytes)
        data.extend_from_slice(&self.identity.peer_id);
        
        // Protocol version (1 byte)
        data.push(PROTOCOL_VERSION);
        
        // Capabilities flags (1 byte)
        let mut flags = 0u8;
        flags |= 0x01; // Supports gaming
        flags |= 0x02; // Supports mesh routing
        flags |= 0x04; // Supports token transactions
        data.push(flags);
        
        data
    }
    
    /// Check if device name indicates BitCraps node
    fn is_bitcraps_device(name: &Option<String>) -> bool {
        if let Some(name) = name {
            name.starts_with("BitCraps") || name.contains("CRAP")
        } else {
            false
        }
    }
    
    /// Extract peer ID from manufacturer data
    fn extract_peer_id(manufacturer_data: &HashMap<u16, Vec<u8>>) -> PeerId {
        // Look for our manufacturer ID
        const BITCRAPS_MANUFACTURER_ID: u16 = 0xFFFF; // Would register real ID
        
        if let Some(data) = manufacturer_data.get(&BITCRAPS_MANUFACTURER_ID) {
            if data.len() >= 32 {
                let mut peer_id = [0u8; 32];
                peer_id.copy_from_slice(&data[0..32]);
                return peer_id;
            }
        }
        
        [0u8; 32] // Default
    }
    
    /// Estimate distance from RSSI using path loss model
    /// 
    /// Feynman: Radio signals get weaker with distance, like sound.
    /// If someone is shouting (strong signal), they're close. If they're
    /// whispering (weak signal), they're far. We use physics formulas to
    /// estimate how far based on how loud the "shout" is.
    fn estimate_distance(rssi: i16) -> f32 {
        // Path loss formula: RSSI = -10 * n * log10(d) + A
        // Where n = path loss exponent (2-4), A = RSSI at 1 meter
        const RSSI_AT_1M: f32 = -59.0; // Typical for BLE
        const PATH_LOSS_EXPONENT: f32 = 2.0;
        
        let distance = 10_f32.powf((RSSI_AT_1M - rssi as f32) / (10.0 * PATH_LOSS_EXPONENT));
        distance.max(0.1).min(100.0) // Clamp to reasonable range
    }
}
```

---

## Day 2: DHT-Based Peer Discovery

### Goals
- Implement Kademlia bootstrap nodes
- Create DHT crawler for peer discovery
- Build reputation-based peer selection
- Handle NAT traversal

### DHT Discovery Implementation

```rust
// src/discovery/dht_discovery.rs
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::protocol::PeerId;
use crate::mesh::kademlia::{KademliaRouter, NodeId};

/// DHT-based peer discovery
/// 
/// Feynman: This is like a distributed phone book where everyone
/// keeps a piece of the directory. To find someone, you ask your
/// neighbors, who ask their neighbors, until someone knows the answer.
/// It's impossible to destroy because there's no central directory.
pub struct DhtDiscovery {
    local_id: PeerId,
    kademlia: Arc<KademliaRouter>,
    bootstrap_nodes: Vec<SocketAddr>,
    discovered_peers: Arc<RwLock<HashMap<PeerId, DhtPeer>>>,
    crawl_queue: Arc<RwLock<VecDeque<PeerId>>>,
}

#[derive(Debug, Clone)]
pub struct DhtPeer {
    pub peer_id: PeerId,
    pub addresses: Vec<SocketAddr>,
    pub reputation: f64,
    pub last_seen: std::time::Instant,
    pub hop_distance: u32,
}

impl DhtDiscovery {
    pub fn new(
        local_id: PeerId,
        bootstrap_nodes: Vec<SocketAddr>,
    ) -> Self {
        Self {
            local_id,
            kademlia: Arc::new(KademliaRouter::new(local_id)),
            bootstrap_nodes,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            crawl_queue: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
    
    /// Bootstrap into the DHT network
    /// 
    /// Feynman: Like arriving in a new city and asking the first person
    /// you meet for directions. Bootstrap nodes are well-known meeting
    /// points where new nodes can join the network.
    pub async fn bootstrap(&self) -> Result<(), Box<dyn std::error::Error>> {
        for bootstrap_addr in &self.bootstrap_nodes {
            // Connect to bootstrap node
            println!("Connecting to bootstrap node: {}", bootstrap_addr);
            
            // Request initial peer list
            // In production, would implement actual protocol
        }
        
        // Start recursive crawl
        self.start_recursive_crawl().await?;
        
        Ok(())
    }
    
    /// Recursively crawl the DHT to discover peers
    async fn start_recursive_crawl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let kademlia = self.kademlia.clone();
        let discovered_peers = self.discovered_peers.clone();
        let crawl_queue = self.crawl_queue.clone();
        let local_id = self.local_id;
        
        tokio::spawn(async move {
            loop {
                // Get next peer to query
                let target = {
                    let mut queue = crawl_queue.write().await;
                    queue.pop_front()
                };
                
                if let Some(target) = target {
                    // Find K closest peers to target
                    let closest = kademlia.find_closest_peers(&target, 20);
                    
                    for peer_id in closest {
                        let mut peers = discovered_peers.write().await;
                        
                        if !peers.contains_key(&peer_id) {
                            // Calculate hop distance
                            let hop_distance = Self::calculate_hop_distance(&local_id, &peer_id);
                            
                            let dht_peer = DhtPeer {
                                peer_id,
                                addresses: Vec::new(), // Would be filled from response
                                reputation: 0.5, // Neutral starting reputation
                                last_seen: std::time::Instant::now(),
                                hop_distance,
                            };
                            
                            peers.insert(peer_id, dht_peer);
                            
                            // Add to crawl queue
                            crawl_queue.write().await.push_back(peer_id);
                        }
                    }
                } else {
                    // Queue empty, sleep and retry
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        });
        
        Ok(())
    }
    
    /// Calculate logical hop distance between peers
    fn calculate_hop_distance(local_id: &PeerId, target_id: &PeerId) -> u32 {
        // XOR distance gives us logical distance in the DHT
        let mut distance = 0u32;
        for i in 0..32 {
            distance += (local_id[i] ^ target_id[i]).count_ones();
        }
        distance
    }
}
```

---

## Day 3: Multi-Transport Coordination

### Goals
- Coordinate between Bluetooth, WiFi, and Internet transports
- Implement intelligent transport selection
- Handle transport failover
- Optimize for latency and bandwidth

### Transport Coordinator Implementation

```rust
// src/coordinator/transport_coordinator.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::transport::{Transport, TransportType, TransportMetrics};
use crate::protocol::{PeerId, BitchatPacket};

/// Multi-transport coordinator
/// 
/// Feynman: Like having multiple roads to the same destination -
/// highway (Internet), local roads (WiFi), and walking paths (Bluetooth).
/// The coordinator picks the best route based on traffic, distance,
/// and whether the road is even open.
pub struct TransportCoordinator {
    transports: Arc<RwLock<HashMap<TransportType, Box<dyn Transport>>>>,
    peer_transports: Arc<RwLock<HashMap<PeerId, Vec<TransportType>>>>,
    transport_metrics: Arc<RwLock<HashMap<TransportType, TransportMetrics>>>,
    failover_policy: FailoverPolicy,
}

#[derive(Debug, Clone, Copy)]
pub enum TransportType {
    Bluetooth,
    WiFiDirect,
    Internet,
    Mesh,
}

#[derive(Debug, Clone)]
pub struct TransportMetrics {
    pub latency_ms: f64,
    pub bandwidth_kbps: f64,
    pub packet_loss: f64,
    pub reliability_score: f64,
    pub last_updated: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum FailoverPolicy {
    FastestFirst,    // Use lowest latency
    MostReliable,    // Use most reliable
    LoadBalanced,    // Distribute across transports
    EnergyEfficient, // Prefer low-power transports
}

impl TransportCoordinator {
    pub fn new(failover_policy: FailoverPolicy) -> Self {
        Self {
            transports: Arc::new(RwLock::new(HashMap::new())),
            peer_transports: Arc::new(RwLock::new(HashMap::new())),
            transport_metrics: Arc::new(RwLock::new(HashMap::new())),
            failover_policy,
        }
    }
    
    /// Register a transport
    pub async fn register_transport(
        &self,
        transport_type: TransportType,
        transport: Box<dyn Transport>,
    ) {
        self.transports.write().await.insert(transport_type, transport);
        
        // Initialize metrics
        self.transport_metrics.write().await.insert(transport_type, TransportMetrics {
            latency_ms: 100.0,
            bandwidth_kbps: 1000.0,
            packet_loss: 0.0,
            reliability_score: 1.0,
            last_updated: std::time::Instant::now(),
        });
    }
    
    /// Send packet selecting best transport
    /// 
    /// Feynman: Like a smart GPS that knows traffic conditions -
    /// it picks the fastest route considering current conditions,
    /// not just distance.
    pub async fn send_packet(
        &self,
        peer_id: PeerId,
        packet: &BitchatPacket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get available transports for peer
        let available = self.get_available_transports(&peer_id).await;
        
        if available.is_empty() {
            return Err("No transport available for peer".into());
        }
        
        // Select best transport based on policy
        let selected = self.select_transport(&available, packet.data.len()).await?;
        
        // Try primary transport
        let transports = self.transports.read().await;
        if let Some(transport) = transports.get(&selected) {
            match transport.send(peer_id, packet.serialize().to_vec()).await {
                Ok(()) => {
                    self.update_success_metrics(selected).await;
                    return Ok(());
                }
                Err(e) => {
                    self.update_failure_metrics(selected).await;
                    eprintln!("Transport {} failed: {}", selected as u8, e);
                }
            }
        }
        
        // Failover to other transports
        for transport_type in available {
            if transport_type == selected {
                continue; // Already tried
            }
            
            if let Some(transport) = transports.get(&transport_type) {
                if transport.send(peer_id, packet.serialize().to_vec()).await.is_ok() {
                    self.update_success_metrics(transport_type).await;
                    return Ok(());
                }
            }
        }
        
        Err("All transports failed".into())
    }
    
    /// Get available transports for a peer
    async fn get_available_transports(&self, peer_id: &PeerId) -> Vec<TransportType> {
        let peer_transports = self.peer_transports.read().await;
        peer_transports.get(peer_id).cloned().unwrap_or_default()
    }
    
    /// Select best transport based on policy and metrics
    async fn select_transport(
        &self,
        available: &[TransportType],
        packet_size: usize,
    ) -> Result<TransportType, Box<dyn std::error::Error>> {
        let metrics = self.transport_metrics.read().await;
        
        match self.failover_policy {
            FailoverPolicy::FastestFirst => {
                // Select transport with lowest latency
                available.iter()
                    .min_by_key(|t| {
                        metrics.get(t)
                            .map(|m| m.latency_ms as u64)
                            .unwrap_or(u64::MAX)
                    })
                    .copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::MostReliable => {
                // Select transport with highest reliability
                available.iter()
                    .max_by_key(|t| {
                        metrics.get(t)
                            .map(|m| (m.reliability_score * 1000.0) as u64)
                            .unwrap_or(0)
                    })
                    .copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::LoadBalanced => {
                // Round-robin or weighted selection
                // For now, just pick first available
                available.first().copied()
                    .ok_or("No transport available".into())
            }
            FailoverPolicy::EnergyEfficient => {
                // Prefer Bluetooth for small packets, WiFi for medium, Internet for large
                if packet_size < 1000 {
                    available.iter()
                        .find(|&&t| t == TransportType::Bluetooth)
                        .or_else(|| available.first())
                        .copied()
                        .ok_or("No transport available".into())
                } else {
                    available.iter()
                        .find(|&&t| t == TransportType::WiFiDirect)
                        .or_else(|| available.first())
                        .copied()
                        .ok_or("No transport available".into())
                }
            }
        }
    }
    
    /// Update metrics after successful send
    async fn update_success_metrics(&self, transport_type: TransportType) {
        let mut metrics = self.transport_metrics.write().await;
        if let Some(m) = metrics.get_mut(&transport_type) {
            m.reliability_score = (m.reliability_score * 0.95) + 0.05; // Smooth increase
            m.last_updated = std::time::Instant::now();
        }
    }
    
    /// Update metrics after failed send
    async fn update_failure_metrics(&self, transport_type: TransportType) {
        let mut metrics = self.transport_metrics.write().await;
        if let Some(m) = metrics.get_mut(&transport_type) {
            m.reliability_score *= 0.9; // Decrease reliability
            m.packet_loss = (m.packet_loss * 0.9) + 0.1; // Increase loss estimate
            m.last_updated = std::time::Instant::now();
        }
    }
}
```

---

## Day 4: Network Health Monitoring

### Goals
- Implement network topology visualization
- Create health metrics collection
- Build anomaly detection
- Handle network partitions

### Network Monitor Implementation

```rust
// src/coordinator/network_monitor.rs
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Network health monitor
/// 
/// Feynman: Like having doctors constantly checking the pulse of
/// the network. They monitor vital signs (latency, connectivity),
/// diagnose problems (network splits), and prescribe treatments
/// (rerouting, reconnection).
pub struct NetworkMonitor {
    topology: Arc<RwLock<NetworkTopology>>,
    health_metrics: Arc<RwLock<HealthMetrics>>,
    anomaly_detector: Arc<AnomalyDetector>,
    alert_sender: mpsc::UnboundedSender<NetworkAlert>,
}

#[derive(Debug, Clone)]
pub struct NetworkTopology {
    pub nodes: HashMap<PeerId, NodeInfo>,
    pub edges: HashMap<(PeerId, PeerId), EdgeInfo>,
    pub clusters: Vec<HashSet<PeerId>>,
    pub bridge_nodes: HashSet<PeerId>,
}

#[derive(Debug, Clone)]
pub struct HealthMetrics {
    pub total_nodes: usize,
    pub active_connections: usize,
    pub average_latency: f64,
    pub network_diameter: u32,
    pub clustering_coefficient: f64,
    pub partition_risk: f64,
}

impl NetworkMonitor {
    pub async fn start_monitoring(&self) {
        // Start periodic health checks
        let topology = self.topology.clone();
        let health_metrics = self.health_metrics.clone();
        let anomaly_detector = self.anomaly_detector.clone();
        let alert_sender = self.alert_sender.clone();
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            
            loop {
                ticker.tick().await;
                
                // Calculate health metrics
                let metrics = Self::calculate_health_metrics(&topology).await;
                *health_metrics.write().await = metrics.clone();
                
                // Check for anomalies
                if let Some(anomaly) = anomaly_detector.check(&metrics).await {
                    alert_sender.send(NetworkAlert::AnomalyDetected(anomaly)).ok();
                }
                
                // Check for network partitions
                if metrics.partition_risk > 0.7 {
                    alert_sender.send(NetworkAlert::PartitionRisk).ok();
                }
            }
        });
    }
    
    async fn calculate_health_metrics(
        topology: &Arc<RwLock<NetworkTopology>>,
    ) -> HealthMetrics {
        let topo = topology.read().await;
        
        HealthMetrics {
            total_nodes: topo.nodes.len(),
            active_connections: topo.edges.len(),
            average_latency: Self::calculate_average_latency(&topo.edges),
            network_diameter: Self::calculate_diameter(&topo),
            clustering_coefficient: Self::calculate_clustering(&topo),
            partition_risk: Self::calculate_partition_risk(&topo),
        }
    }
}
```

---

## Summary

Week 7 delivers a complete network coordination layer with:

- **Bluetooth Discovery**: Automatic proximity-based peer detection
- **DHT Discovery**: Global peer discovery through Kademlia
- **Transport Coordination**: Intelligent multi-transport management
- **Network Monitoring**: Real-time health tracking and anomaly detection
- **Failover Handling**: Automatic transport switching on failure
- **Distance Estimation**: RSSI-based proximity calculation
- **Reputation System**: Trust-based peer selection
- **Partition Detection**: Network split identification and healing

The network coordinator ensures BitCraps nodes can find each other, maintain connections, and route messages efficiently through the mesh network, whether they're connected via Bluetooth, WiFi, or the Internet.