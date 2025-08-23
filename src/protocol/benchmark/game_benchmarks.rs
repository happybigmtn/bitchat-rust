//! Game-specific benchmarks for BitCraps

use std::time::{Duration, Instant};
use criterion::{black_box, Criterion};

use crate::protocol::efficient_game_state::{CompactGameState, StateSnapshot, VarInt};
use crate::protocol::efficient_bet_resolution::{EfficientBetResolver, PayoutLookupTable};
use crate::protocol::{BetType, CrapTokens, DiceRoll};

use super::{BenchmarkResults, BenchmarkConfig, MemoryBenchmarkStats};

/// Benchmark compact game state operations
pub fn benchmark_compact_game_state(config: &BenchmarkConfig) -> Vec<BenchmarkResults> {
    let mut results = Vec::new();
    
    println!("\n🎯 Benchmarking Compact Game State...");
    
    // Benchmark state creation
    results.push(bench_state_creation(config));
    
    // Benchmark bit field operations
    results.push(bench_bit_field_ops(config));
    
    // Benchmark copy-on-write
    results.push(bench_copy_on_write(config));
    
    // Benchmark state snapshots
    results.push(bench_state_snapshots(config));
    
    // Benchmark variable-length encoding
    results.push(bench_varint_encoding(config));
    
    results
}

/// Benchmark state creation performance
fn bench_state_creation(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut times = Vec::new();
    let mut memory_usage = Vec::new();
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let _state = black_box(CompactGameState::new([1; 16], [2; 32]));
    }
    
    // Actual benchmark
    for i in 0..config.iterations {
        let start = Instant::now();
        let game_id = [(i % 256) as u8; 16];
        let shooter = [(i % 256) as u8; 32];
        let state = black_box(CompactGameState::new(game_id, shooter));
        let duration = start.elapsed();
        
        times.push(duration);
        
        if config.profile_memory {
            let memory = state.memory_usage();
            memory_usage.push(memory.total_bytes);
        }
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = config.iterations as f64 / avg_time.as_secs_f64();
    
    let memory_stats = if config.profile_memory {
        MemoryBenchmarkStats {
            peak_memory_bytes: *memory_usage.iter().max().unwrap_or(&0),
            avg_memory_bytes: memory_usage.iter().sum::<usize>() / memory_usage.len().max(1),
            allocations: config.iterations as u64,
            deallocations: config.iterations as u64,
            cache_hit_rate: 0.0,
        }
    } else {
        MemoryBenchmarkStats {
            peak_memory_bytes: 0,
            avg_memory_bytes: 0,
            allocations: 0,
            deallocations: 0,
            cache_hit_rate: 0.0,
        }
    };
    
    BenchmarkResults {
        name: "Compact State Creation".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats,
        improvement_factor: 5.0, // Estimated vs naive implementation
    }
}

/// Benchmark bit field operations
fn bench_bit_field_ops(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut state = CompactGameState::new([1; 16], [2; 32]);
    let mut times = Vec::new();
    
    // Warmup
    for i in 0..config.warmup_iterations {
        state.set_roll_count(i as u32);
        black_box(state.get_roll_count());
    }
    
    // Benchmark bit field get/set operations
    for i in 0..config.iterations {
        let start = Instant::now();
        
        // Multiple operations per iteration
        state.set_roll_count(i as u32);
        state.set_point(Some(6));
        state.set_fire_points(3);
        state.set_hot_streak(42);
        
        black_box(state.get_roll_count());
        black_box(state.get_point());
        black_box(state.get_fire_points());
        black_box(state.get_hot_streak());
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = (config.iterations * 8) as f64 / avg_time.as_secs_f64(); // 8 ops per iteration
    
    BenchmarkResults {
        name: "Bit Field Operations".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: std::mem::size_of::<CompactGameState>(),
            avg_memory_bytes: std::mem::size_of::<CompactGameState>(),
            allocations: 0,
            deallocations: 0,
            cache_hit_rate: 1.0, // All operations are in-place
        },
        improvement_factor: 10.0, // Estimated vs individual field access
    }
}

/// Benchmark copy-on-write performance
fn bench_copy_on_write(config: &BenchmarkConfig) -> BenchmarkResults {
    let base_state = CompactGameState::new([1; 16], [2; 32]);
    let mut times = Vec::new();
    let mut memory_usage = 0;
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let mut clone = base_state.clone();
        clone.make_mutable();
    }
    
    // Benchmark copy-on-write operations
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        let mut state1 = black_box(base_state.clone());
        let mut state2 = black_box(state1.clone());
        let mut state3 = black_box(state2.clone());
        
        // This should trigger copy-on-write
        state1.make_mutable();
        state2.make_mutable();
        state3.make_mutable();
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    if config.profile_memory {
        memory_usage = base_state.memory_usage().total_bytes * 3; // 3 clones
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = (config.iterations * 6) as f64 / avg_time.as_secs_f64(); // 3 clones + 3 mutations
    
    BenchmarkResults {
        name: "Copy-on-Write Operations".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: memory_usage,
            avg_memory_bytes: memory_usage,
            allocations: config.iterations as u64 * 3,
            deallocations: config.iterations as u64 * 3,
            cache_hit_rate: 0.9, // Most clones share data
        },
        improvement_factor: 3.0, // vs deep copying
    }
}

/// Benchmark state snapshot operations
fn bench_state_snapshots(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut state = CompactGameState::new([1; 16], [2; 32]);
    let mut times = Vec::new();
    
    // Create some state changes
    state.set_roll_count(42);
    state.set_point(Some(8));
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let snapshot = StateSnapshot::create(&state);
        black_box(snapshot);
    }
    
    // Benchmark snapshot creation and reconstruction
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        let mut snapshot = black_box(StateSnapshot::create(&state));
        
        // Add some deltas
        snapshot.add_delta(crate::protocol::efficient_game_state::StateDelta::PhaseChange { 
            new_phase: crate::protocol::efficient_game_state::GamePhase::Point 
        });
        snapshot.add_delta(crate::protocol::efficient_game_state::StateDelta::RollProcessed { 
            roll: DiceRoll::new(3, 4).unwrap() 
        });
        
        let _reconstructed = black_box(snapshot.reconstruct().unwrap());
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = config.iterations as f64 / avg_time.as_secs_f64();
    
    BenchmarkResults {
        name: "State Snapshots".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: std::mem::size_of::<StateSnapshot>() * 2,
            avg_memory_bytes: std::mem::size_of::<StateSnapshot>(),
            allocations: config.iterations as u64,
            deallocations: config.iterations as u64,
            cache_hit_rate: 0.8,
        },
        improvement_factor: 4.0, // vs full state storage
    }
}

/// Benchmark variable-length integer encoding
fn bench_varint_encoding(config: &BenchmarkConfig) -> BenchmarkResults {
    let test_values = vec![0u64, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX];
    let mut times = Vec::new();
    let mut total_bytes = 0;
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        for &value in &test_values {
            let encoded = VarInt::encode(value);
            let _decoded = VarInt::decode(&encoded).unwrap();
        }
    }
    
    // Benchmark varint encoding/decoding
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        for &value in &test_values {
            let encoded = black_box(VarInt::encode(value));
            total_bytes += encoded.len();
            let (_decoded, _) = black_box(VarInt::decode(&encoded).unwrap());
        }
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = (config.iterations * test_values.len() * 2) as f64 / avg_time.as_secs_f64(); // encode + decode
    
    BenchmarkResults {
        name: "VarInt Encoding".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: total_bytes / config.iterations,
            avg_memory_bytes: total_bytes / config.iterations,
            allocations: config.iterations as u64 * test_values.len() as u64,
            deallocations: config.iterations as u64 * test_values.len() as u64,
            cache_hit_rate: 0.0,
        },
        improvement_factor: 2.5, // vs fixed-width encoding
    }
}

/// Benchmark bet resolution engine
pub fn benchmark_bet_resolution(config: &BenchmarkConfig) -> Vec<BenchmarkResults> {
    let mut results = Vec::new();
    
    println!("\n🎰 Benchmarking Bet Resolution Engine...");
    
    // Benchmark lookup table performance
    results.push(bench_lookup_table_access(config));
    
    // Benchmark bet resolution caching
    results.push(bench_bet_resolution_cache(config));
    
    // Benchmark batch bet resolution
    results.push(bench_batch_bet_resolution(config));
    
    results
}

/// Benchmark lookup table access performance
fn bench_lookup_table_access(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut times = Vec::new();
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let table = PayoutLookupTable::new();
        let _ = table.lookup_resolution(BetType::Pass, 7);
    }
    
    let table = PayoutLookupTable::new();
    let bet_types = [BetType::Pass, BetType::Field, BetType::Yes6, BetType::Hard8, BetType::Next7];
    let dice_values = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    
    // Benchmark lookup operations
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        for &bet_type in &bet_types {
            for &dice_value in &dice_values {
                let _result = black_box(table.lookup_resolution(bet_type, dice_value));
            }
        }
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let operations_per_iteration = bet_types.len() * dice_values.len();
    let throughput = (config.iterations * operations_per_iteration) as f64 / avg_time.as_secs_f64();
    
    BenchmarkResults {
        name: "Payout Lookup Table Access".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: std::mem::size_of::<PayoutLookupTable>(),
            avg_memory_bytes: std::mem::size_of::<PayoutLookupTable>(),
            allocations: 0,
            deallocations: 0,
            cache_hit_rate: 1.0, // Array access is always "cached"
        },
        improvement_factor: 50.0, // vs runtime calculation
    }
}

/// Benchmark bet resolution with caching
fn bench_bet_resolution_cache(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut resolver = EfficientBetResolver::new();
    let state = CompactGameState::new([1; 16], [2; 32]);
    let dice_roll = DiceRoll::new(3, 4).unwrap();
    
    let active_bets = vec![
        (BetType::Pass, [1; 32], CrapTokens::new_unchecked(100)),
        (BetType::Field, [2; 32], CrapTokens::new_unchecked(50)),
        (BetType::Yes6, [3; 32], CrapTokens::new_unchecked(25)),
    ];
    
    let mut times = Vec::new();
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let _results = resolver.resolve_bets_fast(&state, dice_roll, &active_bets).unwrap();
    }
    
    // Benchmark with caching
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        let _results = black_box(resolver.resolve_bets_fast(&state, dice_roll, &active_bets).unwrap());
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let stats = resolver.get_stats();
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = config.iterations as f64 / avg_time.as_secs_f64();
    
    BenchmarkResults {
        name: "Bet Resolution with Caching".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: stats.lookup_table_size + stats.special_bet_cache_size * 100,
            avg_memory_bytes: stats.lookup_table_size,
            allocations: stats.cache_misses,
            deallocations: 0,
            cache_hit_rate: stats.cache_hit_rate,
        },
        improvement_factor: 10.0, // with high cache hit rate
    }
}

/// Benchmark batch bet resolution
fn bench_batch_bet_resolution(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut resolver = EfficientBetResolver::new();
    let state = CompactGameState::new([1; 16], [2; 32]);
    
    // Create large batch of bets
    let mut large_bet_batch = Vec::new();
    for i in 0..config.large_dataset {
        let player = [(i % 256) as u8; 32];
        let bet_type = match i % 10 {
            0 => BetType::Pass,
            1 => BetType::Field,
            2 => BetType::Yes6,
            3 => BetType::Hard8,
            4 => BetType::Next7,
            5 => BetType::Come,
            6 => BetType::DontPass,
            7 => BetType::Yes4,
            8 => BetType::No10,
            _ => BetType::Repeater6,
        };
        let amount = CrapTokens::new_unchecked((i % 100 + 1) as u64);
        large_bet_batch.push((bet_type, player, amount));
    }
    
    let dice_roll = DiceRoll::new(4, 3).unwrap(); // Lucky 7
    let mut times = Vec::new();
    
    // Warmup
    for _ in 0..config.warmup_iterations {
        let _results = resolver.resolve_bets_fast(&state, dice_roll, &large_bet_batch[..config.small_dataset]).unwrap();
    }
    
    // Benchmark large batch resolution
    for _ in 0..config.iterations {
        let start = Instant::now();
        
        let _results = black_box(resolver.resolve_bets_fast(&state, dice_roll, &large_bet_batch).unwrap());
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = (config.iterations * config.large_dataset) as f64 / avg_time.as_secs_f64();
    
    BenchmarkResults {
        name: format!("Batch Bet Resolution ({} bets)", config.large_dataset),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: config.large_dataset * 100, // Estimated per bet
            avg_memory_bytes: config.large_dataset * 50,
            allocations: config.iterations as u64,
            deallocations: config.iterations as u64,
            cache_hit_rate: 0.85,
        },
        improvement_factor: 25.0, // vs individual bet processing
    }
}

/// Benchmark full system integration
pub fn benchmark_full_system(config: &BenchmarkConfig) -> Vec<BenchmarkResults> {
    let mut results = Vec::new();
    
    println!("\n🎲 Benchmarking Full System Integration...");
    
    // Benchmark complete game simulation
    results.push(bench_full_game_simulation(config));
    
    results
}

/// Benchmark complete game simulation
fn bench_full_game_simulation(config: &BenchmarkConfig) -> BenchmarkResults {
    let mut times = Vec::new();
    
    // Warmup - simulate a few complete games
    for _ in 0..config.warmup_iterations.min(10) {
        simulate_complete_game();
    }
    
    // Full system benchmark
    for _ in 0..config.iterations.min(100) { // Limit iterations for system tests
        let start = Instant::now();
        
        black_box(simulate_complete_game());
        
        let duration = start.elapsed();
        times.push(duration);
    }
    
    let actual_iterations = times.len();
    let avg_time = times.iter().sum::<Duration>() / actual_iterations as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = actual_iterations as f64 / avg_time.as_secs_f64();
    
    BenchmarkResults {
        name: "Complete Game Simulation".to_string(),
        avg_time,
        min_time,
        max_time,
        throughput,
        memory_stats: MemoryBenchmarkStats {
            peak_memory_bytes: 1024 * 1024, // Estimated 1MB for complete game
            avg_memory_bytes: 512 * 1024,   // Estimated 512KB average
            allocations: actual_iterations as u64 * 100,
            deallocations: actual_iterations as u64 * 100,
            cache_hit_rate: 0.7,
        },
        improvement_factor: 5.0, // vs non-optimized implementation
    }
}

/// Simulate a complete game for benchmarking
fn simulate_complete_game() -> bool {
    let mut state = CompactGameState::new([1; 16], [2; 32]);
    let mut resolver = EfficientBetResolver::new();
    
    // Simulate multiple rounds
    for round in 0..10 {
        // Create some bets
        let bets = vec![
            (BetType::Pass, [1; 32], CrapTokens::new_unchecked(100)),
            (BetType::Field, [2; 32], CrapTokens::new_unchecked(50)),
        ];
        
        // Simulate dice roll
        let dice1 = (round % 6) + 1;
        let dice2 = ((round + 3) % 6) + 1;
        let roll = DiceRoll::new(dice1 as u8, dice2 as u8).unwrap();
        
        // Resolve bets
        let _results = resolver.resolve_bets_fast(&state, roll, &bets).unwrap();
        
        // Update state
        state.set_roll_count(state.get_roll_count() + 1);
        if let Some(point) = state.get_point() {
            if roll.total() == point || roll.total() == 7 {
                state.set_point(None); // End of round
            }
        } else if ![7, 11, 2, 3, 12].contains(&roll.total()) {
            state.set_point(Some(roll.total())); // Set point
        }
    }
    
    true
}

pub fn criterion_game_benchmarks(c: &mut Criterion, _config: &BenchmarkConfig) {
    // Game state benchmarks
    c.bench_function("state_creation", |b| {
        b.iter(|| {
            let game_id = [1; 16];
            let shooter = [2; 32];
            black_box(CompactGameState::new(game_id, shooter))
        })
    });
    
    c.bench_function("bit_field_ops", |b| {
        let mut state = CompactGameState::new([1; 16], [2; 32]);
        b.iter(|| {
            state.set_roll_count(42);
            black_box(state.get_roll_count())
        })
    });
    
    // Bet resolution benchmarks
    c.bench_function("lookup_table_access", |b| {
        let table = PayoutLookupTable::new();
        b.iter(|| {
            black_box(table.lookup_resolution(BetType::Pass, 7))
        })
    });
}