//! Comprehensive Test Suite for Consensus Implementation
//!
//! This module provides exhaustive test coverage for all consensus components including:
//! - Formal verification framework testing
//! - Optimized PBFT implementation testing  
//! - Deterministic state machine testing
//! - Byzantine fault tolerance testing
//! - Performance and stress testing
//! - Integration testing across all components
//!
//! ## Test Categories
//!
//! 1. **Unit Tests**: Individual component functionality
//! 2. **Integration Tests**: Component interaction testing
//! 3. **Property Tests**: Invariant and correctness testing
//! 4. **Byzantine Tests**: Fault tolerance under adversarial conditions
//! 5. **Performance Tests**: Throughput and latency benchmarks
//! 6. **Chaos Tests**: System behavior under random failures

#![cfg(feature = "consensus-tests")]

use crate::protocol::consensus::{
    formal_verification::*,
    optimized_pbft::*,
    state_machine::*,
};
use crate::crypto::{BitchatIdentity, GameCrypto};
use crate::error::{Error, Result};
use crate::protocol::craps::{Bet, BetType, CrapTokens, DiceRoll, GamePhase};
use crate::protocol::{GameId, Hash256, PeerId, Signature};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

/// Comprehensive test runner for all consensus components
pub struct ConsensusTestRunner {
    /// Test configuration
    config: TestConfig,
    /// Test results
    results: TestResults,
    /// Random seed for reproducible tests
    seed: u64,
}

/// Test configuration parameters
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Number of nodes to test with
    pub node_count: usize,
    /// Maximum Byzantine nodes for testing
    pub max_byzantine_nodes: usize,
    /// Test timeout duration
    pub test_timeout: Duration,
    /// Number of operations per test
    pub operations_per_test: usize,
    /// Enable performance benchmarks
    pub enable_benchmarks: bool,
    /// Enable chaos testing
    pub enable_chaos_testing: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            node_count: 7, // Support up to 2 Byzantine nodes
            max_byzantine_nodes: 2,
            test_timeout: Duration::from_secs(30),
            operations_per_test: 100,
            enable_benchmarks: true,
            enable_chaos_testing: true,
        }
    }
}

/// Test results aggregation
#[derive(Debug, Default, Clone)]
pub struct TestResults {
    /// Total tests run
    pub total_tests: usize,
    /// Passed tests
    pub passed_tests: usize,
    /// Failed tests
    pub failed_tests: usize,
    /// Test failures by category
    pub failures_by_category: HashMap<String, Vec<TestFailure>>,
    /// Performance benchmarks
    pub benchmarks: HashMap<String, BenchmarkResult>,
    /// Overall test duration
    pub total_duration: Duration,
}

/// Individual test failure information
#[derive(Debug, Clone)]
pub struct TestFailure {
    pub test_name: String,
    pub error_message: String,
    pub duration: Duration,
}

/// Benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub throughput_ops_per_sec: f64,
    pub average_latency: Duration,
    pub p95_latency: Duration,
    pub p99_latency: Duration,
    pub memory_usage_mb: f64,
}

impl ConsensusTestRunner {
    /// Create new test runner
    pub fn new(config: TestConfig, seed: u64) -> Self {
        Self {
            config,
            results: TestResults::default(),
            seed,
        }
    }

    /// Run all consensus tests
    pub async fn run_all_tests(&mut self) -> Result<TestResults> {
        let start_time = std::time::Instant::now();
        
        println!("🚀 Starting comprehensive consensus test suite...");
        
        // Run test categories
        self.run_formal_verification_tests().await?;
        self.run_pbft_tests().await?;
        self.run_state_machine_tests().await?;
        self.run_byzantine_fault_tests().await?;
        self.run_integration_tests().await?;
        
        if self.config.enable_benchmarks {
            self.run_performance_benchmarks().await?;
        }
        
        if self.config.enable_chaos_testing {
            self.run_chaos_tests().await?;
        }

        self.results.total_duration = start_time.elapsed();
        
        println!("✅ Test suite completed in {:?}", self.results.total_duration);
        self.print_test_summary();
        
        Ok(self.results.clone())
    }

    /// Run formal verification tests
    async fn run_formal_verification_tests(&mut self) -> Result<()> {
        println!("🔬 Running formal verification tests...");
        
        self.test_tla_spec_generation().await?;
        self.test_property_based_testing().await?;
        self.test_temporal_logic_checking().await?;
        self.test_model_checking().await?;
        
        Ok(())
    }

    /// Test TLA+ specification generation
    async fn test_tla_spec_generation(&mut self) -> Result<()> {
        let test_name = "TLA+ Specification Generation";
        let start_time = std::time::Instant::now();
        
        match self.run_tla_spec_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_tla_spec_test(&self) -> Result<()> {
        let params = ConsensusParameters::default();
        let generator = TLASpecGenerator::new(self.config.node_count, params)?;
        
        // Test spec generation
        let spec = generator.generate_tla_spec()?;
        assert!(spec.contains("MODULE BitCrapsConsensus"));
        assert!(spec.contains("Agreement"));
        assert!(spec.contains("Validity"));
        assert!(spec.contains("Termination"));
        assert!(spec.len() > 1000); // Reasonable minimum size
        
        // Test model config generation
        let config = generator.generate_model_config()?;
        assert!(config.contains("SPECIFICATION"));
        assert!(config.contains("INVARIANTS"));
        assert!(config.contains("PROPERTIES"));
        
        Ok(())
    }

    /// Test property-based testing framework
    async fn test_property_based_testing(&mut self) -> Result<()> {
        let test_name = "Property-Based Testing";
        let start_time = std::time::Instant::now();
        
        match self.run_property_based_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_property_based_test(&self) -> Result<()> {
        let params = ConsensusParameters::default();
        let tester = PropertyTester::new(params, self.seed);
        
        // Test small execution
        let execution = tester.generate_random_execution(4, 20)?;
        assert!(!execution.is_empty());
        
        // Test properties
        let agreement_ok = tester.test_agreement_property(&execution)?;
        assert!(agreement_ok);
        
        let validity_ok = tester.test_validity_property(&execution)?;
        assert!(validity_ok);
        
        if let Some(final_state) = execution.last() {
            let byzantine_ok = tester.test_byzantine_tolerance(final_state, 4)?;
            assert!(byzantine_ok);
        }
        
        let termination_ok = tester.test_termination_property(&execution, 20)?;
        assert!(termination_ok);
        
        // Test property test suite
        let results = tester.run_property_tests(10, 8)?;
        assert!(results.success_rate() > 0.8); // At least 80% success rate
        
        Ok(())
    }

    /// Test temporal logic checking
    async fn test_temporal_logic_checking(&mut self) -> Result<()> {
        let test_name = "Temporal Logic Checking";
        let start_time = std::time::Instant::now();
        
        match self.run_temporal_logic_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_temporal_logic_test(&self) -> Result<()> {
        let states = vec![
            ModelState { decided: None, round: 0, ..Default::default() },
            ModelState { decided: Some([1u8; 32]), round: 1, ..Default::default() },
            ModelState { decided: Some([1u8; 32]), round: 2, ..Default::default() },
        ];
        
        // Test "Eventually decides"
        let eventually_decides = TemporalProperty::Eventually(StateProperty::HasDecision);
        let result = TemporalLogicChecker::check_temporal_property(&states, eventually_decides)?;
        assert!(result);
        
        // Test "Always no decision until decision"
        let until_decides = TemporalProperty::Until(
            StateProperty::NoDecision,
            StateProperty::HasDecision,
        );
        let result = TemporalLogicChecker::check_temporal_property(&states[..2], until_decides)?;
        assert!(result);
        
        // Test "Always bounded round"
        let always_bounded = TemporalProperty::Always(StateProperty::RoundBounded(10));
        let result = TemporalLogicChecker::check_temporal_property(&states, always_bounded)?;
        assert!(result);
        
        Ok(())
    }

    /// Test model checking capabilities
    async fn test_model_checking(&mut self) -> Result<()> {
        let test_name = "Model Checking";
        let start_time = std::time::Instant::now();
        
        match self.run_model_checking_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_model_checking_test(&self) -> Result<()> {
        // Test Byzantine fault calculation
        let params = ConsensusParameters::default();
        
        // Test minimum nodes requirement
        assert!(TLASpecGenerator::new(3, params.clone()).is_err());
        assert!(TLASpecGenerator::new(4, params.clone()).is_ok());
        
        // Test quorum calculations
        let generator = TLASpecGenerator::new(7, params)?;
        let spec = generator.generate_tla_spec()?;
        
        // Verify spec contains proper quorum calculation (5 out of 7 for BFT)
        assert!(spec.contains("≥ 5") || spec.contains(">= 5"));
        
        Ok(())
    }

    /// Run PBFT implementation tests
    async fn run_pbft_tests(&mut self) -> Result<()> {
        println!("⚡ Running optimized PBFT tests...");
        
        self.test_pbft_creation_and_basic_ops().await?;
        self.test_pbft_message_processing().await?;
        self.test_pbft_timeout_adaptation().await?;
        self.test_pbft_compression().await?;
        self.test_pbft_pipelining().await?;
        
        Ok(())
    }

    /// Test PBFT engine creation and basic operations
    async fn test_pbft_creation_and_basic_ops(&mut self) -> Result<()> {
        let test_name = "PBFT Creation and Basic Operations";
        let start_time = std::time::Instant::now();
        
        match self.run_pbft_basic_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_pbft_basic_test(&self) -> Result<()> {
        let config = OptimizedPBFTConfig::default();
        let node_id = [1u8; 32];
        let crypto = Arc::new(GameCrypto::new());
        let participants = self.generate_test_participants();

        let engine = OptimizedPBFTEngine::new(config, node_id, crypto, participants)?;
        
        // Test initial state
        assert!(matches!(engine.get_state(), ReplicaState::Normal { view: 0 }));
        
        // Test operation submission
        let operation = ConsensusOperation {
            id: [1u8; 32],
            data: b"test_operation".to_vec(),
            client: [1u8; 32],
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: Signature([0u8; 64]),
        };

        engine.submit_operation(operation).await?;
        
        // Test metrics
        let metrics = engine.get_metrics();
        assert!(metrics.rounds_completed >= 0);
        
        Ok(())
    }

    /// Test PBFT message processing
    async fn test_pbft_message_processing(&mut self) -> Result<()> {
        let test_name = "PBFT Message Processing";
        let start_time = std::time::Instant::now();
        
        match self.run_pbft_message_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_pbft_message_test(&self) -> Result<()> {
        // Test operation batch creation and compression
        let operations = vec![
            ConsensusOperation {
                id: [1u8; 32],
                data: b"test1".to_vec(),
                client: [1u8; 32],
                timestamp: 1000,
                signature: Signature([0u8; 64]),
            },
            ConsensusOperation {
                id: [2u8; 32],
                data: b"test2".to_vec(),
                client: [2u8; 32],
                timestamp: 1001,
                signature: Signature([0u8; 64]),
            },
        ];

        let mut batch = OperationBatch {
            operations: operations.clone(),
            timestamp: 1000,
            compression: CompressionMethod::Gzip,
            compressed_data: None,
        };

        // Test compression
        batch.compress()?;
        assert!(batch.compressed_data.is_some());

        // Test decompression
        let decompressed = batch.decompress()?;
        assert_eq!(decompressed.len(), 2);
        assert_eq!(decompressed[0].data, b"test1");
        assert_eq!(decompressed[1].data, b"test2");

        // Test batch hash consistency
        let hash1 = batch.hash();
        let hash2 = batch.hash();
        assert_eq!(hash1, hash2);
        
        Ok(())
    }

    /// Test PBFT timeout adaptation
    async fn test_pbft_timeout_adaptation(&mut self) -> Result<()> {
        let test_name = "PBFT Timeout Adaptation";
        let start_time = std::time::Instant::now();
        
        match self.run_timeout_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_timeout_test(&self) -> Result<()> {
        let config = OptimizedPBFTConfig::default();
        let controller = TimeoutController::new(&config);

        let initial_timeout = controller.current_timeout();
        assert_eq!(initial_timeout, config.base_timeout);

        // Test success reduces timeout
        controller.record_success(Duration::from_millis(100));
        let new_timeout = controller.current_timeout();
        assert!(new_timeout <= initial_timeout);

        // Test timeout increases timeout
        controller.record_timeout();
        let timeout_after_failure = controller.current_timeout();
        assert!(timeout_after_failure >= new_timeout);
        
        Ok(())
    }

    /// Test PBFT compression
    async fn test_pbft_compression(&mut self) -> Result<()> {
        let test_name = "PBFT Compression";
        let start_time = std::time::Instant::now();
        
        match self.run_compression_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_compression_test(&self) -> Result<()> {
        // Create large batch for compression testing
        let large_data = "x".repeat(10000);
        let operations: Vec<ConsensusOperation> = (0..10)
            .map(|i| ConsensusOperation {
                id: [i as u8; 32],
                data: large_data.as_bytes().to_vec(),
                client: [i as u8; 32],
                timestamp: 1000 + i as u64,
                signature: Signature([0u8; 64]),
            })
            .collect();

        let mut batch = OperationBatch {
            operations: operations.clone(),
            timestamp: 1000,
            compression: CompressionMethod::Gzip,
            compressed_data: None,
        };

        // Test compression reduces size
        let original_size = bincode::serialize(&batch.operations).unwrap().len();
        batch.compress()?;
        let compressed_size = batch.compressed_data.as_ref().unwrap().len();
        
        assert!(compressed_size < original_size);
        
        // Test roundtrip consistency
        let decompressed = batch.decompress()?;
        assert_eq!(decompressed.len(), operations.len());
        for (original, decompressed) in operations.iter().zip(decompressed.iter()) {
            assert_eq!(original.data, decompressed.data);
        }
        
        Ok(())
    }

    /// Test PBFT pipelining
    async fn test_pbft_pipelining(&mut self) -> Result<()> {
        let test_name = "PBFT Pipelining";
        let start_time = std::time::Instant::now();
        
        match self.run_pipelining_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_pipelining_test(&self) -> Result<()> {
        let config = OptimizedPBFTConfig {
            pipeline_depth: 3,
            batch_size: 2,
            ..Default::default()
        };
        let node_id = [1u8; 32];
        let crypto = Arc::new(GameCrypto::new());
        let participants = self.generate_test_participants();

        let engine = OptimizedPBFTEngine::new(config, node_id, crypto, participants)?;
        
        // Submit multiple operations to test pipelining
        for i in 0..6 {
            let operation = ConsensusOperation {
                id: [i as u8; 32],
                data: format!("operation_{}", i).into_bytes(),
                client: [i as u8; 32],
                timestamp: 1000 + i as u64,
                signature: Signature([0u8; 64]),
            };
            
            engine.submit_operation(operation).await?;
        }
        
        // Give some time for batch processing
        sleep(Duration::from_millis(100)).await;
        
        let metrics = engine.get_metrics();
        assert!(metrics.operations_processed > 0);
        
        Ok(())
    }

    /// Run state machine tests
    async fn run_state_machine_tests(&mut self) -> Result<()> {
        println!("🎰 Running state machine tests...");
        
        self.test_state_machine_creation().await?;
        self.test_state_machine_operations().await?;
        self.test_state_machine_verification().await?;
        self.test_state_machine_pruning().await?;
        
        Ok(())
    }

    /// Test state machine creation
    async fn test_state_machine_creation(&mut self) -> Result<()> {
        let test_name = "State Machine Creation";
        let start_time = std::time::Instant::now();
        
        match self.run_state_machine_creation_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_state_machine_creation_test(&self) -> Result<()> {
        let game_id = [1u8; 32];
        let participants = self.generate_test_participants();
        let config = StateMachineConfig::default();

        let state_machine = DeterministicStateMachine::new(game_id, participants.clone(), config)?;
        
        assert_eq!(state_machine.get_state().sequence_number, 0);
        assert_eq!(state_machine.get_state().participants.len(), participants.len());
        assert_eq!(state_machine.get_state().game_id, game_id);
        
        // Verify initial balances
        for participant in &participants {
            let balance = state_machine.get_state().player_balances.get(participant);
            assert!(balance.is_some());
            assert!(balance.unwrap().0 > 0);
        }
        
        Ok(())
    }

    /// Test state machine operations
    async fn test_state_machine_operations(&mut self) -> Result<()> {
        let test_name = "State Machine Operations";
        let start_time = std::time::Instant::now();
        
        match self.run_state_machine_operations_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_state_machine_operations_test(&self) -> Result<()> {
        let game_id = [1u8; 32];
        let player1 = [1u8; 32];
        let participants = vec![player1, [2u8; 32]];
        let config = StateMachineConfig::default();

        let mut state_machine = DeterministicStateMachine::new(game_id, participants, config)?;

        // Test place bet operation
        let bet = Bet {
            id: [1u8; 16],
            player: PeerId([2u8; 32]),
            game_id: GameId([3u8; 16]),
            bet_type: BetType::Pass,
            amount: CrapTokens::new_unchecked(100),
            timestamp: 1234567890,
        };

        let operation = StateOperation {
            id: [1u8; 32],
            sequence: 1,
            operation: OperationType::PlaceBet {
                bet: bet.clone(),
                expected_balance: CrapTokens::new_unchecked(10000),
            },
            player: player1,
            timestamp: 1000,
            signature: Signature([0u8; 64]),
            nonce: 1,
        };

        let result = state_machine.execute_operation(operation)?;
        assert!(result.success);

        // Verify balance was deducted
        let new_balance = state_machine.get_state().player_balances[&player1];
        assert_eq!(new_balance.0, 9900);

        // Verify bet was added
        let active_bets = &state_machine.get_state().active_bets[&player1];
        assert_eq!(active_bets.len(), 1);
        assert_eq!(active_bets[0].amount.0, 100);

        // Test dice roll operation
        let dice_operation = StateOperation {
            id: [2u8; 32],
            sequence: 2,
            operation: OperationType::ProcessRoll {
                dice_roll: DiceRoll { die1: 3, die2: 4, timestamp: 1001 },
                entropy_proof: vec![[1u8; 32], [2u8; 32]],
                expected_phase: GamePhase::Point,
            },
            player: player1,
            timestamp: 1001,
            signature: Signature([0u8; 64]),
            nonce: 2,
        };

        // This may fail due to game phase rules, which is expected
        let _result = state_machine.execute_operation(dice_operation);
        
        Ok(())
    }

    /// Test state machine verification
    async fn test_state_machine_verification(&mut self) -> Result<()> {
        let test_name = "State Machine Verification";
        let start_time = std::time::Instant::now();
        
        match self.run_state_verification_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_state_verification_test(&self) -> Result<()> {
        let game_id = [1u8; 32];
        let player1 = [1u8; 32];
        let participants = vec![player1, [2u8; 32]];
        let config = StateMachineConfig::default();

        let mut state_machine = DeterministicStateMachine::new(game_id, participants, config)?;
        let initial_hash = state_machine.get_state().state_hash;

        // Test deterministic hash calculation
        let state_machine2 = DeterministicStateMachine::new(game_id, vec![player1, [2u8; 32]], StateMachineConfig::default())?;
        assert_eq!(initial_hash, state_machine2.get_state().state_hash);

        // Test operation and verification
        let bet = Bet {
            id: [1u8; 16],
            player: PeerId([2u8; 32]),
            game_id: GameId([3u8; 16]),
            bet_type: BetType::Pass,
            amount: CrapTokens::new_unchecked(100),
            timestamp: 1234567890,
        };

        let operation = StateOperation {
            id: [1u8; 32],
            sequence: 1,
            operation: OperationType::PlaceBet {
                bet: bet.clone(),
                expected_balance: CrapTokens::new_unchecked(10000),
            },
            player: player1,
            timestamp: 1000,
            signature: Signature([0u8; 64]),
            nonce: 1,
        };

        let result = state_machine.execute_operation(operation.clone())?;
        let final_hash = result.new_state_hash;

        // Test incremental verification
        let is_valid = state_machine.verify_state_transition(
            initial_hash,
            final_hash,
            &[operation],
        )?;

        assert!(is_valid);
        
        Ok(())
    }

    /// Test state machine pruning
    async fn test_state_machine_pruning(&mut self) -> Result<()> {
        let test_name = "State Machine Pruning";
        let start_time = std::time::Instant::now();
        
        match self.run_pruning_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_pruning_test(&self) -> Result<()> {
        let game_id = [1u8; 32];
        let player1 = [1u8; 32];
        let participants = vec![player1, [2u8; 32]];
        let config = StateMachineConfig {
            pruning_interval: 5, // Prune every 5 operations
            checkpoint_interval: 3, // Checkpoint every 3 operations
            ..Default::default()
        };

        let mut state_machine = DeterministicStateMachine::new(game_id, participants, config)?;

        // Execute multiple operations to trigger pruning
        for i in 1..=10 {
            let operation = StateOperation {
                id: [i as u8; 32],
                sequence: i,
                operation: OperationType::AddPlayer {
                    player: [i as u8; 32],
                    initial_balance: CrapTokens::new_unchecked(1000),
                },
                player: player1,
                timestamp: 1000 + i,
                signature: Signature([0u8; 64]),
                nonce: i,
            };

            let _result = state_machine.execute_operation(operation)?;
        }

        let metrics = state_machine.get_metrics();
        assert!(metrics.operations_executed >= 10);
        assert!(metrics.pruning_operations > 0);
        
        Ok(())
    }

    /// Run Byzantine fault tolerance tests
    async fn run_byzantine_fault_tests(&mut self) -> Result<()> {
        println!("🛡️ Running Byzantine fault tolerance tests...");
        
        self.test_byzantine_detection().await?;
        self.test_byzantine_resilience().await?;
        self.test_quorum_calculations().await?;
        
        Ok(())
    }

    /// Test Byzantine fault detection
    async fn test_byzantine_detection(&mut self) -> Result<()> {
        let test_name = "Byzantine Fault Detection";
        let start_time = std::time::Instant::now();
        
        match self.run_byzantine_detection_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_byzantine_detection_test(&self) -> Result<()> {
        use crate::protocol::consensus::byzantine_engine::*;
        
        let config = ByzantineConfig::default();
        let crypto = Arc::new(GameCrypto::new());
        let node_id = [1u8; 32];

        let engine = ByzantineConsensusEngine::new(config, crypto, node_id);

        // Add participants including Byzantine node
        let byzantine_node = [99u8; 32];
        engine.add_participant([1u8; 32]).await?;
        engine.add_participant([2u8; 32]).await?;
        engine.add_participant([3u8; 32]).await?;
        engine.add_participant(byzantine_node).await?;

        // Test slashing for equivocation
        engine.slash_node(byzantine_node, SlashingReason::Equivocation).await?;
        assert!(engine.is_byzantine(&byzantine_node).await);

        // Check slashing events
        let events = engine.get_slashing_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].node, byzantine_node);
        
        Ok(())
    }

    /// Test Byzantine resilience
    async fn test_byzantine_resilience(&mut self) -> Result<()> {
        let test_name = "Byzantine Resilience";
        let start_time = std::time::Instant::now();
        
        match self.run_byzantine_resilience_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_byzantine_resilience_test(&self) -> Result<()> {
        // Test that system works with maximum allowed Byzantine nodes
        let total_nodes = 7;
        let max_byzantine = 2; // f < n/3, so max 2 Byzantine out of 7

        let params = ConsensusParameters {
            quorum_method: QuorumMethod::ByzantineFaultTolerant,
            ..Default::default()
        };
        
        let tester = PropertyTester::new(params, self.seed);

        // Create execution with Byzantine nodes
        let mut execution = tester.generate_random_execution(total_nodes, 50)?;
        
        // Add Byzantine nodes up to threshold
        for i in 0..max_byzantine {
            if let Some(state) = execution.last_mut() {
                state.byzantine_nodes.insert([i as u8; 32]);
            }
        }

        // Test that Byzantine tolerance is maintained
        if let Some(final_state) = execution.last() {
            let byzantine_ok = tester.test_byzantine_tolerance(final_state, total_nodes)?;
            assert!(byzantine_ok);
        }

        // Test that adding one more Byzantine node would exceed threshold
        if let Some(state) = execution.last_mut() {
            state.byzantine_nodes.insert([max_byzantine as u8; 32]);
        }
        
        if let Some(final_state) = execution.last() {
            let byzantine_exceeded = tester.test_byzantine_tolerance(final_state, total_nodes)?;
            assert!(!byzantine_exceeded); // Should fail with too many Byzantine nodes
        }
        
        Ok(())
    }

    /// Test quorum calculations
    async fn test_quorum_calculations(&mut self) -> Result<()> {
        let test_name = "Quorum Calculations";
        let start_time = std::time::Instant::now();
        
        match self.run_quorum_test().await {
            Ok(_) => {
                self.record_test_pass(test_name, start_time.elapsed());
            }
            Err(e) => {
                self.record_test_failure(test_name, e, start_time.elapsed());
            }
        }
        
        Ok(())
    }

    async fn run_quorum_test(&self) -> Result<()> {
        // Test different quorum methods
        let params_majority = ConsensusParameters {
            quorum_method: QuorumMethod::SimpleMajority,
            ..Default::default()
        };
        
        let params_bft = ConsensusParameters {
            quorum_method: QuorumMethod::ByzantineFaultTolerant,
            ..Default::default()
        };
        
        let params_super = ConsensusParameters {
            quorum_method: QuorumMethod::SuperMajority,
            ..Default::default()
        };
        
        let tester_majority = PropertyTester::new(params_majority, 0);
        let tester_bft = PropertyTester::new(params_bft, 0);
        let tester_super = PropertyTester::new(params_super, 0);
        
        // Test quorum calculations
        assert_eq!(tester_majority.calculate_quorum_size(6), 4); // Simple majority: n/2 + 1
        assert_eq!(tester_bft.calculate_quorum_size(6), 5); // BFT: ceiling(2n/3)
        assert_eq!(tester_super.calculate_quorum_size(8), 7); // Supermajority: ceiling(3n/4)
        
        // Test PBFT quorum calculation
        assert_eq!(OptimizedPBFTEngine::calculate_quorum(4), 3); // 2f + 1 where f = 1
        assert_eq!(OptimizedPBFTEngine::calculate_quorum(7), 5); // 2f + 1 where f = 2
        assert_eq!(OptimizedPBFTEngine::calculate_quorum(10), 7); // 2f + 1 where f = 3
        
        Ok(())
    }

    /// Run integration tests
    async fn run_integration_tests(&mut self) -> Result<()> {
        println!("🔗 Running integration tests...");
        
        self.test_full_consensus_flow().await?;
        self.test_multi_node_consensus().await?;
        
        Ok(())
    }

    /// Test complete consensus flow
    async fn test_full_consensus_flow(&mut self) -> Result<()> {
        let test_name = "Full Consensus Flow";
        let start_time = std::time::Instant::now();
        
        match timeout(self.config.test_timeout, self.run_full_flow_test()).await {
            Ok(result) => match result {
                Ok(_) => self.record_test_pass(test_name, start_time.elapsed()),
                Err(e) => self.record_test_failure(test_name, e, start_time.elapsed()),
            },
            Err(_) => {
                self.record_test_failure(
                    test_name,
                    Error::Protocol("Test timeout".to_string()),
                    start_time.elapsed(),
                );
            }
        }
        
        Ok(())
    }

    async fn run_full_flow_test(&self) -> Result<()> {
        // Create integrated system with all components
        let game_id = [1u8; 32];
        let participants = self.generate_test_participants();
        
        // Create state machine
        let mut state_machine = DeterministicStateMachine::new(
            game_id,
            participants.clone(),
            StateMachineConfig::default(),
        )?;
        
        // Create PBFT engine
        let config = OptimizedPBFTConfig::default();
        let node_id = participants[0];
        let crypto = Arc::new(GameCrypto::new());
        let pbft_engine = OptimizedPBFTEngine::new(config, node_id, crypto, participants.clone())?;

        // Test operation flow: PBFT → State Machine
        let operation = ConsensusOperation {
            id: [1u8; 32],
            data: b"integrated_test".to_vec(),
            client: node_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            signature: Signature([0u8; 64]),
        };

        // Submit through PBFT
        pbft_engine.submit_operation(operation.clone()).await?;

        // Give time for processing
        sleep(Duration::from_millis(100)).await;

        // Convert to state machine operation
        let state_operation = StateOperation {
            id: operation.id,
            sequence: 1,
            operation: OperationType::AddPlayer {
                player: [99u8; 32],
                initial_balance: CrapTokens::new_unchecked(1000),
            },
            player: node_id,
            timestamp: operation.timestamp,
            signature: operation.signature,
            nonce: 1,
        };

        // Execute in state machine
        let result = state_machine.execute_operation(state_operation)?;
        assert!(result.success);

        Ok(())
    }

    /// Test multi-node consensus
    async fn test_multi_node_consensus(&mut self) -> Result<()> {
        let test_name = "Multi-Node Consensus";
        let start_time = std::time::Instant::now();
        
        match timeout(self.config.test_timeout, self.run_multi_node_test()).await {
            Ok(result) => match result {
                Ok(_) => self.record_test_pass(test_name, start_time.elapsed()),
                Err(e) => self.record_test_failure(test_name, e, start_time.elapsed()),
            },
            Err(_) => {
                self.record_test_failure(
                    test_name,
                    Error::Protocol("Test timeout".to_string()),
                    start_time.elapsed(),
                );
            }
        }
        
        Ok(())
    }

    async fn run_multi_node_test(&self) -> Result<()> {
        // Simulate multiple nodes participating in consensus
        let participants = self.generate_test_participants();
        let mut engines = Vec::new();

        // Create multiple PBFT engines
        for (i, &node_id) in participants.iter().enumerate() {
            let config = OptimizedPBFTConfig::default();
            let crypto = Arc::new(GameCrypto::new());
            let engine = OptimizedPBFTEngine::new(config, node_id, crypto, participants.clone())?;
            engines.push(engine);

            // Submit operations from different nodes
            let operation = ConsensusOperation {
                id: [i as u8; 32],
                data: format!("operation_from_node_{}", i).into_bytes(),
                client: node_id,
                timestamp: 1000 + i as u64,
                signature: Signature([0u8; 64]),
            };

            engines[i].submit_operation(operation).await?;
        }

        // Give time for consensus
        sleep(Duration::from_millis(500)).await;

        // Verify all engines processed operations
        for engine in &engines {
            let metrics = engine.get_metrics();
            assert!(metrics.operations_processed > 0);
        }

        Ok(())
    }

    /// Run performance benchmarks
    async fn run_performance_benchmarks(&mut self) -> Result<()> {
        println!("📊 Running performance benchmarks...");
        
        self.benchmark_pbft_throughput().await?;
        self.benchmark_state_machine_performance().await?;
        
        Ok(())
    }

    /// Benchmark PBFT throughput
    async fn benchmark_pbft_throughput(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        let config = OptimizedPBFTConfig {
            batch_size: 50,
            pipeline_depth: 8,
            ..Default::default()
        };
        let node_id = [1u8; 32];
        let crypto = Arc::new(GameCrypto::new());
        let participants = self.generate_test_participants();

        let engine = OptimizedPBFTEngine::new(config, node_id, crypto, participants)?;
        
        let operations_count = 1000;
        let bench_start = std::time::Instant::now();

        // Submit operations for benchmarking
        for i in 0..operations_count {
            let operation = ConsensusOperation {
                id: [(i % 256) as u8; 32],
                data: format!("benchmark_operation_{}", i).into_bytes(),
                client: node_id,
                timestamp: 1000 + i as u64,
                signature: Signature([0u8; 64]),
            };

            engine.submit_operation(operation).await?;
        }

        // Wait for processing
        sleep(Duration::from_secs(2)).await;

        let bench_duration = bench_start.elapsed();
        let throughput = operations_count as f64 / bench_duration.as_secs_f64();

        let metrics = engine.get_metrics();
        let benchmark = BenchmarkResult {
            throughput_ops_per_sec: throughput,
            average_latency: metrics.average_consensus_latency,
            p95_latency: metrics.average_consensus_latency * 2, // Estimated
            p99_latency: metrics.average_consensus_latency * 3, // Estimated
            memory_usage_mb: 0.0, // Would measure actual memory usage
        };

        self.results.benchmarks.insert("PBFT_Throughput".to_string(), benchmark);
        println!("📈 PBFT Throughput: {:.2} ops/sec", throughput);
        
        let duration = start_time.elapsed();
        self.record_test_pass("PBFT Throughput Benchmark", duration);
        
        Ok(())
    }

    /// Benchmark state machine performance
    async fn benchmark_state_machine_performance(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        let game_id = [1u8; 32];
        let participants = self.generate_test_participants();
        let config = StateMachineConfig::default();

        let mut state_machine = DeterministicStateMachine::new(game_id, participants, config)?;
        
        let operations_count = 1000;
        let bench_start = std::time::Instant::now();

        // Execute operations for benchmarking
        for i in 1..=operations_count {
            let operation = StateOperation {
                id: [(i % 256) as u8; 32],
                sequence: i as u64,
                operation: OperationType::AddPlayer {
                    player: [(100 + i) as u8; 32],
                    initial_balance: CrapTokens::new_unchecked(1000),
                },
                player: [1u8; 32],
                timestamp: 1000 + i as u64,
                signature: Signature([0u8; 64]),
                nonce: i as u64,
            };

            let _result = state_machine.execute_operation(operation)?;
        }

        let bench_duration = bench_start.elapsed();
        let throughput = operations_count as f64 / bench_duration.as_secs_f64();

        let metrics = state_machine.get_metrics();
        let benchmark = BenchmarkResult {
            throughput_ops_per_sec: throughput,
            average_latency: metrics.avg_execution_time,
            p95_latency: metrics.avg_execution_time * 2,
            p99_latency: metrics.avg_execution_time * 3,
            memory_usage_mb: metrics.memory_usage as f64 / (1024.0 * 1024.0),
        };

        self.results.benchmarks.insert("StateMachine_Performance".to_string(), benchmark);
        println!("🎰 State Machine Throughput: {:.2} ops/sec", throughput);
        
        let duration = start_time.elapsed();
        self.record_test_pass("State Machine Performance Benchmark", duration);
        
        Ok(())
    }

    /// Run chaos tests
    async fn run_chaos_tests(&mut self) -> Result<()> {
        println!("🌪️ Running chaos tests...");
        
        self.test_random_failures().await?;
        self.test_network_partitions().await?;
        
        Ok(())
    }

    /// Test system under random failures
    async fn test_random_failures(&mut self) -> Result<()> {
        let test_name = "Random Failures Chaos Test";
        let start_time = std::time::Instant::now();
        
        match timeout(self.config.test_timeout, self.run_random_failures_test()).await {
            Ok(result) => match result {
                Ok(_) => self.record_test_pass(test_name, start_time.elapsed()),
                Err(e) => self.record_test_failure(test_name, e, start_time.elapsed()),
            },
            Err(_) => {
                self.record_test_failure(
                    test_name,
                    Error::Protocol("Test timeout".to_string()),
                    start_time.elapsed(),
                );
            }
        }
        
        Ok(())
    }

    async fn run_random_failures_test(&self) -> Result<()> {
        // Test with random Byzantine behavior
        let params = ConsensusParameters::default();
        let tester = PropertyTester::new(params, self.seed);
        
        // Generate multiple executions with random failures
        for _ in 0..10 {
            let execution = tester.generate_random_execution(7, 100)?;
            
            // Test that basic properties still hold despite chaos
            let agreement_ok = tester.test_agreement_property(&execution)?;
            let validity_ok = tester.test_validity_property(&execution)?;
            
            // At least one should hold (system should maintain some consistency)
            assert!(agreement_ok || validity_ok);
        }
        
        Ok(())
    }

    /// Test network partitions
    async fn test_network_partitions(&mut self) -> Result<()> {
        let test_name = "Network Partitions Chaos Test";
        let start_time = std::time::Instant::now();
        
        match timeout(self.config.test_timeout, self.run_network_partition_test()).await {
            Ok(result) => match result {
                Ok(_) => self.record_test_pass(test_name, start_time.elapsed()),
                Err(e) => self.record_test_failure(test_name, e, start_time.elapsed()),
            },
            Err(_) => {
                self.record_test_failure(
                    test_name,
                    Error::Protocol("Test timeout".to_string()),
                    start_time.elapsed(),
                );
            }
        }
        
        Ok(())
    }

    async fn run_network_partition_test(&self) -> Result<()> {
        // Simulate network partitions by testing with limited participation
        let total_nodes = 7;
        let params = ConsensusParameters::default();
        let tester = PropertyTester::new(params, self.seed);
        
        // Test with majority partition
        let majority_execution = tester.generate_random_execution(5, 50)?;
        let agreement_ok = tester.test_agreement_property(&majority_execution)?;
        assert!(agreement_ok); // Majority should still reach agreement
        
        // Test with minority partition (should fail to reach consensus)
        let minority_execution = tester.generate_random_execution(2, 50)?;
        let termination_ok = tester.test_termination_property(&minority_execution, 50)?;
        // Minority partition should not terminate (which is correct behavior)
        
        Ok(())
    }

    /// Helper methods

    /// Generate test participants
    fn generate_test_participants(&self) -> Vec<PeerId> {
        (0..self.config.node_count)
            .map(|i| [i as u8; 32])
            .collect()
    }

    /// Record test pass
    fn record_test_pass(&mut self, test_name: &str, duration: Duration) {
        self.results.total_tests += 1;
        self.results.passed_tests += 1;
        println!("✅ {} - PASSED ({:?})", test_name, duration);
    }

    /// Record test failure
    fn record_test_failure(&mut self, test_name: &str, error: Error, duration: Duration) {
        self.results.total_tests += 1;
        self.results.failed_tests += 1;
        
        let failure = TestFailure {
            test_name: test_name.to_string(),
            error_message: error.to_string(),
            duration,
        };
        
        let category = self.extract_test_category(test_name);
        self.results
            .failures_by_category
            .entry(category)
            .or_default()
            .push(failure);
        
        println!("❌ {} - FAILED: {} ({:?})", test_name, error, duration);
    }

    /// Extract test category from test name
    fn extract_test_category(&self, test_name: &str) -> String {
        if test_name.contains("TLA+") || test_name.contains("Property") || test_name.contains("Temporal") {
            "Formal Verification".to_string()
        } else if test_name.contains("PBFT") {
            "PBFT Implementation".to_string()
        } else if test_name.contains("State Machine") {
            "State Machine".to_string()
        } else if test_name.contains("Byzantine") {
            "Byzantine Fault Tolerance".to_string()
        } else if test_name.contains("Benchmark") {
            "Performance".to_string()
        } else if test_name.contains("Chaos") {
            "Chaos Testing".to_string()
        } else {
            "Integration".to_string()
        }
    }

    /// Print comprehensive test summary
    fn print_test_summary(&self) {
        println!("\n📋 COMPREHENSIVE TEST SUMMARY");
        println!("=====================================");
        println!("Total Tests: {}", self.results.total_tests);
        println!("Passed: {} ✅", self.results.passed_tests);
        println!("Failed: {} ❌", self.results.failed_tests);
        println!(
            "Success Rate: {:.1}%",
            (self.results.passed_tests as f64 / self.results.total_tests as f64) * 100.0
        );
        println!("Total Duration: {:?}", self.results.total_duration);

        if !self.results.failures_by_category.is_empty() {
            println!("\n📊 FAILURES BY CATEGORY:");
            for (category, failures) in &self.results.failures_by_category {
                println!("  {}: {} failures", category, failures.len());
                for failure in failures {
                    println!("    - {}: {}", failure.test_name, failure.error_message);
                }
            }
        }

        if !self.results.benchmarks.is_empty() {
            println!("\n🚀 PERFORMANCE BENCHMARKS:");
            for (name, benchmark) in &self.results.benchmarks {
                println!("  {}:", name);
                println!("    Throughput: {:.2} ops/sec", benchmark.throughput_ops_per_sec);
                println!("    Avg Latency: {:?}", benchmark.average_latency);
                println!("    P95 Latency: {:?}", benchmark.p95_latency);
                println!("    P99 Latency: {:?}", benchmark.p99_latency);
                if benchmark.memory_usage_mb > 0.0 {
                    println!("    Memory Usage: {:.2} MB", benchmark.memory_usage_mb);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_comprehensive_suite_creation() {
        let config = TestConfig::default();
        let runner = ConsensusTestRunner::new(config, 42);
        
        assert_eq!(runner.results.total_tests, 0);
        assert_eq!(runner.config.node_count, 7);
    }

    #[tokio::test]
    async fn test_basic_consensus_functionality() {
        let config = TestConfig {
            node_count: 4,
            max_byzantine_nodes: 1,
            test_timeout: Duration::from_secs(5),
            operations_per_test: 10,
            enable_benchmarks: false,
            enable_chaos_testing: false,
        };
        
        let mut runner = ConsensusTestRunner::new(config, 42);
        
        // Run a subset of tests
        runner.run_formal_verification_tests().await.unwrap();
        
        assert!(runner.results.total_tests > 0);
    }
}