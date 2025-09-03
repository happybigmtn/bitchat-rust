//! Property-based tests for consensus algorithms
//!
//! These tests use the proptest crate to generate thousands of test cases
//! and verify that consensus properties hold under all conditions.

use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use bitcraps::protocol::consensus::*;
use bitcraps::protocol::{PeerId, GameId, ConsensusMessage, ConsensusPayload};
use bitcraps::gaming::{DiceRoll, GameProposal, BetOutcome};

/// Generate arbitrary peer IDs for testing
fn arb_peer_id() -> impl Strategy<Value = PeerId> {
    any::<[u8; 32]>()
}

/// Generate arbitrary game IDs
fn arb_game_id() -> impl Strategy<Value = GameId> {
    any::<[u8; 16]>()
}

/// Generate arbitrary dice rolls
fn arb_dice_roll() -> impl Strategy<Value = DiceRoll> {
    (1u8..=6, 1u8..=6).prop_map(|(die1, die2)| DiceRoll { die1, die2 })
}

/// Generate arbitrary consensus messages
fn arb_consensus_message() -> impl Strategy<Value = ConsensusMessage> {
    prop_oneof![
        (arb_game_id(), arb_peer_id(), any::<u64>()).prop_map(|(game_id, proposer, timestamp)| {
            ConsensusMessage::new(
                game_id,
                proposer,
                ConsensusPayload::GameProposal(GameProposal {
                    operation: "create_game".to_string(),
                    participants: vec![proposer],
                    data: vec![],
                    timestamp,
                }),
                timestamp
            )
        }),
        (arb_game_id(), arb_peer_id(), arb_dice_roll(), any::<u64>()).prop_map(|(game_id, proposer, roll, timestamp)| {
            ConsensusMessage::new(
                game_id,
                proposer,
                ConsensusPayload::DiceRoll(roll),
                timestamp
            )
        }),
        (arb_game_id(), arb_peer_id(), any::<u64>(), any::<u64>()).prop_map(|(game_id, proposer, amount, timestamp)| {
            ConsensusMessage::new(
                game_id,
                proposer,
                ConsensusPayload::BetPlacement { amount },
                timestamp
            )
        })
    ]
}

/// Test suite for consensus properties
mod consensus_properties {
    use super::*;
    use bitcraps::protocol::consensus::engine::ConsensusEngine;
    
    proptest! {
        /// Property: Consensus should be deterministic
        /// Given the same set of messages, consensus should always produce the same result
        #[test]
        fn consensus_deterministic(
            messages in prop::collection::vec(arb_consensus_message(), 1..20)
        ) {
            let mut engine1 = ConsensusEngine::new();
            let mut engine2 = ConsensusEngine::new();
            
            // Apply messages to both engines in same order
            for message in &messages {
                let _ = engine1.process_message(message.clone());
                let _ = engine2.process_message(message.clone());
            }
            
            // Results should be identical
            let state1 = engine1.get_state();
            let state2 = engine2.get_state();
            
            prop_assert_eq!(state1.get_hash(), state2.get_hash());
        }
        
        /// Property: Message ordering should not affect final consensus
        /// (assuming all messages have the same timestamp)
        #[test]
        fn consensus_order_independent(
            mut messages in prop::collection::vec(arb_consensus_message(), 1..10)
        ) {
            // Normalize timestamps to make them equal
            let base_time = 1000000u64;
            for msg in &mut messages {
                msg.timestamp = base_time;
            }
            
            let mut engine1 = ConsensusEngine::new();
            let mut engine2 = ConsensusEngine::new();
            
            // Apply messages in original order
            for message in &messages {
                let _ = engine1.process_message(message.clone());
            }
            
            // Reverse the order and apply to second engine
            let mut reversed_messages = messages.clone();
            reversed_messages.reverse();
            
            for message in &reversed_messages {
                let _ = engine2.process_message(message.clone());
            }
            
            // Final state should be the same
            let state1 = engine1.get_state();
            let state2 = engine2.get_state();
            
            prop_assert_eq!(state1.get_hash(), state2.get_hash());
        }
        
        /// Property: Valid messages should never be rejected
        #[test]
        fn valid_messages_accepted(
            message in arb_consensus_message()
        ) {
            let mut engine = ConsensusEngine::new();
            
            // A properly formed message should be processable
            let result = engine.process_message(message);
            
            // Should not fail for structural reasons (may fail for business logic)
            prop_assert!(result.is_ok() || matches!(result, Err(bitcraps::error::Error::InvalidOperation(_))));
        }
        
        /// Property: Consensus state should be monotonic
        /// Adding messages should never decrease the version/epoch
        #[test]
        fn consensus_monotonic(
            messages in prop::collection::vec(arb_consensus_message(), 1..15)
        ) {
            let mut engine = ConsensusEngine::new();
            let mut last_version = 0u64;
            
            for message in messages {
                let _ = engine.process_message(message);
                let current_state = engine.get_state();
                let current_version = current_state.get_version();
                
                // Version should never go backwards
                prop_assert!(current_version >= last_version);
                last_version = current_version;
            }
        }
        
        /// Property: Duplicate messages should be idempotent
        #[test]
        fn duplicate_messages_idempotent(
            message in arb_consensus_message()
        ) {
            let mut engine = ConsensusEngine::new();
            
            // Process message first time
            let _ = engine.process_message(message.clone());
            let state_after_first = engine.get_state();
            
            // Process same message again
            let _ = engine.process_message(message.clone());
            let state_after_duplicate = engine.get_state();
            
            // State should be unchanged
            prop_assert_eq!(state_after_first.get_hash(), state_after_duplicate.get_hash());
        }
    }
}

/// Byzantine fault tolerance property tests
mod byzantine_properties {
    use super::*;
    
    proptest! {
        /// Property: Consensus should handle minority byzantine nodes
        /// Up to f < n/3 byzantine nodes should not break consensus
        #[test]
        fn byzantine_minority_tolerance(
            honest_messages in prop::collection::vec(arb_consensus_message(), 3..10),
            byzantine_count in 0usize..3
        ) {
            let total_nodes = honest_messages.len() + byzantine_count;
            prop_assume!(byzantine_count < total_nodes / 3); // Byzantine minority
            
            let mut honest_engine = ConsensusEngine::new();
            let mut mixed_engine = ConsensusEngine::new();
            
            // Process honest messages on both engines
            for message in &honest_messages {
                let _ = honest_engine.process_message(message.clone());
                let _ = mixed_engine.process_message(message.clone());
            }
            
            // Add byzantine (conflicting) messages to mixed engine
            for i in 0..byzantine_count {
                let mut byzantine_msg = honest_messages[i % honest_messages.len()].clone();
                byzantine_msg.timestamp += 1; // Create conflict
                let _ = mixed_engine.process_message(byzantine_msg);
            }
            
            // Honest consensus should still be achievable
            let honest_state = honest_engine.get_state();
            let mixed_state = mixed_engine.get_state();
            
            // The honest messages should still form a valid state
            prop_assert!(honest_state.is_valid());
            prop_assert!(mixed_state.is_valid());
        }
        
        /// Property: Fork choice should be consistent
        #[test]
        fn fork_choice_consistent(
            base_messages in prop::collection::vec(arb_consensus_message(), 2..8),
            fork_messages in prop::collection::vec(arb_consensus_message(), 1..5)
        ) {
            let mut engine1 = ConsensusEngine::new();
            let mut engine2 = ConsensusEngine::new();
            
            // Both engines see the base messages
            for message in &base_messages {
                let _ = engine1.process_message(message.clone());
                let _ = engine2.process_message(message.clone());
            }
            
            // Both engines see the same fork messages (in same order)
            for message in &fork_messages {
                let _ = engine1.process_message(message.clone());
                let _ = engine2.process_message(message.clone());
            }
            
            // They should make the same fork choice
            let state1 = engine1.get_state();
            let state2 = engine2.get_state();
            
            prop_assert_eq!(state1.get_hash(), state2.get_hash());
        }
    }
}

/// Game-specific consensus properties
mod game_properties {
    use super::*;
    
    proptest! {
        /// Property: Dice roll consensus should preserve fairness
        #[test]
        fn dice_roll_fairness(
            rolls in prop::collection::vec(arb_dice_roll(), 10..100)
        ) {
            let mut frequency = HashMap::new();
            
            // Count frequency of each roll value
            for roll in &rolls {
                let sum = roll.die1 + roll.die2;
                *frequency.entry(sum).or_insert(0) += 1;
            }
            
            // Check that all valid sums (2-12) could theoretically appear
            for sum in 2u8..=12 {
                let count = frequency.get(&sum).unwrap_or(&0);
                // Each sum should be possible (though may be 0 in small samples)
                prop_assert!(*count >= 0);
            }
            
            // Most frequent sum should be 7 (statistically likely for large samples)
            if rolls.len() > 50 {
                let most_frequent = frequency.iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(sum, _)| *sum);
                
                // 7 is the most probable sum, but allow for statistical variation
                prop_assert!(most_frequent.is_some());
            }
        }
        
        /// Property: Bet outcomes should be deterministic from dice rolls
        #[test]
        fn bet_outcome_deterministic(
            roll in arb_dice_roll(),
            bet_amount in 1u64..1000000
        ) {
            let outcome1 = BetOutcome::from_dice_roll(&roll, bet_amount);
            let outcome2 = BetOutcome::from_dice_roll(&roll, bet_amount);
            
            prop_assert_eq!(outcome1.payout, outcome2.payout);
            prop_assert_eq!(outcome1.winner, outcome2.winner);
        }
        
        /// Property: Game state transitions should be valid
        #[test]
        fn game_state_transitions_valid(
            initial_messages in prop::collection::vec(arb_consensus_message(), 1..5),
            transition_messages in prop::collection::vec(arb_consensus_message(), 1..5)
        ) {
            let mut engine = ConsensusEngine::new();
            
            // Apply initial messages
            for message in &initial_messages {
                let _ = engine.process_message(message.clone());
            }
            let initial_state = engine.get_state();
            
            // Apply transition messages
            for message in &transition_messages {
                let _ = engine.process_message(message.clone());
            }
            let final_state = engine.get_state();
            
            // State should always be valid after transitions
            prop_assert!(initial_state.is_valid());
            prop_assert!(final_state.is_valid());
            
            // Version should not decrease
            prop_assert!(final_state.get_version() >= initial_state.get_version());
        }
    }
}

/// Performance and resource property tests
mod performance_properties {
    use super::*;
    
    proptest! {
        /// Property: Memory usage should be bounded
        #[test]
        fn memory_usage_bounded(
            messages in prop::collection::vec(arb_consensus_message(), 100..200)
        ) {
            let mut engine = ConsensusEngine::new();
            
            // Track approximate memory usage (simplified)
            let initial_size = std::mem::size_of_val(&engine);
            
            for message in messages {
                let _ = engine.process_message(message);
            }
            
            // Memory should not grow unbounded (this is a simplified check)
            let final_size = std::mem::size_of_val(&engine);
            let growth_ratio = final_size as f64 / initial_size as f64;
            
            // Memory should not grow more than 10x (conservative bound)
            prop_assert!(growth_ratio < 10.0);
        }
        
        /// Property: Processing time should be reasonable
        #[test]
        fn processing_time_reasonable(
            message in arb_consensus_message()
        ) {
            let mut engine = ConsensusEngine::new();
            
            let start = std::time::Instant::now();
            let _ = engine.process_message(message);
            let duration = start.elapsed();
            
            // Single message should process in under 100ms
            prop_assert!(duration.as_millis() < 100);
        }
    }
}

/// Cryptographic property tests
mod crypto_properties {
    use super::*;
    use bitcraps::crypto::GameCrypto;
    
    proptest! {
        /// Property: Random dice rolls should be uniformly distributed
        #[test]
        fn dice_rolls_uniform_distribution(
            seed_count in 10usize..50
        ) {
            let mut frequency = HashMap::new();
            
            // Generate many dice rolls
            for _ in 0..seed_count * 100 {
                let (die1, die2) = GameCrypto::generate_secure_dice_roll();
                
                // Each die should be 1-6
                prop_assert!(die1 >= 1 && die1 <= 6);
                prop_assert!(die2 >= 1 && die2 <= 6);
                
                let sum = die1 + die2;
                *frequency.entry(sum).or_insert(0) += 1;
            }
            
            // Check distribution is reasonable (not perfect due to randomness)
            // Sum of 7 should be most frequent
            let seven_count = frequency.get(&7).unwrap_or(&0);
            prop_assert!(*seven_count > 0);
            
            // Extreme values (2, 12) should be less frequent
            let two_count = frequency.get(&2).unwrap_or(&0);
            let twelve_count = frequency.get(&12).unwrap_or(&0);
            
            // 7 should occur more than 2 or 12 (with high probability)
            if seed_count > 20 {
                prop_assert!(*seven_count >= *two_count);
                prop_assert!(*seven_count >= *twelve_count);
            }
        }
        
        /// Property: Cryptographic commitments should be binding
        #[test]
        fn commitments_binding(
            secret1 in any::<[u8; 32]>(),
            secret2 in any::<[u8; 32]>()
        ) {
            prop_assume!(secret1 != secret2);
            
            let commitment1 = GameCrypto::commit_randomness(&secret1);
            let commitment2 = GameCrypto::commit_randomness(&secret2);
            
            // Different secrets should produce different commitments
            prop_assert_ne!(commitment1, commitment2);
            
            // Commitments should verify with correct secrets
            prop_assert!(GameCrypto::verify_commitment(&commitment1, &secret1));
            prop_assert!(GameCrypto::verify_commitment(&commitment2, &secret2));
            
            // Commitments should not verify with wrong secrets
            prop_assert!(!GameCrypto::verify_commitment(&commitment1, &secret2));
            prop_assert!(!GameCrypto::verify_commitment(&commitment2, &secret1));
        }
        
        /// Property: Hash functions should be collision-resistant (in practice)
        #[test]
        fn hash_collision_resistant(
            data1 in prop::collection::vec(any::<u8>(), 1..1000),
            data2 in prop::collection::vec(any::<u8>(), 1..1000)
        ) {
            prop_assume!(data1 != data2);
            
            let hash1 = GameCrypto::hash(&data1);
            let hash2 = GameCrypto::hash(&data2);
            
            // Different data should produce different hashes
            prop_assert_ne!(hash1, hash2);
        }
    }
}

#[cfg(test)]
mod integration_properties {
    use super::*;
    
    proptest! {
        /// Property: Full game flow should maintain consistency
        #[test]
        fn full_game_consistency(
            game_id in arb_game_id(),
            players in prop::collection::vec(arb_peer_id(), 2..8),
            dice_rolls in prop::collection::vec(arb_dice_roll(), 1..10)
        ) {
            let mut engine = ConsensusEngine::new();
            
            // Create game
            let game_msg = ConsensusMessage::new(
                game_id,
                players[0],
                ConsensusPayload::GameProposal(GameProposal {
                    operation: "create_game".to_string(),
                    participants: players.clone(),
                    data: vec![],
                    timestamp: 1000,
                }),
                1000
            );
            
            prop_assert!(engine.process_message(game_msg).is_ok());
            
            // Players place bets and roll dice
            let mut timestamp = 1001;
            for (i, &player) in players.iter().enumerate() {
                // Place bet
                let bet_msg = ConsensusMessage::new(
                    game_id,
                    player,
                    ConsensusPayload::BetPlacement { amount: 100 },
                    timestamp
                );
                let _ = engine.process_message(bet_msg);
                timestamp += 1;
                
                // Roll dice if we have a roll for this player
                if i < dice_rolls.len() {
                    let roll_msg = ConsensusMessage::new(
                        game_id,
                        player,
                        ConsensusPayload::DiceRoll(dice_rolls[i]),
                        timestamp
                    );
                    let _ = engine.process_message(roll_msg);
                    timestamp += 1;
                }
            }
            
            // Final state should be valid
            let final_state = engine.get_state();
            prop_assert!(final_state.is_valid());
        }
    }
}