use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use bitcraps::crypto::BitchatIdentity;
use bitcraps::discovery::bluetooth_discovery::BluetoothDiscovery;
use bitcraps::transport::kademlia::{KademliaNode, NodeId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("BitCraps Peer Discovery Demo");
    println!("============================");
    
    // Create identity
    let identity = Arc::new(BitchatIdentity::generate_with_pow(8));
    println!("Generated peer identity: {:?}", identity.peer_id);
    
    // Test Bluetooth Discovery
    println!("\n🔵 Testing Bluetooth Discovery...");
    test_bluetooth_discovery(identity.clone()).await?;
    
    // Test Kademlia DHT  
    println!("\n🌐 Testing Kademlia DHT...");
    test_kademlia_dht(identity.peer_id).await?;
    
    println!("\n✅ All tests completed successfully!");
    Ok(())
}

async fn test_bluetooth_discovery(identity: Arc<BitchatIdentity>) -> Result<(), Box<dyn std::error::Error>> {
    // Create Bluetooth discovery service
    let discovery = BluetoothDiscovery::new(identity).await?;
    
    // Start discovery (this would normally run indefinitely)
    tokio::spawn(async move {
        if let Err(e) = discovery.start_discovery().await {
            eprintln!("Discovery error: {}", e);
        }
    });
    
    // Wait a bit to see discovery in action
    sleep(Duration::from_secs(2)).await;
    
    println!("✅ Bluetooth discovery started successfully");
    Ok(())
}

async fn test_kademlia_dht(peer_id: [u8; 32]) -> Result<(), Box<dyn std::error::Error>> {
    // Create Kademlia node
    let listen_addr = "127.0.0.1:0".parse()?;
    let node = KademliaNode::new(peer_id, listen_addr, 20, 3).await?;
    
    // Start the node
    node.start().await?;
    
    // Test storing and retrieving a value
    let key = b"test_key".to_vec();
    let value = b"test_value".to_vec();
    
    println!("📦 Storing value in DHT...");
    let stored = node.store(key.clone(), value.clone()).await?;
    println!("Store result: {}", stored);
    
    println!("🔍 Retrieving value from DHT...");
    let retrieved = node.get(key).await;
    
    match retrieved {
        Some(retrieved_value) => {
            if retrieved_value == value {
                println!("✅ Successfully stored and retrieved value!");
            } else {
                println!("❌ Retrieved value doesn't match original");
            }
        },
        None => {
            println!("⚠️ Value not found (this is expected for isolated node)");
        }
    }
    
    // Get node statistics
    let stats = node.get_stats().await;
    println!("📊 Node stats: {:#?}", stats);
    
    println!("✅ Kademlia DHT test completed");
    Ok(())
}