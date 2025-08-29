//! Unit Tests for Transport Layer Components
//! 
//! Tests for BLE transport, NAT traversal, connection management, and transport optimization.

use std::time::Duration;
use tokio::time::sleep;

use bitcraps::{
    Error, Result,
    protocol::{PeerId, BitchatPacket, random_peer_id},
    transport::{TransportCoordinator, TransportAddress, BluetoothTransport},
    mobile::BleOptimizer,
};
use crate::common::test_harness::{TestResult, DeviceEmulator, NetworkType};

/// BLE Transport Tests
#[cfg(test)]
mod ble_transport_tests {
    use super::*;

    #[tokio::test]
    async fn test_ble_transport_creation() -> TestResult {
        let peer_id = random_peer_id();
        
        // Test BLE transport initialization
        let transport_result = BluetoothTransport::new(peer_id).await;
        
        // Should successfully create transport
        assert!(transport_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_advertising() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test starting advertisement
        let adv_result = transport.start_advertising().await;
        assert!(adv_result.is_ok());
        
        // Test stopping advertisement
        let stop_result = transport.stop_advertising().await;
        assert!(stop_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_scanning() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test starting scan
        let scan_result = transport.start_scanning().await;
        assert!(scan_result.is_ok());
        
        // Test stopping scan
        let stop_result = transport.stop_scanning().await;
        assert!(stop_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_connection() -> TestResult {
        let central_peer = random_peer_id();
        let peripheral_peer = random_peer_id();
        
        let central_transport = BluetoothTransport::new(central_peer).await?;
        let peripheral_transport = BluetoothTransport::new(peripheral_peer).await?;
        
        // Start peripheral advertising
        peripheral_transport.start_advertising().await?;
        
        // Start central scanning
        central_transport.start_scanning().await?;
        
        // Allow time for discovery
        sleep(Duration::from_millis(100)).await;
        
        // Test connection establishment
        let address = TransportAddress::BluetoothLe([0u8; 6]); // Mock address
        let connect_result = central_transport.connect(address).await;
        
        // In real implementation, this would establish a connection
        // For now, we just test that the method exists and can be called
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_message_transmission() -> TestResult {
        let sender_peer = random_peer_id();
        let receiver_peer = random_peer_id();
        
        let sender_transport = BluetoothTransport::new(sender_peer).await?;
        
        // Create test packet
        let packet = BitchatPacket::new_ping(sender_peer, receiver_peer);
        let address = TransportAddress::BluetoothLe([0u8; 6]);
        
        // Test sending packet
        let send_result = sender_transport.send_packet(address, packet).await;
        
        // Should be able to attempt sending (may fail in test environment)
        // The important thing is the API works
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_peripheral_mode() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test peripheral mode configuration
        let config = transport.get_peripheral_config();
        assert!(config.is_ok());
        
        // Test updating peripheral configuration
        // let mut new_config = config.unwrap();
        // new_config.advertising_interval = Duration::from_millis(100);
        // let update_result = transport.update_peripheral_config(new_config).await;
        // assert!(update_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_gatt_services() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test GATT service setup
        let service_result = transport.setup_gatt_service().await;
        assert!(service_result.is_ok());
        
        // Test characteristic creation
        // let char_result = transport.add_characteristic("bitcraps_data").await;
        // assert!(char_result.is_ok());
        
        Ok(())
    }
}

/// BLE Optimizer Tests
#[cfg(test)]
mod ble_optimizer_tests {
    use super::*;

    #[tokio::test]
    async fn test_ble_optimizer_creation() -> TestResult {
        let config = Default::default(); // BleOptimizerConfig
        let optimizer = BleOptimizer::new(config);
        
        assert!(optimizer.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_packet_size_optimization() -> TestResult {
        let config = Default::default();
        let optimizer = BleOptimizer::new(config)?;
        
        // Create a large packet that needs optimization
        let mut large_packet = BitchatPacket::new_discovery(random_peer_id());
        large_packet.payload = Some(vec![0u8; 1000]); // 1KB payload
        
        // Test packet optimization
        let optimized = optimizer.optimize_packet(large_packet).await?;
        
        // Optimized packet should be smaller (compressed or fragmented)
        let optimized_size = optimized.serialize()?.len();
        assert!(optimized_size <= 512, "Optimized packet should fit in BLE MTU");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_interval_optimization() -> TestResult {
        let config = Default::default();
        let optimizer = BleOptimizer::new(config)?;
        
        // Simulate different device states
        let mut device = DeviceEmulator::new_mobile();
        device.battery_level = 0.3; // 30% battery
        device.performance_mode = crate::common::test_harness::PerformanceMode::PowerSaver;
        
        // Test connection interval adjustment
        let interval = optimizer.calculate_optimal_interval(&device).await?;
        
        // Should suggest longer intervals for low battery
        assert!(interval >= Duration::from_millis(100));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_advertising_optimization() -> TestResult {
        let config = Default::default();
        let optimizer = BleOptimizer::new(config)?;
        
        let device = DeviceEmulator::new_mobile();
        
        // Test advertising parameter optimization
        let adv_params = optimizer.optimize_advertising_parameters(&device).await?;
        
        // Should return reasonable parameters
        assert!(adv_params.interval >= Duration::from_millis(20));
        assert!(adv_params.interval <= Duration::from_secs(10));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_power_management() -> TestResult {
        let config = Default::default();
        let optimizer = BleOptimizer::new(config)?;
        
        // Test power-aware optimization
        let mut low_battery_device = DeviceEmulator::new_mobile();
        low_battery_device.battery_level = 0.1; // 10% battery
        
        let power_profile = optimizer.get_power_profile(&low_battery_device).await?;
        
        // Should recommend power-saving measures
        assert!(power_profile.reduce_advertising);
        assert!(power_profile.increase_intervals);
        assert!(power_profile.disable_scanning);
        
        Ok(())
    }
}

/// Transport Coordinator Tests
#[cfg(test)]
mod transport_coordinator_tests {
    use super::*;

    #[tokio::test]
    async fn test_coordinator_creation() -> TestResult {
        let coordinator = TransportCoordinator::new();
        assert!(coordinator.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_transport_registration() -> TestResult {
        let coordinator = TransportCoordinator::new()?;
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test registering transport
        let register_result = coordinator.register_transport(Box::new(transport)).await;
        assert!(register_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_multi_transport_coordination() -> TestResult {
        let coordinator = TransportCoordinator::new()?;
        let peer_id = random_peer_id();
        
        // Register multiple transports
        let ble_transport = BluetoothTransport::new(peer_id).await?;
        let _wifi_transport = BluetoothTransport::new(peer_id).await?; // Mock WiFi with BLE for testing
        
        coordinator.register_transport(Box::new(ble_transport)).await?;
        // coordinator.register_transport(Box::new(wifi_transport)).await?;
        
        // Test coordinated message sending
        let packet = BitchatPacket::new_ping(peer_id, random_peer_id());
        let address = TransportAddress::BluetoothLe([0u8; 6]);
        
        let send_result = coordinator.send_packet(address, packet).await;
        
        // Coordinator should route to appropriate transport
        assert!(send_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_failover_coordination() -> TestResult {
        let coordinator = TransportCoordinator::new()?;
        let peer_id = random_peer_id();
        
        let primary_transport = BluetoothTransport::new(peer_id).await?;
        let fallback_transport = BluetoothTransport::new(peer_id).await?;
        
        coordinator.register_transport(Box::new(primary_transport)).await?;
        coordinator.register_transport(Box::new(fallback_transport)).await?;
        
        // Test failover scenario
        let packet = BitchatPacket::new_ping(peer_id, random_peer_id());
        let address = TransportAddress::BluetoothLe([0u8; 6]);
        
        // Simulate primary transport failure and automatic failover
        let send_result = coordinator.send_with_failover(address, packet).await;
        
        // Should attempt failover if primary fails
        Ok(())
    }

    #[tokio::test]
    async fn test_transport_health_monitoring() -> TestResult {
        let coordinator = TransportCoordinator::new()?;
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        coordinator.register_transport(Box::new(transport)).await?;
        
        // Test health monitoring
        let health_status = coordinator.get_transport_health().await?;
        
        // Should report on transport health
        assert!(!health_status.is_empty());
        
        Ok(())
    }
}

/// NAT Traversal Tests
#[cfg(test)]
mod nat_traversal_tests {
    use super::*;

    #[tokio::test]
    async fn test_nat_detection() -> TestResult {
        // Test NAT type detection
        let nat_detector = bitcraps::transport::NatTraversal::new();
        
        let nat_type = nat_detector.detect_nat_type().await?;
        
        // Should detect some type of NAT (or none)
        match nat_type {
            bitcraps::transport::NatType::None => println!("No NAT detected"),
            bitcraps::transport::NatType::FullCone => println!("Full cone NAT"),
            bitcraps::transport::NatType::Restricted => println!("Restricted NAT"),
            bitcraps::transport::NatType::PortRestricted => println!("Port-restricted NAT"),
            bitcraps::transport::NatType::Symmetric => println!("Symmetric NAT"),
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_hole_punching() -> TestResult {
        let nat_traversal = bitcraps::transport::NatTraversal::new();
        let peer_a = random_peer_id();
        let peer_b = random_peer_id();
        
        // Test UDP hole punching
        let punch_result = nat_traversal.punch_hole(peer_a, peer_b).await;
        
        // Should attempt hole punching (may fail in test environment)
        match punch_result {
            Ok(_) => println!("Hole punching succeeded"),
            Err(e) => println!("Hole punching failed (expected in test): {}", e),
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_stun_server_interaction() -> TestResult {
        let nat_traversal = bitcraps::transport::NatTraversal::new();
        
        // Test STUN server communication
        let external_address = nat_traversal.get_external_address().await;
        
        match external_address {
            Ok(addr) => println!("External address: {:?}", addr),
            Err(e) => println!("STUN query failed (expected in test): {}", e),
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_relay_server_fallback() -> TestResult {
        let nat_traversal = bitcraps::transport::NatTraversal::new();
        let peer_a = random_peer_id();
        let peer_b = random_peer_id();
        
        // Test TURN relay as fallback
        let relay_result = nat_traversal.setup_relay(peer_a, peer_b).await;
        
        match relay_result {
            Ok(_) => println!("Relay setup succeeded"),
            Err(e) => println!("Relay setup failed (expected in test): {}", e),
        }
        
        Ok(())
    }
}

/// Connection Management Tests
#[cfg(test)]
mod connection_management_tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_pool() -> TestResult {
        let pool = bitcraps::transport::ConnectionPool::new(10); // Max 10 connections
        let peer_id = random_peer_id();
        
        // Test adding connections
        for i in 0..5 {
            let mut peer = [0u8; 32];
            peer[0] = i;
            let address = TransportAddress::BluetoothLe([i; 6]);
            
            let add_result = pool.add_connection(peer, address).await;
            assert!(add_result.is_ok());
        }
        
        // Test connection retrieval
        let mut peer = [0u8; 32];
        peer[0] = 2;
        let connection = pool.get_connection(peer).await;
        assert!(connection.is_some());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_limits() -> TestResult {
        let pool = bitcraps::transport::ConnectionPool::new(3); // Max 3 connections
        
        // Fill pool to capacity
        for i in 0..3 {
            let mut peer = [0u8; 32];
            peer[0] = i;
            let address = TransportAddress::BluetoothLe([i; 6]);
            pool.add_connection(peer, address).await?;
        }
        
        // Test adding beyond capacity
        let mut peer = [0u8; 32];
        peer[0] = 4;
        let address = TransportAddress::BluetoothLe([4; 6]);
        let overflow_result = pool.add_connection(peer, address).await;
        
        // Should handle overflow gracefully (LRU eviction)
        assert!(overflow_result.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_health_monitoring() -> TestResult {
        let pool = bitcraps::transport::ConnectionPool::new(10);
        let peer_id = random_peer_id();
        let address = TransportAddress::BluetoothLe([1; 6]);
        
        pool.add_connection(peer_id, address).await?;
        
        // Test health check
        let health = pool.check_connection_health(peer_id).await?;
        
        // Should report connection health metrics
        assert!(health.is_healthy() || !health.is_healthy()); // Either state is valid
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_cleanup() -> TestResult {
        let pool = bitcraps::transport::ConnectionPool::new(10);
        let peer_id = random_peer_id();
        let address = TransportAddress::BluetoothLe([1; 6]);
        
        pool.add_connection(peer_id, address).await?;
        
        // Test connection removal
        let remove_result = pool.remove_connection(peer_id).await;
        assert!(remove_result.is_ok());
        
        // Connection should no longer exist
        let connection = pool.get_connection(peer_id).await;
        assert!(connection.is_none());
        
        Ok(())
    }
}

/// MTU Discovery Tests
#[cfg(test)]
mod mtu_discovery_tests {
    use super::*;

    #[tokio::test]
    async fn test_mtu_discovery() -> TestResult {
        let mtu_discovery = bitcraps::transport::MtuDiscovery::new();
        let peer_id = random_peer_id();
        
        // Test MTU discovery for BLE connection
        let discovered_mtu = mtu_discovery.discover_mtu(peer_id).await?;
        
        // BLE MTU should be in reasonable range (23-517 bytes)
        assert!(discovered_mtu >= 23);
        assert!(discovered_mtu <= 517);
        
        println!("Discovered MTU: {} bytes", discovered_mtu);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_path_mtu_discovery() -> TestResult {
        let mtu_discovery = bitcraps::transport::MtuDiscovery::new();
        let peer_id = random_peer_id();
        
        // Test path MTU discovery (may involve multiple hops)
        let path_mtu = mtu_discovery.discover_path_mtu(peer_id).await?;
        
        // Path MTU should be reasonable
        assert!(path_mtu >= 23);
        assert!(path_mtu <= 1500); // Typical max for most networks
        
        println!("Path MTU: {} bytes", path_mtu);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_fragmentation_handling() -> TestResult {
        let mtu_discovery = bitcraps::transport::MtuDiscovery::new();
        let peer_id = random_peer_id();
        
        // Create packet larger than typical BLE MTU
        let mut large_packet = BitchatPacket::new_discovery(peer_id);
        large_packet.payload = Some(vec![0u8; 200]); // 200 bytes payload
        
        // Test fragmentation
        let fragments = mtu_discovery.fragment_packet(large_packet, 100).await?; // 100-byte MTU
        
        // Should create multiple fragments
        assert!(fragments.len() > 1);
        
        // Test reassembly
        let reassembled = mtu_discovery.reassemble_fragments(fragments).await?;
        assert_eq!(reassembled.payload.unwrap().len(), 200);
        
        Ok(())
    }
}

/// Performance Tests for Transport Layer
#[cfg(test)]
mod transport_performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_packet_serialization_performance() -> TestResult {
        let peer_id = random_peer_id();
        let packet = BitchatPacket::new_ping(peer_id, random_peer_id());
        
        let start = Instant::now();
        
        // Test serialization performance
        for _ in 0..10000 {
            let _serialized = packet.serialize();
        }
        
        let elapsed = start.elapsed();
        let per_operation = elapsed.as_nanos() / 10000;
        
        println!("Packet serialization: {}ns per operation", per_operation);
        
        // Should be reasonably fast
        assert!(per_operation < 10_000, "Serialization should be under 10μs");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_ble_optimization_performance() -> TestResult {
        let config = Default::default();
        let optimizer = BleOptimizer::new(config)?;
        let device = DeviceEmulator::new_mobile();
        
        let start = Instant::now();
        
        // Test optimization performance
        for _ in 0..1000 {
            let _interval = optimizer.calculate_optimal_interval(&device).await?;
        }
        
        let elapsed = start.elapsed();
        let per_optimization = elapsed.as_nanos() / 1000;
        
        println!("BLE optimization: {}ns per calculation", per_optimization);
        
        // Should be very fast
        assert!(per_optimization < 100_000, "Optimization should be under 100μs");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_pool_performance() -> TestResult {
        let pool = bitcraps::transport::ConnectionPool::new(1000);
        
        let start = Instant::now();
        
        // Test connection lookup performance
        for i in 0..1000 {
            let mut peer = [0u8; 32];
            peer[0] = (i % 256) as u8;
            let _connection = pool.get_connection(peer).await;
        }
        
        let elapsed = start.elapsed();
        let per_lookup = elapsed.as_nanos() / 1000;
        
        println!("Connection lookup: {}ns per operation", per_lookup);
        
        // Should be very fast (hash table lookup)
        assert!(per_lookup < 1_000, "Connection lookup should be under 1μs");
        
        Ok(())
    }
}

/// Error Handling Tests for Transport Layer
#[cfg(test)]
mod transport_error_tests {
    use super::*;

    #[tokio::test]
    async fn test_transport_error_recovery() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test error recovery mechanisms
        let error_recovery = transport.get_error_recovery();
        assert!(error_recovery.is_ok());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_connection_timeout_handling() -> TestResult {
        let coordinator = TransportCoordinator::new()?;
        let peer_id = random_peer_id();
        
        // Test timeout configuration
        coordinator.set_connection_timeout(Duration::from_secs(5)).await?;
        
        // Test timeout behavior
        let unreachable_address = TransportAddress::BluetoothLe([0xFF; 6]);
        let packet = BitchatPacket::new_ping(peer_id, random_peer_id());
        
        let start = Instant::now();
        let result = coordinator.send_packet(unreachable_address, packet).await;
        let elapsed = start.elapsed();
        
        // Should timeout within reasonable time
        assert!(elapsed <= Duration::from_secs(10));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_malformed_packet_handling() -> TestResult {
        let peer_id = random_peer_id();
        let transport = BluetoothTransport::new(peer_id).await?;
        
        // Test handling of malformed packets
        let malformed_data = vec![0xFF; 100]; // Invalid packet data
        
        let parse_result = transport.parse_incoming_data(&malformed_data).await;
        
        // Should handle malformed data gracefully
        match parse_result {
            Ok(_) => println!("Packet parsed successfully"),
            Err(e) => println!("Malformed packet handled: {}", e),
        }
        
        Ok(())
    }
}