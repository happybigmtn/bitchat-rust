//! Comprehensive security tests for BitCraps transport layer
//!
//! This test suite validates:
//! - Transport-layer encryption and authentication
//! - Connection prioritization and management
//! - Bounded queue overflow protection
//! - GATT server security
//! - Replay attack prevention
//! - Key rotation mechanisms

use bitcraps::error::Error;
use bitcraps::protocol::PeerId;
use bitcraps::transport::{
    bounded_queue::{BoundedEventQueue, BoundedQueueError, OverflowBehavior, QueueConfig},
    crypto::{ConnectionPriority, SecureKeyExchange, TransportCrypto},
    secure_gatt_server::{GattClient, SecureGattServer},
    TransportEvent,
};
use std::time::Duration;
use tokio::time::sleep;

/// Helper function to create test peer IDs
fn create_test_peer_id(id: u8) -> PeerId {
    let mut peer_id = [0u8; 32];
    peer_id[0] = id;
    peer_id
}

/// Test basic ECDH key exchange functionality
#[tokio::test]
async fn test_ecdh_key_exchange() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Get public keys
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    // Perform key exchange
    assert!(crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .is_ok());
    assert!(crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .is_ok());

    // Test that both sides can encrypt and decrypt
    let message = b"Test message for ECDH";
    let encrypted1 = crypto1.encrypt_message(peer_id2, message).await.unwrap();
    let decrypted1 = crypto2
        .decrypt_message(peer_id1, &encrypted1)
        .await
        .unwrap();

    assert_eq!(message, &decrypted1[..]);

    let encrypted2 = crypto2.encrypt_message(peer_id1, message).await.unwrap();
    let decrypted2 = crypto1
        .decrypt_message(peer_id2, &encrypted2)
        .await
        .unwrap();

    assert_eq!(message, &decrypted2[..]);
}

/// Test encryption/decryption with various message sizes
#[tokio::test]
async fn test_encryption_message_sizes() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Set up keys
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Test various message sizes
    let test_sizes = vec![1, 16, 64, 247, 512, 1024, 4096];

    for size in test_sizes {
        let message = vec![42u8; size];
        let encrypted = crypto1.encrypt_message(peer_id2, &message).await.unwrap();
        let decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();

        assert_eq!(message, decrypted, "Failed for message size {}", size);

        // Verify ciphertext is different from plaintext
        assert_ne!(
            message,
            encrypted[18..].to_vec(),
            "Ciphertext equals plaintext for size {}",
            size
        );
    }
}

/// Test replay attack prevention
#[tokio::test]
async fn test_replay_attack_prevention() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Set up keys
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Encrypt a message
    let message = b"Secret message";
    let encrypted = crypto1.encrypt_message(peer_id2, message).await.unwrap();

    // First decryption should succeed
    let decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();
    assert_eq!(message, &decrypted[..]);

    // Replay attack should fail
    let replay_result = crypto2.decrypt_message(peer_id1, &encrypted).await;
    assert!(replay_result.is_err());

    match replay_result.unwrap_err() {
        Error::Crypto(msg) => assert!(msg.contains("Replay attack detected")),
        _ => panic!("Expected crypto error for replay attack"),
    }
}

/// Test message freshness validation
#[tokio::test]
async fn test_message_freshness() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Set up keys
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Test with current message (should work)
    let message = b"Fresh message";
    let encrypted = crypto1.encrypt_message(peer_id2, message).await.unwrap();
    let decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();
    assert_eq!(message, &decrypted[..]);

    // Note: Testing old messages would require manipulating timestamps,
    // which is not easily done with the current API design
}

/// Test connection prioritization
#[tokio::test]
async fn test_connection_prioritization() {
    let crypto = TransportCrypto::new();

    let peer1 = create_test_peer_id(1);
    let peer2 = create_test_peer_id(2);
    let peer3 = create_test_peer_id(3);

    // Mock key exchanges
    let public_key = crypto.public_key();
    crypto
        .perform_key_exchange(peer1, public_key)
        .await
        .unwrap();
    crypto
        .perform_key_exchange(peer2, public_key)
        .await
        .unwrap();
    crypto
        .perform_key_exchange(peer3, public_key)
        .await
        .unwrap();

    // Set different priorities
    crypto
        .set_connection_priority(peer1, ConnectionPriority::Low)
        .await
        .unwrap();
    crypto
        .set_connection_priority(peer2, ConnectionPriority::High)
        .await
        .unwrap();
    crypto
        .set_connection_priority(peer3, ConnectionPriority::Critical)
        .await
        .unwrap();

    // Update metrics to generate scores
    crypto
        .update_connection_metrics(peer1, 100, true)
        .await
        .unwrap();
    crypto
        .update_connection_metrics(peer2, 50, true)
        .await
        .unwrap();
    crypto
        .update_connection_metrics(peer3, 25, true)
        .await
        .unwrap();

    // Get prioritized list
    let priorities = crypto.get_peers_by_priority().await;

    assert_eq!(priorities.len(), 3);

    // Should be ordered: Critical (peer3), High (peer2), Low (peer1)
    assert_eq!(priorities[0].0, peer3);
    assert_eq!(priorities[1].0, peer2);
    assert_eq!(priorities[2].0, peer1);

    // Verify scores are in descending order
    assert!(priorities[0].1 >= priorities[1].1);
    assert!(priorities[1].1 >= priorities[2].1);
}

/// Test connection metrics updates
#[tokio::test]
async fn test_connection_metrics() {
    let crypto = TransportCrypto::new();
    let peer_id = create_test_peer_id(1);

    // Mock key exchange
    let public_key = crypto.public_key();
    crypto
        .perform_key_exchange(peer_id, public_key)
        .await
        .unwrap();

    // Test successful metrics
    crypto
        .update_connection_metrics(peer_id, 50, true)
        .await
        .unwrap();
    crypto
        .update_connection_metrics(peer_id, 60, true)
        .await
        .unwrap();

    // Test failed metrics
    crypto
        .update_connection_metrics(peer_id, 200, false)
        .await
        .unwrap();

    let stats = crypto.get_crypto_stats().await;
    assert_eq!(stats.active_sessions, 1);

    // Priority score should be affected by success/failure
    let priorities = crypto.get_peers_by_priority().await;
    assert!(!priorities.is_empty());
}

/// Test bounded queue basic functionality
#[tokio::test]
async fn test_bounded_queue_basic() {
    let queue = BoundedEventQueue::<i32>::new();
    let sender = queue.sender();
    let receiver = queue.receiver();

    // Test basic send/receive
    assert!(sender.send(42).await.is_ok());
    assert!(sender.send(43).await.is_ok());

    assert_eq!(receiver.recv().await, Some(42));
    assert_eq!(receiver.recv().await, Some(43));

    // Test statistics
    let stats = queue.stats().await;
    assert_eq!(stats.events_enqueued, 2);
    assert_eq!(stats.events_dequeued, 2);
}

/// Test bounded queue overflow behaviors
#[tokio::test]
async fn test_bounded_queue_overflow() {
    // Test drop oldest behavior
    let config = QueueConfig {
        max_size: 2,
        overflow_behavior: OverflowBehavior::DropOldest,
        ..Default::default()
    };

    let queue = BoundedEventQueue::<i32>::with_config(config);
    let sender = queue.sender();
    let receiver = queue.receiver();

    // Fill queue beyond capacity
    assert!(sender.send(1).await.is_ok());
    assert!(sender.send(2).await.is_ok());
    assert!(sender.send(3).await.is_ok()); // Should drop oldest (1)

    // Should receive 2 and 3, not 1
    assert_eq!(receiver.recv().await, Some(2));
    assert_eq!(receiver.recv().await, Some(3));

    // Test reject behavior
    let config = QueueConfig {
        max_size: 1,
        overflow_behavior: OverflowBehavior::Reject,
        ..Default::default()
    };

    let queue = BoundedEventQueue::<i32>::with_config(config);
    let sender = queue.sender();

    assert!(sender.send(1).await.is_ok());
    assert!(matches!(
        sender.send(2).await,
        Err(BoundedQueueError::QueueFull)
    ));
}

/// Test bounded queue backpressure
#[tokio::test]
async fn test_bounded_queue_backpressure() {
    let config = QueueConfig {
        max_size: 1,
        overflow_behavior: OverflowBehavior::Backpressure,
        backpressure_timeout: Duration::from_millis(50),
        ..Default::default()
    };

    let queue = BoundedEventQueue::<i32>::with_config(config);
    let sender = queue.sender();
    let receiver = queue.receiver();

    // Fill queue
    assert!(sender.send(1).await.is_ok());

    // Next send should timeout due to backpressure
    let result = sender.send(2).await;
    assert!(matches!(
        result,
        Err(BoundedQueueError::BackpressureTimeout)
    ));

    // After receiving, should be able to send again
    assert_eq!(receiver.recv().await, Some(1));
    assert!(sender.send(3).await.is_ok());
}

/// Test transport event queue with realistic load
#[tokio::test]
async fn test_transport_event_queue_load() {
    use bitcraps::transport::bounded_queue::BoundedTransportEventQueue;

    let queue = BoundedTransportEventQueue::for_high_throughput();
    let sender = queue.sender();
    let receiver = queue.receiver();

    let peer_id = create_test_peer_id(1);

    // Send many events rapidly
    let num_events = 1000;
    for i in 0..num_events {
        let event = TransportEvent::DataReceived {
            peer_id,
            data: vec![i as u8; 100],
        };
        assert!(sender.send(event).await.is_ok());
    }

    // Receive all events
    for i in 0..num_events {
        if let Some(TransportEvent::DataReceived { data, .. }) = receiver.recv().await {
            assert_eq!(data[0], i as u8);
        } else {
            panic!("Expected data received event at index {}", i);
        }
    }

    let stats = queue.stats().await;
    assert_eq!(stats.events_enqueued, num_events as u64);
    assert_eq!(stats.events_dequeued, num_events as u64);
    assert!(stats.avg_processing_latency_us > 0);
}

/// Test GATT server security
#[tokio::test]
async fn test_gatt_server_security() {
    let event_queue = BoundedTransportEventQueue::new();
    let event_sender = event_queue.sender();
    let crypto = std::sync::Arc::new(TransportCrypto::new());
    let local_peer_id = create_test_peer_id(99);

    let gatt_server = SecureGattServer::new(local_peer_id, event_sender, crypto.clone());

    // Test server start/stop
    assert!(gatt_server.start().await.is_ok());
    assert!(gatt_server.stop().await.is_ok());

    // Test client connection handling
    let peer_id = create_test_peer_id(1);
    let client_address = "00:11:22:33:44:55".to_string();

    assert!(gatt_server.start().await.is_ok());
    assert!(gatt_server
        .handle_client_connected(peer_id, client_address)
        .await
        .is_ok());

    // Test encrypted communication
    let test_data = b"Test GATT data";

    // This would normally involve the full GATT protocol
    // For now, we test that the server can handle the data
    assert!(gatt_server.send_to_client(peer_id, test_data).await.is_ok());

    // Test client disconnection
    assert!(gatt_server
        .disconnect_client(peer_id, "Test disconnect".to_string())
        .await
        .is_ok());

    assert!(gatt_server.stop().await.is_ok());
}

/// Test connection priority in GATT server
#[tokio::test]
async fn test_gatt_server_prioritization() {
    let event_queue = BoundedTransportEventQueue::new();
    let event_sender = event_queue.sender();
    let crypto = std::sync::Arc::new(TransportCrypto::new());
    let local_peer_id = create_test_peer_id(99);

    let gatt_server = SecureGattServer::new(local_peer_id, event_sender, crypto.clone());
    assert!(gatt_server.start().await.is_ok());

    let peer1 = create_test_peer_id(1);
    let peer2 = create_test_peer_id(2);
    let peer3 = create_test_peer_id(3);

    // Connect clients
    assert!(gatt_server
        .handle_client_connected(peer1, "addr1".to_string())
        .await
        .is_ok());
    assert!(gatt_server
        .handle_client_connected(peer2, "addr2".to_string())
        .await
        .is_ok());
    assert!(gatt_server
        .handle_client_connected(peer3, "addr3".to_string())
        .await
        .is_ok());

    // Set different priorities
    assert!(gatt_server
        .set_client_priority(peer1, ConnectionPriority::Low)
        .await
        .is_ok());
    assert!(gatt_server
        .set_client_priority(peer2, ConnectionPriority::High)
        .await
        .is_ok());
    assert!(gatt_server
        .set_client_priority(peer3, ConnectionPriority::Critical)
        .await
        .is_ok());

    // Get clients by priority
    let clients = gatt_server.get_clients_by_priority().await;

    assert_eq!(clients.len(), 3);
    // Should be ordered by priority: Critical, High, Low
    assert_eq!(clients[0], peer3);
    assert_eq!(clients[1], peer2);
    assert_eq!(clients[2], peer1);

    assert!(gatt_server.stop().await.is_ok());
}

/// Test key rotation mechanism
#[tokio::test]
async fn test_key_rotation() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Initial key exchange
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Test message encryption with initial keys
    let message1 = b"Message with initial keys";
    let encrypted1 = crypto1.encrypt_message(peer_id2, message1).await.unwrap();
    let decrypted1 = crypto2
        .decrypt_message(peer_id1, &encrypted1)
        .await
        .unwrap();
    assert_eq!(message1, &decrypted1[..]);

    // Note: Key rotation is automatic in the background
    // The current implementation rotates keys based on time intervals
    // Full testing would require time manipulation or manual rotation triggers

    // Test that communication continues to work
    let message2 = b"Message after potential rotation";
    let encrypted2 = crypto1.encrypt_message(peer_id2, message2).await.unwrap();
    let decrypted2 = crypto2
        .decrypt_message(peer_id1, &encrypted2)
        .await
        .unwrap();
    assert_eq!(message2, &decrypted2[..]);
}

/// Test secure key exchange implementation
#[tokio::test]
async fn test_secure_key_exchange() {
    // Test ephemeral key generation
    let (secret1, public1) = SecureKeyExchange::generate_keypair();
    let (secret2, public2) = SecureKeyExchange::generate_keypair();

    // Ensure public keys are different
    assert_ne!(public1.as_bytes(), public2.as_bytes());

    // Perform ECDH
    let shared1 = SecureKeyExchange::perform_ecdh(secret1, public2);
    let shared2 = SecureKeyExchange::perform_ecdh(secret2, public1);

    // Both sides should derive the same shared secret
    assert_eq!(shared1, shared2);

    // Test session key derivation
    let peer_id = create_test_peer_id(1);
    let (send_key1, recv_key1) = SecureKeyExchange::derive_session_keys(&shared1, peer_id).unwrap();
    let (send_key2, recv_key2) = SecureKeyExchange::derive_session_keys(&shared2, peer_id).unwrap();

    // Keys should be the same when derived from the same shared secret
    assert_eq!(send_key1.as_slice(), send_key2.as_slice());
    assert_eq!(recv_key1.as_slice(), recv_key2.as_slice());

    // But send and receive keys should be different
    assert_ne!(send_key1.as_slice(), recv_key1.as_slice());
}

/// Test error handling in crypto operations
#[tokio::test]
async fn test_crypto_error_handling() {
    let crypto = TransportCrypto::new();
    let peer_id = create_test_peer_id(1);

    // Try to encrypt without key exchange - should fail
    let result = crypto.encrypt_message(peer_id, b"test").await;
    assert!(result.is_err());

    // Try to decrypt without key exchange - should fail
    let fake_ciphertext = vec![0u8; 50];
    let result = crypto.decrypt_message(peer_id, &fake_ciphertext).await;
    assert!(result.is_err());

    // Try to decrypt invalid ciphertext after key exchange
    let public_key = crypto.public_key();
    crypto
        .perform_key_exchange(peer_id, public_key)
        .await
        .unwrap();

    let result = crypto.decrypt_message(peer_id, &fake_ciphertext).await;
    assert!(result.is_err());
}

/// Integration test for full transport security stack
#[tokio::test]
async fn test_transport_security_integration() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Set up encrypted channel
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Set priorities
    crypto1
        .set_connection_priority(peer_id2, ConnectionPriority::High)
        .await
        .unwrap();
    crypto2
        .set_connection_priority(peer_id1, ConnectionPriority::High)
        .await
        .unwrap();

    // Test bidirectional encrypted communication
    let messages = vec![
        b"Hello from peer 1",
        b"Response from peer 2",
        b"Large message with lots of data to test fragmentation and encryption overhead",
    ];

    for (i, &message) in messages.iter().enumerate() {
        // Peer 1 -> Peer 2
        let encrypted = crypto1.encrypt_message(peer_id2, message).await.unwrap();
        let decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();
        assert_eq!(message, &decrypted[..], "Failed for message {}", i);

        // Update metrics
        crypto1
            .update_connection_metrics(peer_id2, 10 + i as u32, true)
            .await
            .unwrap();
        crypto2
            .update_connection_metrics(peer_id1, 10 + i as u32, true)
            .await
            .unwrap();

        // Peer 2 -> Peer 1
        let encrypted = crypto2.encrypt_message(peer_id1, message).await.unwrap();
        let decrypted = crypto1.decrypt_message(peer_id2, &encrypted).await.unwrap();
        assert_eq!(message, &decrypted[..], "Failed for reverse message {}", i);
    }

    // Verify metrics were updated
    let stats1 = crypto1.get_crypto_stats().await;
    let stats2 = crypto2.get_crypto_stats().await;

    assert_eq!(stats1.active_sessions, 1);
    assert_eq!(stats2.active_sessions, 1);
    assert!(stats1.average_reliability > 0.0);
    assert!(stats2.average_reliability > 0.0);

    // Test priority-based connection management
    let priorities1 = crypto1.get_peers_by_priority().await;
    let priorities2 = crypto2.get_peers_by_priority().await;

    assert_eq!(priorities1.len(), 1);
    assert_eq!(priorities2.len(), 1);
    assert_eq!(priorities1[0].0, peer_id2);
    assert_eq!(priorities2[0].0, peer_id1);
}

/// Benchmark test for encryption performance
#[tokio::test]
async fn test_encryption_performance() {
    let crypto1 = TransportCrypto::new();
    let crypto2 = TransportCrypto::new();

    let peer_id1 = create_test_peer_id(1);
    let peer_id2 = create_test_peer_id(2);

    // Set up keys
    let public1 = crypto1.public_key();
    let public2 = crypto2.public_key();

    crypto1
        .perform_key_exchange(peer_id2, public2)
        .await
        .unwrap();
    crypto2
        .perform_key_exchange(peer_id1, public1)
        .await
        .unwrap();

    // Test performance with different message sizes
    let message_sizes = vec![64, 247, 512, 1024, 4096];

    for size in message_sizes {
        let message = vec![42u8; size];
        let start = std::time::Instant::now();

        // Encrypt/decrypt cycle
        let encrypted = crypto1.encrypt_message(peer_id2, &message).await.unwrap();
        let _decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();

        let duration = start.elapsed();
        println!("Size {}: {:?}", size, duration);

        // Reasonable performance expectation: under 1ms for small messages
        if size <= 1024 {
            assert!(
                duration < Duration::from_millis(10),
                "Encryption too slow for size {}: {:?}",
                size,
                duration
            );
        }
    }
}
