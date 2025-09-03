#![cfg(feature = "mvp")]

use bitcraps::mesh::{MeshEvent, MeshService};
use bitcraps::{BitchatIdentity, BitchatKeypair};
use bitcraps::protocol::{create_game_packet};
use bitcraps::transport::{tcp_transport::TcpTransportConfig, TransportAddress, TransportCoordinator};
use std::net::SocketAddr;
use std::sync::Arc;

// Verifies that when A receives a PACKET_TYPE_GAME_DATA over TCP, Mesh surfaces a MessageReceived.
#[tokio::test]
#[ignore]
async fn game_discovery_surfaces_message() {
    // Identities
    let key_a = BitchatKeypair::generate();
    let key_b = BitchatKeypair::generate();
    let id_a = Arc::new(BitchatIdentity::from_keypair_with_pow(key_a, 0));
    let id_b = Arc::new(BitchatIdentity::from_keypair_with_pow(key_b, 0));

    // Transport A (listener)
    let mut t_a = TransportCoordinator::new();
    t_a.init_tcp_transport(TcpTransportConfig::default(), id_a.peer_id)
        .await
        .expect("init tcp A");
    let listen_addr: SocketAddr = "127.0.0.1:34568".parse().unwrap();
    t_a.listen_tcp(listen_addr).await.expect("A listen");

    // Transport B (dialer)
    let mut t_b = TransportCoordinator::new();
    t_b.init_tcp_transport(TcpTransportConfig::default(), id_b.peer_id())
        .await
        .expect("init tcp B");
    let observed = t_b.connect_tcp(listen_addr).await.expect("connect B->A");
    assert_eq!(observed, id_a.peer_id);

    // Mesh on A
    let mesh_a = Arc::new(MeshService::new(id_a.clone(), Arc::new(t_a)));
    // Start message processing (start() is fine; BLE listen is a no-op without init)
    mesh_a.start().await.expect("mesh start");
    let mut rx = mesh_a.subscribe();

    // Create a game creation packet (discovery)
    let game_id = [9u8; 16];
    let game_packet = create_game_packet(id_b.peer_id, game_id, 4, 1_000);
    let ser = bincode::serialize(&game_packet).expect("serialize");

    // Send from B to A over TCP
    t_b
        .send_to_peer(id_a.peer_id, ser)
        .await
        .expect("send game packet");

    // Expect MeshEvent::MessageReceived with PACKET_TYPE_GAME_DATA
    let mut attempts = 0u8;
    loop {
        attempts += 1;
        if attempts > 40 {
            panic!("mesh did not surface game discovery");
        }

        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(MeshEvent::MessageReceived { from, packet })) => {
                assert_eq!(from, id_b.peer_id);
                assert_eq!(packet.packet_type, bitcraps::protocol::PACKET_TYPE_GAME_DATA);
                break;
            }
            _ => {
                // Keep waiting
            }
        }
    }
}
