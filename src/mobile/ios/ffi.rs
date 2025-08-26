//! FFI interface for iOS CoreBluetooth integration
//! 
//! This module provides the C FFI interface that bridges Rust to Swift/Objective-C
//! for CoreBluetooth operations. It handles memory management, lifecycle events,
//! and data marshaling between Rust and iOS.

use std::ffi::{CStr, CString, c_char, c_void};
use std::os::raw::{c_int, c_uint};
use std::sync::{Arc, Mutex, Once};
use std::collections::HashMap;
use log::{debug, error, info, warn};

use crate::mobile::BitCrapsError;

/// iOS BLE peripheral manager instance
static mut IOS_BLE_MANAGER: Option<Arc<Mutex<IosBleManager>>> = None;
static INIT: Once = Once::new();

/// Initialize the iOS BLE manager (called once)
fn get_or_create_manager() -> Arc<Mutex<IosBleManager>> {
    unsafe {
        INIT.call_once(|| {
            IOS_BLE_MANAGER = Some(Arc::new(Mutex::new(IosBleManager::new())));
        });
        IOS_BLE_MANAGER.as_ref().unwrap().clone()
    }
}

/// iOS BLE manager state
pub struct IosBleManager {
    /// Active peripheral connections
    connections: HashMap<String, PeripheralConnection>,
    /// Swift callback handlers
    event_callback: Option<extern "C" fn(*const c_char, *const c_void, c_uint)>,
    /// Error callback handler  
    error_callback: Option<extern "C" fn(*const c_char)>,
    /// Is currently advertising
    is_advertising: bool,
    /// Is currently scanning
    is_scanning: bool,
    /// Service UUID for BitCraps
    service_uuid: String,
}

/// Peripheral connection state
#[derive(Debug, Clone)]
struct PeripheralConnection {
    peer_id: String,
    is_connected: bool,
    rssi: i32,
    last_seen: u64,
}

impl IosBleManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            event_callback: None,
            error_callback: None,
            is_advertising: false,
            is_scanning: false,
            service_uuid: "12345678-1234-5678-1234-567812345678".to_string(),
        }
    }
    
    pub fn set_event_callback(&mut self, callback: extern "C" fn(*const c_char, *const c_void, c_uint)) {
        self.event_callback = Some(callback);
    }
    
    pub fn set_error_callback(&mut self, callback: extern "C" fn(*const c_char)) {
        self.error_callback = Some(callback);
    }
    
    pub fn start_advertising(&mut self) -> Result<(), BitCrapsError> {
        if self.is_advertising {
            return Ok(());
        }
        
        info!("Starting iOS BLE advertising");
        self.is_advertising = true;
        
        // Notify Swift layer
        self.notify_event("advertising_started", std::ptr::null(), 0);
        Ok(())
    }
    
    pub fn stop_advertising(&mut self) -> Result<(), BitCrapsError> {
        if !self.is_advertising {
            return Ok(());
        }
        
        info!("Stopping iOS BLE advertising");
        self.is_advertising = false;
        
        // Notify Swift layer
        self.notify_event("advertising_stopped", std::ptr::null(), 0);
        Ok(())
    }
    
    pub fn start_scanning(&mut self) -> Result<(), BitCrapsError> {
        if self.is_scanning {
            return Ok(());
        }
        
        info!("Starting iOS BLE scanning");
        self.is_scanning = true;
        
        // Notify Swift layer
        self.notify_event("scanning_started", std::ptr::null(), 0);
        Ok(())
    }
    
    pub fn stop_scanning(&mut self) -> Result<(), BitCrapsError> {
        if !self.is_scanning {
            return Ok(());
        }
        
        info!("Stopping iOS BLE scanning");
        self.is_scanning = false;
        
        // Notify Swift layer
        self.notify_event("scanning_stopped", std::ptr::null(), 0);
        Ok(())
    }
    
    pub fn connect_to_peer(&mut self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Connecting to peer: {}", peer_id);
        
        let connection = PeripheralConnection {
            peer_id: peer_id.to_string(),
            is_connected: false,
            rssi: -50, // Default value
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        
        self.connections.insert(peer_id.to_string(), connection);
        
        // Notify Swift layer to initiate connection
        let peer_id_cstr = CString::new(peer_id).map_err(|_| BitCrapsError::InvalidInput { 
            reason: "Invalid peer ID".to_string() 
        })?;
        
        self.notify_event("connect_peer", peer_id_cstr.as_ptr() as *const c_void, peer_id.len() as c_uint);
        
        Ok(())
    }
    
    pub fn disconnect_from_peer(&mut self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Disconnecting from peer: {}", peer_id);
        
        if let Some(mut connection) = self.connections.get_mut(peer_id) {
            connection.is_connected = false;
        }
        
        // Notify Swift layer
        let peer_id_cstr = CString::new(peer_id).map_err(|_| BitCrapsError::InvalidInput { 
            reason: "Invalid peer ID".to_string() 
        })?;
        
        self.notify_event("disconnect_peer", peer_id_cstr.as_ptr() as *const c_void, peer_id.len() as c_uint);
        
        Ok(())
    }
    
    pub fn send_data(&mut self, peer_id: &str, data: &[u8]) -> Result<(), BitCrapsError> {
        if !self.connections.contains_key(peer_id) {
            return Err(BitCrapsError::NotFound { 
                item: format!("Peer connection: {}", peer_id) 
            });
        }
        
        debug!("Sending {} bytes to peer {}", data.len(), peer_id);
        
        // Create send data structure for Swift
        let send_data = SendDataRequest {
            peer_id: peer_id,
            data: data,
        };
        
        let data_ptr = &send_data as *const SendDataRequest as *const c_void;
        self.notify_event("send_data", data_ptr, std::mem::size_of::<SendDataRequest>() as c_uint);
        
        Ok(())
    }
    
    fn notify_event(&self, event_type: &str, data: *const c_void, data_len: c_uint) {
        if let Some(callback) = self.event_callback {
            if let Ok(event_cstr) = CString::new(event_type) {
                callback(event_cstr.as_ptr(), data, data_len);
            }
        }
    }
    
    fn notify_error(&self, error: &str) {
        if let Some(callback) = self.error_callback {
            if let Ok(error_cstr) = CString::new(error) {
                callback(error_cstr.as_ptr());
            }
        }
    }
}

/// Data structure for sending data requests to Swift
#[repr(C)]
struct SendDataRequest {
    peer_id: *const c_char,
    data: *const u8,
}

// MARK: - C FFI Functions

/// Initialize the iOS BLE manager
#[no_mangle]
pub extern "C" fn ios_ble_initialize() -> c_int {
    env_logger::try_init().ok(); // Initialize logging if not already done
    
    let manager = get_or_create_manager();
    if manager.lock().is_ok() {
        info!("iOS BLE manager initialized");
        return 1; // Success
    }
    
    error!("Failed to initialize iOS BLE manager");
    0 // Failure
}

/// Set event callback for iOS BLE events
#[no_mangle]
pub extern "C" fn ios_ble_set_event_callback(
    callback: extern "C" fn(*const c_char, *const c_void, c_uint)
) -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        mgr.set_event_callback(callback);
        debug!("iOS BLE event callback set");
        return 1;
    }
    
    error!("Failed to set iOS BLE event callback");
    0
}

/// Set error callback for iOS BLE errors
#[no_mangle]
pub extern "C" fn ios_ble_set_error_callback(
    callback: extern "C" fn(*const c_char)
) -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        mgr.set_error_callback(callback);
        debug!("iOS BLE error callback set");
        return 1;
    }
    
    error!("Failed to set iOS BLE error callback");
    0
}

/// Start BLE advertising
#[no_mangle]
pub extern "C" fn ios_ble_start_advertising() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.start_advertising() {
            Ok(()) => {
                debug!("iOS BLE advertising started successfully");
                return 1;
            }
            Err(e) => {
                error!("Failed to start iOS BLE advertising: {}", e);
                mgr.notify_error(&format!("Failed to start advertising: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for advertising");
    0
}

/// Stop BLE advertising
#[no_mangle]
pub extern "C" fn ios_ble_stop_advertising() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.stop_advertising() {
            Ok(()) => {
                debug!("iOS BLE advertising stopped successfully");
                return 1;
            }
            Err(e) => {
                error!("Failed to stop iOS BLE advertising: {}", e);
                mgr.notify_error(&format!("Failed to stop advertising: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for advertising");
    0
}

/// Start BLE scanning
#[no_mangle]
pub extern "C" fn ios_ble_start_scanning() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.start_scanning() {
            Ok(()) => {
                debug!("iOS BLE scanning started successfully");
                return 1;
            }
            Err(e) => {
                error!("Failed to start iOS BLE scanning: {}", e);
                mgr.notify_error(&format!("Failed to start scanning: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for scanning");
    0
}

/// Stop BLE scanning
#[no_mangle]
pub extern "C" fn ios_ble_stop_scanning() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.stop_scanning() {
            Ok(()) => {
                debug!("iOS BLE scanning stopped successfully");
                return 1;
            }
            Err(e) => {
                error!("Failed to stop iOS BLE scanning: {}", e);
                mgr.notify_error(&format!("Failed to stop scanning: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for scanning");
    0
}

/// Connect to a specific peer
#[no_mangle]
pub extern "C" fn ios_ble_connect_peer(peer_id: *const c_char) -> c_int {
    if peer_id.is_null() {
        error!("Null peer_id provided to ios_ble_connect_peer");
        return 0;
    }
    
    let peer_id_str = match unsafe { CStr::from_ptr(peer_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid peer_id string: {}", e);
            return 0;
        }
    };
    
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.connect_to_peer(peer_id_str) {
            Ok(()) => {
                debug!("iOS BLE connection initiated for peer: {}", peer_id_str);
                return 1;
            }
            Err(e) => {
                error!("Failed to connect to peer {}: {}", peer_id_str, e);
                mgr.notify_error(&format!("Failed to connect to peer: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for connection");
    0
}

/// Disconnect from a specific peer
#[no_mangle]
pub extern "C" fn ios_ble_disconnect_peer(peer_id: *const c_char) -> c_int {
    if peer_id.is_null() {
        error!("Null peer_id provided to ios_ble_disconnect_peer");
        return 0;
    }
    
    let peer_id_str = match unsafe { CStr::from_ptr(peer_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid peer_id string: {}", e);
            return 0;
        }
    };
    
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.disconnect_from_peer(peer_id_str) {
            Ok(()) => {
                debug!("iOS BLE disconnection initiated for peer: {}", peer_id_str);
                return 1;
            }
            Err(e) => {
                error!("Failed to disconnect from peer {}: {}", peer_id_str, e);
                mgr.notify_error(&format!("Failed to disconnect from peer: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for disconnection");
    0
}

/// Send data to a specific peer
#[no_mangle]
pub extern "C" fn ios_ble_send_data(
    peer_id: *const c_char,
    data: *const u8,
    data_len: c_uint
) -> c_int {
    if peer_id.is_null() || data.is_null() || data_len == 0 {
        error!("Invalid parameters provided to ios_ble_send_data");
        return 0;
    }
    
    let peer_id_str = match unsafe { CStr::from_ptr(peer_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid peer_id string: {}", e);
            return 0;
        }
    };
    
    let data_slice = unsafe { 
        std::slice::from_raw_parts(data, data_len as usize) 
    };
    
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.send_data(peer_id_str, data_slice) {
            Ok(()) => {
                debug!("iOS BLE data send initiated for peer: {} ({} bytes)", peer_id_str, data_len);
                return 1;
            }
            Err(e) => {
                error!("Failed to send data to peer {}: {}", peer_id_str, e);
                mgr.notify_error(&format!("Failed to send data: {}", e));
                return 0;
            }
        }
    }
    
    error!("Failed to access iOS BLE manager for data send");
    0
}

/// Handle events from iOS (called by Swift/Objective-C)
#[no_mangle]
pub extern "C" fn ios_ble_handle_event(
    event_type: *const c_char,
    event_data: *const c_void,
    data_len: c_uint
) -> c_int {
    if event_type.is_null() {
        error!("Null event_type provided to ios_ble_handle_event");
        return 0;
    }
    
    let event_str = match unsafe { CStr::from_ptr(event_type) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            error!("Invalid event_type string: {}", e);
            return 0;
        }
    };
    
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match event_str {
            "peer_discovered" => {
                if !event_data.is_null() && data_len > 0 {
                    // Handle peer discovery
                    debug!("Peer discovered event received");
                    return 1;
                }
            }
            "peer_connected" => {
                if !event_data.is_null() && data_len > 0 {
                    // Handle peer connection
                    debug!("Peer connected event received");
                    return 1;
                }
            }
            "peer_disconnected" => {
                if !event_data.is_null() && data_len > 0 {
                    // Handle peer disconnection
                    debug!("Peer disconnected event received");
                    return 1;
                }
            }
            "data_received" => {
                if !event_data.is_null() && data_len > 0 {
                    // Handle received data
                    debug!("Data received event: {} bytes", data_len);
                    return 1;
                }
            }
            "bluetooth_state_changed" => {
                debug!("Bluetooth state changed event received");
                return 1;
            }
            _ => {
                warn!("Unknown event type received: {}", event_str);
                return 1; // Not an error, just unknown
            }
        }
    }
    
    error!("Failed to handle iOS BLE event: {}", event_str);
    0
}

/// Get the current status of the iOS BLE manager
#[no_mangle]
pub extern "C" fn ios_ble_get_status() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mgr) = manager.lock() {
        let mut status = 0;
        if mgr.is_advertising {
            status |= 1; // Bit 0: advertising
        }
        if mgr.is_scanning {
            status |= 2; // Bit 1: scanning
        }
        if !mgr.connections.is_empty() {
            status |= 4; // Bit 2: has connections
        }
        return status;
    }
    
    error!("Failed to get iOS BLE manager status");
    -1 // Error indicator
}

/// Cleanup and shutdown the iOS BLE manager
#[no_mangle]
pub extern "C" fn ios_ble_shutdown() -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        let _ = mgr.stop_advertising();
        let _ = mgr.stop_scanning();
        mgr.connections.clear();
        mgr.event_callback = None;
        mgr.error_callback = None;
        
        info!("iOS BLE manager shutdown completed");
        return 1;
    }
    
    error!("Failed to shutdown iOS BLE manager");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_manager_creation() {
        let manager = IosBleManager::new();
        assert!(!manager.is_advertising);
        assert!(!manager.is_scanning);
        assert!(manager.connections.is_empty());
    }
    
    #[test]
    fn test_advertising_lifecycle() {
        let mut manager = IosBleManager::new();
        
        // Test start advertising
        assert!(manager.start_advertising().is_ok());
        assert!(manager.is_advertising);
        
        // Test stop advertising
        assert!(manager.stop_advertising().is_ok());
        assert!(!manager.is_advertising);
    }
    
    #[test]
    fn test_scanning_lifecycle() {
        let mut manager = IosBleManager::new();
        
        // Test start scanning
        assert!(manager.start_scanning().is_ok());
        assert!(manager.is_scanning);
        
        // Test stop scanning
        assert!(manager.stop_scanning().is_ok());
        assert!(!manager.is_scanning);
    }
}