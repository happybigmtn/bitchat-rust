#![cfg(feature = "mvp")]

use bitcraps::protocol::{create_ping_packet, BitchatPacket, PeerId};
use bitcraps::transport::{tcp_transport::TcpTransportConfig, TransportAddress, TransportCoordinator};
use std::net::SocketAddr;
use std::sync::Arc;

// Verifies two coordinators connect over TCP and exchange a ping packet end-to-end.
#[tokio::test]
#[ignore]
async fn tcp_e2e_ping() {
    // Identities
    let peer_a: PeerId = [0xAA; 32];
    let peer_b: PeerId = [0xBB; 32];

    // Server coordinator (A)
    let mut server = TransportCoordinator::new();
    server
        .init_tcp_transport(TcpTransportConfig::default(), peer_a)
        .await
        .expect("init tcp server");

    // Use a fixed localhost port to avoid needing to query ephemeral port
    let listen_addr: SocketAddr = "127.0.0.1:34567".parse().unwrap();
    server
        .listen_tcp(listen_addr)
        .await
        .expect("server listen");

    // Client coordinator (B)
    let mut client = TransportCoordinator::new();
    client
        .init_tcp_transport(TcpTransportConfig::default(), peer_b)
        .await
        .expect("init tcp client");

    // Connect client to server and confirm handshake peer id
    let observed_server_id = client
        .connect_tcp(listen_addr)
        .await
        .expect("client connect");
    assert_eq!(observed_server_id, peer_a);

    // Compose ping packet from B to A and send
    let mut ping = create_ping_packet(peer_b);
    ping.add_receiver(peer_a);
    let payload = bincode::serialize(&ping).expect("serialize packet");
    server
        .send_to_peer(peer_a, payload.clone())
        .await
        .err()
        .expect("send_to_peer on server should not be used for loopback");
    // Correctly send from client to server
    client
        .send_to_peer(peer_a, payload)
        .await
        .expect("client send to server");

    // Server should receive DataReceived event with the ping
    // Poll a few events to tolerate Connected/health notifications
    let server = Arc::new(server);
    let mut tries = 0u8;
    loop {
        if tries > 20 {
            panic!("did not receive ping within timeout");
        }
        tries += 1;

        if let Some(event) = server.next_event().await {
            use bitcraps::transport::TransportEvent;
            match event {
                TransportEvent::DataReceived { peer_id, data } => {
                    assert_eq!(peer_id, peer_b);
                    let pkt: BitchatPacket = bincode::deserialize(&data).expect("decode packet");
                    assert_eq!(pkt.packet_type, bitcraps::protocol::PACKET_TYPE_PING);
                    break;
                }
                _ => {
                    // Ignore other events
                }
            }
        } else {
            // Small delay between polls
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
}
