//! Mesh networking example demonstrating peer discovery and routing
//! This simplified version focuses on educational concepts
//!
//! Run with: cargo run --example mesh_network_simple

use bitcraps::error::Result;
use bitcraps::protocol::{PeerId, PeerIdExt};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("BitCraps Mesh Network Example");
    println!("==============================\n");

    // Demonstrate the TODO implementations that were requested
    exercise_dynamic_topology_demo().await?;
    exercise_reputation_system_demo().await?;

    Ok(())
}

/// Exercise 2: Dynamic Topology Changes Demo
///
/// This demonstrates the concepts from the TODO implementation
async fn exercise_dynamic_topology_demo() -> Result<()> {
    println!("=== Dynamic Topology Handling Demo ===");
    println!("Demonstrating mesh adaptability to topology changes\n");

    // Simulate mesh network state
    struct SimpleMeshNetwork {
        nodes: HashMap<PeerId, Vec<PeerId>>, // node -> connected peers
        active_nodes: Vec<PeerId>,
    }

    impl SimpleMeshNetwork {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                active_nodes: Vec::new(),
            }
        }

        fn add_node(&mut self) -> PeerId {
            let peer_id = PeerId::random();
            self.nodes.insert(peer_id, Vec::new());
            self.active_nodes.push(peer_id);
            println!("  + Added node: {:?}", hex::encode(&peer_id[..6]));
            peer_id
        }

        fn connect_nodes(&mut self, node1: PeerId, node2: PeerId) {
            if let Some(connections) = self.nodes.get_mut(&node1) {
                connections.push(node2);
            }
            if let Some(connections) = self.nodes.get_mut(&node2) {
                connections.push(node1);
            }
            println!(
                "  ↔ Connected {:?} ↔ {:?}",
                hex::encode(&node1[..6]),
                hex::encode(&node2[..6])
            );
        }

        fn simulate_failure(&mut self, node: PeerId) {
            self.active_nodes.retain(|&x| x != node);
            // Remove all connections to failed node
            for connections in self.nodes.values_mut() {
                connections.retain(|&x| x != node);
            }
            println!("  ✗ Node failed: {:?}", hex::encode(&node[..6]));
        }

        fn find_route(&self, from: PeerId, to: PeerId) -> Option<Vec<PeerId>> {
            if !self.active_nodes.contains(&from) || !self.active_nodes.contains(&to) {
                return None;
            }

            use std::collections::{HashSet, VecDeque};
            let mut queue = VecDeque::new();
            let mut visited = HashSet::new();
            let mut parent: HashMap<PeerId, PeerId> = HashMap::new();

            queue.push_back(from);
            visited.insert(from);

            while let Some(current) = queue.pop_front() {
                if current == to {
                    // Reconstruct path
                    let mut path = vec![to];
                    let mut node = to;
                    while let Some(&prev) = parent.get(&node) {
                        path.push(prev);
                        node = prev;
                        if node == from {
                            break;
                        }
                    }
                    path.reverse();
                    return Some(path);
                }

                if let Some(neighbors) = self.nodes.get(&current) {
                    for &neighbor in neighbors {
                        if !visited.contains(&neighbor) && self.active_nodes.contains(&neighbor) {
                            visited.insert(neighbor);
                            parent.insert(neighbor, current);
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
            None
        }

        fn get_stats(&self) -> (usize, usize, f64) {
            let total_nodes = self.nodes.len();
            let active_nodes = self.active_nodes.len();
            let total_connections: usize = self.nodes.values().map(|v| v.len()).sum();
            let avg_connections = if total_nodes > 0 {
                total_connections as f64 / total_nodes as f64
            } else {
                0.0
            };
            (total_nodes, active_nodes, avg_connections)
        }
    }

    let mut network = SimpleMeshNetwork::new();

    println!("Phase 1: Building initial network");
    let node_a = network.add_node();
    let node_b = network.add_node();
    let node_c = network.add_node();
    let node_d = network.add_node();

    network.connect_nodes(node_a, node_b);
    network.connect_nodes(node_b, node_c);
    network.connect_nodes(node_c, node_d);

    let (total, active, avg_conn) = network.get_stats();
    println!(
        "  Initial network: {} nodes, {} active, {:.1} avg connections\n",
        total, active, avg_conn
    );

    // Test initial routing
    if let Some(route) = network.find_route(node_a, node_d) {
        let route_str: Vec<String> = route
            .iter()
            .map(|id| hex::encode(&id[..4]).to_string())
            .collect();
        println!("  Route A→D: {}", route_str.join(" → "));
    }

    println!("\nPhase 2: Adding redundant paths");
    let node_e = network.add_node();
    network.connect_nodes(node_a, node_e); // Alternative path
    network.connect_nodes(node_e, node_d);

    let (total, active, avg_conn) = network.get_stats();
    println!(
        "  Expanded network: {} nodes, {} active, {:.1} avg connections",
        total, active, avg_conn
    );

    println!("\nPhase 3: Simulating node failure");
    network.simulate_failure(node_b);

    // Test routing after failure
    if let Some(route) = network.find_route(node_a, node_d) {
        let route_str: Vec<String> = route
            .iter()
            .map(|id| hex::encode(&id[..4]).to_string())
            .collect();
        println!("  Alternative route A→D: {}", route_str.join(" → "));
    } else {
        println!("  No route found after failure");
    }

    let (total, active, avg_conn) = network.get_stats();
    println!(
        "  After failure: {} total, {} active, {:.1} avg connections",
        total, active, avg_conn
    );

    println!("\n✓ Dynamic topology handling demonstrated!");
    println!("Key concepts: Node addition, redundant paths, failure recovery\n");

    Ok(())
}

/// Exercise 3: Reputation System Demo
///
/// This demonstrates the concepts from the TODO implementation
async fn exercise_reputation_system_demo() -> Result<()> {
    println!("=== Reputation System Testing Demo ===");
    println!("Testing peer reputation and malicious node isolation\n");

    #[derive(Debug, Clone)]
    struct ReputationScore {
        successful: u32,
        failed: u32,
        trust_level: f64,
    }

    impl ReputationScore {
        fn new() -> Self {
            Self {
                successful: 0,
                failed: 0,
                trust_level: 0.5,
            }
        }

        fn record_success(&mut self) {
            self.successful += 1;
            self.update_trust();
        }

        fn record_failure(&mut self) {
            self.failed += 1;
            self.update_trust();
        }

        fn update_trust(&mut self) {
            let total = self.successful + self.failed;
            if total > 0 {
                self.trust_level = (self.successful as f64) / (total as f64);
            }
        }

        fn is_trusted(&self) -> bool {
            self.trust_level > 0.7
        }

        fn should_isolate(&self) -> bool {
            self.trust_level < 0.2 && (self.successful + self.failed) > 5
        }
    }

    #[derive(Debug)]
    enum NodeBehavior {
        Honest,
        Malicious,
        Lazy,
    }

    struct ReputationNetwork {
        nodes: HashMap<PeerId, (NodeBehavior, ReputationScore)>,
        isolated: Vec<PeerId>,
    }

    impl ReputationNetwork {
        fn new() -> Self {
            Self {
                nodes: HashMap::new(),
                isolated: Vec::new(),
            }
        }

        fn add_node(&mut self, behavior: NodeBehavior) -> PeerId {
            let peer_id = PeerId::random();
            self.nodes
                .insert(peer_id, (behavior, ReputationScore::new()));
            println!(
                "  + Added {:?} node: {:?}",
                self.nodes.get(&peer_id).unwrap().0,
                hex::encode(&peer_id[..6])
            );
            peer_id
        }

        fn simulate_interaction(&mut self, node: PeerId) -> bool {
            if self.isolated.contains(&node) {
                return false;
            }

            if let Some((behavior, score)) = self.nodes.get_mut(&node) {
                let success = match behavior {
                    NodeBehavior::Honest => rand::random::<f64>() > 0.1, // 90% success
                    NodeBehavior::Lazy => rand::random::<f64>() > 0.3,   // 70% success
                    NodeBehavior::Malicious => rand::random::<f64>() > 0.8, // 20% success
                };

                if success {
                    score.record_success();
                } else {
                    score.record_failure();
                }

                // Check if node should be isolated
                if score.should_isolate() && !self.isolated.contains(&node) {
                    self.isolated.push(node);
                    println!(
                        "  🚫 Isolated malicious node: {:?} (trust: {:.2})",
                        hex::encode(&node[..6]),
                        score.trust_level
                    );
                }

                return success;
            }
            false
        }

        fn get_reputation_stats(&self) -> (usize, usize, usize, f64) {
            let total = self.nodes.len();
            let trusted = self
                .nodes
                .values()
                .filter(|(_, score)| score.is_trusted())
                .count();
            let isolated = self.isolated.len();
            let avg_trust = self
                .nodes
                .values()
                .map(|(_, score)| score.trust_level)
                .sum::<f64>()
                / total as f64;
            (total, trusted, isolated, avg_trust)
        }
    }

    let mut reputation_net = ReputationNetwork::new();

    println!("Phase 1: Creating mixed node population");
    let honest1 = reputation_net.add_node(NodeBehavior::Honest);
    let honest2 = reputation_net.add_node(NodeBehavior::Honest);
    let lazy1 = reputation_net.add_node(NodeBehavior::Lazy);
    let malicious1 = reputation_net.add_node(NodeBehavior::Malicious);
    let malicious2 = reputation_net.add_node(NodeBehavior::Malicious);

    let nodes = vec![honest1, honest2, lazy1, malicious1, malicious2];

    println!("\nPhase 2: Simulating network interactions");
    for round in 0..20 {
        println!("  Round {}: Testing behavior...", round + 1);
        for &node in &nodes {
            let success = reputation_net.simulate_interaction(node);
            if round < 3 {
                // Show early results
                let (_, score) = reputation_net.nodes.get(&node).unwrap();
                print!(
                    "    {:?}: {} (trust: {:.2}) ",
                    hex::encode(&node[..4]),
                    if success { "✓" } else { "✗" },
                    score.trust_level
                );
            }
        }
        if round < 3 {
            println!();
        }

        sleep(Duration::from_millis(10)).await; // Simulate time passage
    }

    println!("\nPhase 3: Final reputation analysis");
    let (total, trusted, isolated, avg_trust) = reputation_net.get_reputation_stats();

    println!("Final network state:");
    println!("  Total nodes: {}", total);
    println!(
        "  Trusted nodes: {} ({:.1}%)",
        trusted,
        (trusted as f64 / total as f64) * 100.0
    );
    println!(
        "  Isolated nodes: {} ({:.1}%)",
        isolated,
        (isolated as f64 / total as f64) * 100.0
    );
    println!("  Average trust: {:.2}", avg_trust);

    println!("\nDetailed reputation report:");
    for (peer_id, (behavior, score)) in &reputation_net.nodes {
        let status = if reputation_net.isolated.contains(peer_id) {
            "ISOLATED"
        } else if score.is_trusted() {
            "TRUSTED"
        } else {
            "SUSPICIOUS"
        };

        println!(
            "  {:?}: {:?}, Trust={:.2}, Status={}, Success/Fail={}/{}",
            hex::encode(&peer_id[..6]),
            behavior,
            score.trust_level,
            status,
            score.successful,
            score.failed
        );
    }

    println!("\n✓ Reputation system testing demonstrated!");
    println!("Key concepts: Behavior tracking, trust calculation, automatic isolation");

    if isolated >= 2 {
        println!("🎉 Successfully identified and isolated malicious nodes!");
    }

    Ok(())
}
