//! Integration tests for BLE peripheral implementations
//!
//! Tests the platform-specific BLE peripheral factory and basic functionality.

#[cfg(test)]
mod tests {
    use super::super::ble_peripheral::{AdvertisingConfig, BlePeripheral, BlePeripheralFactory};
    use crate::protocol::PeerId;

    #[tokio::test]
    async fn test_ble_peripheral_factory_creation() {
        // Test that we can create a peripheral for the current platform
        let peer_id = PeerId::from([1u8; 32]);

        // This should work on supported platforms, fail gracefully on unsupported ones
        match BlePeripheralFactory::create_peripheral(peer_id).await {
            Ok(mut peripheral) => {
                // Test basic trait methods
                assert!(!peripheral.is_advertising());
                assert!(peripheral.connected_centrals().is_empty());

                let stats = peripheral.get_stats().await;
                assert_eq!(stats.active_connections, 0);

                // Test config update
                let config = AdvertisingConfig::default();
                assert!(peripheral.update_config(&config).await.is_ok());

                log::info!("BLE peripheral factory test passed for current platform");
            }
            Err(e) => {
                // Expected on unsupported platforms
                log::info!("BLE peripheral not supported on this platform: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_advertising_config_defaults() {
        let config = AdvertisingConfig::default();

        assert_eq!(config.local_name, "BitChat");
        assert_eq!(config.advertising_interval_ms, 100);
        assert_eq!(config.tx_power_level, 0);
        assert!(config.include_name);
        assert!(config.connectable);
        assert_eq!(config.max_connections, 8);
    }

    #[cfg(target_os = "android")]
    #[tokio::test]
    async fn test_android_ble_peripheral_creation() {
        use super::super::android_ble::AndroidBlePeripheral;

        let peer_id = PeerId::from([2u8; 32]);
        let peripheral = AndroidBlePeripheral::new(peer_id).await;

        assert!(peripheral.is_ok());
        let peripheral = peripheral.unwrap();
        assert!(!peripheral.is_advertising());
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    #[tokio::test]
    async fn test_ios_ble_peripheral_creation() {
        use super::super::ios_ble::IosBlePeripheral;

        let peer_id = PeerId::from([3u8; 32]);
        let peripheral = IosBlePeripheral::new(peer_id).await;

        assert!(peripheral.is_ok());
        let peripheral = peripheral.unwrap();
        assert!(!peripheral.is_advertising());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn test_linux_ble_peripheral_creation() {
        use super::super::linux_ble::LinuxBlePeripheral;

        let peer_id = PeerId::from([4u8; 32]);
        let peripheral = LinuxBlePeripheral::new(peer_id).await;

        assert!(peripheral.is_ok());
        let peripheral = peripheral.unwrap();
        assert!(!peripheral.is_advertising());
    }
}
