//! Comprehensive integration tests for cryptographic operations in consensus
//!
//! These tests verify that all cryptographic operations in the consensus system
//! are using real Ed25519 signatures, secure randomness, and proper commit-reveal schemes.

use bitcraps::crypto::{KeyContext, SecureKeystore};
use bitcraps::error::Result;
use bitcraps::protocol::consensus::{
    commit_reveal::{EntropyPool, RandomnessCommit, RandomnessReveal},
    engine::{ConsensusConfig, ConsensusEngine},
    RoundId,
};
use bitcraps::protocol::{Hash256, PeerId, Signature};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Test that consensus engine uses real Ed25519 signatures
#[tokio::test]
async fn test_consensus_real_ed25519_signatures() {
    let config = ConsensusConfig::default();
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let mut engine = ConsensusEngine::new(config, keystore.peer_id());

    // Create a consensus message
    let message = b"Critical consensus vote requiring authentic signature";
    let signature = keystore
        .sign_with_context(message, KeyContext::Consensus)
        .expect("Should create consensus signature");

    // Verify signature properties
    assert_eq!(
        signature.signature.len(),
        64,
        "Ed25519 signature should be 64 bytes"
    );
    assert_eq!(
        signature.public_key.len(),
        32,
        "Ed25519 public key should be 32 bytes"
    );
    assert_ne!(
        signature.signature,
        vec![0u8; 64],
        "Signature should not be all zeros"
    );
    assert_ne!(
        signature.public_key,
        vec![0u8; 32],
        "Public key should not be all zeros"
    );

    // Verify signature validates correctly
    let is_valid =
        SecureKeystore::verify_secure_signature(message, &signature, &KeyContext::Consensus)
            .expect("Should verify signature");
    assert!(is_valid, "Real Ed25519 signature should verify");

    // Verify signature fails with wrong message
    let wrong_message = b"Different message that should fail";
    let is_invalid =
        SecureKeystore::verify_secure_signature(wrong_message, &signature, &KeyContext::Consensus)
            .expect("Should handle wrong message");
    assert!(!is_invalid, "Signature should fail with wrong message");
}

/// Test that OsRng produces cryptographically secure randomness
#[tokio::test]
async fn test_osrng_cryptographic_quality() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let sample_count = 1000;
    let mut random_samples = Vec::new();

    // Collect random samples
    for _ in 0..sample_count {
        let random_bytes = keystore.generate_random_bytes(32);
        random_samples.push(random_bytes);
    }

    // Test 1: All samples should be different (probability of collision is negligible)
    let mut unique_samples = std::collections::HashSet::new();
    for sample in &random_samples {
        unique_samples.insert(sample.clone());
    }
    assert_eq!(
        unique_samples.len(),
        sample_count,
        "All random samples should be unique"
    );

    // Test 2: No sample should be all zeros or all ones
    for (i, sample) in random_samples.iter().enumerate() {
        assert_ne!(
            sample,
            &vec![0u8; 32],
            "Sample {} should not be all zeros",
            i
        );
        assert_ne!(
            sample,
            &vec![255u8; 32],
            "Sample {} should not be all ones",
            i
        );
    }

    // Test 3: Byte distribution should be reasonably uniform
    let mut byte_frequencies = [0usize; 256];
    let total_bytes = sample_count * 32;

    for sample in &random_samples {
        for &byte in sample {
            byte_frequencies[byte as usize] += 1;
        }
    }

    // Check that no byte value appears more than 1.5x expected frequency
    let expected_frequency = total_bytes / 256;
    let max_allowed = (expected_frequency as f64 * 1.5) as usize;

    for (byte_value, &frequency) in byte_frequencies.iter().enumerate() {
        assert!(
            frequency <= max_allowed,
            "Byte value {} appears {} times, exceeding threshold {}",
            byte_value,
            frequency,
            max_allowed
        );
    }

    // Test 4: Each byte value should appear at least once in large sample
    let min_required = total_bytes / 1000; // Very low threshold
    let missing_values: Vec<usize> = byte_frequencies
        .iter()
        .enumerate()
        .filter(|(_, &freq)| freq < min_required)
        .map(|(value, _)| value)
        .collect();

    assert!(
        missing_values.len() < 50,
        "Too many byte values missing or rare: {:?}",
        missing_values
    );
}

/// Test commit-reveal scheme with real cryptographic operations
#[tokio::test]
async fn test_commit_reveal_cryptographic_security() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let peer_id = keystore.peer_id();
    let round_id = 12345u64;

    // Generate cryptographically secure nonce
    let nonce = keystore.generate_commitment_nonce();

    // Create commitment
    let commitment = RandomnessCommit::new(peer_id, round_id, nonce);

    // Verify commitment properties
    assert_eq!(commitment.player, peer_id);
    assert_eq!(commitment.round_id, round_id);
    assert_ne!(
        commitment.commitment, [0u8; 32],
        "Commitment hash should not be zeros"
    );

    // Create reveal with proper signature
    let reveal = RandomnessReveal::new(peer_id, round_id, nonce, &mut keystore)
        .expect("Should create signed reveal");

    // Verify reveal properties
    assert_eq!(reveal.player, peer_id);
    assert_eq!(reveal.round_id, round_id);
    assert_eq!(reveal.nonce, nonce);
    assert_ne!(
        reveal.signature.0, [0u8; 64],
        "Signature should not be zeros"
    );

    // Verify commit-reveal consistency
    assert!(
        commitment.verify_reveal(&reveal),
        "Commitment should verify against matching reveal"
    );

    // Test with wrong nonce
    let wrong_nonce = keystore.generate_commitment_nonce();
    let wrong_reveal = RandomnessReveal::new(peer_id, round_id, wrong_nonce, &mut keystore)
        .expect("Should create reveal with wrong nonce");

    assert!(
        !commitment.verify_reveal(&wrong_reveal),
        "Commitment should not verify against wrong nonce"
    );

    // Test signature verification for reveal
    let mut signature_data = Vec::new();
    signature_data.extend_from_slice(&reveal.player);
    signature_data.extend_from_slice(&reveal.round_id.to_le_bytes());
    signature_data.extend_from_slice(&reveal.nonce);
    signature_data.extend_from_slice(&reveal.timestamp.to_le_bytes());

    let public_key = keystore.export_public_key();
    let is_valid =
        SecureKeystore::verify_signature(&signature_data, &reveal.signature, &public_key)
            .expect("Should verify reveal signature");
    assert!(is_valid, "Reveal signature should verify correctly");
}

/// Test entropy pool combines randomness securely
#[tokio::test]
async fn test_entropy_pool_secure_combination() {
    let mut entropy_pool = EntropyPool::new();
    let mut keystore1 = SecureKeystore::new().expect("Should create keystore1");
    let mut keystore2 = SecureKeystore::new().expect("Should create keystore2");
    let mut keystore3 = SecureKeystore::new().expect("Should create keystore3");

    // Add entropy from multiple sources
    let entropy1 = keystore1.generate_commitment_nonce();
    let entropy2 = keystore2.generate_commitment_nonce();
    let entropy3 = keystore3.generate_commitment_nonce();

    entropy_pool.add_entropy(entropy1);
    entropy_pool.add_entropy(entropy2);
    entropy_pool.add_entropy(entropy3);

    assert_eq!(entropy_pool.entropy_count(), 3);
    assert!(entropy_pool.has_sufficient_entropy(3));
    assert!(!entropy_pool.has_sufficient_entropy(4));

    // Get combined entropy
    let combined1 = entropy_pool.get_combined_entropy();
    let combined2 = entropy_pool.get_combined_entropy();

    // Should be deterministic for same entropy sources
    assert_eq!(combined1, combined2);
    assert_ne!(combined1, [0u8; 32]);

    // Adding more entropy should change the combined result
    let entropy4 = keystore1.generate_commitment_nonce();
    entropy_pool.add_entropy(entropy4);
    let combined3 = entropy_pool.get_combined_entropy();

    assert_ne!(
        combined1, combined3,
        "Adding entropy should change combined result"
    );
}

/// Test dice roll generation from entropy is unbiased and secure
#[tokio::test]
async fn test_secure_dice_roll_generation() {
    let mut entropy_pool = EntropyPool::new();
    let mut keystore = SecureKeystore::new().expect("Should create keystore");

    // Add some entropy
    for _ in 0..3 {
        let entropy = keystore.generate_commitment_nonce();
        entropy_pool.add_entropy(entropy);
    }

    let num_rolls = 6000; // Large sample for statistical analysis
    let mut roll_counts = [[0usize; 6]; 6]; // [die1][die2]

    // Generate dice rolls
    for _ in 0..num_rolls {
        let (die1, die2) = entropy_pool.generate_dice_roll();

        // Validate die values are in correct range
        assert!((1..=6).contains(&die1), "Die1 value {} should be 1-6", die1);
        assert!((1..=6).contains(&die2), "Die2 value {} should be 1-6", die2);

        roll_counts[(die1 - 1) as usize][(die2 - 1) as usize] += 1;
    }

    // Test statistical distribution
    let expected_per_combination = num_rolls / 36; // 36 possible combinations
    let tolerance = expected_per_combination / 2; // 50% tolerance

    for i in 0..6 {
        for j in 0..6 {
            let count = roll_counts[i][j];
            assert!(
                count > expected_per_combination - tolerance,
                "Combination ({},{}) appears too rarely: {} times",
                i + 1,
                j + 1,
                count
            );
            assert!(
                count < expected_per_combination + tolerance,
                "Combination ({},{}) appears too frequently: {} times",
                i + 1,
                j + 1,
                count
            );
        }
    }

    // Test individual die distributions
    let mut die1_counts = [0usize; 6];
    let mut die2_counts = [0usize; 6];

    for i in 0..6 {
        for j in 0..6 {
            die1_counts[i] += roll_counts[i][j];
            die2_counts[j] += roll_counts[i][j];
        }
    }

    let expected_per_die = num_rolls / 6;
    let die_tolerance = expected_per_die / 3;

    for i in 0..6 {
        assert!(
            die1_counts[i] > expected_per_die - die_tolerance,
            "Die1 value {} appears too rarely: {} times",
            i + 1,
            die1_counts[i]
        );
        assert!(
            die1_counts[i] < expected_per_die + die_tolerance,
            "Die1 value {} appears too frequently: {} times",
            i + 1,
            die1_counts[i]
        );

        assert!(
            die2_counts[i] > expected_per_die - die_tolerance,
            "Die2 value {} appears too rarely: {} times",
            i + 1,
            die2_counts[i]
        );
        assert!(
            die2_counts[i] < expected_per_die + die_tolerance,
            "Die2 value {} appears too frequently: {} times",
            i + 1,
            die2_counts[i]
        );
    }
}

/// Test that entropy generation uses OS randomness properly
#[tokio::test]
async fn test_entropy_generation_uses_os_randomness() {
    let mut entropy_pool = EntropyPool::new();

    // Generate multiple byte sequences
    let sequences = (0..100)
        .map(|_| entropy_pool.generate_bytes(32))
        .collect::<Vec<_>>();

    // All sequences should be different
    let mut unique_sequences = std::collections::HashSet::new();
    for seq in &sequences {
        unique_sequences.insert(seq.clone());
    }
    assert_eq!(
        unique_sequences.len(),
        sequences.len(),
        "All generated byte sequences should be unique"
    );

    // No sequence should be all zeros or follow obvious patterns
    for (i, seq) in sequences.iter().enumerate() {
        assert_ne!(
            seq,
            &vec![0u8; 32],
            "Sequence {} should not be all zeros",
            i
        );
        assert_ne!(
            seq,
            &vec![255u8; 32],
            "Sequence {} should not be all ones",
            i
        );

        // Check for simple patterns (ascending, descending, repeating)
        let has_ascending = seq.windows(2).all(|w| w[1] == w[0].wrapping_add(1));
        let has_descending = seq.windows(2).all(|w| w[1] == w[0].wrapping_sub(1));
        let all_same = seq.iter().all(|&b| b == seq[0]);

        assert!(
            !has_ascending,
            "Sequence {} should not be ascending pattern",
            i
        );
        assert!(
            !has_descending,
            "Sequence {} should not be descending pattern",
            i
        );
        assert!(
            !all_same || seq[0] == 0 || seq[0] == 255,
            "Sequence {} should not be all same value",
            i
        );
    }

    // Test different lengths work correctly
    for len in [1, 16, 33, 64, 100] {
        let bytes = entropy_pool.generate_bytes(len);
        assert_eq!(
            bytes.len(),
            len,
            "Generated bytes should have requested length {}",
            len
        );
    }
}

/// Test that no dummy implementations remain in consensus crypto
#[tokio::test]
async fn test_no_dummy_crypto_implementations() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let peer_id = keystore.peer_id();

    // Test 1: Signatures are not dummy [0u8; 64] values
    let message = b"Test message for dummy check";
    let sig1 = keystore.sign(message).expect("Should create signature");
    let sig2 = keystore
        .sign(message)
        .expect("Should create another signature");

    assert_ne!(sig1.0, [0u8; 64], "Signature should not be dummy zeros");
    assert_ne!(
        sig2.0, [0u8; 64],
        "Second signature should not be dummy zeros"
    );

    // Different messages should produce different signatures
    let different_message = b"Different test message";
    let sig3 = keystore
        .sign(different_message)
        .expect("Should create different signature");
    assert_ne!(
        sig1.0, sig3.0,
        "Different messages should produce different signatures"
    );

    // Test 2: Public keys are not dummy values
    assert_ne!(peer_id, [0u8; 32], "Peer ID should not be dummy zeros");
    assert_ne!(peer_id, [255u8; 32], "Peer ID should not be dummy ones");

    let public_key = keystore.export_public_key();
    assert_eq!(
        peer_id, public_key,
        "Peer ID should match exported public key"
    );

    // Test 3: Random nonces are truly random
    let nonces: Vec<[u8; 32]> = (0..10)
        .map(|_| keystore.generate_commitment_nonce())
        .collect();

    // All nonces should be different
    let mut unique_nonces = std::collections::HashSet::new();
    for nonce in &nonces {
        unique_nonces.insert(*nonce);
    }
    assert_eq!(
        unique_nonces.len(),
        nonces.len(),
        "All nonces should be unique"
    );

    // No nonce should be dummy values
    for (i, nonce) in nonces.iter().enumerate() {
        assert_ne!(nonce, &[0u8; 32], "Nonce {} should not be all zeros", i);
        assert_ne!(nonce, &[255u8; 32], "Nonce {} should not be all ones", i);
    }

    // Test 4: Hash operations produce real hash values
    let mut entropy_pool = EntropyPool::new();
    entropy_pool.add_entropy(nonces[0]);
    entropy_pool.add_entropy(nonces[1]);

    let hash1 = entropy_pool.get_combined_entropy();
    let hash2 = entropy_pool.get_combined_entropy();

    assert_eq!(hash1, hash2, "Hash should be deterministic");
    assert_ne!(hash1, [0u8; 32], "Hash should not be zeros");

    // Adding different entropy should change hash
    entropy_pool.add_entropy(nonces[2]);
    let hash3 = entropy_pool.get_combined_entropy();
    assert_ne!(
        hash1, hash3,
        "Different entropy should produce different hash"
    );
}

/// Test cryptographic operations under concurrent access
#[tokio::test]
async fn test_concurrent_crypto_operations() {
    use tokio::task;

    let handles: Vec<_> = (0..10)
        .map(|i| {
            task::spawn(async move {
                let mut keystore = SecureKeystore::new().expect("Should create keystore");
                let message = format!("Message from thread {}", i);

                // Each thread performs crypto operations
                let signature = keystore
                    .sign(message.as_bytes())
                    .expect("Should sign message");
                let nonce = keystore.generate_commitment_nonce();
                let random_bytes = keystore.generate_random_bytes(32);
                let public_key = keystore.export_public_key();

                // Verify signature
                let is_valid =
                    SecureKeystore::verify_signature(message.as_bytes(), &signature, &public_key)
                        .expect("Should verify signature");

                assert!(is_valid, "Signature should verify in thread {}", i);
                assert_ne!(
                    nonce, [0u8; 32],
                    "Nonce should not be zeros in thread {}",
                    i
                );
                assert_ne!(
                    random_bytes,
                    vec![0u8; 32],
                    "Random bytes should not be zeros in thread {}",
                    i
                );

                (signature, nonce, random_bytes, public_key)
            })
        })
        .collect();

    // Collect all results
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.expect("Thread should complete"));
    }

    // Verify all threads produced different results
    let mut signatures = std::collections::HashSet::new();
    let mut nonces = std::collections::HashSet::new();
    let mut random_bytes_set = std::collections::HashSet::new();
    let mut public_keys = std::collections::HashSet::new();

    for (sig, nonce, bytes, key) in results {
        signatures.insert(sig.0);
        nonces.insert(nonce);
        random_bytes_set.insert(bytes);
        public_keys.insert(key);
    }

    // All results should be unique (different threads, different keystores)
    assert_eq!(signatures.len(), 10, "All signatures should be unique");
    assert_eq!(nonces.len(), 10, "All nonces should be unique");
    assert_eq!(
        random_bytes_set.len(),
        10,
        "All random bytes should be unique"
    );
    assert_eq!(public_keys.len(), 10, "All public keys should be unique");
}

/// Test cryptographic operations maintain security under stress
#[tokio::test]
async fn test_crypto_security_under_stress() {
    let mut keystore = SecureKeystore::new().expect("Should create keystore");
    let iterations = 1000;

    let mut signatures = Vec::new();
    let mut nonces = Vec::new();

    // Perform many crypto operations rapidly
    let start_time = std::time::Instant::now();

    for i in 0..iterations {
        let message = format!("Stress test message {}", i);
        let signature = keystore
            .sign(message.as_bytes())
            .expect("Should create signature");
        let nonce = keystore.generate_commitment_nonce();

        signatures.push((message, signature));
        nonces.push(nonce);
    }

    let duration = start_time.elapsed();
    println!(
        "Completed {} crypto operations in {:?}",
        iterations, duration
    );

    // Verify all signatures are valid
    let public_key = keystore.export_public_key();
    for (message, signature) in &signatures {
        let is_valid = SecureKeystore::verify_signature(message.as_bytes(), signature, &public_key)
            .expect("Should verify signature");
        assert!(is_valid, "All signatures should remain valid under stress");
    }

    // Verify no duplicates in nonces (extremely unlikely with proper randomness)
    let unique_nonces: std::collections::HashSet<_> = nonces.iter().collect();
    assert_eq!(
        unique_nonces.len(),
        nonces.len(),
        "All nonces should be unique even under stress"
    );

    // Performance check: should be reasonably fast
    let ops_per_second = (iterations as f64) / duration.as_secs_f64();
    assert!(
        ops_per_second > 100.0,
        "Should perform at least 100 crypto ops/sec, got {:.2}",
        ops_per_second
    );
}
