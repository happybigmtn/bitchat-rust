# Chapter 72: Multi-Protocol Peer Discovery

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: Finding Friends in a Crowd

Imagine you're at a massive international conference where people speak different languages and use different communication methods—some use phones, others use radios, and some use sign language. You need to find and connect with specific people efficiently. This is the challenge of peer discovery in distributed systems.

## The Fundamentals: Discovery Mechanisms

Peer discovery must work across:
- Different network protocols (TCP, UDP, Bluetooth)
- Various discovery methods (DHT, mDNS, broadcast)
- Dynamic network conditions
- NAT and firewall traversal

## Deep Dive: Unified Discovery Framework

### Multi-Protocol Discovery Engine

```rust
pub struct UnifiedDiscovery {
    /// Available discovery protocols
    protocols: HashMap<ProtocolType, Box<dyn DiscoveryProtocol>>,
    
    /// Discovered peers
    peers: Arc<RwLock<PeerRegistry>>,
    
    /// Discovery coordinator
    coordinator: DiscoveryCoordinator,
}

#[async_trait]
pub trait DiscoveryProtocol: Send + Sync {
    async fn discover(&self) -> Result<Vec<PeerInfo>>;
    async fn announce(&self, info: &LocalPeerInfo) -> Result<()>;
    fn protocol_type(&self) -> ProtocolType;
}
```

## Bluetooth Discovery

### Local Peer Discovery

```rust
pub struct BluetoothDiscovery {
    /// BLE scanner
    scanner: BleScanner,
    
    /// Advertisement manager
    advertiser: BleAdvertiser,
    
    /// Service UUID
    service_uuid: Uuid,
}

impl BluetoothDiscovery {
    pub async fn scan_for_peers(&mut self) -> Result<Vec<BlePeer>> {
        let mut discovered = Vec::new();
        
        self.scanner.start().await?;
        
        let mut events = self.scanner.events();
        while let Some(event) = events.next().await {
            match event {
                ScanEvent::DeviceDiscovered { device, rssi } => {
                    if self.is_bitcraps_device(&device).await? {
                        discovered.push(BlePeer {
                            device,
                            rssi,
                            discovered_at: Instant::now(),
                        });
                    }
                }
                ScanEvent::ScanComplete => break,
            }
        }
        
        Ok(discovered)
    }
}
```

## DHT-Based Discovery

### Kademlia Integration

```rust
pub struct DhtDiscovery {
    /// Kademlia DHT
    dht: KademliaDht,
    
    /// Bootstrap nodes
    bootstrap: Vec<NodeInfo>,
    
    /// Local node ID
    node_id: NodeId,
}

impl DhtDiscovery {
    pub async fn find_peers(&mut self, count: usize) -> Result<Vec<PeerInfo>> {
        // Find nodes close to us
        let closest = self.dht.find_node(self.node_id).await?;
        
        // Filter for BitCraps nodes
        let peers = closest
            .into_iter()
            .filter(|n| n.supports_protocol("bitcraps"))
            .take(count)
            .collect();
        
        Ok(peers)
    }
    
    pub async fn register(&mut self) -> Result<()> {
        // Store our info in the DHT
        let key = self.node_id.to_key();
        let value = self.create_announcement();
        
        self.dht.store(key, value).await
    }
}
```

## mDNS/DNS-SD Discovery

### Local Network Discovery

```rust
pub struct MdnsDiscovery {
    /// mDNS responder
    responder: MdnsResponder,
    
    /// Service browser
    browser: ServiceBrowser,
    
    /// Service type
    service_type: String,
}

impl MdnsDiscovery {
    pub async fn browse_services(&mut self) -> Result<Vec<ServiceInfo>> {
        self.browser.browse(&self.service_type).await?;
        
        let mut services = Vec::new();
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        
        while start.elapsed() < timeout {
            if let Some(service) = self.browser.next_service().await {
                services.push(service);
            }
        }
        
        Ok(services)
    }
}
```

## Hybrid Discovery Strategies

### Combining Multiple Protocols

```rust
pub struct HybridDiscovery {
    /// Discovery methods in priority order
    methods: Vec<Box<dyn DiscoveryProtocol>>,
    
    /// Peer deduplication
    deduplicator: PeerDeduplicator,
    
    /// Discovery scheduler
    scheduler: DiscoveryScheduler,
}

impl HybridDiscovery {
    pub async fn discover_peers(&mut self) -> Result<Vec<UniquePeer>> {
        let mut all_peers = Vec::new();
        
        // Run discovery methods in parallel
        let futures: Vec<_> = self.methods
            .iter()
            .map(|method| method.discover())
            .collect();
        
        let results = futures::future::join_all(futures).await;
        
        // Aggregate results
        for result in results {
            if let Ok(peers) = result {
                all_peers.extend(peers);
            }
        }
        
        // Deduplicate
        Ok(self.deduplicator.deduplicate(all_peers))
    }
}
```

## NAT Traversal

### STUN/TURN Integration

```rust
pub struct NatTraversal {
    /// STUN client
    stun: StunClient,
    
    /// TURN relay
    turn: Option<TurnClient>,
    
    /// UPnP manager
    upnp: UpnpManager,
}

impl NatTraversal {
    pub async fn establish_connection(&mut self, peer: &PeerInfo) -> Result<Connection> {
        // Try direct connection
        if let Ok(conn) = self.try_direct(peer).await {
            return Ok(conn);
        }
        
        // Try STUN
        let public_addr = self.stun.get_public_address().await?;
        if let Ok(conn) = self.try_with_stun(peer, public_addr).await {
            return Ok(conn);
        }
        
        // Fallback to TURN relay
        if let Some(turn) = &mut self.turn {
            return turn.relay_connection(peer).await;
        }
        
        Err(Error::ConnectionFailed)
    }
}
```

## Testing Peer Discovery

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_multi_protocol_discovery() {
        let mut discovery = UnifiedDiscovery::new();
        
        // Add multiple discovery protocols
        discovery.add_protocol(Box::new(BluetoothDiscovery::new()));
        discovery.add_protocol(Box::new(DhtDiscovery::new()));
        discovery.add_protocol(Box::new(MdnsDiscovery::new()));
        
        // Discover peers
        let peers = discovery.discover_all().await.unwrap();
        
        // Should find peers from multiple sources
        assert!(!peers.is_empty());
        assert!(peers.iter().any(|p| p.source == DiscoverySource::Bluetooth));
        assert!(peers.iter().any(|p| p.source == DiscoverySource::DHT));
    }
}
```

## Conclusion

Multi-protocol peer discovery enables robust connectivity in diverse network environments. By combining multiple discovery mechanisms, we ensure peers can find each other regardless of network topology.

Key takeaways:
1. **Unified framework** abstracts protocol differences
2. **Bluetooth** enables local discovery
3. **DHT** provides global discovery
4. **mDNS** works on local networks
5. **NAT traversal** handles firewalls
6. **Hybrid strategies** maximize discovery success

Remember: The best discovery system is one that works everywhere, from local Bluetooth to global internet.
