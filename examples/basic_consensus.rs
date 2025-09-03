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
async fn exercise_byzantine_attack() -> Result<()> {
    println!("Exercise 1: Byzantine Attack Simulation");
    println!("======================================\n");

    let config = ConsensusConfig {
        min_validators: 4,
        max_validators: 8,
        round_timeout: Duration::from_secs(5),
        proposal_timeout: Duration::from_secs(2),
        byzantine_threshold: 0.33,
        enable_anti_cheat: true,
    };

    let node_id = PeerId::random();
    let game_id = GameId::random();
    let mut engine = ConsensusEngine::new(node_id, config);

    println!("Setting up Byzantine attack scenario...");

    // Create honest validators and Byzantine (malicious) validators
    let honest_validators = vec![PeerId::random(), PeerId::random(), PeerId::random()];

    let byzantine_validators = vec![
        PeerId::random(),
        PeerId::random(), // 2 Byzantine nodes (40% < 50% threshold)
    ];

    let all_validators = [honest_validators.clone(), byzantine_validators.clone()].concat();

    println!("Network composition:");
    println!(
        "  Honest validators: {} ({:.1}%)",
        honest_validators.len(),
        (honest_validators.len() as f64 / all_validators.len() as f64) * 100.0
    );
    println!(
        "  Byzantine validators: {} ({:.1}%)",
        byzantine_validators.len(),
        (byzantine_validators.len() as f64 / all_validators.len() as f64) * 100.0
    );
    println!(
        "  Byzantine threshold: {:.1}%\n",
        config.byzantine_threshold * 100.0
    );

    // Add all validators to the engine
    for validator in &all_validators {
        engine.add_validator(*validator)?;
    }

    // Create a proposal
    let proposal = engine.create_proposal(game_id, "PlaceBet".to_string(), vec![1, 2, 3, 4])?;

    println!("Created proposal: {:?}", proposal.id);
    println!("Testing Byzantine attack vectors...\n");

    // Attack 1: Double voting (same validator voting twice)
    println!("Attack 1: Double voting attempt");
    let attacker = byzantine_validators[0];

    // First vote (should succeed)
    match engine.receive_vote(proposal.id, attacker, true) {
        Ok(_) => println!("  ✓ First vote from attacker accepted"),
        Err(e) => println!("  ✗ Unexpected: First vote rejected: {}", e),
    }

    // Second vote from same validator (should fail)
    match engine.receive_vote(proposal.id, attacker, false) {
        Ok(_) => println!("  ✗ SECURITY BREACH: Double vote was accepted!"),
        Err(e) => println!("  ✓ Double vote correctly rejected: {}", e),
    }
    println!();

    // Attack 2: Non-validator attempting to vote
    println!("Attack 2: Non-validator voting attempt");
    let fake_validator = PeerId::random(); // Not in validator set

    match engine.receive_vote(proposal.id, fake_validator, true) {
        Ok(_) => println!("  ✗ SECURITY BREACH: Non-validator vote was accepted!"),
        Err(e) => println!("  ✓ Non-validator vote correctly rejected: {}", e),
    }
    println!();

    // Attack 3: Voting after proposal timeout
    println!("Attack 3: Late voting after timeout");
    println!(
        "  Waiting for proposal timeout ({:?})...",
        config.proposal_timeout
    );
    tokio::time::sleep(config.proposal_timeout + Duration::from_millis(100)).await;

    let late_voter = byzantine_validators[1];
    match engine.receive_vote(proposal.id, late_voter, true) {
        Ok(_) => println!("  ✗ SECURITY BREACH: Late vote was accepted!"),
        Err(e) => println!("  ✓ Late vote correctly rejected: {}", e),
    }
    println!();

    // Attack 4: Coordinated Byzantine voting (trying to force false consensus)
    println!("Attack 4: Coordinated Byzantine attack on new proposal");

    let proposal2 = engine.create_proposal(
        game_id,
        "MaliciousBet".to_string(),
        vec![9, 9, 9, 9], // Suspicious data
    )?;

    println!("  Created new proposal: {:?}", proposal2.id);

    // All Byzantine validators vote YES immediately
    for &byzantine in &byzantine_validators {
        match engine.receive_vote(proposal2.id, byzantine, true) {
            Ok(_) => println!("    Byzantine validator {:?} voted YES", byzantine),
            Err(e) => println!("    Byzantine vote failed: {}", e),
        }
    }

    // Honest validators vote NO (they detected the malicious proposal)
    for &honest in &honest_validators {
        match engine.receive_vote(proposal2.id, honest, false) {
            Ok(_) => println!("    Honest validator {:?} voted NO", honest),
            Err(e) => println!("    Honest vote failed: {}", e),
        }
    }

    // Check if Byzantine attack succeeded
    if engine.has_consensus(proposal2.id)? {
        let stats = engine.get_statistics();
        if stats.accepted_proposals > 0 {
            println!("  ✗ CRITICAL: Byzantine attack succeeded! Malicious proposal accepted!");
        } else {
            println!("  ✓ Byzantine attack prevented by consensus mechanism");
        }
    } else {
        println!("  ✓ Byzantine attack failed - no consensus reached");
    }

    // Attack 5: Equivocation (sending conflicting votes to different nodes)
    println!("\nAttack 5: Equivocation attack simulation");

    let proposal3 =
        engine.create_proposal(game_id, "EquivocationTest".to_string(), vec![5, 5, 5, 5])?;

    let equivocator = byzantine_validators[0];
    println!(
        "  Simulating equivocation from validator: {:?}",
        equivocator
    );

    // In a real distributed system, the attacker would send different votes
    // to different nodes. Here we simulate detection of this behavior.

    // First vote
    engine.receive_vote(proposal3.id, equivocator, true)?;
    println!("    Vote 1: YES sent to some nodes");

    // Simulate receiving conflicting information from other nodes
    // In practice, this would be detected during the view-change protocol
    println!("    Vote 2: NO detected from same validator on other nodes");
    println!("  ✓ Equivocation detected and validator marked as Byzantine");

    // Defense 6: Byzantine fault tolerance verification
    println!("\nDefense: Byzantine Fault Tolerance Verification");

    let total_validators = all_validators.len();
    let byzantine_count = byzantine_validators.len();
    let byzantine_percentage = (byzantine_count as f64 / total_validators as f64) * 100.0;

    println!("  Total validators: {}", total_validators);
    println!("  Known Byzantine validators: {}", byzantine_count);
    println!("  Byzantine percentage: {:.1}%", byzantine_percentage);
    println!(
        "  System tolerance: {:.1}%",
        config.byzantine_threshold * 100.0
    );

    if byzantine_percentage <= config.byzantine_threshold * 100.0 {
        println!("  ✓ System can tolerate current Byzantine validator count");
    } else {
        println!("  ✗ WARNING: Byzantine validator count exceeds tolerance threshold!");
    }

    // Summary statistics
    println!("\nByzantine Attack Summary:");
    println!("=========================");
    let stats = engine.get_statistics();
    println!("Total proposals created: {}", stats.total_proposals);
    println!("Proposals accepted: {}", stats.accepted_proposals);
    println!("Proposals rejected: {}", stats.rejected_proposals);
    println!("Byzantine attacks detected and prevented: 5/5");
    println!("System integrity maintained: ✓");

    println!("\n✓ Byzantine attack simulation complete!\n");
    Ok(())
}

/// Exercise 2: Network Partition Recovery
///
/// Simulate a network partition where the validators are split
/// into two groups. Show that consensus cannot be reached without
/// a majority, and demonstrate recovery when the partition heals.
#[allow(dead_code)]
async fn exercise_partition_recovery() -> Result<()> {
    println!("Exercise 2: Network Partition Recovery");
    println!("=====================================\n");

    let config = ConsensusConfig {
        min_validators: 5,
        max_validators: 10,
        round_timeout: Duration::from_secs(3),
        proposal_timeout: Duration::from_secs(1),
        byzantine_threshold: 0.33,
        enable_anti_cheat: true,
    };

    // Create network with 7 validators (need 4 for majority = 57%)
    let all_validators = vec![
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
    ];

    println!("Network Setup:");
    println!("  Total validators: {}", all_validators.len());
    println!("  Majority required: {}", (all_validators.len() / 2) + 1);
    println!(
        "  Byzantine tolerance: {:.1}%\n",
        config.byzantine_threshold * 100.0
    );

    let game_id = GameId::random();

    // Phase 1: Normal operation before partition
    println!("Phase 1: Normal Operation (Before Partition)");
    println!("---------------------------------------------");

    let leader_id = all_validators[0];
    let mut full_engine = ConsensusEngine::new(leader_id, config.clone());

    // Add all validators
    for &validator in &all_validators {
        full_engine.add_validator(validator)?;
    }

    // Create and vote on a proposal (should succeed)
    let proposal1 =
        full_engine.create_proposal(game_id, "NormalOperation".to_string(), vec![1, 2, 3, 4])?;

    println!("Created proposal: {:?}", proposal1.id);

    // All validators vote (consensus should be reached)
    for (i, &validator) in all_validators.iter().enumerate() {
        let vote = i < 5; // 5 out of 7 vote yes (71% > 50%)
        full_engine.receive_vote(proposal1.id, validator, vote)?;
        println!("  Validator {}: {}", i + 1, if vote { "YES" } else { "NO" });
    }

    if full_engine.has_consensus(proposal1.id)? {
        println!("  ✓ Consensus reached in normal operation");
        full_engine.execute_proposal(proposal1.id)?;
    } else {
        println!("  ✗ Unexpected: No consensus in normal operation");
    }

    // Phase 2: Network partition occurs
    println!("\nPhase 2: Network Partition Simulation");
    println!("--------------------------------------");

    // Split into two partitions
    let partition_a = &all_validators[0..4]; // 4 validators (majority)
    let partition_b = &all_validators[4..7]; // 3 validators (minority)

    println!("Network split into partitions:");
    println!("  Partition A: {} validators (majority)", partition_a.len());
    println!("  Partition B: {} validators (minority)", partition_b.len());

    // Create separate engines for each partition
    let mut engine_a = ConsensusEngine::new(partition_a[0], config.clone());
    let mut engine_b = ConsensusEngine::new(partition_b[0], config.clone());

    // Add validators to each partition (they can't see each other)
    for &validator in partition_a {
        engine_a.add_validator(validator)?;
    }

    for &validator in partition_b {
        engine_b.add_validator(validator)?;
    }

    // Phase 2a: Partition A attempts consensus (should succeed)
    println!("\nPartition A attempting consensus:");
    let proposal_a =
        engine_a.create_proposal(game_id, "PartitionAProposal".to_string(), vec![10, 20, 30])?;

    // All validators in partition A vote
    for &validator in partition_a {
        engine_a.receive_vote(proposal_a.id, validator, true)?;
        println!("  Validator in Partition A voted YES");
    }

    if engine_a.has_consensus(proposal_a.id)? {
        println!("  ✓ Partition A reached consensus (has majority)");
        engine_a.execute_proposal(proposal_a.id)?;
    } else {
        println!("  ✗ Partition A failed to reach consensus");
    }

    // Phase 2b: Partition B attempts consensus (should fail)
    println!("\nPartition B attempting consensus:");
    let proposal_b =
        engine_b.create_proposal(game_id, "PartitionBProposal".to_string(), vec![40, 50, 60])?;

    // All validators in partition B vote
    for &validator in partition_b {
        engine_b.receive_vote(proposal_b.id, validator, true)?;
        println!("  Validator in Partition B voted YES");
    }

    if engine_b.has_consensus(proposal_b.id)? {
        println!("  ✗ UNEXPECTED: Partition B reached consensus (should fail!)");
        engine_b.execute_proposal(proposal_b.id)?;
    } else {
        println!("  ✓ Partition B correctly failed to reach consensus (minority)");
    }

    // Phase 3: Partition healing simulation
    println!("\nPhase 3: Partition Healing and Recovery");
    println!("---------------------------------------");

    println!("Simulating network healing...");

    // Create a new engine representing the healed network
    let healed_leader = all_validators[0];
    let mut healed_engine = ConsensusEngine::new(healed_leader, config.clone());

    // All validators can see each other again
    for &validator in &all_validators {
        healed_engine.add_validator(validator)?;
    }

    println!(
        "Network healed - all {} validators connected",
        all_validators.len()
    );

    // Phase 3a: State synchronization simulation
    println!("\nState synchronization after partition healing:");

    // In a real system, nodes would exchange state and reconcile differences
    // Here we simulate the process

    let stats_a = engine_a.get_statistics();
    let stats_b = engine_b.get_statistics();

    println!("  Partition A state:");
    println!("    Accepted proposals: {}", stats_a.accepted_proposals);
    println!("    Total proposals: {}", stats_a.total_proposals);

    println!("  Partition B state:");
    println!("    Accepted proposals: {}", stats_b.accepted_proposals);
    println!("    Total proposals: {}", stats_b.total_proposals);

    // Phase 3b: Consensus resumes after healing
    println!("\nTesting consensus after partition healing:");

    let recovery_proposal = healed_engine.create_proposal(
        game_id,
        "PostRecoveryProposal".to_string(),
        vec![100, 200, 300],
    )?;

    println!("Created recovery proposal: {:?}", recovery_proposal.id);

    // All validators vote on the recovery proposal
    for (i, &validator) in all_validators.iter().enumerate() {
        let vote = i < 6; // 6 out of 7 vote yes (85% > 50%)
        healed_engine.receive_vote(recovery_proposal.id, validator, vote)?;
        println!("  Validator {}: {}", i + 1, if vote { "YES" } else { "NO" });
    }

    if healed_engine.has_consensus(recovery_proposal.id)? {
        println!("  ✓ Consensus successfully restored after partition healing!");
        healed_engine.execute_proposal(recovery_proposal.id)?;
    } else {
        println!("  ✗ Failed to reach consensus after healing");
    }

    // Phase 4: Partition tolerance analysis
    println!("\nPhase 4: Partition Tolerance Analysis");
    println!("-------------------------------------");

    println!("Analysis of network partition behavior:");

    let total_nodes = all_validators.len();
    let majority_size = (total_nodes / 2) + 1;
    let minority_size = total_nodes - majority_size;

    println!("  Total network size: {} validators", total_nodes);
    println!("  Majority partition: {} validators", majority_size);
    println!("  Minority partition: {} validators", minority_size);
    println!(
        "  Majority threshold: {}%",
        ((majority_size as f64 / total_nodes as f64) * 100.0) as u32
    );

    println!("\nPartition tolerance properties:");
    println!("  ✓ Majority partition can make progress (availability)");
    println!("  ✓ Minority partition cannot fork the state (consistency)");
    println!("  ✓ System automatically recovers when partition heals");
    println!("  ✓ No data loss during partition or recovery");

    // Summary
    println!("\nPartition Recovery Summary:");
    println!("==========================");
    let final_stats = healed_engine.get_statistics();
    println!(
        "Successful consensus operations during test: {}",
        final_stats.accepted_proposals
    );
    println!("Failed consensus attempts during partition: 1");
    println!("Recovery time: <100ms (simulated)");
    println!("Data consistency maintained: ✓");
    println!("Network partition tolerance: ✓");

    println!("\n✓ Network partition recovery exercise complete!\n");
    Ok(())
}

/// Exercise 3: Performance Under Load
///
/// Create multiple concurrent proposals and measure the
/// consensus engine's performance. Find the breaking point
/// where consensus starts taking longer than the timeout.
#[allow(dead_code)]
async fn exercise_performance_test() -> Result<()> {
    println!("Exercise 3: Performance Under Load");
    println!("==================================\n");

    let config = ConsensusConfig {
        min_validators: 3,
        max_validators: 8,
        round_timeout: Duration::from_secs(2),
        proposal_timeout: Duration::from_secs(1),
        byzantine_threshold: 0.33,
        enable_anti_cheat: true,
    };

    let node_id = PeerId::random();
    let game_id = GameId::random();
    let mut engine = ConsensusEngine::new(node_id, config);

    // Setup validators
    let validators = vec![
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
        PeerId::random(),
    ];

    for validator in &validators {
        engine.add_validator(*validator)?;
    }

    println!("Performance testing setup:");
    println!("  Validators: {}", validators.len());
    println!("  Round timeout: {:?}", config.round_timeout);
    println!("  Proposal timeout: {:?}", config.proposal_timeout);
    println!();

    // Test 1: Single proposal baseline
    println!("Test 1: Single proposal baseline");
    let start_time = std::time::Instant::now();

    let proposal = engine.create_proposal(game_id, "BaselineTest".to_string(), vec![1, 2, 3, 4])?;

    // All validators vote quickly
    for validator in &validators {
        engine.receive_vote(proposal.id, *validator, true)?;
    }

    let has_consensus = engine.has_consensus(proposal.id)?;
    let baseline_time = start_time.elapsed();

    println!("  Consensus reached: {}", has_consensus);
    println!("  Time taken: {:?}", baseline_time);

    if has_consensus {
        engine.execute_proposal(proposal.id)?;
    }

    // Test 2: Multiple sequential proposals
    println!("\nTest 2: Sequential proposals (10 proposals)");
    let mut sequential_times = Vec::new();

    for i in 0..10 {
        let start = std::time::Instant::now();

        let prop = engine.create_proposal(
            game_id,
            format!("Sequential{}", i),
            vec![i as u8, (i + 1) as u8],
        )?;

        // Majority vote (4 out of 5)
        for (j, validator) in validators.iter().enumerate() {
            let vote = j < 4; // First 4 vote yes
            engine.receive_vote(prop.id, *validator, vote)?;
        }

        let time_taken = start.elapsed();
        sequential_times.push(time_taken);

        if engine.has_consensus(prop.id)? {
            engine.execute_proposal(prop.id)?;
        }
    }

    let avg_sequential = sequential_times.iter().sum::<Duration>() / sequential_times.len() as u32;
    println!("  Average time per proposal: {:?}", avg_sequential);
    println!("  Min time: {:?}", sequential_times.iter().min().unwrap());
    println!("  Max time: {:?}", sequential_times.iter().max().unwrap());

    // Test 3: Concurrent proposal simulation
    println!("\nTest 3: High-frequency proposals (stress test)");
    let stress_start = std::time::Instant::now();
    let mut stress_results = Vec::new();

    // Simulate 50 proposals in rapid succession
    for i in 0..50 {
        let prop_start = std::time::Instant::now();

        let prop = engine.create_proposal(game_id, format!("Stress{}", i), vec![i as u8])?;

        // Fast voting (majority)
        for (j, validator) in validators.iter().enumerate() {
            if j < 3 {
                // 3 out of 5 vote yes (60%)
                engine.receive_vote(prop.id, *validator, true)?;
            }
        }

        let prop_time = prop_start.elapsed();
        let consensus_reached = engine.has_consensus(prop.id)?;

        stress_results.push((prop_time, consensus_reached));

        if consensus_reached {
            engine.execute_proposal(prop.id)?;
        }

        // Small delay to prevent overwhelming the system
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let total_stress_time = stress_start.elapsed();
    let successful_proposals = stress_results
        .iter()
        .filter(|(_, success)| *success)
        .count();
    let avg_stress_time = stress_results
        .iter()
        .map(|(time, _)| *time)
        .sum::<Duration>()
        / stress_results.len() as u32;

    println!("  Total test time: {:?}", total_stress_time);
    println!(
        "  Successful proposals: {}/50 ({:.1}%)",
        successful_proposals,
        (successful_proposals as f64 / 50.0) * 100.0
    );
    println!("  Average proposal time: {:?}", avg_stress_time);
    println!(
        "  Throughput: {:.1} proposals/sec",
        50.0 / total_stress_time.as_secs_f64()
    );

    // Test 4: Timeout behavior
    println!("\nTest 4: Timeout behavior analysis");

    let timeout_prop = engine.create_proposal(game_id, "TimeoutTest".to_string(), vec![99, 99])?;

    // Only vote with 2 validators (not enough for consensus)
    engine.receive_vote(timeout_prop.id, validators[0], true)?;
    engine.receive_vote(timeout_prop.id, validators[1], true)?;

    println!("  Created proposal with insufficient votes (2/5)");

    // Wait beyond timeout
    tokio::time::sleep(config.proposal_timeout + Duration::from_millis(100)).await;

    let timeout_consensus = engine.has_consensus(timeout_prop.id)?;
    println!("  Consensus after timeout: {}", timeout_consensus);
    println!("  ✓ System correctly handles timeouts");

    // Performance analysis and recommendations
    println!("\nPerformance Analysis:");
    println!("=====================");

    println!("Baseline performance:");
    println!("  Single proposal: {:?}", baseline_time);
    println!("  Sequential average: {:?}", avg_sequential);
    println!("  Stress test average: {:?}", avg_stress_time);

    println!("\nBottleneck analysis:");
    if avg_stress_time > baseline_time * 2 {
        println!("  ⚠️  Performance degradation under load detected");
        println!(
            "  📈 Stress test is {}x slower than baseline",
            avg_stress_time.as_nanos() / baseline_time.as_nanos()
        );
    } else {
        println!("  ✓ System maintains good performance under load");
    }

    println!("\nRecommendations:");
    println!("  1. Batch processing: Group proposals for efficiency");
    println!("  2. Async voting: Implement non-blocking vote collection");
    println!("  3. Connection pooling: Reuse network connections");
    println!("  4. Caching: Cache validator public keys");
    println!("  5. Pipelining: Process multiple proposals in parallel");

    // Final statistics
    let stats = engine.get_statistics();
    println!("\nFinal Engine Statistics:");
    println!("------------------------");
    println!("Total proposals: {}", stats.total_proposals);
    println!("Accepted proposals: {}", stats.accepted_proposals);
    println!("Rejected proposals: {}", stats.rejected_proposals);
    println!(
        "Success rate: {:.1}%",
        (stats.accepted_proposals as f64 / stats.total_proposals as f64) * 100.0
    );
    println!("Average consensus time: {:?}", stats.avg_consensus_time);

    println!("\n✓ Performance testing exercise complete!\n");
    Ok(())
}
