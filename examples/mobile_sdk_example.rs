//! BitCraps Mobile SDK Example
//!
//! This example shows how to use BitCraps on mobile platforms with:
//! 1. UniFFI bindings for cross-platform compatibility
//! 2. Battery-optimized discovery and connection
//! 3. Mobile-friendly game flows
//! 4. Platform-specific optimizations

use bitcraps::mobile::{
    create_node, BetType, BitCrapsConfig, GameConfig, LogLevel, PlatformConfig, PlatformType,
    PowerMode,
};
use std::time::Duration;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for mobile development
    tracing_subscriber::init();

    info!("🚀 Starting BitCraps Mobile SDK Example");

    // 1. Configure for mobile platform
    let platform_config = PlatformConfig {
        platform: PlatformType::Android, // Would be detected automatically in real app
        background_scanning: true,
        scan_window_ms: 1000,    // 1 second scan windows for battery efficiency
        scan_interval_ms: 10000, // 10 second intervals between scans
        low_power_mode: true,
        service_uuids: vec![
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(), // BitCraps service UUID
        ],
    };

    let config = BitCrapsConfig {
        // UniFFI compatible fields
        bluetooth_name: "MyPhone-BitCraps".to_string(),
        enable_battery_optimization: true,
        max_peers: 8,                  // Lower for mobile
        discovery_timeout_seconds: 15, // Shorter timeout for responsive UI

        // Full configuration
        data_dir: "./mobile_data".to_string(),
        pow_difficulty: 4, // Lower for mobile CPUs
        protocol_version: 1,
        power_mode: PowerMode::BatterySaver,
        platform_config: Some(platform_config),
        enable_logging: true,
        log_level: LogLevel::Info,
    };

    info!("📱 Creating BitCraps node with mobile-optimized config");
    let node = create_node(config)?;

    // 2. Start discovery with battery optimization
    info!("🔍 Starting Bluetooth discovery (battery optimized)");
    match node.start_discovery().await {
        Ok(_) => info!("✓ Discovery started successfully"),
        Err(e) => {
            warn!("Discovery failed (expected in test environment): {}", e);
            info!("Continuing with mobile demo...");
        }
    }

    // Wait a bit for discovery
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 3. Check discovered peers
    match node.get_discovered_peers().await {
        Ok(peers) => {
            info!("✓ Discovered {} peers:", peers.len());
            for (i, peer) in peers.iter().enumerate() {
                info!("  {}. Peer: {}", i + 1, peer);
            }
        }
        Err(e) => {
            info!("Peer discovery completed (expected in test mode): {}", e);
        }
    }

    // 4. Get mobile-friendly status
    let status = node.get_status();
    info!("📊 Node Status:");
    info!("  State: {:?}", status.state);
    info!("  Bluetooth Enabled: {}", status.bluetooth_enabled);
    info!("  Discovery Active: {}", status.discovery_active);
    info!("  Active Connections: {}", status.active_connections);
    info!("  Power Mode: {:?}", status.current_power_mode);

    // 5. Create a mobile-friendly game
    let game_config = GameConfig {
        game_name: Some("Quick Mobile Craps".to_string()),
        min_bet: 1, // Low stakes for mobile
        max_bet: 50,
        max_players: 4,       // Smaller games for mobile
        timeout_seconds: 120, // 2 minutes for responsive mobile play
    };

    info!("🎮 Creating mobile game...");
    match node.create_game(game_config).await {
        Ok(game_handle) => {
            info!("✓ Game created! ID: {}", game_handle.get_game_id());

            // 6. Demonstrate mobile game interaction
            info!("🎲 Placing mobile-friendly bets...");

            // Small bets suitable for mobile play
            let mobile_bets = vec![(BetType::Pass, 5), (BetType::Field, 2)];

            for (bet_type, amount) in mobile_bets {
                match game_handle.place_bet(bet_type, amount).await {
                    Ok(_) => info!("✓ Placed bet: {:?} for {} CRAP", bet_type, amount),
                    Err(e) => info!("Bet placement demo: {} - {:?}", e, bet_type),
                }
            }

            // 7. Roll dice with mobile optimization
            info!("🎲 Rolling dice...");
            match game_handle.roll_dice().await {
                Ok(_) => {
                    info!("✓ Dice rolled successfully!");

                    // Get the roll result
                    if let Some(roll) = game_handle.get_last_roll().await {
                        info!(
                            "🎲 Roll result: {} + {} = {}",
                            roll.die1,
                            roll.die2,
                            roll.die1 + roll.die2
                        );
                        info!("  Rolled by: {}", roll.roller_peer_id);
                    }
                }
                Err(e) => info!("Dice roll demo completed: {}", e),
            }

            // 8. Get game history for mobile UI
            let history = game_handle.get_game_history();
            info!("📜 Game History ({} events):", history.len());
            for (i, event) in history.iter().take(5).enumerate() {
                info!("  {}. {:?}", i + 1, event);
            }
        }
        Err(e) => {
            info!(
                "Game creation demo completed (expected in test mode): {}",
                e
            );
        }
    }

    // 9. Mobile-specific power management
    info!("⚡ Demonstrating power management...");

    // Switch to ultra-low power mode (like when app goes to background)
    if let Err(e) = node.set_power_mode(PowerMode::UltraLowPower) {
        info!("Power mode demo: {}", e);
    } else {
        info!("✓ Switched to ultra-low power mode");
    }

    // Adjust scan intervals for battery life
    if let Err(e) = node.set_scan_interval(30000) {
        // 30 seconds when backgrounded
        info!("Scan interval demo: {}", e);
    } else {
        info!("✓ Adjusted scan interval for battery optimization");
    }

    // 10. Get network stats for mobile UI
    let network_stats = node.get_network_stats();
    info!("📶 Network Statistics (for mobile UI):");
    info!("  Peers Discovered: {}", network_stats.peers_discovered);
    info!("  Active Connections: {}", network_stats.active_connections);
    info!("  Data Sent: {} bytes", network_stats.bytes_sent);
    info!("  Data Received: {} bytes", network_stats.bytes_received);
    info!("  Packets Dropped: {}", network_stats.packets_dropped);

    // 11. Get peer info for mobile peer list UI
    let connected_peers = node.get_connected_peers();
    info!("👥 Connected Peers ({} total):", connected_peers.len());
    for peer in connected_peers.iter().take(3) {
        info!(
            "  📱 {}: Signal {}% (last seen: {})",
            peer.display_name.as_deref().unwrap_or(&peer.peer_id),
            peer.signal_strength,
            peer.last_seen
        );
    }

    // 12. Event polling for mobile UI updates
    info!("📱 Polling for mobile UI events...");
    for i in 0..3 {
        if let Some(event) = node.poll_event().await {
            info!("🎮 Mobile Event {}: {:?}", i + 1, event);
        } else {
            info!("📱 No events (poll {})", i + 1);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // 13. Drain all events for UI synchronization
    let all_events = node.drain_events().await;
    if !all_events.is_empty() {
        info!("📱 Drained {} events for UI sync", all_events.len());
    }

    // 14. Stop discovery to save battery
    info!("🔋 Stopping discovery to save battery...");
    if let Err(e) = node.stop_discovery().await {
        info!("Discovery stop demo: {}", e);
    } else {
        info!("✓ Discovery stopped, battery optimized");
    }

    // 15. Leave game before exiting (good mobile practice)
    info!("🚪 Leaving game (mobile cleanup)...");
    if let Err(e) = node.leave_game().await {
        info!("Leave game demo: {}", e);
    } else {
        info!("✓ Left game cleanly");
    }

    info!("🎉 BitCraps Mobile SDK Example completed!");
    info!("📱 Mobile Development Notes:");
    info!("  - Use battery-optimized scanning intervals");
    info!("  - Lower PoW difficulty for mobile CPUs");
    info!("  - Smaller games and lower stakes for mobile UX");
    info!("  - Event polling pattern for reactive UI updates");
    info!("  - Power management integration");
    info!("  - Clean resource management on app lifecycle");

    Ok(())
}
#![cfg(feature = "legacy-examples")]
