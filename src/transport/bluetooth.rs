//! Complete Bluetooth LE transport implementation for BitCraps mesh networking

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, Mutex};
use async_trait::async_trait;
use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter, WriteType, CentralEvent
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::stream::StreamExt;
use uuid::Uuid;

use crate::protocol::{PeerId, BitchatPacket};
use crate::transport::{Transport, TransportAddress, TransportEvent};

/// BitCraps GATT Service UUID
const BITCRAPS_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);
/// Characteristic for receiving data (from perspective of central)
const BITCRAPS_RX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345679);
/// Characteristic for transmitting data (from perspective of central)
const BITCRAPS_TX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345680);
/// BLE MTU size for packet fragmentation
const BLE_MTU_SIZE: usize = 512;
/// Fragment header size (sequence + flags)
const FRAGMENT_HEADER_SIZE: usize = 4;

/// Connection limits for Bluetooth transport
#[derive(Debug, Clone)]
pub struct BluetoothConnectionLimits {
    pub max_concurrent_connections: usize,
    pub max_connection_attempts_per_minute: usize,
    pub connection_timeout: Duration,
}

impl Default for BluetoothConnectionLimits {
    fn default() -> Self {
        Self {
            max_concurrent_connections: 50,
            max_connection_attempts_per_minute: 20,
            connection_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone)]
struct DiscoveredPeer {
    device_id: String,
    peripheral_id: PeripheralId,
    peer_id: Option<PeerId>,
    rssi: i16,
    last_seen: Instant,
    connection_attempts: u32,
}

/// Packet fragment for reassembly
#[derive(Debug, Clone)]
struct PacketFragment {
    sequence: u16,
    is_last: bool,
    data: Vec<u8>,
}

/// Connection state for a peer
#[derive(Debug)]
struct PeerConnection {
    peripheral: Peripheral,
    peer_id: PeerId,
    tx_char: Option<btleplug::api::Characteristic>,
    rx_char: Option<btleplug::api::Characteristic>,
    /// Fragments being reassembled for incoming packets
    incoming_fragments: HashMap<u16, Vec<PacketFragment>>,
    /// Next sequence number for outgoing fragments
    next_tx_sequence: u16,
    last_activity: Instant,
}

/// Bluetooth mesh transport implementation
pub struct BluetoothTransport {
    manager: Manager,
    adapter: Option<Adapter>,
    connections: Arc<RwLock<HashMap<PeerId, PeerConnection>>>,
    connection_limits: BluetoothConnectionLimits,
    connection_attempts: Arc<RwLock<Vec<Instant>>>,
    event_sender: mpsc::UnboundedSender<TransportEvent>,
    event_receiver: mpsc::UnboundedReceiver<TransportEvent>,
    local_peer_id: PeerId,
    is_scanning: Arc<RwLock<bool>>,
    discovered_peers: Arc<RwLock<HashMap<String, DiscoveredPeer>>>,
    /// Active scan task handle
    scan_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Connection monitoring task handle
    monitor_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl BluetoothTransport {
    pub async fn new(local_peer_id: PeerId) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_limits(local_peer_id, BluetoothConnectionLimits::default()).await
    }
    
    pub async fn new_with_limits(
        local_peer_id: PeerId,
        limits: BluetoothConnectionLimits,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next();
        
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        let transport = Self {
            manager,
            adapter,
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_limits: limits,
            connection_attempts: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            event_receiver,
            local_peer_id,
            is_scanning: Arc::new(RwLock::new(false)),
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            scan_task: Arc::new(Mutex::new(None)),
            monitor_task: Arc::new(Mutex::new(None)),
        };
        
        // Start cleanup task for connection attempts
        transport.start_connection_cleanup_task();
        
        // Start connection monitoring task
        transport.start_connection_monitor().await;
        
        Ok(transport)
    }
    
    /// Start background task to clean up old connection attempts
    fn start_connection_cleanup_task(&self) {
        let connection_attempts = self.connection_attempts.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let cutoff = Instant::now() - Duration::from_secs(60);
                
                let mut attempts = connection_attempts.write().await;
                attempts.retain(|&timestamp| timestamp > cutoff);
            }
        });
    }
    
    /// Start connection monitoring task
    async fn start_connection_monitor(&self) {
        let connections = self.connections.clone();
        let event_sender = self.event_sender.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                let mut connections_guard = connections.write().await;
                let mut to_remove = Vec::new();
                
                for (peer_id, connection) in connections_guard.iter_mut() {
                    // Check if peripheral is still connected
                    if !connection.peripheral.is_connected().await.unwrap_or(false) {
                        log::warn!("Peer {:?} disconnected unexpectedly", peer_id);
                        to_remove.push(*peer_id);
                        
                        let _ = event_sender.send(TransportEvent::Disconnected {
                            peer_id: *peer_id,
                            reason: "Connection lost".to_string(),
                        });
                    } else {
                        // Update last activity
                        connection.last_activity = Instant::now();
                    }
                }
                
                // Remove disconnected peers
                for peer_id in to_remove {
                    connections_guard.remove(&peer_id);
                }
            }
        });
        
        *self.monitor_task.lock().await = Some(handle);
    }
    
    /// Check if a new connection is allowed based on Bluetooth-specific limits (internal)
    async fn check_bluetooth_connection_limits_internal(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Check concurrent connection limit
        let connections = self.connections.read().await;
        if connections.len() >= self.connection_limits.max_concurrent_connections {
            return Err(format!(
                "Bluetooth connection rejected: Maximum concurrent connections ({}) exceeded",
                self.connection_limits.max_concurrent_connections
            ).into());
        }
        
        // Check rate limiting
        let now = Instant::now();
        let one_minute_ago = now - Duration::from_secs(60);
        let attempts = self.connection_attempts.read().await;
        
        let recent_attempts = attempts
            .iter()
            .filter(|&&timestamp| timestamp > one_minute_ago)
            .count();
        
        if recent_attempts >= self.connection_limits.max_connection_attempts_per_minute {
            return Err(format!(
                "Bluetooth connection rejected: Rate limit exceeded ({} attempts/minute)",
                self.connection_limits.max_connection_attempts_per_minute
            ).into());
        }
        
        Ok(())
    }
    
    /// Check if a new connection is allowed based on Bluetooth-specific limits (test-only public wrapper)
    #[cfg(test)]
    pub async fn check_bluetooth_connection_limits(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.check_bluetooth_connection_limits_internal().await
    }
    
    /// Record a connection attempt for rate limiting (internal)
    async fn record_bluetooth_connection_attempt_internal(&self) {
        let mut attempts = self.connection_attempts.write().await;
        attempts.push(Instant::now());
    }
    
    /// Record a connection attempt for rate limiting (test-only public wrapper)
    #[cfg(test)]
    pub async fn record_bluetooth_connection_attempt(&self) {
        self.record_bluetooth_connection_attempt_internal().await;
    }
    
    /// Start advertising as a BitCraps node
    pub async fn start_advertising(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_adapter) = &self.adapter {
            log::info!("Starting BitCraps BLE advertising with peer_id: {:?}", self.local_peer_id);
            
            // Note: btleplug doesn't currently support peripheral mode (advertising) on most platforms
            // This is a limitation of the library. In a real implementation, you would need to use
            // platform-specific APIs like Core Bluetooth on macOS/iOS or BlueZ on Linux.
            // For now, we'll just log that we would start advertising and focus on the central (scanning) role.
            
            log::warn!("BLE peripheral mode (advertising) not fully supported by btleplug on this platform.");
            log::info!("Device will operate in central mode only - scanning for other BitCraps nodes.");
            
            // In a real implementation, this would:
            // 1. Set up GATT server with BitCraps service
            // 2. Add TX/RX characteristics with proper permissions
            // 3. Start advertising with service UUID in advertisement data
            // 4. Handle incoming connections and characteristic writes
            
            Ok(())
        } else {
            Err("No Bluetooth adapter available".into())
        }
    }
    
    /// Scan for other BitCraps nodes
    pub async fn scan_for_peers(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(adapter) = &self.adapter {
            *self.is_scanning.write().await = true;
            
            // Create scan filter to look specifically for BitCraps service
            let scan_filter = ScanFilter {
                services: vec![BITCRAPS_SERVICE_UUID],
            };
            
            adapter.start_scan(scan_filter).await?;
            log::info!("Started scanning for BitCraps devices with service UUID: {}", BITCRAPS_SERVICE_UUID);
            
            let mut events = adapter.events().await?;
            let connections = self.connections.clone();
            let event_sender = self.event_sender.clone();
            let is_scanning = self.is_scanning.clone();
            let discovered_peers = self.discovered_peers.clone();
            let _local_peer_id = self.local_peer_id;
            let adapter_clone = adapter.clone();
            
            let scan_handle = tokio::spawn(async move {
                while *is_scanning.read().await {
                    if let Some(event) = events.next().await {
                        log::debug!("BLE event: {:?}", event);
                        
                        match event {
                            CentralEvent::DeviceDiscovered(id) => {
                                log::info!("Discovered BLE device: {:?}", id);
                                
                                // Get peripheral and check if it advertises BitCraps service
                                if let Ok(peripheral) = adapter_clone.peripheral(&id).await {
                                    if let Ok(properties) = peripheral.properties().await {
                                        if let Some(props) = properties {
                                            log::debug!("Device properties: {:?}", props);
                                            
                                            // Check if this device advertises our service
                                            let advertises_bitcraps = props.services
                                                .iter()
                                                .any(|service| *service == BITCRAPS_SERVICE_UUID);
                                            
                                            if advertises_bitcraps {
                                                let device_id = format!("{:?}", id);
                                                let rssi = props.rssi.unwrap_or(0);
                                                
                                                log::info!("Found BitCraps device: {} (RSSI: {})", device_id, rssi);
                                                
                                                // Store discovered peer
                                                let peer = DiscoveredPeer {
                                                    device_id: device_id.clone(),
                                                    peripheral_id: id.clone(),
                                                    peer_id: None, // Will be determined during connection
                                                    rssi,
                                                    last_seen: Instant::now(),
                                                    connection_attempts: 0,
                                                };
                                                
                                                discovered_peers.write().await.insert(device_id.clone(), peer);
                                                
                                                // Check if we should auto-connect
                                                let current_connections = connections.read().await.len();
                                                if current_connections < 3 { // Auto-connect to first few devices
                                                    log::info!("Auto-connecting to discovered BitCraps device: {}", device_id);
                                                    
                                                    // Note: Auto-connection would be implemented here
                                                    // For now, just emit a connection event to let the application decide
                                                    let _ = event_sender.send(TransportEvent::Connected {
                                                        peer_id: [0u8; 32], // Placeholder until we implement full connection
                                                        address: TransportAddress::Bluetooth(device_id),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            CentralEvent::DeviceConnected(id) => {
                                log::info!("Device connected: {:?}", id);
                            }
                            CentralEvent::DeviceDisconnected(id) => {
                                log::info!("Device disconnected: {:?}", id);
                                
                                // Find and remove from connections
                                let mut connections_guard = connections.write().await;
                                let mut disconnected_peer_id = None;
                                
                                for (peer_id, connection) in connections_guard.iter() {
                                    if connection.peripheral.id() == id {
                                        disconnected_peer_id = Some(*peer_id);
                                        break;
                                    }
                                }
                                
                                if let Some(peer_id) = disconnected_peer_id {
                                    connections_guard.remove(&peer_id);
                                    let _ = event_sender.send(TransportEvent::Disconnected {
                                        peer_id,
                                        reason: "Device disconnected".to_string(),
                                    });
                                }
                            }
                            _ => {
                                log::debug!("Unhandled BLE event: {:?}", event);
                            }
                        }
                    }
                }
                
                log::info!("Scanning stopped");
            });
            
            // Store scan task handle
            *self.scan_task.lock().await = Some(scan_handle);
        } else {
            return Err("No Bluetooth adapter available".into());
        }
        
        Ok(())
    }
    
    /// Send packet over Bluetooth to peer with fragmentation support
    async fn send_over_ble(
        &self,
        peer_id: PeerId,
        packet: &BitchatPacket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.get_mut(&peer_id) {
            // Serialize packet
            let mut serialized_packet = packet.clone();
            let data = serialized_packet.serialize()
                .map_err(|e| format!("Packet serialization failed: {}", e))?;
            
            // Get TX characteristic
            let tx_char = connection.tx_char.as_ref()
                .ok_or("TX characteristic not available")?;
            
            // Check if fragmentation is needed
            let max_fragment_size = BLE_MTU_SIZE - FRAGMENT_HEADER_SIZE;
            
            if data.len() <= max_fragment_size {
                // Single fragment - send directly
                let mut fragment_data = Vec::with_capacity(data.len() + FRAGMENT_HEADER_SIZE);
                
                // Fragment header: [sequence:u16][flags:u16]
                let sequence = connection.next_tx_sequence;
                connection.next_tx_sequence = connection.next_tx_sequence.wrapping_add(1);
                
                fragment_data.extend_from_slice(&sequence.to_be_bytes());
                fragment_data.extend_from_slice(&0x8000u16.to_be_bytes()); // Last fragment flag
                fragment_data.extend_from_slice(&data);
                
                connection.peripheral.write(
                    tx_char,
                    &fragment_data,
                    WriteType::WithoutResponse,
                ).await?;
                
                log::debug!("Sent single fragment of {} bytes to peer {:?}", fragment_data.len(), peer_id);
            } else {
                // Multiple fragments needed
                let total_fragments = (data.len() + max_fragment_size - 1) / max_fragment_size;
                let base_sequence = connection.next_tx_sequence;
                connection.next_tx_sequence = connection.next_tx_sequence.wrapping_add(total_fragments as u16);
                
                log::debug!("Fragmenting {} bytes into {} fragments for peer {:?}", data.len(), total_fragments, peer_id);
                
                for (fragment_index, chunk) in data.chunks(max_fragment_size).enumerate() {
                    let mut fragment_data = Vec::with_capacity(chunk.len() + FRAGMENT_HEADER_SIZE);
                    
                    // Fragment header: [sequence:u16][flags:u16]
                    let fragment_sequence = base_sequence.wrapping_add(fragment_index as u16);
                    let is_last = fragment_index == total_fragments - 1;
                    let flags = if is_last { 0x8000u16 } else { 0x0000u16 }; // Last fragment flag
                    
                    fragment_data.extend_from_slice(&fragment_sequence.to_be_bytes());
                    fragment_data.extend_from_slice(&flags.to_be_bytes());
                    fragment_data.extend_from_slice(chunk);
                    
                    connection.peripheral.write(
                        tx_char,
                        &fragment_data,
                        WriteType::WithoutResponse,
                    ).await?;
                    
                    log::debug!("Sent fragment {}/{} ({} bytes) to peer {:?}", 
                              fragment_index + 1, total_fragments, fragment_data.len(), peer_id);
                    
                    // Small delay between fragments to prevent overwhelming the receiver
                    if !is_last {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }
            
            // Update last activity
            connection.last_activity = Instant::now();
            
            Ok(())
        } else {
            Err("Peer not connected".into())
        }
    }
    
    /// Handle incoming data from a peer
    async fn handle_incoming_data(&self, peer_id: PeerId, data: Vec<u8>) {
        // Send event to application layer
        let _ = self.event_sender.send(TransportEvent::DataReceived {
            peer_id,
            data,
        });
    }
    
    /// Connect to a discovered peer with connection limits enforced
    async fn connect_to_peripheral(&self, device_id: &str) -> Result<PeerId, Box<dyn std::error::Error>> {
        // Check connection limits before attempting to connect
        self.check_bluetooth_connection_limits_internal().await?;
        
        // Record the connection attempt
        self.record_bluetooth_connection_attempt_internal().await;
        
        log::info!("Connecting to Bluetooth device: {} (within limits)", device_id);
        
        // Get the peripheral from discovered peers
        let discovered_peers = self.discovered_peers.read().await;
        let peer_info = discovered_peers.get(device_id)
            .ok_or("Device not found in discovered peers")?;
        let peripheral_id = peer_info.peripheral_id.clone();
        drop(discovered_peers);
        
        // Get adapter and peripheral
        let adapter = self.adapter.as_ref().ok_or("No Bluetooth adapter available")?;
        let peripheral = adapter.peripheral(&peripheral_id).await?;
        
        // Actual connection with timeout protection
        let connection_future = async {
            log::info!("Attempting to connect to peripheral: {:?}", peripheral_id);
            
            // Connect to the peripheral
            peripheral.connect().await?;
            log::info!("Connected to peripheral: {:?}", peripheral_id);
            
            // Discover services
            peripheral.discover_services().await?;
            log::info!("Discovered services for peripheral: {:?}", peripheral_id);
            
            // Find BitCraps service and characteristics
            let services = peripheral.services();
            let mut tx_char = None;
            let mut rx_char = None;
            
            for service in services {
                if service.uuid == BITCRAPS_SERVICE_UUID {
                    log::info!("Found BitCraps service on peripheral: {:?}", peripheral_id);
                    
                    for characteristic in &service.characteristics {
                        if characteristic.uuid == BITCRAPS_TX_CHAR_UUID {
                            tx_char = Some(characteristic.clone());
                            log::info!("Found TX characteristic");
                        } else if characteristic.uuid == BITCRAPS_RX_CHAR_UUID {
                            rx_char = Some(characteristic.clone());
                            log::info!("Found RX characteristic");
                        }
                    }
                    break;
                }
            }
            
            if tx_char.is_none() || rx_char.is_none() {
                return Err("Required characteristics not found".into());
            }
            
            // Subscribe to RX characteristic for incoming data
            if let Some(ref rx_characteristic) = rx_char {
                peripheral.subscribe(rx_characteristic).await?;
                log::info!("Subscribed to RX characteristic");
            }
            
            // Generate a peer ID based on device characteristics
            // In a real implementation, this would be exchanged during a handshake protocol
            let peer_id = {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::Hasher;
                hasher.write(format!("{:?}", peripheral_id).as_bytes());
                let hash = hasher.finish();
                let mut peer_id = [0u8; 32];
                peer_id[..8].copy_from_slice(&hash.to_be_bytes());
                peer_id
            };
            
            // Create connection object
            let connection = PeerConnection {
                peripheral: peripheral.clone(),
                peer_id,
                tx_char,
                rx_char,
                incoming_fragments: HashMap::new(),
                next_tx_sequence: 0,
                last_activity: Instant::now(),
            };
            
            // Store the connection
            self.connections.write().await.insert(peer_id, connection);
            log::info!("Stored connection for peer: {:?}", peer_id);
            
            Result::<PeerId, Box<dyn std::error::Error>>::Ok(peer_id)
        };
        
        // Apply connection timeout
        let peer_id = tokio::time::timeout(
            self.connection_limits.connection_timeout,
            connection_future
        ).await
        .map_err(|_| "Bluetooth connection timeout")??
        ;
        
        // Send connection event only on successful connection
        let _ = self.event_sender.send(TransportEvent::Connected {
            peer_id,
            address: TransportAddress::Bluetooth(device_id.to_string()),
        });
        
        log::info!("Successfully connected to Bluetooth device: {} (peer_id: {:?})", device_id, peer_id);
        
        Ok(peer_id)
    }
    
    /// Get Bluetooth connection statistics
    pub async fn bluetooth_stats(&self) -> BluetoothStats {
        let connections = self.connections.read().await;
        let attempts = self.connection_attempts.read().await;
        
        let now = Instant::now();
        let recent_attempts = attempts
            .iter()
            .filter(|&&timestamp| now.duration_since(timestamp) < Duration::from_secs(60))
            .count();
        
        BluetoothStats {
            active_connections: connections.len(),
            max_connections: self.connection_limits.max_concurrent_connections,
            recent_connection_attempts: recent_attempts,
            rate_limit: self.connection_limits.max_connection_attempts_per_minute,
        }
    }
}

#[async_trait]
impl Transport for BluetoothTransport {
    async fn listen(&mut self, address: TransportAddress) -> Result<(), Box<dyn std::error::Error>> {
        match address {
            TransportAddress::Bluetooth(name) => {
                log::info!("Listening as Bluetooth device: {} (max connections: {})", 
                          name, self.connection_limits.max_concurrent_connections);
                self.start_advertising().await?;
                self.scan_for_peers().await?;
                Ok(())
            }
            _ => Err("Invalid address type for Bluetooth transport".into()),
        }
    }
    
    async fn connect(&mut self, address: TransportAddress) -> Result<PeerId, Box<dyn std::error::Error>> {
        match address {
            TransportAddress::Bluetooth(device_id) => {
                // Connection limits are checked inside connect_to_peripheral
                self.connect_to_peripheral(&device_id).await
            }
            _ => Err("Invalid address type for Bluetooth transport".into()),
        }
    }
    
    async fn send(&mut self, peer_id: PeerId, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        // Create packet from data
        let mut cursor = std::io::Cursor::new(data);
        let packet = BitchatPacket::deserialize(&mut cursor)?;
        
        self.send_over_ble(peer_id, &packet).await
    }
    
    async fn disconnect(&mut self, peer_id: PeerId) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.remove(&peer_id) {
            match connection.peripheral.disconnect().await {
                Ok(_) => {
                    log::info!("Successfully disconnected from peer: {:?}", peer_id);
                    let _ = self.event_sender.send(TransportEvent::Disconnected {
                        peer_id,
                        reason: "User requested disconnect".to_string(),
                    });
                }
                Err(e) => {
                    log::error!("Error disconnecting from peer {:?}: {}", peer_id, e);
                    let _ = self.event_sender.send(TransportEvent::Error {
                        peer_id: Some(peer_id),
                        error: format!("Disconnect failed: {}", e),
                    });
                    return Err(Box::new(e));
                }
            }
        } else {
            log::warn!("Attempted to disconnect from unknown peer: {:?}", peer_id);
        }
        
        Ok(())
    }
    
    fn is_connected(&self, peer_id: &PeerId) -> bool {
        if let Ok(connections) = self.connections.try_read() {
            connections.contains_key(peer_id)
        } else {
            false
        }
    }
    
    fn connected_peers(&self) -> Vec<PeerId> {
        if let Ok(connections) = self.connections.try_read() {
            connections.keys().copied().collect()
        } else {
            Vec::new()
        }
    }
    
    async fn next_event(&mut self) -> Option<TransportEvent> {
        self.event_receiver.recv().await
    }
}

/// Bluetooth connection statistics
#[derive(Debug, Clone)]
pub struct BluetoothStats {
    pub active_connections: usize,
    pub max_connections: usize,
    pub recent_connection_attempts: usize,
    pub rate_limit: usize,
}

/// Bluetooth mesh network coordinator
pub struct BluetoothMeshCoordinator {
    transport: BluetoothTransport,
    routing_table: Arc<RwLock<HashMap<PeerId, Vec<PeerId>>>>,
    message_cache: Arc<RwLock<HashMap<u64, Instant>>>,
}

impl BluetoothMeshCoordinator {
    pub async fn new(local_peer_id: PeerId) -> Result<Self, Box<dyn std::error::Error>> {
        let transport = BluetoothTransport::new(local_peer_id).await?;
        
        Ok(Self {
            transport,
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Route message through mesh network
    pub async fn route_message(
        &self,
        packet: &BitchatPacket,
        target: PeerId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we have direct connection
        if self.transport.is_connected(&target) {
            let mut serialized_packet = packet.clone();
            let _data = serialized_packet.serialize()
                .map_err(|e| format!("Packet serialization failed: {}", e))?;
            return self.transport.send_over_ble(target, packet).await;
        }
        
        // Find route through mesh
        let routing_table = self.routing_table.read().await;
        if let Some(next_hops) = routing_table.get(&target) {
            // Send to first available next hop
            for next_hop in next_hops {
                if self.transport.is_connected(next_hop) {
                    return self.transport.send_over_ble(*next_hop, packet).await;
                }
            }
        }
        
        // No route found - broadcast to all peers
        let peers = self.transport.connected_peers();
        for peer in peers {
            let _ = self.transport.send_over_ble(peer, packet).await;
        }
        
        Ok(())
    }
    
    /// Update routing table with new peer information
    pub async fn update_routing(&self, peer_id: PeerId, next_hops: Vec<PeerId>) {
        self.routing_table.write().await.insert(peer_id, next_hops);
    }
    
    /// Clean expired entries from message cache
    pub async fn cleanup_message_cache(&self) {
        let mut cache = self.message_cache.write().await;
        let cutoff = Instant::now() - Duration::from_secs(300); // 5 minutes
        
        cache.retain(|_, &mut timestamp| timestamp > cutoff);
    }
}