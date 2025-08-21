//! BitChat CLI application
//! 
//! Feynman: This is like the ignition key for your car.
//! It starts everything up and lets you control the whole system.

use bitchat::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Set the verbosity level
    #[arg(short, long, default_value = "info")]
    verbosity: String,
    
    /// Path to the data directory
    #[arg(short, long, default_value = "~/.bitchat")]
    data_dir: String,
    
    /// Port to listen on
    #[arg(short, long, default_value = "8338")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Initialize logging
    use tracing_subscriber::{fmt, EnvFilter};
    fmt()
        .with_env_filter(EnvFilter::new(&args.verbosity))
        .init();
    
    tracing::info!("Starting BitChat...");
    
    // TODO: Initialize the application
    println!("BitChat is starting on port {}...", args.port);
    println!("Data directory: {}", args.data_dir);
    
    Ok(())
}
