use std::time::Duration;
use tokio::time::sleep;
use log::{info, warn};

use bitcraps::{
    AppConfig, Result, Error, CrapTokens,
};

// Import new modules
mod app_config;
mod app_state;
mod commands;

use app_config::{Cli, Commands, parse_bet_type, parse_game_id, format_game_id, resolve_data_dir};
use app_state::{BitCrapsApp, AppStats};
use commands::{CommandExecutor, commands as cmd};

#[tokio::main]
async fn main() -> Result<()> {
    use clap::Parser;
    
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
    let data_dir = resolve_data_dir(&cli.data_dir)
        .map_err(|e| Error::Protocol(e))?;
    
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
        
        Commands::Bet { game_id, bet_type, amount } => {
            cmd::place_bet_command(&BitCrapsApp::new(config.clone()).await?, &game_id, &bet_type, amount).await?;
            
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