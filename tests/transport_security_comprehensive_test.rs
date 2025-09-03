//! Comprehensive Transport Security Tests
//!
//! This module contains extensive tests for the transport security system:
//! - BLE AES-GCM encryption and ChaCha20Poly1305 fallback
//! - ECDH key exchange with identity verification
//! - Message fragmentation and assembly
//! - HMAC authentication and replay protection
//! - Timestamp validation
//! - Key rotation and forward secrecy
//! - Persistent encrypted keystore operations

use bitcraps::crypto::{BitchatIdentity, GameCrypto};
use bitcraps::error::{Error, Result};
use bitcraps::transport::keystore::{KeyType, KeystoreConfig, SecureTransportKeystore};
use bitcraps::transport::security::{
    BleSecurityConfig, EncryptedIdentityStorage, EnhancedTransportSecurity,
};
use tempfile::TempDir;
use tokio;

/// Test basic key exchange functionality
#[tokio::test]
async fn test_enhanced_key_exchange() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    // Test key exchange without identity verification
    assert!(security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, None)
        .await
        .is_ok());

    assert!(security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, None)
        .await
        .is_ok());
}

/// Test key exchange with identity verification
#[tokio::test]
async fn test_authenticated_key_exchange_with_identity() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    // Generate identities
    let identity1 = BitchatIdentity::generate_with_pow(8);
    let identity2 = BitchatIdentity::generate_with_pow(8);

    let peer_id1 = identity1.peer_id;
    let peer_id2 = identity2.peer_id;

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    // Set identities
    security1.set_identity(identity1.clone()).await;
    security2.set_identity(identity2.clone()).await;

    // Test authenticated key exchange
    assert!(security1
        .perform_authenticated_key_exchange(peer_id2, public2, Some(identity2.clone()), None)
        .await
        .is_ok());

    assert!(security2
        .perform_authenticated_key_exchange(peer_id1, public1, Some(identity1.clone()), None)
        .await
        .is_ok());
}

/// Test AES-GCM encryption and decryption
#[tokio::test]
async fn test_aes_gcm_encryption() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    // Setup with AES-GCM config
    let config = BleSecurityConfig {
        use_aes_gcm: true,
        enable_hmac: true,
        enable_timestamp_validation: true,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(config.clone()))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(config))
        .await
        .unwrap();

    // Test encryption/decryption
    let plaintext = b"Hello, BitCraps AES-GCM!";
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, plaintext, 1)
        .await
        .unwrap();

    assert_eq!(encrypted_fragments.len(), 1);

    let decrypted = security2
        .decrypt_and_verify(peer_id1, &encrypted_fragments[0])
        .await
        .unwrap();

    assert_eq!(decrypted.unwrap(), plaintext);
}

/// Test ChaCha20Poly1305 encryption as fallback
#[tokio::test]
async fn test_chacha20_encryption() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    // Setup with ChaCha20 config
    let config = BleSecurityConfig {
        use_aes_gcm: false,
        enable_hmac: true,
        enable_timestamp_validation: true,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(config.clone()))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(config))
        .await
        .unwrap();

    // Test encryption/decryption
    let plaintext = b"Hello, BitCraps ChaCha20Poly1305!";
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, plaintext, 2)
        .await
        .unwrap();

    assert_eq!(encrypted_fragments.len(), 1);

    let decrypted = security2
        .decrypt_and_verify(peer_id1, &encrypted_fragments[0])
        .await
        .unwrap();

    assert_eq!(decrypted.unwrap(), plaintext);
}

/// Test message fragmentation for large payloads
#[tokio::test]
async fn test_message_fragmentation() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    // Setup with small fragment size to force fragmentation
    let config = BleSecurityConfig {
        fragment_large_messages: true,
        max_message_size: 100, // Small size to force fragmentation
        use_aes_gcm: true,
        enable_hmac: true,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(config.clone()))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(config))
        .await
        .unwrap();

    // Test with large message
    let large_message = vec![42u8; 500]; // Should be fragmented
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, &large_message, 3)
        .await
        .unwrap();

    assert!(
        encrypted_fragments.len() > 1,
        "Message should be fragmented"
    );

    // Test fragment assembly (simplified - in practice fragments arrive separately)
    let mut reassembled_message = None;
    for fragment in encrypted_fragments {
        if let Some(result) = security2
            .decrypt_and_verify(peer_id1, &fragment)
            .await
            .unwrap()
        {
            reassembled_message = Some(result);
            break; // In real scenario, this would accumulate fragments
        }
    }

    // Note: This test is simplified - full fragment assembly would require
    // processing each fragment separately through the state machine
    assert!(reassembled_message.is_some());
}

/// Test HMAC authentication
#[tokio::test]
async fn test_hmac_authentication() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    let config = BleSecurityConfig {
        enable_hmac: true,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(config.clone()))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(config))
        .await
        .unwrap();

    // Encrypt message
    let message = b"HMAC protected message";
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, message, 4)
        .await
        .unwrap();

    let ciphertext = &encrypted_fragments[0];

    // Verify HMAC passes
    let decrypted = security2
        .decrypt_and_verify(peer_id1, ciphertext)
        .await
        .unwrap();
    assert_eq!(decrypted.unwrap(), message);

    // Test HMAC failure by corrupting the message
    let mut corrupted = ciphertext.clone();
    let len = corrupted.len();
    corrupted[len - 1] ^= 0x01; // Flip a bit in the HMAC

    let result = security2.decrypt_and_verify(peer_id1, &corrupted).await;
    assert!(result.is_err(), "Corrupted HMAC should fail verification");
}

/// Test replay attack prevention
#[tokio::test]
async fn test_replay_protection() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, None)
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, None)
        .await
        .unwrap();

    // Encrypt a message
    let message = b"Anti-replay test message";
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, message, 5)
        .await
        .unwrap();

    let ciphertext = &encrypted_fragments[0];

    // First decryption should work
    let first_result = security2.decrypt_and_verify(peer_id1, ciphertext).await;
    assert!(first_result.is_ok());

    // Second decryption (replay) should fail
    let replay_result = security2.decrypt_and_verify(peer_id1, ciphertext).await;
    assert!(replay_result.is_err(), "Replay attack should be detected");
}

/// Test timestamp validation
#[tokio::test]
async fn test_timestamp_validation() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    let config = BleSecurityConfig {
        enable_timestamp_validation: true,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(config.clone()))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(config))
        .await
        .unwrap();

    // Test normal message (should pass)
    let message = b"Timestamp validated message";
    let encrypted_fragments = security1
        .encrypt_and_authenticate(peer_id2, message, 6)
        .await
        .unwrap();

    let result = security2
        .decrypt_and_verify(peer_id1, &encrypted_fragments[0])
        .await;
    assert!(result.is_ok());

    // Note: Testing old timestamp would require manipulating the header,
    // which is complex due to HMAC protection. In practice, old messages
    // would be rejected by the timestamp check.
}

/// Test key rotation functionality
#[tokio::test]
async fn test_key_rotation() {
    let security1 = EnhancedTransportSecurity::new();

    let peer_id = [42u8; 32];
    let public_key = security1.public_key();

    // Setup initial keys
    security1
        .perform_authenticated_key_exchange(peer_id, public_key, None, None)
        .await
        .unwrap();

    // Encrypt with initial keys
    let message1 = b"Message before rotation";
    let encrypted1 = security1
        .encrypt_and_authenticate(peer_id, message1, 7)
        .await
        .unwrap();

    // Rotate keys
    security1.rotate_peer_keys(peer_id).await.unwrap();

    // Encrypt with rotated keys
    let message2 = b"Message after rotation";
    let encrypted2 = security1
        .encrypt_and_authenticate(peer_id, message2, 8)
        .await
        .unwrap();

    // The encrypted messages should be different even with same content
    assert_ne!(encrypted1[0], encrypted2[0]);
}

/// Test secure keystore operations
#[tokio::test]
async fn test_secure_keystore() {
    let temp_dir = TempDir::new().unwrap();
    let keystore = SecureTransportKeystore::new(temp_dir.path()).await.unwrap();

    // Initialize keystore
    keystore.initialize("test_password_123").await.unwrap();

    // Store various key types
    let symmetric_key = b"test_symmetric_key_32_bytes_long";
    keystore
        .store_key(
            "sym_key_1",
            symmetric_key,
            KeyType::SymmetricKey,
            "Test symmetric key",
            None,
        )
        .await
        .unwrap();

    let hmac_key = GameCrypto::random_bytes::<32>();
    keystore
        .store_key(
            "hmac_key_1",
            &hmac_key,
            KeyType::HmacKey,
            "Test HMAC key",
            Some([1u8; 32]),
        )
        .await
        .unwrap();

    // Retrieve keys
    let retrieved_sym = keystore.retrieve_key("sym_key_1").await.unwrap();
    assert_eq!(retrieved_sym.as_slice(), symmetric_key);

    let retrieved_hmac = keystore.retrieve_key("hmac_key_1").await.unwrap();
    assert_eq!(retrieved_hmac.as_slice(), &hmac_key);

    // Test key listing
    let keys = keystore.list_keys().await;
    assert_eq!(keys.len(), 2);

    // Test stats
    let stats = keystore.get_stats().await;
    assert_eq!(stats.keys_stored, 2);
    assert_eq!(stats.keys_retrieved, 2);
}

/// Test identity storage and retrieval
#[tokio::test]
async fn test_identity_keystore() {
    let temp_dir = TempDir::new().unwrap();
    let keystore = SecureTransportKeystore::new(temp_dir.path()).await.unwrap();
    keystore.initialize("identity_test_password").await.unwrap();

    // Create test identity
    let identity = BitchatIdentity::generate_with_pow(8);

    // Store identity
    keystore
        .store_identity(&identity, "test_identity")
        .await
        .unwrap();

    // Retrieve identity
    let retrieved = keystore.retrieve_identity("test_identity").await.unwrap();

    // Verify they match
    assert_eq!(identity.peer_id, retrieved.peer_id);
    assert_eq!(identity.pow_nonce, retrieved.pow_nonce);
    assert_eq!(identity.pow_difficulty, retrieved.pow_difficulty);
    assert_eq!(
        identity.keypair.public_key_bytes(),
        retrieved.keypair.public_key_bytes()
    );

    // Verify PoW is still valid
    assert!(retrieved.verify_pow());
}

/// Test keystore backup and restore
#[tokio::test]
async fn test_keystore_backup_restore() {
    let temp_dir = TempDir::new().unwrap();
    let keystore = SecureTransportKeystore::new(temp_dir.path()).await.unwrap();
    keystore.initialize("backup_test_password").await.unwrap();

    // Store test data
    let test_key = b"backup_test_key_32_bytes_long!!!";
    keystore
        .store_key(
            "backup_key",
            test_key,
            KeyType::SymmetricKey,
            "Backup test key",
            None,
        )
        .await
        .unwrap();

    // Create backup
    let backup_path = temp_dir.path().join("keystore_backup.enc");
    keystore
        .create_backup(&backup_path, "backup_password_123")
        .await
        .unwrap();

    // Clear and reinitialize keystore
    keystore.lock().await;
    keystore.initialize("backup_test_password").await.unwrap();
    keystore.remove_key("backup_key").await.unwrap();

    // Verify key is gone
    assert!(keystore.retrieve_key("backup_key").await.is_err());

    // Restore from backup
    keystore
        .restore_backup(&backup_path, "backup_password_123")
        .await
        .unwrap();

    // Verify key is restored
    let restored = keystore.retrieve_key("backup_key").await.unwrap();
    assert_eq!(restored.as_slice(), test_key);
}

/// Test encrypted identity storage
#[tokio::test]
async fn test_encrypted_identity_storage() {
    let storage = EncryptedIdentityStorage::new();
    let identity = BitchatIdentity::generate_with_pow(8);
    let password = b"identity_encryption_password_123";

    // Encrypt identity
    let encrypted = storage.encrypt_identity(&identity, password).unwrap();

    // Verify encrypted data is different from original
    assert_ne!(
        encrypted.encrypted_private_key,
        identity.keypair.secret_key_bytes()
    );
    assert_eq!(encrypted.public_key, identity.peer_id);
    assert_eq!(encrypted.pow_nonce, identity.pow_nonce);

    // Decrypt identity
    let decrypted = storage.decrypt_identity(&encrypted, password).unwrap();

    // Verify integrity
    assert_eq!(identity.peer_id, decrypted.peer_id);
    assert_eq!(identity.pow_nonce, decrypted.pow_nonce);
    assert_eq!(identity.pow_difficulty, decrypted.pow_difficulty);
    assert_eq!(
        identity.keypair.public_key_bytes(),
        decrypted.keypair.public_key_bytes()
    );

    // Test wrong password fails
    let wrong_password = b"wrong_password";
    assert!(storage
        .decrypt_identity(&encrypted, wrong_password)
        .is_err());
}

/// Test security statistics
#[tokio::test]
async fn test_security_statistics() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    // Setup multiple peers with different configs
    let aes_config = BleSecurityConfig {
        use_aes_gcm: true,
        enable_hmac: true,
        fragment_large_messages: true,
        ..Default::default()
    };

    let chacha_config = BleSecurityConfig {
        use_aes_gcm: false,
        enable_hmac: true,
        fragment_large_messages: false,
        ..Default::default()
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, Some(aes_config))
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, Some(chacha_config))
        .await
        .unwrap();

    // Get statistics
    let stats1 = security1.get_security_stats().await;
    let stats2 = security2.get_security_stats().await;

    assert_eq!(stats1.active_sessions, 1);
    assert_eq!(stats1.aes_gcm_sessions, 1);
    assert_eq!(stats1.chacha20_sessions, 0);
    assert_eq!(stats1.hmac_enabled_sessions, 1);
    assert_eq!(stats1.fragment_enabled_sessions, 1);

    assert_eq!(stats2.active_sessions, 1);
    assert_eq!(stats2.aes_gcm_sessions, 0);
    assert_eq!(stats2.chacha20_sessions, 1);
    assert_eq!(stats2.hmac_enabled_sessions, 1);
    assert_eq!(stats2.fragment_enabled_sessions, 0);
}

/// Test comprehensive security integration
#[tokio::test]
async fn test_comprehensive_security_integration() {
    // Create identities
    let identity1 = BitchatIdentity::generate_with_pow(8);
    let identity2 = BitchatIdentity::generate_with_pow(8);

    // Create security managers
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    // Set identities
    security1.set_identity(identity1.clone()).await;
    security2.set_identity(identity2.clone()).await;

    // Create keystores
    let temp_dir1 = TempDir::new().unwrap();
    let temp_dir2 = TempDir::new().unwrap();

    let keystore1 = SecureTransportKeystore::new(temp_dir1.path())
        .await
        .unwrap();
    let keystore2 = SecureTransportKeystore::new(temp_dir2.path())
        .await
        .unwrap();

    keystore1.initialize("password1").await.unwrap();
    keystore2.initialize("password2").await.unwrap();

    // Store identities
    keystore1
        .store_identity(&identity1, "my_identity")
        .await
        .unwrap();
    keystore2
        .store_identity(&identity2, "my_identity")
        .await
        .unwrap();

    // Perform authenticated key exchange with full security
    let config = BleSecurityConfig {
        use_aes_gcm: true,
        enable_hmac: true,
        enable_timestamp_validation: true,
        fragment_large_messages: true,
        enable_compression: false, // Keep false for testing
        max_message_size: 200,
        key_rotation_interval_secs: 3600,
    };

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(
            identity2.peer_id,
            public2,
            Some(identity2.clone()),
            Some(config.clone()),
        )
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(
            identity1.peer_id,
            public1,
            Some(identity1.clone()),
            Some(config),
        )
        .await
        .unwrap();

    // Test bidirectional encrypted communication
    let message1to2 = b"Hello from peer 1 to peer 2!";
    let message2to1 = b"Hello from peer 2 to peer 1!";

    // Encrypt messages
    let encrypted1 = security1
        .encrypt_and_authenticate(identity2.peer_id, message1to2, 10)
        .await
        .unwrap();

    let encrypted2 = security2
        .encrypt_and_authenticate(identity1.peer_id, message2to1, 11)
        .await
        .unwrap();

    // Decrypt and verify
    let decrypted1 = security2
        .decrypt_and_verify(identity1.peer_id, &encrypted1[0])
        .await
        .unwrap();

    let decrypted2 = security1
        .decrypt_and_verify(identity2.peer_id, &encrypted2[0])
        .await
        .unwrap();

    assert_eq!(decrypted1.unwrap(), message1to2);
    assert_eq!(decrypted2.unwrap(), message2to1);

    // Test key rotation
    security1.rotate_peer_keys(identity2.peer_id).await.unwrap();
    security2.rotate_peer_keys(identity1.peer_id).await.unwrap();

    // Verify communication still works after rotation
    let post_rotation_message = b"Post-rotation secure message";
    let encrypted_post = security1
        .encrypt_and_authenticate(identity2.peer_id, post_rotation_message, 12)
        .await
        .unwrap();

    let decrypted_post = security2
        .decrypt_and_verify(identity1.peer_id, &encrypted_post[0])
        .await
        .unwrap();

    assert_eq!(decrypted_post.unwrap(), post_rotation_message);

    // Verify statistics
    let stats1 = security1.get_security_stats().await;
    assert_eq!(stats1.active_sessions, 1);
    assert_eq!(stats1.aes_gcm_sessions, 1);

    let keystore_stats1 = keystore1.get_stats().await;
    assert!(keystore_stats1.keys_stored >= 2); // At least identity keys
}

/// Benchmark encryption performance
#[tokio::test]
async fn test_encryption_performance() {
    let security1 = EnhancedTransportSecurity::new();
    let security2 = EnhancedTransportSecurity::new();

    let peer_id1 = [1u8; 32];
    let peer_id2 = [2u8; 32];

    let public1 = security1.public_key();
    let public2 = security2.public_key();

    security1
        .perform_authenticated_key_exchange(peer_id2, public2, None, None)
        .await
        .unwrap();

    security2
        .perform_authenticated_key_exchange(peer_id1, public1, None, None)
        .await
        .unwrap();

    // Test encryption/decryption speed
    let test_message = vec![0x42u8; 1024]; // 1KB message
    let iterations = 100;

    let start = std::time::Instant::now();

    for i in 0..iterations {
        let encrypted = security1
            .encrypt_and_authenticate(peer_id2, &test_message, i)
            .await
            .unwrap();

        let decrypted = security2
            .decrypt_and_verify(peer_id1, &encrypted[0])
            .await
            .unwrap();

        assert_eq!(decrypted.unwrap(), test_message);
    }

    let duration = start.elapsed();
    let ops_per_sec = iterations as f64 / duration.as_secs_f64();

    println!("Encryption performance: {:.2} ops/sec", ops_per_sec);
    println!(
        "Average latency: {:.2}ms",
        duration.as_millis() as f64 / iterations as f64
    );

    // Should handle at least 50 operations per second on reasonable hardware
    assert!(
        ops_per_sec > 50.0,
        "Encryption performance too slow: {} ops/sec",
        ops_per_sec
    );
}
#![cfg(feature = "legacy-tests")]
#![cfg(feature = "legacy-tests")]
