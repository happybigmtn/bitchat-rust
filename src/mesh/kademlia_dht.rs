//! Kademlia DHT Implementation for Multi-hop Routing
//!
//! This module implements a Kademlia-based distributed hash table
//! optimized for local mesh networks. It enables multi-hop routing
//! beyond direct Bluetooth range and provides resilience against
//! network partitions.

use crate::crypto::GameCrypto;
use crate::error::Error;
use crate::protocol::{Hash256, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Kademlia constants
pub const K_BUCKET_SIZE: usize = 20; // Number of nodes per bucket
pub const ALPHA: usize = 3; // Concurrent lookups
pub const BITS: usize = 256; // Key space size (256 bits for SHA256)
pub const REPUBLISH_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour
pub const EXPIRATION_TIME: Duration = Duration::from_secs(86400); // 24 hours
pub const PING_INTERVAL: Duration = Duration::from_secs(60); // 1 minute
pub const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// XOR distance metric for Kademlia
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance([u8; 32]);

impl Distance {
    /// Calculate XOR distance between two node IDs
    pub fn between(a: &PeerId, b: &PeerId) -> Self {
        let mut distance = [0u8; 32];
        for i in 0..32 {
            distance[i] = a[i] ^ b[i];
        }
        Distance(distance)
    }

    /// Get the highest bit set (leading zeros)
    pub fn leading_zeros(&self) -> u32 {
        for byte in &self.0 {
            if *byte != 0 {
                return byte.leading_zeros()
                    + (self.0.iter().position(|&b| b != 0).unwrap() as u32 * 8);
            }
        }
        256 // All zeros
    }

    /// Get bucket index for this distance
    pub fn bucket_index(&self) -> usize {
        let lz = self.leading_zeros();
        if lz >= 256 {
            0
        } else {
            (255 - lz) as usize
        }
    }
}

/// Node information in the DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: PeerId,
    pub address: SocketAddr,
    pub last_seen: u64,        // Unix timestamp in seconds
    pub rtt: Option<Duration>, // Round-trip time
    pub failures: u32,
}

impl Node {
    /// Create a new node
    pub fn new(id: PeerId, address: SocketAddr) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        Self {
            id,
            address,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            rtt: None,
            failures: 0,
        }
    }

    /// Check if node is likely alive
    pub fn is_alive(&self) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (now - self.last_seen) < PING_INTERVAL.as_secs() * 3 && self.failures < 3
    }

    /// Update node on successful contact
    pub fn update_success(&mut self, rtt: Duration) {
        use std::time::{SystemTime, UNIX_EPOCH};
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.rtt = Some(rtt);
        self.failures = 0;
    }

    /// Update node on failed contact
    pub fn update_failure(&mut self) {
        self.failures += 1;
    }
}

/// K-bucket for storing nodes at a specific distance
#[derive(Debug, Clone)]
pub struct KBucket {
    nodes: Vec<Node>,
    capacity: usize,
    replacement_cache: VecDeque<Node>,
}

impl KBucket {
    /// Create a new k-bucket
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            capacity,
            replacement_cache: VecDeque::with_capacity(capacity),
        }
    }

    /// Add or update a node
    pub fn add_node(&mut self, node: Node) -> bool {
        // Check if node already exists
        if let Some(_existing) = self.nodes.iter_mut().find(|n| n.id == node.id) {
            // Move to end (most recently seen)
            let idx = self.nodes.iter().position(|n| n.id == node.id).unwrap();
            self.nodes.remove(idx);
            self.nodes.push(node);
            return true;
        }

        // Add new node if space available
        if self.nodes.len() < self.capacity {
            self.nodes.push(node);
            return true;
        }

        // Check if least recently seen node is dead
        if let Some(first) = self.nodes.first() {
            if !first.is_alive() {
                self.nodes.remove(0);
                self.nodes.push(node);
                return true;
            }
        }

        // Add to replacement cache
        if !self.replacement_cache.iter().any(|n| n.id == node.id) {
            self.replacement_cache.push_back(node);
            if self.replacement_cache.len() > self.capacity {
                self.replacement_cache.pop_front();
            }
        }

        false
    }

    /// Remove a node
    pub fn remove_node(&mut self, id: &PeerId) {
        self.nodes.retain(|n| n.id != *id);

        // Try to fill from replacement cache
        while self.nodes.len() < self.capacity && !self.replacement_cache.is_empty() {
            if let Some(replacement) = self.replacement_cache.pop_front() {
                if replacement.is_alive() {
                    self.nodes.push(replacement);
                }
            }
        }
    }

    /// Get closest nodes to a target
    pub fn closest_nodes(&self, target: &PeerId, count: usize) -> Vec<Node> {
        let mut nodes: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| n.is_alive())
            .map(|n| (Distance::between(&n.id, target), n.clone()))
            .collect();

        nodes.sort_by_key(|(dist, _)| *dist);
        nodes.truncate(count);
        nodes.into_iter().map(|(_, node)| node).collect()
    }
}

/// Stored value in the DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredValue {
    pub key: Hash256,
    pub value: Vec<u8>,
    pub publisher: PeerId,
    pub published_at: u64, // Unix timestamp in seconds
    pub expires_at: u64,   // Unix timestamp in seconds
}

/// Kademlia DHT routing table
pub struct KademliaDHT {
    /// Our node ID
    local_id: PeerId,

    /// K-buckets indexed by distance
    buckets: Vec<KBucket>,

    /// Stored values
    storage: HashMap<Hash256, StoredValue>,

    /// Active lookups
    lookups: HashMap<Hash256, LookupState>,

    /// Routing metrics
    metrics: RoutingMetrics,
}

/// Lookup state for finding nodes or values
#[derive(Debug)]
struct LookupState {
    target: Hash256,
    queried: HashSet<PeerId>,
    to_query: VecDeque<Node>,
    closest_nodes: Vec<Node>,
    started_at: u64, // Unix timestamp
    is_value_lookup: bool,
    found_value: Option<Vec<u8>>,
}

/// Routing metrics
#[derive(Debug, Default)]
pub struct RoutingMetrics {
    pub total_nodes: usize,
    pub lookups_initiated: u64,
    pub lookups_succeeded: u64,
    pub values_stored: u64,
    pub messages_routed: u64,
}

impl KademliaDHT {
    /// Create a new Kademlia DHT
    pub fn new(local_id: PeerId) -> Self {
        let mut buckets = Vec::with_capacity(BITS);
        for _ in 0..BITS {
            buckets.push(KBucket::new(K_BUCKET_SIZE));
        }

        Self {
            local_id,
            buckets,
            storage: HashMap::new(),
            lookups: HashMap::new(),
            metrics: RoutingMetrics::default(),
        }
    }

    /// Add a node to the routing table
    pub fn add_node(&mut self, node: Node) {
        if node.id == self.local_id {
            return; // Don't add ourselves
        }

        let distance = Distance::between(&self.local_id, &node.id);
        let bucket_idx = distance.bucket_index();

        if bucket_idx < self.buckets.len() {
            self.buckets[bucket_idx].add_node(node);
            self.update_metrics();
        }
    }

    /// Remove a node from the routing table
    pub fn remove_node(&mut self, id: &PeerId) {
        let distance = Distance::between(&self.local_id, id);
        let bucket_idx = distance.bucket_index();

        if bucket_idx < self.buckets.len() {
            self.buckets[bucket_idx].remove_node(id);
            self.update_metrics();
        }
    }

    /// Find K closest nodes to a target
    pub fn find_closest_nodes(&self, target: &PeerId, k: usize) -> Vec<Node> {
        let mut all_nodes = Vec::new();

        // Collect nodes from all buckets
        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes.iter().filter(|n| n.is_alive()).cloned());
        }

        // Sort by distance to target
        all_nodes.sort_by_key(|n| Distance::between(&n.id, target));
        all_nodes.truncate(k);
        all_nodes
    }

    /// Start a node lookup
    pub fn lookup_node(&mut self, target: PeerId) -> Hash256 {
        let lookup_id = GameCrypto::hash(&target);

        // Find initial nodes to query
        let closest = self.find_closest_nodes(&target, ALPHA);

        let lookup_state = LookupState {
            target: lookup_id,
            queried: HashSet::new(),
            to_query: closest.clone().into_iter().collect(),
            closest_nodes: closest,
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            is_value_lookup: false,
            found_value: None,
        };

        self.lookups.insert(lookup_id, lookup_state);
        self.metrics.lookups_initiated += 1;

        lookup_id
    }

    /// Store a value in the DHT
    pub fn store_value(&mut self, key: Hash256, value: Vec<u8>) -> Result<(), Error> {
        if value.len() > 65536 {
            return Err(Error::InvalidState(
                "Value too large for DHT storage".into(),
            ));
        }

        let stored_value = StoredValue {
            key,
            value: value.clone(),
            publisher: self.local_id,
            published_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            expires_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + EXPIRATION_TIME.as_secs(),
        };

        self.storage.insert(key, stored_value);
        self.metrics.values_stored += 1;

        // Replicate to K closest nodes
        let target_id: PeerId = key; // Treat key as node ID for distance
        let closest = self.find_closest_nodes(&target_id, K_BUCKET_SIZE);

        // In practice, would send STORE messages to these nodes
        log::debug!(
            "Storing value with key {:?} to {} nodes",
            key,
            closest.len()
        );

        Ok(())
    }

    /// Retrieve a value from the DHT
    pub fn get_value(&mut self, key: Hash256) -> Option<Vec<u8>> {
        // Check local storage first
        if let Some(stored) = self.storage.get(&key) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if stored.expires_at > now {
                return Some(stored.value.clone());
            } else {
                // Expired, remove it
                self.storage.remove(&key);
            }
        }

        // Start lookup for value
        let target_id: PeerId = key;
        let lookup_id = self.lookup_node(target_id);

        // Mark as value lookup
        if let Some(lookup) = self.lookups.get_mut(&lookup_id) {
            lookup.is_value_lookup = true;
        }

        None // Would return after async lookup completes
    }

    /// Process a lookup response
    pub fn process_lookup_response(
        &mut self,
        lookup_id: Hash256,
        responder: PeerId,
        nodes: Vec<Node>,
        value: Option<Vec<u8>>,
    ) {
        // Clone nodes to avoid borrow issues
        let nodes_to_add: Vec<Node> = nodes.clone();

        if let Some(lookup) = self.lookups.get_mut(&lookup_id) {
            // Mark responder as queried
            lookup.queried.insert(responder);

            // If value found, we're done
            if let Some(val) = value {
                lookup.found_value = Some(val);
                self.metrics.lookups_succeeded += 1;
                return;
            }

            // Process new nodes
            for node in &nodes {
                if !lookup.queried.contains(&node.id) && node.id != self.local_id {
                    // Add to query list if closer than current closest
                    let distance = Distance::between(&node.id, &lookup.target);
                    let mut should_query = false;

                    if lookup.closest_nodes.len() < K_BUCKET_SIZE {
                        should_query = true;
                    } else if let Some(furthest) = lookup.closest_nodes.last() {
                        let furthest_dist = Distance::between(&furthest.id, &lookup.target);
                        if distance < furthest_dist {
                            should_query = true;
                        }
                    }

                    if should_query {
                        lookup.to_query.push_back(node.clone());
                        lookup.closest_nodes.push(node.clone());
                        lookup
                            .closest_nodes
                            .sort_by_key(|n| Distance::between(&n.id, &lookup.target));
                        lookup.closest_nodes.truncate(K_BUCKET_SIZE);
                    }
                }
            }
        }

        // Update routing table with new nodes (after lookup borrow ends)
        for node in nodes_to_add {
            self.add_node(node);
        }
    }

    /// Get next nodes to query for a lookup
    pub fn get_next_query_nodes(&mut self, lookup_id: Hash256) -> Vec<Node> {
        if let Some(lookup) = self.lookups.get_mut(&lookup_id) {
            let mut nodes = Vec::new();

            // Get up to ALPHA nodes to query
            while nodes.len() < ALPHA && !lookup.to_query.is_empty() {
                if let Some(node) = lookup.to_query.pop_front() {
                    if !lookup.queried.contains(&node.id) {
                        nodes.push(node);
                    }
                }
            }

            return nodes;
        }

        Vec::new()
    }

    /// Check if a lookup is complete
    pub fn is_lookup_complete(&self, lookup_id: &Hash256) -> bool {
        if let Some(lookup) = self.lookups.get(lookup_id) {
            // Complete if value found
            if lookup.found_value.is_some() {
                return true;
            }

            // Complete if no more nodes to query
            if lookup.to_query.is_empty() {
                return true;
            }

            // Complete if timeout
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if Duration::from_secs(now - lookup.started_at) > LOOKUP_TIMEOUT {
                return true;
            }

            // Complete if we've queried K closest nodes
            let closest_queried = lookup
                .closest_nodes
                .iter()
                .take(K_BUCKET_SIZE)
                .all(|n| lookup.queried.contains(&n.id));

            return closest_queried;
        }

        true
    }

    /// Clean up expired values and completed lookups
    pub fn cleanup(&mut self) {
        // Remove expired values
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.storage.retain(|_, v| v.expires_at > now);

        // Remove completed lookups
        let completed: Vec<Hash256> = self
            .lookups
            .iter()
            .filter(|(id, _)| self.is_lookup_complete(id))
            .map(|(id, _)| *id)
            .collect();

        for id in completed {
            self.lookups.remove(&id);
        }
    }

    /// Update routing metrics
    fn update_metrics(&mut self) {
        self.metrics.total_nodes = self.buckets.iter().map(|b| b.nodes.len()).sum();
    }

    /// Get routing table statistics
    pub fn get_stats(&self) -> RoutingStats {
        let mut bucket_sizes = Vec::new();
        for (i, bucket) in self.buckets.iter().enumerate() {
            if !bucket.nodes.is_empty() {
                bucket_sizes.push((i, bucket.nodes.len()));
            }
        }

        RoutingStats {
            total_nodes: self.metrics.total_nodes,
            active_buckets: bucket_sizes.len(),
            bucket_sizes,
            stored_values: self.storage.len(),
            active_lookups: self.lookups.len(),
            metrics: self.metrics.clone(),
        }
    }

    /// Bootstrap the DHT with seed nodes
    pub fn bootstrap(&mut self, seed_nodes: Vec<Node>) {
        for node in seed_nodes {
            self.add_node(node);
        }

        // Perform self-lookup to populate routing table
        self.lookup_node(self.local_id);
    }
}

/// Routing table statistics
#[derive(Debug)]
pub struct RoutingStats {
    pub total_nodes: usize,
    pub active_buckets: usize,
    pub bucket_sizes: Vec<(usize, usize)>,
    pub stored_values: usize,
    pub active_lookups: usize,
    pub metrics: RoutingMetrics,
}

impl Clone for RoutingMetrics {
    fn clone(&self) -> Self {
        Self {
            total_nodes: self.total_nodes,
            lookups_initiated: self.lookups_initiated,
            lookups_succeeded: self.lookups_succeeded,
            values_stored: self.values_stored,
            messages_routed: self.messages_routed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_xor_distance() {
        let a = [0xFF; 32];
        let b = [0x00; 32];
        let distance = Distance::between(&a, &b);

        assert_eq!(distance.0, [0xFF; 32]);
        assert_eq!(distance.leading_zeros(), 0);
        assert_eq!(distance.bucket_index(), 255);
    }

    #[test]
    fn test_kbucket_operations() {
        let mut bucket = KBucket::new(3);

        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        // Add nodes
        for i in 0..3 {
            let mut id = [0; 32];
            id[0] = i;
            let node = Node::new(id, addr);
            assert!(bucket.add_node(node));
        }

        // Bucket full, new node goes to cache
        let new_node = Node::new([4; 32], addr);
        assert!(!bucket.add_node(new_node));
        assert_eq!(bucket.nodes.len(), 3);
    }

    #[test]
    fn test_dht_creation() {
        let local_id = [1; 32];
        let dht = KademliaDHT::new(local_id);

        assert_eq!(dht.local_id, local_id);
        assert_eq!(dht.buckets.len(), BITS);
        assert!(dht.storage.is_empty());
    }

    #[test]
    fn test_store_and_retrieve() {
        let mut dht = KademliaDHT::new([1; 32]);

        let key = [2; 32];
        let value = b"test value".to_vec();

        assert!(dht.store_value(key, value.clone()).is_ok());
        assert_eq!(dht.get_value(key), Some(value));
    }
}
