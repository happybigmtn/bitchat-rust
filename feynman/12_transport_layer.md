# Chapter 12: Transport Layer - Connecting the Distributed Casino
## Understanding `src/transport/mod.rs`

*"The network is the computer."* - John Gage, Sun Microsystems

*"In a distributed system, the transport layer is the blood vessels - it must reliably carry messages between the organs, or the whole body fails."* - Network Systems Engineer

---

## Part I: Network Transport for Complete Beginners
### A 500+ Line Journey from "How Do Computers Talk?" to "Mesh Network Communication"

Let me begin with a story that illustrates why transport layers matter.

In 1962, J.C.R. Licklider wrote a series of memos describing an "Intergalactic Computer Network." His vision was remarkable: computers around the world would be interconnected, sharing resources and information seamlessly. But there was a fundamental problem - how do you make different computers, using different protocols, on different networks, communicate reliably?

The answer became the transport layer, and it's one of the most elegant solutions in computer science. But to understand why our casino needs sophisticated transport management, we need to start with the basics of how computers communicate.

### What Is Network Communication, Really?

Imagine you're trying to have a conversation with someone in another building. You could:

1. **Shout**: Loud, but limited range and everyone hears you
2. **Send a letter**: Reliable, but slow and requires infrastructure  
3. **Use a telephone**: Fast, direct, but needs continuous connection
4. **Use radio**: Flexible range, but can have interference

Computer networks face the same trade-offs. Different transport mechanisms have different characteristics, and choosing the right one is crucial.

### The Fundamental Challenges of Network Transport

#### Challenge 1: Physical Connection - How Do Signals Travel?

**Wired Connections**:
- **Ethernet**: Electrical signals over copper wire
- **Fiber Optic**: Light pulses through glass cables
- **USB**: Digital data over standardized connectors

**Wireless Connections**:
- **WiFi**: Radio waves in 2.4GHz and 5GHz bands
- **Bluetooth**: Short-range radio in 2.4GHz band
- **Cellular**: Long-range radio with tower infrastructure

Each has different properties:
```
Medium      | Range    | Speed      | Power  | Cost
------------|----------|------------|--------|--------
Ethernet    | 100m     | 1000 Mbps  | None   | Low
WiFi        | 50m      | 300 Mbps   | Medium | Low  
Bluetooth   | 10m      | 2 Mbps     | Low    | Low
Cellular    | 10km+    | 100 Mbps   | High   | High
```

#### Challenge 2: Addressing - Who Am I Talking To?

**MAC Addresses**: Hardware-level identification
```
00:1B:44:11:3A:B7  (48 bits, unique per network card)
```

**IP Addresses**: Network-level routing
```
IPv4: 192.168.1.100    (32 bits, ~4 billion addresses)
IPv6: 2001:db8::1      (128 bits, ~340 undecillion addresses)
```

**Port Numbers**: Application-level multiplexing
```
HTTP: Port 80
HTTPS: Port 443  
BitCraps: Port 8765 (custom)
```

**Our Challenge**: In a P2P casino, there's no central authority to assign addresses. We use cryptographic identities (public keys) as addresses!

#### Challenge 3: Reliability - Did My Message Arrive?

Networks are unreliable by nature:

**Packet Loss**:
```
Alice sends: [1][2][3][4][5]
Network:     [1][X][3][X][5]  (packets 2 and 4 lost)
Bob receives: [1][3][5]       (missing data!)
```

**Duplication**:
```
Alice sends: [1][2][3]
Network bug: [1][2][2][3]  (packet 2 duplicated)
Bob receives: [1][2][2][3] (duplicate data!)
```

**Reordering**:
```
Alice sends: [1][2][3]
Bob receives:[3][1][2]  (out of order!)
```

**Solutions**:
- **Acknowledgments**: "I got packet 2"
- **Sequence Numbers**: Order detection
- **Timeouts**: Resend if no ACK
- **Checksums**: Detect corruption

#### Challenge 4: Flow Control - Don't Overwhelm the Receiver

```
Fast sender:    [MSG][MSG][MSG][MSG][MSG]... (1000 msgs/sec)
Slow receiver:  [MSG].......[MSG]......... (10 msgs/sec)

Problem: Receiver buffer overflows!
```

**Solutions**:
- **Stop-and-Wait**: Send one, wait for ACK, send next
- **Sliding Window**: Send N packets before requiring ACK
- **Rate Limiting**: Limit sending speed

#### Challenge 5: Congestion Control - Don't Overwhelm the Network

```
     [A]
      |\
      | \
      |  \[Bottleneck: 1 Mbps]
      |  /
      | /
      |/
     [B]

If A sends 10 Mbps, the network breaks!
```

**Solutions**:
- **Slow Start**: Begin slowly, increase gradually
- **Congestion Avoidance**: Back off when packets are lost
- **Fair Queuing**: Give each flow equal bandwidth

### The OSI Model and Transport Layer

The OSI (Open Systems Interconnection) model provides a framework for understanding network communication:

```
Layer 7: Application  (HTTP, FTP, BitCraps Protocol)
Layer 6: Presentation (Encryption, Compression)
Layer 5: Session      (Connection Management)
Layer 4: Transport    (TCP, UDP)  ← WE ARE HERE
Layer 3: Network      (IP Routing)
Layer 2: Data Link    (Ethernet, WiFi)
Layer 1: Physical     (Cables, Radio)
```

**Transport Layer Responsibilities**:
1. **End-to-End Communication**: Connect applications across networks
2. **Reliability**: Ensure data arrives correctly
3. **Flow Control**: Match sender and receiver speeds
4. **Multiplexing**: Multiple applications on one machine
5. **Error Detection**: Find and correct transmission errors

### TCP vs UDP: The Fundamental Choice

#### TCP (Transmission Control Protocol) - Reliable Stream

**Characteristics**:
- **Connection-oriented**: Establish connection before sending data
- **Reliable**: Guarantees delivery and ordering
- **Flow control**: Prevents overwhelming receiver
- **Congestion control**: Adapts to network conditions

**TCP Connection Lifecycle**:
```
Client                    Server
  |                         |
  |------- SYN ----------->|  (1. Request connection)
  |<--- SYN-ACK ----------|  (2. Acknowledge + respond)
  |------- ACK ----------->|  (3. Acknowledge server)
  |                         |
  |====== DATA ==========>|  (4. Reliable data transfer)
  |<===== DATA ==========|
  |                         |
  |------- FIN ----------->|  (5. Close connection)
  |<--- FIN-ACK ----------|
  |------- ACK ----------->|
```

**When to Use TCP**:
- File transfers (every byte matters)
- Web browsing (HTML must be complete)
- Email (messages can't be corrupted)
- Database connections (ACID properties)

#### UDP (User Datagram Protocol) - Fast Datagrams

**Characteristics**:
- **Connectionless**: Just send packets
- **Unreliable**: No delivery guarantees
- **Fast**: Minimal overhead
- **Simple**: Fire-and-forget

**UDP Message Format**:
```
[Source Port: 16 bits][Dest Port: 16 bits]
[Length: 16 bits]     [Checksum: 16 bits]
[Data: Variable length]
```

**When to Use UDP**:
- Video streaming (old frames don't matter)
- Online gaming (speed over reliability)
- DNS lookups (retry is cheaper than connection overhead)
- Real-time data (latest value matters, not all values)

### Specialized Transport Protocols

#### QUIC (Quick UDP Internet Connections)
Modern protocol combining TCP reliability with UDP speed:

**Features**:
- Built-in encryption (TLS 1.3)
- Multiplexing without head-of-line blocking
- Fast connection establishment (0-RTT)
- Connection migration (switch networks seamlessly)

**Used by**: Google, Cloudflare, Facebook

#### SCTP (Stream Control Transmission Protocol)
Multi-streaming protocol:

**Features**:
- Multiple streams in one connection
- Partial reliability options
- Multi-homing (multiple IP addresses)
- Message-oriented (not byte stream)

**Used by**: Telecom signaling, WebRTC

#### WebRTC Transport
Peer-to-peer communication for browsers:

**Components**:
- **ICE**: Interactive Connectivity Establishment
- **STUN**: Session Traversal Utilities for NAT
- **TURN**: Traversal Using Relays around NAT
- **DTLS**: Datagram Transport Layer Security

### Bluetooth as a Transport Medium

Bluetooth is particularly interesting for our P2P casino because it enables direct device-to-device communication without infrastructure.

#### Bluetooth Protocol Stack

```
Application Layer    (BitCraps Protocol)
       ↓
L2CAP               (Logical Link Control)
       ↓  
HCI                 (Host Controller Interface)
       ↓
Link Manager        (Connection Management)
       ↓
Baseband           (Timing, Frequency Hopping)
       ↓
Radio              (2.4 GHz ISM Band)
```

#### Bluetooth Low Energy (BLE)

**Key Differences from Classic Bluetooth**:
- **Power**: 10-100x less power consumption
- **Range**: Similar (10m) or better with BLE 5.0
- **Speed**: Slower (1 Mbps vs 3 Mbps)
- **Connection**: Faster to connect/disconnect
- **Cost**: Lower cost chips

**BLE Roles**:
- **Central**: Initiates connections (like a phone)
- **Peripheral**: Advertises services (like a fitness tracker)
- **Observer**: Listens to advertisements
- **Broadcaster**: Sends advertisements

**BLE Communication Pattern**:
```
1. Peripheral advertises: "I'm a BitCraps casino node!"
2. Central scans and discovers: "Found casino node!"
3. Central connects to peripheral
4. Central subscribes to characteristics
5. Data exchange begins
```

### Mesh Networking Fundamentals

Traditional networks are hub-and-spoke:
```
     [Device A]
         |
    [Router/Hub]
         |
     [Device B]
```

Mesh networks are peer-to-peer:
```
[A] ←→ [B] ←→ [C]
 ↑       ↑     ↑
 ↓       ↓     ↓
[D] ←→ [E] ←→ [F]
```

**Mesh Advantages**:
- **Resilience**: Multiple paths between nodes
- **Self-healing**: Route around failures
- **Range extension**: Multi-hop communication
- **Decentralized**: No single point of failure

**Mesh Challenges**:
- **Routing complexity**: How to find best path?
- **Flooding**: How to prevent message loops?
- **Scalability**: Performance degrades with size
- **Power**: More radio time = more battery drain

#### Routing Algorithms

**Flood Routing** (Simple but inefficient):
```rust
fn flood_message(message: Message, sender: PeerId) {
    for peer in connected_peers() {
        if peer != sender {  // Don't send back to sender
            send_to_peer(peer, message.clone());
        }
    }
}
```

**Distance Vector** (Share routing tables):
```
Node A knows:
  To B: 1 hop via B
  To C: 2 hops via B  
  To D: 3 hops via B
```

**Link State** (Global topology knowledge):
```
Each node knows complete network graph
Calculates shortest paths using Dijkstra's algorithm
```

**Geographic Routing** (Position-based):
```
Forward packet to node closest to destination
Requires GPS/positioning information
```

### Network Address Translation (NAT) and Firewalls

Most devices today are behind NAT, which complicates P2P communication:

**NAT Translation**:
```
Internal Network:      External Network:
[A: 192.168.1.100] ←→ [NAT: 203.0.113.1] ←→ Internet
[B: 192.168.1.101]         (Port mapping)
```

**NAT Types**:
1. **Full Cone**: External → Internal freely
2. **Restricted Cone**: Only after internal → external
3. **Port Restricted**: Must match both IP and port
4. **Symmetric**: Different external port per destination

**NAT Traversal Techniques**:
- **UPnP**: Automatic port forwarding
- **STUN**: Discover external IP/port
- **TURN**: Relay server for hopeless cases
- **ICE**: Try multiple methods

### Transport Security Considerations

#### Man-in-the-Middle Attacks
```
Alice thinks she's talking to Bob:
Alice ←→ [Attacker] ←→ Bob

Attacker can:
- Read all messages
- Modify messages  
- Inject fake messages
```

**Defenses**:
- **TLS/SSL**: Encrypted channels
- **Certificate Validation**: Verify identity
- **Public Key Pinning**: Remember first key

#### Denial of Service (DoS) Attacks

**SYN Flood** (TCP vulnerability):
```
Attacker → [SYN] → Server
           [SYN] → Server (hundreds per second)
           [SYN] → Server

Server runs out of connection slots
```

**UDP Flood**:
```
Attacker sends massive UDP packets to overwhelm bandwidth
```

**Amplification Attacks**:
```
Attacker: Small request to DNS server
DNS Server: Large response to victim
Result: Attacker gets 50x amplification
```

**Defenses**:
- **Rate Limiting**: Max connections per IP
- **SYN Cookies**: Stateless connection tracking
- **Traffic Shaping**: Limit bandwidth per user
- **Blacklisting**: Block known bad actors

### Quality of Service (QoS)

Not all network traffic is equal:

**Traffic Classifications**:
1. **Interactive** (gaming, VoIP): Low latency critical
2. **Streaming** (video): Consistent bandwidth needed
3. **Bulk** (file transfer): Can tolerate delays
4. **Background** (backups): Use leftover bandwidth

**QoS Mechanisms**:
- **Traffic Shaping**: Limit bandwidth
- **Priority Queuing**: High priority goes first
- **Weighted Fair Queuing**: Proportional sharing
- **Red/WRED**: Drop packets before congestion

### Transport Layer Protocols in Practice

#### HTTP/1.1 over TCP
```
GET /casino/status HTTP/1.1
Host: casino.example.com
Connection: keep-alive

HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 123

{"players": 42, "active_games": 7}
```

**Problems**:
- **Head-of-line blocking**: One slow request blocks others
- **Connection overhead**: New TCP connection for each request
- **Text-based**: Verbose headers

#### HTTP/2 over TCP
```
Binary framing, multiplexing:
Stream 1: GET /casino/status
Stream 2: GET /casino/games  
Stream 3: POST /casino/bet

All over single TCP connection
```

**Improvements**:
- **Multiplexing**: Multiple requests simultaneously
- **Server Push**: Server can send resources proactively
- **Header Compression**: HPACK reduces overhead

#### HTTP/3 over QUIC
```
Built on UDP with QUIC:
- Multiple streams with independent flow control
- Built-in encryption
- 0-RTT connection establishment
```

### The BitCraps Transport Challenge

Our distributed casino has unique requirements:

1. **No Infrastructure**: Can't rely on servers or internet
2. **Mobile Devices**: Battery life is critical
3. **Real-time Gaming**: Low latency for dice rolls
4. **Security**: Money requires authenticated transport
5. **Resilience**: Must work with node failures
6. **Range**: BLE limits us to ~10m per hop

**Our Solution Strategy**:
- **BLE Mesh**: Direct device-to-device communication
- **Redundant Paths**: Multiple routes for reliability
- **Connection Pooling**: Reuse expensive BLE connections
- **Rate Limiting**: Prevent DoS attacks
- **Graceful Degradation**: Reduce functionality rather than fail

### Modern Transport Innovations

#### QUIC Evolution
```
QUIC v1 (2021): HTTP/3 foundation
QUIC v2 (draft): Improved performance, new features
```

#### WebTransport
```
Bidirectional communication for web applications
Built on HTTP/3 and QUIC
Alternative to WebSockets
```

#### BBR Congestion Control
```
Bandwidth and round-trip propagation time (BBR)
Used by Google, YouTube
Better performance than traditional TCP
```

---

## Part II: The Code - Complete Walkthrough

Now let's examine how BitCraps implements these transport concepts in real Rust code, creating a robust mesh networking foundation for our casino.

### Transport Architecture Overview

BitCraps implements a sophisticated transport layer that coordinates multiple communication mechanisms:

```rust
// Lines 96-107
pub struct TransportCoordinator {
    bluetooth: Option<Arc<RwLock<BluetoothTransport>>>,            // Basic BLE
    enhanced_bluetooth: Option<Arc<RwLock<EnhancedBluetoothTransport>>>, // Advanced BLE
    connections: Arc<RwLock<HashMap<PeerId, TransportAddress>>>,   // Active connections
    connection_counts_per_address: Arc<RwLock<HashMap<TransportAddress, usize>>>, // Rate limiting
    connection_attempts: Arc<RwLock<Vec<ConnectionAttempt>>>,      // DoS protection
    connection_limits: ConnectionLimits,                          // Configurable limits
    event_sender: mpsc::UnboundedSender<TransportEvent>,          // Event notifications
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<TransportEvent>>>, // Event handling
}
```

This architecture provides:
- **Multiple Transport Support**: BLE, TCP, UDP (extensible)
- **Connection Management**: Track and limit connections
- **DoS Protection**: Rate limiting and cooldowns
- **Event System**: Asynchronous notifications
- **Thread Safety**: Arc<RwLock> for concurrent access

### Transport Address Abstraction

```rust
// Lines 47-54
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransportAddress {
    Tcp(SocketAddr),      // TCP connection (for testing/development)
    Udp(SocketAddr),      // UDP connection (for testing/development)  
    Bluetooth(String),    // Bluetooth device ID/address
    Mesh(PeerId),        // Abstract mesh routing via peer ID
}
```

**Why Use an Enum?**

1. **Type Safety**: Can't accidentally pass TCP address to Bluetooth function
2. **Pattern Matching**: Compiler ensures all cases are handled
3. **Future Extensibility**: Easy to add new transport types
4. **Serialization**: Can store/transmit address information

**Usage Example**:
```rust
match address {
    TransportAddress::Bluetooth(device_id) => {
        // Connect via BLE
        bluetooth.connect_to_device(&device_id).await?;
    }
    TransportAddress::Tcp(socket_addr) => {
        // Connect via TCP
        TcpStream::connect(socket_addr).await?;
    }
    TransportAddress::Mesh(peer_id) => {
        // Route through mesh network
        mesh_router.route_to_peer(peer_id, packet).await?;
    }
    _ => return Err("Unsupported transport"),
}
```

### Transport Events System

```rust
// Lines 56-63
#[derive(Debug, Clone)]
pub enum TransportEvent {
    Connected { peer_id: PeerId, address: TransportAddress },
    Disconnected { peer_id: PeerId, reason: String },
    DataReceived { peer_id: PeerId, data: Vec<u8> },
    Error { peer_id: Option<PeerId>, error: String },
}
```

**Event-Driven Architecture Benefits**:
- **Decoupling**: Transport layer doesn't need to know about application logic
- **Asynchronous**: Non-blocking event processing
- **Monitoring**: Easy to log and track network events
- **Resilience**: Can handle events even if some components fail

**Event Flow**:
```
[Bluetooth Hardware] → [Transport Layer] → [TransportEvent] → [Application]
                                             ↓
                                        [Event Handler]
                                             ↓
                                    [Update Game State]
```

### DoS Protection and Rate Limiting

```rust
// Lines 66-87
#[derive(Debug, Clone)]
pub struct ConnectionLimits {
    /// Maximum total connections allowed
    pub max_total_connections: usize,
    /// Maximum connections per peer address  
    pub max_connections_per_peer: usize,
    /// Rate limit: max new connections per time window
    pub max_new_connections_per_minute: usize,
    /// Connection attempt cooldown period
    pub connection_cooldown: Duration,
}

impl Default for ConnectionLimits {
    fn default() -> Self {
        Self {
            max_total_connections: 100,        // Total casino capacity
            max_connections_per_peer: 3,       // Prevent single peer from monopolizing
            max_new_connections_per_minute: 10, // Rate limiting
            connection_cooldown: Duration::from_secs(60), // Cooldown period
        }
    }
}
```

**Why These Limits Matter**:

1. **Resource Protection**: BLE connections consume memory and radio time
2. **Fair Usage**: Prevent one peer from using all connections
3. **DoS Mitigation**: Limit connection floods
4. **Battery Life**: Too many connections drain battery

### Connection Attempt Tracking

```rust
// Lines 89-94
#[derive(Debug, Clone)]
struct ConnectionAttempt {
    timestamp: Instant,
    peer_address: TransportAddress,
}
```

**Connection Limit Checking Algorithm**:

```rust
// Lines 157-213
async fn check_connection_limits(&self, address: &TransportAddress) -> Result<()> {
    // Check total connection limit
    let connections = self.connections.read().await;
    if connections.len() >= self.connection_limits.max_total_connections {
        return Err(Error::Network(format!(
            "Connection rejected: Maximum total connections ({}) exceeded",
            self.connection_limits.max_total_connections
        )));
    }
    
    // Check per-peer connection limit
    let connection_counts = self.connection_counts_per_address.read().await;
    if let Some(&count) = connection_counts.get(address) {
        if count >= self.connection_limits.max_connections_per_peer {
            return Err(Error::Network(format!(
                "Connection rejected: Maximum connections per peer ({}) exceeded for {:?}",
                self.connection_limits.max_connections_per_peer, address
            )));
        }
    }
    
    // Check rate limiting
    let now = Instant::now();
    let rate_limit_window = Duration::from_secs(60); // 1 minute window
    let attempts = self.connection_attempts.read().await;
    
    let recent_attempts = attempts
        .iter()
        .filter(|attempt| now.duration_since(attempt.timestamp) < rate_limit_window)
        .count();
    
    if recent_attempts >= self.connection_limits.max_new_connections_per_minute {
        return Err(Error::Network(format!(
            "Connection rejected: Rate limit exceeded ({} connections/minute)",
            self.connection_limits.max_new_connections_per_minute
        )));
    }
    
    // Check connection cooldown for this specific address
    let last_attempt_for_address = attempts
        .iter()
        .filter(|attempt| attempt.peer_address == *address)
        .max_by_key(|attempt| attempt.timestamp);
        
    if let Some(last_attempt) = last_attempt_for_address {
        if now.duration_since(last_attempt.timestamp) < self.connection_limits.connection_cooldown {
            return Err(Error::Network(format!(
                "Connection rejected: Cooldown period active for {:?} ({}s remaining)",
                address,
                (self.connection_limits.connection_cooldown - now.duration_since(last_attempt.timestamp)).as_secs()
            )));
        }
    }
    
    Ok(())
}
```

**Multi-Layer Protection**:
1. **Global Limit**: Max total connections across all peers
2. **Per-Peer Limit**: Max connections from single peer
3. **Rate Limiting**: Max new connections per minute globally
4. **Cooldown**: Per-address connection attempt limiting

### Background Cleanup Task

```rust
// Lines 140-155
fn start_cleanup_task(&self) {
    let connection_attempts = self.connection_attempts.clone();
    let cleanup_interval = Duration::from_secs(60); // Clean up every minute
    
    tokio::spawn(async move {
        let mut interval = interval(cleanup_interval);
        loop {
            interval.tick().await;
            let cutoff = Instant::now() - Duration::from_secs(300); // Keep last 5 minutes
            
            let mut attempts = connection_attempts.write().await;
            attempts.retain(|attempt| attempt.timestamp > cutoff);
        }
    });
}
```

**Why Cleanup is Necessary**:
- **Memory Management**: Connection attempt history grows over time
- **Accurate Rate Limiting**: Old attempts shouldn't count against limits
- **Performance**: Smaller vectors are faster to iterate

### Bluetooth Transport Management

```rust
// Lines 258-282
/// Initialize enhanced Bluetooth transport with both central and peripheral roles
pub async fn init_enhanced_bluetooth(&mut self, local_peer_id: PeerId) -> Result<()> {
    log::info!("Initializing enhanced Bluetooth transport");
    
    let mut enhanced_bluetooth = EnhancedBluetoothTransport::new(local_peer_id).await
        .map_err(|e| Error::Network(format!("Failed to initialize enhanced Bluetooth: {}", e)))?;
    
    // Initialize the transport
    enhanced_bluetooth.initialize().await
        .map_err(|e| Error::Network(format!("Failed to initialize enhanced Bluetooth components: {}", e)))?;
    
    self.enhanced_bluetooth = Some(Arc::new(RwLock::new(enhanced_bluetooth)));
    
    log::info!("Enhanced Bluetooth transport initialized successfully");
    Ok(())
}
```

**Enhanced vs Basic Bluetooth**:
- **Basic**: Central role only (can connect to others)
- **Enhanced**: Both central and peripheral (can be discovered and can discover)
- **Mesh Capability**: Enhanced supports mesh networking patterns

### Connection Establishment with Rate Limiting

```rust
// Lines 343-387
pub async fn connect_to_peer(&self, peer_id: PeerId, address: TransportAddress) -> Result<()> {
    // Check connection limits before attempting to connect
    self.check_connection_limits(&address).await?;
    
    // Record the connection attempt
    self.record_connection_attempt(&address).await;
    
    match address {
        TransportAddress::Bluetooth(_) => {
            if let Some(bluetooth) = &self.bluetooth {
                let mut bt = bluetooth.write().await;
                
                // Attempt the connection
                match bt.connect(address.clone()).await {
                    Ok(_) => {
                        // Connection successful - update tracking
                        self.connections.write().await.insert(peer_id, address.clone());
                        self.increment_connection_count(&address).await;
                        
                        // Send connection event
                        let _ = self.event_sender.send(TransportEvent::Connected {
                            peer_id,
                            address: address.clone(),
                        });
                    }
                    Err(e) => {
                        // Connection failed - send error event
                        let error_msg = format!("Bluetooth connect failed: {}", e);
                        let _ = self.event_sender.send(TransportEvent::Error {
                            peer_id: Some(peer_id),
                            error: error_msg.clone(),
                        });
                        return Err(Error::Network(error_msg));
                    }
                }
            }
        }
        _ => {
            return Err(Error::Network("Unsupported transport type".to_string()));
        }
    }
    
    Ok(())
}
```

**Connection Algorithm**:
1. **Pre-flight Check**: Validate against connection limits
2. **Record Attempt**: Track for rate limiting
3. **Transport-Specific Connect**: Use appropriate transport mechanism
4. **Update Tracking**: Maintain connection state
5. **Event Notification**: Inform other components
6. **Error Handling**: Clean up on failure

### Data Transmission

```rust
// Lines 451-473
pub async fn send_to_peer(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
    let connections = self.connections.read().await;
    
    if let Some(address) = connections.get(&peer_id) {
        match address {
            TransportAddress::Bluetooth(_) => {
                if let Some(bluetooth) = &self.bluetooth {
                    let mut bt = bluetooth.write().await;
                    bt.send(peer_id, data).await
                        .map_err(|e| Error::Network(format!("Bluetooth send failed: {}", e)))?;
                }
            }
            _ => {
                return Err(Error::Network("Unsupported transport type".to_string()));
            }
        }
    } else {
        return Err(Error::Network("Peer not connected".to_string()));
    }
    
    Ok(())
}
```

**Send Process**:
1. **Lookup Connection**: Find peer's transport address
2. **Transport Selection**: Route to appropriate transport
3. **Send Data**: Use transport-specific send mechanism
4. **Error Handling**: Report transport failures

### Broadcasting to Multiple Peers

```rust
// Lines 475-490
pub async fn broadcast_packet(&self, packet: BitchatPacket) -> Result<()> {
    let mut serialized_packet = packet.clone();
    let data = serialized_packet.serialize()
        .map_err(|e| Error::Protocol(format!("Packet serialization failed: {}", e)))?;
    
    let connections = self.connections.read().await;
    
    for peer_id in connections.keys() {
        if let Err(e) = self.send_to_peer(*peer_id, data.clone()).await {
            log::warn!("Failed to broadcast to peer {:?}: {}", peer_id, e);
        }
    }
    
    Ok(())
}
```

**Broadcast Strategy**:
- **Serialize Once**: Convert packet to bytes once, send to all peers
- **Best Effort**: Continue broadcasting even if some peers fail
- **Logging**: Record failures for debugging
- **Non-blocking**: Don't fail entire broadcast if one peer fails

### Connection Statistics and Monitoring

```rust
// Lines 431-449
pub async fn connection_stats(&self) -> ConnectionStats {
    let connections = self.connections.read().await;
    let counts = self.connection_counts_per_address.read().await;
    let attempts = self.connection_attempts.read().await;
    
    let now = Instant::now();
    let recent_attempts = attempts
        .iter()
        .filter(|attempt| now.duration_since(attempt.timestamp) < Duration::from_secs(60))
        .count();
    
    ConnectionStats {
        total_connections: connections.len(),
        connections_by_address: counts.clone(),
        recent_connection_attempts: recent_attempts,
        connection_limit: self.connection_limits.max_total_connections,
    }
}
```

**Monitoring Data**:
- **Current Load**: How many connections are active?
- **Distribution**: Which addresses have most connections?
- **Activity**: How much connection activity recently?
- **Capacity**: How close are we to limits?

### Advanced Bluetooth Features

```rust
// Lines 284-315
/// Start mesh mode (both advertising and scanning)
pub async fn start_mesh_mode(&self, config: AdvertisingConfig) -> Result<()> {
    if let Some(enhanced_bt) = &self.enhanced_bluetooth {
        let mut bt = enhanced_bt.write().await;
        bt.start_mesh_mode(config).await
            .map_err(|e| Error::Network(format!("Failed to start mesh mode: {}", e)))
    } else {
        Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
    }
}

/// Get enhanced Bluetooth statistics
pub async fn get_enhanced_bluetooth_stats(&self) -> Result<EnhancedBluetoothStats> {
    if let Some(enhanced_bt) = &self.enhanced_bluetooth {
        let bt = enhanced_bt.read().await;
        Ok(bt.get_combined_stats().await)
    } else {
        Err(Error::Network("Enhanced Bluetooth transport not initialized".to_string()))
    }
}
```

**Mesh Mode Capabilities**:
- **Dual Role**: Both advertise (discoverable) and scan (discover others)
- **Dynamic Discovery**: Continuously find new peers
- **Statistics**: Monitor mesh network health
- **Configuration**: Adjust advertising parameters

### Transport Statistics

```rust
// Lines 504-515
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connection_count: usize,
    pub error_count: u64,
}
```

**Key Metrics**:
- **Throughput**: Bytes and packets sent/received
- **Connectivity**: Number of active connections
- **Reliability**: Error count and rates

---

## Transport Layer Design Patterns

### Pattern 1: Connection Pool Management

```rust
// Reuse expensive connections
let connection = pool.get_connection(peer_id).await?;
connection.send(data).await?;
pool.return_connection(connection);  // Don't close, reuse
```

**Benefits**:
- **Performance**: Avoid connection establishment overhead
- **Resource Efficiency**: Limit total connections
- **Reliability**: Maintain persistent connections

### Pattern 2: Event-Driven Architecture

```rust
// Transport layer emits events
event_sender.send(TransportEvent::Connected { peer_id, address });

// Application layer handles events
while let Some(event) = transport.next_event().await {
    match event {
        TransportEvent::DataReceived { peer_id, data } => {
            process_game_message(peer_id, data).await?;
        }
        TransportEvent::Disconnected { peer_id, reason } => {
            handle_player_disconnect(peer_id, reason).await?;
        }
        _ => {}
    }
}
```

**Benefits**:
- **Decoupling**: Transport and application concerns separated
- **Scalability**: Can handle multiple events concurrently
- **Reliability**: Events can be queued and processed later

### Pattern 3: Rate Limiting with Token Bucket

```rust
struct TokenBucket {
    tokens: u32,
    max_tokens: u32,
    refill_rate: u32,  // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }
}
```

**Benefits**:
- **Burst Handling**: Allow short bursts within limits
- **Fair Scheduling**: Smooth out traffic over time
- **DoS Protection**: Limit resource consumption

### Pattern 4: Circuit Breaker

```rust
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing recovery
}

struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    last_failure: Instant,
    timeout: Duration,
}
```

**Benefits**:
- **Fast Failure**: Don't wait for timeouts when service is down
- **Recovery**: Automatically retry when service might be back
- **Resource Protection**: Prevent cascading failures

---

## Security Considerations

### Transport Layer Security (TLS)

```rust
// Encrypt transport layer
let tls_config = TlsConfig {
    certificates: load_certificates()?,
    private_key: load_private_key()?,
    verify_peer: true,
};

transport.enable_tls(tls_config).await?;
```

**TLS Provides**:
- **Encryption**: Data confidentiality
- **Authentication**: Verify peer identity
- **Integrity**: Detect message tampering
- **Forward Secrecy**: Past messages safe if key compromised

### BLE Security Challenges

**BLE Security Issues**:
- **Weak Pairing**: Many devices use default PINs
- **Eavesdropping**: Radio signals can be intercepted
- **Replay Attacks**: Captured packets can be replayed
- **Jamming**: 2.4GHz band is crowded

**Our Mitigations**:
- **Application-Layer Encryption**: End-to-end security
- **Message Authentication**: Ed25519 signatures
- **Replay Protection**: Timestamps and nonces
- **Frequency Hopping**: BLE changes channels rapidly

---

## Performance Optimization

### Connection Pooling

```rust
struct ConnectionPool {
    active: HashMap<PeerId, Connection>,
    idle: VecDeque<Connection>,
    max_idle: usize,
    max_active: usize,
}

impl ConnectionPool {
    async fn get_connection(&mut self, peer_id: PeerId) -> Result<Connection> {
        // Try to reuse existing connection
        if let Some(conn) = self.active.get(&peer_id) {
            if conn.is_healthy().await {
                return Ok(conn.clone());
            }
        }
        
        // Try to get from idle pool
        if let Some(conn) = self.idle.pop_front() {
            if conn.is_healthy().await {
                self.active.insert(peer_id, conn.clone());
                return Ok(conn);
            }
        }
        
        // Create new connection
        let conn = self.create_connection(peer_id).await?;
        self.active.insert(peer_id, conn.clone());
        Ok(conn)
    }
}
```

### Batch Operations

```rust
// Instead of sending individual packets
for peer in peers {
    transport.send_to_peer(peer, packet.clone()).await?;
}

// Batch multiple packets
let batch = peers.into_iter().map(|peer| (peer, packet.clone())).collect();
transport.send_batch(batch).await?;
```

### Adaptive Timeouts

```rust
struct AdaptiveTimeout {
    base_timeout: Duration,
    current_timeout: Duration,
    min_timeout: Duration,
    max_timeout: Duration,
}

impl AdaptiveTimeout {
    fn on_success(&mut self) {
        // Decrease timeout on success
        self.current_timeout = (self.current_timeout * 95 / 100).max(self.min_timeout);
    }
    
    fn on_timeout(&mut self) {
        // Increase timeout on failure
        self.current_timeout = (self.current_timeout * 150 / 100).min(self.max_timeout);
    }
}
```

---

## Exercises

### Exercise 1: Implement WebSocket Transport

Add WebSocket support to the transport coordinator:

```rust
pub struct WebSocketTransport {
    connections: HashMap<PeerId, WebSocketStream>,
}

impl WebSocketTransport {
    pub async fn connect(&mut self, address: TransportAddress) -> Result<()> {
        // Implement WebSocket connection logic
    }
    
    pub async fn send(&mut self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        // Implement WebSocket send logic
    }
}
```

### Exercise 2: Add Connection Health Monitoring

Implement periodic health checks for all connections:

```rust
struct HealthChecker {
    check_interval: Duration,
    timeout: Duration,
}

impl HealthChecker {
    async fn check_all_connections(&self, transport: &TransportCoordinator) {
        // Send ping to all connected peers
        // Remove unresponsive peers
        // Update connection statistics
    }
}
```

### Exercise 3: Implement Transport Failover

Add automatic failover between transport types:

```rust
struct TransportFailover {
    primary: Box<dyn Transport>,
    secondary: Box<dyn Transport>,
    failover_threshold: Duration,
}

impl TransportFailover {
    async fn send_with_failover(&mut self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        // Try primary transport first
        // Fallback to secondary on failure
        // Track failure rates for each transport
    }
}
```

---

## Key Takeaways

1. **Transport Abstraction is Powerful**: Single API for multiple transport types
2. **Rate Limiting Prevents DoS**: Multiple layers of protection needed
3. **Connection Pooling Improves Performance**: Reuse expensive connections
4. **Event-Driven Architecture Scales**: Decouple transport from application logic
5. **Background Tasks Handle Maintenance**: Cleanup and monitoring in background
6. **Error Handling is Critical**: Network failures are common, plan for them
7. **Statistics Enable Optimization**: Monitor performance and capacity
8. **Security Must be End-to-End**: Don't rely only on transport security
9. **BLE Has Unique Constraints**: Range, power, and connection limits
10. **Mesh Networks Need Routing**: Simple flooding doesn't scale

---

## Next Chapter

[Chapter 13: Mesh Routing →](./13_mesh_routing.md)

Next, we'll explore how our transport layer enables sophisticated mesh routing algorithms, allowing casino nodes to communicate across multiple hops even when not directly connected!

---

*Remember: "The network is reliable" is the first fallacy of distributed computing. Always design for failure.*