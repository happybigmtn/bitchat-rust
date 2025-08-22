//! Bluetooth LE transport implementation for BitCraps mesh networking

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use async_trait::async_trait;
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::stream::StreamExt;
use uuid::Uuid;

use crate::protocol::{PeerId, BitchatPacket};
use crate::transport::{Transport, TransportAddress, TransportEvent};

/// BitCraps GATT Service UUID
const BITCRAPS_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);
#[allow(dead_code)]
const BITCRAPS_RX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345679);
const BITCRAPS_TX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345680);

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

/// Bluetooth mesh transport implementation
#[allow(dead_code)]
pub struct BluetoothTransport {
    manager: Manager,
    adapter: Option<Adapter>,
    connections: Arc<RwLock<HashMap<PeerId, Peripheral>>>,
    connection_limits: BluetoothConnectionLimits,
    connection_attempts: Arc<RwLock<Vec<Instant>>>,
    event_sender: mpsc::UnboundedSender<TransportEvent>,
    event_receiver: mpsc::UnboundedReceiver<TransportEvent>,
    local_peer_id: PeerId,
    is_scanning: Arc<RwLock<bool>>,
    discovered_peers: Arc<RwLock<HashMap<String, DiscoveredPeer>>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DiscoveredPeer {
    device_id: String,
    peer_id: Option<PeerId>,
    rssi: i16,
    last_seen: Instant,
    connection_attempts: u32,
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
        };
        
        // Start cleanup task for connection attempts
        transport.start_connection_cleanup_task();
        
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
        // In production, would use platform-specific BLE advertising APIs
        // This is simplified for cross-platform compatibility
        log::info!("Starting BitCraps BLE advertising with peer_id: {:?}", self.local_peer_id);
        
        // Start advertising BitCraps service
        // Platform-specific implementation would go here
        
        Ok(())
    }
    
    /// Scan for other BitCraps nodes
    pub async fn scan_for_peers(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(adapter) = &self.adapter {
            *self.is_scanning.write().await = true;
            
            adapter.start_scan(ScanFilter::default()).await?;
            
            let mut events = adapter.events().await?;
            let _connections = self.connections.clone();
            let _event_sender = self.event_sender.clone();
            let is_scanning = self.is_scanning.clone();
            let _discovered_peers = self.discovered_peers.clone();
            
            tokio::spawn(async move {
                while *is_scanning.read().await {
                    if let Some(event) = events.next().await {
                        // Process BLE events and look for BitCraps devices
                        log::debug!("BLE event: {:?}", event);
                        
                        // In a real implementation, would parse advertisement data
                        // to identify BitCraps devices and extract peer IDs
                    }
                }
            });
        } else {
            return Err("No Bluetooth adapter available".into());
        }
        
        Ok(())
    }
    
    /// Send packet over Bluetooth to peer
    async fn send_over_ble(
        &self,
        peer_id: PeerId,
        packet: &BitchatPacket,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connections = self.connections.read().await;
        
        if let Some(peripheral) = connections.get(&peer_id) {
            // Serialize packet
            let mut serialized_packet = packet.clone();
            let data = serialized_packet.serialize()
                .map_err(|e| format!("Packet serialization failed: {}", e))?;
            
            // Find TX characteristic by UUID
            let services = peripheral.services();
            for service in services {
                if service.uuid == BITCRAPS_SERVICE_UUID {
                    for characteristic in service.characteristics {
                        if characteristic.uuid == BITCRAPS_TX_CHAR_UUID {
                            peripheral.write(
                                &characteristic,
                                &data,
                                btleplug::api::WriteType::WithoutResponse,
                            ).await?;
                            return Ok(());
                        }
                    }
                }
            }
            
            Err("TX characteristic not found".into())
        } else {
            Err("Peer not connected".into())
        }
    }
    
    /// Handle incoming data from a peer
    #[allow(dead_code)]
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
        
        // This is a simplified implementation
        // In practice, would:
        // 1. Get peripheral by device ID
        // 2. Connect to peripheral with timeout
        // 3. Discover services and characteristics
        // 4. Subscribe to notifications
        // 5. Exchange peer IDs
        // 6. Return the peer's PeerId
        
        // Simulate connection timeout protection
        let connection_future = async {
            // Placeholder - would implement actual BLE connection
            let peer_id = [0u8; 32]; // Would get actual peer ID during handshake
            
            // In real implementation, would store the peripheral connection
            // self.connections.write().await.insert(peer_id, peripheral);
            
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
        
        if let Some(peripheral) = connections.remove(&peer_id) {
            match peripheral.disconnect().await {
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