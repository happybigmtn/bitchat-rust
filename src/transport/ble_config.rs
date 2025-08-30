//! BLE Peripheral Advertising Configuration and Utilities
//!
//! This module provides configuration management and initialization helpers
//! for BLE peripheral advertising across different platforms.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::{AdvertisingConfig, TransportCoordinator, BITCRAPS_SERVICE_UUID};

/// Complete BLE configuration for BitChat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleTransportConfig {
    /// BLE advertising configuration
    pub advertising: AdvertisingConfig,

    /// Whether to enable BLE peripheral mode (advertising)
    pub enable_peripheral: bool,

    /// Whether to enable BLE central mode (scanning)
    pub enable_central: bool,

    /// Auto-start mesh mode on initialization
    pub auto_start_mesh: bool,

    /// Connection management settings
    pub connection_settings: BleConnectionSettings,

    /// Platform-specific settings
    pub platform_settings: PlatformSpecificSettings,
}

/// BLE connection management settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BleConnectionSettings {
    /// Maximum number of simultaneous peripheral connections
    pub max_peripheral_connections: u8,

    /// Maximum number of simultaneous central connections
    pub max_central_connections: usize,

    /// Connection timeout for outgoing connections
    pub connection_timeout: Duration,

    /// Advertising restart interval if it fails
    pub advertising_restart_interval: Duration,

    /// Whether to automatically reconnect to known peers
    pub auto_reconnect: bool,

    /// Interval for connection health checks
    pub health_check_interval: Duration,
}

/// Platform-specific BLE settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSpecificSettings {
    /// Android-specific settings
    #[cfg(target_os = "android")]
    pub android: AndroidBleSettings,

    /// iOS/macOS-specific settings
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub ios: IosBleSettings,

    /// Linux-specific settings
    #[cfg(target_os = "linux")]
    pub linux: LinuxBleSettings,

    /// Fallback settings for unsupported platforms
    pub fallback: FallbackBleSettings,
}

/// Android BLE settings
#[cfg(target_os = "android")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidBleSettings {
    /// Use foreground service for BLE operations
    pub use_foreground_service: bool,

    /// Request battery optimization exemption
    pub request_battery_optimization_exemption: bool,

    /// Wake lock for BLE operations
    pub use_wake_lock: bool,

    /// Advertise mode preference
    pub advertise_mode_preference: AndroidAdvertiseMode,

    /// GATT server connection priority
    pub connection_priority: AndroidConnectionPriority,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AndroidAdvertiseMode {
    LowLatency,
    Balanced,
    LowPower,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AndroidConnectionPriority {
    Balanced,
    High,
    LowPower,
}

/// iOS/macOS BLE settings
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosBleSettings {
    /// Background modes for BLE operations
    pub background_modes: Vec<String>,

    /// Restore identifier for state restoration
    pub restore_identifier: Option<String>,

    /// Whether to show power alert when Bluetooth is off
    pub show_power_alert: bool,

    /// Peripheral manager options
    pub peripheral_manager_options: IosCBManagerOptions,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosCBManagerOptions {
    /// Whether to show power alert
    pub show_power_alert: bool,

    /// Restore identifier
    pub restore_identifier: Option<String>,
}

/// Linux BLE settings
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxBleSettings {
    /// BlueZ adapter to use (e.g., "hci0")
    pub adapter_name: String,

    /// D-Bus connection timeout
    pub dbus_timeout: Duration,

    /// Whether to auto-power-on the adapter
    pub auto_power_on: bool,

    /// GATT service registration timeout
    pub service_registration_timeout: Duration,

    /// Advertisement registration timeout
    pub advertisement_registration_timeout: Duration,
}

/// Fallback settings for unsupported platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackBleSettings {
    /// Whether to emit warnings about unsupported operations
    pub emit_warnings: bool,

    /// Simulated operation delays for testing
    pub simulated_delays: bool,
}

impl Default for BleTransportConfig {
    fn default() -> Self {
        Self {
            advertising: AdvertisingConfig::default(),
            enable_peripheral: true,
            enable_central: true,
            auto_start_mesh: false,
            connection_settings: BleConnectionSettings::default(),
            platform_settings: PlatformSpecificSettings::default(),
        }
    }
}

impl Default for BleConnectionSettings {
    fn default() -> Self {
        Self {
            max_peripheral_connections: 8,
            max_central_connections: 10,
            connection_timeout: Duration::from_secs(30),
            advertising_restart_interval: Duration::from_secs(5),
            auto_reconnect: true,
            health_check_interval: Duration::from_secs(30),
        }
    }
}

impl Default for PlatformSpecificSettings {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "android")]
            android: AndroidBleSettings::default(),

            #[cfg(any(target_os = "ios", target_os = "macos"))]
            ios: IosBleSettings::default(),

            #[cfg(target_os = "linux")]
            linux: LinuxBleSettings::default(),

            fallback: FallbackBleSettings::default(),
        }
    }
}

#[cfg(target_os = "android")]
impl Default for AndroidBleSettings {
    fn default() -> Self {
        Self {
            use_foreground_service: true,
            request_battery_optimization_exemption: true,
            use_wake_lock: false,
            advertise_mode_preference: AndroidAdvertiseMode::Balanced,
            connection_priority: AndroidConnectionPriority::Balanced,
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Default for IosBleSettings {
    fn default() -> Self {
        Self {
            background_modes: vec!["bluetooth-peripheral".to_string()],
            restore_identifier: Some("BitChatPeripheralManager".to_string()),
            show_power_alert: true,
            peripheral_manager_options: IosCBManagerOptions::default(),
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Default for IosCBManagerOptions {
    fn default() -> Self {
        Self {
            show_power_alert: true,
            restore_identifier: Some("BitChatPeripheralManager".to_string()),
        }
    }
}

#[cfg(target_os = "linux")]
impl Default for LinuxBleSettings {
    fn default() -> Self {
        Self {
            adapter_name: "hci0".to_string(),
            dbus_timeout: Duration::from_secs(10),
            auto_power_on: true,
            service_registration_timeout: Duration::from_secs(10),
            advertisement_registration_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for FallbackBleSettings {
    fn default() -> Self {
        Self {
            emit_warnings: true,
            simulated_delays: false,
        }
    }
}

/// BLE transport initialization helper
pub struct BleTransportInitializer {
    config: BleTransportConfig,
    local_peer_id: PeerId,
}

impl BleTransportInitializer {
    /// Create new initializer with configuration
    pub fn new(local_peer_id: PeerId, config: BleTransportConfig) -> Self {
        Self {
            config,
            local_peer_id,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(local_peer_id: PeerId) -> Self {
        Self::new(local_peer_id, BleTransportConfig::default())
    }

    /// Create BitChat configuration with custom service name
    pub fn for_bitchat(local_peer_id: PeerId, device_name: &str) -> Self {
        let mut config = BleTransportConfig::default();
        config.advertising.local_name = device_name.to_string();
        config.advertising.service_uuid = BITCRAPS_SERVICE_UUID;
        config.auto_start_mesh = true;

        Self::new(local_peer_id, config)
    }

    /// Initialize enhanced Bluetooth transport with the configuration
    pub async fn initialize_transport(&self) -> Result<TransportCoordinator> {
        log::info!(
            "Initializing BLE transport for peer {:?}",
            self.local_peer_id
        );

        let mut coordinator = TransportCoordinator::new();

        // Initialize enhanced Bluetooth transport
        coordinator
            .init_enhanced_bluetooth(self.local_peer_id)
            .await?;

        // Start mesh mode if configured
        if self.config.auto_start_mesh {
            log::info!("Auto-starting mesh mode");
            coordinator
                .start_mesh_mode(self.config.advertising.clone())
                .await?;
        }

        log::info!("BLE transport initialization completed");
        Ok(coordinator)
    }

    /// Validate configuration for current platform
    pub fn validate_config(&self) -> Result<()> {
        log::debug!("Validating BLE configuration");

        // Validate advertising config
        if self.config.advertising.advertising_interval_ms < 20
            || self.config.advertising.advertising_interval_ms > 10240
        {
            return Err(Error::Network(
                "Advertising interval must be between 20ms and 10.24s".to_string(),
            ));
        }

        if self.config.advertising.tx_power_level < -127
            || self.config.advertising.tx_power_level > 20
        {
            return Err(Error::Network(
                "TX power level must be between -127 and +20 dBm".to_string(),
            ));
        }

        if self.config.advertising.max_connections == 0 {
            return Err(Error::Network(
                "Maximum connections must be at least 1".to_string(),
            ));
        }

        // Platform-specific validation
        self.validate_platform_config()?;

        log::debug!("BLE configuration validation passed");
        Ok(())
    }

    /// Platform-specific configuration validation
    fn validate_platform_config(&self) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            // Android-specific validation
            if self.config.connection_settings.max_peripheral_connections > 8 {
                log::warn!("Android typically supports max 8 peripheral connections");
            }
        }

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            // iOS-specific validation
            if self.config.advertising.advertising_interval_ms < 100 {
                log::warn!("iOS may not support advertising intervals below 100ms");
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux-specific validation
            let linux_settings = &self.config.platform_settings.linux;
            if !linux_settings.adapter_name.starts_with("hci") {
                log::warn!("Linux adapter name should typically start with 'hci'");
            }
        }

        Ok(())
    }

    /// Get platform capabilities summary
    pub fn get_platform_capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::for_current_platform()
    }

    /// Update configuration
    pub fn with_config(mut self, config: BleTransportConfig) -> Self {
        self.config = config;
        self
    }

    /// Set advertising configuration
    pub fn with_advertising(mut self, advertising: AdvertisingConfig) -> Self {
        self.config.advertising = advertising;
        self
    }

    /// Enable/disable auto-start mesh mode
    pub fn with_auto_mesh(mut self, auto_start: bool) -> Self {
        self.config.auto_start_mesh = auto_start;
        self
    }
}

/// Platform capability information
#[derive(Debug, Clone)]
pub struct PlatformCapabilities {
    /// Platform name
    pub platform: String,

    /// Whether BLE peripheral mode is supported
    pub supports_peripheral: bool,

    /// Whether BLE central mode is supported
    pub supports_central: bool,

    /// Whether background operation is supported
    pub supports_background: bool,

    /// Maximum simultaneous connections
    pub max_connections: Option<u8>,

    /// Supported advertising intervals (min, max) in milliseconds
    pub advertising_interval_range: Option<(u16, u16)>,

    /// Supported TX power levels (min, max) in dBm
    pub tx_power_range: Option<(i8, i8)>,

    /// Platform-specific limitations
    pub limitations: Vec<String>,

    /// Required permissions or setup steps
    pub requirements: Vec<String>,
}

impl PlatformCapabilities {
    /// Get capabilities for the current platform
    pub fn for_current_platform() -> Self {
        #[cfg(target_os = "android")]
        {
            Self {
                platform: "Android".to_string(),
                supports_peripheral: true,
                supports_central: true,
                supports_background: true,
                max_connections: Some(8),
                advertising_interval_range: Some((20, 10240)),
                tx_power_range: Some((-21, 1)),
                limitations: vec![
                    "Battery optimization may kill service".to_string(),
                    "BLE peripheral mode has limited support on older devices".to_string(),
                    "Some manufacturers disable BLE advertising".to_string(),
                ],
                requirements: vec![
                    "BLUETOOTH permission".to_string(),
                    "BLUETOOTH_ADMIN permission".to_string(),
                    "ACCESS_COARSE_LOCATION permission".to_string(),
                    "Foreground service for background operation".to_string(),
                ],
            }
        }

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            Self {
                platform: if cfg!(target_os = "ios") {
                    "iOS".to_string()
                } else {
                    "macOS".to_string()
                },
                supports_peripheral: true,
                supports_central: true,
                supports_background: cfg!(target_os = "ios"), // Limited on iOS
                max_connections: None,
                advertising_interval_range: Some((100, 10240)), // iOS restrictions
                tx_power_range: None,                           // Not controllable
                limitations: vec![
                    "Background BLE has severe restrictions on iOS".to_string(),
                    "Local name not advertised in background".to_string(),
                    "Service UUID filtering required in background".to_string(),
                    "Peripheral cannot disconnect central".to_string(),
                ],
                requirements: vec![
                    "NSBluetoothAlwaysUsageDescription in Info.plist".to_string(),
                    "NSBluetoothPeripheralUsageDescription in Info.plist".to_string(),
                    "Background modes: bluetooth-peripheral".to_string(),
                ],
            }
        }

        #[cfg(target_os = "linux")]
        {
            Self {
                platform: "Linux".to_string(),
                supports_peripheral: true,
                supports_central: true,
                supports_background: true,
                max_connections: None,
                advertising_interval_range: Some((20, 10240)),
                tx_power_range: Some((-127, 20)),
                limitations: vec![
                    "Requires BlueZ 5.40+".to_string(),
                    "Root permissions may be required".to_string(),
                    "Adapter must support BLE".to_string(),
                ],
                requirements: vec![
                    "BlueZ installed and running".to_string(),
                    "D-Bus access permissions".to_string(),
                    "Bluetooth adapter powered on".to_string(),
                ],
            }
        }

        #[cfg(target_os = "windows")]
        {
            Self {
                platform: "Windows".to_string(),
                supports_peripheral: true,
                supports_central: true,
                supports_background: false,
                max_connections: None,
                advertising_interval_range: Some((100, 10240)),
                tx_power_range: None,
                limitations: vec![
                    "Requires Windows 10 version 1703+".to_string(),
                    "BLE peripheral support varies by hardware".to_string(),
                ],
                requirements: vec![
                    "Windows Runtime APIs access".to_string(),
                    "Bluetooth capability in app manifest".to_string(),
                ],
            }
        }

        #[cfg(not(any(
            target_os = "android",
            target_os = "ios",
            target_os = "macos",
            target_os = "linux",
            target_os = "windows"
        )))]
        {
            Self {
                platform: "Unsupported".to_string(),
                supports_peripheral: false,
                supports_central: false,
                supports_background: false,
                max_connections: None,
                advertising_interval_range: None,
                tx_power_range: None,
                limitations: vec!["Platform not supported".to_string()],
                requirements: vec![],
            }
        }
    }
}

/// Configuration builder for BLE transport
pub struct BleConfigBuilder {
    config: BleTransportConfig,
}

impl BleConfigBuilder {
    /// Start building configuration
    pub fn new() -> Self {
        Self {
            config: BleTransportConfig::default(),
        }
    }

    /// Set service UUID
    pub fn service_uuid(mut self, uuid: Uuid) -> Self {
        self.config.advertising.service_uuid = uuid;
        self
    }

    /// Set local device name
    pub fn local_name(mut self, name: String) -> Self {
        self.config.advertising.local_name = name;
        self
    }

    /// Set advertising interval
    pub fn advertising_interval(mut self, interval_ms: u16) -> Self {
        self.config.advertising.advertising_interval_ms = interval_ms;
        self
    }

    /// Set TX power level
    pub fn tx_power(mut self, power_dbm: i8) -> Self {
        self.config.advertising.tx_power_level = power_dbm;
        self
    }

    /// Enable/disable peripheral mode
    pub fn peripheral_mode(mut self, enabled: bool) -> Self {
        self.config.enable_peripheral = enabled;
        self
    }

    /// Enable/disable central mode
    pub fn central_mode(mut self, enabled: bool) -> Self {
        self.config.enable_central = enabled;
        self
    }

    /// Set maximum peripheral connections
    pub fn max_peripheral_connections(mut self, max: u8) -> Self {
        self.config.connection_settings.max_peripheral_connections = max;
        self
    }

    /// Set connection timeout
    pub fn connection_timeout(mut self, timeout: Duration) -> Self {
        self.config.connection_settings.connection_timeout = timeout;
        self
    }

    /// Enable auto-start mesh mode
    pub fn auto_start_mesh(mut self) -> Self {
        self.config.auto_start_mesh = true;
        self
    }

    /// Build the configuration
    pub fn build(self) -> BleTransportConfig {
        self.config
    }
}
