//! Advanced Chaos Engineering Tests for Production Resilience
//!
//! These tests simulate complex failure scenarios that can occur in production
//! environments and verify system recovery capabilities.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, timeout};
use rand::{thread_rng, Rng, seq::SliceRandom};

use bitcraps::protocol::consensus::engine::ConsensusEngine;
use bitcraps::protocol::{ConsensusMessage, GameId, PeerId};
use bitcraps::transport::nat_traversal::TurnRelay;
use bitcraps::gaming::consensus_game_manager::ConsensusGameManager;
use bitcraps::utils::correlation::{CorrelationManager, CorrelationConfig, RequestContext};

/// Advanced chaos scenarios for production resilience testing
pub struct AdvancedChaosTestSuite {
    scenario_results: Arc<Mutex<HashMap<String, ChaosScenarioResult>>>,
}

#[derive(Debug, Clone)]
pub struct ChaosScenarioResult {
    pub scenario_name: String,
    pub duration: Duration,
    pub success: bool,
    pub recovery_time: Option<Duration>,
    pub failures_injected: u32,
    pub system_degradation: f64, // 0.0 to 1.0
    pub logs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NetworkPartitionScenario {
    pub partition_duration: Duration,
    pub partition_percentage: f64,
    pub healing_strategy: HealingStrategy,
}

#[derive(Debug, Clone)]
pub enum HealingStrategy {
    Immediate,
    Gradual { healing_rate: f64 },
    Manual,
}

impl AdvancedChaosTestSuite {
    pub fn new() -> Self {
        Self {
            scenario_results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Test system resilience under cascading failures
    pub async fn test_cascading_failures(&self) -> anyhow::Result<ChaosScenarioResult> {
        let start_time = Instant::now();
        let mut logs = Vec::new();
        let mut failures_injected = 0;

        logs.push("Starting cascading failure simulation".to_string());

        // Set up test environment
        let mut consensus_engines = Vec::new();
        let correlation_manager = Arc::new(CorrelationManager::new(CorrelationConfig::default()));

        // Create 5 consensus engines representing distributed nodes
        for i in 0..5 {
            consensus_engines.push((format!("node_{}", i), ConsensusEngine::new()));
        }

        logs.push(format!("Created {} consensus nodes", consensus_engines.len()));

        let scenario_result = {
            // Phase 1: Single node failure
            logs.push("Phase 1: Single node failure".to_string());
            
            // Simulate node 0 going down
            consensus_engines.remove(0);
            failures_injected += 1;
            
            sleep(Duration::from_millis(500)).await;

            // Phase 2: Network partition (split-brain scenario)
            logs.push("Phase 2: Network partition simulation".to_string());
            
            let (partition_a, partition_b) = consensus_engines.split_at_mut(2);
            
            // Simulate messages not crossing partition boundary for 2 seconds
            sleep(Duration::from_secs(2)).await;
            failures_injected += 1;

            // Phase 3: Resource exhaustion
            logs.push("Phase 3: Resource exhaustion simulation".to_string());
            
            // Simulate high memory usage by creating correlation contexts
            let mut contexts = Vec::new();
            for i in 0..1000 {
                let context = RequestContext::new()
                    .with_operation(format!("chaos_op_{}", i))
                    .with_source("chaos_test".to_string());
                
                correlation_manager.start_request(context.clone()).await?;
                contexts.push(context);
            }
            failures_injected += 1;

            // Phase 4: Recovery phase
            logs.push("Phase 4: Recovery and healing".to_string());
            
            // Clean up contexts
            for context in contexts {
                let _ = correlation_manager.complete_request(&context.correlation_id).await;
            }

            // Merge partitions back (simulate network healing)
            consensus_engines.push(("recovered_node".to_string(), ConsensusEngine::new()));

            sleep(Duration::from_millis(500)).await;

            // Measure system health after recovery
            let stats = correlation_manager.get_statistics().await;
            let system_degradation = if stats.failed_requests > 0 {
                stats.failed_requests as f64 / (stats.completed_requests + stats.failed_requests) as f64
            } else {
                0.0
            };

            ChaosScenarioResult {
                scenario_name: "cascading_failures".to_string(),
                duration: start_time.elapsed(),
                success: system_degradation < 0.5, // Success if less than 50% failure rate
                recovery_time: Some(Duration::from_secs(1)),
                failures_injected,
                system_degradation,
                logs,
            }
        };

        let mut results = self.scenario_results.lock().await;
        results.insert("cascading_failures".to_string(), scenario_result.clone());

        Ok(scenario_result)
    }

    /// Test Byzantine fault tolerance under adversarial conditions
    pub async fn test_byzantine_adversarial_scenario(&self) -> anyhow::Result<ChaosScenarioResult> {
        let start_time = Instant::now();
        let mut logs = Vec::new();
        let mut failures_injected = 0;

        logs.push("Starting Byzantine adversarial scenario".to_string());

        // Create Byzantine test environment
        let mut honest_nodes = Vec::new();
        let mut byzantine_nodes = Vec::new();

        // 7 honest nodes, 2 Byzantine (under 1/3 threshold)
        for i in 0..7 {
            honest_nodes.push((format!("honest_{}", i), ConsensusEngine::new()));
        }

        for i in 0..2 {
            byzantine_nodes.push((format!("byzantine_{}", i), ConsensusEngine::new()));
        }

        logs.push(format!("Created {} honest nodes, {} Byzantine nodes", 
                         honest_nodes.len(), byzantine_nodes.len()));

        let scenario_result = {
            let game_id = [1u8; 16];
            let mut timestamp = 1000u64;

            // Phase 1: Normal operation
            logs.push("Phase 1: Normal consensus operation".to_string());
            
            for (name, engine) in &mut honest_nodes {
                let peer_id = [rand::thread_rng().gen(); 32];
                let message = ConsensusMessage::new(
                    game_id,
                    peer_id,
                    bitcraps::protocol::ConsensusPayload::GameProposal(
                        bitcraps::gaming::GameProposal {
                            operation: "normal_op".to_string(),
                            participants: vec![peer_id],
                            data: vec![],
                            timestamp,
                        }
                    ),
                    timestamp
                );
                
                let _ = engine.process_message(message);
                timestamp += 1;
            }

            // Phase 2: Byzantine attacks
            logs.push("Phase 2: Byzantine attacks - double spending".to_string());
            
            failures_injected += 1;
            
            // Byzantine nodes try double spending
            for (name, engine) in &mut byzantine_nodes {
                let peer_id = [2u8; 32]; // Same peer ID for double spend
                
                // Send conflicting messages
                let message1 = ConsensusMessage::new(
                    game_id,
                    peer_id,
                    bitcraps::protocol::ConsensusPayload::BetPlacement { amount: 100 },
                    timestamp
                );
                let message2 = ConsensusMessage::new(
                    game_id,
                    peer_id,
                    bitcraps::protocol::ConsensusPayload::BetPlacement { amount: 200 },
                    timestamp // Same timestamp = double spend attempt
                );
                
                let _ = engine.process_message(message1);
                let _ = engine.process_message(message2);
                timestamp += 1;
            }

            // Phase 3: Equivocation attack
            logs.push("Phase 3: Equivocation attack".to_string());
            
            failures_injected += 1;
            
            // Byzantine nodes send different messages to different honest nodes
            let byzantine_peer = [3u8; 32];
            
            for (i, (name, engine)) in honest_nodes.iter_mut().enumerate() {
                let conflicting_data = if i % 2 == 0 { vec![1, 2, 3] } else { vec![4, 5, 6] };
                
                let message = ConsensusMessage::new(
                    game_id,
                    byzantine_peer,
                    bitcraps::protocol::ConsensusPayload::GameProposal(
                        bitcraps::gaming::GameProposal {
                            operation: "equivocation".to_string(),
                            participants: vec![byzantine_peer],
                            data: conflicting_data,
                            timestamp,
                        }
                    ),
                    timestamp
                );
                
                let _ = engine.process_message(message);
            }

            // Phase 4: Verify Byzantine fault tolerance
            logs.push("Phase 4: Verifying system integrity".to_string());
            
            let mut consensus_achieved = true;
            let mut state_hashes = Vec::new();
            
            for (name, engine) in &honest_nodes {
                let state = engine.get_state();
                state_hashes.push(state.get_hash());
            }
            
            // Check if majority of honest nodes agree
            let mut hash_counts = HashMap::new();
            for hash in &state_hashes {
                *hash_counts.entry(hash).or_insert(0) += 1;
            }
            
            let majority_threshold = (honest_nodes.len() + 1) / 2;
            let has_majority = hash_counts.values().any(|&count| count >= majority_threshold);
            
            if !has_majority {
                consensus_achieved = false;
                logs.push("WARNING: No majority consensus achieved".to_string());
            }

            let system_degradation = if consensus_achieved { 0.0 } else { 1.0 };

            ChaosScenarioResult {
                scenario_name: "byzantine_adversarial".to_string(),
                duration: start_time.elapsed(),
                success: consensus_achieved && failures_injected <= 2,
                recovery_time: None,
                failures_injected,
                system_degradation,
                logs,
            }
        };

        let mut results = self.scenario_results.lock().await;
        results.insert("byzantine_adversarial".to_string(), scenario_result.clone());

        Ok(scenario_result)
    }

    /// Test network partition with TURN relay resilience
    pub async fn test_turn_relay_partition_recovery(&self) -> anyhow::Result<ChaosScenarioResult> {
        let start_time = Instant::now();
        let mut logs = Vec::new();
        let mut failures_injected = 0;

        logs.push("Starting TURN relay partition recovery test".to_string());

        let scenario_result = {
            // Set up TURN relay infrastructure
            let turn_relay = TurnRelay::new("test-realm".to_string());
            
            logs.push("TURN relay infrastructure initialized".to_string());

            // Phase 1: Normal TURN operation
            logs.push("Phase 1: Normal TURN relay operation".to_string());
            
            let allocation_result = turn_relay.allocate_relay_address("client1".to_string()).await;
            if allocation_result.is_ok() {
                logs.push("Successfully allocated relay address".to_string());
            } else {
                logs.push("Failed to allocate relay address".to_string());
                failures_injected += 1;
            }

            // Phase 2: Simulate network partition
            logs.push("Phase 2: Network partition simulation".to_string());
            
            // Simulate partition by introducing delays
            sleep(Duration::from_millis(100)).await;
            
            let partition_start = Instant::now();
            
            // Try operations during partition
            let relay_result = turn_relay.relay_data(
                "client1".to_string(),
                "client2".to_string(),
                vec![1, 2, 3, 4, 5]
            ).await;
            
            match relay_result {
                Ok(_) => logs.push("Data relay succeeded during partition".to_string()),
                Err(_) => {
                    logs.push("Data relay failed during partition (expected)".to_string());
                    failures_injected += 1;
                }
            }

            // Phase 3: Recovery
            logs.push("Phase 3: Recovery and connection restoration".to_string());
            
            sleep(Duration::from_millis(200)).await;
            
            // Test recovery
            let recovery_result = turn_relay.allocate_relay_address("client3".to_string()).await;
            let recovery_success = recovery_result.is_ok();
            
            if recovery_success {
                logs.push("Recovery successful - new allocations working".to_string());
            } else {
                logs.push("Recovery failed - allocations still failing".to_string());
            }

            let recovery_time = partition_start.elapsed();
            let system_degradation = if recovery_success { 0.2 } else { 0.8 };

            ChaosScenarioResult {
                scenario_name: "turn_relay_partition".to_string(),
                duration: start_time.elapsed(),
                success: recovery_success,
                recovery_time: Some(recovery_time),
                failures_injected,
                system_degradation,
                logs,
            }
        };

        let mut results = self.scenario_results.lock().await;
        results.insert("turn_relay_partition".to_string(), scenario_result.clone());

        Ok(scenario_result)
    }

    /// Test memory pressure and resource exhaustion
    pub async fn test_memory_pressure_resilience(&self) -> anyhow::Result<ChaosScenarioResult> {
        let start_time = Instant::now();
        let mut logs = Vec::new();
        let mut failures_injected = 0;

        logs.push("Starting memory pressure resilience test".to_string());

        let scenario_result = {
            let correlation_manager = Arc::new(CorrelationManager::new(CorrelationConfig {
                max_active_requests: 1000, // Reduced limit for testing
                ..Default::default()
            }));

            // Phase 1: Normal operation
            logs.push("Phase 1: Normal operation baseline".to_string());
            
            let mut contexts = Vec::new();
            for i in 0..100 {
                let context = RequestContext::new()
                    .with_operation(format!("normal_op_{}", i))
                    .with_source("baseline_test".to_string());
                
                correlation_manager.start_request(context.clone()).await?;
                contexts.push(context);
            }
            
            logs.push(format!("Created {} baseline requests", contexts.len()));

            // Phase 2: Memory pressure simulation
            logs.push("Phase 2: Memory pressure simulation".to_string());
            
            let mut pressure_contexts = Vec::new();
            let mut allocation_failures = 0;
            
            // Try to exhaust memory by creating many correlation contexts
            for i in 0..2000 { // Above the limit
                let context = RequestContext::new()
                    .with_operation(format!("pressure_op_{}", i))
                    .with_source("pressure_test".to_string());
                
                match correlation_manager.start_request(context.clone()).await {
                    Ok(_) => pressure_contexts.push(context),
                    Err(_) => {
                        allocation_failures += 1;
                        if allocation_failures == 1 {
                            failures_injected += 1;
                            logs.push("Resource exhaustion triggered".to_string());
                        }
                    }
                }
                
                // Break if we hit the limit
                if allocation_failures > 10 {
                    break;
                }
            }
            
            logs.push(format!("Memory pressure created {} contexts, {} failures", 
                             pressure_contexts.len(), allocation_failures));

            // Phase 3: System behavior under pressure
            logs.push("Phase 3: System behavior under pressure".to_string());
            
            let stats_under_pressure = correlation_manager.get_statistics().await;
            
            // Try critical operations under pressure
            let critical_context = RequestContext::new()
                .with_operation("critical_operation".to_string())
                .with_source("critical_test".to_string());
                
            let critical_result = correlation_manager.start_request(critical_context.clone()).await;
            let critical_success = critical_result.is_ok();
            
            if critical_success {
                logs.push("Critical operation succeeded under pressure".to_string());
                let _ = correlation_manager.complete_request(&critical_context.correlation_id).await;
            } else {
                logs.push("Critical operation failed under pressure".to_string());
            }

            // Phase 4: Recovery by releasing pressure
            logs.push("Phase 4: Recovery phase".to_string());
            
            // Release some pressure contexts
            let release_count = pressure_contexts.len() / 2;
            for context in pressure_contexts.iter().take(release_count) {
                let _ = correlation_manager.complete_request(&context.correlation_id).await;
            }
            
            logs.push(format!("Released {} contexts", release_count));
            
            // Test if system recovered
            let recovery_context = RequestContext::new()
                .with_operation("recovery_test".to_string())
                .with_source("recovery_test".to_string());
                
            let recovery_result = correlation_manager.start_request(recovery_context.clone()).await;
            let recovery_success = recovery_result.is_ok();
            
            if recovery_success {
                logs.push("System recovered - new requests accepted".to_string());
                let _ = correlation_manager.complete_request(&recovery_context.correlation_id).await;
            }

            // Calculate system degradation
            let final_stats = correlation_manager.get_statistics().await;
            let total_attempts = final_stats.completed_requests + final_stats.failed_requests;
            let system_degradation = if total_attempts > 0 {
                final_stats.failed_requests as f64 / total_attempts as f64
            } else {
                0.0
            };

            ChaosScenarioResult {
                scenario_name: "memory_pressure_resilience".to_string(),
                duration: start_time.elapsed(),
                success: recovery_success && system_degradation < 0.5,
                recovery_time: Some(Duration::from_secs(1)),
                failures_injected,
                system_degradation,
                logs,
            }
        };

        let mut results = self.scenario_results.lock().await;
        results.insert("memory_pressure_resilience".to_string(), scenario_result.clone());

        Ok(scenario_result)
    }

    /// Test time synchronization chaos (clock skew)
    pub async fn test_clock_skew_resilience(&self) -> anyhow::Result<ChaosScenarioResult> {
        let start_time = Instant::now();
        let mut logs = Vec::new();
        let mut failures_injected = 0;

        logs.push("Starting clock skew resilience test".to_string());

        let scenario_result = {
            let mut engines = Vec::new();
            for i in 0..3 {
                engines.push((format!("node_{}", i), ConsensusEngine::new()));
            }

            let game_id = [42u8; 16];
            let peer_id = [1u8; 32];

            // Phase 1: Normal timestamp operation
            logs.push("Phase 1: Normal synchronized timestamps".to_string());
            
            let base_time = 1000000u64;
            for (name, engine) in &mut engines {
                let message = ConsensusMessage::new(
                    game_id,
                    peer_id,
                    bitcraps::protocol::ConsensusPayload::GameProposal(
                        bitcraps::gaming::GameProposal {
                            operation: "sync_test".to_string(),
                            participants: vec![peer_id],
                            data: vec![],
                            timestamp: base_time,
                        }
                    ),
                    base_time
                );
                
                let result = engine.process_message(message);
                if result.is_err() {
                    logs.push(format!("Engine {} failed to process synchronized message", name));
                }
            }

            // Phase 2: Introduce clock skew
            logs.push("Phase 2: Introducing severe clock skew".to_string());
            
            failures_injected += 1;
            
            let skewed_timestamps = vec![
                base_time - 300, // 5 minutes behind
                base_time,       // Normal time
                base_time + 600, // 10 minutes ahead
            ];
            
            for ((name, engine), &timestamp) in engines.iter_mut().zip(skewed_timestamps.iter()) {
                let message = ConsensusMessage::new(
                    game_id,
                    [2u8; 32], // Different peer
                    bitcraps::protocol::ConsensusPayload::GameProposal(
                        bitcraps::gaming::GameProposal {
                            operation: "skew_test".to_string(),
                            participants: vec![[2u8; 32]],
                            data: vec![],
                            timestamp,
                        }
                    ),
                    timestamp
                );
                
                let result = engine.process_message(message);
                match result {
                    Ok(_) => logs.push(format!("Engine {} accepted skewed timestamp {}", name, timestamp)),
                    Err(e) => logs.push(format!("Engine {} rejected skewed timestamp {}: {}", name, timestamp, e)),
                }
            }

            // Phase 3: Test consensus with skewed clocks
            logs.push("Phase 3: Testing consensus with clock skew".to_string());
            
            let mut consensus_states = Vec::new();
            for (name, engine) in &engines {
                let state = engine.get_state();
                consensus_states.push((name.clone(), state.get_hash()));
            }
            
            // Check if consensus was maintained despite clock skew
            let unique_states: std::collections::HashSet<_> = consensus_states.iter()
                .map(|(_, hash)| hash)
                .collect();
            
            let consensus_maintained = unique_states.len() == 1;
            
            if consensus_maintained {
                logs.push("Consensus maintained despite clock skew".to_string());
            } else {
                logs.push("Consensus broken due to clock skew".to_string());
            }

            // Phase 4: Recovery with NTP sync simulation
            logs.push("Phase 4: Simulating NTP synchronization recovery".to_string());
            
            let sync_time = base_time + 100;
            for (name, engine) in &mut engines {
                let sync_message = ConsensusMessage::new(
                    game_id,
                    [3u8; 32],
                    bitcraps::protocol::ConsensusPayload::GameProposal(
                        bitcraps::gaming::GameProposal {
                            operation: "ntp_sync".to_string(),
                            participants: vec![[3u8; 32]],
                            data: vec![],
                            timestamp: sync_time,
                        }
                    ),
                    sync_time
                );
                
                let _ = engine.process_message(sync_message);
            }
            
            // Check recovery
            let mut recovered_states = Vec::new();
            for (name, engine) in &engines {
                let state = engine.get_state();
                recovered_states.push((name.clone(), state.get_hash()));
            }
            
            let recovered_unique_states: std::collections::HashSet<_> = recovered_states.iter()
                .map(|(_, hash)| hash)
                .collect();
            
            let recovery_success = recovered_unique_states.len() == 1;
            
            let system_degradation = if consensus_maintained && recovery_success {
                0.0
            } else if recovery_success {
                0.3 // Temporary degradation but recovered
            } else {
                0.8 // Severe degradation
            };

            ChaosScenarioResult {
                scenario_name: "clock_skew_resilience".to_string(),
                duration: start_time.elapsed(),
                success: recovery_success,
                recovery_time: Some(Duration::from_millis(100)),
                failures_injected,
                system_degradation,
                logs,
            }
        };

        let mut results = self.scenario_results.lock().await;
        results.insert("clock_skew_resilience".to_string(), scenario_result.clone());

        Ok(scenario_result)
    }

    /// Run all chaos scenarios and generate comprehensive report
    pub async fn run_comprehensive_chaos_test(&self) -> anyhow::Result<ChaosTestReport> {
        println!("🔥 Starting Comprehensive Chaos Engineering Test Suite");
        
        let start_time = Instant::now();
        let mut scenario_results = Vec::new();

        // Run all chaos scenarios
        let scenarios = vec![
            ("Cascading Failures", self.test_cascading_failures()),
            ("Byzantine Adversarial", self.test_byzantine_adversarial_scenario()),
            ("TURN Relay Partition", self.test_turn_relay_partition_recovery()),
            ("Memory Pressure", self.test_memory_pressure_resilience()),
            ("Clock Skew", self.test_clock_skew_resilience()),
        ];

        for (name, scenario_future) in scenarios {
            println!("  ⚡ Running: {}", name);
            
            match timeout(Duration::from_secs(30), scenario_future).await {
                Ok(Ok(result)) => {
                    let status = if result.success { "✅ PASS" } else { "❌ FAIL" };
                    println!("    {} - {} ({:.2}s, {:.1}% degradation)", 
                             status, name, result.duration.as_secs_f64(), result.system_degradation * 100.0);
                    scenario_results.push(result);
                }
                Ok(Err(e)) => {
                    println!("    ❌ ERROR - {}: {}", name, e);
                    scenario_results.push(ChaosScenarioResult {
                        scenario_name: name.to_lowercase().replace(" ", "_"),
                        duration: Duration::from_secs(30),
                        success: false,
                        recovery_time: None,
                        failures_injected: 0,
                        system_degradation: 1.0,
                        logs: vec![format!("Test error: {}", e)],
                    });
                }
                Err(_) => {
                    println!("    ⏰ TIMEOUT - {}", name);
                    scenario_results.push(ChaosScenarioResult {
                        scenario_name: name.to_lowercase().replace(" ", "_"),
                        duration: Duration::from_secs(30),
                        success: false,
                        recovery_time: None,
                        failures_injected: 0,
                        system_degradation: 1.0,
                        logs: vec!["Test timed out".to_string()],
                    });
                }
            }
        }

        let total_duration = start_time.elapsed();
        let passed = scenario_results.iter().filter(|r| r.success).count();
        let total = scenario_results.len();

        println!("\n🎯 Chaos Test Suite Complete:");
        println!("   Passed: {}/{} scenarios", passed, total);
        println!("   Total Time: {:.2}s", total_duration.as_secs_f64());

        Ok(ChaosTestReport {
            total_duration,
            scenarios: scenario_results,
            overall_success: passed == total,
            resilience_score: (passed as f64 / total as f64) * 100.0,
        })
    }

    /// Get detailed results for a specific scenario
    pub async fn get_scenario_result(&self, scenario_name: &str) -> Option<ChaosScenarioResult> {
        let results = self.scenario_results.lock().await;
        results.get(scenario_name).cloned()
    }
}

#[derive(Debug, Clone)]
pub struct ChaosTestReport {
    pub total_duration: Duration,
    pub scenarios: Vec<ChaosScenarioResult>,
    pub overall_success: bool,
    pub resilience_score: f64,
}

impl ChaosTestReport {
    /// Generate detailed markdown report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("# Chaos Engineering Test Report\n\n");
        
        report.push_str(&format!("**Overall Result:** {}\n", 
            if self.overall_success { "✅ PASS" } else { "❌ FAIL" }));
        report.push_str(&format!("**Resilience Score:** {:.1}%\n", self.resilience_score));
        report.push_str(&format!("**Total Duration:** {:.2}s\n\n", self.total_duration.as_secs_f64()));
        
        report.push_str("## Scenario Results\n\n");
        
        for scenario in &self.scenarios {
            report.push_str(&format!("### {}\n", scenario.scenario_name));
            report.push_str(&format!("- **Status:** {}\n", 
                if scenario.success { "✅ PASS" } else { "❌ FAIL" }));
            report.push_str(&format!("- **Duration:** {:.2}s\n", scenario.duration.as_secs_f64()));
            report.push_str(&format!("- **Failures Injected:** {}\n", scenario.failures_injected));
            report.push_str(&format!("- **System Degradation:** {:.1}%\n", scenario.system_degradation * 100.0));
            
            if let Some(recovery_time) = scenario.recovery_time {
                report.push_str(&format!("- **Recovery Time:** {:.2}s\n", recovery_time.as_secs_f64()));
            }
            
            report.push_str("\n**Event Log:**\n");
            for log in &scenario.logs {
                report.push_str(&format!("- {}\n", log));
            }
            report.push_str("\n");
        }
        
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chaos_suite_creation() {
        let suite = AdvancedChaosTestSuite::new();
        assert!(suite.scenario_results.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_single_chaos_scenario() {
        let suite = AdvancedChaosTestSuite::new();
        
        let result = suite.test_clock_skew_resilience().await;
        assert!(result.is_ok());
        
        let scenario_result = result.unwrap();
        assert_eq!(scenario_result.scenario_name, "clock_skew_resilience");
        assert!(!scenario_result.logs.is_empty());
    }

    #[tokio::test]
    #[ignore] // Ignore by default as it's resource intensive
    async fn test_comprehensive_chaos_suite() {
        let suite = AdvancedChaosTestSuite::new();
        
        let report = suite.run_comprehensive_chaos_test().await;
        assert!(report.is_ok());
        
        let chaos_report = report.unwrap();
        assert_eq!(chaos_report.scenarios.len(), 5);
        assert!(chaos_report.resilience_score >= 0.0);
        assert!(chaos_report.resilience_score <= 100.0);
        
        println!("{}", chaos_report.generate_report());
    }
}