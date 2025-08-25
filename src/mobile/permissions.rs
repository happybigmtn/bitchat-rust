//! Cross-platform permission handling and validation
//!
//! This module provides comprehensive permission management for:
//! - Android: Runtime permissions (API 23+) and special permissions
//! - iOS: Info.plist permissions and usage descriptions
//! - Permission request flows and user education
//! - Permission state monitoring and graceful degradation

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use crate::error::{Result, Error};

/// Cross-platform permission manager
pub struct PermissionManager {
    required_permissions: HashSet<Permission>,
    optional_permissions: HashSet<Permission>,
    permission_states: Arc<Mutex<HashMap<Permission, PermissionState>>>,
    callbacks: Arc<Mutex<Vec<Box<dyn Fn(Permission, PermissionState) + Send + Sync>>>>,
}

impl PermissionManager {
    /// Create new permission manager with required permissions
    pub fn new(required: Vec<Permission>, optional: Vec<Permission>) -> Self {
        let manager = Self {
            required_permissions: required.into_iter().collect(),
            optional_permissions: optional.into_iter().collect(),
            permission_states: Arc::new(Mutex::new(HashMap::new())),
            callbacks: Arc::new(Mutex::new(Vec::new())),
        };
        
        // Initialize permission states
        manager.initialize_permission_states().unwrap_or_else(|e| {
            log::error!("Failed to initialize permission states: {}", e);
        });
        
        manager
    }
    
    /// Check current state of all permissions
    pub fn check_all_permissions(&self) -> Result<PermissionSummary> {
        let mut permission_states = self.permission_states.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire permission states lock".to_string())
        })?;
        
        let mut granted_required = Vec::new();
        let mut denied_required = Vec::new();
        let mut granted_optional = Vec::new();
        let mut denied_optional = Vec::new();
        
        // Check required permissions
        for permission in &self.required_permissions {
            let state = self.check_permission_state(*permission)?;
            permission_states.insert(*permission, state);
            
            match state {
                PermissionState::Granted => granted_required.push(*permission),
                PermissionState::Denied | PermissionState::PermanentlyDenied => {
                    denied_required.push(*permission);
                },
                _ => {},
            }
        }
        
        // Check optional permissions
        for permission in &self.optional_permissions {
            let state = self.check_permission_state(*permission)?;
            permission_states.insert(*permission, state);
            
            match state {
                PermissionState::Granted => granted_optional.push(*permission),
                PermissionState::Denied | PermissionState::PermanentlyDenied => {
                    denied_optional.push(*permission);
                },
                _ => {},
            }
        }
        
        let all_required_granted = denied_required.is_empty();
        
        Ok(PermissionSummary {
            all_required_granted,
            granted_required,
            denied_required,
            granted_optional,
            denied_optional,
            can_continue: all_required_granted,
        })
    }
    
    /// Request specific permission with rationale
    pub async fn request_permission(
        &self,
        permission: Permission,
        rationale: &str,
    ) -> Result<PermissionState> {
        log::info!("Requesting permission: {:?}", permission);
        
        // Check current state first
        let current_state = self.check_permission_state(permission)?;
        
        match current_state {
            PermissionState::Granted => return Ok(current_state),
            PermissionState::PermanentlyDenied => {
                // Direct user to settings
                self.show_settings_redirect(permission, rationale).await?;
                return Ok(current_state);
            },
            _ => {},
        }
        
        // Show rationale if needed
        if self.should_show_rationale(permission)? {
            self.show_permission_rationale(permission, rationale).await?;
        }
        
        // Request permission
        let new_state = self.platform_request_permission(permission).await?;
        
        // Update state and notify callbacks
        self.update_permission_state(permission, new_state)?;
        
        Ok(new_state)
    }
    
    /// Request multiple permissions with batch rationale
    pub async fn request_permissions(
        &self,
        permissions: Vec<Permission>,
        rationale: &str,
    ) -> Result<HashMap<Permission, PermissionState>> {
        log::info!("Requesting {} permissions", permissions.len());
        
        // Filter permissions that need to be requested
        let mut to_request = Vec::new();
        let mut results = HashMap::new();
        
        for permission in permissions {
            let state = self.check_permission_state(permission)?;
            match state {
                PermissionState::Granted => {
                    results.insert(permission, state);
                },
                PermissionState::PermanentlyDenied => {
                    results.insert(permission, state);
                },
                _ => {
                    to_request.push(permission);
                },
            }
        }
        
        if !to_request.is_empty() {
            // Show batch rationale if needed
            let needs_rationale = to_request.iter()
                .any(|p| self.should_show_rationale(*p).unwrap_or(false));
            
            if needs_rationale {
                self.show_batch_permission_rationale(&to_request, rationale).await?;
            }
            
            // Request permissions (platform-specific batch or sequential)
            let batch_results = self.platform_request_permissions(to_request).await?;
            
            for (permission, state) in batch_results {
                self.update_permission_state(permission, state)?;
                results.insert(permission, state);
            }
        }
        
        Ok(results)
    }
    
    /// Get BitCraps-specific required permissions
    pub fn get_bitcraps_required_permissions() -> Vec<Permission> {
        vec![
            Permission::Bluetooth,
            Permission::BluetoothAdmin,
            Permission::AccessCoarseLocation, // Required for BLE scanning
            Permission::ForegroundService,
        ]
    }
    
    /// Get BitCraps-specific optional permissions  
    pub fn get_bitcraps_optional_permissions() -> Vec<Permission> {
        vec![
            Permission::AccessFineLocation,   // Better location for BLE
            Permission::Camera,               // QR code scanning
            Permission::Vibrate,              // Haptic feedback
            Permission::WakeLock,             // Keep screen on during games
            Permission::SystemAlertWindow,    // Overlay notifications
        ]
    }
    
    /// Register callback for permission state changes
    pub fn add_permission_callback<F>(&self, callback: F) -> Result<()>
    where
        F: Fn(Permission, PermissionState) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire callbacks lock".to_string())
        })?;
        
        callbacks.push(Box::new(callback));
        Ok(())
    }
    
    /// Check if app can function without specific permission
    pub fn can_function_without(&self, permission: Permission) -> bool {
        !self.required_permissions.contains(&permission)
    }
    
    /// Get user-friendly permission explanation
    pub fn get_permission_explanation(&self, permission: Permission) -> &'static str {
        match permission {
            Permission::Bluetooth => {
                "BitCraps uses Bluetooth to connect with other players for peer-to-peer gaming. \
                This enables secure, private games without internet connection."
            },
            Permission::BluetoothAdmin => {
                "BitCraps needs to manage Bluetooth connections to host and join games. \
                This allows you to become discoverable to other players."
            },
            Permission::AccessCoarseLocation => {
                "Location permission is required by Android for Bluetooth Low Energy scanning. \
                BitCraps does not access your actual location - this is just needed for BLE discovery."
            },
            Permission::AccessFineLocation => {
                "Precise location can improve Bluetooth connection reliability and enable \
                location-based game features in the future."
            },
            Permission::Camera => {
                "Camera access enables QR code scanning for quick game joining and \
                easy wallet address sharing."
            },
            Permission::ForegroundService => {
                "BitCraps runs a background service to maintain game connections even \
                when the app is minimized. This ensures uninterrupted gameplay."
            },
            Permission::Vibrate => {
                "Haptic feedback enhances the gaming experience with tactile responses \
                to dice rolls, wins, and game events."
            },
            Permission::WakeLock => {
                "Prevents the screen from turning off during active games, \
                ensuring you don't miss important game events."
            },
            Permission::SystemAlertWindow => {
                "Allows important game notifications to appear over other apps, \
                so you never miss your turn."
            },
            _ => "This permission helps BitCraps provide the best gaming experience.",
        }
    }
    
    /// Generate permission request flow for onboarding
    pub fn create_onboarding_flow(&self) -> PermissionFlow {
        let mut steps = Vec::new();
        
        // Step 1: Essential Bluetooth permissions
        steps.push(PermissionStep {
            permissions: vec![Permission::Bluetooth, Permission::BluetoothAdmin],
            title: "Enable Bluetooth Gaming".to_string(),
            description: "BitCraps connects players directly through Bluetooth for secure, private games without internet.".to_string(),
            required: true,
        });
        
        // Step 2: Location for BLE (required but needs explanation)
        steps.push(PermissionStep {
            permissions: vec![Permission::AccessCoarseLocation],
            title: "Enable Player Discovery".to_string(),
            description: "Android requires location permission for Bluetooth Low Energy discovery. BitCraps doesn't access your location.".to_string(),
            required: true,
        });
        
        // Step 3: Foreground service
        steps.push(PermissionStep {
            permissions: vec![Permission::ForegroundService],
            title: "Background Gaming".to_string(),
            description: "Keep games running even when switching apps, ensuring uninterrupted gameplay.".to_string(),
            required: true,
        });
        
        // Step 4: Optional enhancements
        steps.push(PermissionStep {
            permissions: vec![
                Permission::Camera,
                Permission::Vibrate,
                Permission::WakeLock,
            ],
            title: "Enhanced Experience".to_string(),
            description: "Optional permissions for QR scanning, haptic feedback, and screen management.".to_string(),
            required: false,
        });
        
        PermissionFlow {
            steps,
            current_step: 0,
            completed: false,
        }
    }
    
    /// Check permission state for specific permission
    fn check_permission_state(&self, permission: Permission) -> Result<PermissionState> {
        #[cfg(target_os = "android")]
        {
            self.android_check_permission(permission)
        }
        
        #[cfg(target_os = "ios")]
        {
            self.ios_check_permission(permission)
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Simulation mode - assume all permissions granted
            Ok(PermissionState::Granted)
        }
    }
    
    /// Request permission on platform
    async fn platform_request_permission(&self, permission: Permission) -> Result<PermissionState> {
        #[cfg(target_os = "android")]
        {
            self.android_request_permission(permission).await
        }
        
        #[cfg(target_os = "ios")]
        {
            self.ios_request_permission(permission).await
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Simulation mode
            Ok(PermissionState::Granted)
        }
    }
    
    /// Request multiple permissions on platform
    async fn platform_request_permissions(
        &self,
        permissions: Vec<Permission>,
    ) -> Result<HashMap<Permission, PermissionState>> {
        #[cfg(target_os = "android")]
        {
            self.android_request_permissions(permissions).await
        }
        
        #[cfg(target_os = "ios")]
        {
            self.ios_request_permissions(permissions).await
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            // Simulation mode
            let mut results = HashMap::new();
            for permission in permissions {
                results.insert(permission, PermissionState::Granted);
            }
            Ok(results)
        }
    }
    
    /// Initialize permission states
    fn initialize_permission_states(&self) -> Result<()> {
        let mut states = self.permission_states.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire permission states lock".to_string())
        })?;
        
        // Initialize all known permissions
        let all_permissions = self.required_permissions.iter()
            .chain(self.optional_permissions.iter());
        
        for &permission in all_permissions {
            let state = self.check_permission_state(permission)?;
            states.insert(permission, state);
        }
        
        Ok(())
    }
    
    /// Update permission state and notify callbacks
    fn update_permission_state(&self, permission: Permission, new_state: PermissionState) -> Result<()> {
        // Update state
        {
            let mut states = self.permission_states.lock().map_err(|_| {
                Error::InvalidState("Failed to acquire permission states lock".to_string())
            })?;
            states.insert(permission, new_state);
        }
        
        // Notify callbacks
        let callbacks = self.callbacks.lock().map_err(|_| {
            Error::InvalidState("Failed to acquire callbacks lock".to_string())
        })?;
        
        for callback in callbacks.iter() {
            callback(permission, new_state);
        }
        
        Ok(())
    }
    
    /// Check if we should show rationale for permission
    fn should_show_rationale(&self, permission: Permission) -> Result<bool> {
        #[cfg(target_os = "android")]
        {
            self.android_should_show_rationale(permission)
        }
        
        #[cfg(target_os = "ios")]
        {
            // iOS doesn't have rationale concept
            Ok(false)
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            Ok(false)
        }
    }
    
    /// Show permission rationale UI
    async fn show_permission_rationale(&self, permission: Permission, rationale: &str) -> Result<()> {
        log::info!("Showing rationale for {:?}: {}", permission, rationale);
        // In real implementation, would show native UI dialog
        Ok(())
    }
    
    /// Show batch permission rationale
    async fn show_batch_permission_rationale(&self, permissions: &[Permission], rationale: &str) -> Result<()> {
        log::info!("Showing batch rationale for {} permissions: {}", permissions.len(), rationale);
        // In real implementation, would show native UI dialog
        Ok(())
    }
    
    /// Show settings redirect dialog
    async fn show_settings_redirect(&self, permission: Permission, reason: &str) -> Result<()> {
        log::info!("Showing settings redirect for {:?}: {}", permission, reason);
        // In real implementation, would show dialog with "Open Settings" button
        Ok(())
    }
}

// ============= Android Permission Implementation =============

#[cfg(target_os = "android")]
impl PermissionManager {
    fn android_check_permission(&self, permission: Permission) -> Result<PermissionState> {
        let permission_str = permission.to_android_string();
        let permission_cstr = CString::new(permission_str)?;
        
        let result = unsafe { android_check_permission(permission_cstr.as_ptr()) };
        
        match result {
            0 => Ok(PermissionState::Granted),
            1 => Ok(PermissionState::Denied),
            2 => Ok(PermissionState::PermanentlyDenied),
            _ => Ok(PermissionState::NotDetermined),
        }
    }
    
    async fn android_request_permission(&self, permission: Permission) -> Result<PermissionState> {
        let permission_str = permission.to_android_string();
        let permission_cstr = CString::new(permission_str)?;
        
        let result = unsafe { android_request_permission(permission_cstr.as_ptr()) };
        
        match result {
            0 => Ok(PermissionState::Granted),
            1 => Ok(PermissionState::Denied),
            2 => Ok(PermissionState::PermanentlyDenied),
            _ => Err(Error::Platform(format!("Android permission request failed: {}", result))),
        }
    }
    
    async fn android_request_permissions(&self, permissions: Vec<Permission>) -> Result<HashMap<Permission, PermissionState>> {
        let mut permission_strings = Vec::new();
        let mut permission_cstrs = Vec::new();
        
        for permission in &permissions {
            let perm_str = permission.to_android_string();
            let cstr = CString::new(perm_str)?;
            permission_strings.push(perm_str);
            permission_cstrs.push(cstr);
        }
        
        // Convert to C array
        let c_permissions: Vec<*const c_char> = permission_cstrs.iter()
            .map(|cstr| cstr.as_ptr())
            .collect();
        
        let mut results = vec![0i32; permissions.len()];
        
        let result = unsafe {
            android_request_permissions(
                c_permissions.as_ptr(),
                c_permissions.len() as c_int,
                results.as_mut_ptr()
            )
        };
        
        if result != 0 {
            return Err(Error::Platform(format!("Android batch permission request failed: {}", result)));
        }
        
        let mut permission_results = HashMap::new();
        for (i, permission) in permissions.iter().enumerate() {
            let state = match results[i] {
                0 => PermissionState::Granted,
                1 => PermissionState::Denied,
                2 => PermissionState::PermanentlyDenied,
                _ => PermissionState::NotDetermined,
            };
            permission_results.insert(*permission, state);
        }
        
        Ok(permission_results)
    }
    
    fn android_should_show_rationale(&self, permission: Permission) -> Result<bool> {
        let permission_str = permission.to_android_string();
        let permission_cstr = CString::new(permission_str)?;
        
        let result = unsafe { android_should_show_rationale(permission_cstr.as_ptr()) };
        Ok(result == 1)
    }
}

// ============= iOS Permission Implementation =============

#[cfg(target_os = "ios")]
impl PermissionManager {
    fn ios_check_permission(&self, permission: Permission) -> Result<PermissionState> {
        let permission_str = permission.to_ios_string();
        let permission_cstr = CString::new(permission_str)?;
        
        let result = unsafe { ios_check_permission(permission_cstr.as_ptr()) };
        
        match result {
            0 => Ok(PermissionState::NotDetermined),
            1 => Ok(PermissionState::Denied),
            2 => Ok(PermissionState::Granted),
            3 => Ok(PermissionState::Restricted),
            _ => Ok(PermissionState::NotDetermined),
        }
    }
    
    async fn ios_request_permission(&self, permission: Permission) -> Result<PermissionState> {
        let permission_str = permission.to_ios_string();
        let permission_cstr = CString::new(permission_str)?;
        
        let result = unsafe { ios_request_permission(permission_cstr.as_ptr()) };
        
        match result {
            0 => Ok(PermissionState::Granted),
            1 => Ok(PermissionState::Denied),
            _ => Err(Error::Platform(format!("iOS permission request failed: {}", result))),
        }
    }
    
    async fn ios_request_permissions(&self, permissions: Vec<Permission>) -> Result<HashMap<Permission, PermissionState>> {
        // iOS typically handles permissions individually
        let mut results = HashMap::new();
        
        for permission in permissions {
            let state = self.ios_request_permission(permission).await?;
            results.insert(permission, state);
        }
        
        Ok(results)
    }
}

// External JNI/FFI functions
extern "C" {
    // Android permission functions
    #[cfg(target_os = "android")]
    fn android_check_permission(permission: *const c_char) -> c_int;
    #[cfg(target_os = "android")]
    fn android_request_permission(permission: *const c_char) -> c_int;
    #[cfg(target_os = "android")]
    fn android_request_permissions(
        permissions: *const *const c_char,
        count: c_int,
        results: *mut c_int
    ) -> c_int;
    #[cfg(target_os = "android")]
    fn android_should_show_rationale(permission: *const c_char) -> c_int;
    
    // iOS permission functions
    #[cfg(target_os = "ios")]
    fn ios_check_permission(permission: *const c_char) -> c_int;
    #[cfg(target_os = "ios")]
    fn ios_request_permission(permission: *const c_char) -> c_int;
}

/// Cross-platform permissions enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Bluetooth permissions
    Bluetooth,
    BluetoothAdmin,
    BluetoothConnect,
    BluetoothScan,
    BluetoothAdvertise,
    
    // Location permissions  
    AccessCoarseLocation,
    AccessFineLocation,
    AccessBackgroundLocation,
    
    // Camera and media
    Camera,
    RecordAudio,
    ReadExternalStorage,
    WriteExternalStorage,
    
    // System permissions
    ForegroundService,
    WakeLock,
    Vibrate,
    Internet,
    AccessNetworkState,
    AccessWifiState,
    ChangeWifiState,
    SystemAlertWindow,
    
    // iOS specific
    LocationWhenInUse,
    LocationAlways,
    Microphone,
    PhotoLibrary,
    Contacts,
    
    // Notification permissions
    PostNotifications,
}

impl Permission {
    /// Convert to Android permission string
    pub fn to_android_string(&self) -> &'static str {
        match self {
            Permission::Bluetooth => "android.permission.BLUETOOTH",
            Permission::BluetoothAdmin => "android.permission.BLUETOOTH_ADMIN",
            Permission::BluetoothConnect => "android.permission.BLUETOOTH_CONNECT",
            Permission::BluetoothScan => "android.permission.BLUETOOTH_SCAN",
            Permission::BluetoothAdvertise => "android.permission.BLUETOOTH_ADVERTISE",
            Permission::AccessCoarseLocation => "android.permission.ACCESS_COARSE_LOCATION",
            Permission::AccessFineLocation => "android.permission.ACCESS_FINE_LOCATION",
            Permission::AccessBackgroundLocation => "android.permission.ACCESS_BACKGROUND_LOCATION",
            Permission::Camera => "android.permission.CAMERA",
            Permission::RecordAudio => "android.permission.RECORD_AUDIO",
            Permission::ReadExternalStorage => "android.permission.READ_EXTERNAL_STORAGE",
            Permission::WriteExternalStorage => "android.permission.WRITE_EXTERNAL_STORAGE",
            Permission::ForegroundService => "android.permission.FOREGROUND_SERVICE",
            Permission::WakeLock => "android.permission.WAKE_LOCK",
            Permission::Vibrate => "android.permission.VIBRATE",
            Permission::Internet => "android.permission.INTERNET",
            Permission::AccessNetworkState => "android.permission.ACCESS_NETWORK_STATE",
            Permission::AccessWifiState => "android.permission.ACCESS_WIFI_STATE",
            Permission::ChangeWifiState => "android.permission.CHANGE_WIFI_STATE",
            Permission::SystemAlertWindow => "android.permission.SYSTEM_ALERT_WINDOW",
            Permission::PostNotifications => "android.permission.POST_NOTIFICATIONS",
            _ => "android.permission.UNKNOWN",
        }
    }
    
    /// Convert to iOS permission string
    pub fn to_ios_string(&self) -> &'static str {
        match self {
            Permission::Bluetooth => "NSBluetoothPeripheralUsageDescription",
            Permission::LocationWhenInUse => "NSLocationWhenInUseUsageDescription",
            Permission::LocationAlways => "NSLocationAlwaysAndWhenInUseUsageDescription",
            Permission::Camera => "NSCameraUsageDescription",
            Permission::Microphone => "NSMicrophoneUsageDescription",
            Permission::PhotoLibrary => "NSPhotoLibraryUsageDescription",
            Permission::Contacts => "NSContactsUsageDescription",
            _ => "NSUnknownUsageDescription",
        }
    }
    
    /// Get permission category
    pub fn category(&self) -> PermissionCategory {
        match self {
            Permission::Bluetooth | Permission::BluetoothAdmin | Permission::BluetoothConnect
            | Permission::BluetoothScan | Permission::BluetoothAdvertise => PermissionCategory::Bluetooth,
            
            Permission::AccessCoarseLocation | Permission::AccessFineLocation 
            | Permission::AccessBackgroundLocation | Permission::LocationWhenInUse 
            | Permission::LocationAlways => PermissionCategory::Location,
            
            Permission::Camera | Permission::RecordAudio | Permission::ReadExternalStorage
            | Permission::WriteExternalStorage | Permission::PhotoLibrary => PermissionCategory::Media,
            
            Permission::ForegroundService | Permission::WakeLock | Permission::Vibrate
            | Permission::SystemAlertWindow => PermissionCategory::System,
            
            Permission::Internet | Permission::AccessNetworkState | Permission::AccessWifiState
            | Permission::ChangeWifiState => PermissionCategory::Network,
            
            Permission::PostNotifications => PermissionCategory::Notifications,
            
            Permission::Microphone | Permission::Contacts => PermissionCategory::Privacy,
        }
    }
}

/// Permission states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionState {
    NotDetermined,
    Granted,
    Denied,
    PermanentlyDenied,
    Restricted, // iOS only
}

/// Permission categories for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionCategory {
    Bluetooth,
    Location,
    Media,
    System,
    Network,
    Notifications,
    Privacy,
}

/// Permission check summary
#[derive(Debug)]
pub struct PermissionSummary {
    pub all_required_granted: bool,
    pub granted_required: Vec<Permission>,
    pub denied_required: Vec<Permission>,
    pub granted_optional: Vec<Permission>,
    pub denied_optional: Vec<Permission>,
    pub can_continue: bool,
}

/// Permission request flow for onboarding
#[derive(Debug)]
pub struct PermissionFlow {
    pub steps: Vec<PermissionStep>,
    pub current_step: usize,
    pub completed: bool,
}

/// Individual permission step in flow
#[derive(Debug)]
pub struct PermissionStep {
    pub permissions: Vec<Permission>,
    pub title: String,
    pub description: String,
    pub required: bool,
}

impl PermissionFlow {
    /// Move to next step in flow
    pub fn next_step(&mut self) -> Option<&PermissionStep> {
        if self.current_step < self.steps.len() - 1 {
            self.current_step += 1;
            Some(&self.steps[self.current_step])
        } else {
            self.completed = true;
            None
        }
    }
    
    /// Get current step
    pub fn current(&self) -> Option<&PermissionStep> {
        if self.current_step < self.steps.len() {
            Some(&self.steps[self.current_step])
        } else {
            None
        }
    }
    
    /// Check if flow is complete
    pub fn is_complete(&self) -> bool {
        self.completed || self.current_step >= self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_permission_manager_creation() {
        let required = PermissionManager::get_bitcraps_required_permissions();
        let optional = PermissionManager::get_bitcraps_optional_permissions();
        
        let manager = PermissionManager::new(required, optional);
        assert!(!manager.required_permissions.is_empty());
        assert!(!manager.optional_permissions.is_empty());
    }
    
    #[tokio::test]
    async fn test_permission_summary() {
        let required = vec![Permission::Bluetooth, Permission::BluetoothAdmin];
        let optional = vec![Permission::Camera, Permission::Vibrate];
        
        let manager = PermissionManager::new(required, optional);
        let summary = manager.check_all_permissions().unwrap();
        
        // In simulation mode, all permissions should be granted
        assert!(summary.all_required_granted);
        assert!(summary.can_continue);
    }
    
    #[tokio::test]
    async fn test_single_permission_request() {
        let manager = PermissionManager::new(vec![], vec![]);
        
        let result = manager.request_permission(
            Permission::Camera,
            "Camera needed for QR code scanning"
        ).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PermissionState::Granted);
    }
    
    #[tokio::test]
    async fn test_batch_permission_request() {
        let manager = PermissionManager::new(vec![], vec![]);
        
        let permissions = vec![
            Permission::Bluetooth,
            Permission::BluetoothAdmin,
            Permission::AccessCoarseLocation,
        ];
        
        let results = manager.request_permissions(
            permissions.clone(),
            "These permissions are needed for Bluetooth gaming"
        ).await;
        
        assert!(results.is_ok());
        
        let permission_states = results.unwrap();
        assert_eq!(permission_states.len(), permissions.len());
        
        for permission in permissions {
            assert_eq!(permission_states[&permission], PermissionState::Granted);
        }
    }
    
    #[test]
    fn test_permission_explanations() {
        let manager = PermissionManager::new(vec![], vec![]);
        
        let explanation = manager.get_permission_explanation(Permission::Bluetooth);
        assert!(explanation.contains("peer-to-peer gaming"));
        
        let location_explanation = manager.get_permission_explanation(Permission::AccessCoarseLocation);
        assert!(location_explanation.contains("Bluetooth Low Energy"));
    }
    
    #[test]
    fn test_onboarding_flow() {
        let manager = PermissionManager::new(vec![], vec![]);
        let mut flow = manager.create_onboarding_flow();
        
        assert!(!flow.is_complete());
        assert!(flow.steps.len() >= 3);
        
        // Check first step is Bluetooth
        let first_step = flow.current().unwrap();
        assert!(first_step.permissions.contains(&Permission::Bluetooth));
        assert!(first_step.required);
        
        // Move through flow
        while !flow.is_complete() {
            flow.next_step();
        }
        
        assert!(flow.is_complete());
    }
    
    #[test]
    fn test_permission_categories() {
        assert_eq!(Permission::Bluetooth.category(), PermissionCategory::Bluetooth);
        assert_eq!(Permission::AccessCoarseLocation.category(), PermissionCategory::Location);
        assert_eq!(Permission::Camera.category(), PermissionCategory::Media);
        assert_eq!(Permission::ForegroundService.category(), PermissionCategory::System);
    }
    
    #[test]
    fn test_permission_strings() {
        assert_eq!(
            Permission::Bluetooth.to_android_string(),
            "android.permission.BLUETOOTH"
        );
        
        assert_eq!(
            Permission::Camera.to_ios_string(),
            "NSCameraUsageDescription"
        );
    }
    
    #[test]
    fn test_can_function_without() {
        let required = vec![Permission::Bluetooth];
        let optional = vec![Permission::Camera];
        
        let manager = PermissionManager::new(required, optional);
        
        assert!(!manager.can_function_without(Permission::Bluetooth));
        assert!(manager.can_function_without(Permission::Camera));
    }
}