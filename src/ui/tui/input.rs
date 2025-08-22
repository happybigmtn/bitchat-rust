//! Input module for BitCraps UI
//! 
//! This module implements input handling for the BitCraps casino TUI,
//! including keyboard navigation, bet placement, and command processing.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::path::PathBuf;
use std::fs;
use tokio::sync::RwLock;
use super::events::{Config, MessageStore};
use super::widgets::AutoComplete;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event};
use ratatui::Terminal;
use dirs;
use crate::protocol::BetType;

// Add missing NetworkManager
pub struct NetworkManager;

impl NetworkManager {
    pub async fn new(_config: &Config) -> Result<Self, NetworkError> {
        Ok(NetworkManager)
    }
    
    pub async fn send_message(&self, _peer: String, _message: String) -> Result<(), NetworkError> {
        Ok(())
    }
    
    pub async fn connect_peer(&self, _address: String) -> Result<(), NetworkError> {
        Ok(())
    }
    
    pub async fn list_peers(&self) -> Vec<String> {
        vec![]
    }
    
    pub async fn shutdown(&self) -> Result<(), NetworkError> {
        Ok(())
    }
    
    pub async fn join_channel(&self, _channel: &str) -> Result<(), NetworkError> {
        Ok(())
    }
    
    pub async fn leave_channel(&self, _channel: &str) -> Result<(), NetworkError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct NetworkError {
    pub message: String,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NetworkError {}

#[derive(Debug)]
pub struct AppError {
    pub message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError { message: err.to_string() }
    }
}

impl From<super::events::ConfigError> for AppError {
    fn from(err: super::events::ConfigError) -> Self {
        AppError { message: err.message }
    }
}

impl From<super::events::StorageError> for AppError {
    fn from(err: super::events::StorageError) -> Self {
        AppError { message: err.message }
    }
}

impl From<NetworkError> for AppError {
    fn from(err: NetworkError) -> Self {
        AppError { message: err.message }
    }
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    #[arg(short, long)]
    pub config: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Chat,
    Connect { address: String },
    Send { peer: String, message: String },
    Peers,
}

/// Input state for the TUI application
#[derive(Debug, Clone)]
pub struct InputState {
    pub text: String,
    pub cursor_position: usize,
    pub history: VecDeque<String>,
    pub history_index: Option<usize>,
    pub auto_complete: AutoComplete,
    pub completion_candidates: Vec<String>,
    pub completion_index: Option<usize>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_position: 0,
            history: VecDeque::with_capacity(100),
            history_index: None,
            auto_complete: AutoComplete::new(),
            completion_candidates: Vec::new(),
            completion_index: None,
        }
    }
    
    pub fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor_position, c);
        self.cursor_position += 1;
        self.clear_completion();
    }
    
    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.text.remove(self.cursor_position);
            self.clear_completion();
        }
    }
    
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }
    
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.text.len() {
            self.cursor_position += 1;
        }
    }
    
    pub fn move_to_start(&mut self) {
        self.cursor_position = 0;
    }
    
    pub fn move_to_end(&mut self) {
        self.cursor_position = self.text.len();
    }
    
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
        self.clear_completion();
        self.history_index = None;
    }
    
    pub fn submit(&mut self) -> String {
        let text = self.text.clone();
        if !text.trim().is_empty() {
            self.add_to_history(text.clone());
        }
        self.clear();
        text
    }
    
    fn add_to_history(&mut self, text: String) {
        // Don't add duplicate consecutive entries
        if self.history.front() != Some(&text) {
            self.history.push_front(text);
            if self.history.len() > 100 {
                self.history.pop_back();
            }
        }
    }
    
    pub fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        
        let new_index = match self.history_index {
            None => Some(0),
            Some(i) if i < self.history.len() - 1 => Some(i + 1),
            Some(i) => Some(i),
        };
        
        if let Some(index) = new_index {
            if let Some(text) = self.history.get(index) {
                self.text = text.clone();
                self.cursor_position = self.text.len();
                self.history_index = Some(index);
                self.clear_completion();
            }
        }
    }
    
    pub fn history_next(&mut self) {
        match self.history_index {
            None => {},
            Some(0) => {
                self.clear();
            },
            Some(i) => {
                let new_index = i - 1;
                if let Some(text) = self.history.get(new_index) {
                    self.text = text.clone();
                    self.cursor_position = self.text.len();
                    self.history_index = Some(new_index);
                    self.clear_completion();
                }
            }
        }
    }
    
    pub fn start_completion(&mut self) {
        let current_word = self.get_current_word();
        self.completion_candidates = self.auto_complete.complete(&current_word);
        if !self.completion_candidates.is_empty() {
            self.completion_index = Some(0);
        }
    }
    
    pub fn next_completion(&mut self) {
        if let Some(index) = self.completion_index {
            let new_index = (index + 1) % self.completion_candidates.len();
            self.completion_index = Some(new_index);
            self.apply_completion();
        }
    }
    
    pub fn prev_completion(&mut self) {
        if let Some(index) = self.completion_index {
            let new_index = if index == 0 {
                self.completion_candidates.len() - 1
            } else {
                index - 1
            };
            self.completion_index = Some(new_index);
            self.apply_completion();
        }
    }
    
    fn apply_completion(&mut self) {
        if let (Some(_index), Some(completion)) = (self.completion_index, self.completion_candidates.get(self.completion_index.unwrap_or(0))) {
            let current_word = self.get_current_word();
            let word_start = self.cursor_position.saturating_sub(current_word.len());
            
            // Replace the current word with the completion
            self.text.replace_range(word_start..self.cursor_position, completion);
            self.cursor_position = word_start + completion.len();
        }
    }
    
    fn get_current_word(&self) -> String {
        let text_before_cursor = &self.text[..self.cursor_position];
        text_before_cursor
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_string()
    }
    
    fn clear_completion(&mut self) {
        self.completion_candidates.clear();
        self.completion_index = None;
    }
}

/// Input handler for casino-specific commands
#[derive(Debug, Clone)]
pub struct CasinoInputHandler {
    pub input_state: InputState,
    pub bet_amount: u64,
    pub selected_bet_type: Option<BetType>,
    pub quick_bet_amounts: Vec<u64>,
    pub quick_bet_index: usize,
}

impl CasinoInputHandler {
    pub fn new() -> Self {
        Self {
            input_state: InputState::new(),
            bet_amount: 50,
            selected_bet_type: Some(BetType::Pass),
            quick_bet_amounts: vec![10, 25, 50, 100, 250, 500],
            quick_bet_index: 2, // Default to 50
        }
    }
    
    pub fn handle_key(&mut self, key: KeyEvent) -> CasinoInputResult {
        match key.modifiers {
            KeyModifiers::CONTROL => self.handle_ctrl_key(key.code),
            KeyModifiers::ALT => self.handle_alt_key(key.code),
            _ => self.handle_normal_key(key.code),
        }
    }
    
    fn handle_normal_key(&mut self, key: KeyCode) -> CasinoInputResult {
        match key {
            KeyCode::Char(c) => {
                self.input_state.insert_char(c);
                CasinoInputResult::None
            },
            KeyCode::Backspace => {
                self.input_state.delete_char();
                CasinoInputResult::None
            },
            KeyCode::Enter => {
                let input = self.input_state.submit();
                self.process_input(input)
            },
            KeyCode::Up => {
                self.input_state.history_prev();
                CasinoInputResult::None
            },
            KeyCode::Down => {
                self.input_state.history_next();
                CasinoInputResult::None
            },
            KeyCode::Left => {
                self.input_state.move_cursor_left();
                CasinoInputResult::None
            },
            KeyCode::Right => {
                self.input_state.move_cursor_right();
                CasinoInputResult::None
            },
            KeyCode::Home => {
                self.input_state.move_to_start();
                CasinoInputResult::None
            },
            KeyCode::End => {
                self.input_state.move_to_end();
                CasinoInputResult::None
            },
            KeyCode::Tab => {
                if self.input_state.completion_candidates.is_empty() {
                    self.input_state.start_completion();
                } else {
                    self.input_state.next_completion();
                }
                CasinoInputResult::None
            },
            KeyCode::F(1) => {
                self.cycle_bet_type(true);
                CasinoInputResult::BetTypeChanged(self.selected_bet_type)
            },
            KeyCode::F(2) => {
                self.cycle_bet_type(false);
                CasinoInputResult::BetTypeChanged(self.selected_bet_type)
            },
            KeyCode::F(3) => {
                self.cycle_bet_amount(true);
                CasinoInputResult::BetAmountChanged(self.bet_amount)
            },
            KeyCode::F(4) => {
                self.cycle_bet_amount(false);
                CasinoInputResult::BetAmountChanged(self.bet_amount)
            },
            _ => CasinoInputResult::None,
        }
    }
    
    fn handle_ctrl_key(&mut self, key: KeyCode) -> CasinoInputResult {
        match key {
            KeyCode::Char('a') => {
                self.input_state.move_to_start();
                CasinoInputResult::None
            },
            KeyCode::Char('e') => {
                self.input_state.move_to_end();
                CasinoInputResult::None
            },
            KeyCode::Char('k') => {
                // Kill to end of line
                let pos = self.input_state.cursor_position;
                self.input_state.text.truncate(pos);
                CasinoInputResult::None
            },
            KeyCode::Char('u') => {
                // Kill entire line
                self.input_state.clear();
                CasinoInputResult::None
            },
            KeyCode::Char('w') => {
                // Kill word backward
                self.kill_word_backward();
                CasinoInputResult::None
            },
            KeyCode::Char('r') => {
                // Quick roll command
                CasinoInputResult::Command(CasinoCommand::Roll)
            },
            KeyCode::Char('b') => {
                // Quick bet command
                if let Some(bet_type) = self.selected_bet_type {
                    CasinoInputResult::Command(CasinoCommand::Bet(bet_type, self.bet_amount))
                } else {
                    CasinoInputResult::None
                }
            },
            _ => CasinoInputResult::None,
        }
    }
    
    fn handle_alt_key(&mut self, key: KeyCode) -> CasinoInputResult {
        match key {
            KeyCode::Char('1') => self.quick_bet(0),
            KeyCode::Char('2') => self.quick_bet(1),
            KeyCode::Char('3') => self.quick_bet(2),
            KeyCode::Char('4') => self.quick_bet(3),
            KeyCode::Char('5') => self.quick_bet(4),
            KeyCode::Char('6') => self.quick_bet(5),
            _ => CasinoInputResult::None,
        }
    }
    
    fn process_input(&mut self, input: String) -> CasinoInputResult {
        let input = input.trim();
        
        if input.is_empty() {
            return CasinoInputResult::None;
        }
        
        // Try to parse as command
        if let Ok(command) = self.parse_casino_command(input) {
            return CasinoInputResult::Command(command);
        }
        
        // Try to parse as chat message
        CasinoInputResult::ChatMessage(input.to_string())
    }
    
    fn parse_casino_command(&self, input: &str) -> Result<CasinoCommand, String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts.first().ok_or("Empty command")?;
        
        match *command {
            "roll" | "r" => Ok(CasinoCommand::Roll),
            "bet" | "b" => {
                if parts.len() < 3 {
                    return Err("Usage: bet <type> <amount>".to_string());
                }
                
                let bet_type = self.parse_bet_type(parts[1])?;
                let amount = parts[2].parse::<u64>()
                    .map_err(|_| "Invalid amount".to_string())?;
                
                Ok(CasinoCommand::Bet(bet_type, amount))
            },
            "balance" | "bal" => Ok(CasinoCommand::ShowBalance),
            "history" | "hist" => Ok(CasinoCommand::ShowHistory),
            "help" | "h" => Ok(CasinoCommand::ShowHelp),
            _ => Err(format!("Unknown command: {}", command)),
        }
    }
    
    fn parse_bet_type(&self, type_str: &str) -> Result<BetType, String> {
        match type_str.to_lowercase().as_str() {
            "pass" | "p" => Ok(BetType::Pass),
            "dontpass" | "dp" => Ok(BetType::DontPass),
            "come" | "c" => Ok(BetType::Come),
            "dontcome" | "dc" => Ok(BetType::DontCome),
            "field" | "f" => Ok(BetType::Field),
            "hard4" | "h4" => Ok(BetType::Hard4),
            "hard6" | "h6" => Ok(BetType::Hard6),
            "hard8" | "h8" => Ok(BetType::Hard8),
            "hard10" | "h10" => Ok(BetType::Hard10),
            "any7" | "7" => Ok(BetType::Next7),
            "any11" | "11" => Ok(BetType::Next11),
            "snake" | "2" => Ok(BetType::Next2),
            "box" | "12" => Ok(BetType::Next12),
            _ => Err(format!("Unknown bet type: {}", type_str)),
        }
    }
    
    fn cycle_bet_type(&mut self, forward: bool) {
        let bet_types = [
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
            if let Some(pos) = bet_types.iter().position(|&x| x == current) {
                let new_pos = if forward {
                    (pos + 1) % bet_types.len()
                } else {
                    if pos == 0 { bet_types.len() - 1 } else { pos - 1 }
                };
                self.selected_bet_type = Some(bet_types[new_pos]);
            }
        } else {
            self.selected_bet_type = Some(bet_types[0]);
        }
    }
    
    fn cycle_bet_amount(&mut self, forward: bool) {
        if forward {
            self.quick_bet_index = (self.quick_bet_index + 1) % self.quick_bet_amounts.len();
        } else {
            self.quick_bet_index = if self.quick_bet_index == 0 {
                self.quick_bet_amounts.len() - 1
            } else {
                self.quick_bet_index - 1
            };
        }
        self.bet_amount = self.quick_bet_amounts[self.quick_bet_index];
    }
    
    fn quick_bet(&mut self, index: usize) -> CasinoInputResult {
        if index < self.quick_bet_amounts.len() {
            self.bet_amount = self.quick_bet_amounts[index];
            self.quick_bet_index = index;
            CasinoInputResult::BetAmountChanged(self.bet_amount)
        } else {
            CasinoInputResult::None
        }
    }
    
    fn kill_word_backward(&mut self) {
        let pos = self.input_state.cursor_position;
        let text_before = &self.input_state.text[..pos];
        
        // Find the start of the current word
        let word_start = text_before
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        
        // Remove the word
        self.input_state.text.replace_range(word_start..pos, "");
        self.input_state.cursor_position = word_start;
    }
}

/// Result type for casino input processing
#[derive(Debug, Clone)]
pub enum CasinoInputResult {
    None,
    Command(CasinoCommand),
    ChatMessage(String),
    BetTypeChanged(Option<BetType>),
    BetAmountChanged(u64),
}

/// Casino-specific commands
#[derive(Debug, Clone)]
pub enum CasinoCommand {
    Roll,
    Bet(BetType, u64),
    ShowBalance,
    ShowHistory,
    ShowHelp,
}

pub struct App {
    pub running: bool,
    pub casino_input: CasinoInputHandler,
}

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
            dirs::config_dir()
                .unwrap_or_else(|| std::env::temp_dir()) // Fallback to temp dir
                .join("bitchat")
                .join("config.toml")
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
        
        let db_path = config_path.parent()
            .unwrap_or_else(|| std::path::Path::new("/tmp")) // Fallback to /tmp
            .join("messages.db");
        let message_store = MessageStore::new(&db_path)?;
        let network = NetworkManager::new(&config).await?;
        
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

#[tokio::main]
#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn setup_terminal() -> Result<Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, AppError> {
    use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
    use crossterm::execute;
    
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

#[allow(dead_code)]
fn restore_terminal(mut terminal: Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<(), AppError> {
    use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
    use crossterm::execute;
    
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[allow(dead_code)]
fn render_ui(f: &mut ratatui::Frame, _app: &App) {
    // Placeholder UI rendering
    use ratatui::widgets::{Block, Borders, Paragraph};
    let block = Block::default().title("BitCraps TUI").borders(Borders::ALL);
    let paragraph = Paragraph::new("Welcome to BitCraps!").block(block);
    f.render_widget(paragraph, f.area());
}

impl App {
    pub fn new(_app_state: AppState) -> Self {
        App { 
            running: true,
            casino_input: CasinoInputHandler::new(),
        }
    }
    
    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Result<CasinoInputResult, AppError> {
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
                Ok(CasinoInputResult::None)
            },
            KeyCode::Esc => {
                self.running = false;
                Ok(CasinoInputResult::None)
            },
            _ => {
                let result = self.casino_input.handle_key(key);
                Ok(result)
            }
        }
    }
    
    pub fn get_input_text(&self) -> &str {
        &self.casino_input.input_state.text
    }
    
    pub fn get_cursor_position(&self) -> usize {
        self.casino_input.input_state.cursor_position
    }
    
    pub fn get_selected_bet_type(&self) -> Option<BetType> {
        self.casino_input.selected_bet_type
    }
    
    pub fn get_bet_amount(&self) -> u64 {
        self.casino_input.bet_amount
    }
}

