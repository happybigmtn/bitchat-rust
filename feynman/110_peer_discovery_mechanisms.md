# Chapter 85: Peer Discovery Mechanisms - Finding Friends in the Digital Wild

## Understanding Peer Discovery Through BitCraps Mesh Network
*"Peer discovery is like being new in town and trying to find people to hang out with - except the town is the entire internet."*

---

## Part I: The Challenge of Finding Peers

Imagine you want to play BitCraps, but you don't know anyone else who plays. How do you find other players? In traditional client-server systems, this is easy - everyone connects to the same server. But in peer-to-peer systems like BitCraps, there is no central server. Every player needs to find other players directly.

This is like trying to organize a pickup basketball game in a city where:
- You don't know anyone
- There's no central court where everyone meets
- People come and go randomly
- Some people are using different rules
- You need to find people nearby (for good connection quality)

BitCraps solves this using multiple peer discovery mechanisms working together. Let's explore how the `src/discovery/` and `src/mesh/` modules tackle this challenge.

## Part II: The BitCraps Discovery Architecture

### Multi-Protocol Discovery System

```rust
// From src/discovery/mod.rs
pub struct PeerDiscoveryManager {
    bluetooth_discovery: BluetoothDiscovery,
    dht_discovery: DHTDiscovery,
    mdns_discovery: mDNSDiscovery,
    bootstrap_nodes: Vec<BootstrapNode>,
    discovered_peers: Arc<RwLock<HashMap<PeerId, PeerInfo>>>,
    discovery_config: DiscoveryConfig,
}

impl PeerDiscoveryManager {
    pub async fn start_discovery(&self) -> Result<(), DiscoveryError> {
        // Start all discovery mechanisms in parallel
        let bluetooth_task = self.bluetooth_discovery.start_scanning();
        let dht_task = self.dht_discovery.start_searching();
        let mdns_task = self.mdns_discovery.start_broadcasting();
        let bootstrap_task = self.connect_to_bootstrap_nodes();
        
        // Run all discovery methods concurrently
        tokio::try_join!(
            bluetooth_task,
            dht_task,
            mdns_task,
            bootstrap_task
        )?;
        
        // Start periodic cleanup of stale peers
        self.start_peer_maintenance().await?;
        
        Ok(())
    }
    
    pub async fn discover_peers_for_game(&self, game_type: GameType) -> Result<Vec<PeerId>, DiscoveryError> {
        let mut discovered_peers = Vec::new();
        
        // Try different discovery methods with timeout
        let discovery_timeout = Duration::from_secs(30);
        
        // Method 1: Local network discovery (fastest)
        let local_peers = timeout(
            discovery_timeout / 3,
            self.discover_local_peers(game_type)
        ).await.unwrap_or_else(|_| Ok(Vec::new()))?;
        
        discovered_peers.extend(local_peers);
        
        // Method 2: DHT lookup (medium speed, wide reach)
        let dht_peers = timeout(
            discovery_timeout / 3,
            self.dht_discovery.find_game_peers(game_type)
        ).await.unwrap_or_else(|_| Ok(Vec::new()))?;
        
        discovered_peers.extend(dht_peers);
        
        // Method 3: Bootstrap node referrals (slowest but most reliable)
        if discovered_peers.len() < self.discovery_config.min_peers {
            let bootstrap_peers = timeout(
                discovery_timeout / 3,
                self.get_peers_from_bootstrap_nodes(game_type)
            ).await.unwrap_or_else(|_| Ok(Vec::new()))?;
            
            discovered_peers.extend(bootstrap_peers);
        }
        
        // Remove duplicates and invalid peers
        discovered_peers.sort();
        discovered_peers.dedup();
        
        // Filter by compatibility and preferences
        let compatible_peers = self.filter_compatible_peers(discovered_peers, game_type).await?;
        
        Ok(compatible_peers)
    }
}
```

### Bluetooth Proximity Discovery

For mobile players, Bluetooth discovery finds nearby players without internet:

```rust
// From src/discovery/bluetooth_discovery.rs
pub struct BluetoothDiscovery {
    adapter: BluetoothAdapter,
    service_uuid: Uuid,
    discovery_cache: Arc<RwLock<HashMap<BluetoothAddress, DiscoveredPeer>>>,
    scan_config: BluetoothScanConfig,
}

impl BluetoothDiscovery {
    pub async fn start_scanning(&self) -> Result<(), BluetoothError> {
        // Start advertising our own service
        self.start_advertising().await?;
        
        // Start scanning for other BitCraps players
        let mut scan_stream = self.adapter.start_scan(ScanFilter {
            service_uuids: vec![self.service_uuid],
            tx_power_level: Some(TxPowerLevel::High),
            connectable: true,
        }).await?;
        
        while let Some(scan_result) = scan_stream.next().await {
            match scan_result {
                Ok(discovered_device) => {
                    if let Err(e) = self.handle_discovered_device(discovered_device).await {
                        eprintln!("Error handling discovered device: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Scan error: {}", e);
                    // Continue scanning despite errors
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_discovered_device(&self, device: DiscoveredDevice) -> Result<(), BluetoothError> {
        // Extract BitCraps-specific service data
        let service_data = device.service_data.get(&self.service_uuid)
            .ok_or(BluetoothError::NoServiceData)?;
        
        // Parse peer information from service data
        let peer_info = self.parse_peer_info(service_data)?;
        
        // Check if this peer is compatible with our games
        if self.is_compatible_peer(&peer_info)? {
            // Attempt to establish connection
            let connection = self.connect_to_peer(device.address, peer_info.clone()).await?;
            
            // Add to discovered peers
            let discovered_peer = DiscoveredPeer {
                peer_id: peer_info.peer_id,
                connection_info: ConnectionInfo::Bluetooth(connection),
                capabilities: peer_info.capabilities,
                signal_strength: device.rssi,
                discovered_at: Utc::now(),
                last_seen: Utc::now(),
            };
            
            self.discovery_cache.write().await.insert(device.address, discovered_peer);
            
            // Notify discovery manager
            self.notify_peer_discovered(peer_info.peer_id).await?;
        }
        
        Ok(())
    }
    
    async fn start_advertising(&self) -> Result<(), BluetoothError> {
        // Create advertisement data
        let peer_info = self.create_peer_info().await?;
        let service_data = self.serialize_peer_info(&peer_info)?;
        
        let advertisement = Advertisement::builder()
            .local_name("BitCraps Player")
            .service_uuid(self.service_uuid)
            .service_data(self.service_uuid, service_data)
            .connectable(true)
            .tx_power_level(TxPowerLevel::High)
            .build();
        
        // Start advertising
        self.adapter.start_advertising(advertisement).await?;
        
        // Schedule periodic advertisement updates
        self.schedule_advertisement_refresh().await?;
        
        Ok(())
    }
    
    fn serialize_peer_info(&self, peer_info: &PeerInfo) -> Result<Vec<u8>, BluetoothError> {
        let mut data = Vec::new();
        
        // Protocol version (1 byte)
        data.push(BITCRAPS_PROTOCOL_VERSION);
        
        // Peer capabilities flags (2 bytes)
        let capabilities = self.encode_capabilities(&peer_info.capabilities);
        data.extend_from_slice(&capabilities.to_le_bytes());
        
        // Current game types (variable length)
        let game_types = self.encode_game_types(&peer_info.active_games);
        data.push(game_types.len() as u8);
        data.extend_from_slice(&game_types);
        
        // Peer ID hash (4 bytes - truncated for space)
        let peer_id_hash = self.hash_peer_id(peer_info.peer_id);
        data.extend_from_slice(&peer_id_hash[..4]);
        
        // Network load indicator (1 byte)
        data.push(peer_info.network_load as u8);
        
        Ok(data)
    }
}
```

### DHT-Based Global Discovery

For internet-connected players, BitCraps uses a Distributed Hash Table for global discovery:

```rust
// From src/discovery/dht_discovery.rs
pub struct DHTDiscovery {
    kademlia_dht: KademliaDHT,
    node_id: NodeId,
    routing_table: RoutingTable,
    peer_store: PeerStore,
}

impl DHTDiscovery {
    pub async fn find_game_peers(&self, game_type: GameType) -> Result<Vec<PeerId>, DHTError> {
        // Create DHT key for this game type
        let game_key = self.create_game_key(game_type);
        
        // Search for peers announcing this game
        let search_results = self.kademlia_dht.iterative_find_value(game_key).await?;
        
        match search_results {
            FindResult::Value(peer_list) => {
                // Found cached peer list
                let peers: Vec<PeerAdvertisement> = bincode::deserialize(&peer_list)?;
                
                // Verify peers are still active
                let mut active_peers = Vec::new();
                for peer_ad in peers {
                    if self.verify_peer_active(&peer_ad).await? {
                        active_peers.push(peer_ad.peer_id);
                    }
                }
                
                Ok(active_peers)
            }
            
            FindResult::ClosestNodes(nodes) => {
                // No cached list, query closest nodes directly
                let mut found_peers = Vec::new();
                
                for node in nodes {
                    let peers = self.query_node_for_game_peers(node, game_type).await?;
                    found_peers.extend(peers);
                }
                
                Ok(found_peers)
            }
        }
    }
    
    pub async fn announce_game_participation(&self, game_type: GameType) -> Result<(), DHTError> {
        let game_key = self.create_game_key(game_type);
        
        // Create our advertisement
        let advertisement = PeerAdvertisement {
            peer_id: self.node_id.into(),
            game_type,
            endpoint: self.get_public_endpoint().await?,
            capabilities: self.get_our_capabilities(),
            load_factor: self.get_current_load().await?,
            announcement_time: Utc::now(),
            signature: self.sign_advertisement()?,
        };
        
        let serialized_ad = bincode::serialize(&advertisement)?;
        
        // Store in DHT with TTL
        self.kademlia_dht.store_value(
            game_key,
            serialized_ad,
            Duration::from_secs(300) // 5-minute TTL
        ).await?;
        
        // Also add to our local routing table
        self.routing_table.add_peer_for_game(game_type, self.node_id.into()).await;
        
        Ok(())
    }
    
    async fn query_node_for_game_peers(&self, 
        node: NodeInfo, 
        game_type: GameType
    ) -> Result<Vec<PeerId>, DHTError> {
        // Send direct query to node
        let query = PeerQuery {
            game_type,
            max_peers: 20,
            exclude_peers: vec![self.node_id.into()], // Don't include ourselves
            preferred_regions: self.get_preferred_regions(),
        };
        
        let response = self.send_peer_query(node, query).await?;
        
        // Validate response peers
        let mut valid_peers = Vec::new();
        for peer_info in response.peers {
            if self.validate_peer_info(&peer_info)? {
                valid_peers.push(peer_info.peer_id);
            }
        }
        
        Ok(valid_peers)
    }
    
    fn create_game_key(&self, game_type: GameType) -> DHTKey {
        // Create deterministic key for game type
        let key_material = format!("bitcraps:game:{:?}", game_type);
        let hash = blake3::hash(key_material.as_bytes());
        DHTKey::from_bytes(hash.as_bytes())
    }
}
```

### mDNS Local Network Discovery

For players on the same WiFi network, mDNS provides instant local discovery:

```rust
// From src/discovery/mdns_discovery.rs  
pub struct mDNSDiscovery {
    service_name: String,
    service_type: String,
    port: u16,
    mdns_responder: MDNSResponder,
    service_browser: ServiceBrowser,
}

impl mDNSDiscovery {
    pub async fn start_broadcasting(&self) -> Result<(), mDNSError> {
        // Register our service
        let service_info = ServiceInfo {
            name: self.service_name.clone(),
            service_type: self.service_type.clone(),
            port: self.port,
            txt_records: self.create_txt_records().await?,
            ttl: Duration::from_secs(120),
        };
        
        self.mdns_responder.register_service(service_info).await?;
        
        // Start browsing for other services
        let mut service_events = self.service_browser.browse(&self.service_type).await?;
        
        while let Some(event) = service_events.next().await {
            match event {
                ServiceEvent::ServiceFound(service) => {
                    if let Err(e) = self.handle_service_found(service).await {
                        eprintln!("Error handling found service: {}", e);
                    }
                }
                
                ServiceEvent::ServiceLost(service_name) => {
                    self.handle_service_lost(service_name).await;
                }
                
                ServiceEvent::ServiceUpdated(service) => {
                    if let Err(e) = self.handle_service_updated(service).await {
                        eprintln!("Error handling service update: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn create_txt_records(&self) -> Result<HashMap<String, String>, mDNSError> {
        let mut txt_records = HashMap::new();
        
        // Protocol version
        txt_records.insert("version".to_string(), BITCRAPS_VERSION.to_string());
        
        // Supported game types
        let supported_games = self.get_supported_games().await;
        let games_str = supported_games.iter()
            .map(|g| format!("{:?}", g))
            .collect::<Vec<_>>()
            .join(",");
        txt_records.insert("games".to_string(), games_str);
        
        // Current load
        let load = self.get_current_load().await?;
        txt_records.insert("load".to_string(), load.to_string());
        
        // Peer capabilities
        let capabilities = self.get_capabilities().await;
        txt_records.insert("caps".to_string(), capabilities.to_flags_string());
        
        // Region/timezone
        txt_records.insert("region".to_string(), self.get_region().await);
        
        Ok(txt_records)
    }
    
    async fn handle_service_found(&self, service: ServiceInfo) -> Result<(), mDNSError> {
        // Parse TXT records to get peer info
        let peer_info = self.parse_peer_info_from_txt(&service.txt_records)?;
        
        // Check compatibility
        if !self.is_compatible_peer(&peer_info) {
            return Ok(()); // Skip incompatible peers
        }
        
        // Try to connect
        let endpoint = format!("{}:{}", service.ip_address, service.port);
        let connection = self.establish_connection(endpoint, peer_info.clone()).await?;
        
        // Add to discovered peers
        let discovered_peer = DiscoveredPeer {
            peer_id: peer_info.peer_id,
            connection_info: ConnectionInfo::TCP(connection),
            capabilities: peer_info.capabilities,
            signal_strength: 100, // Local network = full strength
            discovered_at: Utc::now(),
            last_seen: Utc::now(),
        };
        
        // Notify discovery manager
        self.notify_peer_discovered(discovered_peer).await?;
        
        Ok(())
    }
}
```

## Part III: Bootstrap Node Strategy

Bootstrap nodes help new players find their first peers:

```rust
pub struct BootstrapNode {
    address: SocketAddr,
    public_key: PublicKey,
    region: Region,
    supported_games: Vec<GameType>,
    last_contact: Option<Instant>,
    reliability_score: f64,
}

impl PeerDiscoveryManager {
    async fn connect_to_bootstrap_nodes(&self) -> Result<(), BootstrapError> {
        // Sort bootstrap nodes by reliability and region preference
        let mut sorted_nodes = self.bootstrap_nodes.clone();
        sorted_nodes.sort_by(|a, b| {
            // Prefer nodes in our region
            let a_region_score = if a.region == self.get_our_region() { 1.0 } else { 0.5 };
            let b_region_score = if b.region == self.get_our_region() { 1.0 } else { 0.5 };
            
            // Combine region preference with reliability
            let a_score = a.reliability_score * a_region_score;
            let b_score = b.reliability_score * b_region_score;
            
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Try to connect to top bootstrap nodes
        let mut successful_connections = 0;
        let target_connections = 3; // Connect to at least 3 bootstrap nodes
        
        for bootstrap_node in sorted_nodes.iter().take(6) { // Try up to 6 nodes
            match self.connect_to_bootstrap_node(bootstrap_node).await {
                Ok(peers) => {
                    successful_connections += 1;
                    
                    // Add discovered peers to our peer list
                    for peer in peers {
                        self.add_discovered_peer(peer).await;
                    }
                    
                    if successful_connections >= target_connections {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to connect to bootstrap node {:?}: {}", 
                             bootstrap_node.address, e);
                    // Continue trying other nodes
                }
            }
        }
        
        if successful_connections == 0 {
            return Err(BootstrapError::NoBootstrapNodesAvailable);
        }
        
        Ok(())
    }
    
    async fn connect_to_bootstrap_node(&self, node: &BootstrapNode) -> Result<Vec<PeerInfo>, BootstrapError> {
        // Establish secure connection
        let mut connection = TcpStream::connect(node.address).await?;
        
        // Perform handshake
        let handshake = BootstrapHandshake {
            protocol_version: BITCRAPS_PROTOCOL_VERSION,
            client_public_key: self.get_our_public_key(),
            requested_games: self.get_games_we_want_to_play(),
            region_preference: self.get_our_region(),
        };
        
        self.send_handshake(&mut connection, handshake).await?;
        let response = self.receive_handshake_response(&mut connection).await?;
        
        // Verify bootstrap node's identity
        if !self.verify_bootstrap_node_signature(&response, node.public_key) {
            return Err(BootstrapError::InvalidBootstrapNode);
        }
        
        // Request peer list
        let peer_request = PeerListRequest {
            games: self.get_games_we_want_to_play(),
            max_peers: 50,
            exclude_regions: vec![], // We'll take peers from any region
        };
        
        self.send_peer_request(&mut connection, peer_request).await?;
        let peer_list = self.receive_peer_list(&mut connection).await?;
        
        // Update bootstrap node stats
        self.update_bootstrap_node_stats(node, true).await;
        
        Ok(peer_list.peers)
    }
}
```

## Part IV: Peer Quality Assessment

Not all discovered peers are equally good. BitCraps assesses peer quality:

```rust
pub struct PeerQualityAssessor {
    connection_tester: ConnectionTester,
    performance_metrics: HashMap<PeerId, PeerMetrics>,
    reputation_system: ReputationSystem,
}

impl PeerQualityAssessor {
    pub async fn assess_peer_quality(&self, peer_id: PeerId) -> Result<PeerQuality, AssessmentError> {
        // Test connection quality
        let connection_quality = self.test_connection_quality(peer_id).await?;
        
        // Check performance history
        let performance_score = self.calculate_performance_score(peer_id).await;
        
        // Check reputation
        let reputation_score = self.reputation_system.get_reputation(peer_id).await?;
        
        // Assess compatibility
        let compatibility_score = self.assess_compatibility(peer_id).await?;
        
        // Combine scores
        let overall_score = self.calculate_overall_score(
            connection_quality,
            performance_score,
            reputation_score,
            compatibility_score
        );
        
        Ok(PeerQuality {
            peer_id,
            overall_score,
            connection_quality,
            performance_score,
            reputation_score,
            compatibility_score,
            assessed_at: Utc::now(),
        })
    }
    
    async fn test_connection_quality(&self, peer_id: PeerId) -> Result<ConnectionQuality, TestError> {
        let start_time = Instant::now();
        
        // Ping test
        let ping_result = self.connection_tester.ping(peer_id).await?;
        let latency = ping_result.round_trip_time;
        
        // Bandwidth test
        let bandwidth_result = self.connection_tester.test_bandwidth(peer_id).await?;
        
        // Stability test (multiple pings over time)
        let stability_result = self.connection_tester.test_stability(peer_id, Duration::from_secs(30)).await?;
        
        let connection_quality = ConnectionQuality {
            latency,
            bandwidth_up: bandwidth_result.upload_speed,
            bandwidth_down: bandwidth_result.download_speed,
            packet_loss: stability_result.packet_loss_rate,
            jitter: stability_result.jitter,
            stability_score: stability_result.stability_score,
        };
        
        Ok(connection_quality)
    }
    
    async fn calculate_performance_score(&self, peer_id: PeerId) -> f64 {
        if let Some(metrics) = self.performance_metrics.get(&peer_id) {
            let mut score = 0.0;
            let mut weight_sum = 0.0;
            
            // Game completion rate (30% weight)
            score += metrics.game_completion_rate * 0.3;
            weight_sum += 0.3;
            
            // Response time (25% weight)
            let response_score = 1.0 - (metrics.average_response_time.as_millis() as f64 / 5000.0).min(1.0);
            score += response_score * 0.25;
            weight_sum += 0.25;
            
            // Uptime (20% weight)
            score += metrics.uptime_percentage * 0.2;
            weight_sum += 0.2;
            
            // Error rate (15% weight)
            let error_score = 1.0 - metrics.error_rate.min(1.0);
            score += error_score * 0.15;
            weight_sum += 0.15;
            
            // Protocol compliance (10% weight)
            score += metrics.protocol_compliance_score * 0.1;
            weight_sum += 0.1;
            
            if weight_sum > 0.0 {
                score / weight_sum
            } else {
                0.5 // Neutral score for new peers
            }
        } else {
            0.5 // Neutral score for unknown peers
        }
    }
}
```

## Part V: Adaptive Discovery Strategies

BitCraps adapts its discovery strategy based on current conditions:

```rust
pub struct AdaptiveDiscoveryEngine {
    current_strategy: DiscoveryStrategy,
    network_conditions: NetworkConditions,
    peer_requirements: PeerRequirements,
    strategy_effectiveness: HashMap<DiscoveryStrategy, f64>,
}

#[derive(Clone, Debug)]
pub enum DiscoveryStrategy {
    LocalFirst,      // Prioritize local network discovery
    GlobalFirst,     // Prioritize internet-based discovery
    Balanced,        // Equal weight to all methods
    PowerSaver,      // Minimize battery drain
    HighThroughput,  // Maximize peer discovery rate
}

impl AdaptiveDiscoveryEngine {
    pub async fn select_optimal_strategy(&mut self) -> DiscoveryStrategy {
        let conditions = self.assess_current_conditions().await;
        
        let optimal_strategy = match conditions {
            // On mobile with low battery - conserve power
            NetworkConditions {
                device_type: DeviceType::Mobile,
                battery_level: Some(level),
                ..
            } if level < 20 => DiscoveryStrategy::PowerSaver,
            
            // On local WiFi with good signal - prioritize local discovery
            NetworkConditions {
                wifi_available: true,
                wifi_signal_strength: Some(strength),
                cellular_available: false,
                ..
            } if strength > 0.7 => DiscoveryStrategy::LocalFirst,
            
            // Good internet connection - use global discovery
            NetworkConditions {
                internet_bandwidth: Some(bandwidth),
                internet_latency: Some(latency),
                ..
            } if bandwidth > 5_000_000 && latency < Duration::from_millis(100) => {
                DiscoveryStrategy::GlobalFirst
            },
            
            // Need many peers quickly - high throughput mode
            _ if self.peer_requirements.min_peers > 10 => DiscoveryStrategy::HighThroughput,
            
            // Default to balanced approach
            _ => DiscoveryStrategy::Balanced,
        };
        
        // Track effectiveness
        if self.current_strategy != optimal_strategy {
            self.track_strategy_change(&self.current_strategy, &optimal_strategy).await;
            self.current_strategy = optimal_strategy.clone();
        }
        
        optimal_strategy
    }
    
    pub async fn execute_discovery_strategy(&self, strategy: DiscoveryStrategy) -> Result<(), DiscoveryError> {
        match strategy {
            DiscoveryStrategy::LocalFirst => {
                // Start local discovery immediately
                tokio::spawn(self.bluetooth_discovery.start_scanning());
                tokio::spawn(self.mdns_discovery.start_broadcasting());
                
                // Delay global discovery
                tokio::time::sleep(Duration::from_secs(5)).await;
                tokio::spawn(self.dht_discovery.start_searching());
            }
            
            DiscoveryStrategy::GlobalFirst => {
                // Start global discovery immediately
                tokio::spawn(self.dht_discovery.start_searching());
                tokio::spawn(self.bootstrap_connection_manager.connect_to_bootstrap_nodes());
                
                // Start local discovery in parallel but with lower priority
                tokio::spawn(self.bluetooth_discovery.start_scanning());
                tokio::spawn(self.mdns_discovery.start_broadcasting());
            }
            
            DiscoveryStrategy::PowerSaver => {
                // Use longer intervals, fewer simultaneous scans
                let power_saver_config = DiscoveryConfig {
                    bluetooth_scan_interval: Duration::from_secs(60),
                    bluetooth_scan_duration: Duration::from_secs(10),
                    dht_query_interval: Duration::from_secs(120),
                    mdns_announcement_interval: Duration::from_secs(300),
                };
                
                self.apply_discovery_config(power_saver_config).await?;
            }
            
            DiscoveryStrategy::HighThroughput => {
                // Use aggressive discovery settings
                let high_throughput_config = DiscoveryConfig {
                    bluetooth_scan_interval: Duration::from_secs(5),
                    bluetooth_scan_duration: Duration::from_secs(30),
                    dht_query_interval: Duration::from_secs(10),
                    mdns_announcement_interval: Duration::from_secs(30),
                    max_concurrent_discoveries: 10,
                };
                
                self.apply_discovery_config(high_throughput_config).await?;
            }
            
            DiscoveryStrategy::Balanced => {
                // Use default settings with all methods active
                let balanced_config = DiscoveryConfig::default();
                self.apply_discovery_config(balanced_config).await?;
            }
        }
        
        Ok(())
    }
}
```

## Part VI: Privacy-Preserving Discovery

BitCraps protects player privacy during discovery:

```rust
pub struct PrivateDiscoveryManager {
    identity_manager: IdentityManager,
    discovery_anonymizer: DiscoveryAnonymizer,
    privacy_config: PrivacyConfig,
}

impl PrivateDiscoveryManager {
    pub async fn create_anonymous_advertisement(&self) -> Result<AnonymousAdvertisement, PrivacyError> {
        // Generate temporary identity for this discovery session
        let temp_identity = self.identity_manager.create_temporary_identity().await?;
        
        // Create advertisement without revealing real identity
        let anonymous_ad = AnonymousAdvertisement {
            session_id: temp_identity.session_id,
            game_capabilities: self.get_sanitized_capabilities(),
            region_hint: self.get_approximate_region(), // Country-level, not precise location
            performance_class: self.get_performance_class(), // General category, not exact specs
            protocol_version: BITCRAPS_PROTOCOL_VERSION,
            timestamp: Utc::now(),
            proof_of_work: self.generate_proof_of_work().await?,
        };
        
        Ok(anonymous_ad)
    }
    
    async fn sanitize_peer_info(&self, peer_info: &PeerInfo) -> SanitizedPeerInfo {
        SanitizedPeerInfo {
            // Hash real peer ID to prevent tracking
            anonymous_id: self.hash_peer_id(peer_info.peer_id),
            
            // Generalize capabilities to prevent fingerprinting
            capability_class: self.generalize_capabilities(&peer_info.capabilities),
            
            // Round timestamps to prevent timing correlation
            approximate_online_time: self.round_timestamp(peer_info.online_since),
            
            // Generalize network info
            network_class: self.classify_network(&peer_info.network_info),
        }
    }
    
    pub async fn perform_private_handshake(&self, peer_address: SocketAddr) -> Result<SecureConnection, HandshakeError> {
        // Use anonymous key exchange
        let temp_keypair = self.identity_manager.generate_temporary_keypair().await?;
        
        // Establish encrypted tunnel before revealing any real information
        let encrypted_connection = self.establish_encrypted_tunnel(peer_address, temp_keypair).await?;
        
        // Only after encryption is established, exchange real game information
        let secure_connection = self.negotiate_game_parameters(encrypted_connection).await?;
        
        Ok(secure_connection)
    }
}
```

## Part VII: Practical Discovery Exercise

Let's implement a simple peer discovery system:

**Exercise: Local Network Game Finder**

```rust
pub struct LocalGameFinder {
    our_games: Vec<GameType>,
    discovered_games: Arc<Mutex<HashMap<GameType, Vec<LocalPeer>>>>,
    discovery_socket: UdpSocket,
}

impl LocalGameFinder {
    pub async fn new(games: Vec<GameType>) -> Result<Self, std::io::Error> {
        let discovery_socket = UdpSocket::bind("0.0.0.0:8080").await?;
        discovery_socket.set_broadcast(true)?;
        
        Ok(LocalGameFinder {
            our_games: games,
            discovered_games: Arc::new(Mutex::new(HashMap::new())),
            discovery_socket,
        })
    }
    
    pub async fn start_discovery(&self) -> Result<(), DiscoveryError> {
        // Start broadcasting our games
        let broadcast_task = self.start_broadcasting();
        
        // Start listening for other players
        let listening_task = self.start_listening();
        
        // Run both tasks concurrently
        tokio::try_join!(broadcast_task, listening_task)?;
        
        Ok(())
    }
    
    async fn start_broadcasting(&self) -> Result<(), DiscoveryError> {
        let broadcast_addr = "255.255.255.255:8080";
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            let announcement = GameAnnouncement {
                player_id: self.get_our_player_id(),
                games: self.our_games.clone(),
                endpoint: self.get_our_endpoint().await?,
                timestamp: Utc::now(),
            };
            
            let message = bincode::serialize(&announcement)?;
            
            if let Err(e) = self.discovery_socket.send_to(&message, broadcast_addr).await {
                eprintln!("Failed to send broadcast: {}", e);
            }
        }
    }
    
    async fn start_listening(&self) -> Result<(), DiscoveryError> {
        let mut buffer = [0u8; 1024];
        
        loop {
            match self.discovery_socket.recv_from(&mut buffer).await {
                Ok((len, sender_addr)) => {
                    if let Ok(announcement) = bincode::deserialize::<GameAnnouncement>(&buffer[..len]) {
                        // Don't process our own announcements
                        if announcement.player_id != self.get_our_player_id() {
                            self.handle_game_announcement(announcement, sender_addr).await;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to receive announcement: {}", e);
                }
            }
        }
    }
    
    async fn handle_game_announcement(&self, announcement: GameAnnouncement, sender: SocketAddr) {
        let mut discovered = self.discovered_games.lock().await;
        
        let local_peer = LocalPeer {
            player_id: announcement.player_id,
            endpoint: announcement.endpoint,
            discovered_at: Utc::now(),
            last_seen: Utc::now(),
            sender_address: sender,
        };
        
        // Add peer to each game type they support
        for game_type in announcement.games {
            discovered.entry(game_type)
                .or_insert_with(Vec::new)
                .push(local_peer.clone());
        }
        
        // Clean up old entries
        self.cleanup_stale_peers(&mut discovered).await;
    }
    
    pub async fn get_peers_for_game(&self, game_type: GameType) -> Vec<LocalPeer> {
        let discovered = self.discovered_games.lock().await;
        discovered.get(&game_type).cloned().unwrap_or_default()
    }
    
    async fn cleanup_stale_peers(&self, discovered: &mut HashMap<GameType, Vec<LocalPeer>>) {
        let stale_threshold = Utc::now() - chrono::Duration::seconds(30);
        
        for (_, peers) in discovered.iter_mut() {
            peers.retain(|peer| peer.last_seen > stale_threshold);
        }
    }
}

#[tokio::test]
async fn test_local_game_discovery() {
    // Start two game finders
    let games1 = vec![GameType::Craps];
    let games2 = vec![GameType::Craps, GameType::Poker];
    
    let finder1 = LocalGameFinder::new(games1).await.unwrap();
    let finder2 = LocalGameFinder::new(games2).await.unwrap();
    
    // Start discovery on both
    tokio::spawn(finder1.start_discovery());
    tokio::spawn(finder2.start_discovery());
    
    // Wait for discovery
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // Each should discover the other
    let peers1 = finder1.get_peers_for_game(GameType::Craps).await;
    let peers2 = finder2.get_peers_for_game(GameType::Craps).await;
    
    assert!(!peers1.is_empty());
    assert!(!peers2.is_empty());
}
```

## Conclusion: Discovery as the Foundation of P2P Networks

Peer discovery is what makes peer-to-peer networks possible. Without it, you have isolated nodes that can't find each other. The key insights:

1. **Use multiple discovery methods** - No single method works in all situations
2. **Adapt to conditions** - Battery, network, and location all matter
3. **Assess peer quality** - Not all discovered peers are worth connecting to
4. **Protect privacy** - Don't reveal more than necessary during discovery
5. **Handle failures gracefully** - Discovery methods will fail, have backups

In BitCraps, peer discovery is what allows players to find games, whether they're on the same WiFi network or across the globe. It's the first step in creating the distributed gaming network that makes real-money peer-to-peer gaming possible.

Remember: A peer-to-peer network is only as strong as its discovery mechanisms. Great discovery leads to great connections, which lead to great games.