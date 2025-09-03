//! TUI (Terminal User Interface) module for BitCraps
//!
//! This module implements terminal-based user interfaces for BitCraps
//! using ratatui, providing rich interactive casino experiences.

pub mod casino;
pub mod chat;
pub mod events;
pub mod input;
pub mod widgets;

pub use casino::*;
pub use chat::*;
pub use events::*;
pub use input::*;
pub use widgets::*;

use crate::protocol::DiceRoll;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Main TUI application state for BitCraps casino
pub struct TuiApp {
    pub casino_ui: CasinoUI,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub peers: Vec<PeerInfo>,
    pub current_view: ViewMode,
    pub network_status: NetworkStatus,
    pub mining_stats: MiningStats,
    pub animation_state: AnimationState,
    pub last_update: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Casino,
    Chat,
    PeerList,
    Settings,
    GameLobby,
    ActiveGame,
}

#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub connected_peers: usize,
    pub total_games: usize,
    pub network_hash_rate: f64,
    pub connection_quality: ConnectionQuality,
}

#[derive(Debug, Clone)]
pub enum ConnectionQuality {
    Excellent,
    Good,
    Fair,
    Poor,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct MiningStats {
    pub tokens_mined: u64,
    pub mining_rate: f64,
    pub last_reward: Option<u64>,
    pub blocks_found: u64,
}

#[derive(Debug, Clone)]
pub struct AnimationState {
    pub dice_rolling: bool,
    pub dice_animation_frame: usize,
    pub last_dice_result: Option<DiceRoll>,
    pub animation_start: Option<Instant>,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            casino_ui: CasinoUI::new(),
            messages: Vec::new(),
            input: String::new(),
            peers: Vec::new(),
            current_view: ViewMode::Casino,
            network_status: NetworkStatus {
                connected_peers: 0,
                total_games: 0,
                network_hash_rate: 0.0,
                connection_quality: ConnectionQuality::Disconnected,
            },
            mining_stats: MiningStats {
                tokens_mined: 0,
                mining_rate: 1.5, // 1.5 CRAP per second
                last_reward: None,
                blocks_found: 0,
            },
            animation_state: AnimationState {
                dice_rolling: false,
                dice_animation_frame: 0,
                last_dice_result: None,
                animation_start: None,
            },
            last_update: Instant::now(),
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_update);

        // Update dice animation
        if self.animation_state.dice_rolling {
            if let Some(start) = self.animation_state.animation_start {
                let elapsed = now.duration_since(start);
                if elapsed > Duration::from_millis(2000) {
                    // Animation complete
                    self.animation_state.dice_rolling = false;
                    self.animation_state.animation_start = None;
                } else {
                    // Update animation frame
                    self.animation_state.dice_animation_frame =
                        (elapsed.as_millis() / 100) as usize % 6 + 1;
                }
            }
        }

        // Update mining stats (simulate)
        self.mining_stats.tokens_mined +=
            (delta.as_secs_f64() * self.mining_stats.mining_rate) as u64;

        // Simulate network activity
        self.network_status.connected_peers = 12 + (now.elapsed().as_secs() % 8) as usize;
        self.network_status.total_games = 3 + (now.elapsed().as_secs() % 5) as usize;

        self.last_update = now;
    }

    pub fn start_dice_animation(&mut self, result: DiceRoll) {
        self.animation_state.dice_rolling = true;
        self.animation_state.animation_start = Some(Instant::now());
        self.animation_state.last_dice_result = Some(result);
        self.animation_state.dice_animation_frame = 1;
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => return false, // Quit
            KeyCode::Tab => self.cycle_view(),
            KeyCode::Char('c') => self.current_view = ViewMode::Casino,
            KeyCode::Char('t') => self.current_view = ViewMode::Chat,
            KeyCode::Char('p') => self.current_view = ViewMode::PeerList,
            KeyCode::Char('s') => self.current_view = ViewMode::Settings,
            KeyCode::Char('l') => self.current_view = ViewMode::GameLobby,
            KeyCode::Char('g') => self.current_view = ViewMode::ActiveGame,
            KeyCode::Enter => self.handle_enter(),
            KeyCode::Esc => self.handle_escape(),
            KeyCode::Up => self.handle_up(),
            KeyCode::Down => self.handle_down(),
            KeyCode::Left => self.handle_left(),
            KeyCode::Right => self.handle_right(),
            KeyCode::Char(c) => self.handle_char_input(c),
            KeyCode::Backspace => self.handle_backspace(),
            _ => {}
        }
        true
    }

    fn cycle_view(&mut self) {
        self.current_view = match self.current_view {
            ViewMode::Casino => ViewMode::Chat,
            ViewMode::Chat => ViewMode::PeerList,
            ViewMode::PeerList => ViewMode::Settings,
            ViewMode::Settings => ViewMode::GameLobby,
            ViewMode::GameLobby => ViewMode::ActiveGame,
            ViewMode::ActiveGame => ViewMode::Casino,
        };
    }

    fn handle_enter(&mut self) {
        // Handle enter based on current view
        match self.current_view {
            ViewMode::Casino | ViewMode::ActiveGame => {
                // Place bet or perform casino action
                // This would integrate with the casino UI
            }
            ViewMode::Chat => {
                // Send chat message
                if !self.input.trim().is_empty() {
                    // Send message logic here
                    self.input.clear();
                }
            }
            _ => {}
        }
    }

    fn handle_escape(&mut self) {
        match self.current_view {
            ViewMode::ActiveGame => self.current_view = ViewMode::GameLobby,
            ViewMode::GameLobby => self.current_view = ViewMode::Casino,
            _ => self.current_view = ViewMode::Casino,
        }
    }

    fn handle_up(&mut self) {
        match self.current_view {
            ViewMode::Casino | ViewMode::ActiveGame => {
                // Navigate betting options
            }
            _ => {}
        }
    }

    fn handle_down(&mut self) {
        match self.current_view {
            ViewMode::Casino | ViewMode::ActiveGame => {
                // Navigate betting options
            }
            _ => {}
        }
    }

    fn handle_left(&mut self) {
        match self.current_view {
            ViewMode::Casino | ViewMode::ActiveGame => {
                // Navigate betting options
            }
            _ => {}
        }
    }

    fn handle_right(&mut self) {
        match self.current_view {
            ViewMode::Casino | ViewMode::ActiveGame => {
                // Navigate betting options
            }
            _ => {}
        }
    }

    fn handle_char_input(&mut self, c: char) {
        match self.current_view {
            ViewMode::Chat => {
                self.input.push(c);
            }
            ViewMode::Casino | ViewMode::ActiveGame => {
                match c {
                    'r' => {
                        // Roll dice using cryptographically secure RNG
                        use rand::{rngs::OsRng, Rng};
                        let mut rng = OsRng;
                        if let Ok(roll) = DiceRoll::new(rng.gen_range(1..=6), rng.gen_range(1..=6))
                        {
                            self.start_dice_animation(roll);
                        }
                    }
                    'b' => {
                        // Place bet
                        // This would integrate with the casino UI
                    }
                    '+' => {
                        // Increase bet amount
                        self.casino_ui.bet_amount = (self.casino_ui.bet_amount + 10).min(1000);
                    }
                    '-' => {
                        // Decrease bet amount
                        self.casino_ui.bet_amount =
                            self.casino_ui.bet_amount.saturating_sub(10).max(10);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_backspace(&mut self) {
        if self.current_view == ViewMode::Chat {
            self.input.pop();
        }
    }
}

/// Integrated TUI application that connects to real BitCrapsApp
pub struct IntegratedTuiApp {
    pub app_ref: Arc<crate::BitCrapsApp>,
    pub tui_state: TuiApp,
    pub last_refresh: Instant,
}

impl IntegratedTuiApp {
    pub fn new(app_ref: Arc<crate::BitCrapsApp>) -> Self {
        Self {
            app_ref,
            tui_state: TuiApp::new(),
            last_refresh: Instant::now(),
        }
    }

    /// Update TUI state with real application data
    pub async fn update(&mut self) {
        // Update base TUI animations
        self.tui_state.update();

        // Refresh data every second
        if self.last_refresh.elapsed() > Duration::from_secs(1) {
            // Update wallet balance with real data
            if let Ok(balance_tokens) = self.app_ref.get_balance().await {
                self.tui_state.casino_ui.wallet_balance = balance_tokens.amount();
            }

            // Get active games count
            if let Ok(games) = self.app_ref.get_active_games().await {
                self.tui_state.network_status.total_games = games.len();
                
                // Simulate connection quality based on games available
                self.tui_state.network_status.connection_quality = match games.len() {
                    0 => ConnectionQuality::Disconnected,
                    1..=2 => ConnectionQuality::Poor,
                    3..=5 => ConnectionQuality::Fair,
                    6..=10 => ConnectionQuality::Good,
                    _ => ConnectionQuality::Excellent,
                };
                
                // Estimate connected peers based on games (rough approximation)
                self.tui_state.network_status.connected_peers = games.len() * 2;
            }

            self.last_refresh = Instant::now();
        }
    }

    /// Handle key events with real app integration
    pub async fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') => return false, // Quit
            KeyCode::Tab => self.tui_state.cycle_view(),
            KeyCode::Char('c') => self.tui_state.current_view = ViewMode::Casino,
            KeyCode::Char('t') => self.tui_state.current_view = ViewMode::Chat,
            KeyCode::Char('p') => self.tui_state.current_view = ViewMode::PeerList,
            KeyCode::Char('s') => self.tui_state.current_view = ViewMode::Settings,
            KeyCode::Char('l') => self.tui_state.current_view = ViewMode::GameLobby,
            KeyCode::Char('g') => self.tui_state.current_view = ViewMode::ActiveGame,
            
            // Real game actions
            KeyCode::Char('r') => {
                // Try to roll dice in an active game
                if let Ok(games) = self.app_ref.get_active_games().await {
                    if let Some(game_id) = games.first() {
                        // Simulate dice roll - in real implementation this would trigger game logic
                        use rand::{rngs::OsRng, Rng};
                        let mut rng = OsRng;
                        if let Ok(roll) = DiceRoll::new(rng.gen_range(1..=6), rng.gen_range(1..=6)) {
                            self.tui_state.start_dice_animation(roll);
                        }
                    }
                }
            },
            
            KeyCode::Char('b') => {
                // Place a bet if in a game
                if let Ok(games) = self.app_ref.get_active_games().await {
                    if let Some(game_id) = games.first() {
                        let amount = self.tui_state.casino_ui.bet_amount;
                        // Convert u64 to CrapTokens for the bet
                        if let Ok(crap_amount) = crate::protocol::CrapTokens::from_crap(amount as f64) {
                            // Try to place a pass line bet
                            if let Err(e) = self.app_ref.place_bet(*game_id, crate::BetType::Pass, crap_amount).await {
                                // Handle error (could show in status)
                                log::warn!("Failed to place bet: {}", e);
                            }
                        }
                    }
                }
            },
            
            KeyCode::Char('+') => {
                self.tui_state.casino_ui.bet_amount = (self.tui_state.casino_ui.bet_amount + 10).min(1000);
            },
            KeyCode::Char('-') => {
                self.tui_state.casino_ui.bet_amount = self.tui_state.casino_ui.bet_amount.saturating_sub(10).max(10);
            },
            
            _ => {
                // Delegate other keys to base TUI
                self.tui_state.handle_key_event(key);
            }
        }
        true
    }
}

/// Render function for integrated TUI
pub fn render_integrated_ui(f: &mut Frame, app: &IntegratedTuiApp) {
    // Use the existing render logic with real data
    render_ui(f, &app.tui_state);
}

/// Main TUI render function
pub fn render_ui(f: &mut Frame, app: &TuiApp) {
    match app.current_view {
        ViewMode::Casino => render_casino_main(f, app),
        ViewMode::Chat => render_chat_view(f, app),
        ViewMode::PeerList => render_peer_list(f, app),
        ViewMode::Settings => render_settings(f, app),
        ViewMode::GameLobby => render_game_lobby(f, app),
        ViewMode::ActiveGame => render_active_game(f, app),
    }

    // Always render status bar at bottom
    render_status_bar(f, app);
}

/// Render the main casino view with craps table
fn render_casino_main(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(6), // Network status and mining
        ])
        .split(f.area());

    render_header(f, chunks[0], app);
    render_craps_table(f, chunks[1], app);
    render_network_mining_status(f, chunks[2], app);
}

/// Render the chat view
fn render_chat_view(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Chat area
            Constraint::Length(3), // Input area
        ])
        .split(f.area());

    render_chat_area_impl(f, chunks[0], app);
    render_input_area_impl(f, chunks[1], app);
}

/// Render the main chat area
fn render_chat_area_impl(f: &mut Frame, area: Rect, app: &TuiApp) {
    let messages: Vec<ListItem> = app
        .messages
        .iter()
        .map(|m| {
            let content_str = match &m.content {
                MessageContent::Text(text) => text.clone(),
                MessageContent::File(filename) => format!("📁 {}", filename),
                MessageContent::System(sys_msg) => format!("🔧 {:?}", sys_msg),
                MessageContent::Encrypted(enc) => format!("🔒 {}", enc),
            };
            ListItem::new(vec![Line::from(vec![
                Span::styled(format!("{:?}", m.sender), Style::default().fg(Color::Cyan)),
                Span::raw(": "),
                Span::raw(content_str),
            ])])
        })
        .collect();

    let messages_widget =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Chat"));

    f.render_widget(messages_widget, area);
}

/// Render the input area where users type
fn render_input_area_impl(f: &mut Frame, area: Rect, app: &TuiApp) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, area);
}

/// Render the status bar
fn render_status_bar(f: &mut Frame, app: &TuiApp) {
    let area = Rect {
        x: 0,
        y: f.area().height.saturating_sub(1),
        width: f.area().width,
        height: 1,
    };

    let status = match app.current_view {
        ViewMode::Casino => "🎲 Casino | Tab: Switch views | r: Roll dice | b: Bet | q: Quit",
        ViewMode::Chat => "💬 Chat | Tab: Switch views | Enter: Send | q: Quit",
        ViewMode::PeerList => "👥 Peers | Tab: Switch views | q: Quit",
        ViewMode::Settings => "⚙️ Settings | Tab: Switch views | q: Quit",
        ViewMode::GameLobby => "🏛️ Lobby | Tab: Switch views | Enter: Join | q: Quit",
        ViewMode::ActiveGame => "🎯 Game | Tab: Switch views | Esc: Lobby | q: Quit",
    };

    let status_widget = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(status_widget, area);
}

/// Render the header with title and key info
fn render_header(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    // Title
    let title = Paragraph::new("🎲 BitCraps Casino 🎲")
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Current view
    let view_text = match app.current_view {
        ViewMode::Casino => "Casino Floor",
        ViewMode::Chat => "Chat Room",
        ViewMode::PeerList => "Peer List",
        ViewMode::Settings => "Settings",
        ViewMode::GameLobby => "Game Lobby",
        ViewMode::ActiveGame => "Active Game",
    };
    let view = Paragraph::new(format!("📍 {}", view_text))
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(view, chunks[1]);

    // Wallet balance
    let balance_color = if app.casino_ui.wallet_balance > 500 {
        Color::Green
    } else {
        Color::Red
    };
    let wallet = Paragraph::new(format!("💰 {} CRAP", app.casino_ui.wallet_balance))
        .style(
            Style::default()
                .fg(balance_color)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(wallet, chunks[2]);
}

/// Render the craps table with dice and betting areas
fn render_craps_table(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Dice area
            Constraint::Min(0),    // Betting table
        ])
        .split(area);

    render_dice_area(f, chunks[0], app);
    render_betting_table(f, chunks[1], app);
}

/// Render the dice display with animation
fn render_dice_area(f: &mut Frame, area: Rect, app: &TuiApp) {
    let dice_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(40),
        ])
        .split(area);

    let (die1, die2) = if app.animation_state.dice_rolling {
        (
            app.animation_state.dice_animation_frame as u8,
            app.animation_state.dice_animation_frame as u8,
        )
    } else if let Some(roll) = app.animation_state.last_dice_result {
        (roll.die1, roll.die2)
    } else {
        (1, 1)
    };

    let dice_faces = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];

    // Die 1
    let die1_text = if (1..=6).contains(&die1) {
        dice_faces[(die1 - 1) as usize]
    } else {
        "⚀"
    };
    let die1_widget = Paragraph::new(Line::from(vec![Span::styled(
        die1_text,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Die 1"));
    f.render_widget(die1_widget, dice_chunks[0]);

    // Total and status
    let total = die1 + die2;
    let (total_color, status_text) = match total {
        7 | 11 => (Color::Green, "NATURAL WIN!"),
        2 | 3 | 12 => (Color::Red, "CRAPS!"),
        4 | 5 | 6 | 8 | 9 | 10 => (Color::Yellow, "POINT"),
        _ => (Color::White, ""),
    };

    let status_lines = vec![
        Line::from(vec![Span::styled(
            format!("Total: {}", total),
            Style::default()
                .fg(total_color)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            status_text,
            Style::default()
                .fg(total_color)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        if app.animation_state.dice_rolling {
            Line::from(vec![Span::styled(
                "🎲 ROLLING... 🎲",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
            )])
        } else {
            Line::from(vec![Span::styled(
                "Press 'r' to roll",
                Style::default().fg(Color::Gray),
            )])
        },
    ];

    let status_widget = Paragraph::new(status_lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status_widget, dice_chunks[1]);

    // Die 2
    let die2_text = if (1..=6).contains(&die2) {
        dice_faces[(die2 - 1) as usize]
    } else {
        "⚀"
    };
    let die2_widget = Paragraph::new(Line::from(vec![Span::styled(
        die2_text,
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Die 2"));
    f.render_widget(die2_widget, dice_chunks[2]);
}

/// Render the betting table
fn render_betting_table(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    // Pass Line
    let pass_line = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "PASS LINE",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Even Money"),
        Line::from(""),
        Line::from("Come-out:"),
        Line::from("7 or 11 wins"),
        Line::from("2, 3, 12 lose"),
        Line::from(""),
        Line::from("Point phase:"),
        Line::from("Point wins"),
        Line::from("7 loses"),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Pass Line"));
    f.render_widget(pass_line, chunks[0]);

    // Don't Pass
    let dont_pass = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "DON'T PASS",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Even Money"),
        Line::from(""),
        Line::from("Come-out:"),
        Line::from("2 or 3 wins"),
        Line::from("7 or 11 lose"),
        Line::from("12 pushes"),
        Line::from(""),
        Line::from("Point phase:"),
        Line::from("7 wins"),
        Line::from("Point loses"),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Don't Pass"));
    f.render_widget(dont_pass, chunks[1]);

    // Field
    let field = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "FIELD",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("One Roll Bet"),
        Line::from(""),
        Line::from("Wins on:"),
        Line::from("2, 3, 4, 9"),
        Line::from("10, 11, 12"),
        Line::from(""),
        Line::from("Pays:"),
        Line::from("2 & 12: 2:1"),
        Line::from("Others: 1:1"),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Field"));
    f.render_widget(field, chunks[2]);

    // Betting controls
    let betting_info = vec![
        Line::from(vec![Span::styled(
            "BETTING",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Amount: "),
            Span::styled(
                format!("{} CRAP", app.casino_ui.bet_amount),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from("Controls:"),
        Line::from("+ : Increase bet"),
        Line::from("- : Decrease bet"),
        Line::from("b : Place bet"),
        Line::from("r : Roll dice"),
        Line::from(""),
        Line::from("Tab: Switch view"),
    ];

    let betting_widget = Paragraph::new(betting_info)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Controls"));
    f.render_widget(betting_widget, chunks[3]);
}

/// Render network status and mining display
fn render_network_mining_status(f: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_network_status(f, chunks[0], app);
    render_mining_status(f, chunks[1], app);
}

/// Render network status
fn render_network_status(f: &mut Frame, area: Rect, app: &TuiApp) {
    let quality_color = match app.network_status.connection_quality {
        ConnectionQuality::Excellent => Color::Green,
        ConnectionQuality::Good => Color::Yellow,
        ConnectionQuality::Fair => Color::LightYellow,
        ConnectionQuality::Poor => Color::Red,
        ConnectionQuality::Disconnected => Color::DarkGray,
    };

    let network_info = vec![
        Line::from(vec![
            Span::raw("Connected Peers: "),
            Span::styled(
                app.network_status.connected_peers.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Active Games: "),
            Span::styled(
                app.network_status.total_games.to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Network Quality: "),
            Span::styled(
                format!("{:?}", app.network_status.connection_quality),
                Style::default().fg(quality_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("Protocol: "),
            Span::styled("BitCraps v1.0", Style::default().fg(Color::Green)),
        ]),
    ];

    let network_widget = Paragraph::new(network_info).block(
        Block::default()
            .borders(Borders::ALL)
            .title("🌐 Network Status"),
    );
    f.render_widget(network_widget, area);
}

/// Render mining status with real-time updates
fn render_mining_status(f: &mut Frame, area: Rect, app: &TuiApp) {
    let mining_info = vec![
        Line::from(vec![
            Span::raw("Tokens Mined: "),
            Span::styled(
                app.mining_stats.tokens_mined.to_string(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" CRAP"),
        ]),
        Line::from(vec![
            Span::raw("Mining Rate: "),
            Span::styled(
                format!("{:.2}", app.mining_stats.mining_rate),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" CRAP/s"),
        ]),
        Line::from(vec![
            Span::raw("Blocks Found: "),
            Span::styled(
                app.mining_stats.blocks_found.to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("Hash Rate: "),
            Span::styled(
                format!("{:.1} kH/s", app.network_status.network_hash_rate),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    let mining_widget = Paragraph::new(mining_info).block(
        Block::default()
            .borders(Borders::ALL)
            .title("⛏️ Mining Stats"),
    );
    f.render_widget(mining_widget, area);
}

/// Render peer list view
fn render_peer_list(f: &mut Frame, app: &TuiApp) {
    let peer_items: Vec<ListItem> = app
        .peers
        .iter()
        .map(|peer| {
            ListItem::new(vec![Line::from(vec![
                Span::styled(&peer.id, Style::default().fg(Color::Cyan)),
                Span::raw(" - "),
                Span::styled(&peer.address, Style::default().fg(Color::Yellow)),
            ])])
        })
        .collect();

    let peer_list = List::new(peer_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Connected Peers"),
    );

    f.render_widget(peer_list, f.area());
}

/// Render settings view
fn render_settings(f: &mut Frame, _app: &TuiApp) {
    let settings_text = vec![
        Line::from("⚙️ BitCraps Settings"),
        Line::from(""),
        Line::from("🎮 Game Settings:"),
        Line::from("  • Auto-bet: Disabled"),
        Line::from("  • Sound: Enabled"),
        Line::from("  • Animations: Enabled"),
        Line::from(""),
        Line::from("🌐 Network Settings:"),
        Line::from("  • Max peers: 50"),
        Line::from("  • Discovery: Bluetooth + DHT"),
        Line::from("  • Encryption: AES-256"),
        Line::from(""),
        Line::from("⛏️ Mining Settings:"),
        Line::from("  • Mining enabled: Yes"),
        Line::from("  • CPU threads: 4"),
        Line::from("  • Target rate: 1.0 CRAP/min"),
    ];

    let settings_widget = Paragraph::new(settings_text)
        .block(Block::default().borders(Borders::ALL).title("Settings"));

    f.render_widget(settings_widget, f.area());
}

/// Render game lobby
fn render_game_lobby(f: &mut Frame, app: &TuiApp) {
    app.casino_ui.render_game_lobby(f, f.area());
}

/// Render active game
fn render_active_game(f: &mut Frame, app: &TuiApp) {
    app.casino_ui.render_active_game(f, f.area());
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub id: String,
    pub address: String,
    pub last_seen: u64,
}

#[derive(Debug, Clone)]
pub enum ChatCommand {
    // Connection commands
    Connect(String),    // /connect <address>
    Disconnect(String), // /disconnect <peer>

    // Messaging commands
    Msg(String, String), // /msg <peer> <message>
    Broadcast(String),   // /broadcast <message>

    // Channel commands
    Join(String),  // /join <channel>
    Leave(String), // /leave <channel>

    // Utility commands
    Nick(String), // /nick <nickname>
    Peers,        // /peers
    Help,         // /help
    Quit,         // /quit
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
            ChatCommand::Connect(addr) => self.network.connect_peer(addr).await,
            ChatCommand::Msg(peer, message) => self.network.send_message(peer, message).await,
            ChatCommand::Peers => Ok(self.network.list_peers().await),
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

/// Main TUI application runner with integrated BitCrapsApp
pub async fn run_integrated_tui(bitcraps_app: crate::BitCrapsApp) -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Wrap app in Arc for sharing
    let app_ref = Arc::new(bitcraps_app);
    
    // Create TUI app with real data integration
    let mut tui_app = IntegratedTuiApp::new(app_ref.clone());

    // Main loop
    loop {
        // Update app state with real data
        tui_app.update().await;

        // Render
        terminal.draw(|f| render_integrated_ui(f, &tui_app))?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if !tui_app.handle_key_event(key).await {
                    break;
                }
            }
        }
    }

    // Cleanup
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Main TUI application runner
pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = TuiApp::new();

    // Main loop
    loop {
        // Update app state
        app.update();

        // Render
        terminal.draw(|f| render_ui(f, &app))?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if !app.handle_key_event(key) {
                    break;
                }
            }
        }
    }

    // Cleanup
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    Ok(())
}

// Re-export for external use
// Remove duplicate exports since all items are already defined in this module
