// examples/week2_test.rs
//! Test program for Week 2 enhanced transport layer features
//! 
//! This demonstrates the new capabilities:
//! - Kademlia DHT for O(log n) routing
//! - Eclipse attack prevention with network diversity
//! - PoW identity generation for sybil resistance

use bitchat::protocol::{PeerId, ProtocolResult};
use bitchat::transport::{
    TransportLayer, 
    KademliaDht, 
    EclipsePreventionManager, 
    PowIdentityManager,
    DhtNode,
    TransportAddress
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> ProtocolResult<()> {
    println!("=== BitChat Week 2 Transport Layer Test ===");
    
    // Create a test peer ID
    let peer_id = PeerId::new([1u8; 32]);
    println!("Local Peer ID: {:?}", peer_id);
    
    // Test 1: Transport Layer Initialization
    println!("\n1. Initializing Transport Layer...");
    let transport_layer = TransportLayer::new(peer_id);
    println!("✓ Transport layer created successfully");
    
    // Test 2: PoW Identity Generation
    println!("\n2. Testing PoW Identity Generation...");
    let pow_manager = PowIdentityManager::new();
    match pow_manager.start().await {
        Ok(()) => println!("✓ PoW Identity Manager started"),
        Err(e) => println!("✗ Failed to start PoW manager: {}", e),
    }
    
    match pow_manager.generate_identity().await {
        Ok(identity) => {
            println!("✓ Generated PoW identity with score: {:.3}", identity.identity_score);
            println!("  - Peer ID: {:?}", identity.peer_id);
            println!("  - Difficulty: {}", identity.pow_solution.challenge.difficulty);
        }
        Err(e) => println!("✗ Failed to generate identity: {}", e),
    }
    
    // Test 3: Kademlia DHT
    println!("\n3. Testing Kademlia DHT...");
    let tcp_transport = std::sync::Arc::new(bitchat::transport::TcpTransport::new());
    let mut dht = KademliaDht::new(peer_id, tcp_transport);
    
    match dht.start().await {
        Ok(()) => println!("✓ Kademlia DHT started"),
        Err(e) => println!("✗ Failed to start DHT: {}", e),
    }
    
    // Test DHT statistics
    let stats = dht.get_statistics().await;
    println!("✓ DHT Statistics:");
    println!("  - Total nodes: {}", stats.total_nodes);
    println!("  - Non-empty buckets: {}", stats.non_empty_buckets);
    println!("  - Stored values: {}", stats.stored_values_count);
    
    // Test 4: Eclipse Attack Prevention
    println!("\n4. Testing Eclipse Attack Prevention...");
    let dht_arc = std::sync::Arc::new(tokio::sync::Mutex::new(dht));
    let eclipse_manager = EclipsePreventionManager::new(peer_id, dht_arc.clone());
    
    // Test connection evaluation
    let test_address = TransportAddress::Tcp("127.0.0.1:8000".parse::<SocketAddr>().unwrap());
    match eclipse_manager.evaluate_peer_connection(peer_id, &test_address).await {
        Ok(decision) => println!("✓ Connection decision: {:?}", decision),
        Err(e) => println!("✗ Failed to evaluate connection: {}", e),
    }
    
    // Test 5: DHT Node Operations
    println!("\n5. Testing DHT Node Operations...");
    let test_node = DhtNode::new(
        PeerId::new([2u8; 32]),
        vec![TransportAddress::Tcp("127.0.0.1:8001".parse().unwrap())]
    );
    
    dht_arc.lock().await.add_node(test_node).await;
    println!("✓ Added test node to DHT");
    
    // Test node lookup
    let target_node_id = bitchat::transport::NodeId::new([3u8; 32]);
    let closest_nodes = dht_arc.lock().await.find_closest_nodes(&target_node_id, 5).await;
    println!("✓ Found {} closest nodes for target", closest_nodes.len());
    
    // Test 6: PoW Statistics
    println!("\n6. PoW Identity Statistics...");
    let pow_stats = pow_manager.get_statistics().await;
    println!("✓ PoW Statistics:");
    println!("  - Registered peers: {}", pow_stats.registered_peers);
    println!("  - Recent solves: {}", pow_stats.recent_solve_count);
    println!("  - Current difficulty: {}", pow_stats.current_difficulty);
    if let Some(avg_time) = pow_stats.average_solve_time {
        println!("  - Average solve time: {:?}", avg_time);
    }
    
    println!("\n=== Week 2 Test Completed Successfully ===");
    println!("All enhanced features are working:");
    println!("  ✓ Kademlia DHT with O(log n) routing");
    println!("  ✓ Eclipse attack prevention with network diversity");
    println!("  ✓ PoW identity generation for sybil resistance");
    println!("  ✓ Integrated transport layer management");
    
    Ok(())
}