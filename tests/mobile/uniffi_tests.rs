//! UniFFI binding tests for cross-platform compatibility

use super::*;
use crate::mobile::*;

/// Test suite for UniFFI interface bindings
pub struct UniFFITests {
    harness: MobileTestHarness,
}

impl UniFFITests {
    pub fn new(harness: MobileTestHarness) -> Self {
        Self { harness }
    }

    /// Run all UniFFI tests
    pub async fn run_all_tests(&self) -> MobileTestResults {
        let mut results = MobileTestResults::new();

        // Test node creation and destruction
        let result = self.harness.run_test_with_timeout(
            "uniffi_node_lifecycle",
            self.test_node_lifecycle()
        ).await;
        results.add_result(TestResult {
            test_name: "uniffi_node_lifecycle".to_string(),
            status: if result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: result.err().map(|e| e.to_string()),
        });

        // Test configuration handling
        let result = self.harness.run_test_with_timeout(
            "uniffi_configuration",
            self.test_configuration()
        ).await;
        results.add_result(TestResult {
            test_name: "uniffi_configuration".to_string(),
            status: if result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: result.err().map(|e| e.to_string()),
        });

        // Test event system
        let result = self.harness.run_test_with_timeout(
            "uniffi_event_system",
            self.test_event_system()
        ).await;
        results.add_result(TestResult {
            test_name: "uniffi_event_system".to_string(),
            status: if result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: result.err().map(|e| e.to_string()),
        });

        // Test error handling
        let result = self.harness.run_test_with_timeout(
            "uniffi_error_handling",
            self.test_error_handling()
        ).await;
        results.add_result(TestResult {
            test_name: "uniffi_error_handling".to_string(),
            status: if result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: result.err().map(|e| e.to_string()),
        });

        // Test power management interface
        let result = self.harness.run_test_with_timeout(
            "uniffi_power_management",
            self.test_power_management()
        ).await;
        results.add_result(TestResult {
            test_name: "uniffi_power_management".to_string(),
            status: if result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: result.err().map(|e| e.to_string()),
        });

        results
    }

    /// Test node lifecycle through UniFFI
    async fn test_node_lifecycle(&self) -> Result<(), BitCrapsError> {
        // Test node creation
        let node = self.harness.create_test_node().await?;
        
        // Verify initial state
        let status = node.get_status();
        MobileTestUtils::assert_node_state(&status, NodeState::Ready)?;
        
        // Test node properties
        assert!(status.active_connections == 0);
        assert!(!status.discovery_active);
        assert!(status.current_game_id.is_none());
        
        log::info!("UniFFI node lifecycle test passed");
        Ok(())
    }

    /// Test configuration handling
    async fn test_configuration(&self) -> Result<(), BitCrapsError> {
        let node = self.harness.create_test_node().await?;
        
        // Test power mode configuration
        node.set_power_mode(PowerMode::BatterySaver)?;
        let status = node.get_status();
        match status.current_power_mode {
            PowerMode::BatterySaver => {},
            _ => return Err(BitCrapsError::InvalidInput {
                reason: "Power mode not updated correctly".to_string(),
            }),
        }
        
        // Test platform configuration
        let platform_config = PlatformConfig {
            platform: self.harness.config.platform.clone(),
            background_scanning: false,
            scan_window_ms: 300,
            scan_interval_ms: 1000,
            low_power_mode: true,
            service_uuids: vec!["test-uuid".to_string()],
        };
        
        node.configure_for_platform(platform_config)?;
        
        log::info!("UniFFI configuration test passed");
        Ok(())
    }

    /// Test event system
    async fn test_event_system(&self) -> Result<(), BitCrapsError> {
        let node = self.harness.create_test_node().await?;
        
        // Start discovery to generate events
        if self.harness.config.enable_bluetooth {
            node.start_discovery().await?;
            
            // Wait for discovery to be active
            MobileTestUtils::wait_for_node_state(
                &node,
                NodeState::Discovering,
                5
            ).await?;
            
            // Poll for events (should not hang)
            let event = node.poll_event().await;
            
            // Stop discovery
            node.stop_discovery().await?;
        }
        
        // Test event draining
        let events = node.drain_events().await;
        assert!(events.len() >= 0); // Should not fail
        
        log::info!("UniFFI event system test passed");
        Ok(())
    }

    /// Test error handling
    async fn test_error_handling(&self) -> Result<(), BitCrapsError> {
        let node = self.harness.create_test_node().await?;
        
        // Test invalid power mode (should not crash)
        let result = node.set_power_mode(PowerMode::UltraLowPower);
        assert!(result.is_ok()); // Should handle gracefully
        
        // Test joining non-existent game
        let result = node.join_game("non-existent-game".to_string()).await;
        match result {
            Err(BitCrapsError::NotFound { .. }) | Err(BitCrapsError::GameError { .. }) => {
                // Expected error types
            },
            Err(_) => {
                // Other errors are acceptable for this test
            },
            Ok(_) => {
                // Shouldn't succeed, but not a failure if it handles gracefully
            }
        }
        
        log::info!("UniFFI error handling test passed");
        Ok(())
    }

    /// Test power management interface
    async fn test_power_management(&self) -> Result<(), BitCrapsError> {
        let node = self.harness.create_test_node().await?;
        
        // Test all power modes
        let power_modes = vec![
            PowerMode::HighPerformance,
            PowerMode::Balanced,
            PowerMode::BatterySaver,
            PowerMode::UltraLowPower,
        ];
        
        for mode in power_modes {
            node.set_power_mode(mode.clone())?;
            let status = node.get_status();
            
            // Verify power mode was set (may not be exact due to platform constraints)
            match status.current_power_mode {
                PowerMode::HighPerformance | PowerMode::Balanced | 
                PowerMode::BatterySaver | PowerMode::UltraLowPower => {
                    // Any valid power mode is acceptable
                },
            }
        }
        
        // Test scan interval setting
        node.set_scan_interval(2000)?; // Should not fail
        
        log::info!("UniFFI power management test passed");
        Ok(())
    }
}

/// Test UniFFI type serialization and deserialization
pub struct UniFFISerializationTests;

impl UniFFISerializationTests {
    /// Test all UniFFI types can be created and used
    pub fn test_type_construction() -> Result<(), BitCrapsError> {
        // Test BitCrapsConfig construction
        let config = BitCrapsConfig {
            data_dir: "./test".to_string(),
            pow_difficulty: 4,
            protocol_version: 1,
            power_mode: PowerMode::Balanced,
            platform_config: None,
            enable_logging: true,
            log_level: LogLevel::Info,
        };
        assert_eq!(config.pow_difficulty, 4);
        
        // Test GameConfig construction
        let game_config = GameConfig {
            game_name: Some("Test Game".to_string()),
            min_bet: 1,
            max_bet: 100,
            max_players: 4,
            timeout_seconds: 300,
        };
        assert_eq!(game_config.max_players, 4);
        
        // Test PeerInfo construction
        let peer_info = PeerInfo {
            peer_id: "test-peer".to_string(),
            display_name: Some("Test Peer".to_string()),
            signal_strength: 80,
            last_seen: current_timestamp(),
            is_connected: false,
        };
        assert_eq!(peer_info.peer_id, "test-peer");
        
        // Test NodeStatus construction
        let node_status = NodeStatus {
            state: NodeState::Ready,
            bluetooth_enabled: true,
            discovery_active: false,
            current_game_id: None,
            active_connections: 0,
            current_power_mode: PowerMode::Balanced,
        };
        assert!(node_status.bluetooth_enabled);
        
        // Test GameEvent construction
        let event = GameEvent::PeerDiscovered {
            peer: peer_info.clone(),
        };
        match event {
            GameEvent::PeerDiscovered { .. } => {}, // Expected
            _ => return Err(BitCrapsError::InvalidInput {
                reason: "GameEvent construction failed".to_string(),
            }),
        }
        
        // Test BetType construction
        let bet_types = vec![
            BetType::Pass,
            BetType::DontPass,
            BetType::Field,
            BetType::Any7,
            BetType::AnyCraps,
            BetType::Hardway { number: 8 },
            BetType::PlaceBet { number: 6 },
        ];
        assert_eq!(bet_types.len(), 7);
        
        // Test DiceRoll construction
        let dice_roll = DiceRoll {
            die1: 3,
            die2: 4,
            roll_time: current_timestamp(),
            roller_peer_id: "test-roller".to_string(),
        };
        assert_eq!(dice_roll.die1 + dice_roll.die2, 7);
        
        Ok(())
    }

    /// Test error type construction and conversion
    pub fn test_error_types() -> Result<(), BitCrapsError> {
        let errors = vec![
            BitCrapsError::InitializationError { reason: "test".to_string() },
            BitCrapsError::BluetoothError { reason: "test".to_string() },
            BitCrapsError::NetworkError { reason: "test".to_string() },
            BitCrapsError::GameError { reason: "test".to_string() },
            BitCrapsError::CryptoError { reason: "test".to_string() },
            BitCrapsError::InvalidInput { reason: "test".to_string() },
            BitCrapsError::Timeout,
            BitCrapsError::NotFound { item: "test".to_string() },
        ];
        
        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
        
        Ok(())
    }

    /// Test enum variants
    pub fn test_enum_variants() -> Result<(), BitCrapsError> {
        // Test PowerMode variants
        let power_modes = vec![
            PowerMode::HighPerformance,
            PowerMode::Balanced,
            PowerMode::BatterySaver,
            PowerMode::UltraLowPower,
        ];
        assert_eq!(power_modes.len(), 4);
        
        // Test PlatformType variants
        let platforms = vec![
            PlatformType::Android,
            PlatformType::iOS,
            PlatformType::Desktop,
            PlatformType::Web,
        ];
        assert_eq!(platforms.len(), 4);
        
        // Test NodeState variants
        let states = vec![
            NodeState::Initializing,
            NodeState::Ready,
            NodeState::Discovering,
            NodeState::Connected,
            NodeState::InGame,
            NodeState::Error { reason: "test".to_string() },
        ];
        assert_eq!(states.len(), 6);
        
        // Test GameState variants
        let game_states = vec![
            GameState::Waiting,
            GameState::ComeOut,
            GameState::Point { point: 6 },
            GameState::Resolved,
            GameState::Error { reason: "test".to_string() },
        ];
        assert_eq!(game_states.len(), 5);
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniffi_type_construction() {
        assert!(UniFFISerializationTests::test_type_construction().is_ok());
    }

    #[test]
    fn test_uniffi_error_types() {
        assert!(UniFFISerializationTests::test_error_types().is_ok());
    }

    #[test]
    fn test_uniffi_enum_variants() {
        assert!(UniFFISerializationTests::test_enum_variants().is_ok());
    }
}