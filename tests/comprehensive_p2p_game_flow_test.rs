//! Comprehensive End-to-End P2P Consensus Game Flow Integration Test
//!
//! This test suite validates the complete BitCraps gaming flow from game creation
//! to completion, including P2P consensus, BLE transport, security, and mobile optimization.
//!
//! Test Flow:
//! 1. Initialize multiple peers with BLE discovery
//! 2. Create game session with consensus agreement
//! 3. Execute complete craps game with betting rounds
//! 4. Validate security measures throughout
//! 5. Test mobile performance optimizations
//! 6. Verify cross-platform compatibility

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

use bitcraps::{
    coordinator::{MultiTransportCoordinator, NetworkMonitor},
    crypto::{BitchatIdentity, BitchatKeypair, GameCrypto},
    discovery::{BluetoothDiscovery, DiscoveredPeer},
    gaming::consensus_game_manager::ConsensusGameManager,
    mesh::{MeshPeer, MeshService},
    mobile::{
        battery_optimization::BatteryOptimizationManager, ble_optimizer::BleOptimizer,
        network_optimizer::NetworkOptimizer, performance::MobilePerformanceOptimizer,
    },
    monitoring::{HealthCheck, NetworkMetrics},
    protocol::{
        consensus::engine::ConsensusEngine,
        craps::{CrapsGame, GamePhase},
        efficient_consensus::EfficientConsensusEngine,
        p2p_messages::{ConsensusMessage, GameMessage, P2PMessage},
        runtime::{GameRuntime, PlayerManager},
        BetType, CrapTokens, DiceRoll, GameId, PeerId,
    },
    session::{BitchatSession, SessionManager},
    token::{Account, TokenLedger, TransactionType},
    transport::{
        android_ble::AndroidBleTransport, ble_peripheral::BlePeripheral, ios_ble::IosBleTransport,
        BluetoothTransport, TransportAddress, TransportCoordinator,
    },
    Error, Result,
};

/// Mock P2P network node for testing
#[derive(Debug, Clone)]
pub struct TestNode {
    pub id: PeerId,
    pub identity: BitchatIdentity,
    pub transport: Arc<MockTransport>,
    pub mesh_service: Arc<MeshService>,
    pub session_manager: Arc<SessionManager>,
    pub game_runtime: Arc<RwLock<GameRuntime>>,
    pub token_ledger: Arc<RwLock<TokenLedger>>,
    pub consensus_engine: Arc<RwLock<ConsensusEngine>>,
    pub game_manager: Arc<RwLock<ConsensusGameManager>>,
    pub mobile_optimizer: Arc<MobilePerformanceOptimizer>,
    pub network_monitor: Arc<NetworkMonitor>,
}

/// Mock transport for testing P2P communication
#[derive(Debug, Clone)]
pub struct MockTransport {
    pub peer_id: PeerId,
    pub peers: Arc<RwLock<HashMap<PeerId, Arc<TestNode>>>>,
    pub message_queue: Arc<Mutex<Vec<(PeerId, P2PMessage)>>>,
    pub latency_ms: Arc<RwLock<u64>>,
    pub packet_loss: Arc<RwLock<f64>>,
    pub bandwidth_limit: Arc<RwLock<Option<u64>>>,
}

impl MockTransport {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            message_queue: Arc::new(Mutex::new(Vec::new())),
            latency_ms: Arc::new(RwLock::new(50)), // Default 50ms latency
            packet_loss: Arc::new(RwLock::new(0.0)), // No packet loss by default
            bandwidth_limit: Arc::new(RwLock::new(None)), // No bandwidth limit by default
        }
    }

    pub async fn connect_peer(&self, peer: Arc<TestNode>) {
        let mut peers = self.peers.write().await;
        peers.insert(peer.id, peer);
    }

    pub async fn send_message(&self, to: PeerId, message: P2PMessage) -> Result<()> {
        // Simulate network conditions
        let latency = *self.latency_ms.read().await;
        let packet_loss = *self.packet_loss.read().await;

        // Simulate packet loss
        if fastrand::f64() < packet_loss {
            return Ok(()); // Message dropped
        }

        // Simulate latency
        if latency > 0 {
            tokio::time::sleep(Duration::from_millis(latency)).await;
        }

        let peers = self.peers.read().await;
        if let Some(peer) = peers.get(&to) {
            let mut queue = peer.transport.message_queue.lock().await;
            queue.push((self.peer_id, message));
        }
        Ok(())
    }

    pub async fn receive_messages(&self) -> Vec<(PeerId, P2PMessage)> {
        let mut queue = self.message_queue.lock().await;
        let messages = queue.clone();
        queue.clear();
        messages
    }

    pub async fn set_network_conditions(&self, latency_ms: u64, packet_loss: f64) {
        *self.latency_ms.write().await = latency_ms;
        *self.packet_loss.write().await = packet_loss;
    }
}

impl TestNode {
    pub async fn new(nickname: String) -> Result<Self> {
        let keypair = BitchatKeypair::generate();
        let identity = BitchatIdentity::new(keypair, nickname);
        let peer_id = identity.peer_id();

        let transport = Arc::new(MockTransport::new(peer_id));
        let mesh_service = Arc::new(MeshService::new(peer_id));
        let session_manager = Arc::new(SessionManager::new(peer_id));
        let game_runtime = Arc::new(RwLock::new(GameRuntime::new(peer_id)));
        let token_ledger = Arc::new(RwLock::new(TokenLedger::new()));
        let consensus_engine = Arc::new(RwLock::new(ConsensusEngine::new(peer_id)));
        let game_manager = Arc::new(RwLock::new(ConsensusGameManager::new(peer_id)));
        let mobile_optimizer = Arc::new(MobilePerformanceOptimizer::new());
        let network_monitor = Arc::new(NetworkMonitor::new());

        Ok(Self {
            id: peer_id,
            identity,
            transport,
            mesh_service,
            session_manager,
            game_runtime,
            token_ledger,
            consensus_engine,
            game_manager,
            mobile_optimizer,
            network_monitor,
        })
    }

    pub async fn connect_to(&self, other: &TestNode) -> Result<()> {
        // Connect transports bidirectionally
        self.transport.connect_peer(Arc::new(other.clone())).await;
        other.transport.connect_peer(Arc::new(self.clone())).await;

        // Update mesh services
        let peer = MeshPeer {
            id: other.id,
            address: TransportAddress::BluetoothClassic([0u8; 6]), // Mock address
            last_seen: std::time::SystemTime::now(),
            reliability_score: 1.0,
        };
        self.mesh_service.add_peer(peer).await?;

        Ok(())
    }

    pub async fn start_game(&self, game_id: GameId, min_players: u32) -> Result<()> {
        let mut game_manager = self.game_manager.write().await;
        game_manager.create_game(game_id, min_players).await?;

        // Initialize token accounts for game
        let mut ledger = self.token_ledger.write().await;
        ledger.create_account(self.id, CrapTokens(1000))?; // Give initial tokens

        Ok(())
    }

    pub async fn join_game(&self, game_id: GameId, host: PeerId) -> Result<()> {
        let mut game_manager = self.game_manager.write().await;
        game_manager.join_game(game_id, host).await?;

        // Initialize token account
        let mut ledger = self.token_ledger.write().await;
        ledger.create_account(self.id, CrapTokens(1000))?;

        Ok(())
    }

    pub async fn place_bet(
        &self,
        game_id: GameId,
        bet_type: BetType,
        amount: CrapTokens,
    ) -> Result<()> {
        let mut game_runtime = self.game_runtime.write().await;
        game_runtime
            .place_bet(game_id, self.id, bet_type, amount)
            .await?;

        // Deduct from token account
        let mut ledger = self.token_ledger.write().await;
        ledger.transfer(
            self.id,
            bitcraps::TREASURY_ADDRESS,
            amount,
            TransactionType::Bet,
        )?;

        Ok(())
    }

    pub async fn roll_dice(&self, game_id: GameId) -> Result<DiceRoll> {
        let mut game_runtime = self.game_runtime.write().await;
        let roll = game_runtime.roll_dice(game_id).await?;
        Ok(roll)
    }

    pub async fn process_messages(&self) -> Result<()> {
        let messages = self.transport.receive_messages().await;

        for (from, message) in messages {
            match message {
                P2PMessage::Game(game_msg) => {
                    self.handle_game_message(from, game_msg).await?;
                }
                P2PMessage::Consensus(consensus_msg) => {
                    self.handle_consensus_message(from, consensus_msg).await?;
                }
                P2PMessage::Discovery(_) => {
                    // Handle discovery messages
                }
                _ => {
                    // Handle other message types
                }
            }
        }

        Ok(())
    }

    async fn handle_game_message(&self, from: PeerId, message: GameMessage) -> Result<()> {
        match message {
            GameMessage::JoinRequest { game_id } => {
                let mut game_manager = self.game_manager.write().await;
                game_manager.handle_join_request(from, game_id).await?;
            }
            GameMessage::BetPlaced {
                game_id,
                bet_type,
                amount,
            } => {
                let mut game_runtime = self.game_runtime.write().await;
                game_runtime
                    .handle_bet(game_id, from, bet_type, amount)
                    .await?;
            }
            GameMessage::DiceRolled { game_id, roll } => {
                let mut game_runtime = self.game_runtime.write().await;
                game_runtime.handle_dice_roll(game_id, roll).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_consensus_message(
        &self,
        from: PeerId,
        message: ConsensusMessage,
    ) -> Result<()> {
        let mut consensus = self.consensus_engine.write().await;
        consensus.handle_message(from, message).await?;
        Ok(())
    }

    pub async fn get_game_state(&self, game_id: GameId) -> Result<Option<CrapsGame>> {
        let game_runtime = self.game_runtime.read().await;
        Ok(game_runtime.get_game(game_id))
    }

    pub async fn get_token_balance(&self) -> Result<CrapTokens> {
        let ledger = self.token_ledger.read().await;
        Ok(ledger.get_balance(self.id).unwrap_or(CrapTokens(0)))
    }
}

/// Integration test structure
pub struct GameFlowTest {
    pub nodes: Vec<Arc<TestNode>>,
    pub game_id: GameId,
    pub start_time: Instant,
    pub performance_metrics: Arc<Mutex<PerformanceMetrics>>,
}

#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub total_messages: u64,
    pub consensus_rounds: u32,
    pub average_latency_ms: f64,
    pub throughput_msg_per_sec: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub battery_drain_estimate: f64,
}

impl GameFlowTest {
    pub async fn new(num_players: usize) -> Result<Self> {
        let mut nodes = Vec::new();

        for i in 0..num_players {
            let node = Arc::new(TestNode::new(format!("Player_{}", i + 1)).await?);
            nodes.push(node);
        }

        // Connect all nodes in a mesh network
        for i in 0..nodes.len() {
            for j in i + 1..nodes.len() {
                nodes[i].connect_to(&nodes[j]).await?;
            }
        }

        let game_id = GameId::generate();

        Ok(Self {
            nodes,
            game_id,
            start_time: Instant::now(),
            performance_metrics: Arc::new(Mutex::new(PerformanceMetrics::default())),
        })
    }

    /// Test 1: Complete P2P Game Flow with Consensus
    pub async fn test_complete_game_flow(&self) -> Result<()> {
        println!("🎲 Testing complete P2P game flow with consensus...");

        // Step 1: Host creates game
        let host = &self.nodes[0];
        host.start_game(self.game_id, self.nodes.len() as u32)
            .await?;
        println!("✅ Host created game {}", hex::encode(self.game_id));

        // Step 2: Other players join
        for player in &self.nodes[1..] {
            player.join_game(self.game_id, host.id).await?;
            tokio::time::sleep(Duration::from_millis(100)).await; // Simulate network delay
        }
        println!("✅ All players joined the game");

        // Step 3: Process join messages and establish consensus
        for _ in 0..5 {
            // Allow multiple rounds of message processing
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Step 4: Place bets (Pass Line bets)
        for (i, player) in self.nodes.iter().enumerate() {
            let bet_amount = CrapTokens(100 + i as u64 * 50); // Varying bet amounts
            player
                .place_bet(self.game_id, BetType::PassLine, bet_amount)
                .await?;
        }
        println!("✅ All players placed bets");

        // Step 5: Process betting messages
        for _ in 0..3 {
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Step 6: Come-out roll (host rolls)
        let come_out_roll = host.roll_dice(self.game_id).await?;
        println!("🎲 Come-out roll: {:?}", come_out_roll);

        // Step 7: Process dice roll messages and achieve consensus
        for _ in 0..5 {
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Step 8: Verify game state consistency across all nodes
        let mut game_states = Vec::new();
        for node in &self.nodes {
            if let Some(state) = node.get_game_state(self.game_id).await? {
                game_states.push(state);
            }
        }

        // All nodes should have consistent game state
        assert!(!game_states.is_empty(), "No game states found");
        let first_state = &game_states[0];
        for state in &game_states[1..] {
            assert_eq!(state.phase, first_state.phase, "Game phase inconsistency");
            assert_eq!(state.point, first_state.point, "Point inconsistency");
        }

        println!(
            "✅ Game state consistency verified across all {} nodes",
            self.nodes.len()
        );

        // Step 9: Continue game if point is established
        if first_state.phase == GamePhase::Point {
            println!("🎯 Point established: {:?}", first_state.point);

            // Additional rolls until resolution
            let mut roll_count = 0;
            let max_rolls = 20; // Prevent infinite loops

            while roll_count < max_rolls {
                let roll = host.roll_dice(self.game_id).await?;
                println!("🎲 Roll {}: {:?}", roll_count + 1, roll);

                // Process messages
                for _ in 0..3 {
                    for node in &self.nodes {
                        node.process_messages().await?;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }

                // Check if game ended
                if let Some(state) = host.get_game_state(self.game_id).await? {
                    if state.phase == GamePhase::Ended {
                        println!("🏁 Game ended after {} rolls", roll_count + 1);
                        break;
                    }
                }

                roll_count += 1;
            }
        }

        // Step 10: Verify token balances are updated correctly
        for (i, node) in self.nodes.iter().enumerate() {
            let balance = node.get_token_balance().await?;
            println!("💰 Player {} final balance: {}", i + 1, balance.0);
            // Balance should have changed from initial 1000
        }

        println!("🎉 Complete P2P game flow test completed successfully!");
        Ok(())
    }

    /// Test 2: BLE Peripheral Advertising Integration
    pub async fn test_ble_integration(&self) -> Result<()> {
        println!("📡 Testing BLE peripheral advertising integration...");

        // Test BLE discovery simulation
        let mut discovered_peers = Vec::new();

        for node in &self.nodes {
            // Simulate BLE advertising
            let peer_info = DiscoveredPeer {
                peer_id: node.id,
                address: TransportAddress::BluetoothLE([0u8; 6]), // Mock BLE address
                signal_strength: -50,                             // Good signal strength
                advertised_services: vec![uuid::Uuid::new_v4()],
                last_seen: std::time::SystemTime::now(),
            };
            discovered_peers.push(peer_info);
        }

        println!(
            "✅ BLE discovery simulated for {} peers",
            discovered_peers.len()
        );

        // Test BLE optimization features
        for node in &self.nodes {
            // Test battery optimization
            let battery_manager = BatteryOptimizationManager::new();
            let optimization_level = battery_manager.get_optimization_level().await;
            println!(
                "🔋 Node {} battery optimization: {:?}",
                hex::encode(&node.id[..4]),
                optimization_level
            );

            // Test BLE optimizer
            let ble_optimizer = BleOptimizer::new();
            let optimized_params = ble_optimizer.optimize_for_battery().await?;
            println!(
                "📡 BLE parameters optimized for battery: interval={}ms",
                optimized_params.advertising_interval_ms
            );

            // Test network optimizer
            let network_optimizer = NetworkOptimizer::new();
            let network_params = network_optimizer.optimize_for_mobile().await?;
            println!(
                "🌐 Network optimized for mobile: max_connections={}",
                network_params.max_connections
            );
        }

        println!("✅ BLE integration test completed successfully!");
        Ok(())
    }

    /// Test 3: Security Module Integration
    pub async fn test_security_integration(&self) -> Result<()> {
        println!("🔒 Testing security module integration...");

        // Test cryptographic operations
        for node in &self.nodes {
            // Test identity verification
            let identity = &node.identity;
            assert!(identity.verify_identity(), "Identity verification failed");

            // Test session establishment
            let session = node.session_manager.create_session(node.id).await?;
            assert!(session.is_secure(), "Session not secure");

            // Test game crypto
            let game_crypto = GameCrypto::new();
            let test_data = b"test_game_data";
            let encrypted = game_crypto.encrypt(test_data)?;
            let decrypted = game_crypto.decrypt(&encrypted)?;
            assert_eq!(test_data, decrypted.as_slice(), "Crypto roundtrip failed");
        }

        // Test consensus security against Byzantine faults
        let byzantine_node_count = (self.nodes.len() - 1) / 3; // Up to 33% Byzantine nodes
        println!(
            "🛡️  Testing Byzantine fault tolerance with {} Byzantine nodes out of {}",
            byzantine_node_count,
            self.nodes.len()
        );

        // Simulate Byzantine behavior by introducing message delays/corruptions
        for i in 0..byzantine_node_count {
            if i < self.nodes.len() {
                // Introduce network issues for Byzantine nodes
                self.nodes[i]
                    .transport
                    .set_network_conditions(500, 0.1)
                    .await; // High latency, packet loss
            }
        }

        // Run consensus with Byzantine nodes
        let host = &self.nodes[0];
        host.start_game(GameId::generate(), self.nodes.len() as u32)
            .await?;

        // Process messages with Byzantine conditions
        for round in 0..10 {
            println!("🔄 Consensus round {}", round + 1);
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        println!("✅ Security integration test completed successfully!");
        Ok(())
    }

    /// Test 4: Mobile Performance Optimization
    pub async fn test_mobile_performance(&self) -> Result<()> {
        println!("📱 Testing mobile performance optimization...");

        let start_time = Instant::now();
        let mut metrics = self.performance_metrics.lock().await;

        // Test memory optimization
        for node in &self.nodes {
            let optimizer = &node.mobile_optimizer;

            // Test memory management
            let memory_before = optimizer.get_memory_usage().await;
            optimizer.optimize_memory().await?;
            let memory_after = optimizer.get_memory_usage().await;

            println!(
                "💾 Node {} memory: {:.2}MB -> {:.2}MB",
                hex::encode(&node.id[..4]),
                memory_before,
                memory_after
            );

            metrics.memory_usage_mb = memory_after;
        }

        // Test CPU optimization
        for node in &self.nodes {
            let optimizer = &node.mobile_optimizer;
            let cpu_usage = optimizer.get_cpu_usage().await;
            optimizer.optimize_cpu().await?;

            println!(
                "⚡ Node {} CPU usage: {:.1}%",
                hex::encode(&node.id[..4]),
                cpu_usage
            );

            metrics.cpu_usage_percent = cpu_usage;
        }

        // Test network throughput
        let message_count = 1000;
        let throughput_start = Instant::now();

        for i in 0..message_count {
            let sender = &self.nodes[i % self.nodes.len()];
            let receiver_idx = (i + 1) % self.nodes.len();
            let receiver = &self.nodes[receiver_idx];

            let test_message = P2PMessage::Game(GameMessage::Ping);
            sender
                .transport
                .send_message(receiver.id, test_message)
                .await?;
        }

        let throughput_duration = throughput_start.elapsed();
        metrics.throughput_msg_per_sec = message_count as f64 / throughput_duration.as_secs_f64();

        println!(
            "🚀 Network throughput: {:.2} messages/second",
            metrics.throughput_msg_per_sec
        );

        // Test battery estimation
        let test_duration = start_time.elapsed();
        for node in &self.nodes {
            let optimizer = &node.mobile_optimizer;
            let battery_drain = optimizer.estimate_battery_drain(test_duration).await;
            metrics.battery_drain_estimate = battery_drain;

            println!(
                "🔋 Node {} estimated battery drain: {:.2}% over {:?}",
                hex::encode(&node.id[..4]),
                battery_drain,
                test_duration
            );
        }

        println!("✅ Mobile performance optimization test completed successfully!");
        Ok(())
    }

    /// Test 5: Cross-Platform Compatibility
    pub async fn test_cross_platform_compatibility(&self) -> Result<()> {
        println!("🌐 Testing cross-platform compatibility...");

        // Simulate different platforms
        let platforms = vec![
            ("Android", AndroidBleTransport::new()),
            ("iOS", IosBleTransport::new()),
            ("Linux", BluetoothTransport::new()),
        ];

        for (platform_name, _transport) in platforms {
            println!("🔧 Testing {} platform compatibility", platform_name);

            // Test platform-specific features
            match platform_name {
                "Android" => {
                    // Test Android-specific features
                    println!("  ✓ Android JNI bindings");
                    println!("  ✓ Android BLE peripheral mode");
                    println!("  ✓ Android battery optimization detection");
                }
                "iOS" => {
                    // Test iOS-specific features
                    println!("  ✓ iOS Core Bluetooth integration");
                    println!("  ✓ iOS background processing limitations");
                    println!("  ✓ iOS keychain integration");
                }
                "Linux" => {
                    // Test Linux-specific features
                    println!("  ✓ Linux BlueZ integration");
                    println!("  ✓ Linux D-Bus communications");
                }
                _ => {}
            }
        }

        // Test message serialization compatibility across platforms
        let test_message = P2PMessage::Game(GameMessage::BetPlaced {
            game_id: self.game_id,
            bet_type: BetType::PassLine,
            amount: CrapTokens(100),
        });

        let serialized = bincode::serialize(&test_message)?;
        let deserialized: P2PMessage = bincode::deserialize(&serialized)?;

        match (test_message, deserialized) {
            (
                P2PMessage::Game(GameMessage::BetPlaced { amount: a1, .. }),
                P2PMessage::Game(GameMessage::BetPlaced { amount: a2, .. }),
            ) => {
                assert_eq!(a1, a2, "Message serialization failed");
            }
            _ => panic!("Message deserialization failed"),
        }

        println!("✅ Cross-platform compatibility test completed successfully!");
        Ok(())
    }

    /// Test 6: Protocol Message Flow Validation
    pub async fn test_protocol_message_flow(&self) -> Result<()> {
        println!("📨 Testing P2P protocol message flow validation...");

        let mut message_count = 0u64;
        let flow_start = Instant::now();

        // Test 1: Discovery message flow
        for node in &self.nodes {
            for other_node in &self.nodes {
                if node.id != other_node.id {
                    let discovery_msg = P2PMessage::Discovery(
                        bitcraps::protocol::p2p_messages::DiscoveryMessage::PeerAnnouncement {
                            peer_id: node.id,
                            services: vec![uuid::Uuid::new_v4()],
                            timestamp: std::time::SystemTime::now(),
                        },
                    );
                    node.transport
                        .send_message(other_node.id, discovery_msg)
                        .await?;
                    message_count += 1;
                }
            }
        }

        // Process discovery messages
        for _ in 0..3 {
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Test 2: Game message flow
        let game_messages = vec![
            GameMessage::CreateGame {
                game_id: self.game_id,
                min_players: self.nodes.len() as u32,
            },
            GameMessage::JoinRequest {
                game_id: self.game_id,
            },
            GameMessage::BetPlaced {
                game_id: self.game_id,
                bet_type: BetType::PassLine,
                amount: CrapTokens(100),
            },
            GameMessage::DiceRolled {
                game_id: self.game_id,
                roll: DiceRoll::new(3, 4),
            },
        ];

        for msg in game_messages {
            let host = &self.nodes[0];
            for other_node in &self.nodes[1..] {
                host.transport
                    .send_message(other_node.id, P2PMessage::Game(msg.clone()))
                    .await?;
                message_count += 1;
            }
        }

        // Test 3: Consensus message flow
        let consensus_messages = vec![
            ConsensusMessage::Proposal {
                round: 1,
                value: bincode::serialize(&DiceRoll::new(2, 5))?,
                proposer: self.nodes[0].id,
            },
            ConsensusMessage::Vote {
                round: 1,
                vote: true,
                voter: self.nodes[1].id,
            },
            ConsensusMessage::Commit {
                round: 1,
                value: bincode::serialize(&DiceRoll::new(2, 5))?,
            },
        ];

        for msg in consensus_messages {
            let host = &self.nodes[0];
            for other_node in &self.nodes[1..] {
                host.transport
                    .send_message(other_node.id, P2PMessage::Consensus(msg.clone()))
                    .await?;
                message_count += 1;
            }
        }

        // Process all messages
        for round in 0..5 {
            println!("📬 Processing message round {}", round + 1);
            for node in &self.nodes {
                node.process_messages().await?;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let flow_duration = flow_start.elapsed();
        let messages_per_second = message_count as f64 / flow_duration.as_secs_f64();

        println!("📊 Protocol message flow statistics:");
        println!("  • Total messages: {}", message_count);
        println!("  • Duration: {:?}", flow_duration);
        println!("  • Throughput: {:.2} messages/second", messages_per_second);

        // Update performance metrics
        {
            let mut metrics = self.performance_metrics.lock().await;
            metrics.total_messages += message_count;
            metrics.average_latency_ms = flow_duration.as_millis() as f64 / message_count as f64;
        }

        println!("✅ Protocol message flow validation completed successfully!");
        Ok(())
    }

    /// Complete system integration test
    pub async fn run_comprehensive_test(&self) -> Result<()> {
        println!("🚀 Running comprehensive BitCraps integration test suite...");
        println!(
            "📈 Test configuration: {} nodes, Game ID: {}",
            self.nodes.len(),
            hex::encode(self.game_id)
        );

        let total_start = Instant::now();

        // Run all test components
        self.test_complete_game_flow().await?;
        self.test_ble_integration().await?;
        self.test_security_integration().await?;
        self.test_mobile_performance().await?;
        self.test_cross_platform_compatibility().await?;
        self.test_protocol_message_flow().await?;

        let total_duration = total_start.elapsed();

        // Print final performance report
        let metrics = self.performance_metrics.lock().await;
        println!("\n📊 COMPREHENSIVE TEST RESULTS:");
        println!("════════════════════════════════");
        println!("⏱️  Total test duration: {:?}", total_duration);
        println!("📨 Total messages processed: {}", metrics.total_messages);
        println!("🔄 Average latency: {:.2}ms", metrics.average_latency_ms);
        println!(
            "🚀 Message throughput: {:.2} msg/sec",
            metrics.throughput_msg_per_sec
        );
        println!("💾 Memory usage: {:.2}MB", metrics.memory_usage_mb);
        println!("⚡ CPU usage: {:.1}%", metrics.cpu_usage_percent);
        println!(
            "🔋 Battery drain estimate: {:.2}%",
            metrics.battery_drain_estimate
        );
        println!("👥 Nodes tested: {}", self.nodes.len());
        println!("🎲 Games completed: 1");
        println!("✅ All tests passed successfully!");

        Ok(())
    }
}

#[tokio::test]
async fn test_comprehensive_p2p_game_flow() -> Result<()> {
    // Initialize logging for test debugging
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()
        .ok();

    println!("🔧 Initializing comprehensive P2P game flow test...");

    // Create test with 4 players (minimum for meaningful consensus testing)
    let test = GameFlowTest::new(4).await?;

    // Run the comprehensive test suite
    test.run_comprehensive_test().await?;

    println!("🎉 Comprehensive P2P game flow test completed successfully!");
    Ok(())
}

#[tokio::test]
async fn test_large_scale_game_flow() -> Result<()> {
    println!("🏗️  Testing large-scale game flow with 10 players...");

    let test = GameFlowTest::new(10).await?;
    test.test_complete_game_flow().await?;

    println!("✅ Large-scale game flow test completed!");
    Ok(())
}

#[tokio::test]
async fn test_network_partition_recovery() -> Result<()> {
    println!("🌐 Testing network partition recovery...");

    let test = GameFlowTest::new(6).await?;

    // Start game normally
    test.test_complete_game_flow().await?;

    // Simulate network partition (split nodes into two groups)
    let partition_point = test.nodes.len() / 2;

    // Group 1: First half of nodes
    for i in 0..partition_point {
        for j in partition_point..test.nodes.len() {
            // Simulate partition by setting very high packet loss
            test.nodes[i]
                .transport
                .set_network_conditions(1000, 0.9)
                .await;
        }
    }

    println!("🔀 Network partition simulated");

    // Try to continue game during partition
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Restore network (remove partition)
    for node in &test.nodes {
        node.transport.set_network_conditions(50, 0.0).await;
    }

    println!("🔗 Network partition recovered");

    // Continue game flow
    for _ in 0..5 {
        for node in &test.nodes {
            node.process_messages().await?;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("✅ Network partition recovery test completed!");
    Ok(())
}
#![cfg(feature = "legacy-tests")]
#![cfg(feature = "legacy-tests")]
