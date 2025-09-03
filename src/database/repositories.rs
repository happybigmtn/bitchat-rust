//! Database repository implementations for BitCraps entities
//!
//! Provides high-level data access patterns for:
//! - Users and authentication
//! - Games and game state
//! - Bets and transactions  
//! - Consensus messages
//! - System metrics

use crate::database::abstractions::*;
use crate::database::database_manager::DatabaseManager;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// User entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: Vec<u8>,
    pub salt: Vec<u8>,
    pub public_key: Vec<u8>,
    pub reputation_score: i32,
    pub total_games_played: i64,
    pub total_winnings: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_active: i64,
    pub account_status: String,
    pub kyc_status: String,
    pub preferences: String, // JSON string
}

/// Game entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub game_type: String,
    pub status: String,
    pub creator_id: String,
    pub max_players: i32,
    pub current_players: i32,
    pub min_bet: i64,
    pub max_bet: i64,
    pub house_edge: f64,
    pub total_pot: i64,
    pub game_state: String, // JSON string
    pub consensus_state: String, // JSON string
    pub dice_results: String, // JSON array
    pub round_number: i32,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub updated_at: i64,
}

/// Bet entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub id: String,
    pub game_id: String,
    pub user_id: String,
    pub participant_id: String,
    pub bet_type: String,
    pub bet_amount: i64,
    pub potential_payout: i64,
    pub actual_payout: i64,
    pub odds_numerator: i32,
    pub odds_denominator: i32,
    pub round_number: i32,
    pub placed_at: i64,
    pub resolved_at: Option<i64>,
    pub status: String,
    pub bet_data: String, // JSON string
}

/// Transaction entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub user_id: String,
    pub transaction_type: String,
    pub amount: i64,
    pub balance_before: i64,
    pub balance_after: i64,
    pub game_id: Option<String>,
    pub bet_id: Option<String>,
    pub reference_id: Option<String>,
    pub description: Option<String>,
    pub metadata: String, // JSON string
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
    pub status: String,
}

/// User repository implementation
pub struct UserRepository {
    db: Arc<DatabaseManager>,
}

impl UserRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Create a new user
    pub async fn create_user(&self, user: &User) -> Result<()> {
        let sql = r#"
            INSERT INTO users (
                id, username, email, password_hash, salt, public_key,
                reputation_score, total_games_played, total_winnings,
                created_at, updated_at, last_active, account_status, kyc_status, preferences
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        self.db.execute(sql, &[
            &user.id as &dyn SqlParameter,
            &user.username,
            &user.email,
            &user.password_hash,
            &user.salt,
            &user.public_key,
            &user.reputation_score,
            &user.total_games_played,
            &user.total_winnings,
            &user.created_at,
            &user.updated_at,
            &user.last_active,
            &user.account_status,
            &user.kyc_status,
            &user.preferences,
        ]).await?;
        
        Ok(())
    }
    
    /// Find user by ID
    pub async fn find_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let result = self.db.query(
            "SELECT * FROM users WHERE id = ?",
            &[&user_id.to_string() as &dyn SqlParameter]
        ).await?;
        
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let row = &result.rows[0];
            Ok(Some(User {
                id: row.get("id")?,
                username: row.get("username")?,
                email: row.get_opt("email")?,
                password_hash: row.get("password_hash")?,
                salt: row.get("salt")?,
                public_key: row.get("public_key")?,
                reputation_score: row.get("reputation_score")?,
                total_games_played: row.get("total_games_played")?,
                total_winnings: row.get("total_winnings")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                last_active: row.get("last_active")?,
                account_status: row.get("account_status")?,
                kyc_status: row.get("kyc_status")?,
                preferences: row.get("preferences")?,
            }))
        }
    }
    
    /// Find user by username
    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let result = self.db.query(
            "SELECT * FROM users WHERE username = ?",
            &[&username.to_string() as &dyn SqlParameter]
        ).await?;
        
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let row = &result.rows[0];
            Ok(Some(User {
                id: row.get("id")?,
                username: row.get("username")?,
                email: row.get_opt("email")?,
                password_hash: row.get("password_hash")?,
                salt: row.get("salt")?,
                public_key: row.get("public_key")?,
                reputation_score: row.get("reputation_score")?,
                total_games_played: row.get("total_games_played")?,
                total_winnings: row.get("total_winnings")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                last_active: row.get("last_active")?,
                account_status: row.get("account_status")?,
                kyc_status: row.get("kyc_status")?,
                preferences: row.get("preferences")?,
            }))
        }
    }
    
    /// Update user
    pub async fn update_user(&self, user: &User) -> Result<()> {
        let sql = r#"
            UPDATE users SET
                username = ?, email = ?, reputation_score = ?,
                total_games_played = ?, total_winnings = ?,
                updated_at = ?, last_active = ?, account_status = ?,
                kyc_status = ?, preferences = ?
            WHERE id = ?
        "#;
        
        self.db.execute(sql, &[
            &user.username as &dyn SqlParameter,
            &user.email,
            &user.reputation_score,
            &user.total_games_played,
            &user.total_winnings,
            &user.updated_at,
            &user.last_active,
            &user.account_status,
            &user.kyc_status,
            &user.preferences,
            &user.id,
        ]).await?;
        
        Ok(())
    }
    
    /// Get top users by reputation
    pub async fn get_top_by_reputation(&self, limit: i32) -> Result<Vec<User>> {
        let result = self.db.query(
            "SELECT * FROM users WHERE account_status = 'active' ORDER BY reputation_score DESC LIMIT ?",
            &[&limit as &dyn SqlParameter]
        ).await?;
        
        let mut users = Vec::new();
        for row in result.rows {
            users.push(User {
                id: row.get("id")?,
                username: row.get("username")?,
                email: row.get_opt("email")?,
                password_hash: row.get("password_hash")?,
                salt: row.get("salt")?,
                public_key: row.get("public_key")?,
                reputation_score: row.get("reputation_score")?,
                total_games_played: row.get("total_games_played")?,
                total_winnings: row.get("total_winnings")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
                last_active: row.get("last_active")?,
                account_status: row.get("account_status")?,
                kyc_status: row.get("kyc_status")?,
                preferences: row.get("preferences")?,
            });
        }
        
        Ok(users)
    }
}

/// Game repository implementation
pub struct GameRepository {
    db: Arc<DatabaseManager>,
}

impl GameRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Create a new game
    pub async fn create_game(&self, game: &Game) -> Result<()> {
        let sql = r#"
            INSERT INTO games (
                id, game_type, status, creator_id, max_players, current_players,
                min_bet, max_bet, house_edge, total_pot, game_state, consensus_state,
                dice_results, round_number, created_at, started_at, completed_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        self.db.execute(sql, &[
            &game.id as &dyn SqlParameter,
            &game.game_type,
            &game.status,
            &game.creator_id,
            &game.max_players,
            &game.current_players,
            &game.min_bet,
            &game.max_bet,
            &game.house_edge,
            &game.total_pot,
            &game.game_state,
            &game.consensus_state,
            &game.dice_results,
            &game.round_number,
            &game.created_at,
            &game.started_at,
            &game.completed_at,
            &game.updated_at,
        ]).await?;
        
        Ok(())
    }
    
    /// Find game by ID
    pub async fn find_by_id(&self, game_id: &str) -> Result<Option<Game>> {
        let result = self.db.query(
            "SELECT * FROM games WHERE id = ?",
            &[&game_id.to_string() as &dyn SqlParameter]
        ).await?;
        
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let row = &result.rows[0];
            Ok(Some(self.row_to_game(&row)?))
        }
    }
    
    /// Get active games (waiting or in progress)
    pub async fn get_active_games(&self, limit: i32) -> Result<Vec<Game>> {
        let result = self.db.query(
            "SELECT * FROM games WHERE status IN ('waiting', 'active') ORDER BY created_at DESC LIMIT ?",
            &[&limit as &dyn SqlParameter]
        ).await?;
        
        let mut games = Vec::new();
        for row in result.rows {
            games.push(self.row_to_game(&row)?);
        }
        
        Ok(games)
    }
    
    /// Get games waiting for players
    pub async fn get_waiting_games(&self, limit: i32) -> Result<Vec<Game>> {
        let result = self.db.query(
            "SELECT * FROM games WHERE status = 'waiting' AND current_players < max_players ORDER BY created_at ASC LIMIT ?",
            &[&limit as &dyn SqlParameter]
        ).await?;
        
        let mut games = Vec::new();
        for row in result.rows {
            games.push(self.row_to_game(&row)?);
        }
        
        Ok(games)
    }
    
    /// Update game
    pub async fn update_game(&self, game: &Game) -> Result<()> {
        let sql = r#"
            UPDATE games SET
                status = ?, current_players = ?, total_pot = ?, game_state = ?,
                consensus_state = ?, dice_results = ?, round_number = ?,
                started_at = ?, completed_at = ?, updated_at = ?
            WHERE id = ?
        "#;
        
        self.db.execute(sql, &[
            &game.status as &dyn SqlParameter,
            &game.current_players,
            &game.total_pot,
            &game.game_state,
            &game.consensus_state,
            &game.dice_results,
            &game.round_number,
            &game.started_at,
            &game.completed_at,
            &game.updated_at,
            &game.id,
        ]).await?;
        
        Ok(())
    }
    
    /// Get user's game history
    pub async fn get_user_games(&self, user_id: &str, limit: i32) -> Result<Vec<Game>> {
        let sql = r#"
            SELECT DISTINCT g.* FROM games g
            JOIN game_participants gp ON g.id = gp.game_id
            WHERE gp.user_id = ?
            ORDER BY g.created_at DESC
            LIMIT ?
        "#;
        
        let result = self.db.query(sql, &[
            &user_id.to_string() as &dyn SqlParameter,
            &limit as &dyn SqlParameter,
        ]).await?;
        
        let mut games = Vec::new();
        for row in result.rows {
            games.push(self.row_to_game(&row)?);
        }
        
        Ok(games)
    }
    
    /// Helper to convert database row to Game entity
    fn row_to_game(&self, row: &DatabaseRow) -> Result<Game> {
        Ok(Game {
            id: row.get("id")?,
            game_type: row.get("game_type")?,
            status: row.get("status")?,
            creator_id: row.get("creator_id")?,
            max_players: row.get("max_players")?,
            current_players: row.get("current_players")?,
            min_bet: row.get("min_bet")?,
            max_bet: row.get("max_bet")?,
            house_edge: row.get("house_edge")?,
            total_pot: row.get("total_pot")?,
            game_state: row.get("game_state")?,
            consensus_state: row.get("consensus_state")?,
            dice_results: row.get("dice_results")?,
            round_number: row.get("round_number")?,
            created_at: row.get("created_at")?,
            started_at: row.get_opt("started_at")?,
            completed_at: row.get_opt("completed_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// Bet repository implementation
pub struct BetRepository {
    db: Arc<DatabaseManager>,
}

impl BetRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Create a new bet
    pub async fn create_bet(&self, bet: &Bet) -> Result<()> {
        let sql = r#"
            INSERT INTO bets (
                id, game_id, user_id, participant_id, bet_type, bet_amount,
                potential_payout, actual_payout, odds_numerator, odds_denominator,
                round_number, placed_at, resolved_at, status, bet_data
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        self.db.execute(sql, &[
            &bet.id as &dyn SqlParameter,
            &bet.game_id,
            &bet.user_id,
            &bet.participant_id,
            &bet.bet_type,
            &bet.bet_amount,
            &bet.potential_payout,
            &bet.actual_payout,
            &bet.odds_numerator,
            &bet.odds_denominator,
            &bet.round_number,
            &bet.placed_at,
            &bet.resolved_at,
            &bet.status,
            &bet.bet_data,
        ]).await?;
        
        Ok(())
    }
    
    /// Find bet by ID
    pub async fn find_by_id(&self, bet_id: &str) -> Result<Option<Bet>> {
        let result = self.db.query(
            "SELECT * FROM bets WHERE id = ?",
            &[&bet_id.to_string() as &dyn SqlParameter]
        ).await?;
        
        if result.rows.is_empty() {
            Ok(None)
        } else {
            let row = &result.rows[0];
            Ok(Some(self.row_to_bet(&row)?))
        }
    }
    
    /// Get bets for a game
    pub async fn get_game_bets(&self, game_id: &str) -> Result<Vec<Bet>> {
        let result = self.db.query(
            "SELECT * FROM bets WHERE game_id = ? ORDER BY placed_at ASC",
            &[&game_id.to_string() as &dyn SqlParameter]
        ).await?;
        
        let mut bets = Vec::new();
        for row in result.rows {
            bets.push(self.row_to_bet(&row)?);
        }
        
        Ok(bets)
    }
    
    /// Get user's bets
    pub async fn get_user_bets(&self, user_id: &str, limit: i32) -> Result<Vec<Bet>> {
        let result = self.db.query(
            "SELECT * FROM bets WHERE user_id = ? ORDER BY placed_at DESC LIMIT ?",
            &[
                &user_id.to_string() as &dyn SqlParameter,
                &limit as &dyn SqlParameter,
            ]
        ).await?;
        
        let mut bets = Vec::new();
        for row in result.rows {
            bets.push(self.row_to_bet(&row)?);
        }
        
        Ok(bets)
    }
    
    /// Update bet (typically for resolution)
    pub async fn update_bet(&self, bet: &Bet) -> Result<()> {
        let sql = r#"
            UPDATE bets SET
                actual_payout = ?, resolved_at = ?, status = ?
            WHERE id = ?
        "#;
        
        self.db.execute(sql, &[
            &bet.actual_payout as &dyn SqlParameter,
            &bet.resolved_at,
            &bet.status,
            &bet.id,
        ]).await?;
        
        Ok(())
    }
    
    /// Get pending bets for a game round
    pub async fn get_pending_bets_for_round(&self, game_id: &str, round_number: i32) -> Result<Vec<Bet>> {
        let result = self.db.query(
            "SELECT * FROM bets WHERE game_id = ? AND round_number = ? AND status = 'pending' ORDER BY placed_at ASC",
            &[
                &game_id.to_string() as &dyn SqlParameter,
                &round_number as &dyn SqlParameter,
            ]
        ).await?;
        
        let mut bets = Vec::new();
        for row in result.rows {
            bets.push(self.row_to_bet(&row)?);
        }
        
        Ok(bets)
    }
    
    /// Helper to convert database row to Bet entity
    fn row_to_bet(&self, row: &DatabaseRow) -> Result<Bet> {
        Ok(Bet {
            id: row.get("id")?,
            game_id: row.get("game_id")?,
            user_id: row.get("user_id")?,
            participant_id: row.get("participant_id")?,
            bet_type: row.get("bet_type")?,
            bet_amount: row.get("bet_amount")?,
            potential_payout: row.get("potential_payout")?,
            actual_payout: row.get("actual_payout")?,
            odds_numerator: row.get("odds_numerator")?,
            odds_denominator: row.get("odds_denominator")?,
            round_number: row.get("round_number")?,
            placed_at: row.get("placed_at")?,
            resolved_at: row.get_opt("resolved_at")?,
            status: row.get("status")?,
            bet_data: row.get("bet_data")?,
        })
    }
}

/// Transaction repository implementation
pub struct TransactionRepository {
    db: Arc<DatabaseManager>,
}

impl TransactionRepository {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Create a new transaction
    pub async fn create_transaction(&self, transaction: &Transaction) -> Result<()> {
        let sql = r#"
            INSERT INTO transactions (
                id, user_id, transaction_type, amount, balance_before, balance_after,
                game_id, bet_id, reference_id, description, metadata,
                created_at, confirmed_at, status
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;
        
        self.db.execute(sql, &[
            &transaction.id as &dyn SqlParameter,
            &transaction.user_id,
            &transaction.transaction_type,
            &transaction.amount,
            &transaction.balance_before,
            &transaction.balance_after,
            &transaction.game_id,
            &transaction.bet_id,
            &transaction.reference_id,
            &transaction.description,
            &transaction.metadata,
            &transaction.created_at,
            &transaction.confirmed_at,
            &transaction.status,
        ]).await?;
        
        Ok(())
    }
    
    /// Get user's transaction history
    pub async fn get_user_transactions(&self, user_id: &str, limit: i32) -> Result<Vec<Transaction>> {
        let result = self.db.query(
            "SELECT * FROM transactions WHERE user_id = ? ORDER BY created_at DESC LIMIT ?",
            &[
                &user_id.to_string() as &dyn SqlParameter,
                &limit as &dyn SqlParameter,
            ]
        ).await?;
        
        let mut transactions = Vec::new();
        for row in result.rows {
            transactions.push(self.row_to_transaction(&row)?);
        }
        
        Ok(transactions)
    }
    
    /// Get user's current balance (from latest transaction)
    pub async fn get_user_balance(&self, user_id: &str) -> Result<i64> {
        let result = self.db.query(
            "SELECT balance_after FROM transactions WHERE user_id = ? AND status = 'confirmed' ORDER BY created_at DESC LIMIT 1",
            &[&user_id.to_string() as &dyn SqlParameter]
        ).await?;
        
        if result.rows.is_empty() {
            Ok(0) // No transactions yet
        } else {
            Ok(result.rows[0].get("balance_after")?)
        }
    }
    
    /// Confirm a transaction
    pub async fn confirm_transaction(&self, transaction_id: &str, confirmed_at: i64) -> Result<()> {
        self.db.execute(
            "UPDATE transactions SET status = 'confirmed', confirmed_at = ? WHERE id = ?",
            &[
                &confirmed_at as &dyn SqlParameter,
                &transaction_id.to_string() as &dyn SqlParameter,
            ]
        ).await?;
        
        Ok(())
    }
    
    /// Helper to convert database row to Transaction entity
    fn row_to_transaction(&self, row: &DatabaseRow) -> Result<Transaction> {
        Ok(Transaction {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            transaction_type: row.get("transaction_type")?,
            amount: row.get("amount")?,
            balance_before: row.get("balance_before")?,
            balance_after: row.get("balance_after")?,
            game_id: row.get_opt("game_id")?,
            bet_id: row.get_opt("bet_id")?,
            reference_id: row.get_opt("reference_id")?,
            description: row.get_opt("description")?,
            metadata: row.get("metadata")?,
            created_at: row.get("created_at")?,
            confirmed_at: row.get_opt("confirmed_at")?,
            status: row.get("status")?,
        })
    }
}

/// Repository factory for creating all repositories
pub struct RepositoryFactory {
    db: Arc<DatabaseManager>,
}

impl RepositoryFactory {
    pub fn new(db: Arc<DatabaseManager>) -> Self {
        Self { db }
    }
    
    /// Get user repository
    pub fn users(&self) -> UserRepository {
        UserRepository::new(self.db.clone())
    }
    
    /// Get game repository
    pub fn games(&self) -> GameRepository {
        GameRepository::new(self.db.clone())
    }
    
    /// Get bet repository
    pub fn bets(&self) -> BetRepository {
        BetRepository::new(self.db.clone())
    }
    
    /// Get transaction repository
    pub fn transactions(&self) -> TransactionRepository {
        TransactionRepository::new(self.db.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::database_manager::create_database_manager_from_env;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    #[tokio::test]
    async fn test_user_repository() {
        // Use in-memory SQLite for testing
        std::env::set_var("DATABASE_URL", "sqlite::memory:");
        let db_manager = Arc::new(create_database_manager_from_env().await.unwrap());
        let user_repo = UserRepository::new(db_manager);
        
        // Create test user
        let user = User {
            id: Uuid::new_v4().to_string(),
            username: "test_user".to_string(),
            email: Some("test@example.com".to_string()),
            password_hash: vec![1, 2, 3, 4],
            salt: vec![5, 6, 7, 8],
            public_key: vec![9, 10, 11, 12],
            reputation_score: 1000,
            total_games_played: 0,
            total_winnings: 0,
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            updated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            last_active: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            account_status: "active".to_string(),
            kyc_status: "none".to_string(),
            preferences: "{}".to_string(),
        };
        
        // Create user
        user_repo.create_user(&user).await.unwrap();
        
        // Find by ID
        let found_user = user_repo.find_by_id(&user.id).await.unwrap().unwrap();
        assert_eq!(found_user.username, "test_user");
        assert_eq!(found_user.reputation_score, 1000);
        
        // Find by username
        let found_by_username = user_repo.find_by_username("test_user").await.unwrap().unwrap();
        assert_eq!(found_by_username.id, user.id);
        
        // Update user
        let mut updated_user = found_user.clone();
        updated_user.reputation_score = 1100;
        user_repo.update_user(&updated_user).await.unwrap();
        
        // Verify update
        let verified_user = user_repo.find_by_id(&user.id).await.unwrap().unwrap();
        assert_eq!(verified_user.reputation_score, 1100);
    }
    
    #[tokio::test]
    async fn test_repository_factory() {
        std::env::set_var("DATABASE_URL", "sqlite::memory:");
        let db_manager = Arc::new(create_database_manager_from_env().await.unwrap());
        let factory = RepositoryFactory::new(db_manager);
        
        // Test that all repositories can be created
        let _users = factory.users();
        let _games = factory.games();
        let _bets = factory.bets();
        let _transactions = factory.transactions();
    }
}