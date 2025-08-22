use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use bitchat::gaming::{GameSessionManager, AntiCheatDetector, GamingSecurityManager};
use std::time::Duration;

fn benchmark_game_session_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("game_session_creation");
    
    for player_count in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("create_session", player_count),
            player_count,
            |b, &player_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let session_manager = GameSessionManager::new(Default::default());
                        let participants: Vec<PeerId> = (0..player_count)
                            .map(|_| PeerId::random())
                            .collect();
                        
                        let session_id = session_manager.create_session(participants[0]).await.unwrap();
                        
                        for participant in &participants[1..] {
                            session_manager.join_session(&session_id, *participant).await.unwrap();
                        }
                        
                        black_box(session_id);
                    })
                })
            },
        );
    }
    group.finish();
}

fn benchmark_anti_cheat_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("anti_cheat_validation");
    
    for bet_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("validate_bets", bet_count),
            bet_count,
            |b, &bet_count| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.iter(|| {
                    rt.block_on(async {
                        let anti_cheat = AntiCheatDetector::new();
                        let player = PeerId::random();
                        
                        for i in 0..*bet_count {
                            let bet = create_benchmark_bet(player, i);
                            let result = anti_cheat.validate_bet(&bet, &player).await;
                            black_box(result);
                        }
                    })
                })
            },
        );
    }
    group.finish();
}

fn benchmark_bet_escrow_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("bet_escrow");
    
    let rt = tokio::runtime::Runtime::new().unwrap();
    let gaming_security = rt.block_on(async {
        let gs = GamingSecurityManager::new(Default::default());
        let participants = vec![PeerId::random(), PeerId::random(), PeerId::random()];
        gs.create_gaming_session("benchmark_game".to_string(), participants.clone()).await.unwrap();
        gs
    });
    
    group.bench_function("escrow_bet", |b| {
        b.iter(|| {
            rt.block_on(async {
                let bet = create_escrow_benchmark_bet();
                let result = gaming_security.validate_and_escrow_bet("benchmark_game", &bet).await;
                black_box(result);
            })
        })
    });
    
    group.finish();
}

fn benchmark_dice_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dice_simulation");
    
    group.bench_function("single_roll", |b| {
        b.iter(|| {
            let roll = simulate_dice_roll_fast();
            black_box(roll);
        })
    });
    
    group.bench_function("10k_rolls", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(10000);
            for _ in 0..10000 {
                results.push(simulate_dice_roll_fast());
            }
            black_box(results);
        })
    });
    
    group.finish();
}

fn create_benchmark_bet(player: PeerId, sequence: u32) -> CrapsBet {
    CrapsBet {
        player,
        bet_type: BetType::Pass,
        amount: 100,
        timestamp: sequence as u64, // Use sequence as timestamp for benchmarking
    }
}

fn create_escrow_benchmark_bet() -> PendingBet {
    PendingBet {
        bet_id: "benchmark_bet".to_string(),
        player: PeerId::random(),
        amount: 1000,
        bet_hash: [0u8; 32],
        timestamp: 0,
        confirmations: vec![PeerId::random(), PeerId::random()],
        escrow_signature: None,
    }
}

fn simulate_dice_roll_fast() -> (u8, u8) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (rng.gen_range(1..=6), rng.gen_range(1..=6))
}

criterion_group!(
    benches, 
    benchmark_game_session_creation,
    benchmark_anti_cheat_validation,
    benchmark_bet_escrow_operations,
    benchmark_dice_simulation
);
criterion_main!(benches);