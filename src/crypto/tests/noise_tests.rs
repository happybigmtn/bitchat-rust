#[cfg(test)]
mod tests {
    use crate::crypto::*;
    use crate::protocol::*;
    
    #[test]
    fn test_key_generation() {
        let noise_keys = NoiseKeyPair::generate();
        let signing_keys = SigningKeyPair::generate();
        
        assert_eq!(noise_keys.public_bytes().len(), 32);
        assert_eq!(signing_keys.public_bytes().len(), 32);
    }
    
    #[test]
    fn test_signing_and_verification() {
        let keys = SigningKeyPair::generate();
        let message = b"Test message for signing";
        
        let signature = keys.sign(message);
        let result = SigningKeyPair::verify(&keys.verifying_key, message, &signature);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_noise_handshake() {
        let alice_identity = BitchatIdentity::generate();
        let bob_identity = BitchatIdentity::generate();
        
        let mut alice_service = NoiseEncryptionService::new(alice_identity).unwrap();
        let mut bob_service = NoiseEncryptionService::new(bob_identity).unwrap();
        
        let alice_peer_id = alice_service.get_identity().peer_id();
        let bob_peer_id = bob_service.get_identity().peer_id();
        
        // Alice initiates handshake
        let msg1 = alice_service.initiate_handshake(bob_peer_id).unwrap();
        
        // Bob responds
        let (msg2, extracted_alice_id) = bob_service.respond_to_handshake(&msg1).unwrap();
        assert_eq!(extracted_alice_id, alice_peer_id);
        
        // Alice completes handshake
        alice_service.complete_handshake(bob_peer_id, &msg2).unwrap();
        
        // Both should have established sessions
        assert!(alice_service.has_session(&bob_peer_id));
        assert!(bob_service.has_session(&alice_peer_id));
    }
    
    #[test]
    fn test_encryption_decryption() {
        // Set up established session (abbreviated for brevity)
        let alice_identity = BitchatIdentity::generate();
        let bob_identity = BitchatIdentity::generate();
        
        let mut alice_service = NoiseEncryptionService::new(alice_identity).unwrap();
        let mut bob_service = NoiseEncryptionService::new(bob_identity).unwrap();
        
        let alice_peer_id = alice_service.get_identity().peer_id();
        let bob_peer_id = bob_service.get_identity().peer_id();
        
        // Complete handshake (abbreviated)
        let msg1 = alice_service.initiate_handshake(bob_peer_id).unwrap();
        let (msg2, _) = bob_service.respond_to_handshake(&msg1).unwrap();
        alice_service.complete_handshake(bob_peer_id, &msg2).unwrap();
        
        // Test message encryption/decryption
        let plaintext = b"Secret message from Alice to Bob";
        
        let ciphertext = alice_service.encrypt(bob_peer_id, plaintext).unwrap();
        let decrypted = bob_service.decrypt(alice_peer_id, &ciphertext).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_game_crypto_nonce_generation() {
        let nonce1 = GameCrypto::generate_nonce();
        let nonce2 = GameCrypto::generate_nonce();
        
        assert_ne!(nonce1, nonce2);
        assert_eq!(nonce1.len(), NONCE_SIZE);
    }
    
    #[test]
    fn test_game_key_derivation() {
        let identity = BitchatIdentity::generate();
        let game_crypto = GameCrypto::new(identity);
        let game_id = GameId::new();
        
        let key1 = game_crypto.derive_game_key(&game_id);
        let key2 = game_crypto.derive_game_key(&game_id);
        
        // Same game ID should produce same key
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }
    
    #[test]
    fn test_randomness_combination() {
        let player1 = PeerId::new([1u8; 32]);
        let player2 = PeerId::new([2u8; 32]);
        let game_id = GameId::new();
        
        let reveals = vec![
            RandomnessReveal {
                nonce: [1u8; NONCE_SIZE],
                player_id: player1,
                game_id,
                timestamp: 1234567890,
            },
            RandomnessReveal {
                nonce: [2u8; NONCE_SIZE],
                player_id: player2,
                game_id,
                timestamp: 1234567891,
            },
        ];
        
        let seed = GameCrypto::combine_randomness(&reveals).unwrap();
        assert_eq!(seed.len(), 32);
        
        // Should be deterministic
        let seed2 = GameCrypto::combine_randomness(&reveals).unwrap();
        assert_eq!(seed, seed2);
    }
    
    #[test]
    fn test_dice_roll_generation() {
        let seed = [42u8; 32];
        let round = 1;
        
        let roll1 = GameCrypto::generate_dice_roll(&seed, round);
        let roll2 = GameCrypto::generate_dice_roll(&seed, round);
        
        // Should be deterministic
        assert_eq!(roll1.die1, roll2.die1);
        assert_eq!(roll1.die2, roll2.die2);
        
        // Dice should be in valid range
        assert!(roll1.die1 >= 1 && roll1.die1 <= 6);
        assert!(roll1.die2 >= 1 && roll1.die2 <= 6);
        
        // Different rounds should produce different results
        let roll3 = GameCrypto::generate_dice_roll(&seed, round + 1);
        assert!(roll1.die1 != roll3.die1 || roll1.die2 != roll3.die2);
    }
    
    #[test]
    fn test_bet_commitment() {
        let identity = BitchatIdentity::generate();
        let game_crypto = GameCrypto::new(identity.clone());
        let bet_amount = 50;
        let nonce = [42u8; 16];
        
        let commitment = game_crypto.create_bet_commitment(bet_amount, &nonce);
        
        // Should verify correctly
        assert!(game_crypto.verify_bet_commitment(
            &commitment,
            bet_amount,
            &nonce,
            &identity.peer_id()
        ));
        
        // Should fail with wrong amount
        assert!(!game_crypto.verify_bet_commitment(
            &commitment,
            bet_amount + 1,
            &nonce,
            &identity.peer_id()
        ));
        
        // Should fail with wrong nonce
        let wrong_nonce = [43u8; 16];
        assert!(!game_crypto.verify_bet_commitment(
            &commitment,
            bet_amount,
            &wrong_nonce,
            &identity.peer_id()
        ));
    }
}