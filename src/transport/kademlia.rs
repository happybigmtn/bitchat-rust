use std::collections::{HashMap, BTreeMap};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use crate::protocol::PeerId;

/// Kademlia node ID - 256-bit identifier
/// 
/// Feynman: Every node gets a "phone number" - a unique 256-bit ID.
/// The magic is that we can calculate "distance" between IDs using XOR.
/// Close IDs have similar bit patterns, far IDs are very different.
/// This creates a natural "neighborhood" structure in the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    pub fn from_peer_id(peer_id: &PeerId) -> Self {
        // Feynman: Convert a peer's identity into their DHT "address"
        Self(*peer_id)
    }
    
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Calculate XOR distance between two node IDs
    /// 
    /// Feynman: XOR distance is brilliant! It creates a metric space where:
    /// - Distance to yourself is 0
    /// - Distance is symmetric (A to B = B to A)
    /// - Triangle inequality holds
    /// This naturally organizes nodes into a tree structure
    pub fn distance(&self, other: &NodeId) -> Distance {
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = self.0[i] ^ other.0[i];
        }
        Distance(result)
    }
    
    /// Find which bucket this node belongs in relative to us
    pub fn bucket_index(&self, other: &NodeId) -> usize {
        let distance = self.distance(other);
        distance.leading_zeros()
    }
}

/// XOR distance metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance([u8; 32]);

impl Distance {
    /// Count leading zero bits (determines bucket index)
    pub fn leading_zeros(&self) -> usize {
        for (i, &byte) in self.0.iter().enumerate() {
            if byte != 0 {
                return i * 8 + byte.leading_zeros() as usize;
            }
        }
        256 // All zeros means same node
    }
}

/// Contact information for a node
#[derive(Debug, Clone)]
pub struct Contact {
    pub id: NodeId,
    pub peer_id: PeerId,
    pub address: SocketAddr,
    pub last_seen: Instant,
    pub rtt: Option<Duration>, // Round-trip time
}

/// K-bucket storing up to K contacts at a specific distance
/// 
/// Feynman: Imagine organizing your contacts by how "similar" their
/// phone numbers are to yours. Each bucket holds people whose numbers
/// differ by a specific number of initial digits. The genius is that
/// you keep more contacts who are "close" to you and fewer who are "far".
#[allow(dead_code)]
pub struct KBucket {
    contacts: Vec<Contact>,
    max_size: usize,
    last_updated: Instant,
}

impl KBucket {
    pub fn new(max_size: usize) -> Self {
        Self {
            contacts: Vec::new(),
            max_size,
            last_updated: Instant::now(),
        }
    }
    
    /// Add or update a contact
    /// 
    /// Feynman: When we hear from a node, we:
    /// 1. Move it to the end if it exists (most recently seen)
    /// 2. Add it if there's room
    /// 3. Ping the oldest node if bucket is full (to check if it's alive)
    pub fn add_contact(&mut self, contact: Contact) -> Option<Contact> {
        // Check if contact already exists
        if let Some(pos) = self.contacts.iter().position(|c| c.id == contact.id) {
            // Move to end (most recently seen)
            self.contacts.remove(pos);
            self.contacts.push(contact);
            None
        } else if self.contacts.len() < self.max_size {
            // Add new contact
            self.contacts.push(contact);
            None
        } else {
            // Bucket full - return oldest for eviction check
            Some(self.contacts[0].clone())
        }
    }
    
    /// Remove a contact
    pub fn remove_contact(&mut self, id: &NodeId) {
        self.contacts.retain(|c| c.id != *id);
    }
    
    /// Get K closest contacts to a target
    pub fn closest_contacts(&self, target: &NodeId, k: usize) -> Vec<Contact> {
        let mut contacts = self.contacts.clone();
        contacts.sort_by_key(|c| c.id.distance(target));
        contacts.truncate(k);
        contacts
    }
}

/// Kademlia routing table
/// 
/// Feynman: The routing table is like a hierarchical phone book.
/// We organize contacts into 256 buckets based on how many bits
/// they share with our ID. This gives us O(log n) lookups!
pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<RwLock<KBucket>>,
    k: usize, // Replication parameter (typically 20)
    alpha: usize, // Concurrency parameter (typically 3)
}

impl RoutingTable {
    pub fn new(local_id: NodeId, k: usize, alpha: usize) -> Self {
        let mut buckets = Vec::new();
        for _ in 0..256 {
            buckets.push(RwLock::new(KBucket::new(k)));
        }
        
        Self {
            local_id,
            buckets,
            k,
            alpha,
        }
    }
    
    /// Add a contact to the appropriate bucket
    pub async fn add_contact(&self, contact: Contact) {
        let bucket_idx = self.local_id.bucket_index(&contact.id);
        if bucket_idx < 256 {
            let mut bucket = self.buckets[bucket_idx].write().await;
            if let Some(_eviction_candidate) = bucket.add_contact(contact) {
                // TODO: Ping eviction candidate to check if still alive
                // If dead, replace with new contact
            }
        }
    }
    
    /// Find K closest nodes to a target
    /// 
    /// Feynman: To find nodes close to a target, we:
    /// 1. Start with our closest bucket
    /// 2. Expand outward to neighboring buckets
    /// 3. Collect K total nodes sorted by distance
    /// This is like asking "who do you know near address X?"
    pub async fn find_closest(&self, target: &NodeId, k: usize) -> Vec<Contact> {
        let mut all_contacts = Vec::new();
        let bucket_idx = self.local_id.bucket_index(target);
        
        // Start from target bucket and expand outward
        for distance in 0..256 {
            // Check bucket at +distance
            if bucket_idx + distance < 256 {
                let bucket = self.buckets[bucket_idx + distance].read().await;
                all_contacts.extend(bucket.contacts.clone());
            }
            
            // Check bucket at -distance
            if distance > 0 && bucket_idx >= distance {
                let bucket = self.buckets[bucket_idx - distance].read().await;
                all_contacts.extend(bucket.contacts.clone());
            }
            
            // Stop if we have enough contacts
            if all_contacts.len() >= k * 3 {
                break;
            }
        }
        
        // Sort by distance and return K closest
        all_contacts.sort_by_key(|c| c.id.distance(target));
        all_contacts.truncate(k);
        all_contacts
    }
}

/// Kademlia DHT node
/// 
/// Feynman: This is the complete DHT node that can:
/// - Find nodes (discover peers close to a target)
/// - Store values (distributed key-value storage)
/// - Maintain routing table health
/// - Handle incoming queries
#[allow(dead_code)]
pub struct KademliaNode {
    local_id: NodeId,
    routing_table: Arc<RoutingTable>,
    storage: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    pending_queries: Arc<RwLock<HashMap<u64, mpsc::Sender<Vec<Contact>>>>>,
    query_counter: Arc<RwLock<u64>>,
}

impl KademliaNode {
    pub fn new(peer_id: PeerId, k: usize, alpha: usize) -> Self {
        let local_id = NodeId::from_peer_id(&peer_id);
        
        Self {
            local_id,
            routing_table: Arc::new(RoutingTable::new(local_id, k, alpha)),
            storage: Arc::new(RwLock::new(HashMap::new())),
            pending_queries: Arc::new(RwLock::new(HashMap::new())),
            query_counter: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Perform iterative node lookup
    /// 
    /// Feynman: The lookup algorithm is like a game of "hot and cold":
    /// 1. Ask α nodes for their K closest to target
    /// 2. From responses, pick α new nodes even closer
    /// 3. Repeat until no closer nodes are found
    /// This converges in O(log n) steps!
    pub async fn lookup_node(&self, target: NodeId) -> Vec<Contact> {
        let mut queried = HashSet::new();
        let mut to_query = self.routing_table.find_closest(&target, self.routing_table.alpha).await;
        let mut closest = BTreeMap::new();
        
        while !to_query.is_empty() {
            let mut futures = Vec::new();
            
            // Query α nodes in parallel
            for contact in to_query.drain(..).take(self.routing_table.alpha) {
                if queried.insert(contact.id) {
                    futures.push(self.send_find_node(contact.clone(), target));
                }
            }
            
            // Wait for responses
            let responses = futures::future::join_all(futures).await;
            
            // Process responses
            let mut improved = false;
            for response in responses {
                if let Ok(contacts) = response {
                    for contact in contacts {
                        let distance = contact.id.distance(&target);
                        if !closest.contains_key(&distance) {
                            improved = true;
                            closest.insert(distance, contact.clone());
                            
                            // Add to routing table
                            self.routing_table.add_contact(contact.clone()).await;
                            
                            // Consider querying this node
                            if !queried.contains(&contact.id) {
                                to_query.push(contact);
                            }
                        }
                    }
                }
            }
            
            // Stop if no improvement
            if !improved {
                break;
            }
        }
        
        // Return K closest
        closest.into_iter()
            .take(self.routing_table.k)
            .map(|(_, contact)| contact)
            .collect()
    }
    
    /// Store a value in the DHT
    /// 
    /// Feynman: To store data, we:
    /// 1. Find K nodes closest to the key
    /// 2. Send store requests to all of them
    /// This creates K replicas for fault tolerance
    pub async fn store(&self, key: Vec<u8>, value: Vec<u8>) {
        // Calculate key ID
        let mut hasher = Sha256::new();
        hasher.update(&key);
        let key_id = NodeId::new(hasher.finalize().into());
        
        // Find K closest nodes
        let nodes = self.lookup_node(key_id).await;
        
        // Store on each node
        for node in nodes {
            self.send_store(node, key.clone(), value.clone()).await;
        }
        
        // Also store locally if we're close enough
        self.storage.write().await.insert(key, value);
    }
    
    /// Retrieve a value from the DHT
    pub async fn get(&self, key: Vec<u8>) -> Option<Vec<u8>> {
        // Check local storage first
        if let Some(value) = self.storage.read().await.get(&key) {
            return Some(value.clone());
        }
        
        // Calculate key ID and search network
        let mut hasher = Sha256::new();
        hasher.update(&key);
        let key_id = NodeId::new(hasher.finalize().into());
        
        // Find nodes storing this key
        let nodes = self.lookup_node(key_id).await;
        
        // Query each node
        for node in nodes {
            if let Ok(Some(value)) = self.send_get(node, key.clone()).await {
                return Some(value);
            }
        }
        
        None
    }
    
    // Stub methods for network operations
    async fn send_find_node(&self, _contact: Contact, _target: NodeId) -> Result<Vec<Contact>, Box<dyn std::error::Error>> {
        // TODO: Implement actual network call
        Ok(vec![])
    }
    
    async fn send_store(&self, _contact: Contact, _key: Vec<u8>, _value: Vec<u8>) {
        // TODO: Implement actual network call
    }
    
    async fn send_get(&self, _contact: Contact, _key: Vec<u8>) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        // TODO: Implement actual network call
        Ok(None)
    }
}

// Add missing import
use std::collections::HashSet;