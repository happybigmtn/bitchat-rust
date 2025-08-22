//! Casino module for BitCraps UI
//! 
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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
            active_games: Vec::new(),
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
            selected_bet_type: None,
            bet_amount: 10, // Default bet amount
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

    fn render_game_lobby(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Available games list
        let games: Vec<ListItem> = self.active_games
            .iter()
            .map(|game| {
                let status_color = if game.players.len() < game.max_players {
                    Color::Green
                } else {
                    Color::Yellow
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(&game.game_id, Style::default().fg(Color::Cyan)),
                        Span::raw(" - "),
                        Span::styled(
                            format!("{}/{} players", game.players.len(), game.max_players),
                            Style::default().fg(status_color)
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("  Pot: "),
                        Span::styled(
                            format!("{} bits", game.pot_size),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        ),
                        Span::raw(format!(" | Round: {}", game.round_number)),
                    ]),
                ])
            })
            .collect();

        let games_list = List::new(games)
            .block(Block::default()
                .title("Available Games")
                .borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .highlight_symbol(">> ");

        f.render_widget(games_list, chunks[0]);

        // Game creation panel
        let create_game_text = vec![
            Line::from(vec![Span::styled("Create New Game", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from("Press 'n' to create a new BitCraps game"),
            Line::from("Press 'j' to join selected game"),
            Line::from("Press 'r' to refresh game list"),
            Line::from("Press 'w' to open wallet"),
            Line::from("Press 'h' to view history"),
            Line::from("Press 's' to view statistics"),
            Line::from("Press 'q' to quit casino"),
        ];

        let help_panel = Paragraph::new(create_game_text)
            .block(Block::default()
                .title("Controls")
                .borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        f.render_widget(help_panel, chunks[1]);
    }

    fn render_active_game(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),   // Game state
                Constraint::Min(6),      // Betting area
                Constraint::Length(6),   // Players and bets
            ])
            .split(area);

        // Current game state
        if let Some(game) = self.active_games.first() {
            let game_info = vec![
                Line::from(vec![
                    Span::raw("Game ID: "),
                    Span::styled(&game.game_id, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("Phase: "),
                    Span::styled(format!("{:?}", game.current_phase), Style::default().fg(Color::Yellow)),
                    Span::raw("  Round: "),
                    Span::styled(game.round_number.to_string(), Style::default().fg(Color::Green)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Dice: "),
                    if let Some((d1, d2)) = game.dice_result {
                        Span::styled(format!("🎲{} 🎲{} (Total: {})", d1, d2, d1 + d2), 
                                   Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("Not rolled yet", Style::default().fg(Color::Gray))
                    }
                ]),
                Line::from(vec![
                    Span::raw("Point: "),
                    if let Some(point) = game.point {
                        Span::styled(point.to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled("Not set", Style::default().fg(Color::Gray))
                    }
                ]),
                Line::from(vec![
                    Span::raw("Total Pot: "),
                    Span::styled(format!("{} bits", game.pot_size), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                ]),
            ];

            let game_state = Paragraph::new(game_info)
                .block(Block::default()
                    .title("Game State")
                    .borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            f.render_widget(game_state, chunks[0]);
        }

        // Betting interface
        self.render_betting_area(f, chunks[1]);

        // Players and current bets
        self.render_players_and_bets(f, chunks[2]);
    }

    fn render_betting_area(&self, f: &mut Frame, area: Rect) {
        let betting_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Bet types
        let bet_types = vec![
            ListItem::new("Pass Line (1:1)"),
            ListItem::new("Don't Pass (1:1)"),
            ListItem::new("Come (1:1)"),
            ListItem::new("Don't Come (1:1)"),
            ListItem::new("Field (1:1 or 2:1)"),
            ListItem::new("Big 6 (1:1)"),
            ListItem::new("Big 8 (1:1)"),
            ListItem::new("Hard Ways (varies)"),
            ListItem::new("Any 7 (4:1)"),
            ListItem::new("Any 11 (15:1)"),
            ListItem::new("Any Craps (7:1)"),
        ];

        let bet_list = List::new(bet_types)
            .block(Block::default()
                .title("Available Bets")
                .borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("→ ");

        f.render_widget(bet_list, betting_chunks[0]);

        // Bet amount and controls
        let bet_controls = vec![
            Line::from(vec![
                Span::raw("Current Bet: "),
                Span::styled(format!("{} bits", self.bet_amount), 
                           Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from("Controls:"),
            Line::from("↑/↓ - Select bet type"),
            Line::from("+/- - Adjust bet amount"),
            Line::from("Enter - Place bet"),
            Line::from("Esc - Return to lobby"),
            Line::from("r - Roll dice (if dealer)"),
        ];

        let controls = Paragraph::new(bet_controls)
            .block(Block::default()
                .title("Betting Controls")
                .borders(Borders::ALL));

        f.render_widget(controls, betting_chunks[1]);
    }

    fn render_players_and_bets(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Players in game
        if let Some(game) = self.active_games.first() {
            let players: Vec<ListItem> = game.players
                .iter()
                .map(|player| ListItem::new(format!("👤 {}", player)))
                .collect();

            let players_list = List::new(players)
                .block(Block::default()
                    .title("Players")
                    .borders(Borders::ALL));

            f.render_widget(players_list, chunks[0]);
        }

        // Recent bets
        let recent_bets: Vec<ListItem> = self.bet_history
            .iter()
            .rev()
            .take(5)
            .map(|bet| {
                let result_color = match bet.result {
                    BetResult::Won => Color::Green,
                    BetResult::Lost => Color::Red,
                    BetResult::Push => Color::Yellow,
                    BetResult::Pending => Color::Gray,
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{:?}", bet.bet_type), Style::default().fg(Color::Cyan)),
                        Span::raw(format!(" - {} bits", bet.amount)),
                    ]),
                    Line::from(vec![
                        Span::styled(format!("{:?}", bet.result), Style::default().fg(result_color)),
                        if bet.payout > 0 {
                            Span::styled(format!(" (+{})", bet.payout), Style::default().fg(Color::Green))
                        } else {
                            Span::raw("")
                        },
                    ]),
                ])
            })
            .collect();

        let bets_list = List::new(recent_bets)
            .block(Block::default()
                .title("Recent Bets")
                .borders(Borders::ALL));

        f.render_widget(bets_list, chunks[1]);
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
        let footer_text = match self.current_view {
            CasinoView::GameLobby => "Tab: Switch views | n: New game | j: Join game | q: Quit",
            CasinoView::ActiveGame => "b: Bet | r: Roll (dealer) | Esc: Lobby | Tab: Switch views",
            _ => "Tab: Switch views | Esc: Back | q: Quit casino",
        };

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, area);
    }
}

