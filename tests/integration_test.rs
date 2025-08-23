//! Integration tests for BitCraps

use bitcraps::*;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_full_game_flow() {
    // Create players
    let alice_keypair = BitchatKeypair::generate();
    let bob_keypair = BitchatKeypair::generate();
    let charlie_keypair = BitchatKeypair::generate();
    
    let alice_id = alice_keypair.public_key().to_peer_id();
    let bob_id = bob_keypair.public_key().to_peer_id();
    let charlie_id = charlie_keypair.public_key().to_peer_id();
    
    // Create game runtime
    let mut runtime = GameRuntime::new(alice_keypair.clone());
    
    // Create a new game
    let game_id = runtime.create_game(100).await.unwrap();
    
    // Other players join
    runtime.join_game(game_id, bob_id).await.unwrap();
    runtime.join_game(game_id, charlie_id).await.unwrap();
    
    // Place bets
    runtime.place_bet(game_id, alice_id, BetType::Pass, 50).await.unwrap();
    runtime.place_bet(game_id, bob_id, BetType::DontPass, 30).await.unwrap();
    runtime.place_bet(game_id, charlie_id, BetType::Field, 20).await.unwrap();
    
    // Start the game
    runtime.start_game(game_id).await.unwrap();
    
    // Simulate dice roll
    let dice_roll = runtime.roll_dice(game_id).await.unwrap();
    assert!(dice_roll.die1 >= 1 && dice_roll.die1 <= 6);
    assert!(dice_roll.die2 >= 1 && dice_roll.die2 <= 6);
    
    // Process payouts
    runtime.process_payouts(game_id).await.unwrap();
    
    // Verify game state
    let game_state = runtime.get_game_state(game_id).await.unwrap();
    assert_eq!(game_state.players.len(), 3);
}

#[tokio::test]
async fn test_mesh_network_formation() {
    // Create multiple mesh nodes
    let node1 = MeshService::new(BitchatKeypair::generate()).await.unwrap();
    let node2 = MeshService::new(BitchatKeypair::generate()).await.unwrap();
    let node3 = MeshService::new(BitchatKeypair::generate()).await.unwrap();
    
    // Add peers to form mesh
    let peer1 = MeshPeer {
        id: node1.get_peer_id(),
        address: "127.0.0.1:8001".to_string(),
        public_key: node1.get_public_key(),
        last_seen: std::time::Instant::now(),
        relay_score: 100,
        is_relay: true,
    };
    
    let peer2 = MeshPeer {
        id: node2.get_peer_id(),
        address: "127.0.0.1:8002".to_string(),
        public_key: node2.get_public_key(),
        last_seen: std::time::Instant::now(),
        relay_score: 100,
        is_relay: false,
    };
    
    // Connect nodes
    node2.add_peer(peer1.clone()).await.unwrap();
    node3.add_peer(peer1.clone()).await.unwrap();
    node3.add_peer(peer2.clone()).await.unwrap();
    
    // Test routing
    let packet = BitchatPacket::ping(node1.get_peer_id());
    let serialized = packet.serialize().unwrap();
    
    // Route packet from node3 to node1 via node2
    let route = node3.find_best_route(node3.get_peer_id(), node1.get_peer_id()).unwrap();
    assert!(!route.is_empty());
    
    // Send packet
    node3.route_packet(&serialized, node1.get_peer_id()).await.unwrap();
}

#[tokio::test]
async fn test_session_encryption() {
    // Create session managers
    let alice_manager = SessionManager::new(BitchatIdentity::generate_with_pow(0));
    let bob_manager = SessionManager::new(BitchatIdentity::generate_with_pow(0));
    
    let alice_id = alice_manager.get_peer_id();
    let bob_id = bob_manager.get_peer_id();
    
    // Establish session
    let alice_session = alice_manager.create_session(bob_id, true).await.unwrap();
    let bob_session = bob_manager.create_session(alice_id, false).await.unwrap();
    
    // Exchange handshake messages
    let init_msg = alice_session.get_handshake_message().await.unwrap();
    bob_session.process_handshake(&init_msg).await.unwrap();
    
    let resp_msg = bob_session.get_handshake_message().await.unwrap();
    alice_session.process_handshake(&resp_msg).await.unwrap();
    
    // Test encryption/decryption
    let plaintext = b"Secret message for testing";
    let ciphertext = alice_session.encrypt(plaintext).await.unwrap();
    let decrypted = bob_session.decrypt(&ciphertext).await.unwrap();
    
    assert_eq!(plaintext.to_vec(), decrypted);
    
    // Test forward secrecy key rotation
    alice_session.rotate_keys().await.unwrap();
    bob_session.rotate_keys().await.unwrap();
    
    let plaintext2 = b"Another secret message";
    let ciphertext2 = alice_session.encrypt(plaintext2).await.unwrap();
    let decrypted2 = bob_session.decrypt(&ciphertext2).await.unwrap();
    
    assert_eq!(plaintext2.to_vec(), decrypted2);
}

#[tokio::test]
async fn test_token_economy() {
    let mut ledger = TokenLedger::new();
    
    // Create accounts
    let alice = [1u8; 32];
    let bob = [2u8; 32];
    let charlie = [3u8; 32];
    
    ledger.create_account(alice);
    ledger.create_account(bob);
    ledger.create_account(charlie);
    
    // Initial mint
    assert!(ledger.mint(alice, 1000).is_ok());
    assert_eq!(ledger.get_balance(&alice), 1000);
    
    // Transfer tokens
    assert!(ledger.transfer(alice, bob, 250).is_ok());
    assert_eq!(ledger.get_balance(&alice), 750);
    assert_eq!(ledger.get_balance(&bob), 250);
    
    // Test insufficient balance
    assert!(ledger.transfer(bob, charlie, 500).is_err());
    
    // Test proof of relay mining
    let proof = ProofOfRelay {
        relayer: charlie,
        packet_hash: [0xFF; 32],
        timestamp: chrono::Utc::now().timestamp() as u64,
        difficulty: 16,
        nonce: 0,
    };
    
    // Mine valid proof
    let mined_proof = proof.mine(16);
    assert!(mined_proof.validate(16));
    
    // Reward relay
    assert!(ledger.mint(mined_proof.relayer, 10).is_ok());
    assert_eq!(ledger.get_balance(&charlie), 10);
}

#[tokio::test]
async fn test_consensus_mechanism() {
    use bitcraps::protocol::consensus::{ConsensusEngine, ConsensusVote, VoteType};
    
    // Create consensus engine
    let engine = ConsensusEngine::new([1u8; 32]);
    
    // Initialize game
    let game_id = [2u8; 32];
    let players = vec![[3u8; 32], [4u8; 32], [5u8; 32]];
    engine.initialize_game(game_id, players.clone()).await.unwrap();
    
    // Submit votes for dice roll
    for (i, player) in players.iter().enumerate() {
        let vote = ConsensusVote {
            voter: *player,
            game_id,
            round: 1,
            vote_type: VoteType::DiceRoll(7 + i as u8), // Different values
            signature: [0u8; 64], // Would be real signature in production
            timestamp: chrono::Utc::now().timestamp() as u64,
        };
        engine.submit_vote(vote).await.unwrap();
    }
    
    // Finalize round
    let result = engine.finalize_round(game_id, 1).await.unwrap();
    
    // Verify consensus was reached
    assert!(result.consensus_reached);
    assert_eq!(result.participants.len(), 3);
}

#[tokio::test]
async fn test_bluetooth_transport() {
    use bitcraps::transport::{TransportCoordinator, BluetoothTransport};
    
    // Create transport coordinator
    let coordinator = TransportCoordinator::new();
    
    // Initialize Bluetooth transport (will fail gracefully if no adapter)
    let bt_result = BluetoothTransport::new().await;
    
    if let Ok(bluetooth) = bt_result {
        // Register transport
        coordinator.register_transport(Box::new(bluetooth)).await.unwrap();
        
        // Start discovery
        let discovery_handle = coordinator.start_discovery().await;
        
        // Wait for some discoveries
        timeout(Duration::from_secs(5), discovery_handle).await.ok();
        
        // Check discovered peers
        let peers = coordinator.get_discovered_peers().await;
        println!("Discovered {} Bluetooth peers", peers.len());
    } else {
        println!("Bluetooth not available, skipping transport test");
    }
}

#[tokio::test]
async fn test_multi_tier_cache() {
    use bitcraps::cache::MultiTierCache;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let cache: MultiTierCache<String, Vec<u8>> = 
        MultiTierCache::new(temp_dir.path().to_path_buf()).unwrap();
    
    // Test basic operations
    let key1 = "test_key_1".to_string();
    let value1 = vec![1, 2, 3, 4, 5];
    
    cache.insert(key1.clone(), value1.clone()).unwrap();
    assert_eq!(cache.get(&key1), Some(value1.clone()));
    
    // Test cache stats
    let stats = cache.get_stats();
    assert_eq!(stats.l1_hits, 1);
    
    // Clear L1 and L2, data should still be in L3
    cache.clear_all();
    assert_eq!(cache.get(&key1), Some(value1));
    
    // Check promotion
    let stats = cache.get_stats();
    assert_eq!(stats.l3_hits, 1);
    assert_eq!(stats.promotions, 1);
    
    // Test multiple entries
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = vec![i as u8; 100];
        cache.insert(key, value).unwrap();
    }
    
    // Access pattern to test promotion
    for _ in 0..5 {
        cache.get(&"key_50".to_string());
    }
    
    let final_stats = cache.get_stats();
    assert!(final_stats.l1_hits > 0);
}

#[tokio::test]
async fn test_compression() {
    use bitcraps::protocol::compression::{MessageCompressor, CompressionAlgorithm};
    
    let compressor = MessageCompressor::new();
    
    // Test different data types
    let json_data = r#"{"game":"craps","players":10,"bets":[1,2,3,4,5]}"#.as_bytes();
    let binary_data = vec![0xFF; 1000];
    let text_data = "Hello, World! ".repeat(100).into_bytes();
    
    // Test LZ4 compression
    let compressed_lz4 = compressor.compress(&json_data, CompressionAlgorithm::Lz4).unwrap();
    assert!(compressed_lz4.len() < json_data.len());
    
    let decompressed_lz4 = compressor.decompress(&compressed_lz4).unwrap();
    assert_eq!(decompressed_lz4, json_data);
    
    // Test Zlib compression
    let compressed_zlib = compressor.compress(&text_data, CompressionAlgorithm::Zlib).unwrap();
    assert!(compressed_zlib.len() < text_data.len());
    
    let decompressed_zlib = compressor.decompress(&compressed_zlib).unwrap();
    assert_eq!(decompressed_zlib, text_data);
    
    // Test adaptive compression
    let adaptive_json = compressor.compress_adaptive(&json_data).unwrap();
    let adaptive_binary = compressor.compress_adaptive(&binary_data).unwrap();
    
    // Verify decompression works
    assert_eq!(compressor.decompress(&adaptive_json).unwrap(), json_data);
    assert_eq!(compressor.decompress(&adaptive_binary).unwrap(), binary_data);
}

#[tokio::test]
async fn test_monitoring_metrics() {
    use bitcraps::monitoring::{NetworkMetrics, HealthCheck};
    
    let metrics = NetworkMetrics::new();
    
    // Record some metrics
    metrics.record_packet_sent("ping", 100);
    metrics.record_packet_received("pong", 100);
    metrics.record_peer_connected();
    metrics.record_game_created();
    metrics.record_bet_placed(50);
    
    // Get snapshot
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.packets_sent, 1);
    assert_eq!(snapshot.packets_received, 1);
    assert_eq!(snapshot.peers_connected, 1);
    assert_eq!(snapshot.games_active, 1);
    assert_eq!(snapshot.total_bets, 50);
    
    // Test health check
    let health = HealthCheck::new();
    let status = health.check_system().await;
    
    assert!(status.is_healthy);
    assert!(status.memory_usage_mb > 0);
    assert!(status.cpu_usage_percent >= 0.0);
}

#[tokio::test]
async fn test_platform_optimizations() {
    use bitcraps::platform::optimizations::PlatformOptimizer;
    
    let optimizer = PlatformOptimizer::new().unwrap();
    
    // Apply platform-specific optimizations
    optimizer.optimize_for_platform().unwrap();
    
    // Test SIMD operations if available
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        use bitcraps::platform::optimizations::simd;
        
        let src = vec![1u8; 1024];
        let mut dst = vec![0u8; 1024];
        
        unsafe {
            simd::simd_memcpy(dst.as_mut_ptr(), src.as_ptr(), 1024);
        }
        
        assert_eq!(src, dst);
    }
}

#[tokio::test]
async fn test_error_recovery() {
    // Test network partition recovery
    let node1 = MeshService::new(BitchatKeypair::generate()).await.unwrap();
    let node2 = MeshService::new(BitchatKeypair::generate()).await.unwrap();
    
    // Simulate network partition
    node1.disconnect_all().await;
    node2.disconnect_all().await;
    
    // Attempt to send packet (should fail)
    let packet = BitchatPacket::ping(node2.get_peer_id());
    let result = node1.route_packet(
        &packet.serialize().unwrap(),
        node2.get_peer_id()
    ).await;
    assert!(result.is_err());
    
    // Reconnect
    let peer2 = MeshPeer {
        id: node2.get_peer_id(),
        address: "127.0.0.1:9000".to_string(),
        public_key: node2.get_public_key(),
        last_seen: std::time::Instant::now(),
        relay_score: 100,
        is_relay: false,
    };
    node1.add_peer(peer2).await.unwrap();
    
    // Should work now
    let result = node1.route_packet(
        &packet.serialize().unwrap(),
        node2.get_peer_id()
    ).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_persistence() {
    use bitcraps::persistence::PersistenceManager;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let persistence = PersistenceManager::new(temp_dir.path()).await.unwrap();
    
    // Save game state
    let game_id = [1u8; 32];
    let game_state = CrapsGame::new(game_id, 100);
    persistence.save_game_state(&game_state).await.unwrap();
    
    // Load game state
    let loaded = persistence.load_game_state(&game_id).await.unwrap();
    assert_eq!(loaded.game_id, game_id);
    assert_eq!(loaded.min_bet, 100);
    
    // Save peer information
    let peer = MeshPeer {
        id: [2u8; 32],
        address: "192.168.1.100".to_string(),
        public_key: [3u8; 32],
        last_seen: std::time::Instant::now(),
        relay_score: 150,
        is_relay: true,
    };
    persistence.save_peer(&peer).await.unwrap();
    
    // Load peers
    let peers = persistence.load_peers().await.unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].id, peer.id);
}

#[test]
fn test_deterministic_randomness() {
    use bitcraps::protocol::craps::DeterministicRng;
    
    // Test that same seed produces same sequence
    let seed1 = [0x42; 32];
    let seed2 = [0x42; 32];
    
    let mut rng1 = DeterministicRng::new(seed1);
    let mut rng2 = DeterministicRng::new(seed2);
    
    for _ in 0..100 {
        assert_eq!(rng1.next_die(), rng2.next_die());
    }
    
    // Test that different seeds produce different sequences
    let seed3 = [0x43; 32];
    let mut rng3 = DeterministicRng::new(seed3);
    
    let mut different = false;
    for _ in 0..100 {
        if rng1.next_die() != rng3.next_die() {
            different = true;
            break;
        }
    }
    assert!(different);
}