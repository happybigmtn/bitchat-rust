# Week 5: Application Layer & CLI

## ⚠️ IMPORTANT: Updated Implementation Notes

**Before starting this week, please review `/docs/COMPILATION_FIXES.md` for critical dependency and API updates.**

**Key fixes for Week 5:**
- Add UI dependencies: `clap = "4.5.45"`, `ratatui = "0.29.0"`, `crossterm = "0.29.0"`
- Use `f.area()` instead of deprecated `f.size()` for ratatui
- Set cursor position as tuple: `f.set_cursor_position((x, y))`
- Remove unnecessary Backend type parameters from Widget implementations
- Use current clap derive syntax with `#[command(...)]`

## Overview
Implement the user-facing application layer with CLI interface and terminal UI for BitChat, including specialized casino UI components for BitCraps gaming and wallet integration.

## Day 1: CLI Framework with clap and ratatui TUI

### CLI Setup with clap
```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bitchat")]
#[command(about = "Decentralized P2P chat application")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    #[arg(short, long, default_value = "8080")]
    port: u16,
    
    #[arg(short, long)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
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
```

### ratatui TUI Components
```rust
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

struct App {
    messages: Vec<ChatMessage>,
    input: String,
    peers: Vec<PeerInfo>,
    current_view: ViewMode,
}

enum ViewMode {
    Chat,
    PeerList,
    Settings,
}

// Main TUI render function
fn render_ui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &App) {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),     // Chat area
                Constraint::Length(3),  // Input area
                Constraint::Length(1),  // Status bar
            ])
            .split(f.size());
            
        render_chat_area(f, &chunks[0], app);
        render_input_area(f, &chunks[1], app);
        render_status_bar(f, &chunks[2], app);
    });
}
```

## Day 2: Command Processor (IRC-style Commands)

### Command System
```rust
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

pub struct CommandProcessor {
    network: Arc<NetworkManager>,
    config: Arc<RwLock<Config>>,
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
        }
    }
}
```

### Auto-completion System
```rust
pub struct AutoComplete {
    commands: Vec<String>,
    peers: Vec<String>,
    channels: Vec<String>,
}

impl AutoComplete {
    pub fn complete(&self, input: &str) -> Vec<String> {
        if input.starts_with('/') {
            self.complete_command(input)
        } else {
            self.complete_peer_or_channel(input)
        }
    }
    
    fn complete_command(&self, input: &str) -> Vec<String> {
        self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(input))
            .cloned()
            .collect()
    }
}
```

## Day 3: Message Formatting and Display

### Message Types and Formatting
```rust
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
    File(FileTransfer),
    System(SystemMessage),
    Encrypted(EncryptedContent),
}

#[derive(Debug, Clone)]
pub enum SystemMessage {
    PeerJoined(String),
    PeerLeft(String),
    ChannelCreated(String),
    Error(String),
}

pub struct MessageFormatter;

impl MessageFormatter {
    pub fn format_message(msg: &ChatMessage) -> Vec<Span> {
        let timestamp = format_timestamp(msg.timestamp);
        let sender = format_sender(&msg.sender);
        
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
            MessageContent::System(sys_msg) => {
                vec![
                    Span::styled(timestamp, Style::default().fg(Color::Gray)),
                    Span::raw(" "),
                    Span::styled(format!("* {}", sys_msg), Style::default().fg(Color::Yellow)),
                ]
            }
            // ... other message types
        }
    }
    
    pub fn format_for_export(messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|msg| {
                format!(
                    "[{}] {}: {}",
                    format_timestamp(msg.timestamp),
                    msg.sender,
                    msg.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

### Chat View Components
```rust
pub struct ChatView {
    messages: Vec<ChatMessage>,
    scroll_offset: usize,
    filter: MessageFilter,
}

impl ChatView {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let visible_messages = self.get_visible_messages();
        let items: Vec<ListItem> = visible_messages
            .iter()
            .map(|msg| {
                let spans = MessageFormatter::format_message(msg);
                ListItem::new(Line::from(spans))
            })
            .collect();
            
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
    
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }
    
    pub fn scroll_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 1).min(self.messages.len());
    }
}
```

## Day 4: File Transfer Protocol

### File Transfer Implementation
```rust
#[derive(Debug, Clone)]
pub struct FileTransfer {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub hash: String,
    pub chunks: Vec<FileChunk>,
    pub progress: f32,
    pub status: TransferStatus,
}

#[derive(Debug, Clone)]
pub enum TransferStatus {
    Pending,
    Transferring,
    Completed,
    Failed(String),
    Cancelled,
}

pub struct FileTransferManager {
    active_transfers: HashMap<String, FileTransfer>,
    download_dir: PathBuf,
    chunk_size: usize,
}

impl FileTransferManager {
    pub async fn send_file(&mut self, peer: PeerId, filepath: PathBuf) -> Result<String, TransferError> {
        let file = File::open(&filepath).await?;
        let metadata = file.metadata().await?;
        let filename = filepath.file_name().unwrap().to_string_lossy().to_string();
        
        let transfer = FileTransfer {
            id: Uuid::new_v4().to_string(),
            filename,
            size: metadata.len(),
            hash: calculate_file_hash(&filepath).await?,
            chunks: Vec::new(),
            progress: 0.0,
            status: TransferStatus::Pending,
        };
        
        // Send file offer to peer
        let offer = FileOffer {
            transfer_id: transfer.id.clone(),
            filename: transfer.filename.clone(),
            size: transfer.size,
            hash: transfer.hash.clone(),
        };
        
        self.network.send_message(peer, Message::FileOffer(offer)).await?;
        self.active_transfers.insert(transfer.id.clone(), transfer.clone());
        
        Ok(transfer.id)
    }
    
    pub async fn accept_file(&mut self, transfer_id: &str) -> Result<(), TransferError> {
        if let Some(transfer) = self.active_transfers.get_mut(transfer_id) {
            transfer.status = TransferStatus::Transferring;
            
            // Send acceptance and start receiving chunks
            let accept = FileAccept { transfer_id: transfer_id.to_string() };
            // Send to appropriate peer...
        }
        
        Ok(())
    }
    
    pub fn get_transfer_progress(&self, transfer_id: &str) -> Option<f32> {
        self.active_transfers.get(transfer_id).map(|t| t.progress)
    }
}
```

### File Transfer UI
```rust
pub struct FileTransferWidget {
    transfers: Vec<FileTransfer>,
}

impl FileTransferWidget {
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self.transfers
            .iter()
            .map(|transfer| {
                let progress_bar = format!(
                    "[{}{}] {:.1}%",
                    "=".repeat((transfer.progress * 20.0) as usize),
                    " ".repeat(20 - (transfer.progress * 20.0) as usize),
                    transfer.progress * 100.0
                );
                
                let line = Line::from(vec![
                    Span::raw(&transfer.filename),
                    Span::raw(" "),
                    Span::styled(progress_bar, Style::default().fg(Color::Green)),
                    Span::raw(" "),
                    Span::styled(format!("{:?}", transfer.status), Style::default().fg(Color::Yellow)),
                ]);
                
                ListItem::new(line)
            })
            .collect();
            
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("File Transfers"));
            
        Widget::render(list, area, buf);
    }
}
```

## Day 5: Configuration and Persistence

### Configuration System
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub ui: UIConfig,
    pub security: SecurityConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_port: u16,
    pub bootstrap_peers: Vec<String>,
    pub max_connections: usize,
    pub connection_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    pub theme: String,
    pub show_timestamps: bool,
    pub message_format: String,
    pub keybindings: HashMap<String, String>,
}

impl Config {
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
    
    pub fn save_to_file(&self, path: &Path) -> Result<(), ConfigError> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
    
    pub fn default() -> Self {
        Config {
            network: NetworkConfig {
                listen_port: 8080,
                bootstrap_peers: vec![],
                max_connections: 50,
                connection_timeout: Duration::from_secs(30),
            },
            ui: UIConfig {
                theme: "default".to_string(),
                show_timestamps: true,
                message_format: "[{timestamp}] {sender}: {message}".to_string(),
                keybindings: default_keybindings(),
            },
            // ... other defaults
        }
    }
}
```

### Message Persistence
```rust
pub struct MessageStore {
    db: Arc<Mutex<Connection>>,
}

impl MessageStore {
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                channel TEXT
            )",
            [],
        )?;
        
        Ok(MessageStore {
            db: Arc::new(Mutex::new(conn)),
        })
    }
    
    pub async fn save_message(&self, message: &ChatMessage) -> Result<(), StorageError> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO messages (id, sender, content, timestamp, channel) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id,
                message.sender.to_string(),
                serde_json::to_string(&message.content)?,
                message.timestamp.duration_since(UNIX_EPOCH)?.as_secs(),
                message.channel
            ],
        )?;
        Ok(())
    }
    
    pub async fn load_messages(&self, limit: usize) -> Result<Vec<ChatMessage>, StorageError> {
        let db = self.db.lock().await;
        let mut stmt = db.prepare(
            "SELECT id, sender, content, timestamp, channel FROM messages 
             ORDER BY timestamp DESC LIMIT ?1"
        )?;
        
        let rows = stmt.query_map([limit], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                sender: row.get::<_, String>(1)?.parse()?,
                content: serde_json::from_str(&row.get::<_, String>(2)?)?,
                timestamp: UNIX_EPOCH + Duration::from_secs(row.get(3)?),
                channel: row.get(4)?,
            })
        })?;
        
        rows.collect()
    }
}
```

### Application State Management
```rust
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub message_store: MessageStore,
    pub network: Arc<NetworkManager>,
    pub current_channel: Option<String>,
    pub input_history: VecDeque<String>,
    pub running: Arc<AtomicBool>,
}

impl AppState {
    pub async fn new(config_path: Option<PathBuf>) -> Result<Self, AppError> {
        let config_path = config_path.unwrap_or_else(|| {
            dirs::config_dir().unwrap().join("bitchat").join("config.toml")
        });
        
        let config = if config_path.exists() {
            Config::load_from_file(&config_path)?
        } else {
            let config = Config::default();
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            config.save_to_file(&config_path)?;
            config
        };
        
        let db_path = config_path.parent().unwrap().join("messages.db");
        let message_store = MessageStore::new(&db_path)?;
        let network = NetworkManager::new(&config.network).await?;
        
        Ok(AppState {
            config: Arc::new(RwLock::new(config)),
            message_store,
            network: Arc::new(network),
            current_channel: None,
            input_history: VecDeque::with_capacity(100),
            running: Arc::new(AtomicBool::new(true)),
        })
    }
    
    pub async fn shutdown(&self) -> Result<(), AppError> {
        self.running.store(false, Ordering::Relaxed);
        self.network.shutdown().await?;
        Ok(())
    }
}
```

## Integration Points

### Main Application Loop
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let app_state = AppState::new(cli.config.map(PathBuf::from)).await?;
    
    match cli.command {
        Commands::Chat => run_interactive_mode(app_state).await?,
        Commands::Connect { address } => {
            app_state.network.connect_peer(address).await?;
        }
        Commands::Send { peer, message } => {
            app_state.network.send_message(peer, message).await?;
        }
        Commands::Peers => {
            let peers = app_state.network.list_peers().await;
            for peer in peers {
                println!("{}", peer);
            }
        }
    }
    
    Ok(())
}

async fn run_interactive_mode(app_state: AppState) -> Result<(), AppError> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(app_state);
    
    while app.running {
        terminal.draw(|f| render_ui(f, &app))?;
        
        if let Event::Key(key) = event::read()? {
            app.handle_key_event(key).await?;
        }
    }
    
    restore_terminal(terminal)?;
    Ok(())
}
```

## Key Features Implemented

1. **CLI Interface**: Command-line argument parsing with clap
2. **TUI Components**: Terminal-based user interface with ratatui
3. **Command System**: IRC-style command processing with auto-completion
4. **Message Display**: Rich message formatting and chat history
5. **File Transfers**: P2P file sharing with progress tracking
6. **Configuration**: Persistent settings and user preferences
7. **Data Persistence**: SQLite-based message and state storage

## Dependencies Added
```toml
[dependencies]
clap = { version = "4.4", features = ["derive"] }
ratatui = "0.24"
crossterm = "0.27"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
rusqlite = { version = "0.30", features = ["bundled"] }
uuid = { version = "1.6", features = ["v4"] }
dirs = "5.0"
```

### Casino UI Components

```rust
// src/ui/casino.rs
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, 
        Tabs, Wrap, TableState,
    },
    Frame,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CasinoUI {
    pub current_view: CasinoView,
    pub active_games: Vec<GameSession>,
    pub wallet_balance: u64,
    pub bet_history: Vec<BetRecord>,
    pub game_statistics: GameStats,
    pub selected_bet_type: Option<BetType>,
    pub bet_amount: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CasinoView {
    GameLobby,
    ActiveGame,
    BettingInterface,
    GameHistory,
    WalletManager,
    Statistics,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct BetRecord {
    pub bet_id: String,
    pub game_id: String,
    pub bet_type: BetType,
    pub amount: u64,
    pub result: BetResult,
    pub payout: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum BetResult {
    Pending,
    Won,
    Lost,
    Push, // Tie
}

#[derive(Debug, Clone)]
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

    pub fn render<B: Backend>(&mut self, f: &mut Frame<B>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(0),     // Main content
                Constraint::Length(3),  // Footer/Status
            ])
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_main_content(f, chunks[1]);
        self.render_footer(f, chunks[2]);
    }

    fn render_header<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_main_content<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        match self.current_view {
            CasinoView::GameLobby => self.render_game_lobby(f, area),
            CasinoView::ActiveGame => self.render_active_game(f, area),
            CasinoView::BettingInterface => self.render_betting_interface(f, area),
            CasinoView::GameHistory => self.render_game_history(f, area),
            CasinoView::WalletManager => self.render_wallet_manager(f, area),
            CasinoView::Statistics => self.render_statistics(f, area),
        }
    }

    fn render_game_lobby<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_active_game<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_betting_area<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_players_and_bets<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_betting_interface<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        // Similar to render_betting_area but focused on bet placement
        self.render_betting_area(f, area);
    }

    fn render_wallet_manager<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_game_history<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_statistics<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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

    fn render_footer<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
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
```

### Wallet Interface Integration

```rust
// src/ui/wallet_interface.rs
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransaction {
    pub tx_id: String,
    pub tx_type: TransactionType,
    pub amount: u64,
    pub timestamp: u64,
    pub confirmations: u8,
    pub game_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    BetPlaced,
    BetWon,
    BetLost,
    GameEscrow,
    EscrowRelease,
}

pub struct WalletInterface {
    balance: u64,
    pending_balance: u64,
    transactions: Vec<WalletTransaction>,
    pending_bets: HashMap<String, u64>, // bet_id -> amount
}

impl WalletInterface {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: initial_balance,
            pending_balance: 0,
            transactions: Vec::new(),
            pending_bets: HashMap::new(),
        }
    }
    
    /// Place a bet, moving funds to pending
    pub fn place_bet(&mut self, bet_id: String, amount: u64) -> Result<(), WalletError> {
        if amount > self.balance {
            return Err(WalletError::InsufficientFunds);
        }
        
        self.balance -= amount;
        self.pending_balance += amount;
        self.pending_bets.insert(bet_id.clone(), amount);
        
        self.add_transaction(WalletTransaction {
            tx_id: format!("bet_{}", bet_id),
            tx_type: TransactionType::BetPlaced,
            amount,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            confirmations: 1,
            game_id: None,
        });
        
        Ok(())
    }
    
    /// Resolve a bet (win/lose/push)
    pub fn resolve_bet(&mut self, bet_id: &str, result: BetResult, payout: u64) -> Result<(), WalletError> {
        let bet_amount = self.pending_bets.remove(bet_id)
            .ok_or(WalletError::BetNotFound)?;
        
        self.pending_balance -= bet_amount;
        
        match result {
            BetResult::Won => {
                self.balance += payout;
                self.add_transaction(WalletTransaction {
                    tx_id: format!("win_{}", bet_id),
                    tx_type: TransactionType::BetWon,
                    amount: payout,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    confirmations: 1,
                    game_id: None,
                });
            }
            BetResult::Lost => {
                // Funds already deducted, just record the loss
                self.add_transaction(WalletTransaction {
                    tx_id: format!("loss_{}", bet_id),
                    tx_type: TransactionType::BetLost,
                    amount: bet_amount,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    confirmations: 1,
                    game_id: None,
                });
            }
            BetResult::Push => {
                // Return original bet amount
                self.balance += bet_amount;
            }
            BetResult::Pending => {
                // Put back in pending
                self.pending_balance += bet_amount;
                self.pending_bets.insert(bet_id.to_string(), bet_amount);
            }
        }
        
        Ok(())
    }
    
    pub fn get_available_balance(&self) -> u64 {
        self.balance
    }
    
    pub fn get_total_balance(&self) -> u64 {
        self.balance + self.pending_balance
    }
    
    fn add_transaction(&mut self, transaction: WalletTransaction) {
        self.transactions.push(transaction);
        
        // Keep only last 1000 transactions
        if self.transactions.len() > 1000 {
            self.transactions.remove(0);
        }
    }
}

#[derive(Debug)]
pub enum WalletError {
    InsufficientFunds,
    BetNotFound,
    TransactionFailed,
}
```

This completes the application layer implementation, providing a fully functional CLI and TUI interface for the BitChat P2P messaging system with comprehensive casino functionality, including specialized BitCraps game interface, wallet management, and betting systems.