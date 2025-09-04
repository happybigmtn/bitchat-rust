# BitCraps Cross-Chain Bridge Usage Guide

## Quick Start

This guide provides practical examples and step-by-step instructions for using the BitCraps cross-chain bridge infrastructure.

## Prerequisites

### 1. Environment Setup

```bash
# Add bridge features to your Cargo.toml
[dependencies]
bitcraps = { version = "0.1.0", features = ["bridges"] }

# Or enable specific bridges
bitcraps = { version = "0.1.0", features = ["bridge-ethereum"] }
```

### 2. Network Configuration

```rust
use bitcraps::bridges::*;
use std::collections::HashMap;
use std::time::Duration;

// Basic bridge configuration
let bridge_config = BridgeConfig {
    min_amount: 1_000,                    // 0.001 CRAP minimum
    max_amount: 1_000_000_000,            // 1M CRAP maximum  
    fee_percentage: 0.001,                // 0.1% bridge fee
    required_signatures: 2,               // 2-of-3 validator signatures
    timeout_duration: Duration::from_secs(3600), // 1 hour timeout
    confirmation_requirements: {
        let mut confirmations = HashMap::new();
        confirmations.insert(ChainId::Ethereum, 12);
        confirmations.insert(ChainId::Bitcoin, 6);
        confirmations.insert(ChainId::BinanceSmartChain, 15);
        confirmations
    },
    supported_tokens: {
        let mut tokens = HashMap::new();
        tokens.insert("CRAP".to_string(), vec![
            ChainId::Ethereum, 
            ChainId::Bitcoin, 
            ChainId::BitCraps
        ]);
        tokens.insert("USDC".to_string(), vec![
            ChainId::Ethereum, 
            ChainId::BinanceSmartChain
        ]);
        tokens
    },
    validator_keys: vec![], // Populated by governance
    emergency_pause: false,
};
```

## Ethereum Bridge Usage

### Basic Token Transfer

```rust
use bitcraps::bridges::ethereum::*;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Ethereum bridge
    let eth_config = EthereumBridgeConfig {
        rpc_endpoint: "https://mainnet.infura.io/v3/YOUR-PROJECT-ID".to_string(),
        bridge_contract_address: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        token_contract_address: "0x123456789abcdef0123456789abcdef012345678".to_string(),
        gas_price: 20_000_000_000, // 20 gwei
        gas_limit: 300_000,
        confirmation_blocks: 12,
        chain_id: 1, // Mainnet
        ..Default::default()
    };

    // Create bridge instance
    let bridge = EthereumBridge::new(eth_config);
    
    // Initialize with configuration
    bridge.initialize(bridge_config).await?;

    // Check if token is supported
    let is_supported = bridge
        .is_token_supported("CRAP", &ChainId::Ethereum)
        .await?;
    
    if !is_supported {
        panic!("Token not supported on this chain");
    }

    // Calculate bridge fee
    let amount = 1_000_000; // 1 CRAP
    let fee = bridge
        .calculate_bridge_fee(amount, &ChainId::Ethereum, &ChainId::BitCraps)
        .await?;
    
    println!("Bridge fee: {} satoshis", fee);

    // Create bridge transaction
    let transaction = BridgeTransaction {
        tx_id: [1u8; 32], // In production, use proper ID generation
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [0u8; 32], // Will be populated
        target_tx_hash: None,
        token_address: "0x123456789abcdef0123456789abcdef012345678".to_string(),
        amount,
        bridge_fee: fee,
        sender: "0xUserAddress123456789abcdef0123456789abcdef0".to_string(),
        recipient: "bitcraps_recipient_address".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        expires_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() + 3600,
        metadata: std::collections::HashMap::new(),
    };

    // Initiate bridge operation
    let bridge_tx_id = bridge.initiate_bridge(&transaction).await?;
    
    println!("Bridge transaction initiated: {:?}", hex::encode(bridge_tx_id));

    // Monitor transaction status
    loop {
        let status = bridge.get_transaction_status(&bridge_tx_id).await?;
        println!("Transaction status: {:?}", status);
        
        match status {
            BridgeTransactionStatus::Completed => {
                println!("Bridge transaction completed successfully!");
                break;
            }
            BridgeTransactionStatus::Failed(reason) => {
                println!("Bridge transaction failed: {}", reason);
                break;
            }
            _ => {
                // Wait before checking again
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }

    Ok(())
}
```

### Advanced Ethereum Integration

```rust
// Deploy new token contract
async fn deploy_crap_token() -> Result<String, Box<dyn std::error::Error>> {
    let eth_config = EthereumBridgeConfig::default();
    let bridge = EthereumBridge::new(eth_config);

    let contract_address = bridge
        .deploy_token_contract(
            "BitCraps Token",     // Token name
            "CRAP",              // Token symbol
            21_000_000_000_000,  // Total supply (21T with 12 decimals)
            12,                  // Decimals
        )
        .await?;

    println!("CRAP token deployed at: {}", contract_address);
    Ok(contract_address)
}

// Check token balance
async fn check_balance() -> Result<(), Box<dyn std::error::Error>> {
    let eth_config = EthereumBridgeConfig::default();
    let bridge = EthereumBridge::new(eth_config);

    let balance = bridge
        .get_token_balance(
            "0xUserAddress123...",
            "0xTokenContract123...",
        )
        .await?;

    println!("Token balance: {} CRAP", balance);
    Ok(())
}
```

## Bitcoin Bridge Usage

### Multisig Bridge Operations

```rust
use bitcraps::bridges::bitcoin::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Bitcoin bridge
    let btc_config = BitcoinBridgeConfig {
        rpc_endpoint: "http://127.0.0.1:8332".to_string(),
        rpc_username: "bitcoin".to_string(),
        rpc_password: "your_rpc_password".to_string(),
        network: BitcoinNetwork::Mainnet,
        min_confirmations: 6,
        multisig_threshold: 2,
        max_multisig_participants: 3,
        fee_rate: 10, // 10 sat/byte
        lightning_config: None,
        atomic_swap_config: AtomicSwapConfig::default(),
    };

    let bridge = BitcoinBridge::new(btc_config);
    
    // Initialize bridge
    bridge.initialize(bridge_config).await?;

    // Create multisig wallet for bridge operations
    let validator_pubkeys = vec![
        "03a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890".to_string(),
        "03b2c3d4e5f67890123456789012345678901234567890123456789012345678901234".to_string(),
        "03c3d4e5f678901234567890123456789012345678901234567890123456789012345678".to_string(),
    ];

    let wallet_id = bridge
        .create_multisig_wallet(validator_pubkeys, 2)
        .await?;

    println!("Created multisig wallet: {}", wallet_id);

    // Lock Bitcoin for bridge operation
    let recipient_crap_address = [1u8; 32]; // BitCraps address
    let psbt_id = bridge
        .lock_bitcoin(
            &wallet_id,
            1_000_000, // 0.01 BTC (1M satoshis)
            &recipient_crap_address,
            &ChainId::BitCraps,
        )
        .await?;

    println!("Bitcoin locked, PSBT ID: {}", psbt_id);

    Ok(())
}
```

### Lightning Network Integration

```rust
async fn lightning_bridge_example() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Bitcoin bridge with Lightning
    let mut btc_config = BitcoinBridgeConfig::default();
    btc_config.lightning_config = Some(LightningConfig {
        node_endpoint: "127.0.0.1:9735".to_string(),
        node_pubkey: "03abc123def456...".to_string(),
        bridge_channel_capacity: 16_777_216, // ~0.167 BTC
        min_htlc_amount: 1000,               // 1000 sats
        max_htlc_amount: 4_294_967,          // ~0.043 BTC
        htlc_timeout_blocks: 144,            // ~1 day
        lightning_fee_rate: 1000,            // 1 sat base fee
    });

    let bridge = BitcoinBridge::new(btc_config);

    // Fast Lightning payment for immediate settlement
    let payment_hash = bridge
        .initiate_lightning_payment(
            100_000_000, // 100k msat = 100 sats
            "03def456ghi789...", // Destination node pubkey
            "Fast BitCraps bridge payment",
        )
        .await?;

    println!("Lightning payment initiated: {}", payment_hash);
    Ok(())
}
```

### Atomic Swaps

```rust
async fn atomic_swap_example() -> Result<(), Box<dyn std::error::Error>> {
    let btc_config = BitcoinBridgeConfig::default();
    let bridge = BitcoinBridge::new(btc_config);

    // Create atomic swap offer
    let swap_id = bridge
        .create_atomic_swap(
            5_000_000,        // 0.05 BTC
            50_000_000_000,   // 50 CRAP tokens  
            "bc1qUser123...".to_string(), // Bitcoin participant
            [2u8; 32],        // CRAP participant ID
        )
        .await?;

    println!("Atomic swap created: {}", hex::encode(&swap_id));

    // Lock Bitcoin side of the swap
    let contract_tx = bridge
        .lock_bitcoin_atomic_swap(&swap_id)
        .await?;

    println!("Bitcoin locked in atomic swap: {}", contract_tx);

    // When ready to claim (with secret revealed)
    let secret = [3u8; 32]; // This would be the actual secret
    let redeem_tx = bridge
        .redeem_bitcoin_atomic_swap(&swap_id, secret)
        .await?;

    println!("Bitcoin redeemed from atomic swap: {}", redeem_tx);
    Ok(())
}
```

## Universal Bridge Usage

### Multi-Chain Routing

```rust
use bitcraps::bridges::universal::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);
    
    // Initialize universal bridge
    bridge.initialize(bridge_config).await?;

    // Find optimal route between chains
    let route = bridge
        .find_optimal_route(
            &ChainId::Bitcoin,
            &ChainId::Polygon,
            10_000_000, // Amount to bridge
        )
        .await?;

    println!("Optimal route found:");
    println!("  Source: {:?}", route.source_chain);
    println!("  Target: {:?}", route.target_chain);
    println!("  Hops: {}", route.hops.len());
    println!("  Total cost: {}", route.total_cost);
    println!("  Estimated time: {:?}", route.estimated_time);
    println!("  Reliability: {:.2}%", route.reliability_score * 100.0);

    // Display route details
    for (i, hop) in route.hops.iter().enumerate() {
        println!("  Hop {}: {:?} via {:?} (cost: {}, time: {:?})",
            i + 1, hop.chain, hop.protocol, hop.cost, hop.time);
    }

    Ok(())
}
```

### Cross-Chain Messaging

```rust
async fn cross_chain_messaging() -> Result<(), Box<dyn std::error::Error>> {
    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);

    // Send governance proposal across chains
    let proposal_data = serde_json::to_vec(&json!({
        "proposal_id": 42,
        "title": "Enable new bridge route",
        "description": "Add support for Cosmos Hub",
        "voting_period": 7 * 24 * 3600, // 7 days
    }))?;

    let message_id = bridge
        .send_cross_chain_message(
            ChainId::BitCraps,
            ChainId::Ethereum,
            "governance_proposal".to_string(),
            proposal_data,
            "bitcraps_governance_address".to_string(),
            "0xEthereumGovernanceContract...".to_string(),
        )
        .await?;

    println!("Cross-chain governance proposal sent: {}", hex::encode(&message_id));

    // Send token transfer message
    let transfer_data = serde_json::to_vec(&json!({
        "token": "CRAP",
        "amount": 1_000_000,
        "recipient": "cosmos1recipient...",
    }))?;

    let transfer_message_id = bridge
        .send_cross_chain_message(
            ChainId::Ethereum,
            ChainId::Cosmos,
            "token_transfer".to_string(),
            transfer_data,
            "0xEthereumSender...".to_string(),
            "cosmos1recipient...".to_string(),
        )
        .await?;

    println!("Cross-chain token transfer sent: {}", hex::encode(&transfer_message_id));
    Ok(())
}
```

### Liquidity Management

```rust
async fn liquidity_management() -> Result<(), Box<dyn std::error::Error>> {
    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);

    // Check available liquidity for a route
    let liquidity = bridge
        .aggregate_liquidity(&ChainId::Ethereum, &ChainId::BinanceSmartChain)
        .await?;

    println!("Available liquidity: {} tokens", liquidity);

    // Update liquidity pool information
    bridge.update_liquidity_pools().await?;
    println!("Liquidity pools updated");

    Ok(())
}
```

## Security Best Practices

### Validator Signature Verification

```rust
use bitcraps::crypto::*;
use ed25519_dalek::{Verifier, Signature, VerifyingKey};

async fn verify_bridge_transaction(
    transaction: &BridgeTransaction,
    validator_signatures: &[ValidatorSignature],
) -> Result<bool, Box<dyn std::error::Error>> {
    // Verify all validator signatures
    for signature in validator_signatures {
        // Reconstruct signed message
        let mut message = Vec::new();
        message.extend_from_slice(b"BitCraps Bridge Transaction: ");
        message.extend_from_slice(&transaction.tx_id);
        message.extend_from_slice(&transaction.created_at.to_be_bytes());

        // Parse public key and signature
        let public_key_bytes: [u8; 32] = signature.public_key[..32]
            .try_into()
            .map_err(|_| "Invalid public key length")?;
        let public_key = VerifyingKey::from_bytes(&public_key_bytes)
            .map_err(|_| "Invalid public key")?;

        let signature_bytes: [u8; 64] = signature.signature[..64]
            .try_into()
            .map_err(|_| "Invalid signature length")?;
        let sig = Signature::from_bytes(&signature_bytes);

        // Verify signature
        public_key.verify(&message, &sig)
            .map_err(|_| "Signature verification failed")?;
    }

    Ok(true)
}
```

### Fraud Detection Integration

```rust
async fn setup_fraud_detection() -> Result<(), Box<dyn std::error::Error>> {
    use bitcraps::security::SecurityManager;
    
    let security_manager = Arc::new(SecurityManager::new());
    let bridge_security = BridgeSecurityManager::new(security_manager);

    // Add custom fraud detection rules
    let high_value_rule = FraudDetectionRule {
        name: "high_value_transaction".to_string(),
        description: "Flags transactions above 100M CRAP".to_string(),
        severity: 8,
        evaluate: |tx: &BridgeTransaction| -> bool {
            tx.amount <= 100_000_000_000 // 100M CRAP threshold
        },
    };

    bridge_security
        .add_fraud_detection_rule("high_value".to_string(), high_value_rule)
        .await;

    // Rapid transaction rule
    let rapid_tx_rule = FraudDetectionRule {
        name: "rapid_transactions".to_string(),
        description: "Detects multiple transactions from same address".to_string(),
        severity: 6,
        evaluate: |tx: &BridgeTransaction| -> bool {
            // In production, check transaction history
            true // Placeholder
        },
    };

    bridge_security
        .add_fraud_detection_rule("rapid_tx".to_string(), rapid_tx_rule)
        .await;

    println!("Fraud detection rules configured");
    Ok(())
}
```

## Error Handling and Recovery

### Comprehensive Error Handling

```rust
use bitcraps::{Error, Result};

async fn robust_bridge_operation() -> Result<()> {
    let bridge = /* initialize bridge */;

    let transaction = /* create transaction */;

    // Attempt bridge operation with comprehensive error handling
    match bridge.initiate_bridge(&transaction).await {
        Ok(tx_id) => {
            println!("Bridge initiated successfully: {}", hex::encode(&tx_id));
            
            // Monitor transaction with timeout
            let timeout_duration = Duration::from_secs(1800); // 30 minutes
            match tokio::time::timeout(timeout_duration, monitor_transaction(&bridge, &tx_id)).await {
                Ok(Ok(_)) => println!("Transaction completed successfully"),
                Ok(Err(e)) => {
                    eprintln!("Transaction monitoring failed: {}", e);
                    // Attempt recovery
                    attempt_transaction_recovery(&bridge, &tx_id).await?;
                },
                Err(_) => {
                    eprintln!("Transaction timed out");
                    // Cancel if possible
                    if let Err(cancel_err) = bridge.cancel_transaction(&tx_id, "timeout").await {
                        eprintln!("Failed to cancel timed-out transaction: {}", cancel_err);
                    }
                },
            }
        },
        Err(Error::InvalidData(msg)) => {
            eprintln!("Invalid transaction data: {}", msg);
            return Err(Error::InvalidData("Bridge operation failed due to invalid data".to_string()));
        },
        Err(Error::SecurityViolation(msg)) => {
            eprintln!("Security violation detected: {}", msg);
            // Alert security team
            alert_security_team(&msg).await?;
            return Err(Error::SecurityViolation("Bridge operation blocked for security reasons".to_string()));
        },
        Err(Error::Network(msg)) => {
            eprintln!("Network error: {}", msg);
            // Attempt retry with exponential backoff
            retry_with_backoff(|| bridge.initiate_bridge(&transaction)).await?;
        },
        Err(e) => {
            eprintln!("Unexpected error: {}", e);
            return Err(e);
        },
    }

    Ok(())
}

async fn monitor_transaction(bridge: &impl Bridge, tx_id: &[u8; 32]) -> Result<()> {
    loop {
        let status = bridge.get_transaction_status(tx_id).await?;
        
        match status {
            BridgeTransactionStatus::Completed => return Ok(()),
            BridgeTransactionStatus::Failed(reason) => {
                return Err(Error::InvalidData(format!("Transaction failed: {}", reason)));
            },
            BridgeTransactionStatus::TimedOut => {
                return Err(Error::InvalidData("Transaction timed out".to_string()));
            },
            _ => {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    }
}

async fn attempt_transaction_recovery(bridge: &impl Bridge, tx_id: &[u8; 32]) -> Result<()> {
    // Implementation depends on specific bridge type and failure mode
    println!("Attempting transaction recovery for {}", hex::encode(tx_id));
    
    // Check if transaction can be resumed
    if let Ok(Some(transaction)) = bridge.get_transaction(tx_id).await {
        match transaction.status {
            BridgeTransactionStatus::SourceConfirmed => {
                // Resubmit to validators
                println!("Resubmitting transaction to validators");
                // Implementation would resubmit to validator network
            },
            BridgeTransactionStatus::ValidatorsSigned => {
                // Resubmit to target chain
                println!("Resubmitting transaction to target chain");
                // Implementation would resubmit to target blockchain
            },
            _ => {
                println!("Transaction not in recoverable state");
            }
        }
    }

    Ok(())
}

async fn alert_security_team(message: &str) -> Result<()> {
    // Implementation would send alerts via multiple channels
    println!("🚨 SECURITY ALERT: {}", message);
    
    // Send to monitoring system
    // send_to_monitoring_system(message).await?;
    
    // Send emergency notification
    // send_emergency_notification(message).await?;
    
    Ok(())
}

async fn retry_with_backoff<F, Fut, T>(mut operation: F) -> Result<T> 
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(60);
    let max_retries = 5;
    
    for attempt in 1..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_retries {
                    return Err(e);
                }
                
                eprintln!("Attempt {} failed: {}, retrying in {:?}", attempt, e, delay);
                tokio::time::sleep(delay).await;
                
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }
    }
    
    unreachable!()
}
```

## Integration with BitCraps Gaming

### Game-Specific Bridge Operations

```rust
use bitcraps::{gaming::*, token::*, bridges::*};

// Bridge tokens for gaming session
async fn bridge_for_gaming(
    player_address: &str,
    game_session_id: &str,
    amount: u64,
) -> Result<()> {
    // Initialize Ethereum bridge for CRAP tokens
    let eth_config = EthereumBridgeConfig::default();
    let bridge = EthereumBridge::new(eth_config);
    
    // Create bridge transaction with gaming metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("purpose".to_string(), "gaming".to_string());
    metadata.insert("session_id".to_string(), game_session_id.to_string());
    metadata.insert("player".to_string(), player_address.to_string());

    let transaction = BridgeTransaction {
        tx_id: generate_gaming_tx_id(game_session_id),
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        token_address: "0xCRAPTokenAddress...".to_string(),
        amount,
        sender: player_address.to_string(),
        recipient: format!("bitcraps_gaming_{}", game_session_id),
        metadata,
        ..Default::default()
    };

    let bridge_tx_id = bridge.initiate_bridge(&transaction).await?;
    
    // Wait for bridge completion before allowing game to start
    wait_for_bridge_completion(&bridge, &bridge_tx_id).await?;
    
    println!("Gaming tokens bridged successfully for session {}", game_session_id);
    Ok(())
}

// Withdraw winnings back to Ethereum
async fn withdraw_winnings(
    player_address: &str,
    winnings: u64,
) -> Result<()> {
    let eth_config = EthereumBridgeConfig::default();
    let bridge = EthereumBridge::new(eth_config);

    let transaction = BridgeTransaction {
        tx_id: generate_withdrawal_tx_id(player_address),
        source_chain: ChainId::BitCraps,
        target_chain: ChainId::Ethereum,
        amount: winnings,
        sender: format!("bitcraps_gaming_{}", player_address),
        recipient: player_address.to_string(),
        ..Default::default()
    };

    let bridge_tx_id = bridge.initiate_bridge(&transaction).await?;
    
    println!("Withdrawal initiated: {}", hex::encode(&bridge_tx_id));
    Ok(())
}

fn generate_gaming_tx_id(session_id: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"gaming_bridge_");
    hasher.update(session_id.as_bytes());
    hasher.update(&std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_be_bytes());
    
    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

fn generate_withdrawal_tx_id(player_address: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"withdrawal_bridge_");
    hasher.update(player_address.as_bytes());
    hasher.update(&std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_be_bytes());
    
    let result = hasher.finalize();
    let mut id = [0u8; 32];
    id.copy_from_slice(&result);
    id
}

async fn wait_for_bridge_completion(
    bridge: &EthereumBridge, 
    tx_id: &[u8; 32]
) -> Result<()> {
    let timeout = Duration::from_secs(1800); // 30 minutes
    let start_time = std::time::Instant::now();
    
    loop {
        if start_time.elapsed() > timeout {
            return Err(Error::InvalidData("Bridge timeout".to_string()));
        }
        
        match bridge.get_transaction_status(tx_id).await? {
            BridgeTransactionStatus::Completed => return Ok(()),
            BridgeTransactionStatus::Failed(reason) => {
                return Err(Error::InvalidData(format!("Bridge failed: {}", reason)));
            },
            _ => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }
}
```

## Testing and Development

### Local Development Setup

```bash
# Start local blockchain nodes for testing
docker-compose up -d ganache bitcoin-node

# Run bridge tests
cargo test --features bridges test_bridge_integration

# Run specific bridge tests  
cargo test --features bridge-ethereum test_ethereum_bridge
cargo test --features bridge-bitcoin test_bitcoin_bridge
cargo test test_universal_bridge

# Run security tests
cargo test test_fraud_detection_system
cargo test test_bridge_security_manager
```

### Integration Testing

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_end_to_end_bridge_flow() {
        // Setup test environment
        let bridge_config = create_test_bridge_config();
        let state_manager = Arc::new(BridgeStateManager::new(bridge_config.clone()));
        
        // Create test transaction
        let tx = create_test_transaction();
        
        // Test complete flow
        state_manager.store_transaction(tx.clone()).await.unwrap();
        add_test_validator_signatures(&state_manager, &tx.tx_id).await.unwrap();
        
        let final_tx = state_manager.get_transaction(&tx.tx_id).await.unwrap();
        assert_eq!(final_tx.status, BridgeTransactionStatus::ValidatorsSigned);
    }

    // Helper functions for testing
    fn create_test_bridge_config() -> BridgeConfig {
        BridgeConfig {
            min_amount: 1000,
            max_amount: 1000000,
            fee_percentage: 0.001,
            required_signatures: 2,
            timeout_duration: Duration::from_secs(3600),
            confirmation_requirements: HashMap::new(),
            supported_tokens: HashMap::new(),
            validator_keys: Vec::new(),
            emergency_pause: false,
        }
    }

    fn create_test_transaction() -> BridgeTransaction {
        BridgeTransaction {
            tx_id: [1u8; 32],
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::BitCraps,
            source_tx_hash: [2u8; 32],
            target_tx_hash: None,
            token_address: "0x742d35Cc...".to_string(),
            amount: 1000000,
            bridge_fee: 1000,
            sender: "0x742d35Cc...".to_string(),
            recipient: "bitcraps_recipient".to_string(),
            status: BridgeTransactionStatus::Initiated,
            required_confirmations: 2,
            current_confirmations: 0,
            validator_signatures: Vec::new(),
            created_at: 1234567890,
            updated_at: 1234567890,
            expires_at: 1234567890 + 3600,
            metadata: HashMap::new(),
        }
    }

    async fn add_test_validator_signatures(
        state_manager: &BridgeStateManager,
        tx_id: &[u8; 32],
    ) -> Result<()> {
        // Add first validator signature
        let sig1 = ValidatorSignature {
            validator_id: [3u8; 32],
            signature: vec![1, 2, 3, 4],
            timestamp: 1234567890,
            public_key: vec![1; 32],
        };
        state_manager.add_validator_signature(tx_id, sig1).await?;

        // Add second validator signature  
        let sig2 = ValidatorSignature {
            validator_id: [4u8; 32],
            signature: vec![5, 6, 7, 8],
            timestamp: 1234567890,
            public_key: vec![2; 32],
        };
        state_manager.add_validator_signature(tx_id, sig2).await?;

        Ok(())
    }
}
```

## Troubleshooting

### Common Issues

1. **Bridge Transaction Stuck**
   ```rust
   // Check transaction status
   let status = bridge.get_transaction_status(&tx_id).await?;
   
   // For stuck transactions, check validator signatures
   let transaction = bridge.get_transaction(&tx_id).await?;
   if let Some(tx) = transaction {
       println!("Current signatures: {}/{}", 
                tx.current_confirmations, tx.required_confirmations);
   }
   ```

2. **Insufficient Gas/Fees**
   ```rust
   // Calculate optimal gas price
   let gas_price = bridge.get_optimal_gas_price().await?;
   
   // Increase gas limit for complex operations
   let mut config = bridge_config.clone();
   config.gas_limit = 500_000; // Increase from default 300k
   ```

3. **Validator Signature Issues**
   ```rust
   // Verify validator signatures manually
   verify_bridge_transaction(&transaction, &transaction.validator_signatures).await?;
   
   // Check validator network status
   let active_validators = bridge.get_active_validators().await?;
   println!("Active validators: {}", active_validators.len());
   ```

### Debugging Commands

```bash
# Enable debug logging
RUST_LOG=debug cargo run --features bridges

# Run with specific bridge features
cargo run --features bridge-ethereum,bridge-bitcoin

# Test with verbose output
cargo test --features bridges -- --nocapture
```

---

*This usage guide covers the most common bridge operations. For advanced usage and custom implementations, refer to the full API documentation and architecture guide.*