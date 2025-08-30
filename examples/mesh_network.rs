//! Mesh networking example demonstrating peer discovery and routing
//!
//! Run with: cargo run --example mesh_network

use bitcraps::error::Result;
use bitcraps::mesh::{MeshConfig, MeshPeer, MeshService};
use bitcraps::protocol::{BitchatPacket, PeerId};
use bitcraps::transport::{MockTransport, TransportCoordinator};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("BitCraps Mesh Network Example");
    println!("==============================\n");

    // Create mesh configuration
    let config = MeshConfig {
        max_peers: 50,
        ttl: 5,
        cache_size: 10_000,
        heartbeat_interval: std::time::Duration::from_secs(30),
        enable_reputation: true,
    };

    // Create multiple mesh nodes
    let num_nodes = 5;
    let mut nodes = Vec::new();

    println!("Creating {} mesh nodes...", num_nodes);
    for i in 0..num_nodes {
        let peer_id = PeerId::random();
        let transport = Arc::new(MockTransport::new());
        let coordinator = TransportCoordinator::new(transport.clone());

        let mesh = MeshService::new(peer_id, Arc::new(coordinator), config.clone()).await?;

        nodes.push((peer_id, mesh));
        println!("  Node {}: {:?}", i + 1, peer_id);
    }
    println!();

    // Connect nodes in a ring topology
    println!("Connecting nodes in ring topology...");
    for i in 0..num_nodes {
        let next = (i + 1) % num_nodes;
        let (peer_id, mesh) = &nodes[i];
        let (next_id, _) = &nodes[next];

        // Add peer connection
        mesh.add_peer(*next_id).await?;
        println!("  {:?} -> {:?}", peer_id, next_id);
    }
    println!();

    // Send a message from first to last node
    println!("Sending message through mesh...");
    let (sender_id, sender_mesh) = &nodes[0];
    let (target_id, _) = &nodes[num_nodes - 1];

    let message =
        BitchatPacket::create_message(*sender_id, *target_id, b"Hello from the mesh!".to_vec());

    sender_mesh.send_packet(message).await?;
    println!("  Message sent from {:?} to {:?}", sender_id, target_id);

    // Wait for message propagation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check routing table
    println!("\nRouting Tables:");
    println!("---------------");
    for (peer_id, mesh) in &nodes {
        let routing_table = mesh.get_routing_table().await;
        println!("Node {:?}:", peer_id);
        for (dest, route) in routing_table.iter().take(3) {
            println!(
                "  {:?} via {:?} (hop count: {})",
                dest, route.next_hop, route.hop_count
            );
        }
    }

    // Display mesh statistics
    println!("\nMesh Statistics:");
    println!("----------------");
    for (peer_id, mesh) in &nodes {
        let stats = mesh.get_statistics().await;
        println!("Node {:?}:", peer_id);
        println!("  Messages sent: {}", stats.messages_sent);
        println!("  Messages received: {}", stats.messages_received);
        println!("  Messages forwarded: {}", stats.messages_forwarded);
        println!("  Cache hits: {}", stats.cache_hits);
    }

    Ok(())
}

/// Exercise 1: Implement Broadcast Storm Prevention
///
/// Modify the mesh to prevent broadcast storms by implementing
/// intelligent flooding with duplicate detection.
#[allow(dead_code)]
async fn exercise_broadcast_storm() {
    // TODO: Implement broadcast storm prevention
    // Hints:
    // 1. Create a dense mesh topology
    // 2. Send broadcast message
    // 3. Verify each node receives exactly once
    // 4. Measure and minimize redundant transmissions
}

/// Exercise 2: Dynamic Topology Changes
///
/// Simulate nodes joining and leaving the mesh dynamically.
/// Verify that routing tables update correctly and messages
/// still reach their destinations.
#[allow(dead_code)]
async fn exercise_dynamic_topology() {
    // TODO: Implement dynamic topology handling
    // Hints:
    // 1. Start with stable mesh
    // 2. Add new nodes dynamically
    // 3. Remove nodes (simulate failures)
    // 4. Verify mesh self-heals
    // 5. Test message delivery during changes
}

/// Exercise 3: Reputation System
///
/// Implement and test the reputation system by simulating
/// both good and bad behavior, then verify that malicious
/// nodes are gradually isolated.
#[allow(dead_code)]
async fn exercise_reputation_system() {
    // TODO: Implement reputation testing
    // Hints:
    // 1. Create mix of honest and malicious nodes
    // 2. Have malicious nodes drop messages
    // 3. Track reputation scores over time
    // 4. Verify malicious nodes get isolated
}
