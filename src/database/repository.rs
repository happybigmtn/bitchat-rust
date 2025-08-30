//! Database repository pattern for data access

use super::DatabasePool;
use crate::error::{Error, Result};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// User repository for user data access
pub struct UserRepository {
    pool: Arc<DatabasePool>,
}

impl UserRepository {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn create_user(&self, id: &str, username: &str, public_key: &[u8]) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO users (id, username, public_key, created_at, updated_at) 
                 VALUES (?, ?, ?, ?, ?)",
                    params![
                        id,
                        username,
                        public_key,
                        chrono::Utc::now().timestamp(),
                        chrono::Utc::now().timestamp(),
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to create user: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, username, public_key, reputation, created_at, updated_at 
                 FROM users WHERE id = ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let user = stmt
                    .query_row(params![id], |row| {
                        Ok(User {
                            id: row.get(0)?,
                            username: row.get(1)?,
                            public_key: row.get(2)?,
                            reputation: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    })
                    .optional()
                    .map_err(|e| Error::Database(e.to_string()))?;

                Ok(user)
            })
            .await
    }

    pub async fn update_reputation(&self, id: &str, reputation: f64) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE users SET reputation = ?, updated_at = ? WHERE id = ?",
                    params![reputation, chrono::Utc::now().timestamp(), id],
                )
                .map_err(|e| Error::Database(format!("Failed to update reputation: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn list_users(&self, limit: usize) -> Result<Vec<User>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, username, public_key, reputation, created_at, updated_at 
                 FROM users ORDER BY reputation DESC LIMIT ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let users = stmt
                    .query_map(params![limit], |row| {
                        Ok(User {
                            id: row.get(0)?,
                            username: row.get(1)?,
                            public_key: row.get(2)?,
                            reputation: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    })
                    .map_err(|e| Error::Database(e.to_string()))?;

                users
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::Database(e.to_string()))
            })
            .await
    }
}

/// Game repository for game data access
pub struct GameRepository {
    pool: Arc<DatabasePool>,
}

impl GameRepository {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn create_game(&self, game: &Game) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO games (id, state, pot_size, phase, created_at) 
                 VALUES (?, ?, ?, ?, ?)",
                    params![
                        &game.id,
                        serde_json::to_string(&game.state)
                            .map_err(|e| Error::Database(e.to_string()))?,
                        game.pot_size,
                        &game.phase,
                        game.created_at,
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to create game: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn update_game(&self, game: &Game) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE games SET state = ?, pot_size = ?, phase = ?, 
                 completed_at = ?, winner_id = ? WHERE id = ?",
                    params![
                        serde_json::to_string(&game.state)
                            .map_err(|e| Error::Database(e.to_string()))?,
                        game.pot_size,
                        &game.phase,
                        game.completed_at,
                        &game.winner_id,
                        &game.id,
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to update game: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn get_game(&self, id: &str) -> Result<Option<Game>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, state, pot_size, phase, created_at, completed_at, winner_id 
                 FROM games WHERE id = ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let game = stmt
                    .query_row(params![id], |row| {
                        let state_json: String = row.get(1)?;
                        Ok(Game {
                            id: row.get(0)?,
                            state: serde_json::from_str(&state_json).map_err(|e| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(e),
                                )
                            })?,
                            pot_size: row.get(2)?,
                            phase: row.get(3)?,
                            created_at: row.get(4)?,
                            completed_at: row.get(5)?,
                            winner_id: row.get(6)?,
                        })
                    })
                    .optional()
                    .map_err(|e| Error::Database(e.to_string()))?;

                Ok(game)
            })
            .await
    }

    pub async fn list_active_games(&self, limit: usize) -> Result<Vec<Game>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, state, pot_size, phase, created_at, completed_at, winner_id 
                 FROM games WHERE completed_at IS NULL 
                 ORDER BY created_at DESC LIMIT ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let games = stmt
                    .query_map(params![limit], |row| {
                        let state_json: String = row.get(1)?;
                        Ok(Game {
                            id: row.get(0)?,
                            state: serde_json::from_str(&state_json).map_err(|e| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(e),
                                )
                            })?,
                            pot_size: row.get(2)?,
                            phase: row.get(3)?,
                            created_at: row.get(4)?,
                            completed_at: row.get(5)?,
                            winner_id: row.get(6)?,
                        })
                    })
                    .map_err(|e| Error::Database(e.to_string()))?;

                games
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::Database(e.to_string()))
            })
            .await
    }
}

/// Transaction repository for transaction data access
pub struct TransactionRepository {
    pool: Arc<DatabasePool>,
}

impl TransactionRepository {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn create_transaction(&self, tx: &Transaction) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO transactions 
                 (id, from_user_id, to_user_id, amount, transaction_type, status, created_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                    params![
                        &tx.id,
                        &tx.from_user_id,
                        &tx.to_user_id,
                        tx.amount,
                        &tx.transaction_type,
                        &tx.status,
                        tx.created_at,
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to create transaction: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn update_transaction_status(&self, id: &str, status: &str) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE transactions SET status = ?, confirmed_at = ? WHERE id = ?",
                    params![status, chrono::Utc::now().timestamp(), id],
                )
                .map_err(|e| Error::Database(format!("Failed to update transaction: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn get_user_transactions(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<Transaction>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, from_user_id, to_user_id, amount, transaction_type, 
                 status, created_at, confirmed_at 
                 FROM transactions 
                 WHERE from_user_id = ? OR to_user_id = ? 
                 ORDER BY created_at DESC LIMIT ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let txs = stmt
                    .query_map(params![user_id, user_id, limit], |row| {
                        Ok(Transaction {
                            id: row.get(0)?,
                            from_user_id: row.get(1)?,
                            to_user_id: row.get(2)?,
                            amount: row.get(3)?,
                            transaction_type: row.get(4)?,
                            status: row.get(5)?,
                            created_at: row.get(6)?,
                            confirmed_at: row.get(7)?,
                        })
                    })
                    .map_err(|e| Error::Database(e.to_string()))?;

                txs.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::Database(e.to_string()))
            })
            .await
    }

    pub async fn get_balance(&self, user_id: &str) -> Result<i64> {
        self.pool
            .with_connection(|conn| {
                // Calculate balance from transactions
                let received: i64 = conn
                    .query_row(
                        "SELECT COALESCE(SUM(amount), 0) FROM transactions 
                 WHERE to_user_id = ? AND status = 'confirmed'",
                        params![user_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let sent: i64 = conn
                    .query_row(
                        "SELECT COALESCE(SUM(amount), 0) FROM transactions 
                 WHERE from_user_id = ? AND status = 'confirmed'",
                        params![user_id],
                        |row| row.get(0),
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                Ok(received - sent)
            })
            .await
    }
}

/// Statistics repository for analytics
pub struct StatsRepository {
    pool: Arc<DatabasePool>,
}

impl StatsRepository {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    pub async fn update_game_stats(&self, game_id: &str, stats: &GameStats) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO game_statistics 
                 (game_id, total_bets, total_wagered, total_won, house_edge, 
                  duration_seconds, player_count, max_pot_size, created_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        game_id,
                        stats.total_bets,
                        stats.total_wagered,
                        stats.total_won,
                        stats.house_edge,
                        stats.duration_seconds,
                        stats.player_count,
                        stats.max_pot_size,
                        chrono::Utc::now().timestamp(),
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to update game stats: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn update_player_stats(&self, player_id: &str, stats: &PlayerStats) -> Result<()> {
        self.pool
            .with_connection(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO player_statistics 
                 (player_id, games_played, games_won, total_wagered, total_won, 
                  win_rate, avg_bet_size, biggest_win, longest_streak, updated_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        player_id,
                        stats.games_played,
                        stats.games_won,
                        stats.total_wagered,
                        stats.total_won,
                        stats.win_rate,
                        stats.avg_bet_size,
                        stats.biggest_win,
                        stats.longest_streak,
                        chrono::Utc::now().timestamp(),
                    ],
                )
                .map_err(|e| Error::Database(format!("Failed to update player stats: {}", e)))?;
                Ok(())
            })
            .await
    }

    pub async fn get_player_stats(&self, player_id: &str) -> Result<Option<PlayerStats>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT games_played, games_won, total_wagered, total_won, 
                 win_rate, avg_bet_size, biggest_win, longest_streak 
                 FROM player_statistics WHERE player_id = ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let stats = stmt
                    .query_row(params![player_id], |row| {
                        Ok(PlayerStats {
                            games_played: row.get(0)?,
                            games_won: row.get(1)?,
                            total_wagered: row.get(2)?,
                            total_won: row.get(3)?,
                            win_rate: row.get(4)?,
                            avg_bet_size: row.get(5)?,
                            biggest_win: row.get(6)?,
                            longest_streak: row.get(7)?,
                        })
                    })
                    .optional()
                    .map_err(|e| Error::Database(e.to_string()))?;

                Ok(stats)
            })
            .await
    }

    pub async fn get_leaderboard(&self, limit: usize) -> Result<Vec<LeaderboardEntry>> {
        self.pool
            .with_connection(|conn| {
                let mut stmt = conn
                    .prepare(
                        "SELECT u.username, p.games_won, p.total_won, p.win_rate 
                 FROM player_statistics p 
                 JOIN users u ON p.player_id = u.id 
                 ORDER BY p.total_won DESC LIMIT ?",
                    )
                    .map_err(|e| Error::Database(e.to_string()))?;

                let entries = stmt
                    .query_map(params![limit], |row| {
                        Ok(LeaderboardEntry {
                            username: row.get(0)?,
                            games_won: row.get(1)?,
                            total_won: row.get(2)?,
                            win_rate: row.get(3)?,
                        })
                    })
                    .map_err(|e| Error::Database(e.to_string()))?;

                entries
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::Database(e.to_string()))
            })
            .await
    }
}

// Data models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub public_key: Vec<u8>,
    pub reputation: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub state: serde_json::Value,
    pub pot_size: i64,
    pub phase: String,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub winner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub from_user_id: Option<String>,
    pub to_user_id: Option<String>,
    pub amount: i64,
    pub transaction_type: String,
    pub status: String,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub total_bets: i32,
    pub total_wagered: i64,
    pub total_won: i64,
    pub house_edge: f64,
    pub duration_seconds: i32,
    pub player_count: i32,
    pub max_pot_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStats {
    pub games_played: i32,
    pub games_won: i32,
    pub total_wagered: i64,
    pub total_won: i64,
    pub win_rate: f64,
    pub avg_bet_size: i64,
    pub biggest_win: i64,
    pub longest_streak: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub username: String,
    pub games_won: i32,
    pub total_won: i64,
    pub win_rate: f64,
}
