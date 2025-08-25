#[cfg(test)]
mod cross_platform_interoperability_tests {
    use super::*;
    use crate::common::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::sync::broadcast;
    use serde_json::Value;
    
    /// Comprehensive cross-platform interoperability tests
    /// Tests Android ↔ iOS communication, protocol compatibility, and edge cases
    
    #[tokio::test]
    async fn test_android_ios_basic_discovery() {
        let test_context = setup_cross_platform_test().await;
        
        // Simulate Android device
        let android_node = create_mock_android_node("android_001").await;
        
        // Simulate iOS device  
        let ios_node = create_mock_ios_node("ios_001").await;
        
        // Test peer discovery
        let discovery_result = test_peer_discovery(&android_node, &ios_node).await;
        assert!(discovery_result.is_ok(), "Android should discover iOS peer");
        
        let reverse_discovery = test_peer_discovery(&ios_node, &android_node).await;
        assert!(reverse_discovery.is_ok(), "iOS should discover Android peer");
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_cross_platform_game_session() {
        let test_context = setup_cross_platform_test().await;
        
        // Create mixed platform game session
        let android_host = create_mock_android_node("android_host").await;
        let ios_player1 = create_mock_ios_node("ios_player1").await;
        let ios_player2 = create_mock_ios_node("ios_player2").await;
        let android_player = create_mock_android_node("android_player").await;
        
        // Android creates game
        let game_session = android_host.create_game(GameConfig {
            max_players: 4,
            game_type: GameType::Craps,
            betting_limits: BettingLimits::default(),
            timeout_settings: TimeoutSettings::default(),
        }).await?;
        
        // iOS players join
        ios_player1.join_game(&game_session.id).await?;
        ios_player2.join_game(&game_session.id).await?;
        android_player.join_game(&game_session.id).await?;
        
        // Verify all players are connected
        let final_state = game_session.get_state().await?;
        assert_eq!(final_state.players.len(), 4);
        
        // Test cross-platform game mechanics
        test_cross_platform_dice_roll(&game_session, &android_host, &ios_player1).await?;
        test_cross_platform_betting(&game_session, &ios_player1, &android_player).await?;
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_protocol_version_compatibility() {
        let test_context = setup_cross_platform_test().await;
        
        // Test different protocol versions
        let android_v1 = create_mock_android_node_with_version("android_v1", ProtocolVersion::V1).await;
        let ios_v2 = create_mock_ios_node_with_version("ios_v2", ProtocolVersion::V2).await;
        let android_v2 = create_mock_android_node_with_version("android_v2", ProtocolVersion::V2).await;
        
        // Test backward compatibility
        let compat_result = test_protocol_compatibility(&android_v1, &ios_v2).await;
        assert!(compat_result.is_compatible, "V1 should be compatible with V2");
        
        // Test same version optimal performance
        let optimal_result = test_protocol_compatibility(&ios_v2, &android_v2).await;
        assert!(optimal_result.is_optimal, "Same versions should have optimal performance");
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_network_partition_recovery() {
        let test_context = setup_cross_platform_test().await;
        
        // Setup cross-platform game
        let android_host = create_mock_android_node("partition_android").await;
        let ios_player = create_mock_ios_node("partition_ios").await;
        
        let game_session = android_host.create_game(GameConfig::default()).await?;
        ios_player.join_game(&game_session.id).await?;
        
        // Simulate network partition
        simulate_network_partition(&android_host, &ios_player, Duration::from_secs(30)).await;
        
        // Test state during partition
        let android_state = android_host.get_game_state(&game_session.id).await?;
        let ios_state = ios_player.get_game_state(&game_session.id).await?;
        
        // States should diverge during partition
        assert_ne!(android_state.last_update, ios_state.last_update);
        
        // Restore network
        restore_network_connection(&android_host, &ios_player).await;
        
        // Wait for reconciliation
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Verify state convergence
        let final_android_state = android_host.get_game_state(&game_session.id).await?;
        let final_ios_state = ios_player.get_game_state(&game_session.id).await?;
        
        assert_eq!(final_android_state.consensus_hash(), final_ios_state.consensus_hash(),
                   "States should converge after partition recovery");
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_concurrent_operations_across_platforms() {
        let test_context = setup_cross_platform_test().await;
        
        let android_nodes: Vec<MockAndroidNode> = (0..3).map(|i| 
            create_mock_android_node(&format!("android_{}", i))
        ).collect::<Result<Vec<_>, _>>().await?;
        
        let ios_nodes: Vec<MockIOSNode> = (0..3).map(|i|
            create_mock_ios_node(&format!("ios_{}", i))
        ).collect::<Result<Vec<_>, _>>().await?;
        
        // Create game session
        let game_session = android_nodes[0].create_game(GameConfig::default()).await?;
        
        // All nodes join
        for node in &android_nodes[1..] {
            node.join_game(&game_session.id).await?;
        }
        for node in &ios_nodes {
            node.join_game(&game_session.id).await?;
        }
        
        // Perform concurrent operations
        let operations = vec![
            // Android nodes place bets
            tokio::spawn({
                let node = android_nodes[1].clone();
                let session_id = game_session.id.clone();
                async move {
                    node.place_bet(&session_id, BetType::PassLine, 25).await
                }
            }),
            
            // iOS nodes place different bets
            tokio::spawn({
                let node = ios_nodes[0].clone();
                let session_id = game_session.id.clone();
                async move {
                    node.place_bet(&session_id, BetType::DontPass, 50).await
                }
            }),
            
            // Simultaneous dice roll attempts
            tokio::spawn({
                let node = android_nodes[0].clone();
                let session_id = game_session.id.clone();
                async move {
                    node.roll_dice(&session_id).await
                }
            }),
            
            tokio::spawn({
                let node = ios_nodes[1].clone();
                let session_id = game_session.id.clone();
                async move {
                    node.roll_dice(&session_id).await
                }
            }),
        ];
        
        let results = futures::future::join_all(operations).await;
        
        // Verify conflict resolution
        let final_state = game_session.get_state().await?;
        assert!(final_state.is_consistent(), "Final state should be consistent");
        
        // Only one dice roll should have succeeded
        let roll_successes = results.into_iter()
            .filter(|r| matches!(r, Ok(Ok(OperationResult::DiceRoll(_)))))
            .count();
        assert_eq!(roll_successes, 1, "Only one dice roll should succeed");
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test] 
    async fn test_platform_specific_features() {
        let test_context = setup_cross_platform_test().await;
        
        // Test Android-specific features
        let android_node = create_mock_android_node("feature_android").await;
        
        // Test haptic feedback (Android-specific)
        let haptic_result = android_node.test_haptic_feedback().await;
        assert!(haptic_result.is_ok(), "Android haptic feedback should work");
        
        // Test Material Design animations
        let material_animation = android_node.test_material_animations().await;
        assert!(material_animation.frame_rate >= 60, "Should maintain 60fps");
        
        // Test iOS-specific features
        let ios_node = create_mock_ios_node("feature_ios").await;
        
        // Test Haptic Engine (iOS-specific)
        let ios_haptic_result = ios_node.test_haptic_engine().await;
        assert!(ios_haptic_result.is_ok(), "iOS Haptic Engine should work");
        
        // Test Metal rendering
        let metal_performance = ios_node.test_metal_rendering().await;
        assert!(metal_performance.frame_rate >= 60, "Metal should maintain 60fps");
        
        // Test cross-platform feature parity
        let game_session = android_node.create_game(GameConfig::default()).await?;
        ios_node.join_game(&game_session.id).await?;
        
        // Both should support core game features equally
        let android_features = android_node.get_supported_features().await;
        let ios_features = ios_node.get_supported_features().await;
        
        let core_features = [
            Feature::DiceRoll,
            Feature::PlaceBets,
            Feature::ChatMessaging,
            Feature::GameStatistics,
        ];
        
        for feature in core_features {
            assert!(android_features.contains(&feature), "Android missing core feature: {:?}", feature);
            assert!(ios_features.contains(&feature), "iOS missing core feature: {:?}", feature);
        }
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_battery_optimization_coordination() {
        let test_context = setup_cross_platform_test().await;
        
        let android_node = create_mock_android_node("battery_android").await;
        let ios_node = create_mock_ios_node("battery_ios").await;
        
        let game_session = android_node.create_game(GameConfig::default()).await?;
        ios_node.join_game(&game_session.id).await?;
        
        // Simulate low battery on Android
        android_node.simulate_battery_level(15).await; // 15%
        
        // Android should automatically reduce performance
        let android_perf = android_node.get_performance_state().await;
        assert!(android_perf.frame_rate <= 30, "Android should reduce frame rate on low battery");
        
        // iOS should adapt to slower peer
        tokio::time::sleep(Duration::from_secs(5)).await;
        let ios_adaptation = ios_node.get_network_adaptation().await;
        assert!(ios_adaptation.adapted_to_slow_peer, "iOS should adapt to slower Android peer");
        
        // Test game continues to function
        let dice_result = android_node.roll_dice(&game_session.id).await?;
        assert!(dice_result.is_valid(), "Game should continue with battery optimization");
        
        // Restore battery and verify performance recovery
        android_node.simulate_battery_level(80).await; // 80%
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        let recovered_perf = android_node.get_performance_state().await;
        assert!(recovered_perf.frame_rate >= 60, "Performance should recover");
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_platform_reconnection_scenarios() {
        let test_context = setup_cross_platform_test().await;
        
        let android_node = create_mock_android_node("reconnect_android").await;
        let ios_node = create_mock_ios_node("reconnect_ios").await;
        
        let game_session = android_node.create_game(GameConfig::default()).await?;
        ios_node.join_game(&game_session.id).await?;
        
        // Test Android backgrounding (common scenario)
        android_node.simulate_background_transition().await;
        
        // iOS continues game
        let ios_roll = ios_node.roll_dice(&game_session.id).await?;
        assert!(ios_roll.is_valid());
        
        // Android comes back to foreground
        android_node.simulate_foreground_transition().await;
        
        // Android should sync game state
        let synced_state = android_node.get_game_state(&game_session.id).await?;
        assert_eq!(synced_state.last_dice_roll, Some(ios_roll.result));
        
        // Test iOS app switching (iOS-specific behavior)
        ios_node.simulate_app_switch().await;
        
        // Android continues
        let android_roll = android_node.roll_dice(&game_session.id).await?;
        
        // iOS returns
        ios_node.simulate_app_return().await;
        
        // Verify iOS caught up
        let ios_synced_state = ios_node.get_game_state(&game_session.id).await?;
        assert_eq!(ios_synced_state.last_dice_roll, Some(android_roll.result));
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_performance_parity_validation() {
        let test_context = setup_cross_platform_test().await;
        
        let android_node = create_mock_android_node("perf_android").await;
        let ios_node = create_mock_ios_node("perf_ios").await;
        
        // Test operation latencies
        let android_latencies = measure_operation_latencies(&android_node).await;
        let ios_latencies = measure_operation_latencies(&ios_node).await;
        
        // Dice roll should be fast on both platforms
        assert!(android_latencies.dice_roll < Duration::from_millis(100));
        assert!(ios_latencies.dice_roll < Duration::from_millis(100));
        
        // Betting should be instantaneous
        assert!(android_latencies.place_bet < Duration::from_millis(50));
        assert!(ios_latencies.place_bet < Duration::from_millis(50));
        
        // Network operations should be similar (within 20% variance)
        let network_variance = calculate_variance(android_latencies.network_sync, ios_latencies.network_sync);
        assert!(network_variance < 0.2, "Network sync should have similar performance: Android={}ms, iOS={}ms", 
                android_latencies.network_sync.as_millis(), ios_latencies.network_sync.as_millis());
        
        // Memory usage should be reasonable on both
        let android_memory = android_node.get_memory_usage().await;
        let ios_memory = ios_node.get_memory_usage().await;
        
        assert!(android_memory.heap_mb < 150.0, "Android memory usage: {}MB", android_memory.heap_mb);
        assert!(ios_memory.heap_mb < 100.0, "iOS memory usage: {}MB", ios_memory.heap_mb);
        
        cleanup_test_context(test_context).await;
    }
    
    #[tokio::test]
    async fn test_edge_case_scenarios() {
        let test_context = setup_cross_platform_test().await;
        
        // Test rapid connect/disconnect
        let android_node = create_mock_android_node("edge_android").await;
        let ios_node = create_mock_ios_node("edge_ios").await;
        
        for i in 0..5 {
            let game_session = android_node.create_game(GameConfig::default()).await?;
            ios_node.join_game(&game_session.id).await?;
            
            // Rapid disconnect
            ios_node.leave_game(&game_session.id).await?;
            android_node.end_game(&game_session.id).await?;
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Test simultaneous game creation
        let (android_game_result, ios_game_result) = tokio::join!(
            android_node.create_game(GameConfig::default()),
            ios_node.create_game(GameConfig::default())
        );
        
        assert!(android_game_result.is_ok() || ios_game_result.is_ok(), 
                "At least one game creation should succeed");
        
        // Test invalid state transitions
        let game_session = android_node.create_game(GameConfig::default()).await?;
        let invalid_roll = ios_node.roll_dice(&game_session.id).await;
        assert!(invalid_roll.is_err(), "Should not be able to roll dice without joining game");
        
        cleanup_test_context(test_context).await;
    }
    
    // Helper functions
    
    async fn setup_cross_platform_test() -> TestContext {
        TestContext::new().await
    }
    
    async fn create_mock_android_node(id: &str) -> Result<MockAndroidNode, TestError> {
        MockAndroidNode::new(id, PlatformConfig {
            platform: Platform::Android,
            version: "14.0".to_string(),
            device_capabilities: DeviceCapabilities::android_default(),
        }).await
    }
    
    async fn create_mock_ios_node(id: &str) -> Result<MockIOSNode, TestError> {
        MockIOSNode::new(id, PlatformConfig {
            platform: Platform::iOS,
            version: "17.0".to_string(),
            device_capabilities: DeviceCapabilities::ios_default(),
        }).await
    }
    
    async fn test_peer_discovery(discoverer: &dyn GameNode, target: &dyn GameNode) -> Result<DiscoveryResult, TestError> {
        let discovery_timeout = Duration::from_secs(30);
        let start_time = Instant::now();
        
        discoverer.start_discovery().await?;
        
        while start_time.elapsed() < discovery_timeout {
            let discovered_peers = discoverer.get_discovered_peers().await?;
            if discovered_peers.iter().any(|p| p.id == target.id()) {
                return Ok(DiscoveryResult {
                    success: true,
                    time_taken: start_time.elapsed(),
                    peer_info: Some(target.get_peer_info().await?),
                });
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Err(TestError::DiscoveryTimeout)
    }
    
    async fn test_cross_platform_dice_roll(
        game_session: &GameSession, 
        roller: &dyn GameNode, 
        observer: &dyn GameNode
    ) -> Result<(), TestError> {
        let initial_state = game_session.get_state().await?;
        let dice_result = roller.roll_dice(&game_session.id).await?;
        
        // Wait for state propagation
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let observer_state = observer.get_game_state(&game_session.id).await?;
        assert_eq!(observer_state.last_dice_roll, Some(dice_result.result));
        
        Ok(())
    }
    
    async fn test_cross_platform_betting(
        game_session: &GameSession,
        bettor: &dyn GameNode,
        observer: &dyn GameNode
    ) -> Result<(), TestError> {
        let initial_pot = game_session.get_state().await?.total_pot;
        let bet_amount = 25;
        
        bettor.place_bet(&game_session.id, BetType::PassLine, bet_amount).await?;
        
        // Wait for state propagation  
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        let final_pot = observer.get_game_state(&game_session.id).await?.total_pot;
        assert_eq!(final_pot, initial_pot + bet_amount);
        
        Ok(())
    }
    
    async fn measure_operation_latencies(node: &dyn GameNode) -> OperationLatencies {
        let dice_roll_start = Instant::now();
        let game_session = node.create_game(GameConfig::default()).await.unwrap();
        node.roll_dice(&game_session.id).await.unwrap();
        let dice_roll_time = dice_roll_start.elapsed();
        
        let bet_start = Instant::now();
        node.place_bet(&game_session.id, BetType::PassLine, 25).await.unwrap();
        let bet_time = bet_start.elapsed();
        
        let sync_start = Instant::now();
        node.sync_game_state(&game_session.id).await.unwrap();
        let sync_time = sync_start.elapsed();
        
        OperationLatencies {
            dice_roll: dice_roll_time,
            place_bet: bet_time,
            network_sync: sync_time,
        }
    }
    
    fn calculate_variance(duration1: Duration, duration2: Duration) -> f64 {
        let d1 = duration1.as_millis() as f64;
        let d2 = duration2.as_millis() as f64;
        let mean = (d1 + d2) / 2.0;
        let variance = ((d1 - mean).abs() + (d2 - mean).abs()) / 2.0;
        variance / mean
    }
    
    async fn cleanup_test_context(context: TestContext) {
        context.cleanup().await;
    }
    
    // Mock types and traits would be implemented here
    // This is a representative structure of comprehensive cross-platform testing
}

// Supporting types and structures
#[derive(Debug)]
struct TestContext {
    // Test infrastructure context
}

impl TestContext {
    async fn new() -> Self {
        TestContext {}
    }
    
    async fn cleanup(self) {
        // Cleanup test resources
    }
}

#[derive(Debug)]
struct MockAndroidNode {
    id: String,
    config: PlatformConfig,
    // Android-specific mock implementations
}

#[derive(Debug)]  
struct MockIOSNode {
    id: String,
    config: PlatformConfig, 
    // iOS-specific mock implementations
}

// Additional supporting types would be defined here...
struct DiscoveryResult {
    success: bool,
    time_taken: Duration,
    peer_info: Option<PeerInfo>,
}

struct OperationLatencies {
    dice_roll: Duration,
    place_bet: Duration,
    network_sync: Duration,
}

#[derive(Debug)]
enum TestError {
    DiscoveryTimeout,
    NetworkError,
    StateInconsistency,
    ProtocolMismatch,
}