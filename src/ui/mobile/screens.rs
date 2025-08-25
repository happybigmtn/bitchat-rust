//! Screen implementations for mobile UI

use super::*;
use crate::protocol::{GameState as ProtocolGameState, BetType};
use crate::mesh::PeerId;
use crate::token::CRAPToken;

/// Login screen for user authentication
pub struct LoginScreen {
    username_input: String,
    password_input: String,
    error_message: Option<String>,
    loading: bool,
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            username_input: String::new(),
            password_input: String::new(),
            error_message: None,
            loading: false,
        }
    }

    pub fn handle_login(&mut self) -> LoginAction {
        if self.username_input.is_empty() {
            self.error_message = Some("Username is required".to_string());
            return LoginAction::ValidationError;
        }

        if self.password_input.len() < 8 {
            self.error_message = Some("Password must be at least 8 characters".to_string());
            return LoginAction::ValidationError;
        }

        self.loading = true;
        LoginAction::Proceed {
            username: self.username_input.clone(),
            password: self.password_input.clone(),
        }
    }

    pub fn render(&self) -> ScreenContent {
        ScreenContent {
            title: "BitCraps Login".to_string(),
            body: ScreenBody::Form(FormContent {
                fields: vec![
                    FormField::TextInput {
                        label: "Username".to_string(),
                        value: self.username_input.clone(),
                        placeholder: Some("Enter username".to_string()),
                        secure: false,
                    },
                    FormField::TextInput {
                        label: "Password".to_string(),
                        value: self.password_input.clone(),
                        placeholder: Some("Enter password".to_string()),
                        secure: true,
                    },
                ],
                error: self.error_message.clone(),
                submit_button: ButtonConfig {
                    text: "Login".to_string(),
                    enabled: !self.loading,
                    style: ButtonStyle::Primary,
                },
            }),
            navigation: None,
        }
    }
}

/// Home screen showing main menu and stats
pub struct HomeScreen {
    user_profile: UserProfile,
    quick_stats: QuickStats,
    active_games: Vec<GameSummary>,
}

impl HomeScreen {
    pub fn new(user_profile: UserProfile) -> Self {
        Self {
            user_profile,
            quick_stats: QuickStats::default(),
            active_games: Vec::new(),
        }
    }

    pub fn update_stats(&mut self, stats: QuickStats) {
        self.quick_stats = stats;
    }

    pub fn render(&self) -> ScreenContent {
        ScreenContent {
            title: format!("Welcome, {}", self.user_profile.username),
            body: ScreenBody::Dashboard(DashboardContent {
                sections: vec![
                    DashboardSection::Stats {
                        title: "Quick Stats".to_string(),
                        items: vec![
                            StatItem {
                                label: "Wallet Balance".to_string(),
                                value: format!("{} CRAP", self.quick_stats.balance),
                                icon: Some("wallet".to_string()),
                            },
                            StatItem {
                                label: "Win Rate".to_string(),
                                value: format!("{}%", self.quick_stats.win_rate),
                                icon: Some("trophy".to_string()),
                            },
                            StatItem {
                                label: "Games Played".to_string(),
                                value: self.quick_stats.games_played.to_string(),
                                icon: Some("dice".to_string()),
                            },
                        ],
                    },
                    DashboardSection::ActionButtons {
                        buttons: vec![
                            ButtonConfig {
                                text: "Quick Game".to_string(),
                                enabled: true,
                                style: ButtonStyle::Primary,
                            },
                            ButtonConfig {
                                text: "Find Players".to_string(),
                                enabled: true,
                                style: ButtonStyle::Secondary,
                            },
                            ButtonConfig {
                                text: "Tournament".to_string(),
                                enabled: true,
                                style: ButtonStyle::Secondary,
                            },
                        ],
                    },
                    DashboardSection::List {
                        title: "Active Games".to_string(),
                        items: self.active_games.iter().map(|game| ListItem {
                            title: format!("Game #{}", &game.id[..8]),
                            subtitle: Some(format!("{} players", game.player_count)),
                            value: Some(format!("{} CRAP", game.pot_size)),
                            action: Some(ListAction::Navigate(Screen::GamePlay)),
                        }).collect(),
                    },
                ],
            }),
            navigation: Some(NavigationBar {
                title: "BitCraps".to_string(),
                left_action: None,
                right_actions: vec![
                    NavAction::Icon {
                        icon: "settings".to_string(),
                        action: Screen::Settings,
                    },
                    NavAction::Icon {
                        icon: "profile".to_string(),
                        action: Screen::Profile,
                    },
                ],
            }),
        }
    }
}

/// Game play screen for active craps game
pub struct GamePlayScreen {
    game_state: GameState,
    dice_animation: Option<DiceAnimation>,
    bet_controls: BetControls,
    message_log: Vec<GameMessage>,
}

impl GamePlayScreen {
    pub fn new(game_id: String) -> Self {
        Self {
            game_state: GameState {
                game_id,
                players: Vec::new(),
                current_player: String::new(),
                pot_size: 0,
                dice_state: None,
                phase: GamePhase::WaitingForPlayers,
            },
            dice_animation: None,
            bet_controls: BetControls::default(),
            message_log: Vec::new(),
        }
    }

    pub fn handle_dice_roll(&mut self, die1: u8, die2: u8) {
        self.dice_animation = Some(DiceAnimation {
            die1_value: die1,
            die2_value: die2,
            duration_ms: 2000,
            started_at: std::time::SystemTime::now(),
        });

        self.game_state.dice_state = Some(DiceState {
            die1,
            die2,
            point: self.calculate_point(die1, die2),
            roll_count: self.game_state.dice_state
                .as_ref()
                .map(|d| d.roll_count + 1)
                .unwrap_or(1),
        });
    }

    fn calculate_point(&self, die1: u8, die2: u8) -> Option<u8> {
        let sum = die1 + die2;
        match self.game_state.phase {
            GamePhase::ComeOutRoll => {
                match sum {
                    7 | 11 => None, // Natural win
                    2 | 3 | 12 => None, // Craps
                    _ => Some(sum), // Point established
                }
            }
            GamePhase::PointPhase => {
                self.game_state.dice_state
                    .as_ref()
                    .and_then(|d| d.point)
            }
            _ => None,
        }
    }

    pub fn render(&self) -> ScreenContent {
        ScreenContent {
            title: "Game in Progress".to_string(),
            body: ScreenBody::Game(GameContent {
                dice_display: self.dice_animation.as_ref().map(|anim| DiceDisplay {
                    die1: anim.die1_value,
                    die2: anim.die2_value,
                    animating: anim.is_active(),
                }),
                bet_table: BetTable {
                    sections: vec![
                        BetSection {
                            name: "Pass Line".to_string(),
                            bets: self.get_pass_line_bets(),
                            enabled: matches!(self.game_state.phase, GamePhase::PlacingBets),
                        },
                        BetSection {
                            name: "Don't Pass".to_string(),
                            bets: self.get_dont_pass_bets(),
                            enabled: matches!(self.game_state.phase, GamePhase::PlacingBets),
                        },
                        BetSection {
                            name: "Field".to_string(),
                            bets: self.get_field_bets(),
                            enabled: true,
                        },
                    ],
                },
                player_list: self.game_state.players.clone(),
                controls: GameControls {
                    roll_button: ButtonConfig {
                        text: "Roll Dice".to_string(),
                        enabled: self.is_current_player_turn(),
                        style: ButtonStyle::Primary,
                    },
                    bet_input: self.bet_controls.clone(),
                },
                message_log: self.message_log.clone(),
            }),
            navigation: Some(NavigationBar {
                title: format!("Pot: {} CRAP", self.game_state.pot_size),
                left_action: Some(NavAction::Back),
                right_actions: vec![
                    NavAction::Text {
                        text: self.get_phase_text(),
                        action: None,
                    },
                ],
            }),
        }
    }

    fn is_current_player_turn(&self) -> bool {
        // Check if current user is the shooter
        false // Placeholder
    }

    fn get_phase_text(&self) -> String {
        match self.game_state.phase {
            GamePhase::WaitingForPlayers => "Waiting...".to_string(),
            GamePhase::PlacingBets => "Place Bets".to_string(),
            GamePhase::ComeOutRoll => "Come Out".to_string(),
            GamePhase::PointPhase => {
                if let Some(dice) = &self.game_state.dice_state {
                    if let Some(point) = dice.point {
                        format!("Point: {}", point)
                    } else {
                        "Point Phase".to_string()
                    }
                } else {
                    "Point Phase".to_string()
                }
            }
            GamePhase::RoundComplete => "Round Over".to_string(),
            GamePhase::GameOver => "Game Over".to_string(),
        }
    }

    fn get_pass_line_bets(&self) -> Vec<BetDisplay> {
        Vec::new() // Placeholder
    }

    fn get_dont_pass_bets(&self) -> Vec<BetDisplay> {
        Vec::new() // Placeholder
    }

    fn get_field_bets(&self) -> Vec<BetDisplay> {
        Vec::new() // Placeholder
    }
}

/// Wallet screen for managing CRAP tokens
pub struct WalletScreen {
    balance: u64,
    transactions: Vec<Transaction>,
    send_form: Option<SendForm>,
}

impl WalletScreen {
    pub fn new(balance: u64) -> Self {
        Self {
            balance,
            transactions: Vec::new(),
            send_form: None,
        }
    }

    pub fn show_send_form(&mut self) {
        self.send_form = Some(SendForm::default());
    }

    pub fn render(&self) -> ScreenContent {
        ScreenContent {
            title: "Wallet".to_string(),
            body: ScreenBody::Wallet(WalletContent {
                balance_display: BalanceDisplay {
                    amount: self.balance,
                    currency: "CRAP".to_string(),
                    fiat_equivalent: self.calculate_fiat_value(),
                },
                actions: vec![
                    ButtonConfig {
                        text: "Send".to_string(),
                        enabled: self.balance > 0,
                        style: ButtonStyle::Primary,
                    },
                    ButtonConfig {
                        text: "Receive".to_string(),
                        enabled: true,
                        style: ButtonStyle::Secondary,
                    },
                    ButtonConfig {
                        text: "Buy CRAP".to_string(),
                        enabled: true,
                        style: ButtonStyle::Secondary,
                    },
                ],
                transaction_list: self.transactions.clone(),
                send_form: self.send_form.clone(),
            }),
            navigation: Some(NavigationBar {
                title: "Wallet".to_string(),
                left_action: Some(NavAction::Back),
                right_actions: vec![
                    NavAction::Icon {
                        icon: "history".to_string(),
                        action: Screen::Home, // Should be transaction history
                    },
                ],
            }),
        }
    }

    fn calculate_fiat_value(&self) -> Option<String> {
        // Placeholder for fiat conversion
        None
    }
}

/// Peer discovery screen for finding players
pub struct PeerDiscoveryScreen {
    discovered_peers: Vec<PeerInfo>,
    scanning: bool,
    connection_status: HashMap<String, ConnectionStatus>,
}

impl PeerDiscoveryScreen {
    pub fn new() -> Self {
        Self {
            discovered_peers: Vec::new(),
            scanning: false,
            connection_status: HashMap::new(),
        }
    }

    pub fn start_scan(&mut self) {
        self.scanning = true;
        self.discovered_peers.clear();
    }

    pub fn add_peer(&mut self, peer: PeerInfo) {
        if !self.discovered_peers.iter().any(|p| p.id == peer.id) {
            self.discovered_peers.push(peer);
        }
    }

    pub fn render(&self) -> ScreenContent {
        ScreenContent {
            title: "Find Players".to_string(),
            body: ScreenBody::PeerList(PeerListContent {
                scanning: self.scanning,
                peers: self.discovered_peers.iter().map(|peer| {
                    PeerDisplay {
                        info: peer.clone(),
                        status: self.connection_status
                            .get(&peer.id)
                            .cloned()
                            .unwrap_or(ConnectionStatus::Disconnected),
                        actions: vec![
                            ButtonConfig {
                                text: "Connect".to_string(),
                                enabled: !self.is_connected(&peer.id),
                                style: ButtonStyle::Primary,
                            },
                            ButtonConfig {
                                text: "Invite".to_string(),
                                enabled: self.is_connected(&peer.id),
                                style: ButtonStyle::Secondary,
                            },
                        ],
                    }
                }).collect(),
                filters: PeerFilters {
                    connection_type: None,
                    min_signal_strength: None,
                    max_latency: None,
                },
            }),
            navigation: Some(NavigationBar {
                title: format!("{} Players", self.discovered_peers.len()),
                left_action: Some(NavAction::Back),
                right_actions: vec![
                    NavAction::Icon {
                        icon: if self.scanning { "stop" } else { "refresh" }.to_string(),
                        action: Screen::PeerDiscovery,
                    },
                ],
            }),
        }
    }

    fn is_connected(&self, peer_id: &str) -> bool {
        matches!(
            self.connection_status.get(peer_id),
            Some(ConnectionStatus::Connected)
        )
    }
}

// Screen content structures
#[derive(Debug, Clone)]
pub struct ScreenContent {
    pub title: String,
    pub body: ScreenBody,
    pub navigation: Option<NavigationBar>,
}

#[derive(Debug, Clone)]
pub enum ScreenBody {
    Form(FormContent),
    Dashboard(DashboardContent),
    Game(GameContent),
    Wallet(WalletContent),
    PeerList(PeerListContent),
    Settings(SettingsContent),
    Profile(ProfileContent),
}

#[derive(Debug, Clone)]
pub struct FormContent {
    pub fields: Vec<FormField>,
    pub error: Option<String>,
    pub submit_button: ButtonConfig,
}

#[derive(Debug, Clone)]
pub enum FormField {
    TextInput {
        label: String,
        value: String,
        placeholder: Option<String>,
        secure: bool,
    },
    Toggle {
        label: String,
        value: bool,
    },
    Dropdown {
        label: String,
        value: String,
        options: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct DashboardContent {
    pub sections: Vec<DashboardSection>,
}

#[derive(Debug, Clone)]
pub enum DashboardSection {
    Stats {
        title: String,
        items: Vec<StatItem>,
    },
    ActionButtons {
        buttons: Vec<ButtonConfig>,
    },
    List {
        title: String,
        items: Vec<ListItem>,
    },
}

#[derive(Debug, Clone)]
pub struct StatItem {
    pub label: String,
    pub value: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub title: String,
    pub subtitle: Option<String>,
    pub value: Option<String>,
    pub action: Option<ListAction>,
}

#[derive(Debug, Clone)]
pub enum ListAction {
    Navigate(Screen),
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct GameContent {
    pub dice_display: Option<DiceDisplay>,
    pub bet_table: BetTable,
    pub player_list: Vec<PlayerInfo>,
    pub controls: GameControls,
    pub message_log: Vec<GameMessage>,
}

#[derive(Debug, Clone)]
pub struct DiceDisplay {
    pub die1: u8,
    pub die2: u8,
    pub animating: bool,
}

#[derive(Debug, Clone)]
pub struct BetTable {
    pub sections: Vec<BetSection>,
}

#[derive(Debug, Clone)]
pub struct BetSection {
    pub name: String,
    pub bets: Vec<BetDisplay>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BetDisplay {
    pub player: String,
    pub amount: u64,
    pub bet_type: String,
}

#[derive(Debug, Clone)]
pub struct GameControls {
    pub roll_button: ButtonConfig,
    pub bet_input: BetControls,
}

#[derive(Debug, Clone, Default)]
pub struct BetControls {
    pub amount: u64,
    pub bet_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameMessage {
    pub timestamp: std::time::SystemTime,
    pub player: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct WalletContent {
    pub balance_display: BalanceDisplay,
    pub actions: Vec<ButtonConfig>,
    pub transaction_list: Vec<Transaction>,
    pub send_form: Option<SendForm>,
}

#[derive(Debug, Clone)]
pub struct BalanceDisplay {
    pub amount: u64,
    pub currency: String,
    pub fiat_equivalent: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: String,
    pub timestamp: std::time::SystemTime,
    pub transaction_type: TransactionType,
    pub amount: u64,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone)]
pub enum TransactionType {
    Send { to: String },
    Receive { from: String },
    GameWin { game_id: String },
    GameLoss { game_id: String },
}

#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct SendForm {
    pub recipient: String,
    pub amount: u64,
    pub memo: String,
}

#[derive(Debug, Clone)]
pub struct PeerListContent {
    pub scanning: bool,
    pub peers: Vec<PeerDisplay>,
    pub filters: PeerFilters,
}

#[derive(Debug, Clone)]
pub struct PeerDisplay {
    pub info: PeerInfo,
    pub status: ConnectionStatus,
    pub actions: Vec<ButtonConfig>,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone)]
pub struct PeerFilters {
    pub connection_type: Option<ConnectionType>,
    pub min_signal_strength: Option<i32>,
    pub max_latency: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SettingsContent {
    pub sections: Vec<SettingsSection>,
}

#[derive(Debug, Clone)]
pub struct SettingsSection {
    pub title: String,
    pub items: Vec<SettingItem>,
}

#[derive(Debug, Clone)]
pub enum SettingItem {
    Toggle {
        label: String,
        value: bool,
        description: Option<String>,
    },
    Slider {
        label: String,
        value: f32,
        min: f32,
        max: f32,
    },
    Button {
        label: String,
        action: String,
    },
}

#[derive(Debug, Clone)]
pub struct ProfileContent {
    pub user_info: UserProfile,
    pub stats: ProfileStats,
    pub achievements: Vec<Achievement>,
}

#[derive(Debug, Clone)]
pub struct ProfileStats {
    pub total_wagered: u64,
    pub total_won: u64,
    pub biggest_win: u64,
    pub longest_streak: u32,
    pub favorite_bet: String,
}

#[derive(Debug, Clone)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub unlocked: bool,
    pub progress: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct NavigationBar {
    pub title: String,
    pub left_action: Option<NavAction>,
    pub right_actions: Vec<NavAction>,
}

#[derive(Debug, Clone)]
pub enum NavAction {
    Back,
    Icon { icon: String, action: Screen },
    Text { text: String, action: Option<Screen> },
}

#[derive(Debug, Clone)]
pub struct ButtonConfig {
    pub text: String,
    pub enabled: bool,
    pub style: ButtonStyle,
}

// Helper types
#[derive(Debug, Clone, Default)]
pub struct QuickStats {
    pub balance: u64,
    pub win_rate: f32,
    pub games_played: u32,
}

#[derive(Debug, Clone)]
pub struct GameSummary {
    pub id: String,
    pub player_count: usize,
    pub pot_size: u64,
}

#[derive(Debug, Clone)]
pub struct DiceAnimation {
    pub die1_value: u8,
    pub die2_value: u8,
    pub duration_ms: u64,
    pub started_at: std::time::SystemTime,
}

impl DiceAnimation {
    pub fn is_active(&self) -> bool {
        if let Ok(elapsed) = std::time::SystemTime::now().duration_since(self.started_at) {
            elapsed.as_millis() < self.duration_ms as u128
        } else {
            false
        }
    }
}

// Action types
#[derive(Debug, Clone)]
pub enum LoginAction {
    Proceed { username: String, password: String },
    ValidationError,
}

use std::collections::HashMap;