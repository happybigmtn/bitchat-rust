use log::info;

use bitcraps::{AppConfig, Error, Result};

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
        eprintln!("Location: {}", panic_info.location().map_or("unknown".to_string(), |l| l.to_string()));
        eprintln!("Message: {}", panic_info.payload().downcast_ref::<&str>().unwrap_or(&"Unknown panic"));
        eprintln!("Attempting graceful shutdown...");
        
        // Log to file if possible
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("bitcraps_panic.log") 
        {
            use std::io::Write;
            let _ = writeln!(file, "[{}] PANIC: {} at {}", 
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                panic_info.payload().downcast_ref::<&str>().unwrap_or(&"Unknown panic"),
                panic_info.location().map_or("unknown".to_string(), |l| l.to_string())
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
        ..AppConfig::default()
    };

    match cli.command {
        Commands::Start => {
            info!("Starting BitCraps node...");
            let mut app = BitCrapsApp::new(config).await?;
            app.start().await?;
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
