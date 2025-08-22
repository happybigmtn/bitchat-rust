use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};

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
        let _service_data = self.create_service_advertisement();
        
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
        let _adapter = self.adapter.clone();
        
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

// Constants that would be defined elsewhere
const BITCRAPS_SERVICE_UUID: &str = "6E400001-B5A3-F393-E0A9-E50E24DCCA9E";
const PROTOCOL_VERSION: u8 = 1;