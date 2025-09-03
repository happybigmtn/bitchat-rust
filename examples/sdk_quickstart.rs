//! BitCraps SDK Quickstart Example
//!
//! This example demonstrates how to use the BitCraps SDK to:
//! 1. Initialize a client
//! 2. Connect to the network
//! 3. Discover nearby games
//! 4. Create and join games
//! 5. Place bets and interact with the game

use bitcraps::gaming::{GameAction, GameSessionConfig};
use bitcraps::protocol::CrapTokens;
use bitcraps::sdk::client::{
    AuthCredentials, BitCrapsClient, ClientConfig, GameStatus, NetworkConfig, RetryConfig,
    TimeoutConfig, UserRegistrationData,
};
use std::collections::HashMap;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();

    info!("Starting BitCraps SDK Quickstart Example");

    // 1. Initialize the SDK client with configuration
    let config = ClientConfig {
        client_id: "quickstart-example".to_string(),
        network_config: NetworkConfig {
            connect_timeout_seconds: 10,
            keepalive_interval_seconds: 30,
            max_reconnect_attempts: 3,
        },
        game_framework_config: bitcraps::gaming::GameFrameworkConfig::default(),
        retry_config: RetryConfig::default(),
        timeout_config: TimeoutConfig {
            operation_timeout_seconds: 15,
            batch_timeout_seconds: 60,
        },
    };

    let client = BitCrapsClient::new(config).await?;
    info!("✓ SDK client initialized successfully");

    // 2. Connect to the BitCraps network
    info!("Connecting to BitCraps network...");
    if let Err(e) = client.connect().await {
        warn!("Connection failed (expected in example environment): {}", e);
        info!("Continuing with offline demonstration...");
    }

    // 3. User Registration and Authentication
    info!("Demonstrating user registration...");
    let user_registration = UserRegistrationData {
        password: "secure_example_password_123".to_string(),
        email: Some("player@example.com".to_string()),
        initial_balance: Some(1000), // 1000 CRAP tokens
        metadata: HashMap::new(),
    };

    if let Err(e) = client
        .register_user("example_player".to_string(), user_registration)
        .await
    {
        info!(
            "Registration demo completed (expected error in example): {}",
            e
        );
    }

    // 4. Authentication
    info!("Demonstrating authentication...");
    let credentials = AuthCredentials::Password("secure_example_password_123".to_string());
    match client
        .authenticate("example_player".to_string(), credentials)
        .await
    {
        Ok(auth_token) => {
            info!(
                "✓ Authentication successful! Token expires: {:?}",
                auth_token.expires_at
            );
        }
        Err(e) => {
            info!(
                "Authentication demo completed (expected error in example): {}",
                e
            );
        }
    }

    // 5. Discover nearby games
    info!("Discovering nearby games...");
    match client.discover_games(10).await {
        Ok(games) => {
            info!("✓ Found {} discoverable games:", games.len());
            for game in &games {
                info!(
                    "  - {} ({}): {}-{} CRAP, {}/{} players",
                    game.game_name,
                    game.game_type,
                    game.min_bet,
                    game.max_bet,
                    game.current_players,
                    game.max_players
                );
            }
        }
        Err(e) => {
            info!(
                "Game discovery demo completed (expected in offline mode): {}",
                e
            );
        }
    }

    // 6. Quick game creation
    info!("Creating a quick game...");
    match client.quick_create_game("craps", 10, 100).await {
        Ok(result) => {
            info!("✓ Game created successfully!");
            info!("  Session ID: {}", result.session_id);
            info!("  Game Code: {}", result.game_code);
            info!("  Join URL: {}", result.join_url);
            info!("  Share this code with friends: {}", result.game_code);
        }
        Err(e) => {
            info!(
                "Game creation demo completed (expected in offline mode): {}",
                e
            );
        }
    }

    // 7. Traditional game creation with full config
    info!("Creating a traditional game session...");
    let session_config = GameSessionConfig {
        min_bet: 5,
        max_bet: 500,
        auto_start: false,
        game_specific_config: HashMap::from([
            ("dice_type".to_string(), "standard".to_string()),
            ("max_rounds".to_string(), "50".to_string()),
        ]),
    };

    match client.create_game_session("craps", session_config).await {
        Ok(session_id) => {
            info!("✓ Traditional game session created: {}", session_id);

            // 8. Join the game we just created (self-join for demo)
            info!("Demonstrating game joining...");
            if let Err(e) = client
                .join_game_session(&session_id, "example_player".to_string(), 500)
                .await
            {
                info!("Game join demo completed (expected error): {}", e);
            }

            // 9. Place some example bets
            info!("Demonstrating bet placement...");
            let bet_actions = vec![
                GameAction::PlaceBet {
                    bet_type: "pass".to_string(),
                    amount: 25,
                },
                GameAction::PlaceBet {
                    bet_type: "field".to_string(),
                    amount: 10,
                },
            ];

            for action in bet_actions {
                match client
                    .perform_action(&session_id, "example_player", action.clone())
                    .await
                {
                    Ok(result) => {
                        info!("✓ Action completed: {:?}", result);
                    }
                    Err(e) => {
                        info!(
                            "Action demo completed (expected error): {} - {:?}",
                            e, action
                        );
                    }
                }
            }
        }
        Err(e) => {
            info!(
                "Traditional game creation demo completed (expected in offline mode): {}",
                e
            );
        }
    }

    // 10. Wallet operations
    info!("Demonstrating wallet operations...");
    match client.get_wallet_balance().await {
        Ok(balance) => {
            info!("✓ Current wallet balance: {} CRAP tokens", balance.0);

            // Demo transfer
            if balance.0 >= 50 {
                match client
                    .transfer_tokens("friend_player".to_string(), CrapTokens(25))
                    .await
                {
                    Ok(tx_id) => {
                        info!("✓ Transfer successful! Transaction ID: {}", tx_id);
                    }
                    Err(e) => {
                        info!("Transfer demo completed (expected error): {}", e);
                    }
                }
            }
        }
        Err(e) => {
            info!("Wallet demo completed (expected error): {}", e);
        }
    }

    // 11. Get transaction history
    match client.get_transaction_history(Some(10)).await {
        Ok(transactions) => {
            info!(
                "✓ Transaction history ({} transactions):",
                transactions.len()
            );
            for tx in &transactions {
                info!(
                    "  - {} at {}: {:?}",
                    tx.id, tx.timestamp, tx.transaction_type
                );
            }
        }
        Err(e) => {
            info!("Transaction history demo completed (expected error): {}", e);
        }
    }

    // 12. Network and client statistics
    info!("Getting client statistics...");
    let stats = client.get_statistics().await;
    info!("✓ Client Statistics:");
    info!("  Connections: {}", stats.connection_count);
    info!("  Sessions Created: {}", stats.sessions_created);
    info!("  Players Joined: {}", stats.players_joined);
    info!("  Actions Performed: {}", stats.actions_performed);
    info!("  Uptime: {} seconds", stats.uptime_seconds);

    // 13. Health check
    match client.health_check().await {
        Ok(health) => {
            info!("✓ Health Status: {:?}", health);
        }
        Err(e) => {
            warn!("Health check failed: {}", e);
        }
    }

    // 14. Network information
    match client.get_network_info().await {
        Ok(network_info) => {
            info!("✓ Network Information:");
            info!("  Peer Count: {}", network_info.peer_count);
            info!(
                "  Network Latency: {:.2}ms",
                network_info.network_latency_ms
            );
            info!("  Network Health: {:?}", network_info.network_health);
            info!("  Protocol Version: {}", network_info.protocol_version);
        }
        Err(e) => {
            info!(
                "Network info demo completed (expected in offline mode): {}",
                e
            );
        }
    }

    // 15. Event handling demonstration
    info!("Setting up event handlers...");
    client
        .register_event_handler("game_created", SimpleGameEventHandler)
        .await?;

    // Subscribe to events
    let mut event_receiver = client.subscribe_to_events().await?;

    // Poll for a few events (with timeout)
    info!("Polling for events (3 second timeout)...");
    let timeout_duration = tokio::time::Duration::from_secs(3);

    match tokio::time::timeout(timeout_duration, event_receiver.recv()).await {
        Ok(Ok(event)) => {
            info!("✓ Received event: {:?}", event);
        }
        Ok(Err(e)) => {
            info!("Event receiving completed: {}", e);
        }
        Err(_) => {
            info!("✓ Event polling timeout (no events in 3 seconds - this is normal)");
        }
    }

    info!("🎉 BitCraps SDK Quickstart Example completed successfully!");
    info!("📚 Key takeaways:");
    info!("  - SDK client initialization and configuration");
    info!("  - User registration and authentication flows");
    info!("  - Game discovery and creation patterns");
    info!("  - Wallet operations and transaction handling");
    info!("  - Event-driven programming with the SDK");
    info!("  - Health monitoring and statistics");

    Ok(())
}

/// Simple event handler for demonstration
struct SimpleGameEventHandler;

#[async_trait::async_trait]
impl bitcraps::sdk::client::EventHandler for SimpleGameEventHandler {
    async fn handle_event(
        &self,
        event: &bitcraps::gaming::GameFrameworkEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("🎮 Game Event: {:?}", event);
        Ok(())
    }
}
#![cfg(feature = "legacy-examples")]
