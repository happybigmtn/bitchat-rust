//! Secure GATT Server implementation for BitCraps transport layer
//!
//! This module provides a secure GATT server that supports:
//! - Bidirectional communication in peripheral mode
//! - Transport-layer encryption for all data
//! - Connection prioritization and management
//! - Platform-specific implementations

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, mpsc};
use bytes::{Bytes, BytesMut};
use uuid::Uuid;

use crate::protocol::PeerId;
use crate::transport::{
    TransportEvent, BlePeripheral, PeripheralEvent,
    crypto::{TransportCrypto, ConnectionPriority},
    bounded_queue::{BoundedTransportEventSender, BoundedQueueError}
};
use crate::error::{Error, Result};

/// BitCraps GATT Service UUID (same as in bluetooth.rs)
pub const BITCRAPS_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);

/// Characteristic UUIDs
pub const BITCRAPS_RX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345679);
pub const BITCRAPS_TX_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345680);
pub const BITCRAPS_KEY_EXCHANGE_CHAR_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345681);

/// Maximum GATT packet size (247 bytes for BLE 4.2+)
const MAX_GATT_PACKET_SIZE: usize = 247;

/// Connection timeout for GATT clients
const CLIENT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// GATT characteristic properties
#[derive(Debug, Clone, Copy)]
pub struct CharacteristicProperties {
    pub read: bool,
    pub write: bool,
    pub write_without_response: bool,
    pub notify: bool,
    pub indicate: bool,
}

impl Default for CharacteristicProperties {
    fn default() -> Self {
        Self {
            read: false,
            write: false,
            write_without_response: false,
            notify: false,
            indicate: false,
        }
    }
}

/// GATT characteristic definition
#[derive(Debug, Clone)]
pub struct GattCharacteristic {
    pub uuid: Uuid,
    pub properties: CharacteristicProperties,
    pub value: Vec<u8>,
    pub descriptors: HashMap<Uuid, Vec<u8>>,
}

impl GattCharacteristic {
    pub fn new(uuid: Uuid, properties: CharacteristicProperties) -> Self {
        Self {
            uuid,
            properties,
            value: Vec::new(),
            descriptors: HashMap::new(),
        }
    }
    
    pub fn with_value(mut self, value: Vec<u8>) -> Self {
        self.value = value;
        self
    }
    
    pub fn add_descriptor(mut self, descriptor_uuid: Uuid, value: Vec<u8>) -> Self {
        self.descriptors.insert(descriptor_uuid, value);
        self
    }
}

/// GATT service definition
#[derive(Debug, Clone)]
pub struct GattService {
    pub uuid: Uuid,
    pub primary: bool,
    pub characteristics: HashMap<Uuid, GattCharacteristic>,
}

impl GattService {
    pub fn new(uuid: Uuid, primary: bool) -> Self {
        Self {
            uuid,
            primary,
            characteristics: HashMap::new(),
        }
    }
    
    pub fn add_characteristic(mut self, characteristic: GattCharacteristic) -> Self {
        self.characteristics.insert(characteristic.uuid, characteristic);
        self
    }
}

/// Connected GATT client information
#[derive(Debug, Clone)]
pub struct GattClient {
    pub peer_id: PeerId,
    pub client_address: String,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub subscribed_characteristics: Vec<Uuid>,
    pub priority: ConnectionPriority,
    pub mtu: u16,
}

impl GattClient {
    pub fn new(peer_id: PeerId, client_address: String) -> Self {
        let now = Instant::now();
        Self {
            peer_id,
            client_address,
            connected_at: now,
            last_activity: now,
            subscribed_characteristics: Vec::new(),
            priority: ConnectionPriority::Normal,
            mtu: 23, // Default BLE MTU
        }
    }
    
    pub fn is_subscribed(&self, char_uuid: &Uuid) -> bool {
        self.subscribed_characteristics.contains(char_uuid)
    }
    
    pub fn subscribe(&mut self, char_uuid: Uuid) {
        if !self.is_subscribed(&char_uuid) {
            self.subscribed_characteristics.push(char_uuid);
        }
    }
    
    pub fn unsubscribe(&mut self, char_uuid: &Uuid) {
        self.subscribed_characteristics.retain(|uuid| uuid != char_uuid);
    }
}

/// GATT operation types
#[derive(Debug, Clone)]
pub enum GattOperation {
    Read { char_uuid: Uuid, offset: u16 },
    Write { char_uuid: Uuid, data: Vec<u8>, with_response: bool },
    Subscribe { char_uuid: Uuid },
    Unsubscribe { char_uuid: Uuid },
    Notify { char_uuid: Uuid, data: Vec<u8> },
    Indicate { char_uuid: Uuid, data: Vec<u8> },
}

/// GATT server events
#[derive(Debug, Clone)]
pub enum GattServerEvent {
    ClientConnected { client: GattClient },
    ClientDisconnected { peer_id: PeerId, reason: String },
    CharacteristicRead { peer_id: PeerId, char_uuid: Uuid, offset: u16 },
    CharacteristicWrite { peer_id: PeerId, char_uuid: Uuid, data: Vec<u8> },
    ClientSubscribed { peer_id: PeerId, char_uuid: Uuid },
    ClientUnsubscribed { peer_id: PeerId, char_uuid: Uuid },
    MtuChanged { peer_id: PeerId, new_mtu: u16 },
    Error { peer_id: Option<PeerId>, error: String },
}

/// Secure GATT Server implementation
pub struct SecureGattServer {
    /// Local peer ID
    local_peer_id: PeerId,
    
    /// GATT services
    services: Arc<RwLock<HashMap<Uuid, GattService>>>,
    
    /// Connected clients
    clients: Arc<RwLock<HashMap<PeerId, GattClient>>>,
    
    /// Transport encryption
    crypto: Arc<TransportCrypto>,
    
    /// Event sender for transport events
    event_sender: BoundedTransportEventSender,
    
    /// Server state
    is_running: Arc<RwLock<bool>>,
    
    /// Background tasks
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    
    /// Connection timeout monitor
    timeout_monitor: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    
    /// Security settings
    require_encryption: bool,
    require_authentication: bool,
}

impl SecureGattServer {
    /// Create new secure GATT server
    pub fn new(
        local_peer_id: PeerId,
        event_sender: BoundedTransportEventSender,
        crypto: Arc<TransportCrypto>,
    ) -> Self {
        let server = Self {
            local_peer_id,
            services: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
            crypto,
            event_sender,
            is_running: Arc::new(RwLock::new(false)),
            tasks: Arc::new(Mutex::new(Vec::new())),
            timeout_monitor: Arc::new(Mutex::new(None)),
            require_encryption: true,
            require_authentication: true,
        };
        
        // Initialize BitCraps service
        let bitcraps_service = server.create_bitcraps_service();
        tokio::spawn({
            let services = server.services.clone();
            async move {
                services.write().await.insert(BITCRAPS_SERVICE_UUID, bitcraps_service);
            }
        });
        
        server
    }
    
    /// Create BitCraps GATT service with secure characteristics
    fn create_bitcraps_service(&self) -> GattService {
        // RX Characteristic (Central writes to us)
        let rx_char = GattCharacteristic::new(
            BITCRAPS_RX_CHAR_UUID,
            CharacteristicProperties {
                write: true,
                write_without_response: true,
                ..Default::default()
            }
        );
        
        // TX Characteristic (We notify centrals)
        let tx_char = GattCharacteristic::new(
            BITCRAPS_TX_CHAR_UUID,
            CharacteristicProperties {
                read: true,
                notify: true,
                ..Default::default()
            }
        );
        
        // Key Exchange Characteristic (For ECDH)
        let key_exchange_char = GattCharacteristic::new(
            BITCRAPS_KEY_EXCHANGE_CHAR_UUID,
            CharacteristicProperties {
                read: true,
                write: true,
                ..Default::default()
            }
        ).with_value(self.crypto.public_key().as_bytes().to_vec());
        
        GattService::new(BITCRAPS_SERVICE_UUID, true)
            .add_characteristic(rx_char)
            .add_characteristic(tx_char)
            .add_characteristic(key_exchange_char)
    }
    
    /// Start the GATT server
    pub async fn start(&self) -> Result<()> {
        if *self.is_running.read().await {
            return Ok(());
        }
        
        log::info!("Starting secure GATT server for peer {:?}", self.local_peer_id);
        
        *self.is_running.write().await = true;
        
        // Start connection timeout monitor
        self.start_connection_monitor().await;
        
        log::info!("Secure GATT server started successfully");
        Ok(())
    }
    
    /// Stop the GATT server
    pub async fn stop(&self) -> Result<()> {
        if !*self.is_running.read().await {
            return Ok(());
        }
        
        log::info!("Stopping secure GATT server");
        
        *self.is_running.write().await = false;
        
        // Stop all background tasks
        let mut tasks = self.tasks.lock().await;
        for task in tasks.drain(..) {
            task.abort();
        }
        
        if let Some(monitor) = self.timeout_monitor.lock().await.take() {
            monitor.abort();
        }
        
        // Disconnect all clients
        let clients = self.clients.read().await.keys().copied().collect::<Vec<_>>();
        for peer_id in clients {
            self.disconnect_client(peer_id, "Server shutting down".to_string()).await;
        }
        
        log::info!("Secure GATT server stopped");
        Ok(())
    }
    
    /// Handle new client connection
    pub async fn handle_client_connected(&self, peer_id: PeerId, client_address: String) -> Result<()> {
        log::info!("New GATT client connected: {:?} at {}", peer_id, client_address);
        
        let client = GattClient::new(peer_id, client_address.clone());
        self.clients.write().await.insert(peer_id, client.clone());
        
        // Perform key exchange
        let crypto_public_key = self.crypto.public_key();
        self.crypto.perform_key_exchange(peer_id, crypto_public_key).await?;
        
        // Send connection event
        match self.event_sender.send(TransportEvent::Connected {
            peer_id,
            address: crate::transport::TransportAddress::Bluetooth(client_address),
        }).await {
            Ok(()) => (),
            Err(BoundedQueueError::QueueFull) => {
                log::warn!("Event queue full, dropping connection event for peer {:?}", peer_id);
            }
            Err(e) => {
                log::error!("Failed to send connection event for peer {:?}: {}", peer_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Handle client disconnection
    pub async fn disconnect_client(&self, peer_id: PeerId, reason: String) -> Result<()> {
        log::info!("GATT client disconnected: {:?} ({})", peer_id, reason);
        
        // Remove client
        self.clients.write().await.remove(&peer_id);
        
        // Remove crypto state
        self.crypto.remove_peer(peer_id).await;
        
        // Send disconnection event
        match self.event_sender.send(TransportEvent::Disconnected {
            peer_id,
            reason: reason.clone(),
        }).await {
            Ok(()) => (),
            Err(BoundedQueueError::QueueFull) => {
                log::warn!("Event queue full, dropping disconnection event for peer {:?}", peer_id);
            }
            Err(e) => {
                log::error!("Failed to send disconnection event for peer {:?}: {}", peer_id, e);
            }
        }
        
        Ok(())
    }
    
    /// Handle characteristic write from client
    pub async fn handle_characteristic_write(
        &self,
        peer_id: PeerId,
        char_uuid: Uuid,
        data: Vec<u8>
    ) -> Result<()> {
        log::debug!("Characteristic write from {:?} to {}: {} bytes", peer_id, char_uuid, data.len());
        
        // Update client activity
        if let Some(client) = self.clients.write().await.get_mut(&peer_id) {
            client.last_activity = Instant::now();
        }
        
        match char_uuid {
            BITCRAPS_RX_CHAR_UUID => {
                self.handle_data_received(peer_id, data).await
            }
            BITCRAPS_KEY_EXCHANGE_CHAR_UUID => {
                self.handle_key_exchange(peer_id, data).await
            }
            _ => {
                log::warn!("Write to unknown characteristic {} from peer {:?}", char_uuid, peer_id);
                Ok(())
            }
        }
    }
    
    /// Handle encrypted data received from client
    async fn handle_data_received(&self, peer_id: PeerId, encrypted_data: Vec<u8>) -> Result<()> {
        // Decrypt the data
        match self.crypto.decrypt_message(peer_id, &encrypted_data).await {
            Ok(decrypted_data) => {
                log::debug!("Successfully decrypted {} bytes from peer {:?}", decrypted_data.len(), peer_id);
                
                // Send data received event
                match self.event_sender.send(TransportEvent::DataReceived {
                    peer_id,
                    data: decrypted_data,
                }).await {
                    Ok(()) => Ok(()),
                    Err(BoundedQueueError::QueueFull) => {
                        log::warn!("Event queue full, dropping data from peer {:?}", peer_id);
                        Ok(())
                    }
                    Err(e) => {
                        log::error!("Failed to send data event for peer {:?}: {}", peer_id, e);
                        Err(Error::Network(format!("Event queue error: {}", e)))
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to decrypt data from peer {:?}: {}", peer_id, e);
                
                // Send error event
                let _ = self.event_sender.send(TransportEvent::Error {
                    peer_id: Some(peer_id),
                    error: format!("Decryption failed: {}", e),
                }).await;
                
                Err(e)
            }
        }
    }
    
    /// Handle key exchange with client
    async fn handle_key_exchange(&self, peer_id: PeerId, key_data: Vec<u8>) -> Result<()> {
        if key_data.len() != 32 {
            return Err(Error::Crypto("Invalid key exchange data length".to_string()));
        }
        
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&key_data);
        
        let peer_public_key = x25519_dalek::PublicKey::from(key_bytes);
        
        // Perform key exchange
        self.crypto.perform_key_exchange(peer_id, peer_public_key).await?;
        
        log::info!("Key exchange completed with peer {:?}", peer_id);
        Ok(())
    }
    
    /// Send data to a specific client
    pub async fn send_to_client(&self, peer_id: PeerId, data: &[u8]) -> Result<()> {
        let start_time = Instant::now();
        
        // Check if client is connected
        let client = {
            let clients = self.clients.read().await;
            clients.get(&peer_id).cloned()
        };
        
        let client = client.ok_or_else(|| {
            Error::Network(format!("Client {:?} not connected", peer_id))
        })?;
        
        // Encrypt the data
        let encrypted_data = self.crypto.encrypt_message(peer_id, data).await?;
        
        // Fragment data if necessary
        let mtu = client.mtu as usize;
        let max_payload = mtu.saturating_sub(3); // ATT header overhead
        
        if encrypted_data.len() <= max_payload {
            // Single notification
            self.send_notification(peer_id, BITCRAPS_TX_CHAR_UUID, encrypted_data).await?;
        } else {
            // Multiple notifications (fragmented)
            for chunk in encrypted_data.chunks(max_payload) {
                self.send_notification(peer_id, BITCRAPS_TX_CHAR_UUID, chunk.to_vec()).await?;
                // Small delay to avoid overwhelming the client
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }
        
        // Update connection metrics
        let latency_ms = start_time.elapsed().as_millis() as u32;
        let _ = self.crypto.update_connection_metrics(peer_id, latency_ms, true).await;
        
        log::debug!("Sent {} encrypted bytes to client {:?}", encrypted_data.len(), peer_id);
        Ok(())
    }
    
    /// Send notification to client (platform-specific implementation)
    async fn send_notification(&self, peer_id: PeerId, char_uuid: Uuid, data: Vec<u8>) -> Result<()> {
        // This would be implemented by platform-specific GATT server
        log::debug!("Sending notification to {:?} on characteristic {}: {} bytes", peer_id, char_uuid, data.len());
        
        // For now, we'll just log it
        // In a real implementation, this would call the platform GATT API
        Ok(())
    }
    
    /// Set client priority
    pub async fn set_client_priority(&self, peer_id: PeerId, priority: ConnectionPriority) -> Result<()> {
        // Update client priority
        if let Some(client) = self.clients.write().await.get_mut(&peer_id) {
            client.priority = priority;
        }
        
        // Update crypto priority
        self.crypto.set_connection_priority(peer_id, priority).await
    }
    
    /// Get connected clients ordered by priority
    pub async fn get_clients_by_priority(&self) -> Vec<PeerId> {
        let clients = self.clients.read().await;
        let mut client_list: Vec<(PeerId, ConnectionPriority)> = clients
            .iter()
            .map(|(peer_id, client)| (*peer_id, client.priority))
            .collect();
        
        // Sort by priority (Critical > High > Normal > Low)
        client_list.sort_by(|a, b| b.1.cmp(&a.1));
        
        client_list.into_iter().map(|(peer_id, _)| peer_id).collect()
    }
    
    /// Start connection timeout monitor
    async fn start_connection_monitor(&self) {
        let clients = self.clients.clone();
        let event_sender = self.event_sender.clone();
        let crypto = self.crypto.clone();
        
        let monitor = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                let now = Instant::now();
                let mut to_disconnect = Vec::new();
                
                // Check for timed-out clients
                {
                    let clients_guard = clients.read().await;
                    for (peer_id, client) in clients_guard.iter() {
                        if now.duration_since(client.last_activity) > CLIENT_CONNECTION_TIMEOUT {
                            to_disconnect.push((*peer_id, "Connection timeout".to_string()));
                        }
                    }
                }
                
                // Disconnect timed-out clients
                for (peer_id, reason) in to_disconnect {
                    log::warn!("Disconnecting client {:?} due to timeout", peer_id);
                    
                    // Remove client
                    clients.write().await.remove(&peer_id);
                    crypto.remove_peer(peer_id).await;
                    
                    // Send disconnection event
                    let _ = event_sender.send(TransportEvent::Disconnected {
                        peer_id,
                        reason,
                    }).await;
                }
            }
        });
        
        *self.timeout_monitor.lock().await = Some(monitor);
    }
    
    /// Get server statistics
    pub async fn get_stats(&self) -> GattServerStats {
        let clients = self.clients.read().await;
        let crypto_stats = self.crypto.get_crypto_stats().await;
        
        let mut priority_counts = HashMap::new();
        for client in clients.values() {
            *priority_counts.entry(client.priority).or_insert(0) += 1;
        }
        
        GattServerStats {
            connected_clients: clients.len(),
            priority_counts,
            crypto_stats,
            is_running: *self.is_running.read().await,
            uptime: Instant::now().duration_since(
                clients.values()
                .min_by_key(|c| c.connected_at)
                .map(|c| c.connected_at)
                .unwrap_or_else(Instant::now)
            ),
        }
    }
}

/// GATT Server statistics
#[derive(Debug, Clone)]
pub struct GattServerStats {
    pub connected_clients: usize,
    pub priority_counts: HashMap<ConnectionPriority, usize>,
    pub crypto_stats: crate::transport::crypto::CryptoStats,
    pub is_running: bool,
    pub uptime: Duration,
}

impl Drop for SecureGattServer {
    fn drop(&mut self) {
        // Clean up tasks
        if let Ok(mut tasks) = self.tasks.try_lock() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
        
        if let Ok(mut monitor) = self.timeout_monitor.try_lock() {
            if let Some(task) = monitor.take() {
                task.abort();
            }
        }
    }
}