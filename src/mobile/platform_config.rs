//! Platform-specific configuration and adaptations for mobile devices

use super::*;

/// Platform-specific configuration builder
pub struct PlatformConfigBuilder {
    config: PlatformConfig,
}

impl PlatformConfigBuilder {
    /// Create a new configuration builder for the specified platform
    pub fn new(platform: PlatformType) -> Self {
        let default_config = match platform {
            PlatformType::Android => Self::android_defaults(),
            PlatformType::iOS => Self::ios_defaults(),
            PlatformType::Desktop => Self::desktop_defaults(),
            PlatformType::Web => Self::web_defaults(),
            PlatformType::Unknown => Self::desktop_defaults(), // Default to desktop settings
        };

        Self {
            config: default_config,
        }
    }

    /// Enable or disable background scanning
    pub fn background_scanning(mut self, enabled: bool) -> Self {
        self.config.background_scanning = enabled;
        self
    }

    /// Set scan window in milliseconds
    pub fn scan_window_ms(mut self, window_ms: u32) -> Self {
        self.config.scan_window_ms = window_ms;
        self
    }

    /// Set scan interval in milliseconds
    pub fn scan_interval_ms(mut self, interval_ms: u32) -> Self {
        self.config.scan_interval_ms = interval_ms;
        self
    }

    /// Enable or disable low power mode
    pub fn low_power_mode(mut self, enabled: bool) -> Self {
        self.config.low_power_mode = enabled;
        self
    }

    /// Set service UUIDs for filtering
    pub fn service_uuids(mut self, uuids: Vec<String>) -> Self {
        self.config.service_uuids = uuids;
        self
    }

    /// Build the final configuration
    pub fn build(self) -> PlatformConfig {
        self.config
    }

    /// Android-specific default configuration
    fn android_defaults() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::Android,
            background_scanning: true,
            scan_window_ms: 500,    // 0.5 seconds
            scan_interval_ms: 1000, // 1 second
            low_power_mode: false,
            service_uuids: vec![
                "12345678-1234-5678-1234-567812345678".to_string() // BitCraps service UUID
            ],
        }
    }

    /// iOS-specific default configuration
    fn ios_defaults() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::iOS,
            background_scanning: false, // Limited on iOS
            scan_window_ms: 300,        // 0.3 seconds
            scan_interval_ms: 2000,     // 2 seconds (more conservative)
            low_power_mode: true,       // Enable by default
            service_uuids: vec![
                "12345678-1234-5678-1234-567812345678".to_string() // Required for iOS background
            ],
        }
    }

    /// Desktop default configuration
    fn desktop_defaults() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::Desktop,
            background_scanning: true,
            scan_window_ms: 1000,   // 1 second
            scan_interval_ms: 1000, // 1 second
            low_power_mode: false,
            service_uuids: vec![
                "12345678-1234-5678-1234-567812345678".to_string()
            ],
        }
    }

    /// Web default configuration
    fn web_defaults() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::Web,
            background_scanning: false, // Not supported
            scan_window_ms: 1000,       // 1 second
            scan_interval_ms: 1000,     // 1 second
            low_power_mode: false,
            service_uuids: vec![
                "12345678-1234-5678-1234-567812345678".to_string()
            ],
        }
    }
}

/// Android-specific platform adaptations
pub struct AndroidPlatformAdapter;

impl AndroidPlatformAdapter {
    /// Check if the app is whitelisted from battery optimization
    pub fn is_battery_optimized() -> bool {
        // TODO: Implement JNI call to check PowerManager.isIgnoringBatteryOptimizations()
        false
    }

    /// Request user to whitelist the app from battery optimization
    pub fn request_battery_optimization_whitelist() -> Result<(), BitCrapsError> {
        // TODO: Implement JNI call to start ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS intent
        log::info!("Requesting battery optimization whitelist");
        Ok(())
    }

    /// Check if the app has background app restrictions
    pub fn has_background_restrictions() -> bool {
        // TODO: Implement JNI call to check ActivityManager.isBackgroundRestricted()
        false
    }

    /// Configure foreground service for background operation
    pub fn configure_foreground_service() -> Result<(), BitCrapsError> {
        // TODO: Implement foreground service configuration via JNI
        log::info!("Configuring Android foreground service");
        Ok(())
    }

    /// Get optimal Bluetooth parameters for Android device
    pub fn get_optimal_bluetooth_params() -> (u32, u32) {
        // Return (scan_window_ms, scan_interval_ms) optimized for Android
        (500, 1000) // Default Android BLE parameters
    }
}

/// iOS-specific platform adaptations
pub struct IOSPlatformAdapter;

impl IOSPlatformAdapter {
    /// Check if background app refresh is enabled
    pub fn is_background_refresh_enabled() -> bool {
        // TODO: Implement check for UIApplication.backgroundRefreshStatus
        true
    }

    /// Handle app state transition for power optimization
    pub fn handle_app_state_change(to_background: bool) -> Result<(), BitCrapsError> {
        if to_background {
            log::info!("App moved to background - enabling power saving mode");
            // TODO: Implement background-specific optimizations
        } else {
            log::info!("App moved to foreground - resuming normal operation");
            // TODO: Implement foreground-specific optimizations
        }
        Ok(())
    }

    /// Configure Core Bluetooth for background operation
    pub fn configure_background_bluetooth() -> Result<(), BitCrapsError> {
        // TODO: Implement Core Bluetooth background configuration
        log::info!("Configuring iOS background Bluetooth");
        Ok(())
    }

    /// Get optimal Bluetooth parameters for iOS device
    pub fn get_optimal_bluetooth_params() -> (u32, u32) {
        // Return (scan_window_ms, scan_interval_ms) optimized for iOS
        (300, 2000) // Conservative iOS BLE parameters
    }
}

/// Platform detection utilities
pub struct PlatformDetection;

impl PlatformDetection {
    /// Detect the current platform
    pub fn detect_platform() -> PlatformType {
        #[cfg(target_os = "android")]
        return PlatformType::Android;
        
        #[cfg(target_os = "ios")]
        return PlatformType::iOS;
        
        #[cfg(target_arch = "wasm32")]
        return PlatformType::Web;
        
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        return PlatformType::Desktop;
    }

    /// Get platform-specific configuration
    pub fn get_platform_config() -> PlatformConfig {
        let platform = Self::detect_platform();
        PlatformConfigBuilder::new(platform).build()
    }

    /// Check if platform supports background operation
    pub fn supports_background_operation() -> bool {
        match Self::detect_platform() {
            PlatformType::Android => true,
            PlatformType::iOS => false, // Limited support
            PlatformType::Desktop => true,
            PlatformType::Web => false,
            PlatformType::Unknown => false,
        }
    }

    /// Get platform-specific Bluetooth limitations
    pub fn get_bluetooth_limitations() -> Vec<String> {
        let mut limitations = Vec::new();
        
        match Self::detect_platform() {
            PlatformType::Android => {
                limitations.push("Battery optimization may kill background scanning".to_string());
                limitations.push("Requires location permissions for BLE scanning".to_string());
                limitations.push("May be affected by Doze mode and App Standby".to_string());
            },
            PlatformType::iOS => {
                limitations.push("Background scanning severely limited".to_string());
                limitations.push("Local name not available in background".to_string());
                limitations.push("Service UUID filtering required for background".to_string());
                limitations.push("Connection intervals limited in background".to_string());
            },
            PlatformType::Desktop => {
                limitations.push("May require administrator privileges".to_string());
                limitations.push("Bluetooth adapter availability varies".to_string());
            },
            PlatformType::Web => {
                limitations.push("Web Bluetooth API has limited device support".to_string());
                limitations.push("Requires user gesture to initiate scanning".to_string());
                limitations.push("No background operation support".to_string());
            },
            PlatformType::Unknown => {
                limitations.push("Platform not recognized - functionality may be limited".to_string());
            },
        }
        
        limitations
    }
}

/// Cross-platform compatibility layer
pub struct CompatibilityLayer;

impl CompatibilityLayer {
    /// Get maximum supported connections for the platform
    pub fn max_connections() -> u32 {
        match PlatformDetection::detect_platform() {
            PlatformType::Android => 7,  // Most Android devices support 7 concurrent connections
            PlatformType::iOS => 10,     // iOS typically supports more connections
            PlatformType::Desktop => 20, // Desktop has fewer limitations
            PlatformType::Web => 1,      // Web Bluetooth is very limited
            PlatformType::Unknown => 1,  // Conservative default
        }
    }

    /// Get optimal MTU size for the platform
    pub fn optimal_mtu_size() -> u16 {
        match PlatformDetection::detect_platform() {
            PlatformType::Android => 244, // Android default MTU
            PlatformType::iOS => 185,     // iOS conservative MTU
            PlatformType::Desktop => 512, // Desktop can handle larger MTUs
            PlatformType::Web => 20,      // Web Bluetooth has small MTU
            PlatformType::Unknown => 20,  // Conservative default
        }
    }

    /// Check if platform requires specific permissions
    pub fn required_permissions() -> Vec<String> {
        let mut permissions = Vec::new();
        
        match PlatformDetection::detect_platform() {
            PlatformType::Android => {
                permissions.push("android.permission.BLUETOOTH".to_string());
                permissions.push("android.permission.BLUETOOTH_ADMIN".to_string());
                permissions.push("android.permission.BLUETOOTH_ADVERTISE".to_string());
                permissions.push("android.permission.BLUETOOTH_CONNECT".to_string());
                permissions.push("android.permission.ACCESS_FINE_LOCATION".to_string());
            },
            PlatformType::iOS => {
                permissions.push("NSBluetoothAlwaysUsageDescription".to_string());
                permissions.push("NSBluetoothPeripheralUsageDescription".to_string());
            },
            _ => {}, // Desktop and Web don't require specific permissions
        }
        
        permissions
    }

    /// Apply platform-specific workarounds
    pub fn apply_platform_workarounds(config: &mut PlatformConfig) {
        match config.platform {
            PlatformType::Android => {
                // Android workarounds
                if config.scan_interval_ms < 100 {
                    config.scan_interval_ms = 100; // Minimum interval for Android
                }
            },
            PlatformType::iOS => {
                // iOS workarounds
                if config.background_scanning && config.service_uuids.is_empty() {
                    log::warn!("iOS background scanning requires service UUIDs");
                    config.background_scanning = false;
                }
            },
            _ => {},
        }
    }
}