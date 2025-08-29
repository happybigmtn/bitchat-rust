//! BLE Peripheral Advertising for BitChat
//! 
//! This module provides platform-specific implementations for BLE peripheral
//! advertising since btleplug doesn't support peripheral mode on most platforms.
//! It works alongside btleplug for central mode (scanning) functionality.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

use crate::protocol::PeerId;
use crate::error::Result;

// Import platform-specific implementations
#[cfg(target_os = "android")]
use crate::transport::android_ble::AndroidBlePeripheral;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::transport::ios_ble::IosBlePeripheral;
#[cfg(target_os = "linux")]
use crate::transport::linux_ble::LinuxBlePeripheral;

/// BitCraps BLE Service UUID - same as used in bluetooth.rs
pub const BITCRAPS_SERVICE_UUID: Uuid = Uuid::from_u128(0x12345678_1234_5678_1234_567812345678);

/// BLE advertising configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvertisingConfig {
    /// Service UUID to advertise
    pub service_uuid: Uuid,
    /// Local device name
    pub local_name: String,
    /// Advertising interval in milliseconds (20ms - 10.24s range)
    pub advertising_interval_ms: u16,
    /// Transmit power level (-127 to +20 dBm)
    pub tx_power_level: i8,
    /// Whether to include device name in advertisement
    pub include_name: bool,
    /// Whether to make device connectable
    pub connectable: bool,
    /// Maximum number of simultaneous connections
    pub max_connections: u8,
}

impl Default for AdvertisingConfig {
    fn default() -> Self {
        Self {
            service_uuid: BITCRAPS_SERVICE_UUID,
            local_name: "BitChat".to_string(),
            advertising_interval_ms: 100, // 100ms interval
            tx_power_level: 0, // 0 dBm
            include_name: true,
            connectable: true,
            max_connections: 8,
        }
    }
}

/// BLE peripheral advertising events
#[derive(Debug, Clone)]
pub enum PeripheralEvent {
    /// Advertising started successfully
    AdvertisingStarted,
    /// Advertising stopped
    AdvertisingStopped,
    /// Central device connected
    CentralConnected { 
        peer_id: PeerId,
        central_address: String 
    },
    /// Central device disconnected
    CentralDisconnected { 
        peer_id: PeerId,
        reason: String 
    },
    /// Data received from central
    DataReceived { 
        peer_id: PeerId,
        data: Vec<u8> 
    },
    /// Error occurred
    Error { 
        error: String 
    },
    /// Advertising failed with recovery suggestion
    AdvertisingFailed {
        error: String,
        retry_suggested: bool,
        retry_delay_ms: u64,
    },
    /// Connection state changed
    ConnectionStateChanged {
        peer_id: PeerId,
        state: ConnectionState,
    },
    /// Platform-specific event
    PlatformEvent {
        platform: String,
        event_data: Vec<u8>,
    },
}

/// Connection states for state management
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Ready,
    Disconnecting,
    Error(String),
}

/// Statistics for BLE peripheral operations
#[derive(Debug, Clone, Default)]
pub struct PeripheralStats {
    /// Total time advertising has been active
    pub advertising_duration: Duration,
    /// Number of central connections received
    pub total_connections: u64,
    /// Currently connected centrals
    pub active_connections: usize,
    /// Bytes sent to centrals
    pub bytes_sent: u64,
    /// Bytes received from centrals
    pub bytes_received: u64,
    /// Number of advertising errors
    pub error_count: u64,
    /// Number of connection failures
    pub connection_failures: u64,
    /// Number of successful reconnections
    pub reconnection_attempts: u64,
    /// Average connection duration
    pub avg_connection_duration: Duration,
    /// Last error timestamp
    pub last_error_time: Option<Instant>,
    /// Platform-specific metrics
    pub platform_specific: HashMap<String, u64>,
}

/// Connection recovery configuration
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base delay between retries (exponential backoff)
    pub base_retry_delay_ms: u64,
    /// Maximum retry delay
    pub max_retry_delay_ms: u64,
    /// Timeout for connection attempts
    pub connection_timeout_ms: u64,
    /// Whether to enable automatic recovery
    pub auto_recovery_enabled: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_retry_delay_ms: 1000,
            max_retry_delay_ms: 30000,
            connection_timeout_ms: 10000,
            auto_recovery_enabled: true,
        }
    }
}

/// Core trait for BLE peripheral advertising
#[async_trait]
pub trait BlePeripheral: Send + Sync {
    /// Start advertising with the given configuration
    async fn start_advertising(&mut self, config: &AdvertisingConfig) -> Result<()>;
    
    /// Stop advertising
    async fn stop_advertising(&mut self) -> Result<()>;
    
    /// Check if currently advertising
    fn is_advertising(&self) -> bool;
    
    /// Send data to a connected central device
    async fn send_to_central(&mut self, peer_id: PeerId, data: &[u8]) -> Result<()>;
    
    /// Disconnect from a central device
    async fn disconnect_central(&mut self, peer_id: PeerId) -> Result<()>;
    
    /// Get list of connected central devices
    fn connected_centrals(&self) -> Vec<PeerId>;
    
    /// Get the next peripheral event
    async fn next_event(&mut self) -> Option<PeripheralEvent>;
    
    /// Get peripheral statistics
    async fn get_stats(&self) -> PeripheralStats;
    
    /// Update advertising configuration (may require restart)
    async fn update_config(&mut self, config: &AdvertisingConfig) -> Result<()>;
    
    /// Set recovery configuration
    async fn set_recovery_config(&mut self, config: RecoveryConfig) -> Result<()>;
    
    /// Trigger manual recovery attempt
    async fn recover(&mut self) -> Result<()>;
    
    /// Get current connection state for a peer
    async fn get_connection_state(&self, peer_id: PeerId) -> Option<ConnectionState>;
    
    /// Force reconnection to a specific central
    async fn force_reconnect(&mut self, peer_id: PeerId) -> Result<()>;
    
    /// Check platform-specific health status
    async fn health_check(&self) -> Result<bool>;
    
    /// Reset peripheral state (emergency recovery)
    async fn reset(&mut self) -> Result<()>;
}

/// Platform-specific BLE peripheral factory
pub struct BlePeripheralFactory;

impl BlePeripheralFactory {
    /// Create a platform-appropriate BLE peripheral implementation
    pub async fn create_peripheral(local_peer_id: PeerId) -> Result<Box<dyn BlePeripheral>> {
        #[cfg(target_os = "android")]
        {
            log::info!("Creating Android BLE peripheral implementation");
            Ok(Box::new(AndroidBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        {
            log::info!("Creating iOS/macOS BLE peripheral implementation");
            Ok(Box::new(IosBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(target_os = "linux")]
        {
            log::info!("Creating Linux BlueZ BLE peripheral implementation");
            Ok(Box::new(LinuxBlePeripheral::new(local_peer_id).await?))
        }
        
        #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos", target_os = "linux")))]
        {
            log::error!("No BLE peripheral implementation available for this platform");
            Err(Error::Network("BLE peripheral not supported on this platform".to_string()))
        }
    }
}