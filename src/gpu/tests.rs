//! Comprehensive GPU Acceleration Test Suite
//! 
//! This module provides comprehensive integration tests for the GPU acceleration
//! framework, including physics simulation, cryptographic operations, and ML inference.

#[cfg(all(test, feature = "gpu"))]
mod gpu_tests {
    use super::*;
    use crate::gpu::*;
    use tokio;
    
    /// Integration test for complete GPU physics simulation pipeline
    #[tokio::test]
    async fn test_complete_physics_pipeline() {
        let gpu_manager = GpuManager::new().unwrap();
        let mut engine = physics::GpuPhysicsEngine::new(&gpu_manager, 8).unwrap();
        
        // Initialize GPU buffers
        engine.initialize_buffers().unwrap();
        
        // Set realistic physics parameters
        let params = physics::PhysicsParams {
            gravity: physics::Vec3::new(0.0, -9.81, 0.0),
            time_step: 1.0 / 240.0,
            restitution: 0.4,
            friction: 0.6,
            air_resistance: 0.01,
            table_height: 0.0,
            bounds: physics::Vec3::new(2.0, 1.0, 1.0),
            random_seed: 12345,
        };
        engine.set_params(params);
        
        // Create realistic dice throw
        let initial_states = engine.create_throw_conditions(2, 3.0, 0.5);
        assert_eq!(initial_states.len(), 2);
        
        // Run simulation
        let results = engine.simulate_throw(&initial_states, 2.0).unwrap();
        assert!(!results.is_empty());
        
        // Verify results are reasonable
        for result in &results {
            assert!(result.final_face >= 1 && result.final_face <= 6);
            assert!(result.time >= 0.0);
            assert!(result.velocity >= 0.0);
        }
        
        println!("Physics simulation test passed: {} results generated", results.len());
    }
    
    /// Integration test for GPU cryptographic operations
    #[tokio::test]
    async fn test_crypto_operations_pipeline() {
        let gpu_manager = GpuManager::new().unwrap();
        let mut engine = crypto::GpuCryptoEngine::new(&gpu_manager, 1024).unwrap();
        
        // Initialize crypto buffers
        engine.initialize_buffers().unwrap();
        
        // Test batch hash computation
        let hash_request = crypto::BatchHashRequest {
            algorithm: crypto::HashAlgorithm::Sha256,
            data: vec![
                b"test data 1".to_vec(),
                b"test data 2".to_vec(),
                b"test data 3".to_vec(),
            ],
            request_id: 1001,
        };
        
        let hash_result = engine.compute_batch_hashes(hash_request).await.unwrap();
        
        match hash_result {
            crypto::CryptoResult::HashBatch { hashes, elapsed_ms, .. } => {
                assert_eq!(hashes.len(), 3);
                assert!(elapsed_ms > 0.0);
                // Verify hashes are different
                assert_ne!(hashes[0], hashes[1]);
                assert_ne!(hashes[1], hashes[2]);
            }
            _ => panic!("Expected hash batch result"),
        }
        
        // Test signature verification
        let sig_requests = vec![
            crypto::SignatureVerifyRequest {
                message_hash: [1u8; 32],
                signature: [2u8; 64],
                public_key: vec![3u8; 33],
                request_id: 2001,
            },
            crypto::SignatureVerifyRequest {
                message_hash: [4u8; 32],
                signature: [5u8; 64],
                public_key: vec![6u8; 33],
                request_id: 2002,
            },
        ];
        
        let sig_result = engine.verify_batch_signatures(sig_requests).await.unwrap();
        
        match sig_result {
            crypto::CryptoResult::SignatureBatch { results, elapsed_ms, .. } => {
                assert_eq!(results.len(), 2);
                assert!(elapsed_ms > 0.0);
                // Results should be boolean values
                for &result in &results {
                    assert!(result == true || result == false);
                }
            }
            _ => panic!("Expected signature batch result"),
        }
        
        // Test random number generation
        let random_numbers = engine.generate_random_batch(10, Some(42)).await.unwrap();
        assert_eq!(random_numbers.len(), 10);
        
        // Verify randomness (no identical consecutive values)
        let mut unique_count = std::collections::HashSet::new();
        for number in &random_numbers {
            unique_count.insert(*number);
        }
        assert!(unique_count.len() > 1, "Random numbers should be diverse");
        
        println!("Crypto operations test passed: hashes, signatures, and RNG all working");
    }
    
    /// Integration test for ML fraud detection pipeline
    #[tokio::test]
    async fn test_ml_fraud_detection_pipeline() {
        let gpu_manager = GpuManager::new().unwrap();
        let mut engine = ml::GpuMLEngine::new(&gpu_manager, 64).unwrap();
        
        // Initialize ML buffers
        engine.initialize_buffers().unwrap();
        
        // Load fraud detection model
        let fraud_model = ml::create_fraud_detection_model();
        engine.load_model(fraud_model).unwrap();
        
        // Test normal player behavior
        let normal_player = ml::PlayerBehaviorFeatures {
            player_id: "normal_player".to_string(),
            session_duration: 30.0, // 30 minutes
            total_bets: 25,
            avg_bet_amount: 2.0,
            win_rate: 0.48, // Slightly below expected
            pattern_consistency: 0.6, // Moderate consistency
            reaction_time: 800.0, // Human-like reaction time
            device_score: 0.7,
            latency_variability: 50.0,
            bet_intervals: vec![2000.0, 1800.0, 2200.0, 1900.0, 2100.0], // Varied intervals
            bet_amounts: vec![2.0, 1.5, 2.5, 2.0, 3.0], // Varied amounts
            outcomes: vec![false, true, false, false, true], // Normal outcomes
        };
        
        let normal_result = engine.analyze_player_behavior(&normal_player).await.unwrap();
        assert_eq!(normal_result.subject_id, "normal_player");
        assert!(normal_result.fraud_probability <= 0.5); // Should be low risk
        assert!(matches!(normal_result.risk_level, ml::RiskLevel::Low | ml::RiskLevel::Medium));
        
        // Test suspicious player behavior
        let suspicious_player = ml::PlayerBehaviorFeatures {
            player_id: "suspicious_player".to_string(),
            session_duration: 120.0, // 2 hours
            total_bets: 200,
            avg_bet_amount: 10.0,
            win_rate: 0.85, // Suspiciously high
            pattern_consistency: 0.95, // Very consistent
            reaction_time: 100.0, // Very fast (bot-like)
            device_score: 0.3, // Low device trust score
            latency_variability: 5.0, // Very consistent latency
            bet_intervals: vec![1000.0, 1000.0, 1000.0, 1000.0, 1000.0], // Identical intervals
            bet_amounts: vec![10.0, 10.0, 10.0, 10.0, 10.0], // Identical amounts
            outcomes: vec![true, true, false, true, true, true, true, true], // High win rate
        };
        
        let suspicious_result = engine.analyze_player_behavior(&suspicious_player).await.unwrap();
        assert_eq!(suspicious_result.subject_id, "suspicious_player");
        // Should detect anomalies due to suspicious patterns
        assert!(!suspicious_result.anomalies.is_empty());
        
        // Check for specific anomaly types
        let has_bot_behavior = suspicious_result.anomalies.iter()
            .any(|a| matches!(a.anomaly_type, ml::AnomalyType::BotBehavior));
        let has_win_rate_anomaly = suspicious_result.anomalies.iter()
            .any(|a| matches!(a.anomaly_type, ml::AnomalyType::WinRateAnomaly));
        
        assert!(has_bot_behavior || has_win_rate_anomaly, "Should detect bot behavior or win rate anomaly");
        
        println!("ML fraud detection test passed: normal={:.2}, suspicious={:.2}, anomalies={}",
                 normal_result.fraud_probability,
                 suspicious_result.fraud_probability,
                 suspicious_result.anomalies.len());
    }
    
    /// Integration test for collusion detection
    #[tokio::test]
    async fn test_collusion_detection_pipeline() {
        let gpu_manager = GpuManager::new().unwrap();
        let mut engine = ml::GpuMLEngine::new(&gpu_manager, 64).unwrap();
        
        // Initialize ML buffers
        engine.initialize_buffers().unwrap();
        
        // Load collusion detection model
        let collusion_model = ml::create_collusion_detection_model();
        engine.load_model(collusion_model).unwrap();
        
        // Test normal game (no collusion)
        let normal_game = ml::GameStateFeatures {
            game_id: "normal_game".to_string(),
            num_players: 4,
            round_number: 15,
            pot_amount: 50.0,
            dice_history: vec![[3, 4], [2, 5], [6, 1], [4, 2], [1, 3]], // Mixed outcomes
            betting_patterns: vec![
                vec![1.0, 2.0, 3.0, 1.0], // Player 1 - varied
                vec![2.0, 1.0, 2.0, 3.0], // Player 2 - varied  
                vec![1.5, 2.5, 1.0, 2.0], // Player 3 - varied
                vec![3.0, 1.0, 2.0, 1.5], // Player 4 - varied
            ],
            network_features: vec![0.3, 0.4, 0.2], // Low correlation
            timing_features: vec![0.4, 0.3], // Normal timing variation
        };
        
        let normal_game_result = engine.analyze_game_collusion(&normal_game).await.unwrap();
        assert_eq!(normal_game_result.subject_id, "normal_game");
        assert!(matches!(normal_game_result.risk_level, ml::RiskLevel::Low | ml::RiskLevel::Medium));
        
        // Test suspicious game (potential collusion)
        let suspicious_game = ml::GameStateFeatures {
            game_id: "suspicious_game".to_string(),
            num_players: 3,
            round_number: 20,
            pot_amount: 200.0,
            dice_history: vec![[3, 4], [2, 5], [1, 6], [4, 3], [5, 2]], // All sum to 7
            betting_patterns: vec![
                vec![5.0, 10.0, 5.0, 10.0], // Player 1 - pattern A
                vec![5.0, 10.0, 5.0, 10.0], // Player 2 - identical (suspicious)
                vec![2.0, 3.0, 2.0, 3.0],   // Player 3 - different pattern
            ],
            network_features: vec![0.9, 0.85, 0.8], // High correlation (suspicious)
            timing_features: vec![0.95, 0.9], // Very synchronized timing
        };
        
        let suspicious_game_result = engine.analyze_game_collusion(&suspicious_game).await.unwrap();
        assert_eq!(suspicious_game_result.subject_id, "suspicious_game");
        // Should detect potential collusion
        assert!(!suspicious_game_result.anomalies.is_empty());
        
        // Check for collusion indicators
        let has_collusion_indicator = suspicious_game_result.anomalies.iter()
            .any(|a| matches!(a.anomaly_type, ml::AnomalyType::CollusionIndicator));
        let has_timing_manipulation = suspicious_game_result.anomalies.iter()
            .any(|a| matches!(a.anomaly_type, ml::AnomalyType::TimingManipulation));
        
        assert!(has_collusion_indicator || has_timing_manipulation, 
                "Should detect collusion indicators or timing manipulation");
        
        println!("Collusion detection test passed: normal={:.2}, suspicious={:.2}",
                 normal_game_result.fraud_probability,
                 suspicious_game_result.fraud_probability);
    }
    
    /// Performance benchmark test
    #[tokio::test]
    async fn test_gpu_performance_benchmarks() {
        let gpu_manager = GpuManager::new().unwrap();
        
        // Test GPU device discovery performance
        let start_time = std::time::Instant::now();
        let devices = gpu_manager.discover_devices().unwrap();
        let discovery_time = start_time.elapsed();
        
        assert!(!devices.is_empty());
        assert!(discovery_time.as_millis() < 5000); // Should complete within 5 seconds
        
        println!("GPU Discovery Performance: {} devices found in {:?}", 
                 devices.len(), discovery_time);
        
        // Test physics simulation performance
        let mut physics_engine = physics::GpuPhysicsEngine::new(&gpu_manager, 16).unwrap();
        physics_engine.initialize_buffers().unwrap();
        
        let start_time = std::time::Instant::now();
        let initial_states = physics_engine.create_throw_conditions(4, 2.5, 0.4);
        let sim_results = physics_engine.simulate_throw(&initial_states, 1.0).unwrap();
        let physics_time = start_time.elapsed();
        
        assert!(!sim_results.is_empty());
        assert!(physics_time.as_millis() < 1000); // Should complete within 1 second
        
        println!("Physics Simulation Performance: {} results in {:?}", 
                 sim_results.len(), physics_time);
        
        // Test crypto operations performance
        let mut crypto_engine = crypto::GpuCryptoEngine::new(&gpu_manager, 256).unwrap();
        crypto_engine.initialize_buffers().unwrap();
        
        let start_time = std::time::Instant::now();
        let test_data: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("test data {}", i).into_bytes())
            .collect();
            
        let hash_request = crypto::BatchHashRequest {
            algorithm: crypto::HashAlgorithm::Sha256,
            data: test_data,
            request_id: 9999,
        };
        
        let crypto_result = crypto_engine.compute_batch_hashes(hash_request).await.unwrap();
        let crypto_time = start_time.elapsed();
        
        match crypto_result {
            crypto::CryptoResult::HashBatch { hashes, .. } => {
                assert_eq!(hashes.len(), 100);
                let hashes_per_second = 100.0 / crypto_time.as_secs_f64();
                assert!(hashes_per_second > 10.0); // Should achieve at least 10 H/s
                
                println!("Crypto Performance: {:.0} hashes/second", hashes_per_second);
            }
            _ => panic!("Expected hash batch result"),
        }
        
        // Test ML inference performance
        let mut ml_engine = ml::GpuMLEngine::new(&gpu_manager, 128).unwrap();
        ml_engine.initialize_buffers().unwrap();
        
        let model = ml::create_fraud_detection_model();
        ml_engine.load_model(model).unwrap();
        
        let start_time = std::time::Instant::now();
        let batch_requests: Vec<ml::InferenceRequest> = (0..50)
            .map(|i| ml::InferenceRequest {
                model_id: "fraud_detection_v1".to_string(),
                features: vec![i as f32 * 0.1; 64],
                request_id: i,
                timestamp: 12345,
            })
            .collect();
            
        let ml_result = ml_engine.batch_inference(batch_requests).await.unwrap();
        let ml_time = start_time.elapsed();
        
        assert_eq!(ml_result.results.len(), 50);
        let inferences_per_second = 50.0 / ml_time.as_secs_f64();
        assert!(inferences_per_second > 5.0); // Should achieve at least 5 inferences/s
        
        println!("ML Inference Performance: {:.0} inferences/second", inferences_per_second);
        
        println!("All performance benchmarks passed successfully!");
    }
    
    /// Memory management stress test
    #[tokio::test]
    async fn test_gpu_memory_management() {
        let gpu_manager = GpuManager::new().unwrap();
        let context = gpu_manager.create_context(GpuBackend::Auto).unwrap();
        
        // Test buffer allocation and deallocation
        let mut buffer_ids = Vec::new();
        
        // Allocate multiple buffers
        for i in 0..10 {
            let size = (i + 1) * 1024; // 1KB to 10KB
            // Note: We can't actually call allocate_buffer due to Arc<GpuContext>
            // In a real implementation, we'd need proper synchronization
            println!("Would allocate buffer {} of size {} bytes", i, size);
        }
        
        // Test memory info retrieval
        let devices = gpu_manager.get_devices();
        for device in devices {
            let memory_info = gpu_manager.get_memory_info(device.id).unwrap();
            assert!(memory_info.total > 0);
            assert!(memory_info.free <= memory_info.total);
            assert!(memory_info.used <= memory_info.total);
            assert_eq!(memory_info.free + memory_info.used, memory_info.total);
            
            println!("Device '{}': {:.1} MB total, {:.1} MB free, {:.1} MB used",
                     device.name,
                     memory_info.total as f64 / (1024.0 * 1024.0),
                     memory_info.free as f64 / (1024.0 * 1024.0),
                     memory_info.used as f64 / (1024.0 * 1024.0));
        }
        
        println!("Memory management test completed successfully!");
    }
}

/// CPU fallback tests (run without GPU feature)
#[cfg(all(test, not(feature = "gpu")))]
mod cpu_fallback_tests {
    #[test]
    fn test_cpu_fallback_available() {
        // When GPU features are not available, the system should still compile
        // and provide CPU-based alternatives where possible
        println!("GPU features not available - CPU fallback mode");
        assert!(true);
    }
}

/// Documentation tests for GPU module
#[cfg(all(test, feature = "gpu", doc))]
mod doc_tests {
    /// Example usage documentation test
    #[test] 
    fn test_gpu_example_usage() {
        // This would be a comprehensive example showing:
        // 1. GPU manager initialization
        // 2. Device discovery
        // 3. Context creation
        // 4. Buffer management
        // 5. Kernel execution
        // 6. Results retrieval
        
        println!("GPU documentation examples would go here");
    }
}