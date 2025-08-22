use bitcraps::crypto::{BitchatKeypair, GameCrypto};
use bitcraps::protocol::PeerId;

/// Test cryptographic key generation
/// 
/// Feynman: This is like checking that our casino's ID card printer works.
/// Every player and dealer needs a unique, unforgeable ID (keypair).
/// We test that the machine produces IDs of the right size and format.
#[tokio::test]
async fn test_key_generation() {
    let keypair = BitchatKeypair::generate();
    // Verifying key is 32 bytes when serialized
    let public_bytes = keypair.verifying_key.to_bytes();
    assert_eq!(public_bytes.len(), 32);
}

/// Test game ID generation
/// 
/// Feynman: This tests our "game ID system" - like generating unique
/// table numbers in a casino. Each game needs a unique identifier.
#[tokio::test]
async fn test_game_id_generation() {
    let game_id = GameCrypto::generate_game_id();
    assert_eq!(game_id.len(), 16);
}

/// Test randomness commitment
#[tokio::test]  
async fn test_randomness_commitment() {
    let secret = [42u8; 32];
    let commitment = GameCrypto::commit_randomness(&secret);
    
    // Commitment should be 32 bytes
    assert_eq!(commitment.len(), 32);
    
    // Verify commitment
    assert!(GameCrypto::verify_commitment(&commitment, &secret));
    
    // Wrong secret should fail verification
    let wrong_secret = [43u8; 32];
    assert!(!GameCrypto::verify_commitment(&commitment, &wrong_secret));
}