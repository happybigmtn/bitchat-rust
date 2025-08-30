//! Basic consensus example demonstrating Byzantine fault tolerance
//!
//! Run with: cargo run --example basic_consensus

use bitcraps::error::Result;
use bitcraps::protocol::consensus::engine::{ConsensusConfig, ConsensusEngine};
use bitcraps::protocol::{GameId, PeerId};
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("BitCraps Basic Consensus Example");
    println!("=================================\n");

    // Create configuration
    let config = ConsensusConfig {
        min_validators: 3,
        max_validators: 8,
        round_timeout: Duration::from_secs(5),
        proposal_timeout: Duration::from_secs(2),
        byzantine_threshold: 0.33,
        enable_anti_cheat: true,
    };

    // Create consensus engine
    let node_id = PeerId::random();
    let game_id = GameId::random();
    let mut engine = ConsensusEngine::new(node_id, config);

    println!("Node ID: {:?}", node_id);
    println!("Game ID: {:?}", game_id);
    println!();

    // Simulate adding validators
    let validators = vec![
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        node_id, // Include ourselves
    ];

    println!("Adding {} validators...", validators.len());
    for validator in &validators {
        engine.add_validator(*validator)?;
        println!("  Added validator: {:?}", validator);
    }
    println!();

    // Create a game proposal
    println!("Creating game proposal...");
    let proposal = engine.create_proposal(
        game_id,
        "PlaceBet".to_string(),
        vec![1, 2, 3, 4], // Dummy bet data
    )?;

    println!("Proposal created with ID: {:?}", proposal.id);
    println!("Operation: {}", proposal.operation_type);
    println!();

    // Simulate voting (in real system, votes come from network)
    println!("Simulating validator votes...");

    // 3 out of 4 vote yes (75% > 66.7% threshold)
    for i in 0..3 {
        let voter = validators[i];
        engine.receive_vote(proposal.id, voter, true)?;
        println!("  {:?} voted: YES", voter);
    }

    // One votes no
    engine.receive_vote(proposal.id, validators[3], false)?;
    println!("  {:?} voted: NO", validators[3]);
    println!();

    // Check if consensus reached
    if engine.has_consensus(proposal.id)? {
        println!("✓ Consensus reached! Proposal accepted.");

        // Execute the proposal
        engine.execute_proposal(proposal.id)?;
        println!("✓ Proposal executed successfully.");
    } else {
        println!("✗ Consensus not reached. Proposal rejected.");
    }

    // Display engine statistics
    println!("\nConsensus Engine Statistics:");
    println!("----------------------------");
    let stats = engine.get_statistics();
    println!("Total proposals: {}", stats.total_proposals);
    println!("Accepted proposals: {}", stats.accepted_proposals);
    println!("Rejected proposals: {}", stats.rejected_proposals);
    println!("Average consensus time: {:?}", stats.avg_consensus_time);

    Ok(())
}

/// Exercise 1: Byzantine Attack Simulation
///
/// Modify this example to simulate a Byzantine attack where
/// malicious nodes try to vote multiple times or vote after
/// the deadline. Verify that the consensus engine properly
/// rejects these invalid votes.
#[allow(dead_code)]
fn exercise_byzantine_attack() {
    // TODO: Implement Byzantine attack simulation
    // Hints:
    // 1. Try voting twice with same validator
    // 2. Try voting after proposal timeout
    // 3. Try voting with non-validator peer
    // 4. Verify engine rejects all invalid attempts
}

/// Exercise 2: Network Partition Recovery
///
/// Simulate a network partition where the validators are split
/// into two groups. Show that consensus cannot be reached without
/// a majority, and demonstrate recovery when the partition heals.
#[allow(dead_code)]
fn exercise_partition_recovery() {
    // TODO: Implement partition recovery simulation
    // Hints:
    // 1. Split validators into two groups
    // 2. Show neither group can reach consensus alone
    // 3. Reunite groups and show consensus resumes
}

/// Exercise 3: Performance Under Load
///
/// Create multiple concurrent proposals and measure the
/// consensus engine's performance. Find the breaking point
/// where consensus starts taking longer than the timeout.
#[allow(dead_code)]
async fn exercise_performance_test() {
    // TODO: Implement performance testing
    // Hints:
    // 1. Create 100+ proposals concurrently
    // 2. Measure average consensus time
    // 3. Identify bottlenecks
    // 4. Suggest optimizations
}
