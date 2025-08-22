use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::protocol::PeerId;
// use crate::transport::kademlia::NodeId; // Commented out for now

/// DHT-based peer discovery
/// 
/// Feynman: This is like a distributed phone book where everyone
/// keeps a piece of the directory. To find someone, you ask your
/// neighbors, who ask their neighbors, until someone knows the answer.
/// It's impossible to destroy because there's no central directory.
pub struct DhtDiscovery {
    local_id: PeerId,
    bootstrap_nodes: Vec<SocketAddr>,
    discovered_peers: Arc<RwLock<HashMap<PeerId, DhtPeer>>>,
    crawl_queue: Arc<RwLock<VecDeque<PeerId>>>,
}

#[derive(Debug, Clone)]
pub struct DhtPeer {
    pub peer_id: PeerId,
    pub addresses: Vec<SocketAddr>,
    pub reputation: f64,
    pub last_seen: std::time::Instant,
    pub hop_distance: u32,
}

impl DhtDiscovery {
    pub fn new(
        local_id: PeerId,
        bootstrap_nodes: Vec<SocketAddr>,
    ) -> Self {
        Self {
            local_id,
            bootstrap_nodes,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            crawl_queue: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
    
    /// Bootstrap into the DHT network
    /// 
    /// Feynman: Like arriving in a new city and asking the first person
    /// you meet for directions. Bootstrap nodes are well-known meeting
    /// points where new nodes can join the network.
    pub async fn bootstrap(&self) -> Result<(), Box<dyn std::error::Error>> {
        for bootstrap_addr in &self.bootstrap_nodes {
            // Connect to bootstrap node
            println!("Connecting to bootstrap node: {}", bootstrap_addr);
            
            // Request initial peer list
            // In production, would implement actual protocol
        }
        
        // Start recursive crawl
        self.start_recursive_crawl().await?;
        
        Ok(())
    }
    
    /// Recursively crawl the DHT to discover peers
    async fn start_recursive_crawl(&self) -> Result<(), Box<dyn std::error::Error>> {
        let discovered_peers = self.discovered_peers.clone();
        let crawl_queue = self.crawl_queue.clone();
        let local_id = self.local_id;
        
        tokio::spawn(async move {
            loop {
                // Get next peer to query
                let target = {
                    let mut queue = crawl_queue.write().await;
                    queue.pop_front()
                };
                
                if let Some(target) = target {
                    // For now, just add some example peers
                    // In production, this would query the DHT network
                    let example_peers = vec![target]; // Simplified
                    
                    for peer_id in example_peers {
                        let mut peers = discovered_peers.write().await;
                        
                        if !peers.contains_key(&peer_id) {
                            // Calculate hop distance
                            let hop_distance = Self::calculate_hop_distance(&local_id, &peer_id);
                            
                            let dht_peer = DhtPeer {
                                peer_id,
                                addresses: Vec::new(), // Would be filled from response
                                reputation: 0.5, // Neutral starting reputation
                                last_seen: std::time::Instant::now(),
                                hop_distance,
                            };
                            
                            peers.insert(peer_id, dht_peer);
                            
                            // Add to crawl queue
                            crawl_queue.write().await.push_back(peer_id);
                        }
                    }
                } else {
                    // Queue empty, wait with exponential backoff
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        });
        
        Ok(())
    }
    
    /// Calculate logical hop distance between peers
    fn calculate_hop_distance(local_id: &PeerId, target_id: &PeerId) -> u32 {
        // XOR distance gives us logical distance in the DHT
        let mut distance = 0u32;
        for i in 0..32 {
            distance += (local_id[i] ^ target_id[i]).count_ones();
        }
        distance
    }
}