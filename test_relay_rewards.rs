use std::sync::Arc;
use bitcraps::{TokenLedger, ProofOfRelay, PeerId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing Relay Rewards System");
    
    // Setup
    let ledger = Arc::new(TokenLedger::new());
    let proof_of_relay = ProofOfRelay::new(ledger.clone());
    let test_peer: PeerId = [42u8; 32];
    
    // Create account
    ledger.create_account(test_peer).await?;
    let initial_balance = ledger.get_balance(&test_peer).await;
    println!("Initial balance: {} CRAP", initial_balance as f64 / 1_000_000.0);
    
    // Update relay score (simulating message forwarding)
    proof_of_relay.update_relay_score(test_peer, 10).await;
    
    // Process relay rewards (as the test expects)
    let reward_result = ledger.process_relay_reward(test_peer, 10).await;
    assert!(reward_result.is_ok(), "Should successfully process relay rewards");
    
    // Check final balance
    let final_balance = ledger.get_balance(&test_peer).await;
    println!("Final balance: {} CRAP", final_balance as f64 / 1_000_000.0);
    
    assert!(final_balance > initial_balance, "Balance should increase from relay rewards");
    println!("✅ Relay rewards test passed! Earned {} CRAP for relaying messages", 
             (final_balance - initial_balance) as f64 / 1_000_000.0);
    
    Ok(())
}