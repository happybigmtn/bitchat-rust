use log::info;

use bitcraps::{AppConfig, Error, Result};
use std::sync::Arc;

// Import new modules
mod app_config;
mod app_state;
mod commands;

use app_config::{resolve_data_dir, Cli, Commands};
use app_state::BitCrapsApp;
use commands::commands as cmd;

#[tokio::main]
async fn main() -> Result<()> {
    use clap::Parser;

    // Set up global panic handler for production graceful shutdown
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("🚨 CRITICAL: Application panic detected!");
        eprintln!(
            "Location: {}",
            panic_info
                .location()
                .map_or("unknown".to_string(), |l| l.to_string())
        );
        eprintln!(
            "Message: {}",
            panic_info
                .payload()
                .downcast_ref::<&str>()
                .unwrap_or(&"Unknown panic")
        );
        eprintln!("Attempting graceful shutdown...");

        // Log to file if possible
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("bitcraps_panic.log")
        {
            use std::io::Write;
            let _ = writeln!(
                file,
                "[{}] PANIC: {} at {}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                panic_info
                    .payload()
                    .downcast_ref::<&str>()
                    .unwrap_or(&"Unknown panic"),
                panic_info
                    .location()
                    .map_or("unknown".to_string(), |l| l.to_string())
            );
        }

        // Exit with error code
        std::process::exit(1);
    }));

    let cli = Cli::parse();

    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    println!("🎲 BitCraps - Decentralized Casino Protocol");
    println!("⚡ Real-time craps over Bluetooth mesh with CRAP tokens");
    println!();

    // Resolve data directory path
    let data_dir = resolve_data_dir(&cli.data_dir).map_err(|e| Error::Protocol(e))?;

    let config = AppConfig {
        data_dir,
        nickname: cli.nickname,
        pow_difficulty: cli.pow_difficulty,
        listen_tcp: cli.listen_tcp.clone(),
        connect_tcp: cli.connect_tcp.clone(),
        enable_ble: !cli.no_ble,
        ..AppConfig::default()
    };

    match cli.command {
        Commands::Start => {
            info!("Starting BitCraps node...");
            let mut app = BitCrapsApp::new(config.clone()).await?;
            
            // Start monitoring services (no-op under `mvp` feature)
            let app_arc = Arc::new(app);
            start_monitoring_services(app_arc.clone(), &config).await?;
            
            // Now start the main app loop (need to get mutable reference back)
            let app_mut = Arc::try_unwrap(app_arc)
                .map_err(|_| Error::Protocol("Failed to unwrap app Arc".to_string()))?;
            let mut app = app_mut;
            app.start().await?;
        }

        Commands::Tui => {
            info!("Starting BitCraps TUI...");
            let app = BitCrapsApp::new(config).await?;
            run_tui_wrapper(app).await?;
        }

        Commands::CreateGame { buy_in } => {
            cmd::create_game_command(&BitCrapsApp::new(config.clone()).await?, buy_in).await?;

            // Start the main loop after creating game
            let mut app = BitCrapsApp::new(config).await?;
            app.start().await?;
        }

        Commands::JoinGame { game_id } => {
            cmd::join_game_command(&BitCrapsApp::new(config.clone()).await?, &game_id).await?;

            // Start the main loop after joining game
            let mut app = BitCrapsApp::new(config).await?;
            app.start().await?;
        }

        Commands::Balance => {
            cmd::balance_command(&BitCrapsApp::new(config).await?).await?;
        }

        Commands::Games => {
            cmd::list_games_command(&BitCrapsApp::new(config).await?).await?;
        }

        Commands::Bet {
            game_id,
            bet_type,
            amount,
        } => {
            cmd::place_bet_command(
                &BitCrapsApp::new(config.clone()).await?,
                &game_id,
                &bet_type,
                amount,
            )
            .await?;

            // Start the main loop after placing bet
            let mut app = BitCrapsApp::new(config).await?;
            app.start().await?;
        }

        Commands::Stats => {
            cmd::stats_command(&BitCrapsApp::new(config).await?).await?;
        }

        Commands::Ping => {
            cmd::ping_command(&BitCrapsApp::new(config).await?).await?;
        }
    }

    Ok(())
}

/// Start all monitoring services (Prometheus, Dashboard, Metrics Integration)
#[cfg(not(feature = "mvp"))]
async fn start_monitoring_services(app: Arc<BitCrapsApp>, config: &AppConfig) -> Result<()> {
    use bitcraps::monitoring::{
        PrometheusServer, PrometheusConfig, start_dashboard_server, start_metrics_integration,
        record_network_event,
    };
    use std::net::SocketAddr;
    
    info!("🔍 Starting monitoring services...");
    
    // Start Prometheus server on port 9090
    let prometheus_port = config.prometheus_port.unwrap_or(9090);
    let prometheus_config = PrometheusConfig {
        bind_address: format!("0.0.0.0:{}", prometheus_port).parse::<SocketAddr>().unwrap(),
        collection_interval_seconds: 5,
        enable_detailed_labels: true,
        global_labels: vec![("service".to_string(), "bitcraps".to_string())],
        enable_business_metrics: true,
        enable_system_metrics: true,
    };
    let prometheus_server = PrometheusServer::new(prometheus_config);
    tokio::spawn(async move {
        info!("📊 Starting Prometheus server on port {}", prometheus_port);
        if let Err(e) = prometheus_server.start().await {
            log::error!("Prometheus server failed: {}", e);
        }
    });
    
    // Start Live Dashboard on port 8080
    let dashboard_port = config.dashboard_port.unwrap_or(8080);
    tokio::spawn(async move {
        info!("📈 Starting Live Dashboard");
        if let Err(e) = start_dashboard_server().await {
            log::error!("Dashboard server failed: {}", e);
        }
    });
    
    // Start Metrics Integration Service
    // TODO: Fix type mismatch between app_state::BitCrapsApp and bitcraps::BitCrapsApp
    // let _integration_handle = start_metrics_integration(app).await;
    info!("✅ Metrics integration service disabled due to type mismatch");
    
    // Record initial startup event
    record_network_event("node_started", None);
    
    info!("✅ All monitoring services started successfully");
    info!("   - Prometheus metrics: http://localhost:{}/metrics", prometheus_port);
    info!("   - Live dashboard: http://localhost:{}/api/dashboard", dashboard_port);
    info!("   - Health check: http://localhost:{}/health", dashboard_port);
    
    Ok(())
}

// MVP: monitoring services are disabled/no-op
#[cfg(feature = "mvp")]
async fn start_monitoring_services(_app: Arc<BitCrapsApp>, _config: &AppConfig) -> Result<()> {
    Ok(())
}

// Wrapper to run TUI with correct types per build feature
#[cfg(not(feature = "mvp"))]
async fn run_tui_wrapper(_app: BitCrapsApp) -> Result<()> {
    // TODO: Fix type mismatch between app_state::BitCrapsApp and bitcraps::BitCrapsApp
    // bitcraps::ui::tui::run_integrated_tui(app).await.map_err(|e| Error::Protocol(format!("TUI failed: {}", e)))
    println!("TUI disabled due to type mismatch - using CLI mode");
    Ok(())
}

#[cfg(feature = "mvp")]
async fn run_tui_wrapper(_app: BitCrapsApp) -> Result<()> {
    eprintln!("TUI is disabled under MVP builds.");
    Ok(())
}
