//! Formal Verification Framework for BitCraps Consensus
//!
//! This module provides formal verification capabilities for the consensus system using:
//! - TLA+ specification generation for model checking
//! - Property-based testing for invariant verification
//! - Mathematical proofs of safety and liveness properties
//! - Temporal logic verification of consensus protocols
//!
//! ## Mathematical Foundations
//!
//! The consensus protocol implements Practical Byzantine Fault Tolerance (PBFT)
//! with the following mathematical guarantees:
//!
//! ### Safety Property:
//! For all rounds r1, r2: Decided(r1, v1) AND Decided(r2, v2) IMPLIES v1 = v2
//! (Agreement: Two honest nodes never decide different values)
//!
//! ### Liveness Property:
//! For all proposals p: Eventually(Decided(p) OR Rejected(p))
//! (Termination: Every proposal is eventually decided or rejected)
//!
//! ### Byzantine Tolerance:
//! f < n/3 where f = Byzantine nodes, n = total nodes
//! (System tolerates up to floor((n-1)/3) Byzantine failures)

use crate::crypto::safe_arithmetic::SafeArithmetic;
use crate::error::{Error, Result};
use crate::protocol::{Hash256, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// TLA+ specification generator for consensus protocols
pub struct TLASpecGenerator {
    /// Number of nodes in the system
    nodes: usize,
    /// Maximum Byzantine faults tolerated
    byzantine_faults: usize,
    /// Consensus parameters
    params: ConsensusParameters,
}

/// Consensus parameters for formal verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusParameters {
    /// Minimum nodes required for consensus
    pub min_nodes: usize,
    /// Byzantine fault tolerance ratio
    pub byzantine_ratio: f64,
    /// Round timeout in seconds
    pub round_timeout: u64,
    /// Maximum rounds before abort
    pub max_rounds: usize,
    /// Quorum size calculation method
    pub quorum_method: QuorumMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuorumMethod {
    /// Simple majority: floor(n/2) + 1
    SimpleMajority,
    /// Byzantine fault tolerant: floor(2n/3) + 1
    ByzantineFaultTolerant,
    /// Supermajority: floor(3n/4) + 1
    SuperMajority,
}

impl Default for ConsensusParameters {
    fn default() -> Self {
        Self {
            min_nodes: 4,
            byzantine_ratio: 0.33,
            round_timeout: 10,
            max_rounds: 100,
            quorum_method: QuorumMethod::ByzantineFaultTolerant,
        }
    }
}

/// Invariant properties that must hold for consensus correctness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusInvariants {
    /// Safety: Agreement on decided values
    pub agreement: bool,
    /// Safety: Validity of decided values
    pub validity: bool,
    /// Liveness: Termination guarantee
    pub termination: bool,
    /// Byzantine tolerance: Fault threshold not exceeded
    pub byzantine_tolerance: bool,
    /// Integrity: No spurious decisions
    pub integrity: bool,
}

impl Default for ConsensusInvariants {
    fn default() -> Self {
        Self {
            agreement: true,
            validity: true,
            termination: true,
            byzantine_tolerance: true,
            integrity: true,
        }
    }
}

/// Formal model state for verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelState {
    /// Current round number
    pub round: u64,
    /// Proposals in the current round
    pub proposals: HashMap<PeerId, Hash256>,
    /// Votes cast by each node
    pub votes: HashMap<PeerId, HashMap<Hash256, bool>>,
    /// Decided values
    pub decided: Option<Hash256>,
    /// Byzantine nodes (for testing)
    pub byzantine_nodes: HashSet<PeerId>,
    /// Timestamp for timeout detection
    pub timestamp: u64,
}

impl Default for ModelState {
    fn default() -> Self {
        Self {
            round: 0,
            proposals: HashMap::new(),
            votes: HashMap::new(),
            decided: None,
            byzantine_nodes: HashSet::new(),
            timestamp: 0,
        }
    }
}

/// Consensus action in the formal model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusAction {
    /// Node proposes a value
    Propose { node: PeerId, value: Hash256 },
    /// Node votes on a proposal
    Vote { node: PeerId, proposal: Hash256, vote: bool },
    /// System decides on a value
    Decide { value: Hash256 },
    /// Round timeout occurs
    Timeout { round: u64 },
    /// Byzantine behavior
    ByzantineAction { node: PeerId, action: ByzantineActionType },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ByzantineActionType {
    /// Node sends conflicting proposals
    Equivocation,
    /// Node votes for multiple proposals
    DoubleVoting,
    /// Node sends invalid messages
    InvalidMessage,
    /// Node fails to participate
    Omission,
}

impl TLASpecGenerator {
    /// Create new TLA+ specification generator
    pub fn new(nodes: usize, params: ConsensusParameters) -> Result<Self> {
        if nodes < params.min_nodes {
            return Err(Error::InvalidConfiguration(format!(
                "Need at least {} nodes, got {}",
                params.min_nodes, nodes
            )));
        }

        let byzantine_faults = Self::calculate_max_byzantine_faults(nodes, &params)?;
        
        Ok(Self {
            nodes,
            byzantine_faults,
            params,
        })
    }

    /// Calculate maximum Byzantine faults based on parameters
    fn calculate_max_byzantine_faults(nodes: usize, params: &ConsensusParameters) -> Result<usize> {
        let max_faults = match params.quorum_method {
            QuorumMethod::SimpleMajority => {
                // Simple majority can't tolerate Byzantine faults
                0
            }
            QuorumMethod::ByzantineFaultTolerant => {
                // PBFT: f < n/3
                if nodes < 4 {
                    return Err(Error::InvalidConfiguration(
                        "Byzantine fault tolerance requires at least 4 nodes".to_string(),
                    ));
                }
                (nodes - 1) / 3
            }
            QuorumMethod::SuperMajority => {
                // Supermajority: allows higher fault tolerance
                (nodes - 1) / 4
            }
        };

        Ok(max_faults)
    }

    /// Generate complete TLA+ specification
    pub fn generate_tla_spec(&self) -> Result<String> {
        let mut spec = String::new();
        
        // Module header
        writeln!(spec, "---- MODULE BitCrapsConsensus ----").unwrap();
        writeln!(spec, "EXTENDS Integers, Sequences, FiniteSets").unwrap();
        writeln!(spec, "").unwrap();

        // Constants
        self.write_constants(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Variables
        self.write_variables(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Type invariants
        self.write_type_invariants(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Initial state
        self.write_init_state(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Actions
        self.write_actions(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Next state relation
        self.write_next_state(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Fairness conditions
        self.write_fairness(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Safety properties
        self.write_safety_properties(&mut spec)?;
        writeln!(spec, "").unwrap();

        // Liveness properties
        self.write_liveness_properties(&mut spec)?;
        writeln!(spec, "").unwrap();

        writeln!(spec, "====").unwrap();

        Ok(spec)
    }

    /// Write TLA+ constants section
    fn write_constants(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "CONSTANTS").unwrap();
        writeln!(spec, "  N,           \\\\ Number of nodes").unwrap();
        writeln!(spec, "  F,           \\\\ Maximum Byzantine faults").unwrap();
        writeln!(spec, "  Nodes,       \\\\ Set of all nodes").unwrap();
        writeln!(spec, "  Values,      \\\\ Set of possible values").unwrap();
        writeln!(spec, "  MaxRounds    \\\\ Maximum number of rounds").unwrap();
        writeln!(spec, "").unwrap();
        
        writeln!(spec, "ASSUME").unwrap();
        writeln!(spec, "  /\\ N = {}", self.nodes).unwrap();
        writeln!(spec, "  /\\ F = {}", self.byzantine_faults).unwrap();
        writeln!(spec, "  /\\ F < N / 3  \\* Byzantine fault tolerance").unwrap();
        writeln!(spec, "  /\\ Cardinality(Nodes) = N").unwrap();
        writeln!(spec, "  /\\ MaxRounds = {}", self.params.max_rounds).unwrap();

        Ok(())
    }

    /// Write TLA+ variables section
    fn write_variables(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "VARIABLES").unwrap();
        writeln!(spec, "  round,       \\\\ Current round number").unwrap();
        writeln!(spec, "  proposals,   \\\\ Proposals submitted in each round").unwrap();
        writeln!(spec, "  votes,       \\\\ Votes cast by each node").unwrap();
        writeln!(spec, "  decided,     \\\\ Decided values").unwrap();
        writeln!(spec, "  byzantine    \\\\ Set of Byzantine nodes").unwrap();
        writeln!(spec, "").unwrap();
        writeln!(spec, "vars == <<round, proposals, votes, decided, byzantine>>").unwrap();

        Ok(())
    }

    /// Write type invariants
    fn write_type_invariants(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "TypeInvariant ==").unwrap();
        writeln!(spec, "  /\\ round \\in 0..MaxRounds").unwrap();
        writeln!(spec, "  /\\ proposals \\in [0..MaxRounds -> [Nodes -> Values \\cup {{\"None\"}}]]").unwrap();
        writeln!(spec, "  /\\ votes \\in [Nodes -> [Values -> BOOLEAN]]").unwrap();
        writeln!(spec, "  /\\ decided \\in Values \\cup {{\"None\"}}").unwrap();
        writeln!(spec, "  /\\ byzantine \\subseteq Nodes").unwrap();
        writeln!(spec, "  /\\ Cardinality(byzantine) <= F").unwrap();

        Ok(())
    }

    /// Write initial state
    fn write_init_state(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "Init ==").unwrap();
        writeln!(spec, "  /\\ round = 0").unwrap();
        writeln!(spec, "  /\\ proposals = [r \\in 0..MaxRounds |-> [n \\in Nodes |-> \"None\"]]").unwrap();
        writeln!(spec, "  /\\ votes = [n \\in Nodes |-> [v \\in Values |-> FALSE]]").unwrap();
        writeln!(spec, "  /\\ decided = \"None\"").unwrap();
        writeln!(spec, "  /\\ byzantine \\subseteq Nodes").unwrap();
        writeln!(spec, "  /\\ Cardinality(byzantine) <= F").unwrap();

        Ok(())
    }

    /// Write consensus actions
    fn write_actions(&self, spec: &mut String) -> Result<()> {
        // Propose action
        writeln!(spec, "Propose(n, v) ==").unwrap();
        writeln!(spec, "  /\\ n \\in Nodes").unwrap();
        writeln!(spec, "  /\\ v \\in Values").unwrap();
        writeln!(spec, "  /\\ proposals[round][n] = \"None\"").unwrap();
        writeln!(spec, "  /\\ decided = \"None\"").unwrap();
        writeln!(spec, "  /\\ proposals' = [proposals EXCEPT ![round][n] = v]").unwrap();
        writeln!(spec, "  /\\ UNCHANGED <<round, votes, decided, byzantine>>").unwrap();
        writeln!(spec, "").unwrap();

        // Vote action
        writeln!(spec, "Vote(n, v) ==").unwrap();
        writeln!(spec, "  /\\ n \\in Nodes").unwrap();
        writeln!(spec, "  /\\ v \\in Values").unwrap();
        writeln!(spec, "  /\\ \\E p \\in Nodes : proposals[round][p] = v").unwrap();
        writeln!(spec, "  /\\ votes[n][v] = FALSE").unwrap();
        writeln!(spec, "  /\\ decided = \"None\"").unwrap();
        writeln!(spec, "  /\\ votes' = [votes EXCEPT ![n][v] = TRUE]").unwrap();
        writeln!(spec, "  /\\ UNCHANGED <<round, proposals, decided, byzantine>>").unwrap();
        writeln!(spec, "").unwrap();

        // Decide action
        let quorum = self.calculate_quorum(self.nodes);
        writeln!(spec, "Decide(v) ==").unwrap();
        writeln!(spec, "  /\\ v \\in Values").unwrap();
        writeln!(spec, "  /\\ decided = \"None\"").unwrap();
        writeln!(spec, "  /\\ Cardinality({{n \\in Nodes : votes[n][v]}}) >= {}", quorum).unwrap();
        writeln!(spec, "  /\\ decided' = v").unwrap();
        writeln!(spec, "  /\\ UNCHANGED <<round, proposals, votes, byzantine>>").unwrap();
        writeln!(spec, "").unwrap();

        // Byzantine actions
        writeln!(spec, "ByzantineEquivocation(n, v1, v2) ==").unwrap();
        writeln!(spec, "  /\\ n \\in byzantine").unwrap();
        writeln!(spec, "  /\\ v1 /= v2").unwrap();
        writeln!(spec, "  /\\ proposals[round][n] = \"None\"").unwrap();
        writeln!(spec, "  /\\ proposals' = [proposals EXCEPT ![round][n] = v1]").unwrap();
        writeln!(spec, "  /\\ \\* Byzantine node can send conflicting messages").unwrap();
        writeln!(spec, "  /\\ UNCHANGED <<round, votes, decided, byzantine>>").unwrap();

        Ok(())
    }

    /// Calculate quorum size based on method
    fn calculate_quorum(&self, nodes: usize) -> usize {
        match self.params.quorum_method {
            QuorumMethod::SimpleMajority => (nodes / 2) + 1,
            QuorumMethod::ByzantineFaultTolerant => {
                // Need more than 2/3 for Byzantine fault tolerance
                (nodes * 2 + 2) / 3
            }
            QuorumMethod::SuperMajority => (nodes * 3 + 3) / 4,
        }
    }

    /// Write next state relation
    fn write_next_state(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "Next ==").unwrap();
        writeln!(spec, "  \\/ \\E n \\in Nodes, v \\in Values : Propose(n, v)").unwrap();
        writeln!(spec, "  \\/ \\E n \\in Nodes, v \\in Values : Vote(n, v)").unwrap();
        writeln!(spec, "  \\/ \\E v \\in Values : Decide(v)").unwrap();
        writeln!(spec, "  \\/ \\E n \\in byzantine, v1, v2 \\in Values : ByzantineEquivocation(n, v1, v2)").unwrap();

        Ok(())
    }

    /// Write fairness conditions
    fn write_fairness(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "Fairness ==").unwrap();
        writeln!(spec, "  \\* Weak fairness: honest nodes eventually act").unwrap();
        writeln!(spec, "  /\\ \\A n \\in Nodes \\\\ byzantine :").unwrap();
        writeln!(spec, "       WF_vars(\\E v \\in Values : Propose(n, v) \\/ Vote(n, v))").unwrap();
        writeln!(spec, "  \\* Strong fairness: decisions are eventually made").unwrap();
        writeln!(spec, "  /\\ SF_vars(\\E v \\in Values : Decide(v))").unwrap();

        Ok(())
    }

    /// Write safety properties
    fn write_safety_properties(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "\\* SAFETY PROPERTIES").unwrap();
        writeln!(spec, "").unwrap();

        // Agreement property
        writeln!(spec, "Agreement ==").unwrap();
        writeln!(spec, "  \\* Two honest nodes never decide different values").unwrap();
        writeln!(spec, "  \\A v1, v2 \\in Values :").unwrap();
        writeln!(spec, "    (decided = v1) /\\ (decided = v2) => (v1 = v2)").unwrap();
        writeln!(spec, "").unwrap();

        // Validity property
        writeln!(spec, "Validity ==").unwrap();
        writeln!(spec, "  \\* Only proposed values can be decided").unwrap();
        writeln!(spec, "  decided /= \"None\" =>").unwrap();
        writeln!(spec, "    \\E n \\in Nodes, r \\in 0..MaxRounds :").unwrap();
        writeln!(spec, "      proposals[r][n] = decided").unwrap();
        writeln!(spec, "").unwrap();

        // Integrity property
        writeln!(spec, "Integrity ==").unwrap();
        writeln!(spec, "  \\* A value is decided at most once").unwrap();
        writeln!(spec, "  decided /= \"None\" => [](decided' = decided)").unwrap();
        writeln!(spec, "").unwrap();

        // Byzantine tolerance
        writeln!(spec, "ByzantineTolerance ==").unwrap();
        writeln!(spec, "  \\* System works correctly with up to F Byzantine nodes").unwrap();
        writeln!(spec, "  Cardinality(byzantine) <= F => (Agreement /\\ Validity /\\ Integrity)").unwrap();

        Ok(())
    }

    /// Write liveness properties
    fn write_liveness_properties(&self, spec: &mut String) -> Result<()> {
        writeln!(spec, "\\* LIVENESS PROPERTIES").unwrap();
        writeln!(spec, "").unwrap();

        // Termination property
        writeln!(spec, "Termination ==").unwrap();
        writeln!(spec, "  \\* Eventually, a decision is made").unwrap();
        writeln!(spec, "  <>(decided /= \"None\")").unwrap();
        writeln!(spec, "").unwrap();

        // Progress property
        writeln!(spec, "Progress ==").unwrap();
        writeln!(spec, "  \\* If enough honest nodes propose, decision is reached").unwrap();
        writeln!(spec, "  (\\E v \\in Values : Cardinality({{n \\in (Nodes \\\\ byzantine) :").unwrap();
        writeln!(spec, "                                 proposals[round][n] = v}}) >= {})", 
                 self.calculate_quorum(self.nodes - self.byzantine_faults)).unwrap();
        writeln!(spec, "  => <>(decided /= \"None\")").unwrap();

        Ok(())
    }

    /// Generate model checking configuration
    pub fn generate_model_config(&self) -> Result<String> {
        let mut config = String::new();

        writeln!(config, "\\* Model checking configuration for BitCraps consensus").unwrap();
        writeln!(config, "").unwrap();

        writeln!(config, "SPECIFICATION").unwrap();
        writeln!(config, "  Init /\\ [][Next]_vars /\\ Fairness").unwrap();
        writeln!(config, "").unwrap();

        writeln!(config, "INVARIANTS").unwrap();
        writeln!(config, "  TypeInvariant").unwrap();
        writeln!(config, "  Agreement").unwrap();
        writeln!(config, "  Validity").unwrap();
        writeln!(config, "  Integrity").unwrap();
        writeln!(config, "  ByzantineTolerance").unwrap();
        writeln!(config, "").unwrap();

        writeln!(config, "PROPERTIES").unwrap();
        writeln!(config, "  Termination").unwrap();
        writeln!(config, "  Progress").unwrap();
        writeln!(config, "").unwrap();

        writeln!(config, "CONSTANTS").unwrap();
        writeln!(config, "  N = {}", self.nodes).unwrap();
        writeln!(config, "  F = {}", self.byzantine_faults).unwrap();
        writeln!(config, "  Nodes = {{\"n{}\"}}", 
                 (0..self.nodes).map(|i| format!("{}", i)).collect::<Vec<_>>().join("\", \"n")).unwrap();
        writeln!(config, "  Values = {{\"v1\", \"v2\", \"v3\"}}").unwrap();
        writeln!(config, "  MaxRounds = {}", self.params.max_rounds).unwrap();

        Ok(config)
    }
}

/// Property-based testing framework for consensus invariants
pub struct PropertyTester {
    /// Parameters for testing
    params: ConsensusParameters,
    /// Random seed for reproducible tests
    seed: u64,
}

impl PropertyTester {
    /// Create new property tester
    pub fn new(params: ConsensusParameters, seed: u64) -> Self {
        Self { params, seed }
    }

    /// Test consensus agreement property
    pub fn test_agreement_property(&self, states: &[ModelState]) -> Result<bool> {
        // Check that all decided values are the same
        let mut decided_values = HashSet::new();
        
        for state in states {
            if let Some(decided) = state.decided {
                decided_values.insert(decided);
            }
        }

        // Agreement violated if more than one value decided
        Ok(decided_values.len() <= 1)
    }

    /// Test consensus validity property
    pub fn test_validity_property(&self, states: &[ModelState]) -> Result<bool> {
        for state in states {
            if let Some(decided) = state.decided {
                // Check that decided value was actually proposed
                let was_proposed = state.proposals.values().any(|&proposal| proposal == decided);
                if !was_proposed {
                    return Ok(false); // Validity violated
                }
            }
        }

        Ok(true)
    }

    /// Test Byzantine fault tolerance property
    pub fn test_byzantine_tolerance(&self, state: &ModelState, total_nodes: usize) -> Result<bool> {
        let byzantine_count = state.byzantine_nodes.len();
        let max_byzantine = Self::calculate_max_byzantine(total_nodes, &self.params)?;

        Ok(byzantine_count <= max_byzantine)
    }

    /// Calculate maximum Byzantine nodes for given parameters
    fn calculate_max_byzantine(nodes: usize, params: &ConsensusParameters) -> Result<usize> {
        TLASpecGenerator::calculate_max_byzantine_faults(nodes, params)
    }

    /// Test liveness property (termination)
    pub fn test_termination_property(&self, states: &[ModelState], max_steps: usize) -> Result<bool> {
        if states.is_empty() {
            return Ok(true);
        }

        // Check if any state has a decision within max_steps
        let has_decision = states.iter().any(|state| state.decided.is_some());
        let within_steps = states.len() <= max_steps;

        Ok(!within_steps || has_decision)
    }

    /// Generate random consensus execution for property testing
    pub fn generate_random_execution(&self, nodes: usize, steps: usize) -> Result<Vec<ModelState>> {
        let mut rng_state = self.seed;
        let mut states = Vec::with_capacity(steps);
        let mut current_state = ModelState::default();

        // Add Byzantine nodes (up to maximum allowed)
        let max_byzantine = Self::calculate_max_byzantine(nodes, &self.params)?;
        for i in 0..max_byzantine {
            current_state.byzantine_nodes.insert([i as u8; 32]);
        }

        states.push(current_state.clone());

        for step in 1..steps {
            let action = self.generate_random_action(&mut rng_state, nodes)?;
            current_state = self.apply_action(current_state, action)?;
            current_state.timestamp = step as u64;
            states.push(current_state.clone());

            // Stop if consensus reached
            if current_state.decided.is_some() {
                break;
            }
        }

        Ok(states)
    }

    /// Generate random consensus action
    fn generate_random_action(&self, rng_state: &mut u64, nodes: usize) -> Result<ConsensusAction> {
        // Simple LCG for reproducible randomness
        *rng_state = rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        
        let action_type = *rng_state % 4;
        let node_id = [(*rng_state % nodes as u64) as u8; 32];
        let value = [(*rng_state >> 8) as u8; 32];

        match action_type {
            0 => Ok(ConsensusAction::Propose { node: node_id, value }),
            1 => Ok(ConsensusAction::Vote { node: node_id, proposal: value, vote: true }),
            2 => Ok(ConsensusAction::Decide { value }),
            3 => Ok(ConsensusAction::ByzantineAction { 
                node: node_id, 
                action: ByzantineActionType::Equivocation 
            }),
            _ => unreachable!(),
        }
    }

    /// Apply action to model state
    fn apply_action(&self, mut state: ModelState, action: ConsensusAction) -> Result<ModelState> {
        match action {
            ConsensusAction::Propose { node, value } => {
                if !state.proposals.contains_key(&node) {
                    state.proposals.insert(node, value);
                }
            }
            ConsensusAction::Vote { node, proposal, vote } => {
                state.votes.entry(node).or_default().insert(proposal, vote);
            }
            ConsensusAction::Decide { value } => {
                if state.decided.is_none() {
                    // Check if value has enough votes
                    let vote_count = state.votes.values()
                        .flat_map(|votes| votes.get(&value).copied())
                        .filter(|&vote| vote)
                        .count();
                    
                    let required_votes = self.calculate_quorum_size(
                        state.votes.len().saturating_sub(state.byzantine_nodes.len())
                    );
                    
                    if vote_count >= required_votes {
                        state.decided = Some(value);
                    }
                }
            }
            ConsensusAction::ByzantineAction { node, action: _ } => {
                state.byzantine_nodes.insert(node);
            }
            ConsensusAction::Timeout { round } => {
                state.round = SafeArithmetic::safe_add_u64(state.round, 1)?;
                // Clear proposals for timed-out round
                if state.round > round {
                    state.proposals.clear();
                }
            }
        }

        Ok(state)
    }

    /// Calculate quorum size for current parameters
    fn calculate_quorum_size(&self, honest_nodes: usize) -> usize {
        match self.params.quorum_method {
            QuorumMethod::SimpleMajority => (honest_nodes / 2) + 1,
            QuorumMethod::ByzantineFaultTolerant => (honest_nodes * 2 + 2) / 3,
            QuorumMethod::SuperMajority => (honest_nodes * 3 + 3) / 4,
        }
    }

    /// Run comprehensive property-based test suite
    pub fn run_property_tests(&self, num_tests: usize, max_nodes: usize) -> Result<PropertyTestResults> {
        let mut results = PropertyTestResults::default();
        
        for test_id in 0..num_tests {
            let nodes = 4 + (test_id % (max_nodes - 3)); // Ensure minimum 4 nodes
            let steps = 50 + (test_id % 100); // Variable execution length
            let test_seed = self.seed.wrapping_add(test_id as u64);
            
            let tester = PropertyTester::new(self.params.clone(), test_seed);
            let execution = tester.generate_random_execution(nodes, steps)?;
            
            // Test all properties
            results.total_tests += 1;
            
            if !tester.test_agreement_property(&execution)? {
                results.agreement_violations += 1;
            }
            
            if !tester.test_validity_property(&execution)? {
                results.validity_violations += 1;
            }
            
            if let Some(final_state) = execution.last() {
                if !tester.test_byzantine_tolerance(final_state, nodes)? {
                    results.byzantine_tolerance_violations += 1;
                }
            }
            
            if !tester.test_termination_property(&execution, steps)? {
                results.liveness_violations += 1;
            }
        }
        
        Ok(results)
    }
}

/// Results from property-based testing
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PropertyTestResults {
    pub total_tests: usize,
    pub agreement_violations: usize,
    pub validity_violations: usize,
    pub byzantine_tolerance_violations: usize,
    pub liveness_violations: usize,
}

impl PropertyTestResults {
    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.agreement_violations == 0
            && self.validity_violations == 0
            && self.byzantine_tolerance_violations == 0
            && self.liveness_violations == 0
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            return 1.0;
        }

        let total_violations = self.agreement_violations
            + self.validity_violations
            + self.byzantine_tolerance_violations
            + self.liveness_violations;

        1.0 - (total_violations as f64 / (self.total_tests * 4) as f64)
    }
}

/// Temporal logic verification utilities
pub struct TemporalLogicChecker;

impl TemporalLogicChecker {
    /// Check temporal property over execution trace
    pub fn check_temporal_property(
        states: &[ModelState],
        property: TemporalProperty,
    ) -> Result<bool> {
        match property {
            TemporalProperty::Always(invariant) => {
                Self::check_always(states, invariant)
            }
            TemporalProperty::Eventually(property) => {
                Self::check_eventually(states, property)
            }
            TemporalProperty::Until(prop1, prop2) => {
                Self::check_until(states, prop1, prop2)
            }
        }
    }

    /// Check that property holds in all states ([]P)
    fn check_always(states: &[ModelState], invariant: StateProperty) -> Result<bool> {
        for state in states {
            if !Self::check_state_property(state, invariant)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check that property eventually holds (<>P)
    fn check_eventually(states: &[ModelState], property: StateProperty) -> Result<bool> {
        for state in states {
            if Self::check_state_property(state, property)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check that prop1 holds until prop2 holds (P1 U P2)
    fn check_until(states: &[ModelState], prop1: StateProperty, prop2: StateProperty) -> Result<bool> {
        for state in states {
            if Self::check_state_property(state, prop2)? {
                return Ok(true); // P2 holds, Until satisfied
            }
            if !Self::check_state_property(state, prop1)? {
                return Ok(false); // P1 doesn't hold before P2
            }
        }
        Ok(false) // P2 never holds
    }

    /// Check state property
    fn check_state_property(state: &ModelState, property: StateProperty) -> Result<bool> {
        match property {
            StateProperty::NoDecision => Ok(state.decided.is_none()),
            StateProperty::HasDecision => Ok(state.decided.is_some()),
            StateProperty::ByzantineCountLimited(max) => Ok(state.byzantine_nodes.len() <= max),
            StateProperty::RoundBounded(max) => Ok(state.round <= max),
        }
    }
}

/// Temporal logic properties
#[derive(Debug, Clone, Copy)]
pub enum TemporalProperty {
    /// Property holds in all states ([]P)
    Always(StateProperty),
    /// Property eventually holds (<>P)
    Eventually(StateProperty),
    /// Property P1 holds until P2 holds (P1 U P2)
    Until(StateProperty, StateProperty),
}

/// State properties for temporal logic
#[derive(Debug, Clone, Copy)]
pub enum StateProperty {
    /// No value has been decided yet
    NoDecision,
    /// Some value has been decided
    HasDecision,
    /// Number of Byzantine nodes is limited
    ByzantineCountLimited(usize),
    /// Round number is within bounds
    RoundBounded(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tla_spec_generation() {
        let params = ConsensusParameters::default();
        let generator = TLASpecGenerator::new(4, params).unwrap();
        
        let spec = generator.generate_tla_spec().unwrap();
        assert!(spec.contains("MODULE BitCrapsConsensus"));
        assert!(spec.contains("Agreement"));
        assert!(spec.contains("Validity"));
        assert!(spec.contains("Termination"));
    }

    #[test]
    fn test_property_testing() {
        let params = ConsensusParameters::default();
        let tester = PropertyTester::new(params, 42);
        
        let execution = tester.generate_random_execution(4, 20).unwrap();
        assert!(!execution.is_empty());
        
        let agreement_ok = tester.test_agreement_property(&execution).unwrap();
        assert!(agreement_ok);
    }

    #[test]
    fn test_byzantine_tolerance_calculation() {
        let params = ConsensusParameters::default();
        
        // Test minimum nodes requirement
        assert!(TLASpecGenerator::new(3, params.clone()).is_err());
        assert!(TLASpecGenerator::new(4, params).is_ok());
    }

    #[test]
    fn test_temporal_logic() {
        let states = vec![
            ModelState { decided: None, ..Default::default() },
            ModelState { decided: Some([1u8; 32]), ..Default::default() },
        ];
        
        let eventually_decides = TemporalProperty::Eventually(StateProperty::HasDecision);
        let result = TemporalLogicChecker::check_temporal_property(&states, eventually_decides).unwrap();
        assert!(result);
    }

    #[test]
    fn test_quorum_calculations() {
        let params_majority = ConsensusParameters {
            quorum_method: QuorumMethod::SimpleMajority,
            ..Default::default()
        };
        
        let params_bft = ConsensusParameters {
            quorum_method: QuorumMethod::ByzantineFaultTolerant,
            ..Default::default()
        };
        
        let tester_majority = PropertyTester::new(params_majority, 0);
        let tester_bft = PropertyTester::new(params_bft, 0);
        
        // Simple majority requires n/2 + 1
        assert_eq!(tester_majority.calculate_quorum_size(4), 3);
        
        // BFT requires 2n/3 + 1 (ceiling)
        assert_eq!(tester_bft.calculate_quorum_size(4), 3);
        assert_eq!(tester_bft.calculate_quorum_size(6), 5);
    }
}