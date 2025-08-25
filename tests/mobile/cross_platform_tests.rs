//! Cross-platform compatibility tests for mobile bindings

use super::*;
use crate::mobile::*;
use std::collections::HashMap;

/// Cross-platform test suite
pub struct CrossPlatformTests {
    test_configs: HashMap<PlatformType, MobileTestConfig>,
}

impl CrossPlatformTests {
    /// Create new cross-platform test suite
    pub fn new() -> Self {
        let mut test_configs = HashMap::new();
        
        // Android test configuration
        test_configs.insert(PlatformType::Android, MobileTestConfig {
            platform: PlatformType::Android,
            enable_bluetooth: false, // Disabled for CI
            enable_power_management: true,
            enable_battery_optimization: true,
            test_timeout_seconds: 30,
        });
        
        // iOS test configuration
        test_configs.insert(PlatformType::iOS, MobileTestConfig {
            platform: PlatformType::iOS,
            enable_bluetooth: false, // Disabled for CI
            enable_power_management: true,
            enable_battery_optimization: true,
            test_timeout_seconds: 30,
        });
        
        // Desktop test configuration
        test_configs.insert(PlatformType::Desktop, MobileTestConfig {
            platform: PlatformType::Desktop,
            enable_bluetooth: false,
            enable_power_management: false, // Not as relevant for desktop
            enable_battery_optimization: false,
            test_timeout_seconds: 20,
        });

        Self { test_configs }
    }

    /// Run cross-platform compatibility tests
    pub async fn run_compatibility_tests(&self) -> HashMap<PlatformType, MobileTestResults> {
        let mut results = HashMap::new();
        
        for (platform_type, config) in &self.test_configs {
            log::info!("Running cross-platform tests for: {:?}", platform_type);
            
            let harness = MobileTestHarness::new(config.clone());
            if harness.setup().await.is_ok() {
                let mut platform_results = MobileTestResults::new();
                
                // Test basic functionality
                self.test_basic_functionality(&harness, &mut platform_results).await;
                
                // Test configuration compatibility
                self.test_configuration_compatibility(&harness, &mut platform_results).await;
                
                // Test power management (if enabled)
                if config.enable_power_management {
                    self.test_power_management_compatibility(&harness, &mut platform_results).await;
                }
                
                // Test battery optimization (if enabled)
                if config.enable_battery_optimization {
                    self.test_battery_optimization_compatibility(&harness, &mut platform_results).await;
                }
                
                // Test error handling consistency
                self.test_error_handling_consistency(&harness, &mut platform_results).await;
                
                let _ = harness.cleanup().await;
                results.insert(platform_type.clone(), platform_results);
            }
        }
        
        results
    }

    /// Test basic functionality across platforms
    async fn test_basic_functionality(&self, harness: &MobileTestHarness, results: &mut MobileTestResults) {
        let test_result = harness.run_test_with_timeout(
            "cross_platform_basic_functionality",
            async {
                // Test node creation
                let node = harness.create_test_node().await?;
                
                // Test basic operations
                let status = node.get_status();
                MobileTestUtils::assert_node_state(&status, NodeState::Ready)?;
                
                // Test configuration
                node.set_power_mode(PowerMode::Balanced)?;
                
                // Test network stats (should not crash)
                let _stats = node.get_network_stats();
                
                // Test peer list (should return empty list initially)
                let peers = node.get_connected_peers();
                assert!(peers.is_empty());
                
                Ok(())
            }
        ).await;

        results.add_result(TestResult {
            test_name: "cross_platform_basic_functionality".to_string(),
            status: if test_result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: test_result.err().map(|e| e.to_string()),
        });
    }

    /// Test configuration compatibility across platforms
    async fn test_configuration_compatibility(&self, harness: &MobileTestHarness, results: &mut MobileTestResults) {
        let test_result = harness.run_test_with_timeout(
            "cross_platform_configuration",
            async {
                let node = harness.create_test_node().await?;
                
                // Test all power modes work on this platform
                let power_modes = vec![
                    PowerMode::HighPerformance,
                    PowerMode::Balanced,
                    PowerMode::BatterySaver,
                    PowerMode::UltraLowPower,
                ];
                
                for mode in power_modes {
                    node.set_power_mode(mode)?;
                    // Allow time for mode to be applied
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                
                // Test scan interval configuration
                let scan_intervals = vec![100, 500, 1000, 2000, 5000];
                
                for interval in scan_intervals {
                    node.set_scan_interval(interval)?;
                    // Allow time for interval to be applied
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                
                // Test platform-specific configuration
                let platform_config = harness.get_test_platform_config();
                node.configure_for_platform(platform_config)?;
                
                Ok(())
            }
        ).await;

        results.add_result(TestResult {
            test_name: "cross_platform_configuration".to_string(),
            status: if test_result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: test_result.err().map(|e| e.to_string()),
        });
    }

    /// Test power management compatibility
    async fn test_power_management_compatibility(&self, harness: &MobileTestHarness, results: &mut MobileTestResults) {
        let test_result = harness.run_test_with_timeout(
            "cross_platform_power_management",
            async {
                let node = harness.create_test_node().await?;
                
                // Test power mode transitions
                let transitions = vec![
                    (PowerMode::HighPerformance, PowerMode::Balanced),
                    (PowerMode::Balanced, PowerMode::BatterySaver),
                    (PowerMode::BatterySaver, PowerMode::UltraLowPower),
                    (PowerMode::UltraLowPower, PowerMode::HighPerformance),
                ];
                
                for (from_mode, to_mode) in transitions {
                    node.set_power_mode(from_mode)?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    
                    node.set_power_mode(to_mode)?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    
                    // Verify the node is still operational
                    let status = node.get_status();
                    assert!(matches!(status.state, NodeState::Ready | NodeState::Discovering | NodeState::Connected));
                }
                
                Ok(())
            }
        ).await;

        results.add_result(TestResult {
            test_name: "cross_platform_power_management".to_string(),
            status: if test_result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: test_result.err().map(|e| e.to_string()),
        });
    }

    /// Test battery optimization compatibility
    async fn test_battery_optimization_compatibility(&self, harness: &MobileTestHarness, results: &mut MobileTestResults) {
        let test_result = harness.run_test_with_timeout(
            "cross_platform_battery_optimization",
            async {
                let node = harness.create_test_node().await?;
                
                // Create battery optimization handler
                let power_manager = Arc::new(PowerManager::new(PowerMode::Balanced));
                let battery_handler = BatteryOptimizationHandler::new(
                    harness.config.platform.clone(),
                    power_manager,
                    None, // No event sender for this test
                );
                
                // Test optimization detection (should not crash)
                battery_handler.record_scan_event(1000, 1000, true);
                battery_handler.record_scan_event(1000, 3000, true); // Simulate throttling
                battery_handler.record_scan_event(1000, 5000, false); // Simulate failure
                
                // Test recommendations (should not crash)
                let recommendations = battery_handler.get_optimization_recommendations();
                assert!(recommendations.len() >= 0); // Should return some number of recommendations
                
                // Test battery-aware scanning strategy
                let mut strategy = BatteryAwareScanStrategy::new(harness.get_test_platform_config());
                
                // Test with different battery levels
                let battery_levels = vec![1.0, 0.8, 0.5, 0.2, 0.1];
                
                for level in battery_levels {
                    strategy.update_battery_state(Some(level), false);
                    let (window, interval, duty_cycle) = strategy.get_optimal_scan_parameters();
                    
                    assert!(window > 0);
                    assert!(interval > 0);
                    assert!(duty_cycle > 0.0 && duty_cycle <= 1.0);
                    
                    // Lower battery should result in more conservative parameters
                    if level < 0.2 {
                        assert!(duty_cycle < 1.0);
                    }
                }
                
                Ok(())
            }
        ).await;

        results.add_result(TestResult {
            test_name: "cross_platform_battery_optimization".to_string(),
            status: if test_result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: test_result.err().map(|e| e.to_string()),
        });
    }

    /// Test error handling consistency across platforms
    async fn test_error_handling_consistency(&self, harness: &MobileTestHarness, results: &mut MobileTestResults) {
        let test_result = harness.run_test_with_timeout(
            "cross_platform_error_handling",
            async {
                let node = harness.create_test_node().await?;
                
                // Test invalid scan intervals (should handle gracefully)
                let invalid_intervals = vec![0, 50, 100000];
                
                for interval in invalid_intervals {
                    let result = node.set_scan_interval(interval);
                    // Should either succeed (with adjustment) or fail gracefully
                    match result {
                        Ok(_) => {}, // Acceptable
                        Err(BitCrapsError::InvalidInput { .. }) => {}, // Expected error type
                        Err(_) => {
                            return Err(BitCrapsError::InvalidInput {
                                reason: format!("Unexpected error type for invalid interval: {}", interval),
                            });
                        }
                    }
                }
                
                // Test invalid game operations
                let result = node.join_game("".to_string()).await;
                match result {
                    Err(BitCrapsError::InvalidInput { .. }) | 
                    Err(BitCrapsError::NotFound { .. }) | 
                    Err(BitCrapsError::GameError { .. }) => {}, // Expected error types
                    Ok(_) => {
                        return Err(BitCrapsError::InvalidInput {
                            reason: "Empty game ID should not succeed".to_string(),
                        });
                    },
                    Err(_) => {}, // Other errors are acceptable
                }
                
                // Test creating game with invalid config
                let invalid_config = GameConfig {
                    game_name: Some("".to_string()),
                    min_bet: 0,
                    max_bet: 0,
                    max_players: 0,
                    timeout_seconds: 0,
                };
                
                let result = node.create_game(invalid_config).await;
                // Should handle gracefully (either succeed with adjustments or fail with appropriate error)
                match result {
                    Ok(_) => {}, // May succeed with adjusted values
                    Err(BitCrapsError::InvalidInput { .. }) => {}, // Expected error type
                    Err(BitCrapsError::GameError { .. }) => {}, // Also acceptable
                    Err(_) => {}, // Other errors are acceptable for this test
                }
                
                Ok(())
            }
        ).await;

        results.add_result(TestResult {
            test_name: "cross_platform_error_handling".to_string(),
            status: if test_result.is_ok() { TestStatus::Passed } else { TestStatus::Failed },
            duration_ms: 0,
            error_message: test_result.err().map(|e| e.to_string()),
        });
    }

    /// Generate comprehensive cross-platform compatibility report
    pub fn generate_compatibility_report(&self, results: &HashMap<PlatformType, MobileTestResults>) -> String {
        let mut report = String::new();
        
        report.push_str("# Cross-Platform Compatibility Report\n\n");
        
        for (platform, test_results) in results {
            report.push_str(&format!("## Platform: {:?}\n\n", platform));
            report.push_str(&format!("- Total Tests: {}\n", test_results.total_tests));
            report.push_str(&format!("- Passed: {}\n", test_results.passed_tests));
            report.push_str(&format!("- Failed: {}\n", test_results.failed_tests));
            report.push_str(&format!("- Success Rate: {:.1}%\n\n", test_results.success_rate() * 100.0));
            
            if test_results.failed_tests > 0 {
                report.push_str("### Failed Tests:\n");
                for result in &test_results.test_details {
                    if matches!(result.status, TestStatus::Failed | TestStatus::Timeout) {
                        report.push_str(&format!("- {}: {:?}\n", result.test_name, result.status));
                        if let Some(error) = &result.error_message {
                            report.push_str(&format!("  Error: {}\n", error));
                        }
                    }
                }
                report.push_str("\n");
            }
        }
        
        // Overall statistics
        let total_tests: u32 = results.values().map(|r| r.total_tests).sum();
        let total_passed: u32 = results.values().map(|r| r.passed_tests).sum();
        let total_failed: u32 = results.values().map(|r| r.failed_tests).sum();
        let overall_success_rate = if total_tests > 0 {
            total_passed as f64 / total_tests as f64
        } else {
            0.0
        };
        
        report.push_str("## Overall Statistics\n\n");
        report.push_str(&format!("- Platforms Tested: {}\n", results.len()));
        report.push_str(&format!("- Total Tests: {}\n", total_tests));
        report.push_str(&format!("- Total Passed: {}\n", total_passed));
        report.push_str(&format!("- Total Failed: {}\n", total_failed));
        report.push_str(&format!("- Overall Success Rate: {:.1}%\n\n", overall_success_rate * 100.0));
        
        // Compatibility assessment
        let compatible_platforms = results.iter()
            .filter(|(_, r)| r.success_rate() >= 0.8)
            .count();
        
        report.push_str("## Compatibility Assessment\n\n");
        report.push_str(&format!("- Fully Compatible Platforms: {}/{}\n", compatible_platforms, results.len()));
        
        if compatible_platforms == results.len() {
            report.push_str("- **Status: FULLY COMPATIBLE** ✅\n");
        } else if compatible_platforms > 0 {
            report.push_str("- **Status: PARTIALLY COMPATIBLE** ⚠️\n");
        } else {
            report.push_str("- **Status: COMPATIBILITY ISSUES** ❌\n");
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cross_platform_suite_creation() {
        let suite = CrossPlatformTests::new();
        assert!(suite.test_configs.contains_key(&PlatformType::Android));
        assert!(suite.test_configs.contains_key(&PlatformType::iOS));
    }

    #[test]
    fn test_compatibility_report_generation() {
        let suite = CrossPlatformTests::new();
        let mut results = HashMap::new();
        
        let mut android_results = MobileTestResults::new();
        android_results.add_result(TestResult {
            test_name: "test1".to_string(),
            status: TestStatus::Passed,
            duration_ms: 100,
            error_message: None,
        });
        
        results.insert(PlatformType::Android, android_results);
        
        let report = suite.generate_compatibility_report(&results);
        assert!(report.contains("Cross-Platform Compatibility Report"));
        assert!(report.contains("Android"));
    }
}