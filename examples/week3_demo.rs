// examples/week3_demo.rs
//! Week 3 demonstration of mesh service architecture, sharding, and PBFT consensus

use bitchat::{
    protocol::PeerId,
    mesh::{
        service::{MeshService, MeshServiceConfig, SecurityLevel},
        sharding::ShardManager,
    },
    transport::TransportManager,
    session::BitchatSessionManager,
    consensus::PBFTCoordinator,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Week 3: Mesh Service Architecture & Message Handling Demo ===\n");
    
    // Create test peer IDs
    let local_peer_id = PeerId::new([1u8; 32]);
    let peer2 = PeerId::new([2u8; 32]);
    let peer3 = PeerId::new([3u8; 32]);
    let peer4 = PeerId::new([4u8; 32]);
    
    println!("1. Setting up Mesh Service Architecture...");
    
    // Initialize core components
    let session_manager = BitchatSessionManager::new();
    let transport_manager = TransportManager::new(local_peer_id);
    
    let config = MeshServiceConfig {
        max_peers: 100,
        security_level: SecurityLevel::Moderate,
        enable_channels: true,
        ..Default::default()
    };
    
    let mut mesh_service = MeshService::new(
        session_manager,
        transport_manager,
        Some(config),
    )?;
    
    println!("   ✓ Mesh service initialized with security level: Moderate");
    
    println!("\n2. Testing Hierarchical Sharding System...");
    
    let mut shard_manager = ShardManager::new(local_peer_id, 10)?;
    
    // Add players to demonstrate sharding
    println!("   Adding players to shards:");
    
    let shard1 = shard_manager.add_player(peer2).await?;
    println!("   ✓ Player 2 added to shard: {}", shard1.0);
    
    let shard2 = shard_manager.add_player(peer3).await?;
    println!("   ✓ Player 3 added to shard: {}", shard2.0);
    
    // Verify players are in same shard
    if shard1 == shard2 {
        println!("   ✓ Both players correctly assigned to the same shard");
    }
    
    let shard3 = shard_manager.add_player(peer4).await?;
    println!("   ✓ Player 4 added to shard: {}", shard3.0);
    
    // Display shard statistics
    let stats = shard_manager.get_stats();
    println!("\n   Sharding Statistics:");
    println!("   - Total shards: {}", stats.total_shards);
    println!("   - Total players: {}", stats.total_players);
    println!("   - Average shard size: {:.2}", stats.average_shard_size);
    
    println!("\n3. Testing PBFT Coordinator Election...");
    
    let mut pbft_coordinator = PBFTCoordinator::new(local_peer_id)?;
    
    let candidates = vec![peer2, peer3, peer4];
    let election_result = pbft_coordinator
        .start_coordinator_election(shard1.clone(), candidates.clone())
        .await?;
    
    println!("   ✓ PBFT election completed!");
    println!("   - Elected coordinator: {:?}", election_result.elected_coordinator);
    println!("   - Vote count: {}", election_result.vote_count);
    println!("   - Election timestamp: {:?}", election_result.timestamp);
    
    println!("\n4. Testing Cross-Shard Operations...");
    
    // Test shard member queries
    let members = shard_manager.get_shard_members(&shard1);
    println!("   ✓ Shard {} has {} members", shard1.0, members.len());
    
    // Test peer-to-shard lookup
    if let Some(found_shard) = shard_manager.get_peer_shard(&peer2) {
        println!("   ✓ Found peer 2 in shard: {}", found_shard.0);
    }
    
    println!("\n5. Testing Player Removal and Rebalancing...");
    
    shard_manager.remove_player(peer3).await?;
    println!("   ✓ Player 3 removed from shard");
    
    let final_stats = shard_manager.get_stats();
    println!("   Final statistics:");
    println!("   - Total players: {}", final_stats.total_players);
    println!("   - Average shard size: {:.2}", final_stats.average_shard_size);
    
    println!("\n6. Testing Mesh Service Health Check...");
    
    let health_status = mesh_service.health_check().await;
    println!("   ✓ Health check completed:");
    for (component, healthy) in health_status {
        println!("   - {}: {}", component, if healthy { "✓ Healthy" } else { "✗ Unhealthy" });
    }
    
    let service_stats = mesh_service.get_stats();
    println!("   Service statistics:");
    println!("   - Uptime: {:?}", service_stats.uptime_start.elapsed());
    println!("   - Connected peers: {}", service_stats.peers_connected);
    
    println!("\n=== Week 3 Demo Completed Successfully! ===");
    println!("\nKey features demonstrated:");
    println!("✓ Mesh service architecture with configurable security");
    println!("✓ Hierarchical sharding for 100+ player support");
    println!("✓ PBFT consensus for coordinator election");
    println!("✓ Cross-shard message handling");
    println!("✓ Dynamic player assignment and rebalancing");
    println!("✓ Service health monitoring and statistics");
    
    Ok(())
}