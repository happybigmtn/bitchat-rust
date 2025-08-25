//! Simplified UI implementation for BitCraps
//! 
//! This provides a minimal working UI for compilation while maintaining
//! the core CLI functionality.

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

/// Simple terminal UI state
pub struct SimpleUI {
    pub current_view: ViewMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Chat,
    Casino,
    Settings,
}

impl Default for SimpleUI {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleUI {
    pub fn new() -> Self {
        Self {
            current_view: ViewMode::Chat,
        }
    }
    
    pub fn switch_view(&mut self, view: ViewMode) {
        self.current_view = view;
    }
}