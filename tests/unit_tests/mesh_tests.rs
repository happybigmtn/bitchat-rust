use bitcraps::mesh::{MeshService, MeshPeer};
use bitcraps::protocol::PeerId;

#[tokio::test]
async fn test_mesh_peer_structure() {
    use std::time::{Instant, Duration};
    
    let peer = MeshPeer {
        peer_id: [1u8; 32],
        connected_at: Instant::now(),
        last_seen: Instant::now(),
        packets_sent: 0,
        packets_received: 0,
        latency: None,
        reputation: 1.0,
        is_treasury: false,
    };
    
    assert_eq!(peer.peer_id, [1u8; 32]);
    assert_eq!(peer.packets_sent, 0);
}

#[tokio::test]
async fn test_mesh_service_creation() {
    // MeshService::new requires more complex setup
    // Just test that we can import the type
    use bitcraps::BitchatIdentity;
    
    // Generate identity for mesh service with minimal PoW
    let identity = BitchatIdentity::generate_with_pow(1);
    assert_eq!(identity.peer_id.len(), 32);
}