//! iOS BLE peripheral implementation
//! 
//! This module provides the Rust-side implementation for iOS CoreBluetooth
//! peripheral operations, managing the state and data flow between the Rust
//! core and the iOS BLE stack.

use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use log::{debug, error, info, warn};

use crate::mobile::{BitCrapsError, PeerInfo, NetworkStats};
use crate::protocol::optimized_binary::{BinaryMessage, MessageType};

/// iOS BLE peripheral manager for BitCraps
pub struct IosBlePeripheral {
    /// Peripheral state
    state: Arc<RwLock<PeripheralState>>,
    /// Active peer connections
    peers: Arc<Mutex<HashMap<String, PeerConnection>>>,
    /// Statistics tracking
    stats: Arc<Mutex<NetworkStats>>,
    /// Service configuration
    config: PeripheralConfig,
}

/// Internal peripheral state
#[derive(Debug, Clone)]
pub struct PeripheralState {
    pub is_advertising: bool,
    pub is_scanning: bool,
    pub bluetooth_powered: bool,
    pub background_mode: bool,
    pub service_uuid: String,
    pub local_name: String,
}

/// Peer connection information
#[derive(Debug, Clone)]
pub struct PeerConnection {
    pub peer_id: String,
    pub peripheral_id: String,  // iOS CBPeripheral identifier
    pub is_connected: bool,
    pub rssi: i32,
    pub connected_at: u64,
    pub last_activity: u64,
    pub tx_characteristic: Option<String>,
    pub rx_characteristic: Option<String>,
}

/// Configuration for the iOS BLE peripheral
#[derive(Debug, Clone)]
pub struct PeripheralConfig {
    pub service_uuid: String,
    pub tx_characteristic_uuid: String,
    pub rx_characteristic_uuid: String,
    pub local_name: String,
    pub background_mode: bool,
    pub max_connections: u32,
    pub connection_timeout_sec: u64,
}

impl Default for PeripheralConfig {
    fn default() -> Self {
        Self {
            service_uuid: "12345678-1234-5678-1234-567812345678".to_string(),
            tx_characteristic_uuid: "12345678-1234-5678-1234-567812345679".to_string(),
            rx_characteristic_uuid: "12345678-1234-5678-1234-567812345680".to_string(),
            local_name: "BitCraps-Node".to_string(),
            background_mode: true,
            max_connections: 8,
            connection_timeout_sec: 300,
        }
    }
}

impl IosBlePeripheral {
    /// Create a new iOS BLE peripheral instance
    pub fn new(config: PeripheralConfig) -> Result<Self, BitCrapsError> {
        let state = PeripheralState {
            is_advertising: false,
            is_scanning: false,
            bluetooth_powered: false,
            background_mode: config.background_mode,
            service_uuid: config.service_uuid.clone(),
            local_name: config.local_name.clone(),
        };
        
        let stats = NetworkStats {
            peers_discovered: 0,
            active_connections: 0,
            bytes_sent: 0,
            bytes_received: 0,
            packets_dropped: 0,
            average_latency_ms: 0.0,
        };
        
        Ok(Self {
            state: Arc::new(RwLock::new(state)),
            peers: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(stats)),
            config,
        })
    }
    
    /// Start BLE advertising
    pub fn start_advertising(&self) -> Result<(), BitCrapsError> {
        info!("Starting iOS BLE advertising");
        
        // Update internal state
        if let Ok(mut state) = self.state.write() {
            if state.is_advertising {
                return Ok(()); // Already advertising
            }
            
            if !state.bluetooth_powered {
                return Err(BitCrapsError::BluetoothError {
                    reason: "Bluetooth not powered on".to_string()
                });
            }
            
            state.is_advertising = true;
        } else {
            return Err(BitCrapsError::BluetoothError {
                reason: "Failed to acquire state lock".to_string()
            });
        }
        
        // Call FFI to start advertising on iOS side
        let result = unsafe { super::ffi::ios_ble_start_advertising() };
        if result != 1 {
            // Revert state on failure
            if let Ok(mut state) = self.state.write() {
                state.is_advertising = false;
            }
            return Err(BitCrapsError::BluetoothError {
                reason: "Failed to start iOS BLE advertising".to_string()
            });
        }
        
        info!("iOS BLE advertising started successfully");
        Ok(())
    }
    
    /// Stop BLE advertising
    pub fn stop_advertising(&self) -> Result<(), BitCrapsError> {
        info!("Stopping iOS BLE advertising");
        
        // Update internal state
        if let Ok(mut state) = self.state.write() {
            if !state.is_advertising {
                return Ok(()); // Already stopped
            }
            
            state.is_advertising = false;
        }
        
        // Call FFI to stop advertising on iOS side
        let result = unsafe { super::ffi::ios_ble_stop_advertising() };
        if result != 1 {
            warn!("iOS BLE advertising stop may have failed");
        }
        
        info!("iOS BLE advertising stopped");
        Ok(())
    }
    
    /// Start BLE scanning for peers
    pub fn start_scanning(&self) -> Result<(), BitCrapsError> {
        info!("Starting iOS BLE scanning");
        
        // Update internal state
        if let Ok(mut state) = self.state.write() {
            if state.is_scanning {
                return Ok(()); // Already scanning
            }
            
            if !state.bluetooth_powered {
                return Err(BitCrapsError::BluetoothError {
                    reason: "Bluetooth not powered on".to_string()
                });
            }
            
            state.is_scanning = true;
        } else {
            return Err(BitCrapsError::BluetoothError {
                reason: "Failed to acquire state lock".to_string()
            });
        }
        
        // Call FFI to start scanning on iOS side
        let result = unsafe { super::ffi::ios_ble_start_scanning() };
        if result != 1 {
            // Revert state on failure
            if let Ok(mut state) = self.state.write() {
                state.is_scanning = false;
            }
            return Err(BitCrapsError::BluetoothError {
                reason: "Failed to start iOS BLE scanning".to_string()
            });
        }
        
        info!("iOS BLE scanning started successfully");
        Ok(())
    }
    
    /// Stop BLE scanning
    pub fn stop_scanning(&self) -> Result<(), BitCrapsError> {
        info!("Stopping iOS BLE scanning");
        
        // Update internal state
        if let Ok(mut state) = self.state.write() {
            if !state.is_scanning {
                return Ok(()); // Already stopped
            }
            
            state.is_scanning = false;
        }
        
        // Call FFI to stop scanning on iOS side
        let result = unsafe { super::ffi::ios_ble_stop_scanning() };
        if result != 1 {
            warn!("iOS BLE scanning stop may have failed");
        }
        
        info!("iOS BLE scanning stopped");
        Ok(())
    }
    
    /// Connect to a discovered peer
    pub fn connect_to_peer(&self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Connecting to peer: {}", peer_id);
        
        // Check if we're already connected
        if let Ok(peers) = self.peers.lock() {
            if let Some(peer) = peers.get(peer_id) {
                if peer.is_connected {
                    return Ok(()); // Already connected
                }
            }
        }
        
        // Check connection limits
        if let Ok(peers) = self.peers.lock() {
            let active_connections = peers.values().filter(|p| p.is_connected).count();
            if active_connections >= self.config.max_connections as usize {
                return Err(BitCrapsError::NetworkError {
                    reason: format!("Maximum connections reached: {}", self.config.max_connections)
                });
            }
        }
        
        // Call FFI to initiate connection on iOS side
        let peer_id_cstr = std::ffi::CString::new(peer_id).map_err(|_| {
            BitCrapsError::InvalidInput {
                reason: "Invalid peer ID".to_string()
            }
        })?;
        
        let result = unsafe { super::ffi::ios_ble_connect_peer(peer_id_cstr.as_ptr()) };
        if result != 1 {
            return Err(BitCrapsError::BluetoothError {
                reason: "Failed to initiate iOS BLE connection".to_string()
            });
        }
        
        // Create pending connection entry
        if let Ok(mut peers) = self.peers.lock() {
            let connection = PeerConnection {
                peer_id: peer_id.to_string(),
                peripheral_id: peer_id.to_string(), // Will be updated when iOS provides actual ID
                is_connected: false, // Will be set to true on successful connection
                rssi: 0,
                connected_at: 0, // Will be set on successful connection
                last_activity: current_timestamp(),
                tx_characteristic: None,
                rx_characteristic: None,
            };
            
            peers.insert(peer_id.to_string(), connection);
        }
        
        info!("iOS BLE connection initiated for peer: {}", peer_id);
        Ok(())
    }
    
    /// Disconnect from a peer
    pub fn disconnect_from_peer(&self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Disconnecting from peer: {}", peer_id);
        
        // Call FFI to initiate disconnection on iOS side
        let peer_id_cstr = std::ffi::CString::new(peer_id).map_err(|_| {
            BitCrapsError::InvalidInput {
                reason: "Invalid peer ID".to_string()
            }
        })?;
        
        let result = unsafe { super::ffi::ios_ble_disconnect_peer(peer_id_cstr.as_ptr()) };
        if result != 1 {
            warn!("iOS BLE disconnection may have failed for peer: {}", peer_id);
        }
        
        // Update peer state
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(mut peer) = peers.get_mut(peer_id) {
                peer.is_connected = false;
                peer.last_activity = current_timestamp();
            }
        }
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            if stats.active_connections > 0 {
                stats.active_connections -= 1;
            }
        }
        
        info!("Peer disconnection completed: {}", peer_id);
        Ok(())
    }
    
    /// Send data to a connected peer
    pub fn send_data(&self, peer_id: &str, message: &BinaryMessage) -> Result<(), BitCrapsError> {
        debug!("Sending data to peer: {} (type: {:?})", peer_id, message.message_type);
        
        // Check if peer is connected
        let is_connected = if let Ok(peers) = self.peers.lock() {
            peers.get(peer_id).map_or(false, |p| p.is_connected)
        } else {
            false
        };
        
        if !is_connected {
            return Err(BitCrapsError::NotFound {
                item: format!("Connected peer: {}", peer_id)
            });
        }
        
        // Serialize the message
        let data = message.to_bytes().map_err(|e| {
            BitCrapsError::CryptoError {
                reason: format!("Message serialization failed: {}", e)
            }
        })?;
        
        // Call FFI to send data on iOS side
        let peer_id_cstr = std::ffi::CString::new(peer_id).map_err(|_| {
            BitCrapsError::InvalidInput {
                reason: "Invalid peer ID".to_string()
            }
        })?;
        
        let result = unsafe {
            super::ffi::ios_ble_send_data(
                peer_id_cstr.as_ptr(),
                data.as_ptr(),
                data.len() as u32
            )
        };
        
        if result != 1 {
            // Update statistics on failure
            if let Ok(mut stats) = self.stats.lock() {
                stats.packets_dropped += 1;
            }
            
            return Err(BitCrapsError::NetworkError {
                reason: "Failed to send data via iOS BLE".to_string()
            });
        }
        
        // Update statistics on success
        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_sent += data.len() as u64;
        }
        
        // Update peer activity
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(mut peer) = peers.get_mut(peer_id) {
                peer.last_activity = current_timestamp();
            }
        }
        
        debug!("Data sent successfully to peer: {} ({} bytes)", peer_id, data.len());
        Ok(())
    }
    
    /// Handle peer discovery event from iOS
    pub fn handle_peer_discovered(&self, peer_id: &str, rssi: i32, advertisement_data: &[u8]) -> Result<PeerInfo, BitCrapsError> {
        debug!("Peer discovered: {} (RSSI: {})", peer_id, rssi);
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.peers_discovered += 1;
        }
        
        // Create peer info
        let peer_info = PeerInfo {
            peer_id: peer_id.to_string(),
            display_name: None, // iOS background limitations prevent display name discovery
            signal_strength: rssi.abs() as u32,
            last_seen: current_timestamp(),
            is_connected: false,
        };
        
        // Store or update peer in connections
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(existing_peer) = peers.get_mut(peer_id) {
                existing_peer.rssi = rssi;
                existing_peer.last_activity = current_timestamp();
            } else {
                let connection = PeerConnection {
                    peer_id: peer_id.to_string(),
                    peripheral_id: peer_id.to_string(),
                    is_connected: false,
                    rssi,
                    connected_at: 0,
                    last_activity: current_timestamp(),
                    tx_characteristic: None,
                    rx_characteristic: None,
                };
                
                peers.insert(peer_id.to_string(), connection);
            }
        }
        
        Ok(peer_info)
    }
    
    /// Handle peer connection event from iOS
    pub fn handle_peer_connected(&self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Peer connected: {}", peer_id);
        
        // Update peer state
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(mut peer) = peers.get_mut(peer_id) {
                peer.is_connected = true;
                peer.connected_at = current_timestamp();
                peer.last_activity = current_timestamp();
            }
        }
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.active_connections += 1;
        }
        
        Ok(())
    }
    
    /// Handle peer disconnection event from iOS
    pub fn handle_peer_disconnected(&self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Peer disconnected: {}", peer_id);
        
        // Update peer state
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(mut peer) = peers.get_mut(peer_id) {
                peer.is_connected = false;
                peer.last_activity = current_timestamp();
            }
        }
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            if stats.active_connections > 0 {
                stats.active_connections -= 1;
            }
        }
        
        Ok(())
    }
    
    /// Handle data received from peer via iOS
    pub fn handle_data_received(&self, peer_id: &str, data: &[u8]) -> Result<BinaryMessage, BitCrapsError> {
        debug!("Data received from peer: {} ({} bytes)", peer_id, data.len());
        
        // Parse the binary message
        let message = BinaryMessage::from_bytes(data).map_err(|e| {
            BitCrapsError::CryptoError {
                reason: format!("Message deserialization failed: {}", e)
            }
        })?;
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.bytes_received += data.len() as u64;
        }
        
        // Update peer activity
        if let Ok(mut peers) = self.peers.lock() {
            if let Some(mut peer) = peers.get_mut(peer_id) {
                peer.last_activity = current_timestamp();
            }
        }
        
        debug!("Message parsed successfully from peer: {} (type: {:?})", peer_id, message.message_type);
        Ok(message)
    }
    
    /// Handle Bluetooth state change from iOS
    pub fn handle_bluetooth_state_changed(&self, powered_on: bool) -> Result<(), BitCrapsError> {
        info!("Bluetooth state changed: powered_on={}", powered_on);
        
        // Update internal state
        if let Ok(mut state) = self.state.write() {
            state.bluetooth_powered = powered_on;
            
            // If Bluetooth is turned off, stop advertising and scanning
            if !powered_on {
                state.is_advertising = false;
                state.is_scanning = false;
            }
        }
        
        // If Bluetooth is turned off, disconnect all peers
        if !powered_on {
            if let Ok(mut peers) = self.peers.lock() {
                for peer in peers.values_mut() {
                    peer.is_connected = false;
                    peer.last_activity = current_timestamp();
                }
            }
            
            // Reset connection count
            if let Ok(mut stats) = self.stats.lock() {
                stats.active_connections = 0;
            }
        }
        
        Ok(())
    }
    
    /// Get current peripheral state
    pub fn get_state(&self) -> Result<PeripheralState, BitCrapsError> {
        self.state.read()
            .map(|state| state.clone())
            .map_err(|_| BitCrapsError::NetworkError {
                reason: "Failed to read peripheral state".to_string()
            })
    }
    
    /// Get connected peers
    pub fn get_connected_peers(&self) -> Result<Vec<PeerInfo>, BitCrapsError> {
        let peers = self.peers.lock().map_err(|_| BitCrapsError::NetworkError {
            reason: "Failed to acquire peers lock".to_string()
        })?;
        
        let connected_peers = peers.values()
            .filter(|p| p.is_connected)
            .map(|p| PeerInfo {
                peer_id: p.peer_id.clone(),
                display_name: None,
                signal_strength: p.rssi.abs() as u32,
                last_seen: p.last_activity,
                is_connected: p.is_connected,
            })
            .collect();
        
        Ok(connected_peers)
    }
    
    /// Get network statistics
    pub fn get_network_stats(&self) -> Result<NetworkStats, BitCrapsError> {
        self.stats.lock()
            .map(|stats| stats.clone())
            .map_err(|_| BitCrapsError::NetworkError {
                reason: "Failed to acquire stats lock".to_string()
            })
    }
    
    /// Cleanup expired peers
    pub fn cleanup_expired_peers(&self, max_age_sec: u64) -> Result<u32, BitCrapsError> {
        let current_time = current_timestamp();
        let cutoff_time = current_time - max_age_sec;
        
        let mut peers = self.peers.lock().map_err(|_| BitCrapsError::NetworkError {
            reason: "Failed to acquire peers lock".to_string()
        })?;
        
        let initial_count = peers.len();
        
        // Remove expired peers (only if not connected)
        peers.retain(|_, peer| {
            peer.is_connected || peer.last_activity >= cutoff_time
        });
        
        let removed_count = initial_count - peers.len();
        
        if removed_count > 0 {
            debug!("Cleaned up {} expired peers", removed_count);
        }
        
        Ok(removed_count as u32)
    }
}

/// Get current timestamp in seconds since Unix epoch
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_peripheral_creation() {
        let config = PeripheralConfig::default();
        let peripheral = IosBlePeripheral::new(config).unwrap();
        
        let state = peripheral.get_state().unwrap();
        assert!(!state.is_advertising);
        assert!(!state.is_scanning);
        assert!(!state.bluetooth_powered);
    }
    
    #[test]
    fn test_peer_discovery() {
        let config = PeripheralConfig::default();
        let peripheral = IosBlePeripheral::new(config).unwrap();
        
        let peer_info = peripheral.handle_peer_discovered("test-peer", -50, &[]).unwrap();
        
        assert_eq!(peer_info.peer_id, "test-peer");
        assert_eq!(peer_info.signal_strength, 50);
        assert!(!peer_info.is_connected);
    }
    
    #[test]
    fn test_peer_lifecycle() {
        let config = PeripheralConfig::default();
        let peripheral = IosBlePeripheral::new(config).unwrap();
        
        // Discover peer
        let _peer_info = peripheral.handle_peer_discovered("test-peer", -50, &[]).unwrap();
        
        // Connect peer
        peripheral.handle_peer_connected("test-peer").unwrap();
        
        let connected_peers = peripheral.get_connected_peers().unwrap();
        assert_eq!(connected_peers.len(), 1);
        assert!(connected_peers[0].is_connected);
        
        // Disconnect peer
        peripheral.handle_peer_disconnected("test-peer").unwrap();
        
        let connected_peers = peripheral.get_connected_peers().unwrap();
        assert_eq!(connected_peers.len(), 0);
    }
}