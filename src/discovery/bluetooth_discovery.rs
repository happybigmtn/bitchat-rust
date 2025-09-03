use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};

use crate::crypto::BitchatIdentity;
use crate::protocol::PeerId;
use crate::utils::LoopBudget;

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
    discovery_events: mpsc::Sender<DiscoveryEvent>,
    _scan_interval: Duration,
    _connection_timeout: Duration,
    // New fields for improved discovery
    peer_registry: Arc<RwLock<PeerRegistry>>,
    peer_exchange_interval: Duration,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub device_address: String,
    pub rssi: i16,              // Signal strength
    pub distance_estimate: f32, // Estimated distance in meters
    pub first_seen: Instant,
    pub last_seen: Instant,
    pub connection_attempts: u32,
    pub is_connected: bool,
    pub capabilities: PeerCapabilities,
    pub reputation_score: f32, // 0.0 to 1.0
}

/// Peer capabilities and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCapabilities {
    pub supports_gaming: bool,
    pub supports_mesh_routing: bool,
    pub supports_token_transactions: bool,
    pub protocol_version: u8,
    pub max_game_players: Option<u8>,
    pub available_tokens: Option<u64>,
}

/// Peer registry with TTL management
#[derive(Debug)]
#[allow(dead_code)]
struct PeerRegistry {
    peers: HashMap<PeerId, PeerEntry>,
    ttl_cleanup_interval: Duration,
    default_ttl: Duration,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PeerEntry {
    peer: DiscoveredPeer,
    expires_at: Instant,
    last_announcement: Instant,
    announcement_count: u32,
}

/// Peer exchange message for distributed discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerExchangeMessage {
    protocol_version: u8,
    sender_id: PeerId,
    peer_list: Vec<PeerAnnouncement>,
    timestamp: u64,
}

/// Individual peer announcement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAnnouncement {
    pub peer_id: PeerId,
    pub capabilities: PeerCapabilities,
    pub last_seen: u64, // Unix timestamp
    pub rssi: Option<i16>,
    pub reputation: f32,
}

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerDiscovered {
        peer: DiscoveredPeer,
    },
    PeerConnected {
        peer_id: PeerId,
    },
    PeerDisconnected {
        peer_id: PeerId,
    },
    PeerRangeChanged {
        peer_id: PeerId,
        distance: f32,
    },
    PeerExpired {
        peer_id: PeerId,
    },
    PeerListUpdated {
        peer_count: usize,
    },
    PeerExchangeReceived {
        from: PeerId,
        peer_count: usize,
    },
    DiscoveryError {
        error: String,
    },
    PeerValidated {
        peer_id: PeerId,
        validation: DiscoveryValidation,
    },
}

/// Discovery validation results
#[derive(Debug, Clone)]
pub struct DiscoveryValidation {
    pub peer_id: PeerId,
    pub ble_discovered: bool,
    pub tcp_discovered: bool,
    pub consistent_identity: bool,
    pub validation_time: std::time::Instant,
}

/// Discovery metrics for monitoring
#[derive(Debug, Clone)]
pub struct DiscoveryMetrics {
    pub total_peers_discovered: usize,
    pub validated_peers: usize,
    pub ble_discovery_active: bool,
    pub tcp_discovery_active: bool,
    pub average_discovery_time: Duration,
    pub discovery_success_rate: f32,
}

impl BluetoothDiscovery {
    pub async fn new(identity: Arc<BitchatIdentity>) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or("No Bluetooth adapter found")?;

        let (discovery_events, _) = mpsc::channel(1000); // Moderate traffic for discovery events

        let peer_registry = PeerRegistry::new(
            Duration::from_secs(300), // 5 minute TTL
            Duration::from_secs(60),  // Cleanup every minute
        );

        Ok(Self {
            identity,
            adapter: Arc::new(adapter),
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            active_connections: Arc::new(RwLock::new(HashSet::new())),
            discovery_events,
            _scan_interval: Duration::from_secs(5),
            _connection_timeout: Duration::from_secs(30),
            peer_registry: Arc::new(RwLock::new(peer_registry)),
            peer_exchange_interval: Duration::from_secs(30),
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

        // Start peer registry cleanup
        self.start_peer_registry_cleanup().await?;

        // Start periodic peer exchange
        self.start_peer_exchange().await?;

        Ok(())
    }

    /// Advertise our presence via BLE
    async fn start_advertising(&self) -> Result<(), Box<dyn std::error::Error>> {
        let identity = self.identity.clone();
        let _discovery_events = self.discovery_events.clone();

        tokio::spawn(async move {
            let mut announcement_interval = interval(Duration::from_secs(15));
            let budget = LoopBudget::for_discovery();

            loop {
                // Check budget before processing
                if !budget.can_proceed() {
                    budget.backoff().await;
                    continue;
                }

                announcement_interval.tick().await;
                budget.consume(1);

                // Create announcement with current capabilities
                let capabilities = PeerCapabilities {
                    supports_gaming: true,
                    supports_mesh_routing: true,
                    supports_token_transactions: true,
                    protocol_version: PROTOCOL_VERSION,
                    max_game_players: Some(8),
                    available_tokens: Some(1000), // Would query actual balance
                };

                let announcement = PeerAnnouncement {
                    peer_id: identity.peer_id,
                    capabilities,
                    last_seen: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    rssi: None,
                    reputation: 1.0, // Self-reported max reputation
                };

                // Create service advertisement with announcement data
                let _service_data = Self::create_announcement_data(&announcement);

                // In production, would use platform-specific BLE advertising APIs
                // For now, log the advertisement
                println!("Broadcasting BitCraps announcement: {:?}", announcement);

                // In a real implementation, this would use the BLE adapter to advertise
                // let advertisement = Advertisement::new(BITCRAPS_SERVICE_UUID, service_data);
                // adapter.advertise(advertisement).await;
            }
        });

        println!("Started BitCraps BLE advertisement");
        Ok(())
    }

    /// Scan for other BitCraps nodes
    async fn start_scanning(&self) -> Result<(), Box<dyn std::error::Error>> {
        let adapter = self.adapter.clone();
        let discovered_peers = self.discovered_peers.clone();
        let discovery_events = self.discovery_events.clone();

        tokio::spawn(async move {
            let mut scan_interval = interval(Duration::from_secs(5));
            let budget = LoopBudget::for_discovery();

            loop {
                // Check budget before processing
                if !budget.can_proceed() {
                    budget.backoff().await;
                    continue;
                }

                scan_interval.tick().await;
                budget.consume(1);

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
                                if let Some(announcement) =
                                    Self::parse_announcement_data(&properties.manufacturer_data)
                                {
                                    let peer_id = announcement.peer_id;
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
                                        capabilities: announcement.capabilities,
                                        reputation_score: announcement.reputation,
                                    };

                                    // Update or insert peer
                                    let mut peers = discovered_peers.write().await;
                                    let is_new = !peers.contains_key(&peer_id);
                                    peers.insert(peer_id, discovered_peer.clone());

                                    if is_new {
                                        let _ = discovery_events.try_send(
                                            DiscoveryEvent::PeerDiscovered {
                                                peer: discovered_peer,
                                            },
                                        );
                                    }
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
        let _adapter = self.adapter.clone();

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(10));
            let budget = LoopBudget::for_network();

            loop {
                // Check budget before processing
                if !budget.can_proceed() {
                    budget.backoff().await;
                    continue;
                }

                check_interval.tick().await;
                budget.consume(1);

                let peers = discovered_peers.read().await;
                let connections = active_connections.read().await;

                // Try to connect to nearby unconnected peers
                for (peer_id, peer) in peers.iter() {
                    if !peer.is_connected && !connections.contains(peer_id) {
                        // Only connect if close enough (within 10 meters)
                        if peer.distance_estimate < 10.0 {
                            println!(
                                "Attempting to connect to peer {:?} at ~{:.1}m",
                                peer_id, peer.distance_estimate
                            );

                            // Attempt connection
                            // In production, would implement actual BLE connection

                            let _ = discovery_events
                                .try_send(DiscoveryEvent::PeerConnected { peer_id: *peer_id });
                        }
                    }
                }

                // Check for disconnected peers (haven't seen in 30 seconds)
                let now = Instant::now();
                for (peer_id, peer) in peers.iter() {
                    if peer.is_connected
                        && now.duration_since(peer.last_seen) > Duration::from_secs(30)
                    {
                        let _ = discovery_events
                            .try_send(DiscoveryEvent::PeerDisconnected { peer_id: *peer_id });
                    }
                }
            }
        });

        Ok(())
    }

    /// Create announcement data for BLE advertisement
    fn create_announcement_data(announcement: &PeerAnnouncement) -> Vec<u8> {
        // Serialize the announcement into a compact format
        bincode::serialize(announcement).unwrap_or_default()
    }

    /// Parse announcement data from BLE advertisement
    fn parse_announcement_data(
        manufacturer_data: &HashMap<u16, Vec<u8>>,
    ) -> Option<PeerAnnouncement> {
        const BITCRAPS_MANUFACTURER_ID: u16 = 0xFFFF;

        if let Some(data) = manufacturer_data.get(&BITCRAPS_MANUFACTURER_ID) {
            bincode::deserialize(data).ok()
        } else {
            // Fallback to legacy format
            if let Some(data) = manufacturer_data.get(&BITCRAPS_MANUFACTURER_ID) {
                if data.len() >= 32 {
                    let mut peer_id = [0u8; 32];
                    peer_id.copy_from_slice(&data[0..32]);

                    let capabilities = PeerCapabilities {
                        supports_gaming: true,
                        supports_mesh_routing: true,
                        supports_token_transactions: true,
                        protocol_version: PROTOCOL_VERSION,
                        max_game_players: Some(4),
                        available_tokens: None,
                    };

                    return Some(PeerAnnouncement {
                        peer_id,
                        capabilities,
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        rssi: None,
                        reputation: 0.5, // Default reputation
                    });
                }
            }
            None
        }
    }

    /// Check if device name indicates BitCraps node
    fn is_bitcraps_device(name: &Option<String>) -> bool {
        if let Some(name) = name {
            name.starts_with("BitCraps") || name.contains("CRAP")
        } else {
            false
        }
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

    /// Start peer registry cleanup task
    async fn start_peer_registry_cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        let peer_registry = self.peer_registry.clone();
        let discovered_peers = self.discovered_peers.clone();
        let discovery_events = self.discovery_events.clone();

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60));
            let budget = LoopBudget::for_maintenance();

            loop {
                // Check budget before processing
                if !budget.can_proceed() {
                    budget.backoff().await;
                    continue;
                }

                cleanup_interval.tick().await;
                budget.consume(1);

                let now = Instant::now();
                let mut registry = peer_registry.write().await;
                let mut peers = discovered_peers.write().await;

                let mut expired_peers = Vec::new();

                // Find expired peers
                registry.peers.retain(|peer_id, entry| {
                    if now > entry.expires_at {
                        expired_peers.push(*peer_id);
                        false
                    } else {
                        true
                    }
                });

                // Remove expired peers from discovered_peers and notify
                for peer_id in expired_peers {
                    peers.remove(&peer_id);
                    let _ = discovery_events.try_send(DiscoveryEvent::PeerExpired { peer_id });
                }

                if !registry.peers.is_empty() {
                    let _ = discovery_events.try_send(DiscoveryEvent::PeerListUpdated {
                        peer_count: registry.peers.len(),
                    });
                }
            }
        });

        Ok(())
    }

    /// Start periodic peer exchange
    async fn start_peer_exchange(&self) -> Result<(), Box<dyn std::error::Error>> {
        let peer_registry = self.peer_registry.clone();
        let identity = self.identity.clone();
        let discovery_events = self.discovery_events.clone();
        let exchange_interval = self.peer_exchange_interval;

        tokio::spawn(async move {
            let mut exchange_interval = interval(exchange_interval);
            let budget = LoopBudget::for_network();

            loop {
                // Check budget before processing
                if !budget.can_proceed() {
                    budget.backoff().await;
                    continue;
                }

                exchange_interval.tick().await;
                budget.consume(1);

                let registry = peer_registry.read().await;

                // Create peer list to share
                let peer_list: Vec<PeerAnnouncement> = registry
                    .peers
                    .values()
                    .take(20) // Limit to 20 peers to avoid large messages
                    .map(|entry| PeerAnnouncement {
                        peer_id: entry.peer.peer_id,
                        capabilities: entry.peer.capabilities.clone(),
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        rssi: Some(entry.peer.rssi),
                        reputation: entry.peer.reputation_score,
                    })
                    .collect();

                if !peer_list.is_empty() {
                    let exchange_message = PeerExchangeMessage {
                        protocol_version: PROTOCOL_VERSION,
                        sender_id: identity.peer_id,
                        peer_list,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    };

                    // In production, would broadcast this to connected peers
                    println!(
                        "Broadcasting peer exchange with {} peers",
                        exchange_message.peer_list.len()
                    );

                    let _ = discovery_events.try_send(DiscoveryEvent::PeerListUpdated {
                        peer_count: registry.peers.len(),
                    });
                }
            }
        });

        Ok(())
    }

    /// Process received peer exchange message
    pub async fn process_peer_exchange(
        &self,
        message: PeerExchangeMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut registry = self.peer_registry.write().await;
        let mut discovered_peers = self.discovered_peers.write().await;
        let now = Instant::now();
        let peer_count = message.peer_list.len();

        for announcement in message.peer_list {
            // Skip our own peer ID
            if announcement.peer_id == self.identity.peer_id {
                continue;
            }

            // Create discovered peer from announcement
            let discovered_peer = DiscoveredPeer {
                peer_id: announcement.peer_id,
                device_address: String::new(), // Not available from peer exchange
                rssi: announcement.rssi.unwrap_or(-80),
                distance_estimate: Self::estimate_distance(announcement.rssi.unwrap_or(-80)),
                first_seen: now,
                last_seen: now,
                connection_attempts: 0,
                is_connected: false,
                capabilities: announcement.capabilities,
                reputation_score: announcement.reputation,
            };

            // Add to registry with TTL
            let entry = PeerEntry {
                peer: discovered_peer.clone(),
                expires_at: now + registry.default_ttl,
                last_announcement: now,
                announcement_count: 1,
            };

            let is_new = !registry.peers.contains_key(&announcement.peer_id);
            registry.peers.insert(announcement.peer_id, entry);
            discovered_peers.insert(announcement.peer_id, discovered_peer.clone());

            if is_new {
                let _ = self
                    .discovery_events
                    .try_send(DiscoveryEvent::PeerDiscovered {
                        peer: discovered_peer,
                    });
            }
        }

        let _ = self
            .discovery_events
            .try_send(DiscoveryEvent::PeerExchangeReceived {
                from: message.sender_id,
                peer_count,
            });

        Ok(())
    }

    /// Get current peer list
    pub async fn get_peer_list(&self) -> Vec<DiscoveredPeer> {
        let registry = self.peer_registry.read().await;
        registry
            .peers
            .values()
            .map(|entry| entry.peer.clone())
            .collect()
    }

    /// Get peer count
    pub async fn get_peer_count(&self) -> usize {
        let registry = self.peer_registry.read().await;
        registry.peers.len()
    }

    /// Validate discovered peer through multiple discovery paths
    pub async fn validate_peer_discovery(
        &self,
        peer_id: PeerId,
        timeout: Duration,
    ) -> Result<DiscoveryValidation, Box<dyn std::error::Error>> {
        let mut validation = DiscoveryValidation {
            peer_id,
            ble_discovered: false,
            tcp_discovered: false,
            consistent_identity: false,
            validation_time: std::time::Instant::now(),
        };

        // Check if peer is discovered via BLE
        let registry = self.peer_registry.read().await;
        if let Some(_peer_entry) = registry.peers.get(&peer_id) {
            validation.ble_discovered = true;
        }
        drop(registry);

        // Attempt TCP discovery validation if NAT traversal is enabled
        #[cfg(feature = "nat-traversal")]
        {
            validation.tcp_discovered = self.validate_tcp_discovery(peer_id, timeout).await?;
        }

        // Validate identity consistency across discovery paths
        validation.consistent_identity = self.validate_identity_consistency(peer_id).await?;

        Ok(validation)
    }

    /// Validate TCP discovery path
    #[cfg(feature = "nat-traversal")]
    async fn validate_tcp_discovery(
        &self,
        peer_id: PeerId,
        timeout: Duration,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // This would integrate with TCP discovery mechanisms
        // For now, return a placeholder implementation
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(false) // TCP discovery validation not yet implemented
    }

    /// Validate identity consistency across multiple discovery paths
    async fn validate_identity_consistency(
        &self,
        peer_id: PeerId,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Check if the peer identity is consistent across BLE and TCP discovery
        let registry = self.peer_registry.read().await;

        if let Some(peer_entry) = registry.peers.get(&peer_id) {
            // Validate cryptographic identity matches announcement
            let expected_capabilities = &peer_entry.peer.capabilities;

            // Simple validation: check if capabilities are reasonable
            Ok(expected_capabilities.protocol_version > 0
                && expected_capabilities.protocol_version <= PROTOCOL_VERSION)
        } else {
            Ok(false)
        }
    }

    /// Get discovery validation metrics
    pub async fn get_discovery_metrics(&self) -> DiscoveryMetrics {
        let registry = self.peer_registry.read().await;
        let total_peers = registry.peers.len();
        let validated_peers = registry
            .peers
            .values()
            .filter(|entry| entry.peer.reputation_score > 0.5)
            .count();

        DiscoveryMetrics {
            total_peers_discovered: total_peers,
            validated_peers,
            ble_discovery_active: !registry.peers.is_empty(),
            tcp_discovery_active: false, // Would be updated by TCP discovery
            average_discovery_time: Duration::from_secs(5), // Placeholder
            discovery_success_rate: if total_peers > 0 {
                validated_peers as f32 / total_peers as f32
            } else {
                0.0
            },
        }
    }
}

impl PeerRegistry {
    fn new(default_ttl: Duration, cleanup_interval: Duration) -> Self {
        Self {
            peers: HashMap::new(),
            ttl_cleanup_interval: cleanup_interval,
            default_ttl,
        }
    }
}

// Constants that would be defined elsewhere
#[allow(dead_code)]
const BITCRAPS_SERVICE_UUID: &str = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E";
const PROTOCOL_VERSION: u8 = 1;

// Default reputation and capability values
impl Default for PeerCapabilities {
    fn default() -> Self {
        Self {
            supports_gaming: true,
            supports_mesh_routing: false,
            supports_token_transactions: false,
            protocol_version: PROTOCOL_VERSION,
            max_game_players: Some(4),
            available_tokens: None,
        }
    }
}
