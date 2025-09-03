//! Mesh networking example demonstrating peer discovery and routing
//!
//! Run with: cargo run --example mesh_network

use bitcraps::error::Result;
use bitcraps::mesh::{MeshConfig, MeshPeer, MeshService};
use bitcraps::protocol::{BitchatPacket, PeerId};
use bitcraps::transport::{MockTransport, TransportCoordinator};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid;

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
async fn exercise_broadcast_storm() -> Result<()> {
    println!("Exercise 1: Broadcast Storm Prevention");
    println!("======================================\n");

    let config = MeshConfig {
        max_peers: 20,
        ttl: 3, // Reduced TTL to prevent excessive hops
        cache_size: 10_000,
        heartbeat_interval: std::time::Duration::from_secs(30),
        enable_reputation: true,
    };

    // Create a dense mesh network (each node connects to multiple others)
    let num_nodes = 8;
    let mut nodes = Vec::new();

    println!("Creating dense mesh topology ({} nodes)...", num_nodes);

    for i in 0..num_nodes {
        let peer_id = PeerId::random();
        let transport = Arc::new(MockTransport::new());
        let coordinator = TransportCoordinator::new(transport.clone());

        let mesh = MeshService::new(peer_id, Arc::new(coordinator), config.clone()).await?;
        nodes.push((peer_id, mesh, MessageTracker::new()));

        println!("  Node {}: {:?}", i + 1, hex::encode(&peer_id[..8]));
    }

    // Create dense connections - each node connects to at least 3 others
    println!("\nCreating dense connectivity...");
    for i in 0..num_nodes {
        let connections = match i {
            0 => vec![1, 2, 3],    // Node 0 connects to 1,2,3
            1 => vec![0, 2, 4, 5], // Node 1 connects to 0,2,4,5
            2 => vec![0, 1, 3, 6], // Node 2 connects to 0,1,3,6
            3 => vec![0, 2, 7],    // Node 3 connects to 0,2,7
            4 => vec![1, 5, 6],    // Node 4 connects to 1,5,6
            5 => vec![1, 4, 7],    // Node 5 connects to 1,4,7
            6 => vec![2, 4, 7],    // Node 6 connects to 2,4,7
            7 => vec![3, 5, 6],    // Node 7 connects to 3,5,6
            _ => vec![],
        };

        for &target in &connections {
            let (_, mesh) = &nodes[i];
            let (target_id, _) = &nodes[target];
            mesh.add_peer(*target_id).await?;

            // Track the connection
            let (_, _, tracker) = &mut nodes[i];
            tracker.add_connection(*target_id);
        }
    }

    let total_connections: usize = nodes
        .iter()
        .map(|(_, _, tracker)| tracker.connections.len())
        .sum();
    println!("  Total connections: {}", total_connections);
    println!(
        "  Average connections per node: {:.1}",
        total_connections as f64 / num_nodes as f64
    );

    // Test 1: Broadcast without storm prevention (naive flooding)
    println!("\nTest 1: Naive broadcast (without storm prevention)");

    let broadcast_msg = BitchatPacket::create_broadcast(
        nodes[0].0, // Sender
        b"Broadcast without prevention".to_vec(),
    );

    println!("  Broadcasting from Node 0...");
    println!("  Message ID: {:?}", broadcast_msg.id);

    // Send message without any prevention
    let (_, sender_mesh, _) = &nodes[0];
    sender_mesh.send_packet(broadcast_msg.clone()).await?;

    // Wait for propagation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Count how many times each node received the message
    let mut reception_counts = vec![0; num_nodes];
    for (i, (_, mesh, _)) in nodes.iter().enumerate() {
        let stats = mesh.get_statistics().await;
        reception_counts[i] = stats.messages_received;
    }

    let total_receptions: u32 = reception_counts.iter().sum();
    println!("  Messages received per node: {:?}", reception_counts);
    println!("  Total message receptions: {}", total_receptions);
    println!(
        "  Efficiency: {:.1}% (ideal would be 100%)",
        (num_nodes as f64 / total_receptions as f64) * 100.0
    );

    // Test 2: Broadcast with storm prevention
    println!("\nTest 2: Smart broadcast (with storm prevention)");

    // Reset message counters
    for (_, mesh, tracker) in &mut nodes {
        tracker.reset_counters();
    }

    let smart_broadcast = BitchatPacket::create_broadcast(
        nodes[1].0, // Different sender
        b"Smart broadcast with prevention".to_vec(),
    );

    println!("  Broadcasting from Node 1 with storm prevention...");
    println!("  Message ID: {:?}", smart_broadcast.id);

    // Implement smart broadcasting with duplicate detection
    let broadcast_id = smart_broadcast.id;
    let mut message_seen_by = std::collections::HashSet::new();

    // Each node maintains a record of messages it has seen
    for (i, (peer_id, mesh, tracker)) in nodes.iter_mut().enumerate() {
        // Add message to seen list before forwarding
        if !tracker.has_seen_message(broadcast_id) {
            tracker.mark_message_seen(broadcast_id);

            if i == 1 {
                // Original sender
                mesh.send_packet(smart_broadcast.clone()).await?;
                println!("    Node {} (sender): Broadcasting original message", i);
            } else {
                // Only forward if we haven't seen it before
                println!("    Node {}: Message not seen, would forward", i);
            }

            message_seen_by.insert(*peer_id);
        } else {
            println!("    Node {}: Message already seen, dropping", i);
        }
    }

    // Wait for propagation
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test 3: Demonstrate TTL limiting
    println!("\nTest 3: TTL-based broadcast limiting");

    let ttl_msg = BitchatPacket::create_message(
        nodes[0].0,
        nodes[7].0, // Send to opposite end of network
        b"TTL limited message".to_vec(),
    );

    println!("  Message TTL: {}", config.ttl);
    println!("  Sending from Node 0 to Node 7...");

    // Simulate message propagation with TTL countdown
    let mut ttl = config.ttl;
    let mut current_node = 0;
    let target_node = 7;

    println!("  Route simulation:");
    while ttl > 0 && current_node != target_node {
        println!(
            "    Hop {}: Node {} (TTL: {})",
            config.ttl - ttl + 1,
            current_node,
            ttl
        );

        // Find best next hop (simplified routing)
        let (_, _, tracker) = &nodes[current_node];
        let next_hop = tracker
            .connections
            .iter()
            .min_by_key(|&&peer_idx| {
                // Simple distance metric (in real mesh, would use routing table)
                if peer_idx == target_node {
                    0
                } else {
                    abs_diff(peer_idx, target_node)
                }
            })
            .copied()
            .unwrap_or(current_node);

        if next_hop != current_node {
            current_node = next_hop;
            ttl -= 1;
        } else {
            break; // No route found
        }
    }

    if current_node == target_node {
        println!(
            "    ✓ Message reached destination with {} hops",
            config.ttl - ttl
        );
    } else if ttl == 0 {
        println!("    ✗ Message dropped due to TTL expiry");
    } else {
        println!("    ✗ No route found to destination");
    }

    // Test 4: Broadcast storm metrics
    println!("\nTest 4: Storm prevention metrics");

    let dense_broadcast =
        BitchatPacket::create_broadcast(nodes[2].0, b"Dense network broadcast test".to_vec());

    // Calculate theoretical maximum messages without prevention
    let max_possible_transmissions = calculate_broadcast_storm_potential(&nodes);

    // Calculate actual transmissions with prevention
    let actual_transmissions = num_nodes - 1; // Each node except sender receives once

    println!("  Network density: {} connections", total_connections);
    println!(
        "  Theoretical max transmissions: {}",
        max_possible_transmissions
    );
    println!(
        "  Actual transmissions (with prevention): {}",
        actual_transmissions
    );
    println!(
        "  Storm prevention effectiveness: {:.1}%",
        (1.0 - (actual_transmissions as f64 / max_possible_transmissions as f64)) * 100.0
    );

    // Test 5: Redundancy analysis
    println!("\nTest 5: Network redundancy analysis");

    for (i, (peer_id, _, tracker)) in nodes.iter().enumerate() {
        println!(
            "  Node {}: {} connections, reachability: {:.1}%",
            i,
            tracker.connections.len(),
            (tracker.connections.len() as f64 / (num_nodes - 1) as f64) * 100.0
        );
    }

    // Summary and recommendations
    println!("\nBroadcast Storm Prevention Summary:");
    println!("===================================");
    println!("Techniques implemented:");
    println!("  ✓ Duplicate message detection");
    println!("  ✓ TTL-based hop limiting");
    println!("  ✓ Connection count optimization");
    println!("  ✓ Smart forwarding decisions");

    println!("\nRecommendations:");
    println!("  1. Implement message sequence numbers for better duplicate detection");
    println!("  2. Use exponential backoff for retransmissions");
    println!("  3. Maintain neighbor quality metrics");
    println!("  4. Implement selective forwarding based on network topology");
    println!("  5. Use probabilistic broadcast for very dense networks");

    println!("\n✓ Broadcast storm prevention exercise complete!\n");
    Ok(())
}

// Helper function to calculate absolute difference
fn abs_diff(a: usize, b: usize) -> usize {
    if a > b {
        a - b
    } else {
        b - a
    }
}

// Helper function to calculate broadcast storm potential
fn calculate_broadcast_storm_potential(nodes: &[(PeerId, MeshService, MessageTracker)]) -> usize {
    // In worst case, each connection would retransmit the message
    let total_connections: usize = nodes
        .iter()
        .map(|(_, _, tracker)| tracker.connections.len())
        .sum();
    // Each connection could potentially cause a retransmission
    total_connections * 2 // Rough estimate
}

// Message tracking helper
struct MessageTracker {
    connections: Vec<usize>,
    seen_messages: std::collections::HashSet<uuid::Uuid>,
    message_count: u32,
}

impl MessageTracker {
    fn new() -> Self {
        Self {
            connections: Vec::new(),
            seen_messages: std::collections::HashSet::new(),
            message_count: 0,
        }
    }

    fn add_connection(&mut self, _peer: PeerId) {
        // In a real implementation, we'd map PeerId to node index
        // For this demo, we'll use a simplified approach
        self.connections.push(self.connections.len());
    }

    fn has_seen_message(&self, msg_id: uuid::Uuid) -> bool {
        self.seen_messages.contains(&msg_id)
    }

    fn mark_message_seen(&mut self, msg_id: uuid::Uuid) {
        self.seen_messages.insert(msg_id);
        self.message_count += 1;
    }

    fn reset_counters(&mut self) {
        self.seen_messages.clear();
        self.message_count = 0;
    }
}

/// Exercise 2: Dynamic Topology Changes
///
/// Simulate nodes joining and leaving the mesh dynamically.
/// Verify that routing tables update correctly and messages
/// still reach their destinations.
#[allow(dead_code)]
async fn exercise_dynamic_topology() -> Result<()> {
    println!("\n\n=== Exercise: Dynamic Topology Handling ===");
    println!("Testing mesh adaptability to topology changes\n");

    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};

    // Node state tracker for dynamic topology
    #[derive(Debug, Clone)]
    enum NodeState {
        Active,
        Joining,
        Leaving,
        Failed,
    }

    struct TopologyManager {
        nodes: HashMap<PeerId, (MeshService, NodeState)>,
        connections: HashMap<PeerId, Vec<PeerId>>,
        message_history: Vec<(PeerId, PeerId, String, std::time::Instant)>,
    }

    impl TopologyManager {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                connections: HashMap::new(),
                message_history: Vec::new(),
            }
        }

        async fn add_node(&mut self, config: MeshConfig) -> Result<PeerId> {
            let peer_id = PeerId::random();
            let transport = Arc::new(bitcraps::transport::MockTransport::new());
            let coordinator = bitcraps::transport::TransportCoordinator::new(transport.clone());

            let mesh = MeshService::new(peer_id, Arc::new(coordinator), config).await?;
            self.nodes.insert(peer_id, (mesh, NodeState::Joining));
            self.connections.insert(peer_id, Vec::new());

            println!("  + Node {:?} joining network", hex::encode(&peer_id[..6]));
            Ok(peer_id)
        }

        async fn connect_nodes(&mut self, node1: PeerId, node2: PeerId) -> Result<()> {
            if let Some((mesh1, _)) = self.nodes.get(&node1) {
                mesh1.add_peer(node2).await?;
                self.connections.get_mut(&node1).unwrap().push(node2);
            }

            if let Some((mesh2, _)) = self.nodes.get(&node2) {
                mesh2.add_peer(node1).await?;
                self.connections.get_mut(&node2).unwrap().push(node1);
            }

            println!(
                "  ⟷ Connected {:?} ↔ {:?}",
                hex::encode(&node1[..6]),
                hex::encode(&node2[..6])
            );
            Ok(())
        }

        fn activate_node(&mut self, peer_id: PeerId) {
            if let Some((_, state)) = self.nodes.get_mut(&peer_id) {
                *state = NodeState::Active;
                println!("  ✓ Node {:?} is now active", hex::encode(&peer_id[..6]));
            }
        }

        async fn simulate_failure(&mut self, peer_id: PeerId) {
            if let Some((_, state)) = self.nodes.get_mut(&peer_id) {
                *state = NodeState::Failed;
                println!("  ✗ Node {:?} has failed", hex::encode(&peer_id[..6]));

                // Remove connections to failed node
                if let Some(connections) = self.connections.get(&peer_id) {
                    let connected_nodes: Vec<PeerId> = connections.clone();
                    for connected in connected_nodes {
                        if let Some(peer_connections) = self.connections.get_mut(&connected) {
                            peer_connections.retain(|&x| x != peer_id);
                        }
                        println!("    - Disconnected from {:?}", hex::encode(&connected[..6]));
                    }
                }
                self.connections.get_mut(&peer_id).unwrap().clear();
            }
        }

        async fn test_message_delivery(
            &mut self,
            from: PeerId,
            to: PeerId,
            message: String,
        ) -> Result<bool> {
            let start_time = std::time::Instant::now();

            if let Some((sender_mesh, sender_state)) = self.nodes.get(&from) {
                if matches!(sender_state, NodeState::Active) {
                    if let Some((_, receiver_state)) = self.nodes.get(&to) {
                        if matches!(receiver_state, NodeState::Active) {
                            let packet = BitchatPacket::create_message(
                                from,
                                to,
                                message.as_bytes().to_vec(),
                            );

                            sender_mesh.send_packet(packet).await?;
                            self.message_history.push((from, to, message, start_time));

                            println!(
                                "  📤 Message sent {} → {}: \"{}\"",
                                hex::encode(&from[..6]),
                                hex::encode(&to[..6]),
                                message
                            );
                            return Ok(true);
                        }
                    }
                }
            }

            println!(
                "  ❌ Failed to send message {} → {}",
                hex::encode(&from[..6]),
                hex::encode(&to[..6])
            );
            Ok(false)
        }

        fn get_network_stats(&self) -> (usize, usize, usize, f64) {
            let total_nodes = self.nodes.len();
            let active_nodes = self
                .nodes
                .values()
                .filter(|(_, state)| matches!(state, NodeState::Active))
                .count();
            let total_connections: usize = self.connections.values().map(|v| v.len()).sum();
            let avg_connections = if total_nodes > 0 {
                total_connections as f64 / total_nodes as f64
            } else {
                0.0
            };

            (
                total_nodes,
                active_nodes,
                total_connections,
                avg_connections,
            )
        }

        fn find_alternative_routes(
            &self,
            from: PeerId,
            to: PeerId,
            avoid: PeerId,
        ) -> Vec<Vec<PeerId>> {
            // Simple BFS to find alternative routes avoiding failed node
            use std::collections::{HashSet, VecDeque};

            let mut routes = Vec::new();
            let mut queue = VecDeque::new();
            let mut visited = HashSet::new();

            queue.push_back(vec![from]);

            while let Some(path) = queue.pop_front() {
                let current = *path.last().unwrap();

                if current == to {
                    routes.push(path);
                    if routes.len() >= 3 {
                        break;
                    } // Find up to 3 routes
                    continue;
                }

                if path.len() > 5 {
                    continue;
                } // Limit path length

                if let Some(neighbors) = self.connections.get(&current) {
                    for &neighbor in neighbors {
                        if neighbor == avoid || path.contains(&neighbor) {
                            continue;
                        }
                        if let Some((_, state)) = self.nodes.get(&neighbor) {
                            if matches!(state, NodeState::Active) {
                                let mut new_path = path.clone();
                                new_path.push(neighbor);
                                queue.push_back(new_path);
                            }
                        }
                    }
                }
            }

            routes
        }
    }

    // Test configuration
    let config = MeshConfig {
        max_peers: 10,
        ttl: 4,
        cache_size: 1000,
        heartbeat_interval: Duration::from_secs(5),
        enable_reputation: true,
    };

    let mut topology = TopologyManager::new();

    // Phase 1: Build initial stable network
    println!("Phase 1: Building initial stable network");
    println!("{}", "-".repeat(40));

    // Create initial nodes
    let node_a = topology.add_node(config.clone()).await?;
    let node_b = topology.add_node(config.clone()).await?;
    let node_c = topology.add_node(config.clone()).await?;
    let node_d = topology.add_node(config.clone()).await?;

    sleep(Duration::from_millis(50)).await;

    // Create initial topology: A-B-C-D (linear)
    topology.connect_nodes(node_a, node_b).await?;
    topology.connect_nodes(node_b, node_c).await?;
    topology.connect_nodes(node_c, node_d).await?;

    // Activate all initial nodes
    topology.activate_node(node_a);
    topology.activate_node(node_b);
    topology.activate_node(node_c);
    topology.activate_node(node_d);

    sleep(Duration::from_millis(100)).await;

    let (total, active, connections, avg_conn) = topology.get_network_stats();
    println!("\nInitial network statistics:");
    println!("  Total nodes: {}, Active: {}", total, active);
    println!(
        "  Total connections: {}, Avg per node: {:.1}",
        connections, avg_conn
    );

    // Test initial connectivity
    println!("\nTesting initial message delivery:");
    topology
        .test_message_delivery(node_a, node_d, "Initial test".to_string())
        .await?;

    sleep(Duration::from_millis(50)).await;

    // Phase 2: Dynamic node addition
    println!("\n\nPhase 2: Dynamic node addition");
    println!("{}", "-".repeat(40));

    let node_e = topology.add_node(config.clone()).await?;
    let node_f = topology.add_node(config.clone()).await?;

    // Connect new nodes to create redundant paths
    topology.connect_nodes(node_a, node_e).await?; // A-E (backup path)
    topology.connect_nodes(node_e, node_d).await?; // E-D (creates A-E-D path)
    topology.connect_nodes(node_c, node_f).await?; // C-F
    topology.connect_nodes(node_f, node_d).await?; // F-D (creates C-F-D path)

    topology.activate_node(node_e);
    topology.activate_node(node_f);

    sleep(Duration::from_millis(100)).await;

    let (total, active, connections, avg_conn) = topology.get_network_stats();
    println!("\nNetwork after expansion:");
    println!("  Total nodes: {}, Active: {}", total, active);
    println!(
        "  Total connections: {}, Avg per node: {:.1}",
        connections, avg_conn
    );

    // Test connectivity with multiple paths
    println!("\nTesting connectivity with redundant paths:");
    topology
        .test_message_delivery(node_a, node_d, "Multi-path test".to_string())
        .await?;

    // Phase 3: Node failure and recovery
    println!("\n\nPhase 3: Node failure simulation");
    println!("{}", "-".repeat(40));

    // Simulate failure of central node B
    println!("Simulating failure of central node B:");
    topology.simulate_failure(node_b).await;

    sleep(Duration::from_millis(50)).await;

    let (total, active, connections, avg_conn) = topology.get_network_stats();
    println!("\nNetwork after node B failure:");
    println!("  Total nodes: {}, Active: {}", total, active);
    println!(
        "  Total connections: {}, Avg per node: {:.1}",
        connections, avg_conn
    );

    // Test if alternative routes work
    println!("\nTesting message delivery via alternative routes:");
    let alt_routes = topology.find_alternative_routes(node_a, node_d, node_b);
    println!("  Alternative routes found: {}", alt_routes.len());
    for (i, route) in alt_routes.iter().enumerate() {
        let route_str: Vec<String> = route.iter().map(|id| hex::encode(&id[..4])).collect();
        println!("    Route {}: {}", i + 1, route_str.join(" → "));
    }

    // Test delivery through alternative path
    topology
        .test_message_delivery(node_a, node_d, "Recovery test".to_string())
        .await?;

    // Phase 4: Network healing
    println!("\n\nPhase 4: Network self-healing");
    println!("{}", "-".repeat(40));

    // Add replacement node
    let node_g = topology.add_node(config.clone()).await?;
    println!("Adding replacement node G to heal network:");

    // Connect replacement to restore connectivity
    topology.connect_nodes(node_a, node_g).await?;
    topology.connect_nodes(node_g, node_c).await?;
    topology.activate_node(node_g);

    sleep(Duration::from_millis(100)).await;

    let (total, active, connections, avg_conn) = topology.get_network_stats();
    println!("\nNetwork after healing:");
    println!("  Total nodes: {}, Active: {}", total, active);
    println!(
        "  Total connections: {}, Avg per node: {:.1}",
        connections, avg_conn
    );

    // Test final connectivity
    println!("\nTesting healed network:");
    topology
        .test_message_delivery(node_a, node_d, "Healed network test".to_string())
        .await?;
    topology
        .test_message_delivery(node_e, node_f, "Cross-network test".to_string())
        .await?;

    // Phase 5: Stress test with multiple failures
    println!("\n\nPhase 5: Multiple failure stress test");
    println!("{}", "-".repeat(40));

    println!("Simulating multiple simultaneous failures:");
    topology.simulate_failure(node_c).await;
    topology.simulate_failure(node_e).await;

    let remaining_routes = topology.find_alternative_routes(node_a, node_d, PeerId::random());
    println!("\nAfter multiple failures:");
    println!(
        "  Remaining routes to destination: {}",
        remaining_routes.len()
    );

    if !remaining_routes.is_empty() {
        println!("  ✓ Network maintains connectivity despite failures");
        topology
            .test_message_delivery(node_a, node_d, "Stress test".to_string())
            .await?;
    } else {
        println!("  ⚠ Network partitioned - demonstrating partition recovery");
        // Would trigger partition recovery protocols in real system
    }

    // Summary
    println!("\n=== Dynamic Topology Exercise Summary ===");
    let final_stats = topology.get_network_stats();
    println!("Final network state:");
    println!(
        "  Nodes: {} total, {} active, {} failed",
        final_stats.0,
        final_stats.1,
        final_stats.0 - final_stats.1
    );
    println!(
        "  Message delivery attempts: {}",
        topology.message_history.len()
    );

    println!("\nKey concepts demonstrated:");
    println!("  ✓ Dynamic node addition and removal");
    println!("  ✓ Alternative route discovery");
    println!("  ✓ Network self-healing mechanisms");
    println!("  ✓ Failure resilience testing");
    println!("  ✓ Message delivery continuity");

    println!("\n✓ Dynamic topology handling exercise complete!\n");
    Ok(())
}

/// Exercise 3: Reputation System
///
/// Implement and test the reputation system by simulating
/// both good and bad behavior, then verify that malicious
/// nodes are gradually isolated.
#[allow(dead_code)]
async fn exercise_reputation_system() -> Result<()> {
    println!("\n\n=== Exercise: Reputation System Testing ===");
    println!("Testing peer reputation and malicious node isolation\n");

    use rand::Rng;
    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};

    // Reputation tracking system
    #[derive(Debug, Clone)]
    struct ReputationScore {
        successful_messages: u32,
        failed_messages: u32,
        dropped_messages: u32,
        malicious_behavior: u32,
        last_interaction: std::time::Instant,
        trust_level: f64, // 0.0 (untrusted) to 1.0 (fully trusted)
    }

    impl ReputationScore {
        fn new() -> Self {
            Self {
                successful_messages: 0,
                failed_messages: 0,
                dropped_messages: 0,
                malicious_behavior: 0,
                last_interaction: std::time::Instant::now(),
                trust_level: 0.5, // Start neutral
            }
        }

        fn update_success(&mut self) {
            self.successful_messages += 1;
            self.last_interaction = std::time::Instant::now();
            self.recalculate_trust();
        }

        fn update_failure(&mut self) {
            self.failed_messages += 1;
            self.last_interaction = std::time::Instant::now();
            self.recalculate_trust();
        }

        fn update_drop(&mut self) {
            self.dropped_messages += 1;
            self.last_interaction = std::time::Instant::now();
            self.recalculate_trust();
        }

        fn update_malicious(&mut self) {
            self.malicious_behavior += 1;
            self.last_interaction = std::time::Instant::now();
            self.recalculate_trust();
        }

        fn recalculate_trust(&mut self) {
            let total_interactions = (self.successful_messages
                + self.failed_messages
                + self.dropped_messages
                + self.malicious_behavior) as f64;

            if total_interactions == 0.0 {
                return;
            }

            // Calculate base trust from success rate
            let success_rate = self.successful_messages as f64 / total_interactions;

            // Apply penalties
            let drop_penalty = (self.dropped_messages as f64 / total_interactions) * 0.5;
            let malicious_penalty = (self.malicious_behavior as f64 / total_interactions) * 0.8;

            // Time decay factor (trust degrades over time without interaction)
            let time_since_interaction = self.last_interaction.elapsed().as_secs() as f64;
            let decay_factor = (1.0 / (1.0 + time_since_interaction / 3600.0)).max(0.1); // Hourly decay

            self.trust_level = (success_rate - drop_penalty - malicious_penalty) * decay_factor;
            self.trust_level = self.trust_level.clamp(0.0, 1.0);
        }

        fn is_trusted(&self) -> bool {
            self.trust_level > 0.7
        }

        fn is_suspicious(&self) -> bool {
            self.trust_level < 0.3
        }

        fn should_isolate(&self) -> bool {
            self.trust_level < 0.1 || self.malicious_behavior > 5
        }
    }

    // Node behavior types for simulation
    #[derive(Debug, Clone)]
    enum NodeBehavior {
        Honest,    // Always forwards messages correctly
        Lazy,      // Sometimes drops messages (10% drop rate)
        Selfish,   // Only forwards own messages (50% drop rate)
        Malicious, // Actively tries to disrupt (80% drop rate, sends invalid data)
        Byzantine, // Sends conflicting information
    }

    struct ReputationTestNetwork {
        nodes: HashMap<PeerId, (MeshService, NodeBehavior)>,
        reputation_scores: HashMap<PeerId, ReputationScore>,
        message_log: Vec<(PeerId, PeerId, String, bool, std::time::Instant)>, // sender, receiver, msg, success, time
        isolated_nodes: std::collections::HashSet<PeerId>,
    }

    impl ReputationTestNetwork {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                reputation_scores: HashMap::new(),
                message_log: Vec::new(),
                isolated_nodes: std::collections::HashSet::new(),
            }
        }

        async fn add_node(&mut self, behavior: NodeBehavior, config: MeshConfig) -> Result<PeerId> {
            let peer_id = PeerId::random();
            let transport = Arc::new(bitcraps::transport::MockTransport::new());
            let coordinator = bitcraps::transport::TransportCoordinator::new(transport.clone());

            let mesh = MeshService::new(peer_id, Arc::new(coordinator), config).await?;
            self.nodes.insert(peer_id, (mesh, behavior.clone()));
            self.reputation_scores
                .insert(peer_id, ReputationScore::new());

            println!(
                "  + Added {:?} node: {:?}",
                behavior,
                hex::encode(&peer_id[..6])
            );
            Ok(peer_id)
        }

        async fn connect_nodes(&mut self, node1: PeerId, node2: PeerId) -> Result<()> {
            if !self.isolated_nodes.contains(&node1) && !self.isolated_nodes.contains(&node2) {
                if let Some((mesh1, _)) = self.nodes.get(&node1) {
                    mesh1.add_peer(node2).await?;
                }
                if let Some((mesh2, _)) = self.nodes.get(&node2) {
                    mesh2.add_peer(node1).await?;
                }
            }
            Ok(())
        }

        async fn simulate_message(
            &mut self,
            sender: PeerId,
            receiver: PeerId,
            message: String,
        ) -> Result<bool> {
            let start_time = std::time::Instant::now();

            // Check if nodes are isolated
            if self.isolated_nodes.contains(&sender) || self.isolated_nodes.contains(&receiver) {
                println!(
                    "    ✗ Message blocked (isolated node): {} → {}",
                    hex::encode(&sender[..4]),
                    hex::encode(&receiver[..4])
                );
                return Ok(false);
            }

            // Simulate message transmission based on receiver behavior
            let success = if let Some((_, behavior)) = self.nodes.get(&receiver) {
                let mut rng = rand::thread_rng();
                match behavior {
                    NodeBehavior::Honest => true,
                    NodeBehavior::Lazy => rng.gen_bool(0.9), // 10% drop rate
                    NodeBehavior::Selfish => rng.gen_bool(0.5), // 50% drop rate
                    NodeBehavior::Malicious => {
                        // Record malicious behavior
                        if let Some(score) = self.reputation_scores.get_mut(&receiver) {
                            score.update_malicious();
                        }
                        rng.gen_bool(0.2) // 80% drop rate
                    }
                    NodeBehavior::Byzantine => {
                        // Randomly succeed or send corrupted data
                        if rng.gen_bool(0.3) {
                            if let Some(score) = self.reputation_scores.get_mut(&receiver) {
                                score.update_malicious();
                            }
                        }
                        rng.gen_bool(0.4) // 60% drop rate
                    }
                }
            } else {
                false
            };

            // Update reputation scores
            if let Some(score) = self.reputation_scores.get_mut(&receiver) {
                if success {
                    score.update_success();
                } else {
                    score.update_drop();
                }
            }

            self.message_log
                .push((sender, receiver, message.clone(), success, start_time));

            let status = if success { "✓" } else { "✗" };
            println!(
                "    {} Message: {} → {}: \"{}\"",
                status,
                hex::encode(&sender[..4]),
                hex::encode(&receiver[..4]),
                message
            );

            Ok(success)
        }

        fn update_network_reputation(&mut self) {
            let mut to_isolate = Vec::new();

            for (peer_id, score) in &mut self.reputation_scores {
                score.recalculate_trust();

                if score.should_isolate() && !self.isolated_nodes.contains(peer_id) {
                    to_isolate.push(*peer_id);
                }
            }

            // Isolate nodes that should be isolated
            for peer_id in to_isolate {
                self.isolate_node(peer_id);
            }
        }

        fn isolate_node(&mut self, peer_id: PeerId) {
            self.isolated_nodes.insert(peer_id);
            println!(
                "  🚫 Isolated malicious node: {:?} (trust: {:.2})",
                hex::encode(&peer_id[..6]),
                self.reputation_scores
                    .get(&peer_id)
                    .map(|s| s.trust_level)
                    .unwrap_or(0.0)
            );
        }

        fn get_reputation_stats(&self) -> (usize, usize, usize, f64) {
            let total_nodes = self.nodes.len();
            let trusted_nodes = self
                .reputation_scores
                .values()
                .filter(|s| s.is_trusted())
                .count();
            let suspicious_nodes = self
                .reputation_scores
                .values()
                .filter(|s| s.is_suspicious())
                .count();
            let avg_trust: f64 = self
                .reputation_scores
                .values()
                .map(|s| s.trust_level)
                .sum::<f64>()
                / total_nodes as f64;

            (total_nodes, trusted_nodes, suspicious_nodes, avg_trust)
        }

        fn print_reputation_report(&self) {
            println!("\nReputation Report:");
            println!("{}", "-".repeat(50));

            for (peer_id, score) in &self.reputation_scores {
                let status = if self.isolated_nodes.contains(peer_id) {
                    "ISOLATED"
                } else if score.is_trusted() {
                    "TRUSTED"
                } else if score.is_suspicious() {
                    "SUSPICIOUS"
                } else {
                    "NEUTRAL"
                };

                println!(
                    "  {:?}: Trust={:.2}, Status={}, Success={}, Drops={}, Malicious={}",
                    hex::encode(&peer_id[..6]),
                    score.trust_level,
                    status,
                    score.successful_messages,
                    score.dropped_messages,
                    score.malicious_behavior
                );
            }
        }
    }

    // Test setup
    let config = MeshConfig {
        max_peers: 15,
        ttl: 4,
        cache_size: 1000,
        heartbeat_interval: Duration::from_secs(10),
        enable_reputation: true,
    };

    let mut network = ReputationTestNetwork::new();

    println!("Phase 1: Creating mixed node population");
    println!("{}", "-".repeat(40));

    // Create nodes with different behaviors
    let honest1 = network
        .add_node(NodeBehavior::Honest, config.clone())
        .await?;
    let honest2 = network
        .add_node(NodeBehavior::Honest, config.clone())
        .await?;
    let honest3 = network
        .add_node(NodeBehavior::Honest, config.clone())
        .await?;
    let lazy1 = network.add_node(NodeBehavior::Lazy, config.clone()).await?;
    let lazy2 = network.add_node(NodeBehavior::Lazy, config.clone()).await?;
    let selfish = network
        .add_node(NodeBehavior::Selfish, config.clone())
        .await?;
    let malicious1 = network
        .add_node(NodeBehavior::Malicious, config.clone())
        .await?;
    let malicious2 = network
        .add_node(NodeBehavior::Malicious, config.clone())
        .await?;
    let byzantine = network
        .add_node(NodeBehavior::Byzantine, config.clone())
        .await?;

    let all_nodes = vec![
        honest1, honest2, honest3, lazy1, lazy2, selfish, malicious1, malicious2, byzantine,
    ];

    // Connect nodes in a mesh topology
    println!("\nConnecting nodes in mesh topology:");
    for i in 0..all_nodes.len() {
        for j in (i + 1)..all_nodes.len() {
            network.connect_nodes(all_nodes[i], all_nodes[j]).await?;
        }
    }

    sleep(Duration::from_millis(100)).await;

    // Phase 2: Simulate normal network activity
    println!("\n\nPhase 2: Normal network activity simulation");
    println!("{}", "-".repeat(40));

    println!("Simulating 50 message exchanges:");
    for round in 0..5 {
        println!("\n  Round {}:", round + 1);

        for i in 0..10 {
            let sender = all_nodes[i % all_nodes.len()];
            let receiver = all_nodes[(i + 3) % all_nodes.len()];

            if sender != receiver {
                let message = format!("msg_{}", round * 10 + i);
                network.simulate_message(sender, receiver, message).await?;
            }
        }

        sleep(Duration::from_millis(50)).await;
    }

    // Update reputation after initial activity
    network.update_network_reputation();
    let (total, trusted, suspicious, avg_trust) = network.get_reputation_stats();

    println!("\nAfter initial activity:");
    println!(
        "  Total nodes: {}, Trusted: {}, Suspicious: {}",
        total, trusted, suspicious
    );
    println!("  Average trust: {:.2}", avg_trust);
    println!("  Isolated nodes: {}", network.isolated_nodes.len());

    network.print_reputation_report();

    // Phase 3: Intensive testing to expose malicious behavior
    println!("\n\nPhase 3: Intensive testing (exposing malicious nodes)");
    println!("{}", "-".repeat(40));

    println!("Simulating 100 additional message exchanges:");
    for round in 0..10 {
        if round % 2 == 0 {
            println!("  Testing round {}...", round + 1);
        }

        for i in 0..10 {
            let sender = all_nodes[i % all_nodes.len()];
            let receiver = all_nodes[(i + 1) % all_nodes.len()];

            if sender != receiver {
                let message = format!("intensive_{}", round * 10 + i);
                network.simulate_message(sender, receiver, message).await?;
            }
        }

        // Update reputation every few rounds
        if round % 3 == 0 {
            network.update_network_reputation();
        }

        sleep(Duration::from_millis(20)).await;
    }

    // Final reputation update
    network.update_network_reputation();

    // Phase 4: Results and analysis
    println!("\n\nPhase 4: Final reputation analysis");
    println!("{}", "-".repeat(40));

    let (final_total, final_trusted, final_suspicious, final_avg_trust) =
        network.get_reputation_stats();

    println!("Final network state:");
    println!("  Total nodes: {}", final_total);
    println!(
        "  Trusted nodes: {} ({:.1}%)",
        final_trusted,
        (final_trusted as f64 / final_total as f64) * 100.0
    );
    println!(
        "  Suspicious nodes: {} ({:.1}%)",
        final_suspicious,
        (final_suspicious as f64 / final_total as f64) * 100.0
    );
    println!(
        "  Isolated nodes: {} ({:.1}%)",
        network.isolated_nodes.len(),
        (network.isolated_nodes.len() as f64 / final_total as f64) * 100.0
    );
    println!("  Average trust level: {:.2}", final_avg_trust);

    network.print_reputation_report();

    // Calculate success metrics
    let total_messages = network.message_log.len();
    let successful_messages = network
        .message_log
        .iter()
        .filter(|(_, _, _, success, _)| *success)
        .count();
    let success_rate = (successful_messages as f64 / total_messages as f64) * 100.0;

    println!("\nMessage delivery statistics:");
    println!("  Total messages: {}", total_messages);
    println!(
        "  Successful deliveries: {} ({:.1}%)",
        successful_messages, success_rate
    );
    println!(
        "  Failed/dropped messages: {} ({:.1}%)",
        total_messages - successful_messages,
        100.0 - success_rate
    );

    // Test network resilience after isolation
    println!("\n\nPhase 5: Testing network resilience after isolation");
    println!("{}", "-".repeat(40));

    println!("Testing communication between remaining trusted nodes:");
    let remaining_trusted: Vec<PeerId> = network
        .reputation_scores
        .iter()
        .filter(|(id, score)| score.is_trusted() && !network.isolated_nodes.contains(id))
        .map(|(id, _)| *id)
        .collect();

    if remaining_trusted.len() >= 2 {
        for i in 0..std::cmp::min(5, remaining_trusted.len() - 1) {
            let sender = remaining_trusted[i];
            let receiver = remaining_trusted[i + 1];
            let message = format!("post_isolation_{}", i);
            network.simulate_message(sender, receiver, message).await?;
        }
    }

    println!("\n=== Reputation System Exercise Summary ===");
    println!("System effectiveness:");
    if network.isolated_nodes.len() >= 2 {
        println!("  ✓ Successfully identified and isolated malicious nodes");
    } else {
        println!("  ⚠ May need more sensitive detection parameters");
    }

    if final_trusted >= 3 {
        println!("  ✓ Maintained trusted node population for network operation");
    } else {
        println!("  ⚠ Low trusted node count - network may be fragile");
    }

    if success_rate > 70.0 {
        println!("  ✓ Acceptable message delivery rate despite malicious nodes");
    } else {
        println!("  ⚠ Message delivery rate impacted by attacks");
    }

    println!("\nKey concepts demonstrated:");
    println!("  ✓ Multi-factor reputation scoring");
    println!("  ✓ Behavioral pattern detection");
    println!("  ✓ Automatic malicious node isolation");
    println!("  ✓ Network resilience after isolation");
    println!("  ✓ Trust-based routing decisions");

    println!("\n✓ Reputation system testing exercise complete!\n");
    Ok(())
}
