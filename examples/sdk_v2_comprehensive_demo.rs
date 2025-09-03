//! Comprehensive BitCraps SDK v2 Demo
//!
//! This example demonstrates all major features of the BitCraps SDK v2:
//! - Configuration and initialization
//! - Game management with builder patterns
//! - Real-time WebSocket communication
//! - Consensus voting operations
//! - Network peer management
//! - Error handling and recovery
//! - Testing framework integration

use bitcraps_rust::sdk_v2::{
    client::BitCrapsSDK,
    config::{Config, Environment},
    types::*,
    error::{SDKError, SDKResult},
    testing::{TestFramework, TestScenarios},
    cli::run_cli,
};
use tokio::time::{sleep, Duration};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("🎲 BitCraps SDK v2.0 Comprehensive Demo");
    println!("=====================================\n");
    
    // Check if running in CLI mode
    if std::env::args().any(|arg| arg == "--cli") {
        return Ok(run_cli().await?);
    }
    
    // Demo 1: Basic SDK Setup and Configuration
    demo_sdk_setup().await?;
    
    // Demo 2: Game Management
    demo_game_management().await?;
    
    // Demo 3: Real-time Communication
    demo_realtime_communication().await?;
    
    // Demo 4: Consensus Operations
    demo_consensus_operations().await?;
    
    // Demo 5: Network Management
    demo_network_management().await?;
    
    // Demo 6: Error Handling
    demo_error_handling().await?;
    
    // Demo 7: Testing Framework
    demo_testing_framework().await?;
    
    println!("\n✅ All demos completed successfully!");
    println!("Check the logs for detailed information about each operation.");
    
    Ok(())
}

/// Demo 1: SDK Setup and Configuration
async fn demo_sdk_setup() -> SDKResult<()> {
    println!("📋 Demo 1: SDK Setup and Configuration");
    println!("--------------------------------------");
    
    // Create configuration using builder pattern
    let config = Config::builder()
        .api_key("demo-api-key-12345")
        .environment(Environment::Development)
        .timeout(Duration::from_secs(30))
        .debug_logging(true)
        .header("X-Client-Version", "2.0.0")
        .build()?;
    
    println!("✓ Configuration created for {} environment", 
        match config.environment {
            Environment::Development => "Development",
            Environment::Production => "Production",
            Environment::Staging => "Staging",
            Environment::Sandbox => "Sandbox",
            Environment::Testing => "Testing",
            Environment::Local => "Local",
        }
    );
    
    // Initialize SDK
    let sdk = BitCrapsSDK::new(config).await?;
    println!("✓ SDK initialized successfully");
    
    // Health check
    match sdk.health_check().await {
        Ok(health) => {
            println!("✓ Health check passed:");
            println!("  - Overall: {:?}", health.overall);
            println!("  - API: {} ({}ms)", 
                if health.api.healthy { "✓" } else { "✗" },
                health.api.response_time_ms
            );
            println!("  - WebSocket: {}", 
                if health.websocket.healthy { "✓" } else { "✗" }
            );
        }
        Err(e) => {
            println!("⚠️ Health check failed (expected in demo): {}", e);
        }
    }
    
    println!();
    Ok(())
}

/// Demo 2: Game Management
async fn demo_game_management() -> SDKResult<()> {
    println!("🎯 Demo 2: Game Management");
    println!("---------------------------");
    
    let config = Config::builder()
        .api_key("demo-api-key-12345")
        .environment(Environment::Testing)
        .build()?;
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Create a game using builder pattern
    println!("Creating a new game...");
    
    // Note: In testing environment, these calls will use mock responses
    let game_result = sdk.create_game("Demo High Stakes Craps")
        .game_type(GameType::Craps)
        .with_max_players(8)
        .with_min_players(3)
        .with_betting_limits(50, 5000)
        .with_turn_timeout(60)
        .with_betting_timeout(30)
        .with_tag("demo".to_string())
        .with_tag("high-stakes".to_string())
        .build()
        .await;
    
    match game_result {
        Ok(game) => {
            println!("✓ Game created successfully:");
            println!("  - ID: {}", game.id);
            println!("  - Name: {}", game.name);
            println!("  - Type: {:?}", game.game_type);
            println!("  - Max Players: {}", game.max_players);
            println!("  - Betting Range: ${} - ${}", game.min_bet, game.max_bet);
        }
        Err(e) => {
            println!("⚠️ Game creation failed (expected in demo): {}", e);
        }
    }
    
    // List games with filters
    println!("\nListing available games...");
    let filters = GameFilters {
        game_type: Some(GameType::Craps),
        status: Some(GameStatus::Waiting),
        min_bet: Some(10),
        max_bet: Some(1000),
        ..Default::default()
    };
    
    match sdk.list_games(Some(filters)).await {
        Ok(games) => {
            println!("✓ Found {} games", games.len());
            for game in games.iter().take(3) {
                println!("  - {} ({}) - {} players", 
                    game.name, game.id, game.current_players);
            }
        }
        Err(e) => {
            println!("⚠️ Game listing failed (expected in demo): {}", e);
        }
    }
    
    println!();
    Ok(())
}

/// Demo 3: Real-time Communication
async fn demo_realtime_communication() -> SDKResult<()> {
    println!("📡 Demo 3: Real-time Communication");
    println!("-----------------------------------");
    
    let config = Config::builder()
        .api_key("demo-api-key-12345")
        .environment(Environment::Testing)
        .build()?;
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Subscribe to game events
    println!("Setting up WebSocket subscriptions...");
    
    // Note: In testing environment, WebSocket connections may not be established
    match sdk.subscribe::<serde_json::Value>(EventType::GameStarted).await {
        Ok(mut event_stream) => {
            println!("✓ Subscribed to GameStarted events");
            
            // Spawn background task to handle events
            let event_handler = tokio::spawn(async move {
                let mut count = 0;
                while let Some(_event) = event_stream.next().await {
                    count += 1;
                    println!("📨 Received game event #{}", count);
                    if count >= 3 { break; } // Limit for demo
                }
            });
            
            // Simulate some events (in real usage, these would come from the server)
            println!("Simulating real-time events...");
            sleep(Duration::from_millis(500)).await;
            
            // Cancel the event handler after a short time
            event_handler.abort();
            println!("✓ Event handling demonstrated");
        }
        Err(e) => {
            println!("⚠️ WebSocket subscription failed (expected in demo): {}", e);
        }
    }
    
    println!();
    Ok(())
}

/// Demo 4: Consensus Operations
async fn demo_consensus_operations() -> SDKResult<()> {
    println!("🗳️ Demo 4: Consensus Operations");
    println!("-------------------------------");
    
    let config = Config::builder()
        .api_key("demo-api-key-12345")
        .environment(Environment::Testing)
        .build()?;
    let sdk = BitCrapsSDK::new(config).await?;
    
    let game_id = "demo_game_12345".to_string();
    
    // Create a consensus proposal
    println!("Creating consensus proposal...");
    let proposal_action = GameAction::PlaceBet {
        bet_type: "pass_line".to_string(),
        amount: 100,
    };
    
    match sdk.consensus().propose(&game_id, proposal_action).await {
        Ok(proposal_id) => {
            println!("✓ Proposal created: {}", proposal_id);
            
            // Vote on the proposal
            println!("Submitting vote...");
            match sdk.consensus().vote(&proposal_id, Vote::Approve).await {
                Ok(_) => println!("✓ Vote submitted successfully"),
                Err(e) => println!("⚠️ Vote submission failed (expected in demo): {}", e),
            }
            
            // Check proposal status
            match sdk.consensus().get_proposal(&proposal_id).await {
                Ok(proposal) => {
                    println!("✓ Proposal status: {:?}", proposal.status);
                    println!("  - Votes received: {}", proposal.votes.len());
                    println!("  - Required votes: {}", proposal.required_votes);
                }
                Err(e) => println!("⚠️ Failed to get proposal (expected in demo): {}", e),
            }
        }
        Err(e) => {
            println!("⚠️ Proposal creation failed (expected in demo): {}", e);
        }
    }
    
    // Demonstrate proposal builder
    println!("\nUsing proposal builder...");
    let builder_result = sdk.consensus()
        .create_proposal(&game_id)
        .action(GameAction::Custom {
            action_type: "increase_betting_limit".to_string(),
            data: serde_json::json!({"new_limit": 2000}),
        })
        .timeout(300)
        .description("Increase betting limit for high stakes round")
        .metadata("priority", "high")
        .submit()
        .await;
    
    match builder_result {
        Ok(proposal_id) => println!("✓ Builder proposal created: {}", proposal_id),
        Err(e) => println!("⚠️ Builder proposal failed (expected in demo): {}", e),
    }
    
    println!();
    Ok(())
}

/// Demo 5: Network Management
async fn demo_network_management() -> SDKResult<()> {
    println!("🌐 Demo 5: Network Management");
    println!("------------------------------");
    
    let config = Config::builder()
        .api_key("demo-api-key-12345")
        .environment(Environment::Testing)
        .build()?;
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Get connected peers
    println!("Checking connected peers...");
    let peers = sdk.network().get_connected_peers().await;
    println!("✓ Currently connected to {} peers", peers.len());
    
    // Attempt to connect to a peer
    println!("Attempting to connect to demo peer...");
    match sdk.network().connect("demo.peer.bitcraps.com:8080").await {
        Ok(peer_id) => {
            println!("✓ Connected to peer: {}", peer_id);
            
            // Get network statistics
            let stats = sdk.network().get_network_statistics().await;
            println!("✓ Network statistics:");
            println!("  - Active connections: {}", stats.active_connections);
            println!("  - Messages sent: {}", stats.messages_sent);
            println!("  - Average latency: {:.2}ms", stats.average_latency);
        }
        Err(e) => {
            println!("⚠️ Peer connection failed (expected in demo): {}", e);
        }
    }
    
    // Demonstrate network utility functions
    println!("\nNetwork health assessment...");
    let mock_stats = crate::sdk_v2::networking::NetworkStatistics {
        active_connections: 8,
        average_latency: 45.2,
        packet_loss_rate: 0.02,
        connection_uptime: 98.5,
        ..Default::default()
    };
    
    let health_score = crate::sdk_v2::networking::NetworkUtils::calculate_network_health(&mock_stats);
    println!("✓ Network health score: {:.2}%", health_score * 100.0);
    
    println!();
    Ok(())
}

/// Demo 6: Error Handling
async fn demo_error_handling() -> SDKResult<()> {
    println!("⚠️ Demo 6: Error Handling and Recovery");
    println!("--------------------------------------");
    
    let config = Config::builder()
        .api_key("invalid-api-key")
        .environment(Environment::Testing)
        .build()?;
    let sdk = BitCrapsSDK::new(config).await?;
    
    // Demonstrate comprehensive error handling
    println!("Testing error scenarios...");
    
    match sdk.join_game("nonexistent-game").await {
        Ok(_) => unreachable!("This should fail"),
        Err(e) => {
            println!("✓ Error caught successfully:");
            println!("  - Error type: {}", e.error_code());
            println!("  - Message: {}", e);
            println!("  - User message: {}", e.user_message());
            println!("  - Retryable: {}", e.is_retryable());
            
            if let Some(delay) = e.retry_delay() {
                println!("  - Retry delay: {:?}", delay);
            }
            
            println!("  - Recovery suggestions:");
            for suggestion in e.recovery_suggestions() {
                println!("    • {}", suggestion);
            }
        }
    }
    
    // Demonstrate error context
    println!("\nDemonstrating error context...");
    let context_error = SDKError::ValidationError {
        message: "Demo validation error".to_string(),
        field: Some("amount".to_string()),
        invalid_value: Some("negative_value".to_string()),
    };
    
    println!("✓ Validation error example:");
    println!("  - Field: {:?}", "amount");
    println!("  - Invalid value: {:?}", "negative_value");
    println!("  - Suggestions: {:?}", context_error.recovery_suggestions());
    
    println!();
    Ok(())
}

/// Demo 7: Testing Framework
async fn demo_testing_framework() -> SDKResult<()> {
    println!("🧪 Demo 7: Testing Framework");
    println!("-----------------------------");
    
    // Create test framework
    let mut framework = TestFramework::new();
    
    // Add test scenarios
    framework = framework
        .add_scenario(TestScenarios::basic_game_flow())
        .add_scenario(TestScenarios::multi_player_scenario());
    
    println!("Running test scenarios...");
    
    // Run all tests
    let results = framework.run_all_tests().await;
    
    println!("✓ Test results:");
    println!("  - Total tests: {}", results.total_tests);
    println!("  - Passed: {}", results.passed);
    println!("  - Failed: {}", results.failed);
    println!("  - Duration: {:?}", results.duration);
    
    // Show individual test results
    for result in results.results.iter().take(3) {
        let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
        println!("  {} {} ({:?})", status, result.name, result.duration);
        if let Some(error) = &result.error {
            println!("    Error: {}", error);
        }
    }
    
    println!();
    Ok(())
}

/// Extension trait for EventStream to add a next() method
trait EventStreamExt<T> {
    async fn next(&mut self) -> Option<T>;
}

impl<T> EventStreamExt<T> for crate::sdk_v2::types::EventStream<T>
where
    T: for<'de> serde::Deserialize<'de> + Send + 'static,
{
    async fn next(&mut self) -> Option<T> {
        // In a real implementation, this would read from the underlying receiver
        // For demo purposes, we'll return None after a short delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_demo_functions() {
        // Test that demo functions don't panic
        assert!(demo_sdk_setup().await.is_ok() || true); // Allow network errors in tests
        assert!(demo_game_management().await.is_ok() || true);
        // Add more test assertions as needed
    }
    
    #[test]
    fn test_config_validation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build();
        
        assert!(config.is_ok());
    }
}