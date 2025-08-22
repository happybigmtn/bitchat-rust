//! TUI (Terminal User Interface) module for BitCraps
//! 
//! This module implements terminal-based user interfaces for BitCraps
//! using ratatui, providing rich interactive casino experiences.

pub mod widgets;
pub mod chat;
pub mod events;
pub mod input;
pub mod casino;

pub use widgets::*;
pub use chat::*; 
pub use events::*;
pub use input::*;
pub use casino::*;

// Import types we need (already available via pub use widgets::*)

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal, Frame,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::io;

#[allow(dead_code)]
pub struct TuiApp {
    messages: Vec<ChatMessage>,
    input: String,
    peers: Vec<PeerInfo>,
    current_view: ViewMode,
}

#[allow(dead_code)]
enum ViewMode {
    Chat,
    PeerList,
    Settings,
}

/// Main TUI render function
/// 
/// Feynman: This is like the casino's main display board - it shows everything
/// happening at once. The terminal screen is divided into sections (Layout) like
/// different areas of a casino floor: the game tables (messages), the player list
/// (peers), and the betting window (input). Every frame, we redraw the entire
/// casino floor with the latest state.
// Main TUI render function
#[allow(dead_code)]
fn render_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &TuiApp) {
    let _ = terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Chat area
                Constraint::Length(3),  // Input area
                Constraint::Length(1),  // Status bar
            ])
            .split(f.area());
            
        render_chat_area_impl(f, chunks[0], app);
        render_input_area_impl(f, chunks[1], app);
        render_status_bar_impl(f, chunks[2], app);
    });
}

#[derive(Debug, Clone)]
pub enum ChatCommand {
    // Connection commands
    Connect(String),        // /connect <address>
    Disconnect(String),     // /disconnect <peer>
    
    // Messaging commands
    Msg(String, String),    // /msg <peer> <message>
    Broadcast(String),      // /broadcast <message>
    
    // Channel commands
    Join(String),           // /join <channel>
    Leave(String),          // /leave <channel>
    
    // Utility commands
    Nick(String),           // /nick <nickname>
    Peers,                  // /peers
    Help,                   // /help
    Quit,                   // /quit
}

#[allow(dead_code)]
pub struct CommandProcessor {
    network: Arc<NetworkManager>,
    config: Arc<RwLock<events::Config>>,
}

impl CommandProcessor {
    pub fn parse_command(input: &str) -> Result<ChatCommand, CommandError> {
        if !input.starts_with('/') {
            return Err(CommandError::NotACommand);
        }
        
        let parts: Vec<&str> = input[1..].split_whitespace().collect();
        match parts.first() {
            Some(&"connect") => {
                let addr = parts.get(1).ok_or(CommandError::MissingArgument)?;
                Ok(ChatCommand::Connect(addr.to_string()))
            }
            Some(&"msg") => {
                let peer = parts.get(1).ok_or(CommandError::MissingArgument)?;
                let message = parts[2..].join(" ");
                Ok(ChatCommand::Msg(peer.to_string(), message))
            }
            Some(&"peers") => Ok(ChatCommand::Peers),
            _ => Err(CommandError::UnknownCommand),
        }
    }
    
    pub async fn execute_command(&self, command: ChatCommand) -> CommandResult {
        match command {
            ChatCommand::Connect(addr) => {
                self.network.connect_peer(addr).await
            }
            ChatCommand::Msg(peer, message) => {
                self.network.send_message(peer, message).await
            }
            ChatCommand::Peers => {
                Ok(self.network.list_peers().await)
            }
            // ... other commands
            _ => Ok("Command not implemented".to_string()),
        }
    }
}

// Add missing types for compilation
#[derive(Debug)]
pub enum CommandError {
    NotACommand,
    MissingArgument,
    UnknownCommand,
}

pub type CommandResult = Result<String, CommandError>;

// Add missing NetworkManager stub
pub struct NetworkManager;
impl NetworkManager {
    pub async fn connect_peer(&self, _addr: String) -> CommandResult {
        Ok("Connected".to_string())
    }
    pub async fn send_message(&self, _peer: String, _message: String) -> CommandResult {
        Ok("Message sent".to_string())
    }
    pub async fn list_peers(&self) -> String {
        "No peers".to_string()
    }
}

/// Render the main chat area
/// 
/// Feynman: The chat area is like the main casino floor where all the action happens.
/// Messages scroll by like dealers calling out bets and results. Each message shows
/// who said what and when, just like announcements over the casino PA system.
#[allow(dead_code)]
fn render_chat_area_impl(f: &mut Frame, area: Rect, app: &TuiApp) {
    let messages: Vec<ListItem> = app.messages
        .iter()
        .map(|m| {
            let content_str = match &m.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::File(filename) => format!("📁 {}", filename),
                MessageContent::System(sys_msg) => format!("🔧 {}", format!("{:?}", sys_msg)),
                MessageContent::Encrypted(enc) => format!("🔒 {}", enc),
            };
            ListItem::new(vec![
                ratatui::text::Line::from(vec![
                    ratatui::text::Span::styled(format!("{:?}", m.sender), ratatui::style::Style::default().fg(ratatui::style::Color::Cyan)),
                    ratatui::text::Span::raw(": "),
                    ratatui::text::Span::raw(content_str.clone()),
                ])
            ])
        })
        .collect();

    let messages_widget = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Chat"));
    
    f.render_widget(messages_widget, area);
}

/// Render the input area where users type
/// 
/// Feynman: The input area is like the betting window at a casino. This is where
/// you tell the house what you want to do - place a bet, cash out, or just chat
/// with other players. The cursor blinks here waiting for your next move.
#[allow(dead_code)]
fn render_input_area_impl(f: &mut Frame, area: Rect, app: &TuiApp) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, area);
}

/// Render the status bar
/// 
/// Feynman: The status bar is like the casino's information ticker - it shows
/// vital stats like how many people are connected (active players), current
/// game status, and your connection state. Think of it as the "house announcements".
#[allow(dead_code)]
fn render_status_bar_impl(f: &mut Frame, area: Rect, app: &TuiApp) {
    let status = match app.current_view {
        ViewMode::Chat => format!("Chat | {} peers connected", app.peers.len()),
        ViewMode::PeerList => "Peer List | Use arrow keys to navigate".to_string(),
        ViewMode::Settings => "Settings | Press q to return".to_string(),
    };
    
    let status_widget = Paragraph::new(status)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(status_widget, area);
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: String,
    pub address: String,
    pub last_seen: u64,
}

