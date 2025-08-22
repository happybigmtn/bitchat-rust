//! Casino module for BitCraps UI
//! 
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, 
        Wrap,
    },
    Frame,
};
use serde::{Serialize, Deserialize};
use crate::protocol::craps::GamePhase;
use crate::protocol::BetType;
// use ratatui::widgets::Wrap; // Already imported from prelude

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasinoUI {
    pub current_view: CasinoView,
    pub active_games: Vec<GameSession>,
    pub wallet_balance: u64,
    pub bet_history: Vec<BetRecord>,
    pub game_statistics: GameStats,
    pub selected_bet_type: Option<BetType>,
    pub bet_amount: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CasinoView {
    GameLobby,
    ActiveGame,
    BettingInterface,
    GameHistory,
    WalletManager,
    Statistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSession {
    pub game_id: String,
    pub game_type: String,
    pub players: Vec<String>,
    pub max_players: usize,
    pub current_phase: GamePhase,
    pub pot_size: u64,
    pub round_number: u32,
    pub dice_result: Option<(u8, u8)>,
    pub point: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetRecord {
    pub bet_id: String,
    pub game_id: String,
    pub bet_type: BetType,
    pub amount: u64,
    pub result: BetResult,
    pub payout: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BetResult {
    Pending,
    Won,
    Lost,
    Push, // Tie
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub games_played: u64,
    pub total_wagered: u64,
    pub total_winnings: u64,
    pub biggest_win: u64,
    pub current_streak: i32, // Positive for wins, negative for losses
    pub favorite_bet_type: Option<BetType>,
}

impl CasinoUI {
    pub fn new() -> Self {
        Self {
            current_view: CasinoView::GameLobby,
            active_games: vec![
                GameSession {
                    game_id: "game-001".to_string(),
                    game_type: "BitCraps".to_string(),
                    players: vec!["Player1".to_string(), "Player2".to_string()],
                    max_players: 8,
                    current_phase: GamePhase::ComeOut,
                    pot_size: 250,
                    round_number: 1,
                    dice_result: None,
                    point: None,
                },
                GameSession {
                    game_id: "game-002".to_string(),
                    game_type: "BitCraps".to_string(),
                    players: vec!["Player3".to_string()],
                    max_players: 6,
                    current_phase: GamePhase::Point,
                    pot_size: 180,
                    round_number: 3,
                    dice_result: Some((4, 2)),
                    point: Some(6),
                },
            ],
            wallet_balance: 1000, // Starting balance
            bet_history: Vec::new(),
            game_statistics: GameStats {
                games_played: 0,
                total_wagered: 0,
                total_winnings: 0,
                biggest_win: 0,
                current_streak: 0,
                favorite_bet_type: None,
            },
            selected_bet_type: Some(BetType::Pass),
            bet_amount: 50, // Default bet amount
        }
    }
    
    pub fn handle_enter(&mut self) {
        match self.current_view {
            CasinoView::BettingInterface => self.place_current_bet(),
            CasinoView::ActiveGame => self.place_current_bet(),
            _ => {}
        }
    }
    
    pub fn handle_up(&mut self) {
        match self.current_view {
            CasinoView::BettingInterface | CasinoView::ActiveGame => {
                self.previous_bet_type();
            },
            _ => {}
        }
    }
    
    pub fn handle_down(&mut self) {
        match self.current_view {
            CasinoView::BettingInterface | CasinoView::ActiveGame => {
                self.next_bet_type();
            },
            _ => {}
        }
    }
    
    pub fn handle_left(&mut self) {
        self.decrease_bet_amount();
    }
    
    pub fn handle_right(&mut self) {
        self.increase_bet_amount();
    }
    
    pub fn handle_bet_input(&mut self) {
        self.place_current_bet();
    }
    
    pub fn increase_bet_amount(&mut self) {
        self.bet_amount = (self.bet_amount + 10).min(self.wallet_balance.min(1000));
    }
    
    pub fn decrease_bet_amount(&mut self) {
        self.bet_amount = self.bet_amount.saturating_sub(10).max(10);
    }
    
    fn place_current_bet(&mut self) {
        if let Some(bet_type) = self.selected_bet_type {
            if self.bet_amount <= self.wallet_balance {
                // Create bet record
                let bet_record = BetRecord {
                    bet_id: format!("bet-{}", self.bet_history.len() + 1),
                    game_id: "current-game".to_string(),
                    bet_type,
                    amount: self.bet_amount,
                    result: BetResult::Pending,
                    payout: 0,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                
                // Add to history
                self.bet_history.push(bet_record);
                
                // Deduct from wallet
                self.wallet_balance -= self.bet_amount;
                
                // Update statistics
                self.game_statistics.total_wagered += self.bet_amount;
            }
        }
    }
    
    fn next_bet_type(&mut self) {
        let all_bet_types = [
            BetType::Pass,
            BetType::DontPass,
            BetType::Come,
            BetType::DontCome,
            BetType::Field,
            BetType::Hard4,
            BetType::Hard6,
            BetType::Hard8,
            BetType::Hard10,
            BetType::Next7,
            BetType::Next11,
            BetType::Next2,
            BetType::Next12,
        ];
        
        if let Some(current) = self.selected_bet_type {
            if let Some(pos) = all_bet_types.iter().position(|&x| x == current) {
                let next_pos = (pos + 1) % all_bet_types.len();
                self.selected_bet_type = Some(all_bet_types[next_pos]);
            }
        } else {
            self.selected_bet_type = Some(all_bet_types[0]);
        }
    }
    
    fn previous_bet_type(&mut self) {
        let all_bet_types = [
            BetType::Pass,
            BetType::DontPass,
            BetType::Come,
            BetType::DontCome,
            BetType::Field,
            BetType::Hard4,
            BetType::Hard6,
            BetType::Hard8,
            BetType::Hard10,
            BetType::Next7,
            BetType::Next11,
            BetType::Next2,
            BetType::Next12,
        ];
        
        if let Some(current) = self.selected_bet_type {
            if let Some(pos) = all_bet_types.iter().position(|&x| x == current) {
                let prev_pos = if pos == 0 {
                    all_bet_types.len() - 1
                } else {
                    pos - 1
                };
                self.selected_bet_type = Some(all_bet_types[prev_pos]);
            }
        } else {
            self.selected_bet_type = Some(all_bet_types[0]);
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Main content
                Constraint::Length(3),  // Footer/Status
            ])
            .split(f.area());

        self.render_header(f, chunks[0]);
        self.render_main_content(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);

        // Title
        let title = Paragraph::new("🎲 BitCraps Casino 🎲")
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, header_chunks[0]);

        // Current view
        let view_name = match self.current_view {
            CasinoView::GameLobby => "Game Lobby",
            CasinoView::ActiveGame => "Active Game",
            CasinoView::BettingInterface => "Place Bets",
            CasinoView::GameHistory => "Game History",
            CasinoView::WalletManager => "Wallet",
            CasinoView::Statistics => "Statistics",
        };
        
        let current_view = Paragraph::new(format!("Current: {}", view_name))
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(current_view, header_chunks[1]);

        // Wallet balance
        let balance_color = if self.wallet_balance > 500 { Color::Green } else { Color::Red };
        let wallet = Paragraph::new(format!("Balance: {} bits", self.wallet_balance))
            .style(Style::default().fg(balance_color).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(wallet, header_chunks[2]);
    }

    fn render_main_content(&mut self, f: &mut Frame, area: Rect) {
        match self.current_view {
            CasinoView::GameLobby => self.render_game_lobby(f, area),
            CasinoView::ActiveGame => self.render_active_game(f, area),
            CasinoView::BettingInterface => self.render_betting_interface(f, area),
            CasinoView::GameHistory => self.render_game_history(f, area),
            CasinoView::WalletManager => self.render_wallet_manager(f, area),
            CasinoView::Statistics => self.render_statistics(f, area),
        }
    }

    pub fn render_game_lobby(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),   // Header info
                Constraint::Min(0),      // Games list
                Constraint::Length(6),   // Controls
            ])
            .split(area);

        // Lobby header with stats
        let lobby_header = vec![
            Line::from(vec![
                Span::styled("🏛️ BitCraps Game Lobby", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("🎮 Active Games: "),
                Span::styled(self.active_games.len().to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("  |  🎲 Total Players: "),
                Span::styled(
                    self.active_games.iter().map(|g| g.players.len()).sum::<usize>().to_string(),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  |  💰 Total Pot: "),
                Span::styled(
                    format!("{} CRAP", self.active_games.iter().map(|g| g.pot_size).sum::<u64>()),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(""),
            Line::from("Select a game to join or create a new one:"),
        ];
        
        let header_widget = Paragraph::new(lobby_header)
            .block(Block::default().borders(Borders::ALL).title("Welcome"))
            .wrap(Wrap { trim: true });
        
        f.render_widget(header_widget, chunks[0]);

        // Enhanced games list with more details
        let games: Vec<ListItem> = self.active_games
            .iter()
            .enumerate()
            .map(|(i, game)| {
                let status_color = if game.players.len() < game.max_players {
                    Color::Green
                } else {
                    Color::Red
                };
                
                let phase_color = match game.current_phase {
                    GamePhase::ComeOut => Color::Yellow,
                    GamePhase::Point => Color::Blue,
                    GamePhase::Ended => Color::Gray,
                    GamePhase::GameEnded => Color::DarkGray,
                };
                
                let status_text = if game.players.len() < game.max_players {
                    "🟢 Open"
                } else {
                    "🔴 Full"
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{}. {}", i + 1, game.game_id), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" "),
                        Span::styled(status_text, Style::default().fg(status_color)),
                    ]),
                    Line::from(vec![
                        Span::raw("   Players: "),
                        Span::styled(
                            format!("{}/{}", game.players.len(), game.max_players),
                            Style::default().fg(status_color)
                        ),
                        Span::raw("  |  Phase: "),
                        Span::styled(
                            format!("{:?}", game.current_phase),
                            Style::default().fg(phase_color)
                        ),
                        if let Some(point) = game.point {
                            Span::styled(format!(" (Point: {})", point), Style::default().fg(Color::Magenta))
                        } else {
                            Span::raw("")
                        },
                    ]),
                    Line::from(vec![
                        Span::raw("   Pot: "),
                        Span::styled(
                            format!("{} CRAP", game.pot_size),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        ),
                        Span::raw(format!("  |  Round: {}", game.round_number)),
                        if let Some((d1, d2)) = game.dice_result {
                            Span::styled(
                                format!("  |  Last roll: {}+{}={}", d1, d2, d1 + d2),
                                Style::default().fg(Color::White)
                            )
                        } else {
                            Span::raw("  |  No roll yet")
                        },
                    ]),
                    Line::from(""),
                ])
            })
            .collect();

        let games_list = List::new(games)
            .block(Block::default()
                .title("🎯 Available Games")
                .borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol("► ");

        f.render_widget(games_list, chunks[1]);

        // Enhanced controls panel
        let controls_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);
        
        let game_controls = vec![
            Line::from(vec![Span::styled("🎮 Game Controls", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from("n - Create new game"),
            Line::from("j - Join selected game"),
            Line::from("r - Refresh game list"),
        ];
        
        let nav_controls = vec![
            Line::from(vec![Span::styled("🧭 Navigation", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from("w - Open wallet"),
            Line::from("h - View bet history"),
            Line::from("s - View statistics"),
        ];

        let game_panel = Paragraph::new(game_controls)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });
            
        let nav_panel = Paragraph::new(nav_controls)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(game_panel, controls_chunks[0]);
        f.render_widget(nav_panel, controls_chunks[1]);
    }

    pub fn render_active_game(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),  // Enhanced game state
                Constraint::Min(8),      // Betting area
                Constraint::Length(8),   // Players and bets
            ])
            .split(area);

        // Enhanced current game state with visual elements
        if let Some(game) = self.active_games.first() {
            let state_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(60),
                    Constraint::Percentage(40),
                ])
                .split(chunks[0]);
            
            // Game status and dice
            let dice_display = if let Some((d1, d2)) = game.dice_result {
                let total = d1 + d2;
                let dice_faces = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];
                let d1_face = if d1 >= 1 && d1 <= 6 { dice_faces[(d1-1) as usize] } else { "?" };
                let d2_face = if d2 >= 1 && d2 <= 6 { dice_faces[(d2-1) as usize] } else { "?" };
                
                let (total_color, result_text) = match total {
                    7 | 11 => (Color::Green, "🎉 NATURAL!"),
                    2 | 3 | 12 => (Color::Red, "💥 CRAPS!"),
                    _ => (Color::Yellow, "🎯 POINT"),
                };
                
                vec![
                    Line::from(vec![
                        Span::styled("🎲 DICE RESULT 🎲", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(format!("{} {} = {}", d1_face, d2_face, total), 
                                   Style::default().fg(total_color).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled(result_text, Style::default().fg(total_color).add_modifier(Modifier::BOLD)),
                    ]),
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled("🎲 READY TO ROLL 🎲", Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from("Press 'r' to roll the dice!"),
                    Line::from("Place your bets first."),
                ]
            };
            
            let dice_widget = Paragraph::new(dice_display)
                .block(Block::default().borders(Borders::ALL).title("Dice"))
                .alignment(Alignment::Center);
            
            // Game info
            let phase_color = match game.current_phase {
                GamePhase::ComeOut => Color::Yellow,
                GamePhase::Point => Color::Blue,
                GamePhase::Ended => Color::Gray,
                GamePhase::GameEnded => Color::DarkGray,
            };
            
            let game_info = vec![
                Line::from(vec![
                    Span::styled("🎯 GAME STATUS", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("ID: "),
                    Span::styled(&game.game_id, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(vec![
                    Span::raw("Phase: "),
                    Span::styled(format!("{:?}", game.current_phase), Style::default().fg(phase_color).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("Round: "),
                    Span::styled(game.round_number.to_string(), Style::default().fg(Color::Green)),
                ]),
                if let Some(point) = game.point {
                    Line::from(vec![
                        Span::raw("Point: "),
                        Span::styled(format!("🎯 {}", point), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("Point: "),
                        Span::styled("Not set", Style::default().fg(Color::Gray)),
                    ])
                },
                Line::from(vec![
                    Span::raw("Pot: "),
                    Span::styled(format!("💰 {} CRAP", game.pot_size), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
            ];

            let game_state = Paragraph::new(game_info)
                .block(Block::default().title("Status").borders(Borders::ALL));

            f.render_widget(dice_widget, state_chunks[0]);
            f.render_widget(game_state, state_chunks[1]);
        }

        // Enhanced betting interface
        self.render_betting_area(f, chunks[1]);

        // Enhanced players and bets display
        self.render_players_and_bets(f, chunks[2]);
    }

    fn render_betting_area(&self, f: &mut Frame, area: Rect) {
        let betting_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Enhanced bet types with odds and descriptions
        let bet_options = [
            (BetType::Pass, "Pass Line", "1:1", "Win on 7/11, lose on 2/3/12"),
            (BetType::DontPass, "Don't Pass", "1:1", "Opposite of Pass Line"),
            (BetType::Come, "Come", "1:1", "Like Pass but after point"),
            (BetType::DontCome, "Don't Come", "1:1", "Opposite of Come"),
            (BetType::Field, "Field", "1:1/2:1", "One roll: 2,3,4,9,10,11,12"),
            (BetType::Hard4, "Hard 4", "7:1", "Two 2s before any 4 or 7"),
            (BetType::Hard6, "Hard 6", "9:1", "Two 3s before any 6 or 7"),
            (BetType::Hard8, "Hard 8", "9:1", "Two 4s before any 8 or 7"),
            (BetType::Hard10, "Hard 10", "7:1", "Two 5s before any 10 or 7"),
            (BetType::Next7, "Any 7", "4:1", "Next roll is 7"),
            (BetType::Next11, "Any 11", "15:1", "Next roll is 11"),
            (BetType::Next2, "Snake Eyes", "30:1", "Next roll is 2"),
            (BetType::Next12, "Boxcars", "30:1", "Next roll is 12"),
        ];

        let bet_items: Vec<ListItem> = bet_options
            .iter()
            .map(|(bet_type, name, odds, desc)| {
                let is_selected = self.selected_bet_type == Some(*bet_type);
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{} ({})", name, odds), style),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("  {}", desc), Style::default().fg(Color::Gray)),
                    ]),
                ])
            })
            .collect();

        let bet_list = List::new(bet_items)
            .block(Block::default()
                .title("🎯 Available Bets (↑/↓ to select)")
                .borders(Borders::ALL))
            .highlight_style(Style::default())
            .highlight_symbol("");

        f.render_widget(bet_list, betting_chunks[0]);

        // Enhanced betting controls
        let selected_bet_name = if let Some(bet_type) = self.selected_bet_type {
            bet_options.iter()
                .find(|(bt, _, _, _)| *bt == bet_type)
                .map(|(_, name, _, _)| *name)
                .unwrap_or("Unknown")
        } else {
            "None"
        };
        
        let balance_color = if self.bet_amount > self.wallet_balance {
            Color::Red
        } else if self.bet_amount > self.wallet_balance / 2 {
            Color::Yellow
        } else {
            Color::Green
        };
        
        let bet_controls = vec![
            Line::from(vec![
                Span::styled("💰 BETTING CONTROLS", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Selected: "),
                Span::styled(selected_bet_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Amount: "),
                Span::styled(format!("{} CRAP", self.bet_amount), Style::default().fg(balance_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Wallet: "),
                Span::styled(format!("{} CRAP", self.wallet_balance), Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from("🎮 Controls:"),
            Line::from("  ↑/↓  Select bet type"),
            Line::from("  ←/→  Adjust amount"),
            Line::from("  +/-  Quick adjust"),
            Line::from("  Enter Place bet"),
            Line::from("  b    Quick bet"),
            Line::from("  Esc  Return to lobby"),
        ];

        let controls = Paragraph::new(bet_controls)
            .block(Block::default()
                .title("Controls")
                .borders(Borders::ALL));

        f.render_widget(controls, betting_chunks[1]);
    }

    fn render_players_and_bets(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Enhanced players display with status
        if let Some(game) = self.active_games.first() {
            let player_items: Vec<ListItem> = game.players
                .iter()
                .enumerate()
                .map(|(i, player)| {
                    let status = if i == 0 { "🎲 Shooter" } else { "🎯 Player" };
                    let balance = 1000 - (i * 200) as u64; // Simulate different balances
                    
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(format!("{} {}", status, player), Style::default().fg(Color::Cyan)),
                        ]),
                        Line::from(vec![
                            Span::raw("  Balance: "),
                            Span::styled(format!("{} CRAP", balance), Style::default().fg(Color::Yellow)),
                        ]),
                    ])
                })
                .collect();

            let players_list = List::new(player_items)
                .block(Block::default()
                    .title(format!("👥 Players ({}/{})", game.players.len(), game.max_players))
                    .borders(Borders::ALL));

            f.render_widget(players_list, chunks[0]);
        }

        // Enhanced betting display with more details
        let bet_display = if self.bet_history.is_empty() {
            vec![
                Line::from("📊 No bets placed yet"),
                Line::from(""),
                Line::from("Place your first bet to"),
                Line::from("start playing!"),
                Line::from(""),
                Line::from("💡 Tips:"),
                Line::from("• Pass Line is beginner-friendly"),
                Line::from("• Field bets are one-roll only"),
                Line::from("• Hard ways pay big but risky"),
            ]
        } else {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("📊 Betting Summary", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
            ];
            
            // Add recent bets
            for bet in self.bet_history.iter().rev().take(3) {
                let result_color = match bet.result {
                    BetResult::Won => Color::Green,
                    BetResult::Lost => Color::Red,
                    BetResult::Push => Color::Yellow,
                    BetResult::Pending => Color::LightBlue,
                };
                
                let status_icon = match bet.result {
                    BetResult::Won => "✅",
                    BetResult::Lost => "❌",
                    BetResult::Push => "🟡",
                    BetResult::Pending => "⏳",
                };
                
                lines.push(Line::from(vec![
                    Span::raw(format!("{} ", status_icon)),
                    Span::styled(format!("{:?}", bet.bet_type), Style::default().fg(Color::White)),
                ]));
                
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{} CRAP", bet.amount), Style::default().fg(Color::Yellow)),
                    Span::raw(" → "),
                    Span::styled(
                        format!("{:?}", bet.result),
                        Style::default().fg(result_color)
                    ),
                    if bet.payout > 0 {
                        Span::styled(format!(" (+{})", bet.payout), Style::default().fg(Color::Green))
                    } else {
                        Span::raw("")
                    },
                ]));
            }
            
            // Add summary stats
            if self.bet_history.len() > 3 {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::raw("... and "),
                    Span::styled(
                        format!("{} more bets", self.bet_history.len() - 3),
                        Style::default().fg(Color::Gray)
                    ),
                ]));
            }
            
            lines
        };

        let bets_widget = Paragraph::new(bet_display)
            .block(Block::default()
                .title("🎰 Betting Activity")
                .borders(Borders::ALL))
            .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(bets_widget, chunks[1]);
    }

    fn render_betting_interface(&self, f: &mut Frame, area: Rect) {
        // Similar to render_betting_area but focused on bet placement
        self.render_betting_area(f, area);
    }

    fn render_wallet_manager(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),   // Balance info
                Constraint::Min(0),      // Transaction history
            ])
            .split(area);

        // Wallet balance and info
        let balance_info = vec![
            Line::from(vec![
                Span::raw("Current Balance: "),
                Span::styled(format!("{} bits", self.wallet_balance), 
                           Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Total Wagered: "),
                Span::styled(format!("{} bits", self.game_statistics.total_wagered), 
                           Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Total Winnings: "),
                Span::styled(format!("{} bits", self.game_statistics.total_winnings), 
                           Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("Net P&L: "),
                {
                    let net = self.game_statistics.total_winnings as i64 - self.game_statistics.total_wagered as i64;
                    let color = if net >= 0 { Color::Green } else { Color::Red };
                    Span::styled(format!("{} bits", net), Style::default().fg(color).add_modifier(Modifier::BOLD))
                },
            ]),
        ];

        let wallet_info = Paragraph::new(balance_info)
            .block(Block::default()
                .title("Wallet Information")
                .borders(Borders::ALL));

        f.render_widget(wallet_info, chunks[0]);

        // Transaction history would go in chunks[1]
        let tx_placeholder = Paragraph::new("Transaction history coming soon...")
            .block(Block::default()
                .title("Transaction History")
                .borders(Borders::ALL));

        f.render_widget(tx_placeholder, chunks[1]);
    }

    fn render_game_history(&self, f: &mut Frame, area: Rect) {
        let history_items: Vec<ListItem> = self.bet_history
            .iter()
            .rev()
            .map(|bet| {
                let result_color = match bet.result {
                    BetResult::Won => Color::Green,
                    BetResult::Lost => Color::Red,
                    BetResult::Push => Color::Yellow,
                    BetResult::Pending => Color::Gray,
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::raw(format!("Game: {} | ", bet.game_id.chars().take(8).collect::<String>())),
                        Span::styled(format!("{:?}", bet.bet_type), Style::default().fg(Color::Cyan)),
                        Span::raw(format!(" | {} bits", bet.amount)),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("{:?}", bet.result), Style::default().fg(result_color)),
                        if bet.payout > 0 {
                            Span::styled(format!(" | Payout: +{} bits", bet.payout), Style::default().fg(Color::Green))
                        } else {
                            Span::raw(" | No payout")
                        },
                    ]),
                ])
            })
            .collect();

        let history_list = List::new(history_items)
            .block(Block::default()
                .title("Betting History")
                .borders(Borders::ALL));

        f.render_widget(history_list, area);
    }

    fn render_statistics(&self, f: &mut Frame, area: Rect) {
        let stats = &self.game_statistics;
        
        let stats_text = vec![
            Line::from(vec![
                Span::raw("Games Played: "),
                Span::styled(stats.games_played.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Total Wagered: "),
                Span::styled(format!("{} bits", stats.total_wagered), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Total Winnings: "),
                Span::styled(format!("{} bits", stats.total_winnings), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Biggest Win: "),
                Span::styled(format!("{} bits", stats.biggest_win), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Current Streak: "),
                {
                    let (color, prefix) = if stats.current_streak >= 0 {
                        (Color::Green, "+")
                    } else {
                        (Color::Red, "")
                    };
                    Span::styled(format!("{}{}", prefix, stats.current_streak), Style::default().fg(color))
                },
            ]),
            Line::from(vec![
                Span::raw("Win Rate: "),
                {
                    let win_rate = if stats.games_played > 0 {
                        (stats.total_winnings as f64 / stats.total_wagered as f64) * 100.0
                    } else {
                        0.0
                    };
                    let color = if win_rate >= 50.0 { Color::Green } else { Color::Red };
                    Span::styled(format!("{:.1}%", win_rate), Style::default().fg(color))
                },
            ]),
        ];

        let statistics = Paragraph::new(stats_text)
            .block(Block::default()
                .title("Game Statistics")
                .borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(statistics, area);
    }

    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let footer_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(area);
        
        let (left_text, right_text) = match self.current_view {
            CasinoView::GameLobby => (
                "🎮 n: New game | j: Join game | r: Refresh",
                "🔄 Tab: Switch views | q: Quit casino"
            ),
            CasinoView::ActiveGame => (
                "🎲 r: Roll dice | b: Place bet | ↑↓: Select bet",
                "📤 Esc: Leave game | Tab: Switch views"
            ),
            CasinoView::BettingInterface => (
                "💰 ↑↓: Select bet | ←→: Amount | Enter: Place",
                "↩️ Esc: Back | Tab: Switch views"
            ),
            _ => (
                "🎯 Casino controls available",
                "🔄 Tab: Switch views | Esc: Back | q: Quit"
            ),
        };

        let left_footer = Paragraph::new(left_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL));
        
        let right_footer = Paragraph::new(right_text)
            .style(Style::default().fg(Color::LightBlue))
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(left_footer, footer_chunks[0]);
        f.render_widget(right_footer, footer_chunks[1]);
    }
}

