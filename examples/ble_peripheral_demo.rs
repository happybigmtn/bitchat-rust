//! BLE Peripheral Advertising Demo
//!
//! This example demonstrates how to use the new BLE peripheral advertising
//! functionality in BitChat-Rust. It shows both the basic and advanced
//! usage patterns for different platforms.

use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use bitcraps::error::Result;
use bitcraps::protocol::PeerId;
use bitcraps::transport::{
    AdvertisingConfig, BleConfigBuilder, BleTransportInitializer, TransportEvent,
    BITCRAPS_SERVICE_UUID,
};

/// Generate a test peer ID
fn generate_test_peer_id() -> PeerId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    "demo_peer".hash(&mut hasher);
    let hash = hasher.finish();

    let mut peer_id = [0u8; 32];
    peer_id[..8].copy_from_slice(&hash.to_be_bytes());
    peer_id
}

/// Basic BLE peripheral advertising example
async fn basic_example() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("🟦 Starting Basic BLE Peripheral Advertising Example");

    // Generate a peer ID for this demo
    let local_peer_id = generate_test_peer_id();
    println!("Local peer ID: {:?}", hex::encode(local_peer_id));

    // Create basic configuration
    let config = AdvertisingConfig {
        service_uuid: BITCRAPS_SERVICE_UUID,
        local_name: "BitChat-Demo-Basic".to_string(),
        advertising_interval_ms: 1000, // 1 second
        tx_power_level: 0,
        include_name: true,
        connectable: true,
        max_connections: 4,
    };

    // Initialize transport
    let initializer = BleTransportInitializer::new(
        local_peer_id,
        bitcraps::transport::BleTransportConfig {
            advertising: config,
            enable_peripheral: true,
            enable_central: false, // Central only for this example
            auto_start_mesh: false,
            ..Default::default()
        },
    );

    // Validate configuration
    initializer.validate_config()?;

    // Get platform capabilities
    let capabilities = initializer.get_platform_capabilities();
    println!("Platform: {}", capabilities.platform);
    println!("Supports peripheral: {}", capabilities.supports_peripheral);
    println!("Supports central: {}", capabilities.supports_central);

    if !capabilities.supports_peripheral {
        println!("❌ Platform does not support BLE peripheral mode");
        return Ok(());
    }

    // Initialize transport coordinator
    let mut coordinator = initializer.initialize_transport().await?;

    // Start just advertising (not scanning)
    coordinator.start_ble_advertising(config.clone()).await?;

    println!("✅ BLE advertising started successfully");
    println!("📡 Advertising as: {}", config.local_name);
    println!("🔧 Service UUID: {}", config.service_uuid);

    // Monitor for events
    println!("🔍 Monitoring for connection events (30 seconds)...");

    let start_time = tokio::time::Instant::now();
    let timeout = Duration::from_secs(30);

    while start_time.elapsed() < timeout {
        // Check for transport events
        if let Some(event) = coordinator.next_event().await {
            match event {
                TransportEvent::Connected { peer_id, address } => {
                    println!(
                        "📱 Central connected: {:?} from {:?}",
                        hex::encode(peer_id),
                        address
                    );
                }
                TransportEvent::Disconnected { peer_id, reason } => {
                    println!(
                        "📱 Central disconnected: {:?} ({})",
                        hex::encode(peer_id),
                        reason
                    );
                }
                TransportEvent::DataReceived { peer_id, data } => {
                    println!(
                        "📥 Received {} bytes from {:?}: {:?}",
                        data.len(),
                        hex::encode(peer_id),
                        String::from_utf8_lossy(&data)
                    );
                }
                TransportEvent::Error { peer_id, error } => {
                    println!(
                        "❌ Transport error from {:?}: {}",
                        peer_id.map(hex::encode),
                        error
                    );
                }
            }
        }

        // Small delay to prevent busy waiting
        sleep(Duration::from_millis(100)).await;
    }

    // Stop advertising
    coordinator.stop_ble_advertising().await?;
    println!("⏹️ Stopped BLE advertising");

    // Get final statistics
    if let Ok(stats) = coordinator.get_enhanced_bluetooth_stats().await {
        println!("📊 Final Statistics:");
        println!("  - Total connections: {}", stats.total_connections);
        println!("  - Bytes sent: {}", stats.total_bytes_sent);
        println!("  - Bytes received: {}", stats.total_bytes_received);
        println!(
            "  - Advertising duration: {:?}",
            stats.peripheral_stats.advertising_duration
        );
    }

    Ok(())
}

/// Advanced BLE mesh networking example
async fn advanced_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🟨 Starting Advanced BLE Mesh Networking Example");

    let local_peer_id = generate_test_peer_id();
    println!("Local peer ID: {:?}", hex::encode(local_peer_id));

    // Create advanced configuration using builder
    let config = BleConfigBuilder::new()
        .service_uuid(BITCRAPS_SERVICE_UUID)
        .local_name("BitChat-Demo-Advanced".to_string())
        .advertising_interval(500) // 500ms
        .tx_power(4) // Higher power for better range
        .max_peripheral_connections(6)
        .connection_timeout(Duration::from_secs(15))
        .auto_start_mesh() // Enable both advertising and scanning
        .build();

    // Initialize with advanced configuration
    let initializer = BleTransportInitializer::new(local_peer_id, config);

    // Validate configuration
    initializer.validate_config()?;

    let capabilities = initializer.get_platform_capabilities();
    println!("Platform capabilities:");
    for limitation in &capabilities.limitations {
        println!("  ⚠️  {}", limitation);
    }
    for requirement in &capabilities.requirements {
        println!("  ✅ Required: {}", requirement);
    }

    // Initialize transport (auto-starts mesh mode)
    let mut coordinator = initializer.initialize_transport().await?;

    println!("✅ BLE mesh mode started (advertising + scanning)");

    // Send some test data
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;

            let peers = coordinator.connected_peers();
            let peer_list = peers.await;
            if !peer_list.is_empty() {
                let test_message = format!(
                    "Hello from BitChat! Time: {}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                );

                for peer_id in peer_list {
                    if let Err(e) = coordinator
                        .send_to_peer(peer_id, test_message.as_bytes().to_vec())
                        .await
                    {
                        println!("❌ Failed to send to {:?}: {}", hex::encode(peer_id), e);
                    } else {
                        println!("📤 Sent test message to {:?}", hex::encode(peer_id));
                    }
                }
            }
        }
    });

    // Monitor for events
    println!("🔍 Monitoring mesh network events (60 seconds)...");

    let start_time = tokio::time::Instant::now();
    let timeout = Duration::from_secs(60);

    let mut peer_count: u32 = 0;
    let mut message_count = 0;

    while start_time.elapsed() < timeout {
        if let Some(event) = coordinator.next_event().await {
            match event {
                TransportEvent::Connected { peer_id, address } => {
                    peer_count += 1;
                    println!(
                        "🤝 Peer connected: {:?} from {:?} (total: {})",
                        hex::encode(peer_id),
                        address,
                        peer_count
                    );
                }
                TransportEvent::Disconnected { peer_id, reason } => {
                    peer_count = peer_count.saturating_sub(1);
                    println!(
                        "👋 Peer disconnected: {:?} ({}) (remaining: {})",
                        hex::encode(peer_id),
                        reason,
                        peer_count
                    );
                }
                TransportEvent::DataReceived { peer_id, data } => {
                    message_count += 1;
                    println!(
                        "📨 Message #{} from {:?}: {}",
                        message_count,
                        hex::encode(peer_id),
                        String::from_utf8_lossy(&data)
                    );
                }
                TransportEvent::Error { peer_id, error } => {
                    println!(
                        "❌ Network error from {:?}: {}",
                        peer_id.map(hex::encode),
                        error
                    );
                }
            }
        }

        // Print periodic status
        if start_time.elapsed().as_secs() % 10 == 0 {
            if let Ok(stats) = coordinator.get_enhanced_bluetooth_stats().await {
                println!(
                    "📊 Status: {} connections, {} sent, {} received",
                    stats.total_connections, stats.total_bytes_sent, stats.total_bytes_received
                );
            }
        }

        sleep(Duration::from_millis(100)).await;
    }

    // Stop mesh mode
    // coordinator.stop_mesh_mode().await?; // Method not available
    println!("Demo completed - stopping advertising...");
    println!("⏹️ Stopped mesh mode");

    // Final statistics
    if let Ok(stats) = coordinator.get_enhanced_bluetooth_stats().await {
        println!("📊 Final Mesh Statistics:");
        println!("  - Peak connections: {}", stats.total_connections);
        println!("  - Messages sent: {} bytes", stats.total_bytes_sent);
        println!(
            "  - Messages received: {} bytes",
            stats.total_bytes_received
        );
        println!("  - Central connections: {}", stats.central_connections);
        println!(
            "  - Peripheral connections: {}",
            stats.peripheral_stats.active_connections
        );
        println!(
            "  - Total advertising time: {:?}",
            stats.peripheral_stats.advertising_duration
        );
        println!("  - Connection attempts: {}", stats.connection_attempts);
        println!(
            "  - Successful connections: {}",
            stats.successful_connections
        );
    }

    Ok(())
}

/// Platform-specific features demonstration
async fn platform_demo() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("\n🟩 Platform-Specific Features Demo");

    let capabilities = bitcraps::transport::PlatformCapabilities::for_current_platform();

    println!("Current Platform: {}", capabilities.platform);
    println!(
        "BLE Peripheral Support: {}",
        capabilities.supports_peripheral
    );
    println!("BLE Central Support: {}", capabilities.supports_central);
    println!("Background Support: {}", capabilities.supports_background);

    if let Some(max_conn) = capabilities.max_connections {
        println!("Maximum Connections: {}", max_conn);
    }

    if let Some((min, max)) = capabilities.advertising_interval_range {
        println!("Advertising Interval Range: {} - {} ms", min, max);
    }

    if let Some((min, max)) = capabilities.tx_power_range {
        println!("TX Power Range: {} to {} dBm", min, max);
    }

    println!("\nPlatform Limitations:");
    for limitation in &capabilities.limitations {
        println!("  ⚠️  {}", limitation);
    }

    println!("\nPlatform Requirements:");
    for requirement in &capabilities.requirements {
        println!("  📋 {}", requirement);
    }

    // Platform-specific configuration example
    let local_peer_id = generate_test_peer_id();

    #[cfg(target_os = "android")]
    {
        println!("\n📱 Android-Specific Configuration:");
        let mut config = bitcraps::transport::BleTransportConfig::default();
        config.platform_settings.android.use_foreground_service = true;
        config
            .platform_settings
            .android
            .request_battery_optimization_exemption = true;
        config.platform_settings.android.advertise_mode_preference =
            bitcraps::transport::AndroidAdvertiseMode::Balanced;

        println!("  - Foreground service: enabled");
        println!("  - Battery optimization exemption: requested");
        println!("  - Advertise mode: Balanced");
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        println!("\n🍎 iOS/macOS-Specific Configuration:");
        let mut config = bitcraps::transport::BleTransportConfig::default();
        config.platform_settings.ios.background_modes = vec!["bluetooth-peripheral".to_string()];
        config.platform_settings.ios.show_power_alert = true;

        println!("  - Background modes: bluetooth-peripheral");
        println!("  - Power alert: enabled");
        println!("  - State restoration: enabled");
    }

    #[cfg(target_os = "linux")]
    {
        println!("\n🐧 Linux-Specific Configuration:");
        let mut config = bitcraps::transport::BleTransportConfig::default();
        config.platform_settings.linux.adapter_name = "hci0".to_string();
        config.platform_settings.linux.auto_power_on = true;

        println!(
            "  - Bluetooth adapter: {}",
            config.platform_settings.linux.adapter_name
        );
        println!("  - Auto power-on: enabled");
        println!(
            "  - D-Bus timeout: {:?}",
            config.platform_settings.linux.dbus_timeout
        );
    }

    Ok(())
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🚀 BitChat BLE Peripheral Advertising Demo");
    println!("==========================================");

    // Check command line arguments
    let args: Vec<String> = std::env::args().collect();
    let demo_type = args.get(1).map(|s| s.as_str()).unwrap_or("basic");

    match demo_type {
        "basic" => {
            println!("Running basic BLE peripheral advertising demo...");
            basic_example().await?;
        }
        "advanced" => {
            println!("Running advanced BLE mesh networking demo...");
            advanced_example().await?;
        }
        "platform" => {
            println!("Running platform-specific features demo...");
            platform_demo().await?;
        }
        "all" => {
            println!("Running all demos...");
            basic_example().await?;
            advanced_example().await?;
            platform_demo().await?;
        }
        _ => {
            println!("Usage: {} [basic|advanced|platform|all]", args[0]);
            println!("  basic    - Basic BLE peripheral advertising");
            println!("  advanced - Advanced mesh networking");
            println!("  platform - Platform-specific features");
            println!("  all      - Run all demos");
            return Ok(());
        }
    }

    println!("\n✅ Demo completed successfully!");
    Ok(())
}
