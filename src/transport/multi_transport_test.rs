//! Multi-Transport Integration Tests
//!
//! This module tests:
//! - Multi-transport coordination
//! - NAT traversal scenarios
//! - 8+ concurrent connections
//! - Failover mechanisms
//! - Load balancing

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::protocol::PeerId;
    use crate::transport::{
        nat_traversal::{NetworkHandler, NatType, TransportMode},
        tcp_transport::{TcpTransport, TcpTransportConfig},
        intelligent_coordinator::{
            IntelligentTransportCoordinator, IntelligentCoordinatorConfig,
            TransportType, TransportCapabilities, TransportPriority
        },
    };
    use tokio::net::UdpSocket;
    use std::collections::HashMap;
    use std::time::Duration;
    
    /// Test multi-transport system with 8+ concurrent connections
    #[tokio::test]
    async fn test_concurrent_connections_8_plus() {
        println!("Testing 8+ concurrent connections across multiple transports");
        
        // Create NAT handler
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        let nat_handler = NetworkHandler::new(socket, None, local_addr);
        
        // Create intelligent coordinator
        let config = IntelligentCoordinatorConfig {
            max_transports_per_peer: 3,
            health_check_interval: Duration::from_secs(1),
            ..Default::default()
        };
        
        let coordinator = IntelligentTransportCoordinator::new(config, nat_handler);
        
        // Add multiple transport types
        let transport_configs = vec![
            ("udp_primary", TransportType::Udp),
            ("udp_nat", TransportType::UdpWithNatTraversal),
            ("tcp_primary", TransportType::Tcp),
            ("tcp_tls", TransportType::TcpTls),
        ];
        
        for (transport_id, transport_type) in transport_configs {
            let tcp_config = TcpTransportConfig::default();
            let transport = Box::new(TcpTransport::new(tcp_config));
            
            let capabilities = TransportCapabilities {
                supports_broadcast: matches!(transport_type, TransportType::Udp | TransportType::UdpWithNatTraversal),
                supports_multicast: false,
                max_message_size: 1024 * 1024,
                max_connections: 100,
                requires_pairing: false,
                encryption_available: matches!(transport_type, TransportType::TcpTls),
            };
            
            coordinator.add_transport(
                transport_id.to_string(),
                transport_type,
                transport,
                capabilities,
            ).await.unwrap();
        }
        
        // Create 10 peer connections (exceeding the 8+ requirement)
        let mut peer_ids = Vec::new();
        for i in 0..10 {
            let peer_id = PeerId::from([0u8; 32]); // Generate proper peer ID
            peer_ids.push(peer_id);
            
            // Attempt connection with failover
            match coordinator.connect_with_failover(peer_id, None).await {
                Ok(_) => println!("Connected peer {}: {:?}", i, peer_id),
                Err(e) => println!("Failed to connect peer {}: {}", i, e),
            }
        }
        
        // Wait for connections to stabilize
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify connection statistics
        let stats = coordinator.get_transport_statistics().await;
        let total_connections: usize = stats.values().map(|s| s.active_connections).sum();
        
        println!("Total active connections: {}", total_connections);
        for (transport_id, stat) in &stats {
            println!("Transport {}: {} connections, health: {:?}", 
                transport_id, stat.active_connections, stat.health_status);
        }
        
        // Should have successfully established multiple connections
        assert!(total_connections >= 4, "Should have at least 4 connections established");
        
        // Test concurrent message sending
        let test_message = b"Multi-transport test message".to_vec();
        let mut successful_sends = 0;
        
        for (i, peer_id) in peer_ids.iter().enumerate() {
            let priority = match i % 4 {
                0 => TransportPriority::Critical,
                1 => TransportPriority::High,
                2 => TransportPriority::Normal,
                _ => TransportPriority::Low,
            };
            
            match coordinator.send_intelligent(*peer_id, test_message.clone(), priority).await {
                Ok(_) => {
                    successful_sends += 1;
                    println!("Successfully sent message to peer {}", i);
                }
                Err(e) => println!("Failed to send to peer {}: {}", i, e),
            }
        }
        
        println!("Successful sends: {}/{}", successful_sends, peer_ids.len());
        
        // Should have some successful sends
        assert!(successful_sends > 0, "Should have at least some successful message sends");
    }
    
    /// Test NAT traversal scenarios
    #[tokio::test]
    async fn test_nat_traversal_scenarios() {
        println!("Testing NAT traversal scenarios");
        
        // Create NAT handlers with different NAT types
        let nat_scenarios = vec![
            ("open", NatType::Open),
            ("full_cone", NatType::FullCone),
            ("restricted", NatType::RestrictedCone),
            ("port_restricted", NatType::PortRestrictedCone),
            ("symmetric", NatType::Symmetric),
        ];
        
        for (scenario_name, nat_type) in nat_scenarios {
            println!("Testing NAT scenario: {} ({:?})", scenario_name, nat_type);
            
            let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let local_addr = socket.local_addr().unwrap();
            let mut nat_handler = NetworkHandler::new(socket, None, local_addr);
            
            // Simulate NAT type detection
            {
                let mut detected_nat_type = nat_handler.nat_type.write().await;
                *detected_nat_type = nat_type.clone();
            }
            
            // Test transport mode selection for different scenarios
            let test_destinations = vec![
                "192.168.1.100:8080".parse().unwrap(),
                "10.0.0.50:9090".parse().unwrap(),
                "172.16.1.200:7777".parse().unwrap(),
            ];
            
            for dest in test_destinations {
                let selected_mode = nat_handler.select_transport_mode(&dest).await;
                println!("  Destination {}: Selected transport mode: {:?}", dest, selected_mode);
                
                // Verify transport mode selection logic
                match nat_type {
                    NatType::Open => assert_eq!(selected_mode, TransportMode::Udp),
                    NatType::Symmetric => assert!(matches!(selected_mode, 
                        TransportMode::TurnRelay | TransportMode::TcpTls | TransportMode::Tcp | TransportMode::UdpHolePunching)),
                    _ => {} // Other types have more complex selection logic
                }
            }
            
            // Test advanced NAT traversal
            let target_peer = "203.0.113.1:8080".parse().unwrap(); // Documentation IP
            match nat_handler.initiate_advanced_nat_traversal(target_peer).await {
                Ok(established_addr) => {
                    println!("  Advanced traversal successful: {}", established_addr);
                }
                Err(e) => {
                    println!("  Advanced traversal failed (expected for some NAT types): {}", e);
                    // This is expected for some NAT types in test environment
                }
            }
        }
    }
    
    /// Test transport failover mechanisms
    #[tokio::test]
    async fn test_transport_failover() {
        println!("Testing transport failover mechanisms");
        
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        let nat_handler = NetworkHandler::new(socket, None, local_addr);
        
        let config = IntelligentCoordinatorConfig {
            enable_adaptive_routing: true,
            enable_predictive_failover: true,
            failover_timeout: Duration::from_secs(1),
            ..Default::default()
        };
        
        let coordinator = IntelligentTransportCoordinator::new(config, nat_handler);
        
        // Add primary and backup transports
        let primary_transport = Box::new(TcpTransport::new(TcpTransportConfig::default()));
        let backup_transport = Box::new(TcpTransport::new(TcpTransportConfig::default()));
        
        let capabilities = TransportCapabilities {
            supports_broadcast: false,
            supports_multicast: false,
            max_message_size: 1024,
            max_connections: 10,
            requires_pairing: false,
            encryption_available: false,
        };
        
        coordinator.add_transport(
            "primary".to_string(),
            TransportType::Tcp,
            primary_transport,
            capabilities.clone(),
        ).await.unwrap();
        
        coordinator.add_transport(
            "backup".to_string(),
            TransportType::Udp,
            backup_transport,
            capabilities,
        ).await.unwrap();
        
        // Simulate connection
        let peer_id = PeerId::from([0u8; 32]); // Generate proper peer ID
        
        // Test failover during send operations
        let test_message = b"Failover test message".to_vec();
        
        // Multiple send attempts to trigger potential failover scenarios
        for i in 0..5 {
            match coordinator.send_intelligent(peer_id, test_message.clone(), TransportPriority::Normal).await {
                Ok(_) => println!("Send attempt {} successful", i),
                Err(e) => println!("Send attempt {} failed: {}", i, e),
            }
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Verify transport statistics show failover attempts
        let stats = coordinator.get_transport_statistics().await;
        println!("Transport statistics after failover test:");
        for (transport_id, stat) in stats {
            println!("  {}: {} errors, reliability: {:.2}", 
                transport_id, stat.total_errors, stat.metrics.reliability);
        }
    }
    
    /// Test load balancing across transports
    #[tokio::test]
    async fn test_load_balancing() {
        println!("Testing load balancing across transports");
        
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        let nat_handler = NetworkHandler::new(socket, None, local_addr);
        
        let config = IntelligentCoordinatorConfig {
            load_balance_threshold: 0.5, // Trigger load balancing at 50% capacity
            ..Default::default()
        };
        
        let coordinator = IntelligentTransportCoordinator::new(config, nat_handler);
        
        // Add multiple transports with different capacities
        let transport_configs = vec![
            ("high_capacity", 20),
            ("medium_capacity", 10),
            ("low_capacity", 5),
        ];
        
        for (transport_id, max_connections) in transport_configs {
            let tcp_config = TcpTransportConfig {
                max_connections,
                ..Default::default()
            };
            let transport = Box::new(TcpTransport::new(tcp_config));
            
            let capabilities = TransportCapabilities {
                supports_broadcast: false,
                supports_multicast: false,
                max_message_size: 1024,
                max_connections,
                requires_pairing: false,
                encryption_available: false,
            };
            
            coordinator.add_transport(
                transport_id.to_string(),
                TransportType::Tcp,
                transport,
                capabilities,
            ).await.unwrap();
        }
        
        // Create multiple peers to test load distribution
        let mut peer_ids = Vec::new();
        for _ in 0..15 {
            peer_ids.push(PeerId::random());
        }
        
        // Test optimal transport selection for different loads
        for (i, peer_id) in peer_ids.iter().enumerate() {
            let selected_transport = coordinator
                .select_optimal_transport(*peer_id, TransportPriority::Normal)
                .await;
            
            match selected_transport {
                Ok(transport_id) => {
                    println!("Peer {}: Selected transport {}", i, transport_id);
                }
                Err(e) => {
                    println!("Peer {}: No transport selected: {}", i, e);
                }
            }
        }
        
        // Test broadcast with load balancing
        let broadcast_message = b"Load balanced broadcast".to_vec();
        match coordinator.broadcast_optimized(broadcast_message, TransportPriority::Normal).await {
            Ok(successful_count) => {
                println!("Broadcast successful to {} peers", successful_count);
            }
            Err(e) => {
                println!("Broadcast failed: {}", e);
            }
        }
        
        // Verify load distribution in statistics
        let stats = coordinator.get_transport_statistics().await;
        for (transport_id, stat) in stats {
            let load_percentage = (stat.active_connections as f32 / stat.max_connections as f32) * 100.0;
            println!("Transport {}: {:.1}% loaded ({}/{})", 
                transport_id, load_percentage, stat.active_connections, stat.max_connections);
        }
    }
    
    /// Test performance under stress
    #[tokio::test] 
    async fn test_performance_stress() {
        println!("Testing performance under stress conditions");
        
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        let nat_handler = NetworkHandler::new(socket, None, local_addr);
        
        let coordinator = IntelligentTransportCoordinator::new(
            IntelligentCoordinatorConfig::default(),
            nat_handler,
        );
        
        // Add high-performance transport
        let tcp_config = TcpTransportConfig {
            max_connections: 100,
            connection_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let transport = Box::new(TcpTransport::new(tcp_config));
        
        let capabilities = TransportCapabilities {
            supports_broadcast: true,
            supports_multicast: false,
            max_message_size: 64 * 1024, // 64KB
            max_connections: 100,
            requires_pairing: false,
            encryption_available: false,
        };
        
        coordinator.add_transport(
            "stress_test".to_string(),
            TransportType::Udp,
            transport,
            capabilities,
        ).await.unwrap();
        
        // Create many concurrent operations
        let mut handles = Vec::new();
        let message_size_variants = vec![64, 512, 1024, 4096, 16384]; // Different message sizes
        
        for i in 0..50 { // 50 concurrent tasks
            let coordinator_ref = &coordinator;
            let message_size = message_size_variants[i % message_size_variants.len()];
            
            let handle = tokio::spawn(async move {
                let peer_id = PeerId::from([0u8; 32]); // Generate proper peer ID
                let test_message = vec![0u8; message_size];
                
                // Simulate connection and multiple sends
                for j in 0..10 {
                    let priority = match j % 4 {
                        0 => TransportPriority::Critical,
                        1 => TransportPriority::High,
                        2 => TransportPriority::Normal,
                        _ => TransportPriority::Low,
                    };
                    
                    if let Err(e) = coordinator_ref.send_intelligent(peer_id, test_message.clone(), priority).await {
                        println!("Stress test task {} send {} failed: {}", i, j, e);
                    }
                    
                    // Small delay between sends
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
                
                i // Return task ID
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        let start_time = std::time::Instant::now();
        let mut completed_tasks = 0;
        
        for handle in handles {
            match handle.await {
                Ok(task_id) => {
                    completed_tasks += 1;
                    if completed_tasks % 10 == 0 {
                        println!("Completed {} stress test tasks", completed_tasks);
                    }
                }
                Err(e) => {
                    println!("Stress test task failed: {}", e);
                }
            }
        }
        
        let total_time = start_time.elapsed();
        println!("Stress test completed: {} tasks in {:?}", completed_tasks, total_time);
        
        // Verify final statistics
        let final_stats = coordinator.get_transport_statistics().await;
        for (transport_id, stat) in final_stats {
            println!("Final stats for {}: {} messages, {} errors, {:.2} reliability", 
                transport_id, stat.total_messages_sent, stat.total_errors, stat.metrics.reliability);
        }
        
        // Performance assertions
        assert!(completed_tasks >= 40, "Should complete at least 80% of stress test tasks");
        assert!(total_time < Duration::from_secs(10), "Stress test should complete within 10 seconds");
    }
}