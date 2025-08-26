//! iOS state management for BLE operations
//! 
//! This module manages the application state for iOS, handling background/foreground
//! transitions, permission changes, and iOS-specific BLE constraints.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{debug, error, info, warn};

use crate::mobile::BitCrapsError;

/// iOS-specific application state manager
pub struct IosStateManager {
    /// Current application state
    app_state: Arc<RwLock<IosAppState>>,
    /// BLE permission state
    permissions: Arc<Mutex<PermissionState>>,
    /// Background limitations tracker
    background_constraints: Arc<Mutex<BackgroundConstraints>>,
    /// State change callbacks
    callbacks: Arc<Mutex<Vec<Box<dyn Fn(&IosAppState) + Send + Sync>>>>,
}

/// iOS application state
#[derive(Debug, Clone, PartialEq)]
pub struct IosAppState {
    /// Current application lifecycle state
    pub lifecycle_state: AppLifecycleState,
    /// Bluetooth authorization status
    pub bluetooth_authorization: BluetoothAuthStatus,
    /// Whether background app refresh is enabled
    pub background_app_refresh_enabled: bool,
    /// Whether the device is in low power mode
    pub low_power_mode_enabled: bool,
    /// iOS version information
    pub ios_version: IosVersion,
    /// Last state change timestamp
    pub last_updated: u64,
}

/// iOS application lifecycle states
#[derive(Debug, Clone, PartialEq)]
pub enum AppLifecycleState {
    /// App is active and in foreground
    Active,
    /// App is inactive (transitioning or interrupted)
    Inactive,
    /// App is in background
    Background,
    /// App is suspended
    Suspended,
    /// App state is unknown
    Unknown,
}

/// Bluetooth authorization status on iOS
#[derive(Debug, Clone, PartialEq)]
pub enum BluetoothAuthStatus {
    /// Authorization not determined
    NotDetermined,
    /// Authorization denied by user
    Denied,
    /// Authorization granted
    Authorized,
    /// Bluetooth not available
    Unavailable,
    /// Status unknown
    Unknown,
}

/// iOS version information
#[derive(Debug, Clone, PartialEq)]
pub struct IosVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// BLE permission state
#[derive(Debug, Clone)]
pub struct PermissionState {
    /// Bluetooth peripheral permission
    pub peripheral_permission: BluetoothAuthStatus,
    /// Background modes permissions
    pub background_modes: Vec<BackgroundMode>,
    /// Last permission check timestamp
    pub last_checked: u64,
}

/// Background mode types
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundMode {
    BluetoothCentral,
    BluetoothPeripheral,
    BackgroundProcessing,
    BackgroundAppRefresh,
}

/// Background operation constraints
#[derive(Debug, Clone)]
pub struct BackgroundConstraints {
    /// Whether service UUID filtering is required
    pub service_uuid_filtering_required: bool,
    /// Whether local name is available in background
    pub local_name_available: bool,
    /// Maximum background scan duration (seconds)
    pub max_background_scan_duration: u64,
    /// Background scan interval restrictions (seconds)
    pub background_scan_interval: u64,
    /// Maximum number of background connections
    pub max_background_connections: u32,
    /// Whether background advertising is severely limited
    pub advertising_limited: bool,
    /// iOS version-specific constraints
    pub version_constraints: Vec<String>,
}

impl Default for IosVersion {
    fn default() -> Self {
        Self { major: 13, minor: 0, patch: 0 }
    }
}

impl Default for BackgroundConstraints {
    fn default() -> Self {
        Self {
            service_uuid_filtering_required: true,
            local_name_available: false,
            max_background_scan_duration: 10, // iOS typically allows 10 seconds
            background_scan_interval: 30,     // 30 seconds between scans
            max_background_connections: 1,    // Very limited in background
            advertising_limited: true,
            version_constraints: vec![
                "Service UUIDs move to overflow area in background".to_string(),
                "Local name not advertised in background".to_string(),
                "Scan result coalescing reduces discovery frequency".to_string(),
            ],
        }
    }
}

impl IosStateManager {
    /// Create a new iOS state manager
    pub fn new() -> Result<Self, BitCrapsError> {
        let app_state = IosAppState {
            lifecycle_state: AppLifecycleState::Unknown,
            bluetooth_authorization: BluetoothAuthStatus::NotDetermined,
            background_app_refresh_enabled: true,
            low_power_mode_enabled: false,
            ios_version: IosVersion::default(),
            last_updated: current_timestamp(),
        };
        
        let permissions = PermissionState {
            peripheral_permission: BluetoothAuthStatus::NotDetermined,
            background_modes: vec![],
            last_checked: current_timestamp(),
        };
        
        let background_constraints = BackgroundConstraints::default();
        
        Ok(Self {
            app_state: Arc::new(RwLock::new(app_state)),
            permissions: Arc::new(Mutex::new(permissions)),
            background_constraints: Arc::new(Mutex::new(background_constraints)),
            callbacks: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// Update the application lifecycle state
    pub fn update_app_state(&self, new_state: AppLifecycleState) -> Result<(), BitCrapsError> {
        info!("iOS app state changing to: {:?}", new_state);
        
        let mut state = self.app_state.write().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire app state write lock".to_string()
        })?;
        
        let old_state = state.lifecycle_state.clone();
        state.lifecycle_state = new_state.clone();
        state.last_updated = current_timestamp();
        
        // Update background constraints based on new state
        if new_state == AppLifecycleState::Background || new_state == AppLifecycleState::Suspended {
            self.update_background_constraints(&state)?;
        }
        
        // Trigger callbacks
        let state_clone = state.clone();
        drop(state); // Release lock before calling callbacks
        
        self.notify_state_change(&state_clone)?;
        
        // Log important state transitions
        match (old_state, new_state) {
            (AppLifecycleState::Active, AppLifecycleState::Background) => {
                warn!("App entered background - BLE operations will be severely limited");
            }
            (AppLifecycleState::Background, AppLifecycleState::Active) => {
                info!("App returned to foreground - full BLE capabilities restored");
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Update Bluetooth authorization status
    pub fn update_bluetooth_authorization(&self, status: BluetoothAuthStatus) -> Result<(), BitCrapsError> {
        info!("Bluetooth authorization status changed: {:?}", status);
        
        // Update app state
        let mut state = self.app_state.write().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire app state write lock".to_string()
        })?;
        
        state.bluetooth_authorization = status.clone();
        state.last_updated = current_timestamp();
        
        // Update permissions
        let mut permissions = self.permissions.lock().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire permissions lock".to_string()
        })?;
        
        permissions.peripheral_permission = status.clone();
        permissions.last_checked = current_timestamp();
        
        // Trigger callbacks
        let state_clone = state.clone();
        drop(state);
        drop(permissions);
        
        self.notify_state_change(&state_clone)?;
        
        // Log authorization status
        match status {
            BluetoothAuthStatus::Authorized => {
                info!("Bluetooth authorization granted - BLE operations enabled");
            }
            BluetoothAuthStatus::Denied => {
                error!("Bluetooth authorization denied - BLE operations disabled");
            }
            BluetoothAuthStatus::NotDetermined => {
                warn!("Bluetooth authorization not determined - need to request permission");
            }
            BluetoothAuthStatus::Unavailable => {
                error!("Bluetooth unavailable on this device");
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Update iOS system settings
    pub fn update_system_settings(&self, 
                                  background_app_refresh: bool, 
                                  low_power_mode: bool) -> Result<(), BitCrapsError> {
        debug!("Updating iOS system settings: background_refresh={}, low_power={}", 
               background_app_refresh, low_power_mode);
        
        let mut state = self.app_state.write().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire app state write lock".to_string()
        })?;
        
        state.background_app_refresh_enabled = background_app_refresh;
        state.low_power_mode_enabled = low_power_mode;
        state.last_updated = current_timestamp();
        
        // Update background constraints if necessary
        if state.lifecycle_state == AppLifecycleState::Background {
            self.update_background_constraints(&state)?;
        }
        
        let state_clone = state.clone();
        drop(state);
        
        self.notify_state_change(&state_clone)?;
        
        // Log important settings changes
        if !background_app_refresh {
            warn!("Background App Refresh disabled - background BLE operations may be limited");
        }
        
        if low_power_mode {
            warn!("Low Power Mode enabled - BLE performance may be reduced");
        }
        
        Ok(())
    }
    
    /// Set iOS version information
    pub fn set_ios_version(&self, major: u32, minor: u32, patch: u32) -> Result<(), BitCrapsError> {
        info!("iOS version detected: {}.{}.{}", major, minor, patch);
        
        let mut state = self.app_state.write().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire app state write lock".to_string()
        })?;
        
        state.ios_version = IosVersion { major, minor, patch };
        state.last_updated = current_timestamp();
        
        // Update background constraints based on iOS version
        self.update_background_constraints(&state)?;
        
        let state_clone = state.clone();
        drop(state);
        
        self.notify_state_change(&state_clone)?;
        
        Ok(())
    }
    
    /// Check if BLE operations are available
    pub fn is_ble_available(&self) -> bool {
        if let Ok(state) = self.app_state.read() {
            match state.bluetooth_authorization {
                BluetoothAuthStatus::Authorized => true,
                _ => false,
            }
        } else {
            false
        }
    }
    
    /// Check if background BLE operations are viable
    pub fn is_background_ble_viable(&self) -> bool {
        if let Ok(state) = self.app_state.read() {
            // Must have Bluetooth authorization
            if state.bluetooth_authorization != BluetoothAuthStatus::Authorized {
                return false;
            }
            
            // Must have background app refresh enabled
            if !state.background_app_refresh_enabled {
                return false;
            }
            
            // Low power mode significantly limits background operations
            if state.low_power_mode_enabled {
                return false;
            }
            
            // iOS 13+ has better background BLE support
            if state.ios_version.major >= 13 {
                return true;
            }
            
            false
        } else {
            false
        }
    }
    
    /// Get current background constraints
    pub fn get_background_constraints(&self) -> Result<BackgroundConstraints, BitCrapsError> {
        self.background_constraints.lock()
            .map(|constraints| constraints.clone())
            .map_err(|_| BitCrapsError::InitializationError {
                reason: "Failed to acquire background constraints lock".to_string()
            })
    }
    
    /// Add a state change callback
    pub fn add_state_callback<F>(&self, callback: F) -> Result<(), BitCrapsError>
    where
        F: Fn(&IosAppState) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.lock().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire callbacks lock".to_string()
        })?;
        
        callbacks.push(Box::new(callback));
        
        debug!("State callback added (total: {})", callbacks.len());
        Ok(())
    }
    
    /// Get current application state
    pub fn get_app_state(&self) -> Result<IosAppState, BitCrapsError> {
        self.app_state.read()
            .map(|state| state.clone())
            .map_err(|_| BitCrapsError::InitializationError {
                reason: "Failed to acquire app state read lock".to_string()
            })
    }
    
    /// Request Bluetooth permissions (to be called from iOS side)
    pub fn request_bluetooth_permissions(&self) -> Result<(), BitCrapsError> {
        info!("Requesting Bluetooth permissions from iOS");
        
        // This would trigger the iOS permission request dialog
        // Implementation would call into iOS-specific code
        
        Ok(())
    }
    
    /// Update background constraints based on current state and iOS version
    fn update_background_constraints(&self, state: &IosAppState) -> Result<(), BitCrapsError> {
        let mut constraints = self.background_constraints.lock().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire background constraints lock".to_string()
        })?;
        
        // Update constraints based on iOS version
        match state.ios_version.major {
            13 | 14 | 15 | 16 | 17 => {
                // iOS 13+ has service UUID filtering requirement in background
                constraints.service_uuid_filtering_required = true;
                constraints.local_name_available = false;
                constraints.max_background_scan_duration = 10;
                constraints.background_scan_interval = 30;
                constraints.max_background_connections = 1;
                constraints.advertising_limited = true;
            }
            major if major < 13 => {
                // Older iOS versions have even more severe limitations
                constraints.service_uuid_filtering_required = true;
                constraints.local_name_available = false;
                constraints.max_background_scan_duration = 5;
                constraints.background_scan_interval = 60;
                constraints.max_background_connections = 1;
                constraints.advertising_limited = true;
            }
            _ => {
                // Future iOS versions - assume similar constraints
                constraints.service_uuid_filtering_required = true;
                constraints.local_name_available = false;
                constraints.max_background_scan_duration = 10;
                constraints.background_scan_interval = 30;
                constraints.max_background_connections = 1;
                constraints.advertising_limited = true;
            }
        }
        
        // Adjust constraints based on current state
        if state.low_power_mode_enabled {
            constraints.max_background_scan_duration = constraints.max_background_scan_duration.min(5);
            constraints.background_scan_interval = constraints.background_scan_interval.max(60);
            constraints.max_background_connections = 0; // No connections in low power mode
        }
        
        if !state.background_app_refresh_enabled {
            constraints.max_background_scan_duration = 0;
            constraints.max_background_connections = 0;
        }
        
        debug!("Background constraints updated: {:?}", constraints);
        Ok(())
    }
    
    /// Notify all registered callbacks of state change
    fn notify_state_change(&self, state: &IosAppState) -> Result<(), BitCrapsError> {
        let callbacks = self.callbacks.lock().map_err(|_| BitCrapsError::InitializationError {
            reason: "Failed to acquire callbacks lock".to_string()
        })?;
        
        for callback in callbacks.iter() {
            callback(state);
        }
        
        debug!("State change notifications sent to {} callbacks", callbacks.len());
        Ok(())
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
    fn test_state_manager_creation() {
        let manager = IosStateManager::new().unwrap();
        let state = manager.get_app_state().unwrap();
        
        assert_eq!(state.lifecycle_state, AppLifecycleState::Unknown);
        assert_eq!(state.bluetooth_authorization, BluetoothAuthStatus::NotDetermined);
    }
    
    #[test]
    fn test_app_state_updates() {
        let manager = IosStateManager::new().unwrap();
        
        // Test state transition
        manager.update_app_state(AppLifecycleState::Active).unwrap();
        let state = manager.get_app_state().unwrap();
        assert_eq!(state.lifecycle_state, AppLifecycleState::Active);
        
        // Test background transition
        manager.update_app_state(AppLifecycleState::Background).unwrap();
        let state = manager.get_app_state().unwrap();
        assert_eq!(state.lifecycle_state, AppLifecycleState::Background);
    }
    
    #[test]
    fn test_bluetooth_authorization() {
        let manager = IosStateManager::new().unwrap();
        
        // Initially not available
        assert!(!manager.is_ble_available());
        
        // Grant authorization
        manager.update_bluetooth_authorization(BluetoothAuthStatus::Authorized).unwrap();
        assert!(manager.is_ble_available());
        
        // Deny authorization
        manager.update_bluetooth_authorization(BluetoothAuthStatus::Denied).unwrap();
        assert!(!manager.is_ble_available());
    }
    
    #[test]
    fn test_background_ble_viability() {
        let manager = IosStateManager::new().unwrap();
        
        // Initially not viable (no authorization)
        assert!(!manager.is_background_ble_viable());
        
        // Grant authorization
        manager.update_bluetooth_authorization(BluetoothAuthStatus::Authorized).unwrap();
        manager.set_ios_version(15, 0, 0).unwrap(); // iOS 15
        
        // Should be viable with authorization and modern iOS
        assert!(manager.is_background_ble_viable());
        
        // Enable low power mode
        manager.update_system_settings(true, true).unwrap();
        
        // Should not be viable in low power mode
        assert!(!manager.is_background_ble_viable());
    }
    
    #[test]
    fn test_state_callbacks() {
        let manager = IosStateManager::new().unwrap();
        let callback_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        
        let callback_called_clone = callback_called.clone();
        manager.add_state_callback(move |_| {
            callback_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        }).unwrap();
        
        // Trigger state change
        manager.update_app_state(AppLifecycleState::Active).unwrap();
        
        // Callback should have been called
        assert!(callback_called.load(std::sync::atomic::Ordering::SeqCst));
    }
}