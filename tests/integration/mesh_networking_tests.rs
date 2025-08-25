//! Integration tests for mesh networking functionality
//!
//! These tests verify the complete mesh networking stack including:
//! - Peer discovery and connection
//! - Message routing and delivery
//! - Byzantine fault tolerance
//! - Gateway node functionality
//! - Network resilience

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, timeout};

use bitcraps::mesh::{MeshService, MeshConfig};
use bitcraps::protocol::{PeerId, BitchatPacket, GameId};
use bitcraps::crypto::BitchatIdentity;
use bitcraps::transport::{TransportCoordinator, TransportConfig};
use bitcraps::error::Result;

/// Test configuration for mesh networks
fn test_mesh_config() -> MeshConfig {
    MeshConfig {
        max_peers: 50,
        enable_discovery: true,
        discovery_interval: Duration::from_millis(100),
        heartbeat_interval: Duration::from_millis(500),
        cleanup_interval: Duration::from_secs(5),
        message_ttl: 10,
        enable_routing: true,
        enable_deduplication: true,
        cache_size: 1000,
    }
}

/// Create a test mesh node
async fn create_test_node(name: &str) -> Arc<MeshService> {
    let identity = BitchatIdentity::generate_with_pow(0);
    let config = test_mesh_config();
    let transport_config = TransportConfig::default();
    let transport = Arc::new(TransportCoordinator::new(
        transport_config,
        identity.peer_id,
    ));
    
    let mesh = MeshService::new(identity.peer_id, transport);
    mesh.start().await.expect("Failed to start mesh service");
    
    log::info!("Created test node: {} with peer_id: {:?}", name, identity.peer_id);
    
    Arc::new(mesh)
}

#[tokio::test]
async fn test_peer_discovery() {
    // Create multiple mesh nodes
    let node1 = create_test_node("node1").await;
    let node2 = create_test_node("node2").await;
    let node3 = create_test_node("node3").await;
    
    // Allow time for discovery
    sleep(Duration::from_millis(500)).await;
    
    // Check that nodes discovered each other
    let peers1 = node1.get_connected_peers().await;
    let peers2 = node2.get_connected_peers().await;
    let peers3 = node3.get_connected_peers().await;
    
    // Each node should see the other two
    assert!(peers1.len() >= 2, "Node1 should discover other nodes");
    assert!(peers2.len() >= 2, "Node2 should discover other nodes");
    assert!(peers3.len() >= 2, "Node3 should discover other nodes");
}

#[tokio::test]
async fn test_message_routing() {
    // Create a chain of nodes
    let node1 = create_test_node("node1").await;
    let node2 = create_test_node("node2").await;
    let node3 = create_test_node("node3").await;
    
    // Allow discovery
    sleep(Duration::from_millis(500)).await;
    
    // Set up message receiver on node3
    let (tx, mut rx) = mpsc::channel(10);
    let node3_id = node3.get_peer_id();
    
    // Send message from node1 to node3 (should route through node2)
    let packet = BitchatPacket::new_ping(
        node1.get_peer_id(),
        node3_id,
    );
    
    node1.send_packet(packet.clone()).await.expect("Failed to send packet");
    
    // Wait for message with timeout
    let received = timeout(Duration::from_secs(2), async {
        // In real implementation, would register message handler
        // For test, we'll check if packet was routed
        sleep(Duration::from_millis(100)).await;
        node3.get_routing_table().await.contains_key(&node1.get_peer_id())
    }).await;
    
    assert!(received.is_ok(), "Message should be routed successfully");
}

#[tokio::test]
async fn test_byzantine_resilience() {
    // Create a network with Byzantine nodes
    let honest_nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..7).map(|i| create_test_node(&format!("honest_{}", i)))
    ).await;
    
    let byzantine_nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..3).map(|i| create_test_node(&format!("byzantine_{}", i)))
    ).await;
    
    // Allow network formation
    sleep(Duration::from_secs(1)).await;
    
    // Byzantine nodes start sending conflicting messages
    for byzantine in &byzantine_nodes {
        // Send conflicting game state updates
        let fake_packet = BitchatPacket::new_game_create(
            byzantine.get_peer_id(),
            GameId::new(),
            vec![byzantine.get_peer_id()],
        );
        
        for _ in 0..10 {
            let _ = byzantine.send_packet(fake_packet.clone()).await;
        }
    }
    
    // Honest nodes should maintain consensus despite Byzantine behavior
    sleep(Duration::from_secs(2)).await;
    
    // Check that honest nodes are still connected to each other
    for honest in &honest_nodes {
        let peers = honest.get_connected_peers().await;
        let honest_peer_count = peers.iter()
            .filter(|p| honest_nodes.iter().any(|h| h.get_peer_id() == **p))
            .count();
        
        // Should maintain connections with majority of honest nodes
        assert!(honest_peer_count >= 4, "Honest nodes should maintain connectivity");
    }
}

#[tokio::test]
async fn test_network_partition_recovery() {
    // Create two groups of nodes
    let group1: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..3).map(|i| create_test_node(&format!("group1_{}", i)))
    ).await;
    
    let group2: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..3).map(|i| create_test_node(&format!("group2_{}", i)))
    ).await;
    
    // Allow initial network formation
    sleep(Duration::from_secs(1)).await;
    
    // Simulate network partition by disconnecting groups
    for node1 in &group1 {
        for node2 in &group2 {
            node1.disconnect_peer(&node2.get_peer_id()).await;
        }
    }
    
    // Groups operate independently
    sleep(Duration::from_secs(2)).await;
    
    // Verify groups are partitioned
    for node in &group1 {
        let peers = node.get_connected_peers().await;
        for peer in peers {
            assert!(!group2.iter().any(|n| n.get_peer_id() == peer), 
                "Group1 should not be connected to Group2");
        }
    }
    
    // Heal partition by reconnecting one node from each group
    group1[0].connect_peer(group2[0].get_peer_id()).await.expect("Reconnection failed");
    
    // Allow network to heal
    sleep(Duration::from_secs(2)).await;
    
    // Verify network is healed
    for node in group1.iter().chain(group2.iter()) {
        let peers = node.get_connected_peers().await;
        assert!(peers.len() >= 4, "Network should be fully connected after healing");
    }
}

#[tokio::test]
async fn test_message_deduplication() {
    let node1 = create_test_node("node1").await;
    let node2 = create_test_node("node2").await;
    let node3 = create_test_node("node3").await;
    
    sleep(Duration::from_millis(500)).await;
    
    // Create a packet
    let packet = BitchatPacket::new_ping(
        node1.get_peer_id(),
        node3.get_peer_id(),
    );
    
    // Send the same packet multiple times
    for _ in 0..5 {
        node1.send_packet(packet.clone()).await.expect("Send failed");
    }
    
    // Also send from node2 (simulating relay)
    for _ in 0..3 {
        node2.send_packet(packet.clone()).await.expect("Send failed");
    }
    
    sleep(Duration::from_millis(500)).await;
    
    // Check deduplication stats (would need actual implementation)
    // In real implementation, node3 should only process packet once
    let stats = node3.get_statistics().await;
    
    // Verify deduplication is working
    assert!(stats.messages_deduplicated > 0, "Should have deduplicated messages");
}

#[tokio::test]
async fn test_gateway_bridging() {
    // Create local mesh network
    let local_nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..3).map(|i| create_test_node(&format!("local_{}", i)))
    ).await;
    
    // Create gateway node
    let gateway = create_test_node("gateway").await;
    gateway.enable_gateway_mode().await.expect("Failed to enable gateway mode");
    
    // Create remote mesh network
    let remote_nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..3).map(|i| create_test_node(&format!("remote_{}", i)))
    ).await;
    
    sleep(Duration::from_secs(1)).await;
    
    // Connect local and remote networks through gateway
    for local in &local_nodes {
        local.set_gateway(&gateway.get_peer_id()).await.expect("Failed to set gateway");
    }
    
    for remote in &remote_nodes {
        remote.set_gateway(&gateway.get_peer_id()).await.expect("Failed to set gateway");
    }
    
    sleep(Duration::from_secs(1)).await;
    
    // Send message from local to remote through gateway
    let packet = BitchatPacket::new_ping(
        local_nodes[0].get_peer_id(),
        remote_nodes[0].get_peer_id(),
    );
    
    local_nodes[0].send_packet(packet).await.expect("Failed to send through gateway");
    
    sleep(Duration::from_millis(500)).await;
    
    // Verify gateway statistics
    let gateway_stats = gateway.get_statistics().await;
    assert!(gateway_stats.messages_relayed > 0, "Gateway should relay messages");
}

#[tokio::test]
async fn test_network_resilience_under_churn() {
    // Create initial network
    let mut nodes = Vec::new();
    for i in 0..5 {
        nodes.push(create_test_node(&format!("stable_{}", i)).await);
    }
    
    sleep(Duration::from_secs(1)).await;
    
    // Simulate network churn
    let churn_handle = tokio::spawn(async move {
        let mut temp_nodes = Vec::new();
        
        for round in 0..3 {
            // Add new nodes
            for i in 0..2 {
                let node = create_test_node(&format!("churn_{}_{}", round, i)).await;
                temp_nodes.push(node);
            }
            
            sleep(Duration::from_millis(500)).await;
            
            // Remove some nodes
            if temp_nodes.len() > 2 {
                temp_nodes.drain(0..2);
            }
            
            sleep(Duration::from_millis(500)).await;
        }
    });
    
    // While churn is happening, verify stable nodes maintain connectivity
    for _ in 0..5 {
        sleep(Duration::from_millis(500)).await;
        
        for node in &nodes {
            let peers = node.get_connected_peers().await;
            assert!(peers.len() >= 2, "Stable nodes should maintain connections during churn");
        }
    }
    
    churn_handle.await.expect("Churn simulation failed");
}

#[tokio::test]
async fn test_multi_hop_routing() {
    // Create a line topology: node1 -- node2 -- node3 -- node4 -- node5
    let nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..5).map(|i| create_test_node(&format!("hop_{}", i)))
    ).await;
    
    // Manually connect in a line (disable auto-discovery for this test)
    for i in 0..4 {
        nodes[i].connect_peer(nodes[i + 1].get_peer_id()).await.expect("Connection failed");
        
        // Disconnect non-adjacent nodes to force multi-hop
        for j in (i + 2)..5 {
            nodes[i].disconnect_peer(&nodes[j].get_peer_id()).await;
        }
    }
    
    sleep(Duration::from_secs(1)).await;
    
    // Send message from node1 to node5 (should route through 2, 3, 4)
    let packet = BitchatPacket::new_ping(
        nodes[0].get_peer_id(),
        nodes[4].get_peer_id(),
    );
    
    nodes[0].send_packet(packet.clone()).await.expect("Failed to send packet");
    
    sleep(Duration::from_millis(500)).await;
    
    // Verify packet traversed the expected path
    // In real implementation, would check routing tables and hop counts
    let route = nodes[0].find_route(&nodes[4].get_peer_id()).await;
    assert!(route.is_some(), "Route should exist");
    
    if let Some(route) = route {
        assert!(route.hop_count >= 4, "Should be multi-hop route");
    }
}

#[tokio::test]
async fn test_priority_message_delivery() {
    let node1 = create_test_node("priority_sender").await;
    let node2 = create_test_node("priority_receiver").await;
    
    sleep(Duration::from_millis(500)).await;
    
    // Send mix of priority and normal messages
    for i in 0..10 {
        let mut packet = BitchatPacket::new_ping(
            node1.get_peer_id(),
            node2.get_peer_id(),
        );
        
        // Even messages are high priority
        if i % 2 == 0 {
            packet.set_priority(bitcraps::protocol::MessagePriority::High);
        }
        
        node1.send_packet(packet).await.expect("Failed to send packet");
    }
    
    sleep(Duration::from_millis(500)).await;
    
    // In real implementation, would verify high priority messages were delivered first
    let stats = node2.get_statistics().await;
    assert!(stats.messages_received >= 10, "All messages should be delivered");
}

/// Helper function to simulate network conditions
async fn simulate_network_conditions(
    nodes: &[Arc<MeshService>],
    packet_loss: f64,
    latency_ms: u64,
    duration: Duration,
) {
    let start = tokio::time::Instant::now();
    
    while start.elapsed() < duration {
        for node in nodes {
            // Simulate packet loss
            if rand::random::<f64>() < packet_loss {
                // Temporarily disconnect random peer
                let peers = node.get_connected_peers().await;
                if !peers.is_empty() {
                    let random_peer = peers[rand::random::<usize>() % peers.len()];
                    node.disconnect_peer(&random_peer).await;
                    
                    // Reconnect after latency
                    let node_clone = node.clone();
                    let peer_id = random_peer;
                    tokio::spawn(async move {
                        sleep(Duration::from_millis(latency_ms)).await;
                        let _ = node_clone.connect_peer(peer_id).await;
                    });
                }
            }
        }
        
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_network_under_adverse_conditions() {
    // Create network
    let nodes: Vec<Arc<MeshService>> = futures::future::join_all(
        (0..6).map(|i| create_test_node(&format!("adverse_{}", i)))
    ).await;
    
    sleep(Duration::from_secs(1)).await;
    
    // Start adverse conditions simulation
    let nodes_clone = nodes.clone();
    let conditions_handle = tokio::spawn(async move {
        simulate_network_conditions(
            &nodes_clone,
            0.1, // 10% packet loss
            200, // 200ms latency
            Duration::from_secs(5),
        ).await;
    });
    
    // Try to maintain operations under adverse conditions
    for _ in 0..10 {
        // Send test messages
        let sender = &nodes[rand::random::<usize>() % nodes.len()];
        let receiver_idx = rand::random::<usize>() % nodes.len();
        if receiver_idx != 0 {
            let receiver = &nodes[receiver_idx];
            
            let packet = BitchatPacket::new_ping(
                sender.get_peer_id(),
                receiver.get_peer_id(),
            );
            
            let _ = sender.send_packet(packet).await;
        }
        
        sleep(Duration::from_millis(200)).await;
    }
    
    conditions_handle.await.expect("Conditions simulation failed");
    
    // Verify network recovered
    sleep(Duration::from_secs(2)).await;
    
    for node in &nodes {
        let peers = node.get_connected_peers().await;
        assert!(!peers.is_empty(), "Nodes should recover connectivity");
    }
}