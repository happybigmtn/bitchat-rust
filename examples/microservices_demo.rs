//! Microservices Demo
//!
//! Demonstrates the microservices architecture for BitCraps.

use bitcraps::services::{
    ServiceBuilder,
    api_gateway::GatewayConfig,
    consensus::ConsensusConfig,
    game_engine::GameEngineConfig,
    common::discovery::StaticServiceDiscovery,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("🎰 Starting BitCraps Microservices Demo");
    
    // Create service discovery
    let discovery = Arc::new(StaticServiceDiscovery::new());
    
    // Configure services
    let game_engine_config = GameEngineConfig {
        max_concurrent_games: 50,
        max_players_per_game: 8,
        min_bet_amount: 1,
        max_bet_amount: 1000,
        game_timeout: Duration::from_secs(30 * 60), // 30 minutes
    };
    
    let consensus_config = ConsensusConfig {
        byzantine_threshold: 1,
        round_timeout: Duration::from_secs(15),
        max_rounds: 5,
        min_validators: 3,
        algorithm: bitcraps::services::consensus::ConsensusAlgorithm::PBFT,
    };
    
    let gateway_config = GatewayConfig {
        listen_addr: "0.0.0.0:8080".parse().unwrap(),
        request_timeout: Duration::from_secs(30),
        ..Default::default()
    };
    
    // Build service orchestrator
    let mut orchestrator = ServiceBuilder::new()
        .with_service_discovery(discovery)
        .with_game_engine(game_engine_config)
        .with_consensus(consensus_config)
        .with_gateway(gateway_config)
        .build()
        .await?;
    
    println!("📡 Starting all services...");
    
    // Start all services
    orchestrator.start_all().await?;
    
    println!("✅ All services started successfully!");
    println!("🌐 API Gateway listening on http://0.0.0.0:8080");
    println!("🎮 Game Engine Service: http://127.0.0.1:8081");
    println!("🤝 Consensus Service: http://127.0.0.1:8082");
    println!();
    println!("Available endpoints:");
    println!("  GET  /health           - Gateway health check");
    println!("  GET  /metrics          - Gateway metrics");
    println!("  POST /api/v1/games     - Create new game");
    println!("  GET  /api/v1/games     - List active games");
    println!("  GET  /api/v1/games/{id} - Get game state");
    println!("  POST /api/v1/games/{id}/actions - Perform game action");
    println!("  POST /api/v1/consensus/propose - Submit consensus proposal");
    println!("  POST /api/v1/consensus/vote    - Vote on proposal");
    println!("  GET  /api/v1/consensus/status  - Consensus status");
    println!();
    
    // Health check demo
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let health = orchestrator.health_check_all().await;
            println!("🏥 Service Health Check:");
            for (service, status) in health {
                let status_emoji = match status {
                    bitcraps::services::ServiceHealth::Healthy => "✅",
                    bitcraps::services::ServiceHealth::Degraded => "⚠️",
                    bitcraps::services::ServiceHealth::Unhealthy => "❌",
                };
                println!("  {} {}: {:?}", status_emoji, service, status);
            }
            println!();
        }
    });
    
    // Example API calls
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(3)).await;
        
        println!("🧪 Running API demo calls...");
        let client = reqwest::Client::new();
        
        // Health check
        match client.get("http://localhost:8080/health").send().await {
            Ok(response) => {
                println!("  Health Check: {} - {}", response.status(), 
                    response.text().await.unwrap_or_default());
            },
            Err(e) => println!("  Health Check failed: {}", e),
        }
        
        // Metrics
        match client.get("http://localhost:8080/metrics").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    println!("  Metrics: Available ✅");
                } else {
                    println!("  Metrics: {} ❌", response.status());
                }
            },
            Err(e) => println!("  Metrics failed: {}", e),
        }
        
        // Consensus status
        match client.get("http://localhost:8080/api/v1/consensus/status").send().await {
            Ok(response) => {
                if response.status().is_success() {
                    println!("  Consensus Status: Available ✅");
                } else {
                    println!("  Consensus Status: {} ❌", response.status());
                }
            },
            Err(e) => println!("  Consensus Status failed: {}", e),
        }
        
        println!("✨ Demo complete! Services running...");
    });
    
    println!("Press Ctrl+C to stop all services");
    
    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(_) => {},
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        },
    }
    
    println!("\n🛑 Shutting down services...");
    
    // Stop all services
    orchestrator.stop_all().await?;
    
    println!("✅ All services stopped successfully!");
    
    Ok(())
}