//! Tests for connection limits functionality

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_total_connection_limits() {
        let limits = ConnectionLimits {
            max_total_connections: 2,
            max_connections_per_peer: 1,
            max_new_connections_per_minute: 10,
            connection_cooldown: Duration::from_secs(1),
        };

        let coordinator = TransportCoordinator::new_with_limits(limits);
        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];
        let peer3 = [3u8; 32];

        let addr1 = TransportAddress::Bluetooth("device1".to_string());
        let addr2 = TransportAddress::Bluetooth("device2".to_string());
        let addr3 = TransportAddress::Bluetooth("device3".to_string());

        // Mock successful connections by directly updating internal state
        let metadata1 = ConnectionMetadata {
            address: addr1.clone(),
            established_at: std::time::Instant::now(),
        };
        coordinator.connections.insert(peer1, metadata1);
        coordinator.increment_connection_count(&addr1).await;

        let metadata2 = ConnectionMetadata {
            address: addr2.clone(),
            established_at: std::time::Instant::now(),
        };
        coordinator
            .connections.insert(peer2, metadata2);
        coordinator.increment_connection_count(&addr2).await;

        // Now the third connection should be rejected due to total limit
        let result = coordinator.check_connection_limits(&addr3).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Maximum total connections"));
    }

    #[tokio::test]
    async fn test_per_peer_connection_limits() {
        let limits = ConnectionLimits {
            max_total_connections: 10,
            max_connections_per_peer: 1,
            max_new_connections_per_minute: 10,
            connection_cooldown: Duration::from_secs(1),
        };

        let coordinator = TransportCoordinator::new_with_limits(limits);
        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];

        let addr = TransportAddress::Bluetooth("device1".to_string());

        // Mock first connection to this address
        let metadata1 = ConnectionMetadata {
            address: addr.clone(),
            established_at: std::time::Instant::now(),
        };
        coordinator
            .connections.insert(peer1, metadata1);
        coordinator.increment_connection_count(&addr).await;

        // Second connection to the same address should be rejected
        let result = coordinator.check_connection_limits(&addr).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Maximum connections per peer"));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let limits = ConnectionLimits {
            max_total_connections: 10,
            max_connections_per_peer: 5,
            max_new_connections_per_minute: 2,
            connection_cooldown: Duration::from_secs(1),
        };

        let coordinator = TransportCoordinator::new_with_limits(limits);
        let addr1 = TransportAddress::Bluetooth("device1".to_string());
        let addr2 = TransportAddress::Bluetooth("device2".to_string());
        let addr3 = TransportAddress::Bluetooth("device3".to_string());

        // First two connection attempts should succeed (in terms of rate limiting)
        assert!(coordinator.check_connection_limits(&addr1).await.is_ok());
        coordinator.record_connection_attempt(&addr1).await;

        assert!(coordinator.check_connection_limits(&addr2).await.is_ok());
        coordinator.record_connection_attempt(&addr2).await;

        // Third connection attempt should be rate limited
        let result = coordinator.check_connection_limits(&addr3).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Rate limit exceeded"));
    }

    #[tokio::test]
    async fn test_connection_cooldown() {
        let limits = ConnectionLimits {
            max_total_connections: 10,
            max_connections_per_peer: 5,
            max_new_connections_per_minute: 10,
            connection_cooldown: Duration::from_secs(2),
        };

        let coordinator = TransportCoordinator::new_with_limits(limits);
        let addr = TransportAddress::Bluetooth("device1".to_string());

        // First connection attempt should succeed
        assert!(coordinator.check_connection_limits(&addr).await.is_ok());
        coordinator.record_connection_attempt(&addr).await;

        // Immediate second connection attempt to same address should be in cooldown
        let result = coordinator.check_connection_limits(&addr).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cooldown period active"));
    }

    #[tokio::test]
    async fn test_bluetooth_connection_limits() {
        let bt_limits = BluetoothConnectionLimits {
            max_concurrent_connections: 2,
            max_connection_attempts_per_minute: 3,
            connection_timeout: Duration::from_secs(30),
        };

        // Create a dummy peer ID for testing
        let peer_id = [1u8; 32];

        let transport = BluetoothTransport::new_with_limits(peer_id, bt_limits)
            .await
            .expect("Failed to create Bluetooth transport");

        // Test connection limit checking
        assert!(transport.check_bluetooth_connection_limits().await.is_ok());

        // Record some attempts
        transport.record_bluetooth_connection_attempt().await;
        transport.record_bluetooth_connection_attempt().await;
        transport.record_bluetooth_connection_attempt().await;

        // Should now be rate limited
        let result = transport.check_bluetooth_connection_limits().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Rate limit exceeded"));
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let limits = ConnectionLimits {
            max_total_connections: 5,
            max_connections_per_peer: 2,
            max_new_connections_per_minute: 10,
            connection_cooldown: Duration::from_secs(1),
        };

        let coordinator = TransportCoordinator::new_with_limits(limits);
        let peer1 = [1u8; 32];
        let peer2 = [2u8; 32];

        let addr1 = TransportAddress::Bluetooth("device1".to_string());
        let addr2 = TransportAddress::Bluetooth("device2".to_string());

        // Mock connections
        let metadata1 = ConnectionMetadata {
            address: addr1.clone(),
            established_at: std::time::Instant::now(),
        };
        coordinator
            .connections.insert(peer1, metadata1);
        coordinator.increment_connection_count(&addr1).await;

        let metadata2 = ConnectionMetadata {
            address: addr2.clone(),
            established_at: std::time::Instant::now(),
        };
        coordinator
            .connections.insert(peer2, metadata2);
        coordinator.increment_connection_count(&addr2).await;

        // Record some attempts
        coordinator.record_connection_attempt(&addr1).await;
        coordinator.record_connection_attempt(&addr2).await;

        let stats = coordinator.connection_stats().await;

        assert_eq!(stats.total_connections, 2);
        assert_eq!(stats.connection_limit, 5);
        assert_eq!(stats.recent_connection_attempts, 2);
        assert_eq!(stats.connections_by_address.get(&addr1), Some(&1));
        assert_eq!(stats.connections_by_address.get(&addr2), Some(&1));
    }
}
