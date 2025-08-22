//! BitCraps Efficient Game Logic Demo
//! 
//! This example demonstrates the efficient data structures and algorithms
//! implemented for BitCraps, showcasing memory usage and performance improvements.

use std::time::Instant;
use bitcraps::protocol::efficient_game_state::{CompactGameState, StateSnapshot};
use bitcraps::protocol::efficient_bet_resolution::{EfficientBetResolver};
use bitcraps::protocol::efficient_consensus::{EfficientDiceConsensus, ConsensusConfig};
use bitcraps::protocol::efficient_history::{EfficientGameHistory, HistoryConfig};
use bitcraps::protocol::{BetType, CrapTokens, DiceRoll};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎲 BitCraps Efficient Game Logic Demo");
    println!("=====================================");
    
    demo_compact_game_state()?;
    demo_efficient_bet_resolution()?;
    demo_consensus_mechanisms()?;
    demo_history_storage()?;
    
    println!("\n🎉 Demo completed successfully!");
    Ok(())
}

/// Demonstrate compact game state efficiency
fn demo_compact_game_state() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎯 Compact Game State Demo");
    println!("--------------------------");
    
    let start = Instant::now();
    
    // Create game state - only ~64 bytes!
    let mut state = CompactGameState::new([1; 16], [2; 32]);
    
    // Efficient bit field operations
    state.set_roll_count(42);
    state.set_point(Some(8));
    state.set_fire_points(3);
    state.set_hot_streak(15);
    
    let creation_time = start.elapsed();
    
    // Memory usage analysis
    let memory_stats = state.memory_usage();
    println!("✓ State created in {:?}", creation_time);
    println!("✓ Static memory usage: {} bytes", memory_stats.static_bytes);
    println!("✓ Dynamic memory usage: {} bytes", memory_stats.dynamic_bytes);
    println!("✓ Total memory usage: {} bytes", memory_stats.total_bytes);
    println!("✓ Compression ratio: {:.2}", memory_stats.compression_ratio);
    
    // Demonstrate fast access
    let access_start = Instant::now();
    for _ in 0..10000 {
        let _ = state.get_roll_count();
        let _ = state.get_point();
        let _ = state.get_fire_points();
        let _ = state.get_hot_streak();
    }
    let access_time = access_start.elapsed();
    println!("✓ 40,000 field accesses in {:?} ({:.1} ns/access)", 
             access_time, access_time.as_nanos() as f64 / 40000.0);
    
    // Demonstrate copy-on-write
    let cow_start = Instant::now();
    let state1 = state.clone();
    let state2 = state1.clone();
    let mut state3 = state2.clone();
    state3.make_mutable(); // Triggers copy-on-write
    let cow_time = cow_start.elapsed();
    println!("✓ Copy-on-write (3 clones + 1 mutation) in {:?}", cow_time);
    
    // State snapshots
    let snapshot_start = Instant::now();
    let mut snapshot = StateSnapshot::create(&state);
    snapshot.add_delta(bitcraps::protocol::efficient_game_state::StateDelta::RollProcessed {
        roll: DiceRoll::new(3, 4)?
    });
    let _reconstructed = snapshot.reconstruct()?;
    let snapshot_time = snapshot_start.elapsed();
    println!("✓ Snapshot + delta + reconstruction in {:?}", snapshot_time);
    
    Ok(())
}

/// Demonstrate efficient bet resolution
fn demo_efficient_bet_resolution() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎰 Efficient Bet Resolution Demo");
    println!("--------------------------------");
    
    let mut resolver = EfficientBetResolver::new();
    let state = CompactGameState::new([1; 16], [2; 32]);
    let dice_roll = DiceRoll::new(3, 4)?; // Lucky 7!
    
    // Create a batch of bets
    let active_bets = vec![
        (BetType::Pass, [1; 32], CrapTokens::new_unchecked(100)),
        (BetType::Field, [2; 32], CrapTokens::new_unchecked(50)),
        (BetType::Yes6, [3; 32], CrapTokens::new_unchecked(25)),
        (BetType::Hard8, [4; 32], CrapTokens::new_unchecked(10)),
        (BetType::Next7, [5; 32], CrapTokens::new_unchecked(5)),
    ];
    
    // Benchmark bet resolution
    let start = Instant::now();
    let mut total_resolutions = 0;
    
    for _ in 0..1000 {
        let resolutions = resolver.resolve_bets_fast(&state, dice_roll, &active_bets)?;
        total_resolutions += resolutions.len();
    }
    
    let resolution_time = start.elapsed();
    let stats = resolver.get_stats();
    
    println!("✓ Resolved {} bets in {:?}", total_resolutions, resolution_time);
    println!("✓ Average time per batch: {:.2} μs", 
             resolution_time.as_micros() as f64 / 1000.0);
    println!("✓ Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
    println!("✓ Lookup table size: {} bytes", stats.lookup_table_size);
    println!("✓ Throughput: {:.0} bets/second", 
             total_resolutions as f64 / resolution_time.as_secs_f64());
    
    Ok(())
}

/// Demonstrate consensus mechanisms
fn demo_consensus_mechanisms() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🤝 Consensus Mechanisms Demo");
    println!("----------------------------");
    
    let game_id = [1; 16];
    let participants = vec![[1; 32], [2; 32], [3; 32], [4; 32]];
    let initial_state = CompactGameState::new(game_id, participants[0]);
    
    let mut consensus = EfficientDiceConsensus::new(
        game_id,
        participants.clone(),
        ConsensusConfig::default()
    )?;
    
    let start = Instant::now();
    
    // Start consensus round
    let round_id = 1;
    consensus.start_round(round_id)?;
    
    // Simulate commit phase
    let nonces = [[10; 32], [20; 32], [30; 32], [40; 32]];
    for (i, &participant) in participants.iter().enumerate() {
        let commitment = [i as u8; 32]; // Simplified commitment
        consensus.add_commitment(round_id, participant, commitment)?;
    }
    
    // Simulate reveal phase
    for (i, &participant) in participants.iter().enumerate() {
        consensus.add_reveal(round_id, participant, nonces[i])?;
    }
    
    // Process round
    let dice_roll = consensus.process_round(round_id)?;
    let consensus_time = start.elapsed();
    
    let metrics = consensus.get_metrics();
    
    println!("✓ Consensus round completed in {:?}", consensus_time);
    println!("✓ Final dice roll: {} + {} = {}", 
             dice_roll.die1, dice_roll.die2, dice_roll.total());
    println!("✓ Memory usage: {} bytes", metrics.memory_usage_bytes);
    println!("✓ Rounds processed: {}", metrics.rounds_processed);
    println!("✓ Average round time: {:.2} ms", metrics.avg_round_time_ms);
    
    Ok(())
}

/// Demonstrate history storage
fn demo_history_storage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📚 History Storage Demo");
    println!("----------------------");
    
    let config = HistoryConfig {
        ring_buffer_size: 100,
        max_memory_bytes: 10 * 1024 * 1024, // 10MB
        enable_delta_compression: true,
        ..Default::default()
    };
    
    let mut history = EfficientGameHistory::new(config);
    let start = Instant::now();
    
    // Create and store multiple game histories
    for i in 0..50 {
        let game_history = bitcraps::protocol::efficient_history::CompactGameHistory {
            game_id: [i as u8; 16],
            initial_state: bitcraps::protocol::efficient_history::CompressedGameState {
                compressed_data: vec![i as u8; 200], // Simulated compressed data
                original_size: 1000,
                compressed_size: 200,
                game_id: [i as u8; 16],
                phase: 0,
                player_count: 2,
            },
            delta_chain: Vec::new(),
            final_summary: bitcraps::protocol::efficient_history::GameSummary {
                total_rolls: 25 + i as u32,
                final_balances: std::collections::HashMap::new(),
                duration_secs: 180 + i as u32 * 10,
                player_count: 2,
                total_wagered: 500 + i as u64 * 50,
                house_edge: 0.014,
            },
            timestamps: bitcraps::protocol::efficient_history::TimeRange {
                start_time: 1000 + i as u64 * 200,
                end_time: 1180 + i as u64 * 210,
                last_activity: 1180 + i as u64 * 210,
            },
            estimated_size: 400,
        };
        
        history.store_game(game_history)?;
    }
    
    let storage_time = start.elapsed();
    
    // Test retrieval performance
    let retrieval_start = Instant::now();
    let mut retrieved_count = 0;
    
    for i in 0..50 {
        let game_id = [i as u8; 16];
        if let Some(_game) = history.get_game(game_id)? {
            retrieved_count += 1;
        }
    }
    
    let retrieval_time = retrieval_start.elapsed();
    let metrics = history.get_metrics();
    
    println!("✓ Stored 50 games in {:?}", storage_time);
    println!("✓ Retrieved {} games in {:?}", retrieved_count, retrieval_time);
    println!("✓ Total memory usage: {} bytes", metrics.total_memory_bytes);
    println!("✓ Average compression ratio: {:.2}", metrics.average_compression_ratio);
    println!("✓ Recent access time: {:.2} μs", metrics.recent_access_time_us);
    
    // Test time range queries
    let range_start = Instant::now();
    let games_in_range = history.get_games_in_range(1000, 5000);
    let range_time = range_start.elapsed();
    
    println!("✓ Range query returned {} games in {:?}", 
             games_in_range.len(), range_time);
    
    Ok(())
}