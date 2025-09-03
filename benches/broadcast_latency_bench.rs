//! Performance benchmark for message broadcast latency with multiple participants

use bitcraps::crypto::{BitchatIdentity, BitchatKeypair};
use bitcraps::mesh::MeshService;
use bitcraps::protocol::runtime::game_lifecycle::GameLifecycleManager;
use bitcraps::protocol::{GameId, PeerId};
use bitcraps::transport::TransportCoordinator;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;

fn benchmark_broadcast_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("broadcast_latency");

    // Test with different participant counts
    for participant_count in [2, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(participant_count),
            participant_count,
            |b, &count| {
                b.to_async(&rt)
                    .iter(|| async { benchmark_broadcast_with_participants(count).await });
            },
        );
    }

    group.finish();
}

async fn benchmark_broadcast_with_participants(participant_count: usize) {
    // Setup test environment
    let keypair = BitchatKeypair::generate();
    let identity = Arc::new(BitchatIdentity::new(keypair));
    let transport = Arc::new(TransportCoordinator::new());
    let mesh_service = Arc::new(MeshService::new(identity.clone(), transport.clone()));

    // Start mesh service
    let _ = mesh_service.start().await;

    // Create lifecycle manager
    let lifecycle = GameLifecycleManager::new(mesh_service.clone());

    // Generate participant list
    let mut participants = Vec::new();
    for i in 0..participant_count {
        let mut peer_id = [0u8; 32];
        peer_id[0] = i as u8;
        participants.push(peer_id);
    }

    let game_id: GameId = [1u8; 32];
    let joining_player: PeerId = [99u8; 32];

    // Measure broadcast latency
    let start = std::time::Instant::now();

    let _ = lifecycle
        .broadcast_player_join_request(game_id, joining_player, &participants)
        .await;

    let _duration = start.elapsed();

    // Return value for black_box
    black_box(());
}

fn benchmark_message_fragmentation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("message_fragmentation_10kb", |b| {
        b.to_async(&rt).iter(|| async {
            let large_message = vec![0u8; 10_240]; // 10KB message
            benchmark_fragment_and_reassemble(large_message).await
        });
    });
}

async fn benchmark_fragment_and_reassemble(message: Vec<u8>) {
    use bitcraps::protocol::ble_dispatch::BleMessageDispatcher;
    use bitcraps::protocol::consensus::ConsensusMessage;

    // Create dispatcher
    let transport = Arc::new(TransportCoordinator::new());
    let dispatcher = BleMessageDispatcher::new(transport);

    // Fragment the message
    let fragments = dispatcher.fragment_message(&message, 244); // BLE MTU

    // Simulate reassembly
    for fragment in fragments {
        black_box(fragment);
    }
}

criterion_group!(
    benches,
    benchmark_broadcast_latency,
    benchmark_message_fragmentation
);
criterion_main!(benches);
