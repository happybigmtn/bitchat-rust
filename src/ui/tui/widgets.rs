//! Widgets module for BitCraps UI
//! 
//! This module implements specialized UI components for BitCraps casino
//! including dice displays, betting tables, and game state widgets.

use serde::{Serialize, Deserialize};
use std::time::SystemTime;
use crate::PeerId;
use crate::protocol::{DiceRoll, BetType, CrapTokens};
use crate::protocol::craps::GamePhase;
use ratatui::text::{Span, Line};
use ratatui::style::{Style, Color, Modifier};
use ratatui::widgets::{List, ListItem, Block, Borders, StatefulWidget, ListState, Paragraph, Gauge};
use ratatui::layout::{Rect, Layout, Direction, Constraint, Alignment};
use ratatui::buffer::Buffer;
use ratatui::Frame;

// Add missing types
/// Auto-completion helper for commands and inputs
#[derive(Debug, Clone)]
pub struct AutoComplete {
    commands: Vec<String>,
}

impl AutoComplete {
    pub fn new() -> Self {
        Self {
            commands: vec![
                "/connect".to_string(),
                "/disconnect".to_string(),
                "/msg".to_string(),
                "/broadcast".to_string(),
                "/join".to_string(),
                "/leave".to_string(),
                "/nick".to_string(),
                "/peers".to_string(),
                "/help".to_string(),
                "/quit".to_string(),
                "/bet".to_string(),
                "/roll".to_string(),
                "/balance".to_string(),
            ],
        }
    }
    
    pub fn complete(&self, input: &str) -> Vec<String> {
        if input.starts_with('/') {
            self.commands
                .iter()
                .filter(|cmd| cmd.starts_with(input))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageFilter {
    All,
    From(String),
    Channel(String),
    System,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: PeerId,
    pub content: MessageContent,
    pub timestamp: SystemTime,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    File(String), // Simplified file transfer as filename
    System(SystemMessage),
    Encrypted(String), // Simplified encrypted content as string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemMessage {
    PeerJoined(String),
    PeerLeft(String),
    ChannelCreated(String),
    Error(String),
}

pub struct MessageFormatter;

impl MessageFormatter {
    pub fn format_message(msg: &ChatMessage) -> Vec<Span<'_>> {
        let timestamp = format!("{:?}", msg.timestamp);
        let sender = format!("{:?}", msg.sender);
        
        match &msg.content {
            MessageContent::Text(text) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::raw(text.clone()),
                ]
            }
            MessageContent::File(filename) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(format!("📁 {}", filename), Style::default().fg(Color::Green)),
                ]
            }
            MessageContent::System(sys_msg) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(format!("* {:?}", sys_msg), Style::default().fg(Color::Yellow)),
                ]
            }
            MessageContent::Encrypted(content) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(sender, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(format!("🔒 {}", content), Style::default().fg(Color::Magenta)),
                ]
            }
        }
    }
    
    pub fn format_for_export(messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|msg| {
                format!(
                    "[{}] {}: {:?}",
                    format!("{:?}", msg.timestamp),
                    format!("{:?}", msg.sender),
                    msg.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}


/// Specialized widget for displaying dice with visual faces
pub struct DiceWidget {
    pub die1: u8,
    pub die2: u8,
    pub is_rolling: bool,
    pub show_total: bool,
}

impl DiceWidget {
    pub fn new(die1: u8, die2: u8) -> Self {
        Self {
            die1,
            die2,
            is_rolling: false,
            show_total: true,
        }
    }
    
    pub fn from_roll(roll: DiceRoll) -> Self {
        Self {
            die1: roll.die1,
            die2: roll.die2,
            is_rolling: false,
            show_total: true,
        }
    }
    
    pub fn rolling() -> Self {
        Self {
            die1: 1,
            die2: 1,
            is_rolling: true,
            show_total: false,
        }
    }
    
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Percentage(35),
            ])
            .split(area);
        
        let dice_faces = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];
        
        // Die 1
        let die1_face = if self.is_rolling {
            "🎲"
        } else if self.die1 >= 1 && self.die1 <= 6 {
            dice_faces[(self.die1 - 1) as usize]
        } else {
            "?"
        };
        
        let die1_widget = Paragraph::new(Line::from(vec![
            Span::styled(die1_face, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Die 1"));
        
        f.render_widget(die1_widget, chunks[0]);
        
        // Center display (total or status)
        let center_content = if self.is_rolling {
            vec![
                Line::from(vec![
                    Span::styled("🎲 ROLLING 🎲", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK))
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Good luck!", Style::default().fg(Color::Green))
                ])
            ]
        } else if self.show_total {
            let total = self.die1 + self.die2;
            let (color, status) = match total {
                7 | 11 => (Color::Green, "🎉 NATURAL!"),
                2 | 3 | 12 => (Color::Red, "💥 CRAPS!"),
                _ => (Color::Yellow, "🎯 POINT"),
            };
            
            vec![
                Line::from(vec![
                    Span::styled(format!("Total: {}", total), Style::default().fg(color).add_modifier(Modifier::BOLD))
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(status, Style::default().fg(color).add_modifier(Modifier::BOLD))
                ])
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::styled("Ready", Style::default().fg(Color::Gray))
                ])
            ]
        };
        
        let center_widget = Paragraph::new(center_content)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Result"));
        
        f.render_widget(center_widget, chunks[1]);
        
        // Die 2
        let die2_face = if self.is_rolling {
            "🎲"
        } else if self.die2 >= 1 && self.die2 <= 6 {
            dice_faces[(self.die2 - 1) as usize]
        } else {
            "?"
        };
        
        let die2_widget = Paragraph::new(Line::from(vec![
            Span::styled(die2_face, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        ]))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Die 2"));
        
        f.render_widget(die2_widget, chunks[2]);
    }
}

/// Widget for displaying the craps betting table layout
pub struct BettingTableWidget {
    pub selected_bet: Option<BetType>,
    pub bet_amount: u64,
    pub wallet_balance: u64,
    pub game_phase: GamePhase,
}

impl BettingTableWidget {
    pub fn new(game_phase: GamePhase) -> Self {
        Self {
            selected_bet: None,
            bet_amount: 10,
            wallet_balance: 1000,
            game_phase,
        }
    }
    
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(70),  // Main betting area
                Constraint::Percentage(30),  // Controls
            ])
            .split(area);
        
        self.render_betting_grid(f, chunks[0]);
        self.render_controls(f, chunks[1]);
    }
    
    fn render_betting_grid(&self, f: &mut Frame, area: Rect) {
        let grid = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),  // Main line bets
                Constraint::Percentage(50),  // Proposition bets
            ])
            .split(area);
        
        // Main line bets
        let main_bets = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(grid[0]);
        
        self.render_bet_area(f, main_bets[0], BetType::Pass, "PASS LINE", "1:1", Color::Green);
        self.render_bet_area(f, main_bets[1], BetType::DontPass, "DON'T PASS", "1:1", Color::Red);
        self.render_bet_area(f, main_bets[2], BetType::Come, "COME", "1:1", Color::Blue);
        self.render_bet_area(f, main_bets[3], BetType::Field, "FIELD", "1:1/2:1", Color::Yellow);
        
        // Proposition bets
        let prop_bets = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
                Constraint::Percentage(20),
            ])
            .split(grid[1]);
        
        self.render_bet_area(f, prop_bets[0], BetType::Hard4, "HARD 4", "7:1", Color::Magenta);
        self.render_bet_area(f, prop_bets[1], BetType::Hard6, "HARD 6", "9:1", Color::Magenta);
        self.render_bet_area(f, prop_bets[2], BetType::Hard8, "HARD 8", "9:1", Color::Magenta);
        self.render_bet_area(f, prop_bets[3], BetType::Next7, "ANY 7", "4:1", Color::LightRed);
        self.render_bet_area(f, prop_bets[4], BetType::Next11, "ANY 11", "15:1", Color::LightGreen);
    }
    
    fn render_bet_area(&self, f: &mut Frame, area: Rect, bet_type: BetType, name: &str, odds: &str, base_color: Color) {
        let is_selected = self.selected_bet == Some(bet_type);
        let is_available = self.is_bet_available(bet_type);
        
        let (border_color, text_style) = if is_selected {
            (Color::White, Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
        } else if !is_available {
            (Color::DarkGray, Style::default().fg(Color::DarkGray))
        } else {
            (base_color, Style::default().fg(base_color))
        };
        
        let availability_text = if !is_available {
            " (N/A)"
        } else {
            ""
        };
        
        let content = vec![
            Line::from(vec![
                Span::styled(name, text_style.add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(odds, text_style)
            ]),
            Line::from(vec![
                Span::styled(availability_text, Style::default().fg(Color::Gray))
            ]),
        ];
        
        let widget = Paragraph::new(content)
            .alignment(Alignment::Center)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)));
        
        f.render_widget(widget, area);
    }
    
    fn render_controls(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(area);
        
        // Bet amount controls
        let amount_info = vec![
            Line::from(vec![
                Span::styled("💰 BET AMOUNT", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Current: "),
                Span::styled(
                    format!("{} CRAP", self.bet_amount),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                )
            ]),
            Line::from(vec![
                Span::raw("Wallet: "),
                Span::styled(
                    format!("{} CRAP", self.wallet_balance),
                    Style::default().fg(Color::Green)
                )
            ]),
        ];
        
        let amount_widget = Paragraph::new(amount_info)
            .block(Block::default().borders(Borders::ALL).title("Amount"));
        
        // Control instructions
        let control_info = vec![
            Line::from(vec![
                Span::styled("🎮 CONTROLS", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
            ]),
            Line::from(""),
            Line::from("↑↓ Select bet type"),
            Line::from("←→ Adjust amount"),
            Line::from("Enter: Place bet"),
            Line::from("Esc: Cancel"),
        ];
        
        let control_widget = Paragraph::new(control_info)
            .block(Block::default().borders(Borders::ALL).title("Controls"));
        
        f.render_widget(amount_widget, chunks[0]);
        f.render_widget(control_widget, chunks[1]);
    }
    
    fn is_bet_available(&self, bet_type: BetType) -> bool {
        match (&self.game_phase, bet_type) {
            // Pass/Don't Pass only available on come-out
            (GamePhase::Point, BetType::Pass) => false,
            (GamePhase::Point, BetType::DontPass) => false,
            // Come/Don't Come only available after point is established
            (GamePhase::ComeOut, BetType::Come) => false,
            (GamePhase::ComeOut, BetType::DontCome) => false,
            // Most other bets are always available
            _ => true,
        }
    }
}

/// Widget for displaying player information and game status
pub struct PlayerStatsWidget {
    pub player_id: String,
    pub balance: u64,
    pub total_wagered: u64,
    pub total_won: u64,
    pub current_bets: Vec<(BetType, u64)>,
    pub is_shooter: bool,
}

impl PlayerStatsWidget {
    pub fn new(player_id: String) -> Self {
        Self {
            player_id,
            balance: 1000,
            total_wagered: 0,
            total_won: 0,
            current_bets: Vec::new(),
            is_shooter: false,
        }
    }
    
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),   // Player info
                Constraint::Min(0),      // Current bets
            ])
            .split(area);
        
        self.render_player_info(f, chunks[0]);
        self.render_current_bets(f, chunks[1]);
    }
    
    fn render_player_info(&self, f: &mut Frame, area: Rect) {
        let role_icon = if self.is_shooter { "🎲" } else { "🎯" };
        let role_text = if self.is_shooter { "Shooter" } else { "Player" };
        
        let net_result = self.total_won as i64 - self.total_wagered as i64;
        let (net_color, net_sign) = if net_result >= 0 {
            (Color::Green, "+")
        } else {
            (Color::Red, "")
        };
        
        let player_info = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} {} {}", role_icon, role_text, self.player_id),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                )
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Balance: "),
                Span::styled(
                    format!("{} CRAP", self.balance),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                )
            ]),
            Line::from(vec![
                Span::raw("Net P&L: "),
                Span::styled(
                    format!("{}{} CRAP", net_sign, net_result),
                    Style::default().fg(net_color).add_modifier(Modifier::BOLD)
                )
            ]),
        ];
        
        let widget = Paragraph::new(player_info)
            .block(Block::default().borders(Borders::ALL).title("Player"));
        
        f.render_widget(widget, area);
    }
    
    fn render_current_bets(&self, f: &mut Frame, area: Rect) {
        if self.current_bets.is_empty() {
            let no_bets = Paragraph::new("No active bets\n\nPlace a bet to start playing!")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("Active Bets"));
            
            f.render_widget(no_bets, area);
        } else {
            let bet_items: Vec<ListItem> = self.current_bets
                .iter()
                .map(|(bet_type, amount)| {
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(format!("{:?}", bet_type), Style::default().fg(Color::Cyan)),
                            Span::raw(" - "),
                            Span::styled(format!("{} CRAP", amount), Style::default().fg(Color::Yellow)),
                        ])
                    ])
                })
                .collect();
            
            let bet_list = List::new(bet_items)
                .block(Block::default().borders(Borders::ALL).title("Active Bets"));
            
            f.render_widget(bet_list, area);
        }
    }
}

/// Progress bar widget for mining/network operations
pub struct ProgressWidget {
    pub current: u64,
    pub max: u64,
    pub label: String,
    pub color: Color,
}

impl ProgressWidget {
    pub fn new(label: String, current: u64, max: u64) -> Self {
        Self {
            current,
            max,
            label,
            color: Color::Blue,
        }
    }
    
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let progress = if self.max > 0 {
            (self.current as f64 / self.max as f64).min(1.0)
        } else {
            0.0
        };
        
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(&self.label))
            .gauge_style(Style::default().fg(self.color))
            .percent((progress * 100.0) as u16)
            .label(format!("{}/{} ({:.1}%)", self.current, self.max, progress * 100.0));
        
        f.render_widget(gauge, area);
    }
}

