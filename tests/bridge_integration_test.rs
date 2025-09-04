//! Comprehensive Bridge Integration Tests
//!
//! Tests the complete cross-chain bridge infrastructure including:
//! - Bridge core functionality
//! - Ethereum bridge operations
//! - Bitcoin bridge operations  
//! - Universal bridge protocol
//! - Security and fraud detection
//! - Multi-chain routing
//! - IBC protocol implementation

use bitcraps::bridges::*;
use bitcraps::bridges::ethereum::*;
use bitcraps::bridges::bitcoin::*;
use bitcraps::bridges::universal::*;
use bitcraps::{Error, Result};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Test bridge core infrastructure
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_core_infrastructure() {
    // Test ChainId functionality
    assert_eq!(ChainId::Ethereum.native_currency(), "ETH");
    assert_eq!(ChainId::Bitcoin.native_currency(), "BTC");
    assert_eq!(ChainId::BitCraps.native_currency(), "CRAP");
    
    assert!(ChainId::Ethereum.is_evm_compatible());
    assert!(!ChainId::Bitcoin.is_evm_compatible());
    
    assert!(ChainId::Bitcoin.is_utxo_based());
    assert!(!ChainId::Ethereum.is_utxo_based());

    // Test bridge transaction status transitions
    let status = BridgeTransactionStatus::Initiated;
    assert_eq!(status, BridgeTransactionStatus::Initiated);
    
    let status = BridgeTransactionStatus::ValidatorsSigned;
    assert_ne!(status, BridgeTransactionStatus::Failed("test".to_string()));
}

/// Test bridge state manager functionality
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_state_manager() {
    let config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    let state_manager = BridgeStateManager::new(config);
    
    let tx = BridgeTransaction {
        tx_id: [1u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [2u8; 32],
        target_tx_hash: None,
        token_address: "0x1234".to_string(),
        amount: 1000000,
        bridge_fee: 1000,
        sender: "sender".to_string(),
        recipient: "recipient".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    // Store and retrieve transaction
    state_manager.store_transaction(tx.clone()).await.unwrap();
    let retrieved = state_manager.get_transaction(&tx.tx_id).await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().amount, 1000000);

    // Test status update
    state_manager
        .update_transaction_status(&tx.tx_id, BridgeTransactionStatus::SourceConfirmed)
        .await
        .unwrap();
    
    let updated = state_manager.get_transaction(&tx.tx_id).await.unwrap();
    assert_eq!(updated.status, BridgeTransactionStatus::SourceConfirmed);
}

/// Test bridge security manager
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_security_manager() {
    use bitcraps::security::SecurityManager;
    use bitcraps::validation::InputValidator;
    
    let security_manager = Arc::new(SecurityManager::new());
    let bridge_security = BridgeSecurityManager::new(security_manager);
    
    let tx = BridgeTransaction {
        tx_id: [1u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::Bitcoin,
        source_tx_hash: [2u8; 32],
        target_tx_hash: None,
        token_address: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        amount: 1000000,
        bridge_fee: 1000,
        sender: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        recipient: "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    // Test transaction validation
    let result = bridge_security.validate_bridge_transaction(&tx).await;
    assert!(result.is_ok());

    // Test invalid transaction (zero amount)
    let mut invalid_tx = tx.clone();
    invalid_tx.amount = 0;
    let result = bridge_security.validate_bridge_transaction(&invalid_tx).await;
    assert!(result.is_err());

    // Test same chain transaction
    let mut same_chain_tx = tx.clone();
    same_chain_tx.target_chain = ChainId::Ethereum;
    let result = bridge_security.validate_bridge_transaction(&same_chain_tx).await;
    assert!(result.is_err());
}

/// Test Ethereum bridge implementation
#[cfg(all(feature = "bridges", feature = "ethereum"))]
#[tokio::test]
async fn test_ethereum_bridge() {
    let eth_config = EthereumBridgeConfig::default();
    let bridge = EthereumBridge::new(eth_config);
    
    let bridge_config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    // Test initialization
    let result = bridge.initialize(bridge_config).await;
    assert!(result.is_ok());

    // Test token support
    let supported = bridge.is_token_supported("CRAP", &ChainId::Ethereum).await.unwrap();
    assert!(supported);
    
    let not_supported = bridge.is_token_supported("UNKNOWN", &ChainId::Bitcoin).await.unwrap();
    assert!(!not_supported);

    // Test fee calculation
    let fee = bridge.calculate_bridge_fee(100000, &ChainId::Ethereum, &ChainId::BitCraps).await.unwrap();
    assert_eq!(fee, 1000); // Max of calculated fee (100) and minimum fee (1000)

    // Test supported chains
    let chains = bridge.get_supported_chains().await.unwrap();
    assert!(chains.contains(&ChainId::Ethereum));
    assert!(chains.contains(&ChainId::BitCraps));
}

/// Test Bitcoin bridge implementation
#[cfg(all(feature = "bridges", feature = "bitcoin"))]
#[tokio::test]
async fn test_bitcoin_bridge() {
    let btc_config = BitcoinBridgeConfig::default();
    let bridge = BitcoinBridge::new(btc_config);
    
    let bridge_config = BridgeConfig {
        min_amount: 100_000, // 0.001 BTC
        max_amount: 100_000_000, // 1 BTC
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    // Test initialization
    let result = bridge.initialize(bridge_config).await;
    assert!(result.is_ok());

    // Test multisig wallet creation
    let pubkeys = vec![
        "03000000000000000000000000000000000000000000000000000000000000000001".to_string(),
        "03000000000000000000000000000000000000000000000000000000000000000002".to_string(),
        "03000000000000000000000000000000000000000000000000000000000000000003".to_string(),
    ];
    
    let wallet_id = bridge.create_multisig_wallet(pubkeys, 2).await.unwrap();
    assert!(!wallet_id.is_empty());

    // Test atomic swap creation
    let swap_id = bridge.create_atomic_swap(
        1_000_000, // 0.01 BTC
        10_000_000_000, // 10 CRAP
        "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq".to_string(),
        [1u8; 32], // Mock CRAP participant
    ).await.unwrap();
    
    assert_ne!(swap_id, [0u8; 32]);

    // Test Lightning payment (if configured)
    let mut btc_config_with_ln = BitcoinBridgeConfig::default();
    btc_config_with_ln.lightning_config = Some(LightningConfig::default());
    
    let bridge_with_ln = BitcoinBridge::new(btc_config_with_ln);
    
    let payment_hash = bridge_with_ln.initiate_lightning_payment(
        100_000_000, // 100k msat = 100 sats
        "03000000000000000000000000000000000000000000000000000000000000000001",
        "Bridge payment test",
    ).await.unwrap();
    
    assert!(!payment_hash.is_empty());
}

/// Test Universal bridge implementation
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_universal_bridge() {
    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);
    
    let bridge_config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1_000_000_000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    // Test initialization
    let result = bridge.initialize(bridge_config).await;
    assert!(result.is_ok());

    // Test cross-chain message
    let message_id = bridge.send_cross_chain_message(
        ChainId::Ethereum,
        ChainId::Bitcoin,
        "token_transfer".to_string(),
        b"test_payload".to_vec(),
        "sender_address".to_string(),
        "recipient_address".to_string(),
    ).await.unwrap();
    
    assert_ne!(message_id, [0u8; 32]);

    // Test route finding
    let route = bridge.find_optimal_route(
        &ChainId::Ethereum,
        &ChainId::Bitcoin,
        1_000_000,
    ).await.unwrap();
    
    assert_eq!(route.source_chain, ChainId::Ethereum);
    assert_eq!(route.target_chain, ChainId::Bitcoin);
    assert!(!route.hops.is_empty());

    // Test liquidity aggregation
    let liquidity = bridge.aggregate_liquidity(
        &ChainId::Ethereum,
        &ChainId::Bitcoin,
    ).await.unwrap();
    
    // Should return 0 for empty pools in test
    assert_eq!(liquidity, 0);

    // Test supported chains
    let chains = bridge.get_supported_chains().await.unwrap();
    assert!(chains.contains(&ChainId::Ethereum));
    assert!(chains.contains(&ChainId::Bitcoin));
    assert!(chains.contains(&ChainId::BinanceSmartChain));
    assert!(chains.contains(&ChainId::Polygon));
}

/// Test end-to-end bridge transaction flow
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_end_to_end_bridge_flow() {
    // Create bridge configuration
    let config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: {
            let mut confirmations = HashMap::new();
            confirmations.insert(ChainId::Ethereum, 12);
            confirmations.insert(ChainId::Bitcoin, 6);
            confirmations
        },
        supported_tokens: {
            let mut tokens = HashMap::new();
            tokens.insert("CRAP".to_string(), vec![ChainId::Ethereum, ChainId::Bitcoin, ChainId::BitCraps]);
            tokens
        },
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    // Create state manager
    let state_manager = Arc::new(BridgeStateManager::new(config.clone()));
    
    // Create security manager
    use bitcraps::security::SecurityManager;
    let security_manager = Arc::new(SecurityManager::new());
    let bridge_security = Arc::new(BridgeSecurityManager::new(security_manager));
    
    // Create event monitor
    let mut event_monitor = BridgeEventMonitor::new(
        Arc::clone(&state_manager),
        Arc::clone(&bridge_security),
    );

    // Create a bridge transaction
    let tx = BridgeTransaction {
        tx_id: [1u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [2u8; 32],
        target_tx_hash: None,
        token_address: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        amount: 1000000,
        bridge_fee: 1000,
        sender: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        recipient: "bitcraps_recipient_address".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    // 1. Validate transaction
    bridge_security.validate_bridge_transaction(&tx).await.unwrap();

    // 2. Store transaction
    state_manager.store_transaction(tx.clone()).await.unwrap();

    // 3. Simulate validator signatures
    use bitcraps::crypto::BitchatKeypair;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    
    let validator_key = SigningKey::generate(&mut OsRng);
    let validator_signature = ValidatorSignature {
        validator_id: [3u8; 32],
        signature: vec![1, 2, 3, 4], // Mock signature
        timestamp: 1234567890,
        public_key: validator_key.verifying_key().to_bytes().to_vec(),
    };

    // 4. Add validator signature
    state_manager.add_validator_signature(&tx.tx_id, validator_signature).await.unwrap();

    // 5. Check transaction status
    let updated_tx = state_manager.get_transaction(&tx.tx_id).await.unwrap();
    assert_eq!(updated_tx.current_confirmations, 1);

    // 6. Add second validator signature to meet threshold
    let validator_key2 = SigningKey::generate(&mut OsRng);
    let validator_signature2 = ValidatorSignature {
        validator_id: [4u8; 32],
        signature: vec![5, 6, 7, 8], // Mock signature
        timestamp: 1234567890,
        public_key: validator_key2.verifying_key().to_bytes().to_vec(),
    };

    state_manager.add_validator_signature(&tx.tx_id, validator_signature2).await.unwrap();

    // 7. Verify transaction is ready for processing
    let final_tx = state_manager.get_transaction(&tx.tx_id).await.unwrap();
    assert_eq!(final_tx.current_confirmations, 2);
    assert_eq!(final_tx.status, BridgeTransactionStatus::ValidatorsSigned);

    // 8. Record completion event
    state_manager.record_event(BridgeEvent::TransactionCompleted {
        tx_id: tx.tx_id,
        source_chain: tx.source_chain,
        target_chain: tx.target_chain,
        amount: tx.amount,
    }).await;

    // 9. Get recent events
    let events = state_manager.get_recent_events(10).await;
    assert_eq!(events.len(), 1);
    if let BridgeEvent::TransactionCompleted { tx_id, amount, .. } = &events[0] {
        assert_eq!(*tx_id, tx.tx_id);
        assert_eq!(*amount, tx.amount);
    } else {
        panic!("Expected TransactionCompleted event");
    }
}

/// Test multi-chain routing optimization
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_multi_chain_routing() {
    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);

    // Test direct route
    let direct_route = bridge.find_optimal_route(
        &ChainId::Ethereum,
        &ChainId::BitCraps,
        1_000_000,
    ).await.unwrap();

    assert_eq!(direct_route.source_chain, ChainId::Ethereum);
    assert_eq!(direct_route.target_chain, ChainId::BitCraps);
    assert!(direct_route.total_cost > 0);
    assert!(direct_route.estimated_time > Duration::from_secs(0));

    // Test multi-hop route (when direct route is not optimal)
    let multi_hop_route = bridge.find_optimal_route(
        &ChainId::Bitcoin,
        &ChainId::Polygon,
        5_000_000,
    ).await.unwrap();

    assert_eq!(multi_hop_route.source_chain, ChainId::Bitcoin);
    assert_eq!(multi_hop_route.target_chain, ChainId::Polygon);
    
    // Route should exist even if it requires multiple hops
    assert!(!multi_hop_route.hops.is_empty());
}

/// Test bridge plugin system
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_plugin_system() {
    use async_trait::async_trait;

    // Mock bridge plugin
    struct MockBridgePlugin {
        name: String,
        version: String,
    }

    #[async_trait]
    impl BridgePlugin for MockBridgePlugin {
        fn name(&self) -> &str {
            &self.name
        }
        
        fn version(&self) -> &str {
            &self.version
        }
        
        async fn initialize(&self, _config: &PluginConfig) -> Result<()> {
            Ok(())
        }
        
        async fn handle_transaction(&self, transaction: &BridgeTransaction) -> Result<()> {
            // Mock transaction handling
            assert!(transaction.amount > 0);
            Ok(())
        }
        
        async fn supported_chains(&self) -> Vec<ChainId> {
            vec![ChainId::Ethereum, ChainId::BitCraps]
        }
    }

    let config = UniversalBridgeConfig::default();
    let bridge = UniversalBridge::new(config);

    // Create mock plugin
    let plugin = Box::new(MockBridgePlugin {
        name: "MockPlugin".to_string(),
        version: "1.0.0".to_string(),
    });

    // Register plugin
    let result = bridge.register_plugin(plugin).await;
    assert!(result.is_ok());
}

/// Test bridge error handling and edge cases
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_error_handling() {
    let config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    let state_manager = BridgeStateManager::new(config);

    // Test invalid transaction ID lookup
    let result = state_manager.get_transaction(&[255u8; 32]).await;
    assert!(result.is_none());

    // Test duplicate validator signature
    let tx = BridgeTransaction {
        tx_id: [1u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [2u8; 32],
        target_tx_hash: None,
        token_address: "0x1234".to_string(),
        amount: 1000000,
        bridge_fee: 1000,
        sender: "sender".to_string(),
        recipient: "recipient".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    state_manager.store_transaction(tx).await.unwrap();

    let signature = ValidatorSignature {
        validator_id: [3u8; 32],
        signature: vec![1, 2, 3, 4],
        timestamp: 1234567890,
        public_key: vec![1, 2, 3, 4],
    };

    // Add signature once - should succeed
    let result = state_manager.add_validator_signature(&[1u8; 32], signature.clone()).await;
    assert!(result.is_ok());

    // Add same signature again - should fail
    let result = state_manager.add_validator_signature(&[1u8; 32], signature).await;
    assert!(result.is_err());
}

/// Test bridge performance under load
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_performance() {
    let config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    let state_manager = Arc::new(BridgeStateManager::new(config));

    // Create multiple concurrent transactions
    let mut handles = Vec::new();
    
    for i in 0..100 {
        let state_manager = Arc::clone(&state_manager);
        let handle = tokio::spawn(async move {
            let tx = BridgeTransaction {
                tx_id: [i; 32],
                source_chain: ChainId::Ethereum,
                target_chain: ChainId::BitCraps,
                source_tx_hash: [(i + 1) as u8; 32],
                target_tx_hash: None,
                token_address: "0x1234".to_string(),
                amount: 1000000 + i as u64,
                bridge_fee: 1000,
                sender: "sender".to_string(),
                recipient: "recipient".to_string(),
                status: BridgeTransactionStatus::Initiated,
                required_confirmations: 2,
                current_confirmations: 0,
                validator_signatures: Vec::new(),
                created_at: 1234567890,
                updated_at: 1234567890,
                expires_at: 1234567890 + 3600,
                metadata: HashMap::new(),
            };

            state_manager.store_transaction(tx).await.unwrap();
            
            // Add some validator signatures
            let signature = ValidatorSignature {
                validator_id: [(i * 2) as u8; 32],
                signature: vec![i as u8, (i + 1) as u8, (i + 2) as u8, (i + 3) as u8],
                timestamp: 1234567890,
                public_key: vec![i as u8; 32],
            };
            
            state_manager.add_validator_signature(&[i; 32], signature).await.unwrap();
        });
        
        handles.push(handle);
    }

    // Wait for all transactions to complete with timeout
    let result = timeout(Duration::from_secs(10), async {
        for handle in handles {
            handle.await.unwrap();
        }
    }).await;

    assert!(result.is_ok(), "Bridge performance test timed out");
}

/// Test bridge emergency procedures
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_bridge_emergency_procedures() {
    let universal_config = UniversalBridgeConfig::default();
    let universal_bridge = UniversalBridge::new(universal_config);
    
    let bridge_config = BridgeConfig {
        min_amount: 1000,
        max_amount: 1000000,
        fee_percentage: 0.001,
        required_signatures: 2,
        timeout_duration: Duration::from_secs(3600),
        confirmation_requirements: HashMap::new(),
        supported_tokens: HashMap::new(),
        validator_keys: Vec::new(),
        emergency_pause: false,
    };

    // Initialize bridge
    universal_bridge.initialize(bridge_config).await.unwrap();

    // Test emergency pause
    let result = universal_bridge.emergency_pause().await;
    assert!(result.is_ok());

    // Test resume operations
    let result = universal_bridge.resume_operations().await;
    assert!(result.is_ok());
}

/// Test comprehensive fraud detection
#[cfg(feature = "bridges")]
#[tokio::test]
async fn test_fraud_detection_system() {
    use bitcraps::security::SecurityManager;
    
    let security_manager = Arc::new(SecurityManager::new());
    let bridge_security = BridgeSecurityManager::new(security_manager);

    // Add fraud detection rule
    let fraud_rule = FraudDetectionRule {
        name: "excessive_amount_rule".to_string(),
        description: "Detects transactions with suspiciously high amounts".to_string(),
        severity: 9,
        evaluate: |tx: &BridgeTransaction| -> bool {
            tx.amount < 10_000_000_000 // 10B units threshold
        },
    };

    bridge_security.add_fraud_detection_rule(
        "excessive_amount".to_string(),
        fraud_rule,
    ).await;

    // Test normal transaction (should pass)
    let normal_tx = BridgeTransaction {
        tx_id: [1u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [2u8; 32],
        target_tx_hash: None,
        token_address: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        amount: 1_000_000, // 1M units - normal
        bridge_fee: 1000,
        sender: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        recipient: "recipient".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    let result = bridge_security.validate_bridge_transaction(&normal_tx).await;
    assert!(result.is_ok());

    // Test suspicious transaction (should fail)
    let suspicious_tx = BridgeTransaction {
        tx_id: [3u8; 32],
        source_chain: ChainId::Ethereum,
        target_chain: ChainId::BitCraps,
        source_tx_hash: [4u8; 32],
        target_tx_hash: None,
        token_address: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        amount: 20_000_000_000, // 20B units - suspicious
        bridge_fee: 1000,
        sender: "0x742d35Cc6634C0532925a3b8d8A4fA6BE9d98c9a".to_string(),
        recipient: "recipient".to_string(),
        status: BridgeTransactionStatus::Initiated,
        required_confirmations: 2,
        current_confirmations: 0,
        validator_signatures: Vec::new(),
        created_at: 1234567890,
        updated_at: 1234567890,
        expires_at: 1234567890 + 3600,
        metadata: HashMap::new(),
    };

    let result = bridge_security.validate_bridge_transaction(&suspicious_tx).await;
    assert!(result.is_err());
}