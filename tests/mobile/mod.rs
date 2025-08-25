//! Cross-platform mobile testing framework
//! 
//! This module provides comprehensive testing for mobile platforms including
//! Android and iOS integration, UniFFI bindings, and cross-platform compatibility.

pub mod uniffi_tests;
pub mod android_tests;
pub mod ios_tests;
pub mod cross_platform_tests;
pub mod battery_optimization_tests;
pub mod power_management_tests;

use crate::mobile::*;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Test configuration for mobile testing
#[derive(Clone)]
pub struct MobileTestConfig {
    pub platform: PlatformType,
    pub enable_bluetooth: bool,
    pub enable_power_management: bool,
    pub enable_battery_optimization: bool,
    pub test_timeout_seconds: u64,
}

impl Default for MobileTestConfig {
    fn default() -> Self {
        Self {
            platform: PlatformDetection::detect_platform(),
            enable_bluetooth: false, // Disabled by default for CI environments
            enable_power_management: true,
            enable_battery_optimization: true,
            test_timeout_seconds: 30,
        }
    }
}

/// Mobile test harness for cross-platform testing
pub struct MobileTestHarness {
    config: MobileTestConfig,
    test_data_dir: String,
}

impl MobileTestHarness {
    /// Create a new mobile test harness
    pub fn new(config: MobileTestConfig) -> Self {
        let test_data_dir = format!("./test_data/mobile_{:?}", config.platform);
        
        Self {
            config,
            test_data_dir,
        }
    }

    /// Set up test environment
    pub async fn setup(&self) -> Result<(), BitCrapsError> {
        // Create test data directory
        std::fs::create_dir_all(&self.test_data_dir)
            .map_err(|e| BitCrapsError::InitializationError {
                reason: format!("Failed to create test data directory: {}", e),
            })?;

        log::info!("Mobile test harness initialized for platform: {:?}", self.config.platform);
        Ok(())
    }

    /// Clean up test environment
    pub async fn cleanup(&self) -> Result<(), BitCrapsError> {
        // Remove test data directory
        if std::path::Path::new(&self.test_data_dir).exists() {
            std::fs::remove_dir_all(&self.test_data_dir)
                .map_err(|e| BitCrapsError::InitializationError {
                    reason: format!("Failed to remove test data directory: {}", e),
                })?;
        }

        log::info!("Mobile test harness cleaned up");
        Ok(())
    }

    /// Create a test BitCraps node
    pub async fn create_test_node(&self) -> Result<Arc<BitCrapsNode>, BitCrapsError> {
        let config = BitCrapsConfig {
            data_dir: self.test_data_dir.clone(),
            pow_difficulty: 1, // Low difficulty for testing
            protocol_version: 1,
            power_mode: PowerMode::HighPerformance, // High performance for testing
            platform_config: Some(self.get_test_platform_config()),
            enable_logging: true,
            log_level: LogLevel::Debug,
        };

        timeout(
            Duration::from_secs(self.config.test_timeout_seconds),
            create_node(config)
        ).await
        .map_err(|_| BitCrapsError::Timeout)?
    }

    /// Get platform-specific test configuration
    fn get_test_platform_config(&self) -> PlatformConfig {
        match self.config.platform {
            PlatformType::Android => PlatformConfig {
                platform: PlatformType::Android,
                background_scanning: false, // Disabled for testing
                scan_window_ms: 100,        // Fast scanning for tests
                scan_interval_ms: 200,
                low_power_mode: false,
                service_uuids: vec!["12345678-1234-5678-1234-567812345678".to_string()],
            },
            PlatformType::iOS => PlatformConfig {
                platform: PlatformType::iOS,
                background_scanning: false,
                scan_window_ms: 100,
                scan_interval_ms: 200,
                low_power_mode: false,
                service_uuids: vec!["12345678-1234-5678-1234-567812345678".to_string()],
            },
            _ => PlatformConfig {
                platform: self.config.platform.clone(),
                background_scanning: false,
                scan_window_ms: 100,
                scan_interval_ms: 200,
                low_power_mode: false,
                service_uuids: vec!["12345678-1234-5678-1234-567812345678".to_string()],
            }
        }
    }

    /// Run a test with timeout
    pub async fn run_test_with_timeout<F, T>(&self, test_name: &str, test_fn: F) -> Result<T, BitCrapsError>
    where
        F: std::future::Future<Output = Result<T, BitCrapsError>>,
    {
        log::info!("Running mobile test: {}", test_name);
        
        let result = timeout(
            Duration::from_secs(self.config.test_timeout_seconds),
            test_fn
        ).await;

        match result {
            Ok(test_result) => {
                log::info!("Mobile test completed: {}", test_name);
                test_result
            },
            Err(_) => {
                log::error!("Mobile test timed out: {}", test_name);
                Err(BitCrapsError::Timeout)
            }
        }
    }
}

/// Mobile test utilities
pub struct MobileTestUtils;

impl MobileTestUtils {
    /// Create mock peer info for testing
    pub fn create_mock_peer(peer_id: &str) -> PeerInfo {
        PeerInfo {
            peer_id: peer_id.to_string(),
            display_name: Some(format!("Test Peer {}", peer_id)),
            signal_strength: 80,
            last_seen: current_timestamp(),
            is_connected: false,
        }
    }

    /// Create mock game event for testing
    pub fn create_mock_game_event() -> GameEvent {
        GameEvent::PeerDiscovered {
            peer: Self::create_mock_peer("test_peer_001"),
        }
    }

    /// Create test game configuration
    pub fn create_test_game_config() -> GameConfig {
        GameConfig {
            game_name: Some("Test Game".to_string()),
            min_bet: 1,
            max_bet: 100,
            max_players: 4,
            timeout_seconds: 60,
        }
    }

    /// Assert that a node is in the expected state
    pub fn assert_node_state(status: &NodeStatus, expected_state: NodeState) -> Result<(), BitCrapsError> {
        match (&status.state, &expected_state) {
            (NodeState::Ready, NodeState::Ready) |
            (NodeState::Discovering, NodeState::Discovering) |
            (NodeState::Connected, NodeState::Connected) |
            (NodeState::InGame, NodeState::InGame) |
            (NodeState::Initializing, NodeState::Initializing) => Ok(()),
            (NodeState::Error { .. }, NodeState::Error { .. }) => Ok(()),
            (actual, expected) => {
                Err(BitCrapsError::InvalidInput {
                    reason: format!("Node state mismatch: expected {:?}, got {:?}", expected, actual),
                })
            }
        }
    }

    /// Wait for node to reach specific state
    pub async fn wait_for_node_state(
        node: &Arc<BitCrapsNode>,
        expected_state: NodeState,
        timeout_seconds: u64,
    ) -> Result<(), BitCrapsError> {
        let start_time = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(timeout_seconds);

        loop {
            let status = node.get_status();
            if Self::states_match(&status.state, &expected_state) {
                return Ok(());
            }

            if start_time.elapsed() > timeout_duration {
                return Err(BitCrapsError::Timeout);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Check if two node states match (allowing for error state variations)
    fn states_match(actual: &NodeState, expected: &NodeState) -> bool {
        match (actual, expected) {
            (NodeState::Ready, NodeState::Ready) |
            (NodeState::Discovering, NodeState::Discovering) |
            (NodeState::Connected, NodeState::Connected) |
            (NodeState::InGame, NodeState::InGame) |
            (NodeState::Initializing, NodeState::Initializing) => true,
            (NodeState::Error { .. }, NodeState::Error { .. }) => true,
            _ => false,
        }
    }

    /// Simulate battery level change
    pub fn simulate_battery_change(level: f32, is_charging: bool) -> BatteryInfo {
        BatteryInfo {
            level: Some(level.clamp(0.0, 1.0)),
            is_charging,
        }
    }

    /// Create mock network stats
    pub fn create_mock_network_stats() -> NetworkStats {
        NetworkStats {
            peers_discovered: 3,
            active_connections: 2,
            bytes_sent: 1024,
            bytes_received: 2048,
            packets_dropped: 0,
            average_latency_ms: 25.5,
        }
    }
}

/// Battery info for testing
#[derive(Clone, Debug)]
pub struct BatteryInfo {
    pub level: Option<f32>,
    pub is_charging: bool,
}

/// Test result collector for mobile tests
pub struct MobileTestResults {
    pub total_tests: u32,
    pub passed_tests: u32,
    pub failed_tests: u32,
    pub skipped_tests: u32,
    pub test_details: Vec<TestResult>,
}

#[derive(Clone, Debug)]
pub struct TestResult {
    pub test_name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Timeout,
}

impl MobileTestResults {
    pub fn new() -> Self {
        Self {
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            test_details: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: TestResult) {
        self.total_tests += 1;
        
        match result.status {
            TestStatus::Passed => self.passed_tests += 1,
            TestStatus::Failed => self.failed_tests += 1,
            TestStatus::Skipped => self.skipped_tests += 1,
            TestStatus::Timeout => self.failed_tests += 1,
        }

        self.test_details.push(result);
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            0.0
        } else {
            self.passed_tests as f64 / self.total_tests as f64
        }
    }

    pub fn print_summary(&self) {
        println!("Mobile Test Results:");
        println!("  Total Tests: {}", self.total_tests);
        println!("  Passed: {}", self.passed_tests);
        println!("  Failed: {}", self.failed_tests);
        println!("  Skipped: {}", self.skipped_tests);
        println!("  Success Rate: {:.1}%", self.success_rate() * 100.0);

        if self.failed_tests > 0 {
            println!("\nFailed Tests:");
            for result in &self.test_details {
                if matches!(result.status, TestStatus::Failed | TestStatus::Timeout) {
                    println!("  - {}: {:?}", result.test_name, result.status);
                    if let Some(error) = &result.error_message {
                        println!("    Error: {}", error);
                    }
                }
            }
        }
    }
}

// Integration with existing test framework
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mobile_harness_setup() {
        let config = MobileTestConfig::default();
        let harness = MobileTestHarness::new(config);
        
        assert!(harness.setup().await.is_ok());
        assert!(harness.cleanup().await.is_ok());
    }

    #[tokio::test]
    async fn test_mock_data_creation() {
        let peer = MobileTestUtils::create_mock_peer("test123");
        assert_eq!(peer.peer_id, "test123");
        assert!(peer.display_name.is_some());
        
        let event = MobileTestUtils::create_mock_game_event();
        match event {
            GameEvent::PeerDiscovered { .. } => {}, // Expected
            _ => panic!("Expected PeerDiscovered event"),
        }
    }
}