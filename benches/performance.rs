use bitcraps::{
    crypto::{BitchatIdentity, BitchatKeypair},
    protocol::consensus::{ConsensusConfig, ConsensusEngine},
    protocol::{DiceRoll, GameId, PeerId},
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark core consensus operations
fn benchmark_consensus_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("consensus_operations");
    group.sample_size(50);

    // Setup test data
    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let participants: Vec<PeerId> = (0..5)
        .map(|i| {
            let mut peer = [0u8; 32];
            peer[0] = i;
            peer
        })
        .collect();

    group.bench_function("consensus_engine_creation", |b| {
        b.iter(|| {
            let engine = ConsensusEngine::new(
                black_box(game_id),
                black_box(participants.clone()),
                black_box(participants[0]),
                black_box(config.clone()),
            );
            black_box(engine)
        })
    });

    group.bench_function("proposal_submission", |b| {
        let mut engine = ConsensusEngine::new(
            game_id,
            participants.clone(),
            participants[0],
            config.clone(),
        )
        .unwrap();

        b.iter(|| {
            let operation = bitcraps::protocol::consensus::GameOperation::UpdateBalances {
                changes: [(participants[0], bitcraps::protocol::CrapTokens::new(100))]
                    .into_iter()
                    .collect(),
                reason: "test".to_string(),
            };

            let result = engine.submit_proposal(black_box(operation));
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark cryptographic operations
fn benchmark_crypto_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");
    group.sample_size(100);

    group.bench_function("keypair_generation", |b| {
        b.iter(|| {
            let keypair = BitchatKeypair::generate();
            black_box(keypair)
        })
    });

    group.bench_function("identity_generation", |b| {
        b.iter(|| {
            let identity = BitchatIdentity::generate_with_pow(0);
            black_box(identity)
        })
    });

    group.bench_function("message_signing", |b| {
        let identity = BitchatIdentity::generate_with_pow(0);
        let message = b"benchmark message for signing performance test";

        b.iter(|| {
            let signature = identity.keypair.sign(black_box(message));
            black_box(signature)
        })
    });

    group.finish();
}

/// Benchmark game logic operations  
fn benchmark_game_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("game_operations");
    group.sample_size(100);

    group.bench_function("dice_roll_creation", |b| {
        b.iter(|| {
            let dice_roll = DiceRoll::new(black_box(3), black_box(4));
            black_box(dice_roll)
        })
    });

    group.bench_function("dice_roll_validation", |b| {
        let dice_roll = DiceRoll::new(3, 4).unwrap();

        b.iter(|| {
            let total = dice_roll.total();
            let is_natural = dice_roll.is_natural();
            let is_craps = dice_roll.is_craps();
            let is_hard = dice_roll.is_hard_way();
            black_box((total, is_natural, is_craps, is_hard))
        })
    });

    group.finish();
}

/// Benchmark memory and serialization operations
fn benchmark_serialization_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_operations");
    group.sample_size(100);

    let config = ConsensusConfig::default();
    let game_id: GameId = [1u8; 16];
    let participants: Vec<PeerId> = (0..10)
        .map(|i| {
            let mut peer = [0u8; 32];
            peer[0] = i;
            peer
        })
        .collect();

    group.bench_function("consensus_state_serialization", |b| {
        let engine = ConsensusEngine::new(
            game_id,
            participants.clone(),
            participants[0],
            config.clone(),
        )
        .unwrap();

        b.iter(|| {
            let state = engine.get_consensus_state();
            black_box(state)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_consensus_operations,
    benchmark_crypto_operations,
    benchmark_game_operations,
    benchmark_serialization_operations
);
criterion_main!(benches);
