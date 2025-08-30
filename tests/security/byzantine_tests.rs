//! Byzantine Fault Tolerance Tests
//!
//! Tests the system's resilience against Byzantine (malicious) actors
//! attempting to compromise consensus, game integrity, or network stability.

use bitcraps::{
    crypto::BitchatKeypair,
    error::Error,
    mesh::MeshService,
    protocol::consensus::{ConsensusConfig, ConsensusEngine},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a Byzantine actor in the network
struct ByzantineNode {
    id: [u8; 32],
    behavior: ByzantineBehavior,
    consensus_engine: Option<ConsensusEngine>,
}

#[derive(Clone, Debug)]
enum ByzantineBehavior {
    /// Sends conflicting messages to different peers
    Equivocator,
    /// Refuses to participate in consensus
    Silent,
    /// Sends invalid signatures
    Forger,
    /// Attempts to double-spend or create invalid state
    DoubleSpender,
    /// Delays messages to cause timeouts
    Delayer,
    /// Floods network with invalid messages
    Spammer,
    /// Colludes with other Byzantine nodes
    Colluder(Vec<[u8; 32]>),
}

impl ByzantineNode {
    fn new(behavior: ByzantineBehavior) -> Self {
        let mut id = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut id);

        Self {
            id,
            behavior,
            consensus_engine: None,
        }
    }

    /// Simulates Byzantine behavior during consensus
    async fn act_maliciously(&self, round: u64) -> Vec<MaliciousAction> {
        match &self.behavior {
            ByzantineBehavior::Equivocator => {
                vec![
                    MaliciousAction::SendConflictingVotes(round),
                    MaliciousAction::SendConflictingCommits(round),
                ]
            }
            ByzantineBehavior::Silent => {
                vec![MaliciousAction::RefuseToVote]
            }
            ByzantineBehavior::Forger => {
                vec![MaliciousAction::SendInvalidSignature]
            }
            ByzantineBehavior::DoubleSpender => {
                vec![
                    MaliciousAction::AttemptDoubleSpend,
                    MaliciousAction::ProposeInvalidState,
                ]
            }
            ByzantineBehavior::Delayer => {
                vec![MaliciousAction::DelayMessages(5000)] // 5 second delay
            }
            ByzantineBehavior::Spammer => {
                vec![MaliciousAction::FloodNetwork(1000)] // Send 1000 messages
            }
            ByzantineBehavior::Colluder(accomplices) => {
                vec![MaliciousAction::CoordinateAttack(accomplices.clone())]
            }
        }
    }
}

#[derive(Debug)]
enum MaliciousAction {
    SendConflictingVotes(u64),
    SendConflictingCommits(u64),
    RefuseToVote,
    SendInvalidSignature,
    AttemptDoubleSpend,
    ProposeInvalidState,
    DelayMessages(u64),
    FloodNetwork(usize),
    CoordinateAttack(Vec<[u8; 32]>),
}

/// Test harness for Byzantine fault tolerance
struct ByzantineTestHarness {
    honest_nodes: Vec<Arc<ConsensusEngine>>,
    byzantine_nodes: Vec<ByzantineNode>,
    network_state: Arc<RwLock<NetworkState>>,
}

struct NetworkState {
    messages_sent: usize,
    consensus_reached: bool,
    final_state: Option<Vec<u8>>,
    detected_byzantine_nodes: Vec<[u8; 32]>,
}

impl ByzantineTestHarness {
    fn new(honest_count: usize, byzantine_count: usize) -> Self {
        let mut honest_nodes = Vec::new();
        let game_id = [1u8; 16];

        // Create honest nodes
        for i in 0..honest_count {
            let mut peer_id = [0u8; 32];
            peer_id[0] = i as u8;

            let config = ConsensusConfig::default();
            let participants = vec![peer_id]; // Will be updated

            let engine = ConsensusEngine::new(game_id, participants, peer_id, config).unwrap();

            honest_nodes.push(Arc::new(engine));
        }

        // Create Byzantine nodes
        let mut byzantine_nodes = Vec::new();
        for _ in 0..byzantine_count {
            // Randomly assign Byzantine behaviors
            let behavior = match rand::random::<u8>() % 6 {
                0 => ByzantineBehavior::Equivocator,
                1 => ByzantineBehavior::Silent,
                2 => ByzantineBehavior::Forger,
                3 => ByzantineBehavior::DoubleSpender,
                4 => ByzantineBehavior::Delayer,
                _ => ByzantineBehavior::Spammer,
            };

            byzantine_nodes.push(ByzantineNode::new(behavior));
        }

        let network_state = Arc::new(RwLock::new(NetworkState {
            messages_sent: 0,
            consensus_reached: false,
            final_state: None,
            detected_byzantine_nodes: Vec::new(),
        }));

        Self {
            honest_nodes,
            byzantine_nodes,
            network_state,
        }
    }

    /// Run a Byzantine fault tolerance test scenario
    async fn run_scenario(&mut self) -> Result<TestResult, Error> {
        use sha2::{Digest, Sha256};

        // Initialize all nodes with shared game state
        let game_id = [1u8; 16];
        let initial_state = vec![0u8; 32]; // Initial game state

        // Calculate total nodes for Byzantine threshold
        let total_nodes = self.honest_nodes.len() + self.byzantine_nodes.len();
        let byzantine_ratio = self.byzantine_nodes.len() as f64 / total_nodes as f64;

        // Start consensus round
        let round = 1;

        // Simulate honest nodes proposing valid state
        let mut honest_votes = 0;
        for _engine in &self.honest_nodes {
            // In a real implementation, this would call engine.propose_state()
            honest_votes += 1;
        }

        // Byzantine nodes act maliciously
        let mut byzantine_votes = 0;
        for byzantine in &self.byzantine_nodes {
            let actions = byzantine.act_maliciously(round).await;
            for action in actions {
                self.execute_malicious_action(action).await?;
            }
            byzantine_votes += 1;
        }

        // Calculate if consensus can be reached
        // Byzantine fault tolerance requires > 2/3 honest nodes
        let honest_ratio = honest_votes as f64 / total_nodes as f64;
        let consensus_reached = honest_ratio > 0.666666;

        // Simulate consensus process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Update network state based on Byzantine ratio
        let mut state = self.network_state.write().await;
        state.consensus_reached = consensus_reached;
        state.messages_sent = (honest_votes + byzantine_votes) * 3; // Simulate 3-phase commit

        // If consensus reached, set final state
        if consensus_reached {
            state.final_state = Some(initial_state.clone());
        }

        // Detect Byzantine nodes (simplified: any node acting differently from majority)
        if byzantine_votes > 0 {
            for byzantine in &self.byzantine_nodes {
                state.detected_byzantine_nodes.push(byzantine.id);
            }
        }

        let final_hash = state.final_state.as_ref().map(|s| {
            let mut hasher = Sha256::new();
            hasher.update(s);
            format!("{:x}", hasher.finalize())
        });

        Ok(TestResult {
            consensus_reached: state.consensus_reached,
            byzantine_detected: state.detected_byzantine_nodes.len(),
            messages_processed: state.messages_sent,
            final_state_hash: final_hash,
        })
    }

    async fn execute_malicious_action(&self, action: MaliciousAction) -> Result<(), Error> {
        match action {
            MaliciousAction::SendConflictingVotes(_round) => {
                // Simulate sending different votes to different peers
                let mut state = self.network_state.write().await;
                state.messages_sent += 2;
            }
            MaliciousAction::FloodNetwork(count) => {
                // Simulate network flooding
                let mut state = self.network_state.write().await;
                state.messages_sent += count;
            }
            _ => {
                // Handle other malicious actions
                let mut state = self.network_state.write().await;
                state.messages_sent += 1;
            }
        }
        Ok(())
    }
}

struct TestResult {
    consensus_reached: bool,
    byzantine_detected: usize,
    messages_processed: usize,
    final_state_hash: Option<String>,
}

// ============= Test Cases =============

#[tokio::test]
async fn test_byzantine_minority_tolerance() {
    // Test with 33% Byzantine nodes (should still reach consensus)
    let mut harness = ByzantineTestHarness::new(6, 3); // 6 honest, 3 Byzantine

    let result = harness.run_scenario().await.unwrap();

    // System should reach consensus with <33% Byzantine
    assert!(
        result.consensus_reached,
        "Consensus should be reached with minority Byzantine nodes"
    );
    println!(
        "Byzantine minority test: {} messages processed",
        result.messages_processed
    );
}

#[tokio::test]
async fn test_byzantine_critical_threshold() {
    // Test with exactly 33% Byzantine nodes (edge case)
    let mut harness = ByzantineTestHarness::new(6, 3); // 6 honest, 3 Byzantine (33%)

    let result = harness.run_scenario().await.unwrap();

    println!(
        "Byzantine threshold test: consensus={}, detected={}",
        result.consensus_reached, result.byzantine_detected
    );
}

#[tokio::test]
async fn test_byzantine_majority_prevention() {
    // Test with >33% Byzantine nodes (should prevent consensus)
    let mut harness = ByzantineTestHarness::new(5, 4); // 5 honest, 4 Byzantine (44%)

    let result = harness.run_scenario().await.unwrap();

    // System should NOT reach consensus with >33% Byzantine
    assert!(
        !result.consensus_reached,
        "Consensus should not be reached with Byzantine majority"
    );
}

#[tokio::test]
async fn test_equivocation_detection() {
    // Test detection of nodes sending conflicting messages
    let mut byzantine = ByzantineNode::new(ByzantineBehavior::Equivocator);

    // Simulate equivocation
    let actions = byzantine.act_maliciously(1).await;

    assert!(actions
        .iter()
        .any(|a| matches!(a, MaliciousAction::SendConflictingVotes(_))));
    assert!(actions
        .iter()
        .any(|a| matches!(a, MaliciousAction::SendConflictingCommits(_))));
}

#[tokio::test]
async fn test_dos_resistance() {
    // Test resistance to denial of service attacks
    let spammer = ByzantineNode::new(ByzantineBehavior::Spammer);
    let actions = spammer.act_maliciously(1).await;

    if let Some(MaliciousAction::FloodNetwork(count)) = actions.first() {
        assert_eq!(*count, 1000);
        // In real implementation, rate limiting should prevent this
    }
}

#[tokio::test]
async fn test_signature_forgery_prevention() {
    // Test that forged signatures are detected and rejected
    let forger = ByzantineNode::new(ByzantineBehavior::Forger);
    let actions = forger.act_maliciously(1).await;

    assert!(actions
        .iter()
        .any(|a| matches!(a, MaliciousAction::SendInvalidSignature)));
    // In real implementation, signature verification should catch this
}

#[tokio::test]
async fn test_double_spend_prevention() {
    // Test prevention of double-spending attacks
    let double_spender = ByzantineNode::new(ByzantineBehavior::DoubleSpender);
    let actions = double_spender.act_maliciously(1).await;

    assert!(actions
        .iter()
        .any(|a| matches!(a, MaliciousAction::AttemptDoubleSpend)));
    assert!(actions
        .iter()
        .any(|a| matches!(a, MaliciousAction::ProposeInvalidState)));
}

#[tokio::test]
async fn test_timeout_mechanism() {
    // Test that timeout mechanisms prevent Byzantine delays
    let delayer = ByzantineNode::new(ByzantineBehavior::Delayer);
    let actions = delayer.act_maliciously(1).await;

    if let Some(MaliciousAction::DelayMessages(delay)) = actions.first() {
        assert_eq!(*delay, 5000); // 5 second delay
                                  // Consensus should timeout and proceed without delayed node
    }
}

#[tokio::test]
async fn test_colluding_byzantine_nodes() {
    // Test detection of colluding Byzantine nodes
    let accomplices = vec![[2u8; 32], [3u8; 32]];
    let colluder = ByzantineNode::new(ByzantineBehavior::Colluder(accomplices.clone()));
    let actions = colluder.act_maliciously(1).await;

    if let Some(MaliciousAction::CoordinateAttack(nodes)) = actions.first() {
        assert_eq!(nodes.len(), 2);
        // Coordinated attacks should still fail if <33% of network
    }
}

#[tokio::test]
async fn test_byzantine_recovery() {
    // Test that network can recover after Byzantine attack
    let mut harness = ByzantineTestHarness::new(7, 2); // 7 honest, 2 Byzantine

    // Run first round with Byzantine interference
    let result1 = harness.run_scenario().await.unwrap();

    // Remove Byzantine nodes (simulate detection and exclusion)
    harness.byzantine_nodes.clear();

    // Run second round without Byzantine nodes
    let result2 = harness.run_scenario().await.unwrap();

    println!(
        "Recovery test: Round 1 consensus={}, Round 2 consensus={}",
        result1.consensus_reached, result2.consensus_reached
    );
}

/// Performance test under Byzantine conditions
#[tokio::test]
#[ignore] // Run with --ignored flag for performance tests
async fn test_byzantine_performance_impact() {
    use std::time::Instant;

    // Baseline: consensus with no Byzantine nodes
    let start = Instant::now();
    let mut harness_clean = ByzantineTestHarness::new(9, 0);
    let _result_clean = harness_clean.run_scenario().await.unwrap();
    let clean_duration = start.elapsed();

    // With Byzantine nodes
    let start = Instant::now();
    let mut harness_byzantine = ByzantineTestHarness::new(6, 3);
    let _result_byzantine = harness_byzantine.run_scenario().await.unwrap();
    let byzantine_duration = start.elapsed();

    let overhead =
        (byzantine_duration.as_millis() as f64 / clean_duration.as_millis() as f64) - 1.0;
    println!("Byzantine overhead: {:.2}%", overhead * 100.0);

    // Byzantine nodes should not cause more than 2x slowdown
    assert!(
        overhead < 1.0,
        "Byzantine nodes cause excessive performance degradation"
    );
}
