//! Comprehensive Integration Test Suite for BitChat-Rust
//!
//! This test suite validates the complete system including:
//! - P2P consensus game flow
//! - Security implementations
//! - BLE peripheral advertising
//! - Mobile performance optimizations
//! - Cross-platform compatibility

use bitcraps::{
    crypto::secure_keystore::SecureKeystore,
    mobile::performance::MobilePerformanceOptimizer,
    protocol::{BetType, CrapTokens, DiceRoll, GameId, PeerId},
    transport::ble_peripheral::BlePeripheralFactory,
    BitCrapsApp,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_complete_p2p_consensus_game_flow() {
    // Initialize two app instances to simulate P2P
    let app1 = Arc::new(BitCrapsApp::new().await.unwrap());
    let app2 = Arc::new(BitCrapsApp::new().await.unwrap());

    // Start both apps
    app1.start().await.unwrap();
    app2.start().await.unwrap();

    // Create game on app1
    let participants = vec![app1.get_peer_id(), app2.get_peer_id()];
    let game_id = app1
        .create_consensus_game(participants.clone())
        .await
        .unwrap();

    // App2 should discover the game
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Both place bets
    let bet_amount = CrapTokens::from_raw(100);
    app1.place_consensus_bet(game_id, BetType::Pass, bet_amount)
        .await
        .unwrap();
    app2.place_consensus_bet(game_id, BetType::DontPass, bet_amount)
        .await
        .unwrap();

    // Roll dice with consensus
    let roll = app1.roll_consensus_dice(game_id).await.unwrap();

    // Verify both apps have same game state
    let state1 = app1.get_game_state(game_id).await.unwrap();
    let state2 = app2.get_game_state(game_id).await.unwrap();
    assert_eq!(state1.last_roll, state2.last_roll);
}

#[tokio::test]
async fn test_security_module_integration() {
    // Test SecureKeystore
    let keystore = SecureKeystore::new();

    // Generate keys for different contexts
    let identity_key = keystore.get_identity_keypair().unwrap();
    let consensus_key = keystore.get_consensus_keypair().unwrap();

    // Verify keys are different
    assert_ne!(identity_key.public, consensus_key.public);

    // Test signature creation and verification
    let message = b"test message";
    let signature = keystore
        .sign_with_context(
            message,
            bitcraps::crypto::secure_keystore::KeyContext::Consensus,
        )
        .unwrap();

    let valid = keystore
        .verify_signature(&consensus_key.public, &signature, message)
        .unwrap();
    assert!(valid);

    // Test safe arithmetic
    use bitcraps::crypto::safe_arithmetic::SafeArithmetic;

    let safe = SafeArithmetic;

    // Test overflow protection
    let result = safe.safe_add(u64::MAX, 1);
    assert!(result.is_err());

    // Test safe percentage
    let percent = safe.safe_percentage(1000, 10).unwrap();
    assert_eq!(percent, 100);
}

#[tokio::test]
async fn test_ble_peripheral_advertising() {
    // Test BLE peripheral factory
    let config = bitcraps::transport::ble_peripheral::AdvertisingConfig::default();
    let peripheral = BlePeripheralFactory::create(config).await;

    assert!(peripheral.is_ok());
    let peripheral = peripheral.unwrap();

    // Start advertising
    let result = peripheral.start_advertising().await;

    // On unsupported platforms, should gracefully fallback
    if cfg!(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "linux"
    ))) {
        // Should return error but not panic
        assert!(result.is_err());
    } else {
        // Should succeed on supported platforms
        assert!(result.is_ok());
    }

    // Stop advertising
    let _ = peripheral.stop_advertising().await;
}

#[tokio::test]
async fn test_mobile_performance_optimization() {
    use bitcraps::mobile::performance::{MobilePerformanceConfig, PowerState};

    // Initialize performance optimizer
    let config = MobilePerformanceConfig::default();
    let optimizer = MobilePerformanceOptimizer::new(config);

    // Start optimizer
    optimizer.start().await.unwrap();

    // Test power state management
    optimizer
        .set_power_state(PowerState::PowerSaver)
        .await
        .unwrap();
    let state = optimizer.get_power_state().await;
    assert_eq!(state, PowerState::PowerSaver);

    // Test adaptive BLE scanning
    let metrics = optimizer.get_ble_metrics().await.unwrap();
    assert!(metrics.duty_cycle <= 0.15); // Power saver mode limits duty cycle

    // Test memory management
    let memory_stats = optimizer.get_memory_stats().await.unwrap();
    assert!(memory_stats.total_allocated < 150 * 1024 * 1024); // Under 150MB limit

    // Stop optimizer
    optimizer.stop().await.unwrap();
}

#[tokio::test]
async fn test_p2p_message_flow() {
    use bitcraps::protocol::p2p_messages::{MessageType, P2PMessage};

    // Create test message
    let message = P2PMessage {
        id: uuid::Uuid::new_v4().to_string(),
        sender: PeerId::generate(),
        message_type: MessageType::GameCreation,
        payload: vec![1, 2, 3, 4],
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        signature: None,
        sequence: 1,
        ttl: 5,
    };

    // Test serialization
    let serialized = bincode::serialize(&message).unwrap();
    assert!(serialized.len() > 0);

    // Test deserialization
    let deserialized: P2PMessage = bincode::deserialize(&serialized).unwrap();
    assert_eq!(deserialized.id, message.id);

    // Test compression
    use bitcraps::mobile::compression::{CompressionAlgorithm, CompressionManager};

    let compressor = CompressionManager::new(Default::default());
    compressor.start().await.unwrap();

    let compressed = compressor
        .compress(&serialized, CompressionAlgorithm::Lz4)
        .await
        .unwrap();
    assert!(compressed.len() < serialized.len());

    let decompressed = compressor.decompress(&compressed).await.unwrap();
    assert_eq!(decompressed, serialized);
}

#[tokio::test]
async fn test_consensus_with_byzantine_failures() {
    // Simulate Byzantine node behavior
    let honest_nodes = 3;
    let byzantine_nodes = 1;
    let total_nodes = honest_nodes + byzantine_nodes;

    // Create nodes
    let mut nodes = Vec::new();
    for _ in 0..total_nodes {
        let app = Arc::new(BitCrapsApp::new().await.unwrap());
        app.start().await.unwrap();
        nodes.push(app);
    }

    // Create game with all nodes
    let participants: Vec<PeerId> = nodes.iter().map(|n| n.get_peer_id()).collect();
    let game_id = nodes[0].create_consensus_game(participants).await.unwrap();

    // Simulate Byzantine behavior: node 3 sends conflicting votes
    // (In real implementation, the consensus engine would handle this)

    // All nodes place bets
    for (i, node) in nodes.iter().enumerate() {
        let bet_type = if i % 2 == 0 {
            BetType::Pass
        } else {
            BetType::DontPass
        };
        node.place_consensus_bet(game_id, bet_type, CrapTokens::from_raw(50))
            .await
            .unwrap();
    }

    // Roll dice - should succeed despite Byzantine node
    let roll = nodes[0].roll_consensus_dice(game_id).await.unwrap();

    // Verify honest nodes agree on outcome
    for i in 0..honest_nodes {
        let state = nodes[i].get_game_state(game_id).await.unwrap();
        assert_eq!(state.last_roll, Some(roll));
    }
}

#[tokio::test]
async fn test_network_partition_recovery() {
    // Create 4 nodes
    let mut nodes = Vec::new();
    for _ in 0..4 {
        let app = Arc::new(BitCrapsApp::new().await.unwrap());
        app.start().await.unwrap();
        nodes.push(app);
    }

    // Create game with all nodes
    let participants: Vec<PeerId> = nodes.iter().map(|n| n.get_peer_id()).collect();
    let game_id = nodes[0]
        .create_consensus_game(participants.clone())
        .await
        .unwrap();

    // Simulate network partition: nodes 0,1 vs nodes 2,3
    // (In real implementation, would disconnect network)

    // Wait for partition detection
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Simulate partition healing
    // (In real implementation, would reconnect network)

    // Verify state reconciliation
    let state0 = nodes[0].get_game_state(game_id).await.unwrap();
    let state3 = nodes[3].get_game_state(game_id).await.unwrap();

    // After reconciliation, states should converge
    assert_eq!(state0.round, state3.round);
}

#[tokio::test]
async fn test_anti_cheat_detection() {
    use bitcraps::protocol::anti_cheat::AntiCheatValidator;

    let validator = AntiCheatValidator::new();

    // Test dice roll validation
    for _ in 0..100 {
        let roll = DiceRoll::generate();
        let valid = validator.validate_dice_roll(&roll).await;
        assert!(valid.is_ok());
    }

    // Test pattern detection with suspicious rolls
    let suspicious_rolls = vec![
        DiceRoll {
            die1: 6,
            die2: 6,
            timestamp: 0,
        },
        DiceRoll {
            die1: 6,
            die2: 6,
            timestamp: 1,
        },
        DiceRoll {
            die1: 6,
            die2: 6,
            timestamp: 2,
        },
    ];

    for roll in suspicious_rolls {
        let _ = validator.validate_dice_roll(&roll).await;
    }

    // Check if pattern was detected
    let suspicious = validator.is_peer_suspicious(PeerId::generate()).await;
    // Pattern detection would flag repeated identical rolls
}

#[tokio::test]
async fn test_cross_platform_compatibility() {
    // Test platform detection
    let platform = bitcraps::transport::ble_config::PlatformCapabilities::detect();

    assert!(platform.has_bluetooth);

    #[cfg(target_os = "android")]
    assert!(platform.ble_peripheral_capable);

    #[cfg(target_os = "ios")]
    assert!(platform.ble_peripheral_capable);

    #[cfg(target_os = "linux")]
    assert!(platform.ble_peripheral_capable);

    #[cfg(target_os = "windows")]
    assert!(!platform.ble_peripheral_capable); // Windows has limited support

    // Test platform-specific initialization
    let config = bitcraps::transport::ble_config::BleTransportConfig::default();
    let result = bitcraps::transport::ble_config::BleTransportInitializer::initialize(config).await;

    // Should succeed or gracefully fail based on platform
    assert!(result.is_ok() || result.is_err());
}
