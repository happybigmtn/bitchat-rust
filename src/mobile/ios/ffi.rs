//! FFI interface for iOS CoreBluetooth integration
//!
//! This module provides the C FFI interface that bridges Rust to Swift/Objective-C
//! for CoreBluetooth operations. It handles memory management, lifecycle events,
//! and data marshaling between Rust and iOS.

use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::{c_int, c_uint};
use std::sync::{Arc, Mutex, Once};

use crate::mobile::BitCrapsError;

use once_cell::sync::Lazy;

/// iOS BLE peripheral manager instance using safe lazy initialization
static IOS_BLE_MANAGER: Lazy<Arc<Mutex<IosBleManager>>> =
    Lazy::new(|| Arc::new(Mutex::new(IosBleManager::new())));

/// Get the iOS BLE manager
fn get_or_create_manager() -> Arc<Mutex<IosBleManager>> {
    Arc::clone(&IOS_BLE_MANAGER)
}

/// iOS BLE manager state
pub struct IosBleManager {
    /// Active peripheral connections
    connections: HashMap<String, PeripheralConnection>,
    /// Swift callback handlers
    event_callback: Option<extern "C" fn(*const c_char, *const c_void, c_uint)>,
    /// Error callback handler
    error_callback: Option<extern "C" fn(*const c_char)>,
    /// Background mode configuration and state management
    background_task_id: Option<u32>,
    /// State restoration identifier for Core Bluetooth
    restore_identifier: String,
    /// Background advertising data (limited payload)
    background_adv_data: Option<Vec<u8>>,
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

    pub fn set_event_callback(
        &mut self,
        callback: extern "C" fn(*const c_char, *const c_void, c_uint),
    ) {
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
            reason: "Invalid peer ID".to_string(),
        })?;

        self.notify_event(
            "connect_peer",
            peer_id_cstr.as_ptr() as *const c_void,
            peer_id.len() as c_uint,
        );

        Ok(())
    }

    pub fn disconnect_from_peer(&mut self, peer_id: &str) -> Result<(), BitCrapsError> {
        info!("Disconnecting from peer: {}", peer_id);

        if let Some(mut connection) = self.connections.get_mut(peer_id) {
            connection.is_connected = false;
        }

        // Notify Swift layer
        let peer_id_cstr = CString::new(peer_id).map_err(|_| BitCrapsError::InvalidInput {
            reason: "Invalid peer ID".to_string(),
        })?;

        self.notify_event(
            "disconnect_peer",
            peer_id_cstr.as_ptr() as *const c_void,
            peer_id.len() as c_uint,
        );

        Ok(())
    }

    pub fn send_data(&mut self, peer_id: &str, data: &[u8]) -> Result<(), BitCrapsError> {
        if !self.connections.contains_key(peer_id) {
            return Err(BitCrapsError::NotFound {
                item: format!("Peer connection: {}", peer_id),
            });
        }

        debug!("Sending {} bytes to peer {}", data.len(), peer_id);

        // Create send data structure for Swift
        let send_data = SendDataRequest {
            peer_id: peer_id,
            data: data,
        };

        let data_ptr = &send_data as *const SendDataRequest as *const c_void;
        self.notify_event(
            "send_data",
            data_ptr,
            std::mem::size_of::<SendDataRequest>() as c_uint,
        );

        Ok(())
    }

    /// Handle iOS app state transitions for background BLE operation
    pub fn handle_background_transition(
        &mut self,
        entering_background: bool,
    ) -> Result<(), BitCrapsError> {
        if entering_background {
            info!("Transitioning to iOS background mode - implementing BLE restrictions");

            // Begin background task to extend execution time
            self.begin_background_task()?;

            // Switch to background-compatible BLE operations
            self.configure_background_ble()?;

            // Reduce advertising payload for background compliance
            if self.is_advertising {
                self.setup_background_advertising()?;
            }

            // Implement service UUID filtering for scanning
            if self.is_scanning {
                self.setup_background_scanning()?;
            }
        } else {
            info!("Transitioning to iOS foreground mode - enabling full BLE capabilities");

            // End background task
            self.end_background_task();

            // Restore full BLE functionality
            self.configure_foreground_ble()?;
        }

        Ok(())
    }

    fn begin_background_task(&mut self) -> Result<(), BitCrapsError> {
        // Request background execution time from iOS
        // This gives us ~30 seconds to ~10 minutes depending on system conditions

        self.notify_event("begin_background_task", std::ptr::null(), 0);

        // Store a mock background task ID (in real implementation, iOS would provide this)
        self.background_task_id = Some(1001);

        info!("Background task initiated for extended BLE operation");
        Ok(())
    }

    fn end_background_task(&mut self) {
        if let Some(task_id) = self.background_task_id {
            self.notify_event(
                "end_background_task",
                (&task_id as *const u32) as *const c_void,
                std::mem::size_of::<u32>() as c_uint,
            );

            self.background_task_id = None;
            info!("Background task ended");
        }
    }

    fn configure_background_ble(&mut self) -> Result<(), BitCrapsError> {
        // iOS background BLE restrictions:
        // 1. Service UUID filtering required for scanning
        // 2. Limited advertising payload (28 bytes max)
        // 3. No local name in advertising
        // 4. Reduced connection intervals

        info!("Configuring BLE for iOS background operation");

        // Notify Swift layer to configure Core Bluetooth for background
        let config = BackgroundBleConfig {
            require_service_uuid_filtering: true,
            max_advertising_payload: 28,
            allow_local_name: false,
            min_connection_interval: 1.25, // seconds
        };

        let config_ptr = &config as *const BackgroundBleConfig as *const c_void;
        self.notify_event(
            "configure_background_ble",
            config_ptr,
            std::mem::size_of::<BackgroundBleConfig>() as c_uint,
        );

        Ok(())
    }

    fn configure_foreground_ble(&mut self) -> Result<(), BitCrapsError> {
        info!("Configuring BLE for iOS foreground operation");

        // Restore full BLE capabilities
        let config = ForegroundBleConfig {
            allow_full_scanning: true,
            max_advertising_payload: 255,
            allow_local_name: true,
            min_connection_interval: 0.02, // 20ms
        };

        let config_ptr = &config as *const ForegroundBleConfig as *const c_void;
        self.notify_event(
            "configure_foreground_ble",
            config_ptr,
            std::mem::size_of::<ForegroundBleConfig>() as c_uint,
        );

        Ok(())
    }

    fn setup_background_advertising(&mut self) -> Result<(), BitCrapsError> {
        // Create minimal advertising payload for background compliance
        let mut adv_data = Vec::new();

        // Service UUID (16 bytes for 128-bit UUID)
        let service_bytes = self.service_uuid.replace("-", "");
        if service_bytes.len() == 32 {
            // Convert hex string to bytes
            for chunk in service_bytes.as_bytes().chunks(2) {
                if let Ok(byte_str) = std::str::from_utf8(chunk) {
                    if let Ok(byte) = u8::from_str_radix(byte_str, 16) {
                        adv_data.push(byte);
                    }
                }
            }
        }

        // Add minimal game state info (8 bytes max to stay under 28 byte limit)
        adv_data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // Game state flags

        if adv_data.len() <= 28 {
            self.background_adv_data = Some(adv_data.clone());

            // Notify Swift layer
            self.notify_event(
                "setup_background_advertising",
                adv_data.as_ptr() as *const c_void,
                adv_data.len() as c_uint,
            );

            info!(
                "Background advertising configured with {} byte payload",
                adv_data.len()
            );
        } else {
            return Err(BitCrapsError::Platform(
                "Background advertising payload too large".to_string(),
            ));
        }

        Ok(())
    }

    fn setup_background_scanning(&mut self) -> Result<(), BitCrapsError> {
        // Configure scanning with service UUID filtering (required for iOS background)
        let service_uuid_cstr = CString::new(self.service_uuid.clone())
            .map_err(|_| BitCrapsError::Platform("Invalid service UUID".to_string()))?;

        self.notify_event(
            "setup_background_scanning",
            service_uuid_cstr.as_ptr() as *const c_void,
            self.service_uuid.len() as c_uint,
        );

        info!("Background scanning configured with service UUID filtering");
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
    callback: extern "C" fn(*const c_char, *const c_void, c_uint),
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
pub extern "C" fn ios_ble_set_error_callback(callback: extern "C" fn(*const c_char)) -> c_int {
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
    data_len: c_uint,
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

    let data_slice = unsafe { std::slice::from_raw_parts(data, data_len as usize) };

    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.send_data(peer_id_str, data_slice) {
            Ok(()) => {
                debug!(
                    "iOS BLE data send initiated for peer: {} ({} bytes)",
                    peer_id_str, data_len
                );
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
    data_len: c_uint,
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

/// Configuration for iOS background BLE operation
#[repr(C)]
struct BackgroundBleConfig {
    require_service_uuid_filtering: bool,
    max_advertising_payload: u8,
    allow_local_name: bool,
    min_connection_interval: f64,
}

/// Configuration for iOS foreground BLE operation
#[repr(C)]
struct ForegroundBleConfig {
    allow_full_scanning: bool,
    max_advertising_payload: u8,
    allow_local_name: bool,
    min_connection_interval: f64,
}

/// Handle iOS app state changes (called from Swift)
#[no_mangle]
pub extern "C" fn ios_ble_handle_app_state_change(entering_background: bool) -> c_int {
    let manager = get_or_create_manager();
    if let Ok(mut mgr) = manager.lock() {
        match mgr.handle_background_transition(entering_background) {
            Ok(()) => {
                if entering_background {
                    info!("iOS BLE successfully transitioned to background mode");
                } else {
                    info!("iOS BLE successfully transitioned to foreground mode");
                }
                return 1;
            }
            Err(e) => {
                error!("Failed to handle iOS app state change: {}", e);
                mgr.notify_error(&format!("App state transition failed: {}", e));
                return 0;
            }
        }
    }

    error!("Failed to access iOS BLE manager for app state change");
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
