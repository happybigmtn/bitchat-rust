# Chapter 83: MTU Discovery and Optimization - The Goldilocks Problem of Network Packets

## A Primer on Packet Size: From Telegrams to Jumbo Frames

In 1844, Samuel Morse sent the first telegraph message: "What hath God wrought." It was 23 characters. Not because that's all he had to say, but because longer messages were more likely to be corrupted during transmission. This fundamental tradeoff - bigger messages are more efficient but more fragile - has haunted networking ever since.

The Maximum Transmission Unit (MTU) is networking's answer to the question: "How big should a packet be?" It's the Goldilocks problem of networking - not too big, not too small, but just right. Too big and your packets get fragmented or dropped. Too small and you waste bandwidth on headers. The perfect size depends on every link between you and your destination, and those links can change at any moment.

Consider the Internet's dirty secret: it's not actually a network, it's a network of networks, each with its own rules. Your home network might support 1500-byte packets. Your ISP might handle 1492. The backbone might do 9000. That VPN you're using? It adds overhead, reducing effective MTU. Every hop can have a different limit, and the smallest one wins. It's like shipping cargo through a series of tunnels - the narrowest tunnel determines the maximum container size.

The history of MTU is a history of compromises. Ethernet chose 1500 bytes in 1980 not for any profound technical reason, but because it fit nicely in the memory buffers of contemporary hardware. IPv6 mandates a minimum MTU of 1280 bytes - a number chosen because it's divisible by 8 (important for 64-bit processing) and large enough to be useful but small enough to work almost everywhere. These arbitrary decisions from decades ago still shape every packet you send today.

Path MTU Discovery (PMTUD) was supposed to solve this automatically. The idea is elegant: send big packets with the "Don't Fragment" flag set. If a router can't forward it without fragmenting, it sends back an ICMP "Packet Too Big" message telling you the maximum size it accepts. You shrink your packets and try again. In theory, you quickly converge on the optimal size. In practice, many firewalls block ICMP messages, leaving you blind. It's like trying to find the height of a bridge by driving increasingly tall trucks under it, except sometimes the trucks just disappear without explanation.

In distributed systems like BitCraps, MTU becomes critical. Every byte of overhead is a byte not available for game data. In a mesh network where packets might traverse multiple hops, finding the optimal MTU can mean the difference between smooth gameplay and frustrating lag. When you're trying to achieve consensus among nodes with different network capabilities, packet size optimization isn't just about performance - it's about fairness.

The challenge is that MTU isn't static. Network paths change. VPNs turn on and off. Mobile devices switch between WiFi and cellular. What worked five minutes ago might not work now. Modern systems need to continuously discover and adapt to changing MTU conditions, all while maintaining reliable communication.

## The Science of Packet Sizing

Understanding MTU requires understanding how packets actually move through networks:

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Represents the MTU constraints for a network path
#[derive(Debug, Clone)]
pub struct PathMTU {
    /// The destination address
    pub destination: IpAddr,
    
    /// The discovered MTU for this path
    pub mtu: usize,
    
    /// When this MTU was last confirmed
    pub last_confirmed: Instant,
    
    /// Confidence in this MTU value (0.0 to 1.0)
    pub confidence: f64,
    
    /// Number of successful transmissions at this MTU
    pub success_count: u64,
    
    /// History of MTU changes for this path
    pub history: Vec<MTUChange>,
}

#[derive(Debug, Clone)]
pub struct MTUChange {
    pub timestamp: Instant,
    pub old_mtu: usize,
    pub new_mtu: usize,
    pub reason: ChangeReason,
}

#[derive(Debug, Clone)]
pub enum ChangeReason {
    /// ICMP Packet Too Big message received
    ICMPMessage { reported_mtu: usize },
    
    /// Packet loss detected at current MTU
    PacketLoss { loss_rate: f64 },
    
    /// Successful probe at higher MTU
    ProbeSuccess,
    
    /// Timeout waiting for response
    Timeout,
    
    /// Network path changed (different route)
    PathChange { old_hop_count: u8, new_hop_count: u8 },
    
    /// Manual configuration
    Manual,
}

/// Common MTU sizes in networking
pub mod standard_mtus {
    pub const ETHERNET: usize = 1500;        // Standard Ethernet
    pub const ETHERNET_JUMBO: usize = 9000;  // Jumbo frames
    pub const IPV6_MIN: usize = 1280;        // IPv6 minimum
    pub const IPV4_MIN: usize = 68;          // IPv4 minimum (impractical)
    pub const PPPOE: usize = 1492;           // PPPoE (DSL)
    pub const VPN_IPSEC: usize = 1438;       // IPSec VPN typical
    pub const WIFI: usize = 2304;            // 802.11 WiFi
    pub const LOOPBACK: usize = 65536;       // Loopback interface
    pub const INFINIBAND: usize = 65520;     // InfiniBand
}

/// Calculate overhead for different protocols
pub fn calculate_overhead(protocol_stack: &[Protocol]) -> usize {
    protocol_stack.iter().map(|p| match p {
        Protocol::Ethernet => 14,      // Ethernet header
        Protocol::IPv4 => 20,           // IPv4 header (no options)
        Protocol::IPv6 => 40,           // IPv6 header
        Protocol::TCP => 20,            // TCP header (no options)
        Protocol::UDP => 8,             // UDP header
        Protocol::VLAN => 4,            // 802.1Q VLAN tag
        Protocol::MPLS => 4,            // MPLS label
        Protocol::GRE => 8,             // GRE header
        Protocol::IPSec => 50,          // IPSec ESP (approximate)
        Protocol::WireGuard => 32,      // WireGuard header
    }).sum()
}
```

## Path MTU Discovery Implementation

Modern PMTUD must handle both successful and failing cases:

```rust
pub struct MTUDiscovery {
    /// Cached MTU values for different paths
    path_cache: Arc<RwLock<HashMap<IpAddr, PathMTU>>>,
    
    /// Configuration for discovery behavior
    config: MTUConfig,
    
    /// Socket for sending probes
    probe_socket: Arc<UdpSocket>,
    
    /// Metrics for monitoring
    metrics: Arc<Mutex<MTUMetrics>>,
    
    /// Background discovery tasks
    active_probes: Arc<DashMap<IpAddr, JoinHandle<()>>>,
}

pub struct MTUConfig {
    /// Starting MTU for discovery
    pub initial_mtu: usize,
    
    /// Minimum acceptable MTU
    pub min_mtu: usize,
    
    /// Maximum MTU to attempt
    pub max_mtu: usize,
    
    /// How long to wait for probe responses
    pub probe_timeout: Duration,
    
    /// How many probes to send for each size
    pub probes_per_size: u8,
    
    /// Binary search or linear search
    pub search_algorithm: SearchAlgorithm,
    
    /// How often to reverify MTU
    pub reverification_interval: Duration,
    
    /// Enable black hole detection
    pub black_hole_detection: bool,
}

#[derive(Debug, Clone)]
pub enum SearchAlgorithm {
    /// Binary search between min and max
    Binary,
    
    /// Linear increase until failure
    Linear { step_size: usize },
    
    /// Exponential increase then binary search
    Adaptive,
    
    /// Try common MTU values first
    Heuristic { common_values: Vec<usize> },
}

impl MTUDiscovery {
    /// Discover the Path MTU to a destination
    pub async fn discover_path_mtu(&self, destination: IpAddr) -> Result<usize> {
        // Check cache first
        if let Some(cached) = self.get_cached_mtu(destination).await {
            if cached.last_confirmed.elapsed() < self.config.reverification_interval {
                return Ok(cached.mtu);
            }
        }
        
        // Start discovery process
        match self.config.search_algorithm {
            SearchAlgorithm::Binary => {
                self.binary_search_mtu(destination).await
            }
            SearchAlgorithm::Linear { step_size } => {
                self.linear_search_mtu(destination, step_size).await
            }
            SearchAlgorithm::Adaptive => {
                self.adaptive_search_mtu(destination).await
            }
            SearchAlgorithm::Heuristic { ref common_values } => {
                self.heuristic_search_mtu(destination, common_values).await
            }
        }
    }
    
    /// Binary search for optimal MTU
    async fn binary_search_mtu(&self, destination: IpAddr) -> Result<usize> {
        let mut low = self.config.min_mtu;
        let mut high = self.config.max_mtu;
        let mut last_working = low;
        
        while low < high {
            let mid = (low + high + 1) / 2;
            
            if self.probe_mtu(destination, mid).await? {
                last_working = mid;
                low = mid;
            } else {
                high = mid - 1;
            }
            
            // Avoid infinite loops due to off-by-one errors
            if high - low <= 1 {
                break;
            }
        }
        
        // Cache the result
        self.cache_mtu(destination, last_working).await;
        
        Ok(last_working)
    }
    
    /// Send probe packet with specific MTU
    async fn probe_mtu(&self, destination: IpAddr, mtu: usize) -> Result<bool> {
        let probe_id = rand::random::<u64>();
        let probe_data = self.create_probe_packet(probe_id, mtu);
        
        // Set Don't Fragment flag (platform specific)
        self.set_dont_fragment(&self.probe_socket)?;
        
        // Send probes
        let mut successes = 0;
        for _ in 0..self.config.probes_per_size {
            self.probe_socket.send_to(&probe_data, destination).await?;
            
            // Wait for response or ICMP error
            match timeout(self.config.probe_timeout, self.wait_for_probe_response(probe_id)).await {
                Ok(Ok(response)) => {
                    match response {
                        ProbeResponse::Success => successes += 1,
                        ProbeResponse::TooBig(reported_mtu) => {
                            // Router reported smaller MTU
                            self.metrics.lock().unwrap().icmp_received += 1;
                            return Ok(false);
                        }
                        ProbeResponse::Timeout => {
                            // No response might mean black hole
                            if self.config.black_hole_detection {
                                self.detect_black_hole(destination, mtu).await?;
                            }
                        }
                    }
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => {} // Timeout
            }
        }
        
        // Require majority of probes to succeed
        Ok(successes > self.config.probes_per_size / 2)
    }
    
    /// Detect PMTUD black holes (firewalls dropping ICMP)
    async fn detect_black_hole(&self, destination: IpAddr, mtu: usize) -> Result<bool> {
        // Try smaller packets to see if it's really a black hole
        let small_mtu = self.config.min_mtu;
        
        if self.probe_mtu(destination, small_mtu).await? {
            // Small packets work but large ones don't - likely a black hole
            warn!("PMTUD black hole detected for {:?} at MTU {}", destination, mtu);
            
            self.metrics.lock().unwrap().black_holes_detected += 1;
            
            // Fall back to minimum MTU or TCP MSS clamping
            self.cache_mtu(destination, small_mtu).await;
            
            return Ok(true);
        }
        
        Ok(false)
    }
}
```

## MTU Optimization Strategies

Different network scenarios require different optimization approaches:

### 1. Jumbo Frame Detection and Usage

```rust
pub struct JumboFrameOptimizer {
    discovery: Arc<MTUDiscovery>,
    config: JumboConfig,
}

pub struct JumboConfig {
    /// Minimum MTU to consider "jumbo"
    pub jumbo_threshold: usize,
    
    /// Test for jumbo frame support
    pub auto_detect: bool,
    
    /// Gradual or immediate switch
    pub transition_strategy: TransitionStrategy,
}

#[derive(Clone)]
pub enum TransitionStrategy {
    /// Switch immediately to jumbo frames
    Immediate,
    
    /// Gradually increase packet size
    Gradual { increment: usize },
    
    /// Use jumbo frames only for bulk transfers
    Selective { size_threshold: usize },
}

impl JumboFrameOptimizer {
    /// Optimize for local network with potential jumbo frame support
    pub async fn optimize_local_network(&self) -> Result<NetworkOptimization> {
        let mut optimization = NetworkOptimization::default();
        
        // Detect all local interfaces
        let interfaces = self.discover_local_interfaces().await?;
        
        for interface in interfaces {
            // Test jumbo frame support
            let max_mtu = self.test_interface_mtu(interface).await?;
            
            if max_mtu >= self.config.jumbo_threshold {
                optimization.jumbo_capable_interfaces.push(interface);
                
                match self.config.transition_strategy {
                    TransitionStrategy::Immediate => {
                        self.enable_jumbo_frames(interface, max_mtu).await?;
                    }
                    TransitionStrategy::Gradual { increment } => {
                        self.gradually_increase_mtu(interface, increment, max_mtu).await?;
                    }
                    TransitionStrategy::Selective { size_threshold } => {
                        self.setup_selective_jumbo(interface, size_threshold, max_mtu).await?;
                    }
                }
            }
        }
        
        Ok(optimization)
    }
    
    /// Test maximum MTU for an interface
    async fn test_interface_mtu(&self, interface: NetworkInterface) -> Result<usize> {
        // Start with jumbo frame size and work down
        let mut test_mtu = 9000;
        
        while test_mtu > standard_mtus::ETHERNET {
            // Create loopback test
            let test_successful = self.loopback_test(interface, test_mtu).await?;
            
            if test_successful {
                return Ok(test_mtu);
            }
            
            // Try smaller size
            test_mtu -= 500;
        }
        
        Ok(standard_mtus::ETHERNET)
    }
}
```

### 2. TCP MSS Clamping

For TCP connections, Maximum Segment Size (MSS) clamping prevents fragmentation:

```rust
pub struct TCPOptimizer {
    mtu_discovery: Arc<MTUDiscovery>,
    mss_cache: Arc<RwLock<HashMap<SocketAddr, usize>>>,
}

impl TCPOptimizer {
    /// Calculate optimal MSS for a TCP connection
    pub fn calculate_mss(&self, path_mtu: usize, ip_version: IpVersion) -> usize {
        let ip_header_size = match ip_version {
            IpVersion::V4 => 20,  // Minimum IPv4 header
            IpVersion::V6 => 40,  // IPv6 header
        };
        
        let tcp_header_size = 20;  // Minimum TCP header
        
        // MSS = MTU - IP header - TCP header
        path_mtu.saturating_sub(ip_header_size + tcp_header_size)
    }
    
    /// Apply MSS clamping to a TCP socket
    pub fn apply_mss_clamping(&self, socket: &TcpStream, destination: IpAddr) -> Result<()> {
        // Get path MTU
        let path_mtu = self.mtu_discovery.discover_path_mtu(destination).await?;
        
        // Calculate MSS
        let ip_version = match destination {
            IpAddr::V4(_) => IpVersion::V4,
            IpAddr::V6(_) => IpVersion::V6,
        };
        let mss = self.calculate_mss(path_mtu, ip_version);
        
        // Set TCP_MAXSEG socket option (platform specific)
        unsafe {
            let mss_val = mss as c_int;
            let ret = setsockopt(
                socket.as_raw_fd(),
                IPPROTO_TCP,
                TCP_MAXSEG,
                &mss_val as *const _ as *const c_void,
                std::mem::size_of::<c_int>() as socklen_t,
            );
            
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        
        // Cache for future use
        self.mss_cache.write().unwrap().insert(
            socket.peer_addr()?,
            mss
        );
        
        Ok(())
    }
    
    /// Implement PLPMTUD (Packetization Layer Path MTU Discovery) for TCP
    pub async fn plpmtud(&self, socket: &TcpStream) -> Result<usize> {
        let mut current_mss = 1460;  // Start with common value
        let mut probe_size = current_mss;
        
        loop {
            // Send probe packet
            let probe_data = vec![0u8; probe_size];
            match socket.send(&probe_data).await {
                Ok(n) if n == probe_size => {
                    // Success, try larger
                    current_mss = probe_size;
                    probe_size = (probe_size * 110) / 100;  // Increase by 10%
                    
                    if probe_size > 9000 {
                        break;  // Maximum reasonable size
                    }
                }
                Ok(_) | Err(_) => {
                    // Failed, back off
                    probe_size = (current_mss + probe_size) / 2;
                    
                    if probe_size - current_mss < 10 {
                        break;  // Converged
                    }
                }
            }
        }
        
        Ok(current_mss)
    }
}
```

### 3. Application-Layer Fragmentation

When PMTUD fails, implement application-layer fragmentation:

```rust
pub struct ApplicationFragmenter {
    mtu_discovery: Arc<MTUDiscovery>,
    fragment_cache: Arc<DashMap<u64, FragmentCollector>>,
}

pub struct Fragment {
    /// Unique message ID
    pub message_id: u64,
    
    /// Fragment number (0-based)
    pub fragment_num: u16,
    
    /// Total number of fragments
    pub total_fragments: u16,
    
    /// Fragment data
    pub data: Vec<u8>,
    
    /// Checksum of complete message
    pub message_checksum: u32,
}

pub struct FragmentCollector {
    fragments: Vec<Option<Vec<u8>>>,
    total_fragments: u16,
    received_fragments: u16,
    message_checksum: u32,
    started_at: Instant,
}

impl ApplicationFragmenter {
    /// Fragment a message based on path MTU
    pub async fn fragment_message(
        &self,
        message: &[u8],
        destination: IpAddr,
    ) -> Result<Vec<Fragment>> {
        // Get path MTU
        let path_mtu = self.mtu_discovery.discover_path_mtu(destination).await?;
        
        // Calculate usable payload size
        let header_overhead = std::mem::size_of::<Fragment>() - std::mem::size_of::<Vec<u8>>();
        let max_payload = path_mtu.saturating_sub(header_overhead + 28);  // IP + UDP headers
        
        // Calculate number of fragments
        let total_fragments = ((message.len() + max_payload - 1) / max_payload) as u16;
        
        if total_fragments > u16::MAX {
            return Err(Error::MessageTooLarge);
        }
        
        // Calculate checksum of complete message
        let message_checksum = crc32::checksum_ieee(message);
        let message_id = rand::random();
        
        // Create fragments
        let mut fragments = Vec::new();
        for i in 0..total_fragments {
            let start = i as usize * max_payload;
            let end = ((i + 1) as usize * max_payload).min(message.len());
            
            fragments.push(Fragment {
                message_id,
                fragment_num: i,
                total_fragments,
                data: message[start..end].to_vec(),
                message_checksum,
            });
        }
        
        Ok(fragments)
    }
    
    /// Reassemble fragments into original message
    pub async fn reassemble_message(&self, fragment: Fragment) -> Result<Option<Vec<u8>>> {
        let mut entry = self.fragment_cache.entry(fragment.message_id).or_insert_with(|| {
            FragmentCollector {
                fragments: vec![None; fragment.total_fragments as usize],
                total_fragments: fragment.total_fragments,
                received_fragments: 0,
                message_checksum: fragment.message_checksum,
                started_at: Instant::now(),
            }
        });
        
        let collector = entry.get_mut();
        
        // Check if fragment is duplicate
        if collector.fragments[fragment.fragment_num as usize].is_some() {
            return Ok(None);
        }
        
        // Store fragment
        collector.fragments[fragment.fragment_num as usize] = Some(fragment.data);
        collector.received_fragments += 1;
        
        // Check if all fragments received
        if collector.received_fragments == collector.total_fragments {
            // Reassemble message
            let mut message = Vec::new();
            for fragment_data in collector.fragments.iter() {
                if let Some(data) = fragment_data {
                    message.extend_from_slice(data);
                }
            }
            
            // Verify checksum
            let checksum = crc32::checksum_ieee(&message);
            if checksum != collector.message_checksum {
                return Err(Error::ChecksumMismatch);
            }
            
            // Clean up cache entry
            drop(entry);
            self.fragment_cache.remove(&fragment.message_id);
            
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
    
    /// Clean up old incomplete fragments
    pub async fn cleanup_expired_fragments(&self, timeout: Duration) {
        let now = Instant::now();
        
        self.fragment_cache.retain(|_, collector| {
            now.duration_since(collector.started_at) < timeout
        });
    }
}
```

## BitCraps MTU Optimization

Let's see how BitCraps handles MTU in its mesh network:

```rust
// From src/transport/mtu_optimizer.rs

pub struct BitCrapsMTUOptimizer {
    /// MTU discovery for each peer
    peer_mtus: Arc<DashMap<PeerId, PathMTU>>,
    
    /// Network transport
    transport: Arc<dyn Transport>,
    
    /// Consensus on network MTU
    consensus_mtu: AtomicUsize,
    
    /// Configuration
    config: BitCrapsMTUConfig,
}

pub struct BitCrapsMTUConfig {
    /// Minimum MTU for game packets
    pub min_game_mtu: usize,
    
    /// Target MTU for optimal performance
    pub target_mtu: usize,
    
    /// Enable aggressive MTU probing
    pub aggressive_probing: bool,
    
    /// Consensus threshold for MTU agreement
    pub consensus_threshold: f64,
}

impl BitCrapsMTUOptimizer {
    /// Initialize MTU optimization for BitCraps network
    pub async fn initialize(&self) -> Result<()> {
        // Discover MTU to all peers
        let peers = self.transport.get_connected_peers().await?;
        
        let mut discovered_mtus = Vec::new();
        for peer in peers {
            match self.discover_peer_mtu(peer).await {
                Ok(mtu) => discovered_mtus.push(mtu),
                Err(e) => warn!("Failed to discover MTU for peer {:?}: {}", peer, e),
            }
        }
        
        // Calculate consensus MTU (smallest common MTU)
        if !discovered_mtus.is_empty() {
            let consensus = *discovered_mtus.iter().min().unwrap();
            self.consensus_mtu.store(consensus, Ordering::Relaxed);
            info!("Network consensus MTU: {}", consensus);
        }
        
        // Start continuous optimization
        tokio::spawn(self.clone().continuous_optimization());
        
        Ok(())
    }
    
    /// Discover MTU to a specific peer
    async fn discover_peer_mtu(&self, peer: PeerId) -> Result<usize> {
        // For Bluetooth LE connections
        if self.transport.is_bluetooth(peer).await? {
            // BLE has specific MTU negotiation
            return self.discover_ble_mtu(peer).await;
        }
        
        // For TCP/UDP connections
        let mut discovery = MTUDiscovery::new(MTUConfig {
            initial_mtu: self.config.target_mtu,
            min_mtu: self.config.min_game_mtu,
            max_mtu: 9000,  // Maximum we'll attempt
            probe_timeout: Duration::from_millis(500),
            probes_per_size: 3,
            search_algorithm: SearchAlgorithm::Adaptive,
            reverification_interval: Duration::from_secs(300),
            black_hole_detection: true,
        });
        
        let peer_addr = self.transport.get_peer_address(peer).await?;
        let mtu = discovery.discover_path_mtu(peer_addr).await?;
        
        // Cache for this peer
        self.peer_mtus.insert(peer, PathMTU {
            destination: peer_addr,
            mtu,
            last_confirmed: Instant::now(),
            confidence: 0.9,
            success_count: 0,
            history: vec![],
        });
        
        Ok(mtu)
    }
    
    /// BLE-specific MTU discovery
    async fn discover_ble_mtu(&self, peer: PeerId) -> Result<usize> {
        // BLE MTU negotiation is different
        // Default BLE MTU is 23 bytes, but can negotiate up to 517
        
        let mut current_mtu = 23;
        let max_ble_mtu = 517;
        
        // Send MTU request
        let request = BLEMTURequest {
            client_mtu: max_ble_mtu,
        };
        
        match self.transport.send_control(peer, &request).await {
            Ok(BLEMTUResponse { server_mtu }) => {
                current_mtu = current_mtu.max(server_mtu).min(max_ble_mtu);
            }
            Err(e) => {
                warn!("BLE MTU negotiation failed: {}", e);
            }
        }
        
        Ok(current_mtu)
    }
    
    /// Continuously optimize MTU based on network conditions
    async fn continuous_optimization(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            // Re-probe a subset of peers
            let peers = self.transport.get_connected_peers().await.unwrap_or_default();
            let sample_size = (peers.len() / 10).max(1).min(5);
            
            for peer in peers.choose_multiple(&mut rand::thread_rng(), sample_size) {
                if let Some(mut entry) = self.peer_mtus.get_mut(peer) {
                    // Only re-probe if confidence is low or MTU is old
                    if entry.confidence < 0.8 || entry.last_confirmed.elapsed() > Duration::from_secs(600) {
                        if let Ok(new_mtu) = self.discover_peer_mtu(*peer).await {
                            if new_mtu != entry.mtu {
                                entry.history.push(MTUChange {
                                    timestamp: Instant::now(),
                                    old_mtu: entry.mtu,
                                    new_mtu,
                                    reason: ChangeReason::ProbeSuccess,
                                });
                                entry.mtu = new_mtu;
                            }
                            entry.last_confirmed = Instant::now();
                            entry.confidence = (entry.confidence * 0.9) + 0.1;  // Increase confidence
                        }
                    }
                }
            }
            
            // Update consensus MTU if needed
            self.update_consensus_mtu().await;
        }
    }
    
    /// Fragment game messages for transmission
    pub async fn send_game_message(&self, message: GameMessage, recipient: PeerId) -> Result<()> {
        let serialized = bincode::serialize(&message)?;
        
        // Get MTU for this peer
        let mtu = self.peer_mtus
            .get(&recipient)
            .map(|entry| entry.mtu)
            .unwrap_or_else(|| self.consensus_mtu.load(Ordering::Relaxed));
        
        // Account for protocol overhead
        let overhead = 100;  // Approximate overhead for headers, encryption, etc.
        let max_payload = mtu.saturating_sub(overhead);
        
        if serialized.len() <= max_payload {
            // Fits in single packet
            self.transport.send(recipient, &serialized).await?;
        } else {
            // Need to fragment
            let fragmenter = ApplicationFragmenter::new();
            let fragments = fragmenter.fragment_message(
                &serialized,
                self.transport.get_peer_address(recipient).await?
            ).await?;
            
            for fragment in fragments {
                let fragment_data = bincode::serialize(&fragment)?;
                self.transport.send(recipient, &fragment_data).await?;
                
                // Small delay between fragments to avoid overwhelming receiver
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
        }
        
        Ok(())
    }
}

/// Special handling for game consensus messages
impl BitCrapsMTUOptimizer {
    /// Optimize consensus message transmission
    pub async fn broadcast_consensus(&self, consensus: ConsensusMessage) -> Result<()> {
        // For consensus, we need all peers to receive it
        // Use the minimum MTU to ensure it reaches everyone
        let min_mtu = self.consensus_mtu.load(Ordering::Relaxed);
        
        // Consensus messages are critical - ensure they fit in one packet
        let serialized = bincode::serialize(&consensus)?;
        if serialized.len() > min_mtu - 100 {
            // Compress if too large
            let compressed = self.compress_consensus(&consensus)?;
            if compressed.len() > min_mtu - 100 {
                return Err(Error::ConsensusTooLarge);
            }
            
            self.transport.broadcast(&compressed).await?;
        } else {
            self.transport.broadcast(&serialized).await?;
        }
        
        Ok(())
    }
    
    fn compress_consensus(&self, consensus: &ConsensusMessage) -> Result<Vec<u8>> {
        // Use zstd compression for consensus messages
        let mut encoder = zstd::Encoder::new(Vec::new(), 3)?;
        bincode::serialize_into(&mut encoder, consensus)?;
        Ok(encoder.finish()?)
    }
}
```

## Monitoring and Diagnostics

MTU problems are notoriously hard to debug. Comprehensive monitoring is essential:

```rust
pub struct MTUMonitor {
    metrics: Arc<Mutex<MTUMetrics>>,
    alerts: Arc<Mutex<Vec<MTUAlert>>>,
}

#[derive(Default)]
pub struct MTUMetrics {
    pub successful_discoveries: u64,
    pub failed_discoveries: u64,
    pub black_holes_detected: u64,
    pub icmp_received: u64,
    pub icmp_blocked: u64,
    pub fragmentation_required: u64,
    pub avg_mtu: f64,
    pub min_mtu: usize,
    pub max_mtu: usize,
    pub mtu_changes: u64,
}

pub struct MTUAlert {
    pub timestamp: Instant,
    pub severity: Severity,
    pub message: String,
    pub affected_peer: Option<PeerId>,
}

impl MTUMonitor {
    /// Detect and alert on MTU anomalies
    pub async fn detect_anomalies(&self) {
        let metrics = self.metrics.lock().unwrap().clone();
        
        // Check for excessive black holes
        if metrics.black_holes_detected > 10 {
            self.alert(
                Severity::High,
                "Excessive PMTUD black holes detected - ICMP may be filtered",
                None
            );
        }
        
        // Check for MTU instability
        if metrics.mtu_changes > 100 {
            self.alert(
                Severity::Medium,
                "MTU unstable - network path may be changing frequently",
                None
            );
        }
        
        // Check for too-small MTU
        if metrics.avg_mtu < 1280.0 {
            self.alert(
                Severity::High,
                format!("Average MTU {} below IPv6 minimum", metrics.avg_mtu),
                None
            );
        }
    }
    
    /// Generate MTU diagnostic report
    pub fn generate_diagnostic_report(&self) -> MTUDiagnosticReport {
        let metrics = self.metrics.lock().unwrap();
        
        MTUDiagnosticReport {
            timestamp: Utc::now(),
            success_rate: metrics.successful_discoveries as f64 / 
                         (metrics.successful_discoveries + metrics.failed_discoveries) as f64,
            black_hole_rate: metrics.black_holes_detected as f64 / 
                            metrics.successful_discoveries as f64,
            icmp_filter_rate: metrics.icmp_blocked as f64 / 
                             (metrics.icmp_received + metrics.icmp_blocked) as f64,
            fragmentation_rate: metrics.fragmentation_required as f64 / 
                               metrics.successful_discoveries as f64,
            mtu_distribution: self.calculate_mtu_distribution(),
            recommendations: self.generate_recommendations(&metrics),
        }
    }
}
```

## Exercises

### Exercise 1: Implement Packetization Layer Path MTU Discovery

Create PLPMTUD for protocols without ICMP support:

```rust
pub trait PLPMTUD {
    async fn discover_mtu_without_icmp(&self, destination: IpAddr) -> Result<usize>;
    async fn handle_packet_loss(&self, size: usize) -> Result<()>;
    async fn probe_with_padding(&self, size: usize) -> Result<bool>;
}

// TODO: Implement PLPMTUD that works even when ICMP is filtered
```

### Exercise 2: Build MTU-Aware Load Balancer

Create a load balancer that considers MTU when routing:

```rust
pub struct MTUAwareLoadBalancer {
    servers: Vec<Server>,
    mtu_map: HashMap<Server, usize>,
}

impl MTUAwareLoadBalancer {
    pub async fn route_request(&self, request_size: usize) -> Server {
        // TODO: Route to server with appropriate MTU for request size
        // Consider both capacity and MTU constraints
    }
}
```

### Exercise 3: Implement Adaptive Fragmentation

Build a system that adapts fragmentation strategy based on network conditions:

```rust
pub struct AdaptiveFragmenter {
    strategies: Vec<FragmentationStrategy>,
    current_strategy: usize,
    performance_history: VecDeque<PerformanceMetric>,
}

impl AdaptiveFragmenter {
    pub async fn fragment_adaptive(&mut self, data: &[u8]) -> Vec<Fragment> {
        // TODO: Choose fragmentation strategy based on recent performance
        // Switch strategies if current one is performing poorly
        // Track success rates and adapt
    }
}
```

## Common Pitfalls and Solutions

### Pitfall 1: Assuming Symmetric MTU
**Problem**: Path MTU might be different in each direction
**Solution**: Discover MTU independently for each direction

### Pitfall 2: Ignoring MTU Changes
**Problem**: Network paths change, MTU can vary over time
**Solution**: Periodically reverify and adapt to changes

### Pitfall 3: Over-Aggressive Probing
**Problem**: Too many probes trigger DDoS protection
**Solution**: Implement exponential backoff and rate limiting

### Pitfall 4: Not Handling IPv6 Minimum
**Problem**: IPv6 requires minimum 1280 byte MTU
**Solution**: Never go below 1280 for IPv6 connections

## Summary

MTU discovery and optimization is crucial for network performance. The key insights:

1. **MTU is not constant**: It varies by path and changes over time
2. **PMTUD often fails**: Firewalls block ICMP, creating black holes
3. **Application-layer solutions**: When PMTUD fails, handle it yourself
4. **Monitor everything**: MTU problems are hard to diagnose without data
5. **Optimize for your use case**: Gaming needs different MTU strategy than file transfer
6. **Test with real networks**: MTU behavior differs greatly across networks

Master MTU optimization, and you'll eliminate a entire class of mysterious networking problems that plague distributed systems.

## References

- RFC 1191: Path MTU Discovery
- RFC 4821: Packetization Layer Path MTU Discovery
- RFC 8899: Discovering Path MTU Black Holes
- "TCP/IP Illustrated, Volume 1" by W. Richard Stevens
- "High Performance Browser Networking" by Ilya Grigorik

---

*Next Chapter: [Chapter 84: Database Migration Systems](./84_database_migration_systems.md)*

*Previous Chapter: [Chapter 82: Connection Pool Management](./82_connection_pool_management.md)*