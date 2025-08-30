//! Cli module for BitCraps UI
//!
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bitchat")]
#[command(about = "Decentralized P2P chat application")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    #[arg(short, long)]
    pub config: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start interactive chat mode
    Chat,
    /// Connect to a peer
    Connect { address: String },
    /// List connected peers
    Peers,
    /// Send a message
    Send { peer: String, message: String },
    /// Start BitCraps casino mode
    Casino,
    /// Create a new BitCraps game session
    CreateGame { max_players: Option<usize> },
    /// Join an existing game session
    JoinGame { game_id: String },
    /// Place a bet in active game
    Bet { bet_type: String, amount: u64 },
}
