//! Cross-layer integration example showing data flow through all layers
//!
//! Run with: cargo run --example cross_layer_integration

use bitcraps::crypto::{decrypt_message, encrypt_message, Identity};
use bitcraps::error::Result;
use bitcraps::mesh::{MeshPacket, MeshService};
use bitcraps::protocol::{BitchatPacket, PeerId};
use bitcraps::transport::{BluetoothTransport, Transport, TransportPacket};
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
    println!("BitCraps Cross-Layer Integration");
    println!("=================================\n");

    // Show how data flows through each layer of the stack
    demonstrate_layer_flow().await?;

    // Show how layers interact during error conditions
    demonstrate_error_handling().await?;

    // Show optimization techniques across layers
    demonstrate_cross_layer_optimization().await?;

    Ok(())
}

/// Demonstrates data flow from application to network and back
async fn demonstrate_layer_flow() -> Result<()> {
    println!("Data Flow Through Layers");
    println!("------------------------\n");

    let sender = Identity::generate()?;
    let receiver = Identity::generate()?;

    // Application Layer: Create game action
    let game_action = b"PlaceBet:Pass:100";
    println!("1. APPLICATION LAYER");
    println!("   Action: {:?}", String::from_utf8_lossy(game_action));
    println!();

    // Protocol Layer: Wrap in protocol message
    let protocol_msg =
        BitchatPacket::create_message(sender.peer_id(), receiver.peer_id(), game_action.to_vec());
    println!("2. PROTOCOL LAYER");
    println!("   Packet ID: {:?}", protocol_msg.id);
    println!("   From: {:?}", protocol_msg.sender);
    println!("   To: {:?}", protocol_msg.recipient);
    println!();

    // Crypto Layer: Encrypt the message
    let encrypted = encrypt_message(&protocol_msg.payload, &sender, &receiver.public_key)?;
    println!("3. CRYPTO LAYER");
    println!("   Original size: {} bytes", protocol_msg.payload.len());
    println!("   Encrypted size: {} bytes", encrypted.len());
    println!(
        "   Overhead: {} bytes",
        encrypted.len() - protocol_msg.payload.len()
    );
    println!();

    // Compression Layer: Compress if beneficial
    let compressed = if encrypted.len() > 100 {
        let compressed = lz4_flex::compress_prepend_size(&encrypted);
        println!("4. COMPRESSION LAYER");
        println!("   Compressed size: {} bytes", compressed.len());
        println!(
            "   Compression ratio: {:.1}%",
            (compressed.len() as f64 / encrypted.len() as f64) * 100.0
        );
        println!();
        compressed
    } else {
        println!("4. COMPRESSION LAYER");
        println!("   Skipped (message too small)");
        println!();
        encrypted.clone()
    };

    // Mesh Layer: Add routing information
    let mesh_packet = MeshPacket {
        id: uuid::Uuid::new_v4(),
        source: sender.peer_id(),
        destination: receiver.peer_id(),
        ttl: 5,
        payload: compressed,
        route_history: vec![sender.peer_id()],
    };
    println!("5. MESH LAYER");
    println!("   TTL: {}", mesh_packet.ttl);
    println!("   Route: {:?}", mesh_packet.route_history);
    println!();

    // Transport Layer: Prepare for transmission
    let transport_packet = TransportPacket {
        header: vec![0x01, 0x00], // Version and flags
        payload: bincode::serialize(&mesh_packet)?,
    };
    println!("6. TRANSPORT LAYER");
    println!("   Header: {:?}", transport_packet.header);
    println!(
        "   Total size: {} bytes",
        transport_packet.header.len() + transport_packet.payload.len()
    );
    println!();

    // Physical Layer: Bluetooth transmission
    println!("7. PHYSICAL LAYER (Bluetooth)");
    println!("   MTU: 512 bytes");
    println!(
        "   Fragmentation: {}",
        if transport_packet.payload.len() > 512 {
            "Required"
        } else {
            "Not needed"
        }
    );
    println!();

    // Now show the reverse path
    println!("REVERSE PATH (Receiver Processing)");
    println!("-----------------------------------\n");

    // Transport receives
    println!("7→6. TRANSPORT RECEIVES");
    println!("     Reassemble fragments if needed");
    println!();

    // Mesh processes
    println!("6→5. MESH ROUTING");
    println!("     Check if we're destination");
    println!("     Update route history");
    println!("     Decrement TTL");
    println!();

    // Decompress
    println!("5→4. DECOMPRESSION");
    if compressed != encrypted {
        let decompressed = lz4_flex::decompress_size_prepended(&compressed)?;
        println!("     Decompressed to {} bytes", decompressed.len());
    } else {
        println!("     No decompression needed");
    }
    println!();

    // Decrypt
    println!("4→3. DECRYPTION");
    println!("     Verify sender signature");
    println!("     Decrypt with session key");
    println!();

    // Protocol processing
    println!("3→2. PROTOCOL VALIDATION");
    println!("     Check message integrity");
    println!("     Validate sender permissions");
    println!();

    // Application receives
    println!("2→1. APPLICATION PROCESSES");
    println!("     Execute game action");
    println!("     Update game state");
    println!("     Broadcast result to peers");

    Ok(())
}

/// Demonstrates how layers handle errors
async fn demonstrate_error_handling() -> Result<()> {
    println!("\n\nError Handling Across Layers");
    println!("-----------------------------\n");

    println!("Scenario 1: Crypto Validation Failure");
    println!("  → Crypto layer detects invalid signature");
    println!("  → Returns error to mesh layer");
    println!("  → Mesh layer logs and drops packet");
    println!("  → Updates sender reputation (potential attack)");
    println!();

    println!("Scenario 2: Network Partition");
    println!("  → Transport layer loses connectivity");
    println!("  → Mesh layer detects missing heartbeats");
    println!("  → Consensus layer pauses proposals");
    println!("  → Application layer queues actions");
    println!("  → On reconnect: State sync protocol activates");
    println!();

    println!("Scenario 3: Byzantine Node Detection");
    println!("  → Consensus layer detects conflicting votes");
    println!("  → Protocol layer validates evidence");
    println!("  → Mesh layer isolates malicious node");
    println!("  → Transport layer blocks connections");
    println!("  → Crypto layer revokes trust");

    Ok(())
}

/// Demonstrates cross-layer optimizations
async fn demonstrate_cross_layer_optimization() -> Result<()> {
    println!("\n\nCross-Layer Optimizations");
    println!("-------------------------\n");

    println!("1. MTU-Aware Compression");
    println!("   Transport reports MTU to compression layer");
    println!("   Compression targets output < MTU to avoid fragmentation");
    println!();

    println!("2. Topology-Aware Routing");
    println!("   Transport reports link quality to mesh");
    println!("   Mesh adjusts routing preferences");
    println!("   Consensus adjusts timeout based on topology");
    println!();

    println!("3. Battery-Aware Scheduling");
    println!("   Application reports battery level");
    println!("   Transport reduces scan frequency");
    println!("   Mesh increases heartbeat interval");
    println!("   Consensus batches proposals");
    println!();

    println!("4. Security-Performance Tradeoff");
    println!("   High security: Full encryption + signatures");
    println!("   Medium: Encryption only for sensitive data");
    println!("   Low: MAC authentication only");
    println!("   Decision based on threat level and performance needs");

    Ok(())
}

/// Exercise 1: Implement Custom Layer
///
/// Add a new layer between Protocol and Crypto that implements
/// content-based filtering (e.g., profanity filter, rate limiting).
#[allow(dead_code)]
fn exercise_custom_layer() {
    // TODO: Implement filtering layer
    // Hints:
    // 1. Create trait for FilterLayer
    // 2. Implement content checking
    // 3. Add rate limiting per peer
    // 4. Integrate with existing stack
}

/// Exercise 2: Performance Profiling
///
/// Profile the overhead of each layer and identify
/// optimization opportunities.
#[allow(dead_code)]
async fn exercise_performance_profiling() {
    // TODO: Add timing measurements
    // Hints:
    // 1. Use std::time::Instant for timing
    // 2. Measure each layer's processing time
    // 3. Identify bottlenecks
    // 4. Test with various message sizes
}

/// Exercise 3: Failure Injection
///
/// Inject failures at each layer and verify the system
/// handles them gracefully.
#[allow(dead_code)]
async fn exercise_failure_injection() {
    // TODO: Implement failure injection
    // Hints:
    // 1. Random packet drops at transport
    // 2. Corrupt signatures at crypto
    // 3. Invalid routing at mesh
    // 4. Conflicting proposals at consensus
    // 5. Verify system recovers
}
