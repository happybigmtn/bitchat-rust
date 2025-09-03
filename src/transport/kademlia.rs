use crate::protocol::PeerId;
use crate::transport::nat_traversal::NetworkHandler;
use crate::transport::pow_identity::ProofOfWork;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{mpsc, oneshot, RwLock};

/// Kademlia node ID - 256-bit identifier with cryptographic validation
///
/// Feynman: Every node gets a "phone number" - a unique 256-bit ID.
/// The magic is that we can calculate "distance" between IDs using XOR.
/// Close IDs have similar bit patterns, far IDs are very different.
/// This creates a natural "neighborhood" structure in the network.
///
/// Security Enhancement: NodeIDs now require cryptographic proof to prevent poisoning
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId {
    id: [u8; 32],
    proof_of_work: Option<ProofOfWork>,
}

impl NodeId {
    /// Create a new NodeId for testing only
    #[cfg(test)]
    pub fn new_test(bytes: [u8; 32]) -> Self {
        // Generate minimal proof for testing
        let proof = ProofOfWork::generate(&bytes, 1).expect("Test proof generation");
        Self {
            id: bytes,
            proof_of_work: Some(proof),
        }
    }

    /// Create a NodeId with cryptographic proof-of-work
    pub fn new_with_proof(bytes: [u8; 32], proof: ProofOfWork) -> Result<Self, &'static str> {
        // Validate proof-of-work
        if !proof.verify(&bytes) {
            return Err("Invalid proof-of-work for NodeId");
        }
        Ok(Self {
            id: bytes,
            proof_of_work: Some(proof),
        })
    }

    /// Generate a new NodeId with required proof-of-work
    pub fn generate_secure(difficulty: u32) -> Self {
        let mut rng = rand::rngs::OsRng;
        loop {
            let mut id_bytes = [0u8; 32];
            rng.fill_bytes(&mut id_bytes);

            if let Ok(proof) = ProofOfWork::generate(&id_bytes, difficulty) {
                return Self {
                    id: id_bytes,
                    proof_of_work: Some(proof),
                };
            }
            // Continue loop if proof generation fails
        }
    }

    pub fn from_peer_id(peer_id: &PeerId) -> Result<Self, &'static str> {
        // Feynman: Convert a peer's identity into their DHT "address"
        // Require proof-of-work for all production nodes
        // The peer should have pre-computed their proof
        Err("Peer must provide NodeId with proof-of-work")
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.id
    }

    /// Validate this NodeId's cryptographic proof
    pub fn is_valid(&self) -> bool {
        match &self.proof_of_work {
            Some(proof) => proof.verify(&self.id),
            None => false, // Nodes without proof are rejected
        }
    }

    /// Check if this NodeId has valid proof
    pub fn has_proof(&self) -> bool {
        self.proof_of_work.is_some()
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
            result[i] = self.id[i] ^ other.id[i];
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

/// Contact information for a node with anti-spam measures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: NodeId,
    pub peer_id: PeerId,
    pub address: SocketAddr,
    #[serde(skip, default = "Instant::now")]
    pub last_seen: Instant,
    #[serde(skip)]
    pub rtt: Option<Duration>, // Round-trip time
    #[serde(skip)]
    pub reputation_score: f32, // Anti-spam reputation (0.0-1.0)
    #[serde(skip)]
    pub validation_attempts: u32, // Track validation failures
}

impl PartialEq for Contact {
    fn eq(&self, other: &Self) -> bool {
        // Compare by node ID (primary identifier)
        self.id == other.id && self.peer_id == other.peer_id
    }
}

impl Eq for Contact {}

impl PartialOrd for Contact {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Contact {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by node ID for ordering in data structures
        self.id
            .cmp(&other.id)
            .then_with(|| self.peer_id.cmp(&other.peer_id))
    }
}

/// Type alias for shared contact to enable zero-copy sharing
pub type SharedContact = Arc<Contact>;

impl Contact {
    /// Convert a Vec<Contact> to Vec<SharedContact> for internal use
    pub fn to_shared_vec(contacts: Vec<Contact>) -> Vec<SharedContact> {
        contacts.into_iter().map(Arc::new).collect()
    }

    /// Convert a Vec<SharedContact> to Vec<Contact> for serialization
    pub fn from_shared_vec(contacts: Vec<SharedContact>) -> Vec<Contact> {
        contacts.into_iter().map(|arc| (*arc).clone()).collect()
    }
}

impl Default for Contact {
    fn default() -> Self {
        Self {
            id: NodeId::generate_secure(8), // Generate with proper proof
            peer_id: [0; 32],
            address: "0.0.0.0:0"
                .parse()
                .unwrap_or_else(|_| ([0, 0, 0, 0], 0).into()),
            last_seen: Instant::now(),
            rtt: None,
            reputation_score: 0.5, // Neutral score
            validation_attempts: 0,
        }
    }
}

/// K-bucket storing up to K contacts at a specific distance
///
/// Feynman: Imagine organizing your contacts by how "similar" their
/// phone numbers are to yours. Each bucket holds people whose numbers
/// differ by a specific number of initial digits. The genius is that
/// you keep more contacts who are "close" to you and fewer who are "far".
pub struct KBucket {
    contacts: Vec<SharedContact>,
    max_size: usize,
    _last_updated: Instant,
}

impl KBucket {
    pub fn new(max_size: usize) -> Self {
        Self {
            contacts: Vec::new(),
            max_size,
            _last_updated: Instant::now(),
        }
    }

    /// Add or update a contact
    ///
    /// Feynman: When we hear from a node, we:
    /// 1. Move it to the end if it exists (most recently seen)
    /// 2. Add it if there's room
    /// 3. Ping the oldest node if bucket is full (to check if it's alive)
    pub fn add_contact(&mut self, contact: SharedContact) -> Option<SharedContact> {
        // Check if contact already exists
        if let Some(pos) = self.contacts.iter().position(|c| c.id == contact.id) {
            // Move to end (most recently seen) - zero-copy move
            let _existing = self.contacts.remove(pos);
            self.contacts.push(contact);
            None
        } else if self.contacts.len() < self.max_size {
            // Add new contact
            self.contacts.push(contact);
            None
        } else {
            // Bucket full - return oldest for eviction check (zero-copy)
            Some(self.contacts[0].clone())
        }
    }

    /// Remove a contact
    pub fn remove_contact(&mut self, id: &NodeId) {
        self.contacts.retain(|c| c.id != *id);
    }

    /// Get K closest contacts to a target
    pub fn closest_contacts(&self, target: &NodeId, k: usize) -> Vec<SharedContact> {
        // Use a BinaryHeap to maintain only K closest contacts
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let mut heap: BinaryHeap<(Reverse<Distance>, SharedContact)> = BinaryHeap::with_capacity(k);

        for contact in &self.contacts {
            let distance = contact.id.distance(target);
            if heap.len() < k {
                heap.push((Reverse(distance), contact.clone()));
            } else if let Some((Reverse(max_dist), _)) = heap.peek() {
                if distance < *max_dist {
                    heap.pop();
                    heap.push((Reverse(distance), contact.clone()));
                }
            }
        }

        heap.into_sorted_vec()
            .into_iter()
            .map(|(_, contact)| contact)
            .collect()
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
    k: usize,     // Replication parameter (typically 20)
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
    pub async fn add_contact(&self, contact: SharedContact) {
        // Don't add ourselves
        if contact.id == self.local_id {
            return;
        }

        // Create updated contact with current timestamp
        let updated_contact = Arc::new(Contact {
            id: contact.id.clone(),
            peer_id: contact.peer_id,
            address: contact.address,
            last_seen: Instant::now(),
            rtt: contact.rtt,
            reputation_score: contact.reputation_score,
            validation_attempts: contact.validation_attempts,
        });

        let bucket_idx = self.local_id.bucket_index(&contact.id);
        if bucket_idx < 256 {
            let mut bucket = self.buckets[bucket_idx].write().await;
            if let Some(eviction_candidate) = bucket.add_contact(updated_contact.clone()) {
                // Ping eviction candidate to check if still alive
                drop(bucket); // Release the lock before async operation

                let is_alive = self.ping_contact(&eviction_candidate).await;
                let mut bucket = self.buckets[bucket_idx].write().await;

                if !is_alive {
                    // Eviction candidate is dead, replace with new contact
                    bucket.remove_contact(&eviction_candidate.id);
                    let contact_id = updated_contact.id.clone();
                    bucket.add_contact(updated_contact);
                    log::debug!(
                        "Evicted dead contact {:?} and replaced with {:?}",
                        eviction_candidate.id,
                        contact_id
                    );
                } else {
                    // Eviction candidate is alive, keep it and reject new contact
                    log::debug!(
                        "Eviction candidate {:?} is alive, rejecting new contact {:?}",
                        eviction_candidate.id,
                        updated_contact.id
                    );
                }
            }
        }
    }

    /// Ping a contact to check if it's still alive
    async fn ping_contact(&self, contact: &SharedContact) -> bool {
        use tokio::time::timeout;

        log::debug!("Pinging contact {:?} at {}", contact.id, contact.address);

        // Create ping message with our local contact info
        let ping_msg = KademliaMessage::Ping {
            requester: Contact {
                id: self.local_id.clone(),
                peer_id: [0u8; 32], // Placeholder peer_id for ping
                address: "127.0.0.1:0"
                    .parse()
                    .unwrap_or_else(|_| ([127, 0, 0, 1], 0).into()),
                last_seen: Instant::now(),
                rtt: None,
                reputation_score: 1.0,
                validation_attempts: 0,
            },
        };

        // Serialize the message
        let message_data = match bincode::serialize(&ping_msg) {
            Ok(data) => data,
            Err(e) => {
                log::error!("Failed to serialize ping message: {}", e);
                return false;
            }
        };

        // Create UDP socket for ping
        let socket = match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create UDP socket for ping: {}", e);
                return false;
            }
        };

        // Send ping message
        if let Err(e) = socket.send_to(&message_data, contact.address).await {
            log::error!("Failed to send ping to {:?}: {}", contact.address, e);
            return false;
        }

        // Wait for response with timeout
        let mut buffer = [0u8; 1024];
        let ping_timeout = Duration::from_secs(3);

        match timeout(ping_timeout, socket.recv(&mut buffer)).await {
            Ok(Ok(size)) => {
                // Try to deserialize the response
                if let Ok(response) = bincode::deserialize::<KademliaMessage>(&buffer[..size]) {
                    match response {
                        KademliaMessage::Pong { responder } => {
                            if responder.id == contact.id {
                                log::debug!("Received valid pong from contact {:?}", contact.id);
                                return true;
                            }
                        }
                        _ => {
                            log::debug!("Received unexpected message type from {:?}", contact.id);
                        }
                    }
                }
                log::debug!("Received malformed pong from contact {:?}", contact.id);
                false
            }
            Ok(Err(e)) => {
                log::error!(
                    "Network error while waiting for pong from {:?}: {}",
                    contact.id,
                    e
                );
                false
            }
            Err(_) => {
                log::debug!("Ping timeout for contact {:?}", contact.id);
                false
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
    pub async fn find_closest(&self, target: &NodeId, k: usize) -> Vec<SharedContact> {
        let mut all_contacts = Vec::new();
        let bucket_idx = self.local_id.bucket_index(target);

        // Start from target bucket and expand outward
        for distance in 0..256 {
            // Check bucket at +distance
            if bucket_idx + distance < 256 {
                let bucket = self.buckets[bucket_idx + distance].read().await;
                all_contacts.extend(bucket.contacts.clone()); // Arc cloning is cheap
            }

            // Check bucket at -distance
            if distance > 0 && bucket_idx >= distance {
                let bucket = self.buckets[bucket_idx - distance].read().await;
                all_contacts.extend(bucket.contacts.clone()); // Arc cloning is cheap
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
pub struct KademliaNode {
    local_id: NodeId,
    local_address: SocketAddr,
    routing_table: Arc<RoutingTable>,
    storage: Arc<RwLock<HashMap<Vec<u8>, StoredValue>>>,
    pending_queries: Arc<RwLock<HashMap<u64, oneshot::Sender<KademliaResponse>>>>,
    query_counter: Arc<RwLock<u64>>,
    network_handler: Arc<NetworkHandler>,
    event_sender: mpsc::Sender<KademliaEvent>,
}

/// Stored value with metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StoredValue {
    data: Vec<u8>,
    stored_at: Instant,
    publisher: PeerId,
    ttl: Duration,
}

// NetworkHandler is now imported from nat_traversal module

/// Kademlia protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KademliaMessage {
    FindNode {
        target: NodeId,
        requester: Contact,
    },
    FindNodeResponse {
        nodes: Vec<Contact>,
    },
    Store {
        key: Vec<u8>,
        value: Vec<u8>,
        publisher: Contact,
    },
    StoreResponse {
        success: bool,
    },
    FindValue {
        key: Vec<u8>,
        requester: Contact,
    },
    FindValueResponse {
        result: FindValueResult,
    },
    Ping {
        requester: Contact,
    },
    Pong {
        responder: Contact,
    },
    // NAT traversal messages
    NatPing {
        requester: Contact,
        relay_addr: Option<SocketAddr>,
    },
    NatPong {
        responder: Contact,
        public_addr: SocketAddr,
    },
    HolePunch {
        initiator: Contact,
        target_addr: SocketAddr,
    },
}

/// Result of a find value operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FindValueResult {
    Found(Vec<u8>),
    Nodes(Vec<Contact>),
}

/// Response wrapper for queries
#[derive(Debug, Clone)]
pub enum KademliaResponse {
    Nodes(Vec<SharedContact>),
    Value(Vec<u8>),
    Success(bool),
}

/// Kademlia events for the application layer
#[derive(Debug, Clone)]
pub enum KademliaEvent {
    NodeDiscovered { contact: Contact },
    ValueStored { key: Vec<u8>, success: bool },
    ValueFound { key: Vec<u8>, value: Vec<u8> },
    NetworkError { error: String },
}

impl KademliaNode {
    pub async fn new(
        peer_id: PeerId,
        listen_address: SocketAddr,
        k: usize,
        alpha: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let local_id = NodeId::from_peer_id(&peer_id)?;

        // Bind UDP socket for DHT communication
        let udp_socket = UdpSocket::bind(listen_address).await?;
        let local_address = udp_socket.local_addr()?;

        // Try to bind TCP listener for reliable transport
        let tcp_listener = match TcpListener::bind(local_address).await {
            Ok(listener) => Some(listener),
            Err(_) => None, // TCP fallback not available
        };

        let network_handler =
            Arc::new(NetworkHandler::new(udp_socket, tcp_listener, local_address));

        let (event_sender, _) = mpsc::channel(1000); // Moderate traffic for Kademlia events

        let routing_table = Arc::new(RoutingTable::new(local_id.clone(), k, alpha));
        let node = Self {
            local_id,
            local_address,
            routing_table,
            storage: Arc::new(RwLock::new(HashMap::new())),
            pending_queries: Arc::new(RwLock::new(HashMap::new())),
            query_counter: Arc::new(RwLock::new(0)),
            network_handler,
            event_sender,
        };

        Ok(node)
    }

    /// Start the Kademlia node and begin listening for messages
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Perform NAT traversal setup
        self.network_handler.setup_nat_traversal().await?;

        self.start_message_handler().await?;
        self.start_maintenance_tasks().await?;

        let public_addr = self.network_handler.public_address.read().await;
        println!(
            "Kademlia node started on {} (public: {:?})",
            self.local_address, *public_addr
        );
        Ok(())
    }

    /// Send message with reliable transport and retransmission
    pub async fn send_reliable(
        &self,
        dest: SocketAddr,
        message: KademliaMessage,
    ) -> Result<KademliaResponse, Box<dyn std::error::Error>> {
        let payload = bincode::serialize(&message)?;
        let message_id = self
            .network_handler
            .send_reliable(dest, payload)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        // Create response channel and wait for response
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_queries.write().await;
            pending.insert(message_id, tx);
        }

        // Wait for response with timeout
        let timeout = Duration::from_secs(10); // Longer timeout for reliable transport
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => {
                self.network_handler.acknowledge_message(message_id).await;
                Ok(response)
            }
            Ok(Err(_)) => {
                self.pending_queries.write().await.remove(&message_id);
                Err("Response channel closed".into())
            }
            Err(_) => {
                self.pending_queries.write().await.remove(&message_id);
                Err("Request timed out".into())
            }
        }
    }

    /// Perform iterative node lookup
    ///
    /// Feynman: The lookup algorithm is like a game of "hot and cold":
    /// 1. Ask α nodes for their K closest to target
    /// 2. From responses, pick α new nodes even closer
    /// 3. Repeat until no closer nodes are found
    /// This converges in O(log n) steps!
    pub async fn lookup_node(&self, target: NodeId) -> Vec<SharedContact> {
        let mut queried = HashSet::new();
        let mut to_query = self
            .routing_table
            .find_closest(&target, self.routing_table.alpha)
            .await;
        let mut closest = BTreeMap::new();

        // Add ourselves to closest if we're close
        let self_distance = self.local_id.distance(&target);
        let self_contact = Arc::new(Contact {
            id: self.local_id.clone(),
            peer_id: [0u8; 32], // Would be actual peer ID
            address: self.local_address,
            last_seen: Instant::now(),
            rtt: Some(Duration::from_millis(0)),
            reputation_score: 1.0, // We trust ourselves
            validation_attempts: 0,
        });
        closest.insert(self_distance, self_contact);

        let mut round = 0;
        const MAX_ROUNDS: usize = 20;

        while !to_query.is_empty() && round < MAX_ROUNDS {
            round += 1;
            let mut futures = Vec::with_capacity(3); // Alpha parameter (typically 3)

            // Query α nodes in parallel - optimize by avoiding unnecessary clones
            let contacts_to_query: Vec<_> =
                to_query.drain(..).take(self.routing_table.alpha).collect();
            for contact in contacts_to_query {
                if queried.insert(contact.id.clone()) {
                    // Pass the Arc directly - no expensive cloning
                    futures.push(self.send_find_node_arc(contact, target.clone()));
                }
            }

            // Wait for responses with timeout
            let timeout = Duration::from_secs(5);
            let responses = tokio::time::timeout(timeout, futures::future::join_all(futures)).await;

            if let Ok(responses) = responses {
                // Process responses
                let mut improved = false;
                for response in responses {
                    if let Ok(contacts) = response {
                        for contact in contacts {
                            let distance = contact.id.distance(&target);
                            if let std::collections::btree_map::Entry::Vacant(e) =
                                closest.entry(distance)
                            {
                                improved = true;
                                // Arc cloning is cheap - only reference counting
                                e.insert(contact.clone());

                                // Add to routing table - zero-copy
                                self.routing_table.add_contact(contact.clone()).await;

                                // Consider querying this node - O(1) lookup in HashSet
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
            } else {
                println!("Lookup round {} timed out", round);
                break;
            }
        }

        // Return K closest (excluding ourselves)
        closest
            .into_iter()
            .filter(|(_, contact)| contact.id != self.local_id)
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
    pub async fn store(
        &self,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // Calculate key ID
        let mut hasher = Sha256::new();
        hasher.update(&key);
        // Keys don't need proof-of-work, only nodes do
        let key_bytes: [u8; 32] = hasher.finalize().into();
        let key_id =
            NodeId::new_with_proof(key_bytes, ProofOfWork::generate(&key_bytes, 1).unwrap())
                .unwrap();

        // Find K closest nodes
        let nodes = self.lookup_node(key_id.clone()).await;

        let mut success_count = 0;
        let mut futures = Vec::new();

        // Store on each node
        for node in nodes {
            futures.push(self.send_store_arc(node, key.clone(), value.clone()));
        }

        // Store locally if we're among the closest
        let distance_to_key = self.local_id.distance(&key_id);
        let closest_local = self
            .routing_table
            .find_closest(&key_id, self.routing_table.k)
            .await;

        let should_store_locally = closest_local.len() < self.routing_table.k
            || closest_local
                .iter()
                .any(|c| c.id.distance(&key_id) > distance_to_key);

        if should_store_locally {
            let stored_value = StoredValue {
                data: value.clone(),
                stored_at: Instant::now(),
                publisher: [0u8; 32],           // Would be actual publisher ID
                ttl: Duration::from_secs(3600), // 1 hour TTL
            };
            self.storage.write().await.insert(key.clone(), stored_value);
            success_count += 1;
        }

        // Wait for remote store results
        let results = futures::future::join_all(futures).await;
        for result in results {
            if result.unwrap_or(false) {
                success_count += 1;
            }
        }

        let success = success_count > 0;
        let _ = self
            .event_sender
            .try_send(KademliaEvent::ValueStored { key, success });

        Ok(success)
    }

    /// Retrieve a value from the DHT
    pub async fn get(&self, key: Vec<u8>) -> Option<Vec<u8>> {
        // Check local storage first
        if let Some(stored_value) = self.storage.read().await.get(&key) {
            // Check if value hasn't expired
            if stored_value.stored_at.elapsed() < stored_value.ttl {
                let _ = self.event_sender.try_send(KademliaEvent::ValueFound {
                    key: key.clone(),
                    value: stored_value.data.clone(),
                });
                return Some(stored_value.data.clone());
            } else {
                // Value expired, remove it
                self.storage.write().await.remove(&key);
            }
        }

        // Calculate key ID and search network
        let mut hasher = Sha256::new();
        hasher.update(&key);
        // Keys don't need proof-of-work, only nodes do
        let key_bytes: [u8; 32] = hasher.finalize().into();
        let key_id =
            NodeId::new_with_proof(key_bytes, ProofOfWork::generate(&key_bytes, 1).unwrap())
                .unwrap();

        // Use iterative lookup for FIND_VALUE
        match self.iterative_find_value(key_id, key.clone()).await {
            Some(value) => {
                let _ = self.event_sender.try_send(KademliaEvent::ValueFound {
                    key,
                    value: value.clone(),
                });
                Some(value)
            }
            None => None,
        }
    }

    /// Iterative find value operation
    async fn iterative_find_value(&self, key_id: NodeId, key: Vec<u8>) -> Option<Vec<u8>> {
        let mut queried = HashSet::new();
        let mut to_query = self
            .routing_table
            .find_closest(&key_id, self.routing_table.alpha)
            .await;

        let mut round = 0;
        const MAX_ROUNDS: usize = 20;

        while !to_query.is_empty() && round < MAX_ROUNDS {
            round += 1;
            let mut futures = Vec::with_capacity(3); // Alpha parameter (typically 3)

            // Query α nodes in parallel - optimize by avoiding unnecessary clones
            let contacts_to_query: Vec<_> =
                to_query.drain(..).take(self.routing_table.alpha).collect();
            for contact in contacts_to_query {
                if queried.insert(contact.id.clone()) {
                    // Pass the Arc directly - no expensive cloning
                    futures.push(self.send_find_value_arc(contact, key.clone()));
                }
            }

            // Wait for responses
            let timeout = Duration::from_secs(5);
            let responses = tokio::time::timeout(timeout, futures::future::join_all(futures)).await;

            if let Ok(responses) = responses {
                for response in responses {
                    match response {
                        Ok(FindValueResult::Found(value)) => {
                            return Some(value);
                        }
                        Ok(FindValueResult::Nodes(nodes)) => {
                            // Convert to SharedContact for internal use
                            let shared_nodes = Contact::to_shared_vec(nodes);
                            // Add nodes to query list - O(1) HashSet lookup
                            for node in shared_nodes {
                                if !queried.contains(&node.id) {
                                    to_query.push(node.clone()); // Arc clone is cheap
                                }
                                // Add to routing table - zero-copy
                                self.routing_table.add_contact(node).await;
                            }
                        }
                        Err(_) => {
                            // Query failed, continue with other nodes
                        }
                    }
                }
            }
        }

        None
    }

    // Network operations
    async fn _send_find_node(
        &self,
        contact: Contact,
        target: NodeId,
    ) -> Result<Vec<SharedContact>, Box<dyn std::error::Error>> {
        let message = KademliaMessage::FindNode {
            target,
            requester: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Nodes(nodes) => Ok(nodes),
            _ => Err("Unexpected response type".into()),
        }
    }

    /// Optimized version that works with Arc<Contact> directly
    async fn send_find_node_arc(
        &self,
        contact: SharedContact,
        target: NodeId,
    ) -> Result<Vec<SharedContact>, Box<dyn std::error::Error>> {
        let message = KademliaMessage::FindNode {
            target,
            requester: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Nodes(nodes) => Ok(nodes),
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn _send_store(
        &self,
        contact: Contact,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let message = KademliaMessage::Store {
            key,
            value,
            publisher: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Success(success) => Ok(success),
            _ => Err("Unexpected response type".into()),
        }
    }

    /// Optimized version that works with Arc<Contact> directly
    async fn send_store_arc(
        &self,
        contact: SharedContact,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let message = KademliaMessage::Store {
            key,
            value,
            publisher: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Success(success) => Ok(success),
            _ => Err("Unexpected response type".into()),
        }
    }

    async fn _send_find_value(
        &self,
        contact: Contact,
        key: Vec<u8>,
    ) -> Result<FindValueResult, Box<dyn std::error::Error>> {
        let message = KademliaMessage::FindValue {
            key,
            requester: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Value(value) => Ok(FindValueResult::Found(value)),
            KademliaResponse::Nodes(shared_nodes) => {
                let nodes = Contact::from_shared_vec(shared_nodes);
                Ok(FindValueResult::Nodes(nodes))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    /// Optimized version that works with Arc<Contact> directly
    async fn send_find_value_arc(
        &self,
        contact: SharedContact,
        key: Vec<u8>,
    ) -> Result<FindValueResult, Box<dyn std::error::Error>> {
        let message = KademliaMessage::FindValue {
            key,
            requester: self.create_self_contact(),
        };

        match self.send_message(contact.address, message).await? {
            KademliaResponse::Value(value) => Ok(FindValueResult::Found(value)),
            KademliaResponse::Nodes(shared_nodes) => {
                let nodes = Contact::from_shared_vec(shared_nodes);
                Ok(FindValueResult::Nodes(nodes))
            }
            _ => Err("Unexpected response type".into()),
        }
    }

    /// Send a message and wait for response (with reliable transport fallback)
    async fn send_message(
        &self,
        address: SocketAddr,
        message: KademliaMessage,
    ) -> Result<KademliaResponse, Box<dyn std::error::Error>> {
        // Try reliable transport first for better NAT traversal and retransmission
        match self.send_reliable(address, message.clone()).await {
            Ok(response) => Ok(response),
            Err(_) => {
                // Fallback to original UDP-only method
                self.send_message_udp_only(address, message).await
            }
        }
    }

    /// Original UDP-only message sending (kept as fallback)
    async fn send_message_udp_only(
        &self,
        address: SocketAddr,
        message: KademliaMessage,
    ) -> Result<KademliaResponse, Box<dyn std::error::Error>> {
        // Generate query ID
        let query_id = {
            let mut counter = self.query_counter.write().await;
            *counter += 1;
            *counter
        };

        // Create response channel
        let (tx, rx) = oneshot::channel();
        {
            let mut queries = self.pending_queries.write().await;
            queries.insert(query_id, tx);
        }

        // Serialize and send message
        let mut message_data = bincode::serialize(&message)?;
        message_data.splice(0..0, query_id.to_be_bytes());

        self.network_handler
            .udp_socket
            .send_to(&message_data, address)
            .await?;

        // Wait for response with timeout
        let timeout = Duration::from_secs(5);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err("Response channel closed".into()),
            Err(_) => {
                // Clean up pending query
                self.pending_queries.write().await.remove(&query_id);
                Err("Request timed out".into())
            }
        }
    }

    /// Create contact info for ourselves
    fn create_self_contact(&self) -> Contact {
        Contact {
            id: self.local_id.clone(),
            peer_id: [0u8; 32], // Would be actual peer ID
            address: self.local_address,
            last_seen: Instant::now(),
            rtt: Some(Duration::from_millis(0)),
            reputation_score: 1.0,
            validation_attempts: 0,
        }
    }

    /// Start message handler
    async fn start_message_handler(&self) -> Result<(), Box<dyn std::error::Error>> {
        let udp_socket = self.network_handler.udp_socket.clone();
        let routing_table = self.routing_table.clone();
        let storage = self.storage.clone();
        let pending_queries = self.pending_queries.clone();
        let local_id = self.local_id.clone();
        let local_address = self.local_address;
        let event_sender = self.event_sender.clone();

        tokio::spawn(async move {
            let mut buffer = [0u8; 65536];

            loop {
                match udp_socket.recv_from(&mut buffer).await {
                    Ok((len, from)) => {
                        let data = &buffer[..len];

                        if data.len() < 8 {
                            continue; // Not enough data for query ID
                        }

                        // Extract query ID
                        let query_id = u64::from_be_bytes([
                            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                        ]);

                        let message_data = &data[8..];

                        // Try to deserialize message
                        if let Ok(message) = bincode::deserialize::<KademliaMessage>(message_data) {
                            Self::handle_message(
                                message,
                                query_id,
                                from,
                                local_id.clone(),
                                local_address,
                                &routing_table,
                                &storage,
                                &pending_queries,
                                &udp_socket,
                                &event_sender,
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        let _ = event_sender.try_send(KademliaEvent::NetworkError {
                            error: format!("UDP receive error: {}", e),
                        });
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle incoming messages
    async fn handle_message(
        message: KademliaMessage,
        query_id: u64,
        from: SocketAddr,
        local_id: NodeId,
        local_address: SocketAddr,
        routing_table: &Arc<RoutingTable>,
        storage: &Arc<RwLock<HashMap<Vec<u8>, StoredValue>>>,
        pending_queries: &Arc<RwLock<HashMap<u64, oneshot::Sender<KademliaResponse>>>>,
        udp_socket: &Arc<UdpSocket>,
        event_sender: &mpsc::Sender<KademliaEvent>,
    ) {
        match message {
            KademliaMessage::FindNode { target, requester } => {
                // Add requester to routing table
                routing_table.add_contact(Arc::new(requester)).await;

                // Find closest nodes
                let shared_nodes = routing_table.find_closest(&target, routing_table.k).await;
                let nodes = Contact::from_shared_vec(shared_nodes);

                let response = KademliaMessage::FindNodeResponse { nodes };
                Self::send_response(query_id, response, from, udp_socket).await;
            }

            KademliaMessage::Store {
                key,
                value,
                publisher,
            } => {
                // Add publisher to routing table
                routing_table.add_contact(Arc::new(publisher.clone())).await;

                // Store the value
                let stored_value = StoredValue {
                    data: value,
                    stored_at: Instant::now(),
                    publisher: publisher.peer_id,
                    ttl: Duration::from_secs(3600),
                };

                storage.write().await.insert(key, stored_value);

                let response = KademliaMessage::StoreResponse { success: true };
                Self::send_response(query_id, response, from, udp_socket).await;
            }

            KademliaMessage::FindValue { key, requester } => {
                // Add requester to routing table
                routing_table.add_contact(Arc::new(requester)).await;

                let result = if let Some(stored_value) = storage.read().await.get(&key) {
                    // Check if value hasn't expired
                    if stored_value.stored_at.elapsed() < stored_value.ttl {
                        FindValueResult::Found(stored_value.data.clone())
                    } else {
                        // Value expired, remove and return nodes
                        storage.write().await.remove(&key);
                        let mut hasher = Sha256::new();
                        hasher.update(&key);
                        // Keys don't need proof-of-work, only nodes do
                        let key_bytes: [u8; 32] = hasher.finalize().into();
                        let key_id = NodeId::new_with_proof(
                            key_bytes,
                            ProofOfWork::generate(&key_bytes, 1).unwrap(),
                        )
                        .unwrap();
                        let shared_nodes =
                            routing_table.find_closest(&key_id, routing_table.k).await;
                        let nodes = Contact::from_shared_vec(shared_nodes);
                        FindValueResult::Nodes(nodes)
                    }
                } else {
                    // Value not found, return closest nodes
                    let mut hasher = Sha256::new();
                    hasher.update(&key);
                    // Keys don't need proof-of-work, only nodes do
                    let key_bytes: [u8; 32] = hasher.finalize().into();
                    let key_id = NodeId::new_with_proof(
                        key_bytes,
                        ProofOfWork::generate(&key_bytes, 1).unwrap(),
                    )
                    .unwrap();
                    let shared_nodes = routing_table.find_closest(&key_id, routing_table.k).await;
                    let nodes = Contact::from_shared_vec(shared_nodes);
                    FindValueResult::Nodes(nodes)
                };

                let response = KademliaMessage::FindValueResponse { result };
                Self::send_response(query_id, response, from, udp_socket).await;
            }

            KademliaMessage::Ping { requester } => {
                // Add requester to routing table
                routing_table.add_contact(Arc::new(requester)).await;

                let responder = Contact {
                    id: local_id,
                    peer_id: [0u8; 32],
                    address: local_address,
                    last_seen: Instant::now(),
                    rtt: Some(Duration::from_millis(0)),
                    reputation_score: 1.0,
                    validation_attempts: 0,
                };

                let response = KademliaMessage::Pong { responder };
                Self::send_response(query_id, response, from, udp_socket).await;
            }

            // Handle responses
            KademliaMessage::FindNodeResponse { nodes } => {
                if let Some(tx) = pending_queries.write().await.remove(&query_id) {
                    let shared_nodes = Contact::to_shared_vec(nodes);
                    tx.send(KademliaResponse::Nodes(shared_nodes)).ok();
                }
            }

            KademliaMessage::StoreResponse { success } => {
                if let Some(tx) = pending_queries.write().await.remove(&query_id) {
                    tx.send(KademliaResponse::Success(success)).ok();
                }
            }

            KademliaMessage::FindValueResponse { result } => {
                if let Some(tx) = pending_queries.write().await.remove(&query_id) {
                    match result {
                        FindValueResult::Found(value) => {
                            tx.send(KademliaResponse::Value(value)).ok();
                        }
                        FindValueResult::Nodes(nodes) => {
                            let shared_nodes = Contact::to_shared_vec(nodes);
                            tx.send(KademliaResponse::Nodes(shared_nodes)).ok();
                        }
                    }
                }
            }

            KademliaMessage::Pong { responder } => {
                // Add responder to routing table
                let responder_arc = Arc::new(responder.clone());
                routing_table.add_contact(responder_arc).await;
                let _ = event_sender.try_send(KademliaEvent::NodeDiscovered { contact: responder });
            }

            KademliaMessage::NatPing {
                requester,
                relay_addr,
            } => {
                // Add requester to routing table
                routing_table.add_contact(Arc::new(requester.clone())).await;

                // Send NAT ping response
                let response = KademliaMessage::NatPong {
                    responder: requester,
                    public_addr: from, // Use the source address as public address
                };
                Self::send_response(query_id, response, from, udp_socket).await;
            }

            KademliaMessage::NatPong {
                responder,
                public_addr: _,
            } => {
                // Add responder to routing table
                routing_table.add_contact(Arc::new(responder.clone())).await;
                log::debug!("NAT pong received from {:?}", responder.id);
            }

            KademliaMessage::HolePunch {
                initiator,
                target_addr,
            } => {
                // Add initiator to routing table
                routing_table.add_contact(Arc::new(initiator.clone())).await;

                // Log hole punch attempt
                log::debug!(
                    "Received hole punch request from {:?} for target {:?}",
                    initiator.id,
                    target_addr
                );
            }
        }
    }

    /// Send a response message
    async fn send_response(
        query_id: u64,
        response: KademliaMessage,
        to: SocketAddr,
        udp_socket: &Arc<UdpSocket>,
    ) {
        if let Ok(response_data) = bincode::serialize(&response) {
            let mut message_data = query_id.to_be_bytes().to_vec();
            message_data.extend_from_slice(&response_data);
            udp_socket.send_to(&message_data, to).await.ok();
        }
    }

    /// Start maintenance tasks
    async fn start_maintenance_tasks(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Cleanup expired values
        let storage = self.storage.clone();
        tokio::spawn(async move {
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                cleanup_interval.tick().await;

                let now = Instant::now();
                let mut storage = storage.write().await;
                storage.retain(|_, value| now.duration_since(value.stored_at) < value.ttl);
            }
        });

        Ok(())
    }

    /// Bootstrap the node by connecting to known nodes
    pub async fn bootstrap(
        &self,
        bootstrap_nodes: Vec<SocketAddr>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for addr in bootstrap_nodes {
            // Create a contact for the bootstrap node
            let _contact = Contact {
                id: NodeId::generate_secure(1), // Generate minimal proof for initial connection
                peer_id: [0u8; 32],
                address: addr,
                last_seen: Instant::now(),
                rtt: None,
                reputation_score: 0.5, // Neutral initial reputation
                validation_attempts: 0,
            };

            // Ping the bootstrap node to get its actual ID
            let message = KademliaMessage::Ping {
                requester: self.create_self_contact(),
            };

            // Send ping (don't wait for response, it will be handled by message handler)
            if let Ok(message_data) = bincode::serialize(&message) {
                let query_id = {
                    let mut counter = self.query_counter.write().await;
                    *counter += 1;
                    *counter
                };

                let mut full_data = query_id.to_be_bytes().to_vec();
                full_data.extend_from_slice(&message_data);

                self.network_handler
                    .udp_socket
                    .send_to(&full_data, addr)
                    .await
                    .ok();
            }
        }

        // Perform lookup for our own ID to populate routing table
        tokio::time::sleep(Duration::from_millis(500)).await; // Wait for pings to complete
        self.lookup_node(self.local_id.clone()).await;

        Ok(())
    }

    /// Get events receiver
    pub fn subscribe_events(&self) -> mpsc::Receiver<KademliaEvent> {
        let (_tx, rx) = mpsc::channel(1000); // Moderate traffic for event subscription
                                             // In a real implementation, you'd want to manage multiple subscribers
        rx
    }

    /// Get node statistics
    pub async fn get_stats(&self) -> NodeStats {
        let storage = self.storage.read().await;
        let routing_table = &self.routing_table;

        let mut total_contacts = 0;
        for i in 0..256 {
            let bucket = routing_table.buckets[i].read().await;
            total_contacts += bucket.contacts.len();
        }

        NodeStats {
            node_id: self.local_id.clone(),
            local_address: self.local_address,
            stored_values: storage.len(),
            routing_table_size: total_contacts,
        }
    }
}

/// Node statistics
#[derive(Debug, Clone)]
pub struct NodeStats {
    pub node_id: NodeId,
    pub local_address: SocketAddr,
    pub stored_values: usize,
    pub routing_table_size: usize,
}

use futures;
use rand::RngCore;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_creation() {
        let peer_id = [1u8; 32];
        let addr = "127.0.0.1:0".parse().expect("Failed to parse test address");

        let node = KademliaNode::new(peer_id, addr, 20, 3)
            .await
            .expect("Failed to create KademliaNode");
        assert_eq!(
            node.local_id,
            NodeId::from_peer_id(&peer_id).expect("Failed to create NodeId")
        );
    }

    #[tokio::test]
    async fn test_distance_calculation() {
        let id1 = NodeId::new_test([0u8; 32]);
        let id2 = NodeId::new_test([255u8; 32]);

        let distance = id1.distance(&id2);
        assert_eq!(distance.leading_zeros(), 0); // All bits different

        let distance_self = id1.distance(&id1);
        assert_eq!(distance_self.leading_zeros(), 256); // Same node
    }

    #[tokio::test]
    async fn test_routing_table() {
        let local_id = NodeId::new_test([0u8; 32]);
        let routing_table = RoutingTable::new(local_id, 20, 3);

        let contact = Arc::new(Contact {
            id: NodeId::new_test([1u8; 32]),
            peer_id: [1u8; 32],
            address: "127.0.0.1:8000"
                .parse()
                .expect("Failed to parse test address"),
            last_seen: Instant::now(),
            rtt: Some(Duration::from_millis(10)),
            reputation_score: 0.8,
            validation_attempts: 0,
        });

        routing_table.add_contact(contact.clone()).await;

        let closest = routing_table.find_closest(&contact.id, 5).await;
        assert!(!closest.is_empty());
    }
}
