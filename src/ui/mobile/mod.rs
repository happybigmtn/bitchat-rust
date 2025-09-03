//! Mobile UI Components for BitCraps
//!
//! Cross-platform UI components for Android and iOS using a unified API
//! that can be rendered natively on each platform.

pub mod components;
pub mod screens;
pub mod navigation;
pub mod theme;
pub mod state;
pub mod platform_bridge;
pub mod animations;
pub mod screen_base;
pub mod game_screen;
pub mod wallet_screen;
pub mod discovery_screen;
pub mod dice_animation;

// Re-export commonly used types
pub use components::{Button, TextInput, Card, List, Toggle, Component, ComponentView};
pub use screens::{LoginScreen, HomeScreen, GamePlayScreen, WalletScreen, PeerDiscoveryScreen};
pub use navigation::{NavigationController, Route, Tab, TabController};
pub use theme::{ThemeManager, ThemeVariant, SemanticColors};
pub use state::{StateManager, StateAction, StateListener};
pub use animations::{AnimationController, DiceAnimation, FadeAnimation, SlideAnimation, SpringAnimation};

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

/// Mobile UI framework for BitCraps
pub struct MobileUI {
    state: Arc<RwLock<AppState>>,
    navigation: Arc<RwLock<NavigationStack>>,
    theme: Arc<Theme>,
}

/// Application state for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub user: Option<UserProfile>,
    pub current_game: Option<GameState>,
    pub wallet_balance: u64,
    pub connected_peers: Vec<PeerInfo>,
    pub settings: AppSettings,
}

/// User profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub reputation: f64,
    pub games_played: u32,
    pub games_won: u32,
}

/// Current game state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub game_id: String,
    pub players: Vec<PlayerInfo>,
    pub current_player: String,
    pub pot_size: u64,
    pub dice_state: Option<DiceState>,
    pub phase: GamePhase,
}

/// Player information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: String,
    pub username: String,
    pub bet_amount: u64,
    pub is_active: bool,
    pub is_shooter: bool,
}

/// Dice state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiceState {
    pub die1: u8,
    pub die2: u8,
    pub point: Option<u8>,
    pub roll_count: u32,
}

/// Game phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers,
    PlacingBets,
    ComeOutRoll,
    PointPhase,
    RoundComplete,
    GameOver,
}

/// Peer information for mesh network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
    pub username: String,
    pub connection_type: ConnectionType,
    pub signal_strength: i32,
    pub latency_ms: u32,
}

/// Connection type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    Bluetooth,
    WiFiDirect,
    Internet,
}

/// App settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub sound_enabled: bool,
    pub vibration_enabled: bool,
    pub notifications_enabled: bool,
    pub theme_mode: ThemeMode,
    pub language: String,
    pub auto_connect: bool,
}

/// Theme mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    System,
}

/// Navigation stack for screen management
pub struct NavigationStack {
    stack: Vec<Screen>,
    current_index: usize,
}

/// Screen types
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Splash,
    Login,
    Home,
    GameLobby,
    GamePlay,
    Wallet,
    Settings,
    Profile,
    PeerDiscovery,
    Statistics,
}

/// Theme configuration
pub struct Theme {
    pub primary_color: Color,
    pub secondary_color: Color,
    pub background_color: Color,
    pub surface_color: Color,
    pub error_color: Color,
    pub text_color: Color,
    pub font_family: String,
    pub font_sizes: FontSizes,
    pub spacing: Spacing,
    pub border_radius: BorderRadius,
}

/// Color representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 && hex.len() != 8 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).ok()?
        } else {
            255
        };

        Some(Color { r, g, b, a })
    }
}

/// Font sizes
pub struct FontSizes {
    pub heading1: f32,
    pub heading2: f32,
    pub heading3: f32,
    pub body: f32,
    pub caption: f32,
    pub button: f32,
}

/// Spacing values
pub struct Spacing {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
}

/// Border radius values
pub struct BorderRadius {
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub full: f32,
}

impl MobileUI {
    /// Create new mobile UI instance
    pub fn new() -> Self {
        let theme = Arc::new(Theme::default());
        let state = Arc::new(RwLock::new(AppState::default()));
        let navigation = Arc::new(RwLock::new(NavigationStack::new()));

        Self {
            state,
            navigation,
            theme,
        }
    }

    /// Initialize the UI
    pub async fn initialize(&self) -> Result<(), UIError> {
        // Load saved state
        self.load_state().await?;

        // Initialize navigation
        self.navigation.write().await.push(Screen::Splash);

        // Setup theme
        self.apply_theme().await?;

        Ok(())
    }

    /// Navigate to a screen
    pub async fn navigate_to(&self, screen: Screen) -> Result<(), UIError> {
        self.navigation.write().await.push(screen);
        Ok(())
    }

    /// Go back to previous screen
    pub async fn navigate_back(&self) -> Result<(), UIError> {
        self.navigation.write().await.pop();
        Ok(())
    }

    /// Update app state
    pub async fn update_state<F>(&self, updater: F) -> Result<(), UIError>
    where
        F: FnOnce(&mut AppState),
    {
        let mut state = self.state.write().await;
        updater(&mut state);
        self.save_state().await?;
        Ok(())
    }

    /// Get current state
    pub async fn get_state(&self) -> AppState {
        self.state.read().await.clone()
    }

    /// Load saved state
    async fn load_state(&self) -> Result<(), UIError> {
        // In production, would load from persistent storage
        Ok(())
    }

    /// Save current state
    async fn save_state(&self) -> Result<(), UIError> {
        // In production, would save to persistent storage
        Ok(())
    }

    /// Apply theme settings
    async fn apply_theme(&self) -> Result<(), UIError> {
        // In production, would apply theme to native UI
        Ok(())
    }
}

impl NavigationStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            current_index: 0,
        }
    }

    pub fn push(&mut self, screen: Screen) {
        self.stack.push(screen);
        self.current_index = self.stack.len() - 1;
    }

    pub fn pop(&mut self) -> Option<Screen> {
        if self.stack.len() > 1 {
            self.current_index = self.current_index.saturating_sub(1);
            self.stack.pop()
        } else {
            None
        }
    }

    pub fn current(&self) -> Option<&Screen> {
        self.stack.get(self.current_index)
    }

    pub fn clear(&mut self) {
        self.stack.clear();
        self.current_index = 0;
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary_color: Color { r: 102, g: 126, b: 234, a: 255 }, // Purple
            secondary_color: Color { r: 118, g: 75, b: 162, a: 255 }, // Deep purple
            background_color: Color { r: 15, g: 15, b: 15, a: 255 }, // Dark
            surface_color: Color { r: 26, g: 26, b: 26, a: 255 }, // Dark surface
            error_color: Color { r: 239, g: 68, b: 68, a: 255 }, // Red
            text_color: Color { r: 224, g: 224, b: 224, a: 255 }, // Light gray
            font_family: "System".to_string(),
            font_sizes: FontSizes {
                heading1: 32.0,
                heading2: 24.0,
                heading3: 20.0,
                body: 16.0,
                caption: 12.0,
                button: 14.0,
            },
            spacing: Spacing {
                xs: 4.0,
                sm: 8.0,
                md: 16.0,
                lg: 24.0,
                xl: 32.0,
            },
            border_radius: BorderRadius {
                sm: 4.0,
                md: 8.0,
                lg: 16.0,
                full: 999.0,
            },
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            user: None,
            current_game: None,
            wallet_balance: 0,
            connected_peers: Vec::new(),
            settings: AppSettings::default(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            vibration_enabled: true,
            notifications_enabled: true,
            theme_mode: ThemeMode::System,
            language: "en".to_string(),
            auto_connect: true,
        }
    }
}

/// UI Error types
#[derive(Debug)]
pub enum UIError {
    NavigationError(String),
    StateError(String),
    RenderError(String),
    PlatformError(String),
}

impl std::fmt::Display for UIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UIError::NavigationError(msg) => write!(f, "Navigation error: {}", msg),
            UIError::StateError(msg) => write!(f, "State error: {}", msg),
            UIError::RenderError(msg) => write!(f, "Render error: {}", msg),
            UIError::PlatformError(msg) => write!(f, "Platform error: {}", msg),
        }
    }
}

impl std::error::Error for UIError {}

/// Platform-specific UI renderer trait
pub trait UIRenderer {
    /// Render a screen
    fn render_screen(&self, screen: &Screen, state: &AppState, theme: &Theme);

    /// Show dialog
    fn show_dialog(&self, title: &str, message: &str, buttons: Vec<DialogButton>);

    /// Show toast notification
    fn show_toast(&self, message: &str, duration: ToastDuration);

    /// Update status bar
    fn update_status_bar(&self, style: StatusBarStyle);
}

/// Dialog button
pub struct DialogButton {
    pub text: String,
    pub style: ButtonStyle,
    pub action: Box<dyn Fn()>,
}

/// Button style
pub enum ButtonStyle {
    Primary,
    Secondary,
    Danger,
    Text,
}

/// Toast duration
pub enum ToastDuration {
    Short,
    Long,
}

/// Status bar style
pub enum StatusBarStyle {
    Light,
    Dark,
    Hidden,
}