//! Basic end-to-end test scenarios for BitCraps
//! 
//! Simple tests that validate core functionality without complex dependencies

use bitcraps::{
    protocol::{PeerId, BetType},
    crypto::{BitchatKeypair, BitchatIdentity, GameCrypto},
    gaming::{GameSessionManager, random_peer_id, AntiCheatDetector, CrapsBet},
    token::TokenLedger,
};
use tokio::time::{sleep, Duration};

/// Test basic game session creation and management
#[tokio::test]
async fn test_basic_game_session() {
    let session_manager = GameSessionManager::new(Default::default());
    let creator = random_peer_id();
    let player = random_peer_id();
    
    // Create a game session
    let game_id = session_manager.create_session(creator).await.unwrap();
    assert!(!game_id.is_empty(), "Game ID should not be empty");
    
    // Second player joins
    let join_result = session_manager.join_session(&game_id, player).await;
    assert!(join_result.is_ok(), "Player should be able to join game");
}

/// Test token ledger basic operations
#[tokio::test]
async fn test_token_ledger_basics() {
    let ledger = TokenLedger::new();
    
    let player1 = random_peer_id();
    let player2 = random_peer_id();
    
    // Create accounts
    ledger.create_account(player1).await.unwrap();
    ledger.create_account(player2).await.unwrap();
    
    // Check initial balances
    let balance1 = ledger.get_balance(&player1).await;
    let balance2 = ledger.get_balance(&player2).await;
    
    assert_eq!(balance1, 0, "New accounts should start with zero balance");
    assert_eq!(balance2, 0, "New accounts should start with zero balance");
}

/// Test cryptographic operations
#[tokio::test]
async fn test_crypto_operations() {
    // Generate keypairs
    let keypair1 = BitchatKeypair::generate();
    let keypair2 = BitchatKeypair::generate();
    
    // Create identities with proof-of-work
    let identity1 = BitchatIdentity::from_keypair_with_pow(keypair1, 8); // Low difficulty for tests
    let identity2 = BitchatIdentity::from_keypair_with_pow(keypair2, 8);
    
    assert_ne!(identity1.peer_id, identity2.peer_id, "Different keypairs should produce different peer IDs");
    
    // Test signing and verification
    let message = b"test message for signing";
    let signature = identity1.keypair.sign(message);
    
    assert!(identity1.keypair.verify(message, &signature), "Signature should verify with correct key");
    assert!(!identity2.keypair.verify(message, &signature), "Signature should not verify with wrong key");
}

/// Test anti-cheat detection
#[tokio::test]
async fn test_anti_cheat_detection() {
    let detector = AntiCheatDetector::new();
    let player = random_peer_id();
    
    // Create valid bets
    for i in 0..25 { // Below the limit of 30
        let bet = CrapsBet {
            player,
            bet_type: BetType::Pass,
            amount: 100,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + i, // Ensure different timestamps
        };
        
        let result = detector.validate_bet(&bet, &player).await;
        assert!(result.is_ok(), "Valid bet {} should pass anti-cheat", i);
    }
    
    // This bet should trigger anti-cheat (over the limit)
    for i in 25..35 {
        let bet = CrapsBet {
            player,
            bet_type: BetType::Pass,
            amount: 100,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + i,
        };
        
        let result = detector.validate_bet(&bet, &player).await;
        if i >= 30 {
            assert!(result.is_err(), "Bet {} should trigger anti-cheat", i);
        }
    }
}

/// Test randomness commitment scheme
#[tokio::test]
async fn test_randomness_commitment() {
    // Generate secret for commitment
    let secret = GameCrypto::random_bytes::<32>();
    
    // Create commitment
    let commitment = GameCrypto::commit_randomness(&secret);
    
    // Commitments should not be zero and should be deterministic
    assert_ne!(commitment, [0u8; 32], "Commitment should not be zero");
    assert_ne!(secret, [0u8; 32], "Secret should not be zero");
    
    // Same secret should produce same commitment
    let commitment2 = GameCrypto::commit_randomness(&secret);
    assert_eq!(commitment, commitment2, "Same secret should produce same commitment");
    
    // Verify commitment
    let verified = GameCrypto::verify_commitment(&commitment, &secret);
    assert!(verified, "Valid commitment should verify");
    
    // Invalid commitment should not verify
    let mut invalid_commitment = commitment;
    invalid_commitment[0] ^= 0x01; // Flip one bit
    let invalid_verified = GameCrypto::verify_commitment(&invalid_commitment, &secret);
    assert!(!invalid_verified, "Invalid commitment should not verify");
    
    // Wrong secret should not verify
    let wrong_secret = GameCrypto::random_bytes::<32>();
    let wrong_verified = GameCrypto::verify_commitment(&commitment, &wrong_secret);
    assert!(!wrong_verified, "Wrong secret should not verify");
}

/// Test concurrent session operations
#[tokio::test]
async fn test_concurrent_sessions() {
    let session_manager = GameSessionManager::new(Default::default());
    
    // Create multiple sessions concurrently
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let session_manager = session_manager.clone();
        let handle = tokio::spawn(async move {
            let creator = random_peer_id();
            let session_id = session_manager.create_session(creator).await.unwrap();
            (i, session_id)
        });
        handles.push(handle);
    }
    
    // Wait for all sessions to be created
    let mut session_ids = Vec::new();
    for handle in handles {
        let (i, session_id) = handle.await.unwrap();
        session_ids.push((i, session_id));
    }
    
    assert_eq!(session_ids.len(), 10, "All sessions should be created");
    
    // All session IDs should be unique
    let mut unique_ids = std::collections::HashSet::new();
    for (_, session_id) in session_ids {
        assert!(unique_ids.insert(session_id), "Session IDs should be unique");
    }
}

/// Test memory usage patterns
#[tokio::test] 
async fn test_memory_patterns() {
    // Test that we can create and destroy many objects without issues
    for batch in 0..10 {
        let mut objects = Vec::new();
        
        // Create a batch of objects
        for i in 0..100 {
            let keypair = BitchatKeypair::generate();
            let peer_id = random_peer_id();
            objects.push((keypair, peer_id, batch * 100 + i));
        }
        
        // Verify all objects are valid
        assert_eq!(objects.len(), 100);
        
        // Objects will be dropped automatically
        drop(objects);
        
        // Small delay to allow cleanup
        if batch % 3 == 0 {
            sleep(Duration::from_millis(1)).await;
        }
    }
    
    // Test passes if no memory issues occurred
}

/// Test serialization compatibility
#[tokio::test]
async fn test_serialization_compatibility() {
    let peer_id = random_peer_id();
    let bet_type = BetType::Pass;
    
    // Test PeerId serialization
    let peer_bytes = bincode::serialize(&peer_id).unwrap();
    let deserialized_peer: PeerId = bincode::deserialize(&peer_bytes).unwrap();
    assert_eq!(peer_id, deserialized_peer);
    
    // Test BetType serialization
    let bet_bytes = bincode::serialize(&bet_type).unwrap();
    let deserialized_bet: BetType = bincode::deserialize(&bet_bytes).unwrap();
    assert_eq!(bet_type, deserialized_bet);
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_error_handling() {
    let ledger = TokenLedger::new();
    let non_existent_peer = random_peer_id();
    
    // Querying non-existent account should return 0, not error
    let balance = ledger.get_balance(&non_existent_peer).await;
    assert_eq!(balance, 0, "Non-existent account should have zero balance");
    
    // Creating duplicate account should error
    let peer_id = random_peer_id();
    let result1 = ledger.create_account(peer_id).await;
    assert!(result1.is_ok(), "First account creation should succeed");
    
    let result2 = ledger.create_account(peer_id).await;
    assert!(result2.is_err(), "Duplicate account creation should fail");
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    
    /// Test that operations complete within reasonable time
    #[tokio::test]
    async fn test_operation_performance() {
        let start = Instant::now();
        
        // Perform many quick operations
        let mut operations = 0;
        
        // Keypair generation
        for _ in 0..100 {
            let _keypair = BitchatKeypair::generate();
            operations += 1;
        }
        
        // Random peer ID generation
        for _ in 0..1000 {
            let _peer_id = random_peer_id();
            operations += 1;
        }
        
        // Session creation
        let session_manager = GameSessionManager::new(Default::default());
        for _ in 0..50 {
            let creator = random_peer_id();
            let _session = session_manager.create_session(creator).await.unwrap();
            operations += 1;
        }
        
        let duration = start.elapsed();
        let ops_per_sec = operations as f64 / duration.as_secs_f64();
        
        println!("Completed {} operations in {:?} ({:.0} ops/sec)", 
                 operations, duration, ops_per_sec);
        
        // Should be reasonably fast for mobile hardware
        assert!(duration.as_secs() < 30, "Operations should complete within 30 seconds");
        assert!(ops_per_sec > 10.0, "Should maintain reasonable throughput");
    }
}