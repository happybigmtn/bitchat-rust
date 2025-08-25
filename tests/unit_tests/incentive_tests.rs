use bitcraps::token::{ProofOfRelay, TokenLedger};
use std::sync::Arc;

#[tokio::test]
async fn test_proof_of_relay_creation() {
    let ledger = Arc::new(TokenLedger::new());
    let proof_system = ProofOfRelay::new(ledger);
    
    // Test that proof system is created
    assert!(true); // Just verify it compiles and creates
}

#[tokio::test]
async fn test_proof_of_relay_stats() {
    let ledger = Arc::new(TokenLedger::new());
    let proof_system = ProofOfRelay::new(ledger.clone());
    
    // Get initial stats
    let stats = proof_system.get_stats().await;
    
    // Check stats structure
    assert_eq!(stats.total_relays, 0);
    assert_eq!(stats.total_rewards_distributed, 0);
}