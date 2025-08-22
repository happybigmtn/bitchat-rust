//! Events module for BitCraps UI
//! 
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use serde::{Serialize, Deserialize};
use rusqlite::params;
use std::fs;
use std::collections::HashMap;
use std::time::{Duration, UNIX_EPOCH};
use std::path::Path;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::widgets::ChatMessage;

// Add missing error types
#[derive(Debug)]
pub struct ConfigError {
    pub message: String,
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError { message: err.to_string() }
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError { message: err.to_string() }
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(err: toml::ser::Error) -> Self {
        ConfigError { message: err.to_string() }
    }
}

#[derive(Debug)]
pub struct StorageError {
    pub message: String,
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        StorageError { message: err.to_string() }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        StorageError { message: err.to_string() }
    }
}

impl From<std::time::SystemTimeError> for StorageError {
    fn from(err: std::time::SystemTimeError) -> Self {
        StorageError { message: err.to_string() }
    }
}

// Add missing keybinding types
pub type Keybindings = HashMap<String, String>;

pub fn default_keybindings() -> Keybindings {
    let mut bindings = HashMap::new();
    bindings.insert("quit".to_string(), "q".to_string());
    bindings.insert("help".to_string(), "h".to_string());
    bindings.insert("casino".to_string(), "c".to_string());
    bindings.insert("chat".to_string(), "t".to_string());
    bindings
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub encrypt_storage: bool,
    pub auto_accept_files: bool,
    pub trusted_peers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub max_log_size: u64,
    pub auto_cleanup: bool,
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
            security: SecurityConfig {
                encrypt_storage: true,
                auto_accept_files: false,
                trusted_peers: vec![],
            },
            storage: StorageConfig {
                data_dir: "~/.bitcraps".to_string(),
                max_log_size: 100_000_000, // 100MB
                auto_cleanup: true,
            },
        }
    }
}

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
                format!("{:?}", message.sender),
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
            let sender_str: String = row.get(1)?;
            let sender = sender_str.as_bytes().try_into().map_err(|_| {
                rusqlite::Error::InvalidColumnType(1, "Invalid PeerId format".to_string(), rusqlite::types::Type::Text)
            })?;
            
            Ok(ChatMessage {
                id: row.get(0)?,
                sender,
                content: serde_json::from_str(&row.get::<_, String>(2)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(2, e.to_string(), rusqlite::types::Type::Text))?,
                timestamp: UNIX_EPOCH + Duration::from_secs(row.get(3)?),
                channel: row.get(4)?,
            })
        })?;
        
        rows.collect::<Result<Vec<_>, _>>().map_err(StorageError::from)
    }
}

