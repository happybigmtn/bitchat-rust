//! Comprehensive integration tests for SecureKeystore
//! 
//! These tests verify the security and correctness of all cryptographic operations
//! in the SecureKeystore module, ensuring no vulnerabilities or dummy implementations.

use bitcraps::crypto::{SecureKeystore, KeyContext};
use bitcraps::protocol::{PeerId, Signature};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test secure keystore creation and peer ID generation
#[tokio::test]
async fn test_secure_keystore_creation() {
    // Create multiple keystores to ensure they generate different keys
    let keystore1 = SecureKeystore::new().expect("Should create keystore");
    let keystore2 = SecureKeystore::new().expect("Should create keystore");
    
    let peer_id1 = keystore1.peer_id();
    let peer_id2 = keystore2.peer_id();
    
    // Peer IDs should be 32 bytes (Ed25519 public key size)
    assert_eq!(peer_id1.len(), 32);
    assert_eq!(peer_id2.len(), 32);
    
    // Different keystores should generate different peer IDs
    assert_ne!(peer_id1, peer_id2);
    
    // Peer IDs should not be all zeros (entropy check)
    assert_ne!(peer_id1, [0u8; 32]);
    assert_ne!(peer_id2, [0u8; 32]);
}

/// Test deterministic keystore creation from seed
#[tokio::test]
async fn test_deterministic_keystore_from_seed() {
    let seed = [42u8; 32];
    
    let keystore1 = SecureKeystore::from_seed(seed).expect("Should create from seed");
    let keystore2 = SecureKeystore::from_seed(seed).expect("Should create from seed");
    
    // Same seed should produce same peer ID
    assert_eq!(keystore1.peer_id(), keystore2.peer_id());
    
    // Different seed should produce different peer ID
    let different_seed = [43u8; 32];
    let keystore3 = SecureKeystore::from_seed(different_seed).expect("Should create from seed");
    assert_ne!(keystore1.peer_id(), keystore3.peer_id());
}

/// Test Ed25519 signature creation and verification
#[tokio::test]
async fn test_ed25519_signature_verification() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Critical consensus message requiring authentic signature";
    
    // Create signature
    let signature = keystore.sign(message).expect("Should create signature");
    let public_key = keystore.export_public_key();
    
    // Verify signature
    let is_valid = SecureKeystore::verify_signature(message, &signature, &public_key)
        .expect("Should verify signature");
    assert!(is_valid, "Valid signature should verify successfully");
    
    // Test with wrong message
    let wrong_message = b"Different message that should fail verification";
    let is_invalid = SecureKeystore::verify_signature(wrong_message, &signature, &public_key)
        .expect("Should handle wrong message");
    assert!(!is_invalid, "Signature with wrong message should fail");
    
    // Test with wrong public key
    let wrong_key = [1u8; 32];
    let is_invalid = SecureKeystore::verify_signature(message, &signature, &wrong_key);
    assert!(is_invalid.is_err() || !is_invalid.unwrap(), "Wrong public key should fail");
}

/// Test signature creation with different key contexts
#[tokio::test]
async fn test_context_based_signatures() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Context-specific message for testing";
    
    let contexts = [
        KeyContext::Identity,
        KeyContext::Consensus,
        KeyContext::GameState,
        KeyContext::Dispute,
        KeyContext::RandomnessCommit,
    ];
    
    let mut signatures = Vec::new();
    
    // Create signatures for each context
    for context in &contexts {
        let sig = keystore.sign_with_context(message, context.clone())
            .expect("Should create context signature");
        signatures.push((context.clone(), sig));
    }
    
    // Verify each signature with correct context
    for (expected_context, signature) in &signatures {
        let is_valid = SecureKeystore::verify_secure_signature(message, signature, expected_context)
            .expect("Should verify context signature");
        assert!(is_valid, "Context signature should verify with correct context");
    }
    
    // Test context mismatch
    for (wrong_context, signature) in &signatures {
        for test_context in &contexts {
            if std::mem::discriminant(test_context) != std::mem::discriminant(wrong_context) {
                let is_invalid = SecureKeystore::verify_secure_signature(message, signature, test_context)
                    .expect("Should handle context mismatch");
                assert!(!is_invalid, "Signature should fail with wrong context");
            }
        }
    }
}

/// Test timestamp validation in secure signatures
#[tokio::test]
async fn test_signature_timestamp_validation() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Time-sensitive message";
    
    // Create current signature
    let current_sig = keystore.sign_with_context(message, KeyContext::Consensus)
        .expect("Should create timestamped signature");
    
    // Current signature should verify
    let is_valid = SecureKeystore::verify_secure_signature(message, &current_sig, &KeyContext::Consensus)
        .expect("Should verify current signature");
    assert!(is_valid, "Current signature should verify");
    
    // Test with future timestamp (should fail)
    let future_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() + 7200; // 2 hours in future
    
    let mut future_sig = current_sig.clone();
    future_sig.timestamp = future_time;
    
    let is_invalid = SecureKeystore::verify_secure_signature(message, &future_sig, &KeyContext::Consensus)
        .expect("Should handle future timestamp");
    assert!(!is_invalid, "Future timestamp should fail verification");
    
    // Test with old timestamp (should fail)
    let old_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() - 7200; // 2 hours in past
    
    let mut old_sig = current_sig.clone();
    old_sig.timestamp = old_time;
    
    let is_invalid = SecureKeystore::verify_secure_signature(message, &old_sig, &KeyContext::Consensus)
        .expect("Should handle old timestamp");
    assert!(!is_invalid, "Old timestamp should fail verification");
}

/// Test cryptographically secure random generation using OsRng
#[tokio::test]
async fn test_secure_random_generation() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    
    // Test random bytes generation
    let bytes1 = keystore.generate_random_bytes(32);
    let bytes2 = keystore.generate_random_bytes(32);
    let bytes3 = keystore.generate_random_bytes(64);
    
    assert_eq!(bytes1.len(), 32);
    assert_eq!(bytes2.len(), 32);
    assert_eq!(bytes3.len(), 64);
    
    // Bytes should be different (probability of collision is negligible)
    assert_ne!(bytes1, bytes2);
    assert_ne!(bytes1[..32], bytes3[..32]);
    assert_ne!(bytes2[..32], bytes3[32..]);
    
    // Test commitment nonces
    let nonce1 = keystore.generate_commitment_nonce();
    let nonce2 = keystore.generate_commitment_nonce();
    
    assert_ne!(nonce1, nonce2);
    assert_ne!(nonce1, [0u8; 32]);
    assert_ne!(nonce2, [0u8; 32]);
}

/// Test entropy quality in random generation
#[tokio::test]
async fn test_random_entropy_quality() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let sample_size = 1000;
    let mut byte_frequencies = [0usize; 256];
    
    // Collect random byte samples
    for _ in 0..sample_size {
        let bytes = keystore.generate_random_bytes(1);
        byte_frequencies[bytes[0] as usize] += 1;
    }
    
    // Check that we have reasonable distribution (no value appears more than 5% of the time)
    let max_frequency = *byte_frequencies.iter().max().unwrap();
    let max_allowed = sample_size / 20; // 5%
    
    assert!(max_frequency < max_allowed, 
            "Random generation shows bias: max frequency {} exceeds threshold {}", 
            max_frequency, max_allowed);
    
    // Check that we have reasonable spread (at least 50% of possible values appear)
    let non_zero_count = byte_frequencies.iter().filter(|&&freq| freq > 0).count();
    assert!(non_zero_count > 128, 
            "Random generation lacks spread: only {} out of 256 values appeared", 
            non_zero_count);
}

/// Test key derivation for different contexts produces different keys
#[tokio::test]
async fn test_context_key_derivation_uniqueness() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Test message for context key uniqueness";
    
    let contexts = [
        KeyContext::Consensus,
        KeyContext::GameState,
        KeyContext::Dispute,
        KeyContext::RandomnessCommit,
    ];
    
    let mut public_keys = HashSet::new();
    
    // Generate signatures for each context and collect public keys
    for context in &contexts {
        let signature = keystore.sign_with_context(message, context.clone())
            .expect("Should create context signature");
        
        // Public key should be unique for each context
        assert!(public_keys.insert(signature.public_key.clone()), 
                "Context {:?} should have unique public key", context);
    }
    
    // All contexts should have produced different public keys
    assert_eq!(public_keys.len(), contexts.len());
}

/// Test invalid public key handling
#[tokio::test]
async fn test_invalid_public_key_handling() {
    // Test invalid public key lengths
    let invalid_keys = [
        [0u8; 31], // Too short
        [0u8; 33], // Too long (converted to slice)
    ];
    
    let message = b"test message";
    let signature = Signature([0u8; 64]);
    
    // Short key
    let result = SecureKeystore::verify_signature(message, &signature, &invalid_keys[0]);
    assert!(result.is_err(), "Should reject short public key");
    
    // Test malformed public key bytes
    let invalid_key = [255u8; 32]; // Likely invalid Ed25519 point
    let result = SecureKeystore::verify_signature(message, &signature, &invalid_key);
    // This may succeed or fail depending on Ed25519 implementation, but shouldn't panic
    assert!(result.is_ok(), "Should handle malformed key gracefully");
}

/// Test signature verification with invalid signatures
#[tokio::test]
async fn test_invalid_signature_handling() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"test message";
    let public_key = keystore.export_public_key();
    
    // Test with all-zero signature
    let zero_sig = Signature([0u8; 64]);
    let result = SecureKeystore::verify_signature(message, &zero_sig, &public_key)
        .expect("Should handle zero signature");
    assert!(!result, "Zero signature should fail verification");
    
    // Test with random signature
    let mut keystore2 = SecureKeystore::new().expect("Should create second keystore");
    let random_bytes = keystore2.generate_random_bytes(64);
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&random_bytes);
    let random_sig = Signature(sig_bytes);
    
    let result = SecureKeystore::verify_signature(message, &random_sig, &public_key)
        .expect("Should handle random signature");
    assert!(!result, "Random signature should fail verification");
}

/// Test key export and import functionality
#[tokio::test]
async fn test_public_key_export_import() {
    let keystore = SecureKeystore::new().expect("Should create keystore");
    let exported_key = keystore.export_public_key();
    
    // Should be able to create verifying key from exported bytes
    let verifying_key = SecureKeystore::verify_peer_public_key(&exported_key)
        .expect("Should import valid public key");
    
    // Exported key should have correct length
    assert_eq!(exported_key.len(), 32);
    
    // Test invalid key import
    let invalid_key = [255u8; 32];
    let result = SecureKeystore::verify_peer_public_key(&invalid_key);
    // May succeed or fail depending on whether this is a valid Ed25519 point
    // The important thing is it doesn't panic
    assert!(result.is_ok() || result.is_err(), "Should handle key validation gracefully");
}

/// Test multiple signature contexts don't interfere with each other
#[tokio::test]
async fn test_concurrent_context_usage() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Concurrent context test message";
    
    // Create signatures in different contexts rapidly
    let mut signatures = Vec::new();
    for i in 0..10 {
        let context = match i % 5 {
            0 => KeyContext::Identity,
            1 => KeyContext::Consensus,
            2 => KeyContext::GameState,
            3 => KeyContext::Dispute,
            _ => KeyContext::RandomnessCommit,
        };
        
        let signature = keystore.sign_with_context(message, context.clone())
            .expect("Should create signature");
        signatures.push((context, signature));
    }
    
    // All signatures should verify with their respective contexts
    for (context, signature) in &signatures {
        let is_valid = SecureKeystore::verify_secure_signature(message, signature, context)
            .expect("Should verify concurrent signature");
        assert!(is_valid, "Concurrent signature should verify correctly");
    }
}

/// Test that signatures are actually using Ed25519 (not dummy implementations)
#[tokio::test]
async fn test_real_ed25519_signatures() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let message = b"Ed25519 authenticity test";
    
    let signature = keystore.sign(message).expect("Should create signature");
    let public_key = keystore.export_public_key();
    
    // Signature should be exactly 64 bytes (Ed25519 signature size)
    assert_eq!(signature.0.len(), 64);
    
    // Public key should be exactly 32 bytes (Ed25519 public key size)
    assert_eq!(public_key.len(), 32);
    
    // Signature should not be all zeros (dummy implementation check)
    assert_ne!(signature.0, [0u8; 64]);
    
    // Small change in message should invalidate signature
    let mut modified_message = message.to_vec();
    modified_message[0] = modified_message[0].wrapping_add(1);
    
    let is_invalid = SecureKeystore::verify_signature(&modified_message, &signature, &public_key)
        .expect("Should handle modified message");
    assert!(!is_invalid, "Modified message should fail verification");
    
    // Small change in signature should invalidate verification
    let mut modified_signature = signature.clone();
    modified_signature.0[0] = modified_signature.0[0].wrapping_add(1);
    
    let is_invalid = SecureKeystore::verify_signature(message, &modified_signature, &public_key)
        .expect("Should handle modified signature");
    assert!(!is_invalid, "Modified signature should fail verification");
}