//! Player Manager
//!
//! Manages player balances, sessions, and validation.

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// Player session information
#[derive(Debug, Clone)]
pub struct PlayerSession {
    pub peer_id: PeerId,
    pub balance: u64,
    pub total_wagered: u64,
    pub total_won: u64,
    pub games_played: u32,
    pub joined_at: Instant,
    pub last_activity: Instant,
}

/// Manages player data and balances
pub struct PlayerManager {
    /// Player sessions
    sessions: Arc<RwLock<HashMap<PeerId, PlayerSession>>>,

    /// Player balances (persistent)
    balances: Arc<RwLock<HashMap<PeerId, u64>>>,

    /// Locked balances (funds in active bets)
    locked_balances: Arc<RwLock<HashMap<PeerId, u64>>>,
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerManager {
    /// Create a new player manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            balances: Arc::new(RwLock::new(HashMap::new())),
            locked_balances: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new player or get existing
    pub async fn register_player(&self, peer_id: PeerId, initial_balance: u64) -> Result<()> {
        let mut balances = self.balances.write().await;
        balances.entry(peer_id).or_insert(initial_balance);

        let mut sessions = self.sessions.write().await;
        sessions.entry(peer_id).or_insert_with(|| PlayerSession {
            peer_id,
            balance: initial_balance,
            total_wagered: 0,
            total_won: 0,
            games_played: 0,
            joined_at: Instant::now(),
            last_activity: Instant::now(),
        });

        Ok(())
    }

    /// Get player balance
    pub async fn get_balance(&self, peer_id: PeerId) -> u64 {
        let balances = self.balances.read().await;
        balances.get(&peer_id).copied().unwrap_or(0)
    }

    /// Get available balance (total - locked)
    pub async fn get_available_balance(&self, peer_id: PeerId) -> u64 {
        let total_balance = self.get_balance(peer_id).await;
        let locked_amount = self.get_locked_balance(peer_id).await;
        total_balance.saturating_sub(locked_amount)
    }

    /// Get locked balance for a player
    async fn get_locked_balance(&self, peer_id: PeerId) -> u64 {
        let locked_balances = self.locked_balances.read().await;
        locked_balances.get(&peer_id).copied().unwrap_or(0)
    }

    /// Deduct balance
    pub async fn deduct_balance(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        // Update balance
        let new_balance = self.update_balance(peer_id, amount, false).await?;

        // Update session statistics
        self.update_session_on_wager(peer_id, new_balance, amount)
            .await;

        Ok(())
    }

    /// Update balance (internal helper)
    async fn update_balance(&self, peer_id: PeerId, amount: u64, is_credit: bool) -> Result<u64> {
        let mut balances = self.balances.write().await;
        let balance = balances.get_mut(&peer_id).ok_or(Error::PlayerNotFound)?;

        let new_balance = if is_credit {
            balance.saturating_add(amount)
        } else {
            if *balance < amount {
                return Err(Error::InsufficientBalance(format!(
                    "Balance: {}, Required: {}",
                    balance, amount
                )));
            }
            balance.saturating_sub(amount)
        };

        *balance = new_balance;
        Ok(new_balance)
    }

    /// Update session when player places a wager
    async fn update_session_on_wager(
        &self,
        peer_id: PeerId,
        new_balance: u64,
        wagered_amount: u64,
    ) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&peer_id) {
            session.balance = new_balance;
            session.total_wagered = session.total_wagered.saturating_add(wagered_amount);
            session.last_activity = Instant::now();
        }
    }

    /// Add to balance
    pub async fn add_balance(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        // Ensure player exists first
        self.ensure_player_exists(peer_id).await;

        // Update balance
        let new_balance = self.update_balance(peer_id, amount, true).await?;

        // Update session statistics
        self.update_session_on_win(peer_id, new_balance, amount)
            .await;

        Ok(())
    }

    /// Ensure player exists in balances map
    async fn ensure_player_exists(&self, peer_id: PeerId) {
        let mut balances = self.balances.write().await;
        balances.entry(peer_id).or_insert(0);
    }

    /// Update session when player wins
    async fn update_session_on_win(&self, peer_id: PeerId, new_balance: u64, won_amount: u64) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&peer_id) {
            session.balance = new_balance;
            session.total_won = session.total_won.saturating_add(won_amount);
            session.last_activity = Instant::now();
        }
    }

    /// Lock funds for a bet
    pub async fn lock_funds(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        // Validate sufficient available balance
        self.validate_available_balance(peer_id, amount).await?;

        // Lock the funds
        self.increase_locked_balance(peer_id, amount).await;

        Ok(())
    }

    /// Validate player has sufficient available balance
    async fn validate_available_balance(
        &self,
        peer_id: PeerId,
        required_amount: u64,
    ) -> Result<()> {
        let available_balance = self.get_available_balance(peer_id).await;
        if available_balance < required_amount {
            return Err(Error::InsufficientBalance(format!(
                "Available: {}, Required: {}",
                available_balance, required_amount
            )));
        }
        Ok(())
    }

    /// Increase locked balance
    async fn increase_locked_balance(&self, peer_id: PeerId, amount: u64) {
        let mut locked_balances = self.locked_balances.write().await;
        let locked_balance = locked_balances.entry(peer_id).or_insert(0);
        *locked_balance = locked_balance.saturating_add(amount);
    }

    /// Unlock funds
    pub async fn unlock_funds(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        self.decrease_locked_balance(peer_id, amount).await;
        Ok(())
    }

    /// Decrease locked balance
    async fn decrease_locked_balance(&self, peer_id: PeerId, amount: u64) {
        let mut locked_balances = self.locked_balances.write().await;
        let locked_balance = locked_balances.entry(peer_id).or_insert(0);
        *locked_balance = locked_balance.saturating_sub(amount);
    }

    /// Validate a bet
    pub async fn validate_bet(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        let available = self.get_available_balance(peer_id).await;
        if available < amount {
            return Err(Error::InsufficientBalance(format!(
                "Balance: {}, Required: {}",
                available, amount
            )));
        }
        Ok(())
    }

    /// Update player statistics
    pub async fn update_stats(&self, peer_id: PeerId, games_played: u32) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&peer_id) {
            session.games_played += games_played;
            session.last_activity = Instant::now();
        }
        Ok(())
    }

    /// Get player session
    pub async fn get_session(&self, peer_id: PeerId) -> Option<PlayerSession> {
        self.sessions.read().await.get(&peer_id).cloned()
    }

    /// Remove inactive sessions
    pub async fn cleanup_inactive_sessions(&self, timeout: std::time::Duration) {
        let mut sessions = self.sessions.write().await;
        let now = Instant::now();

        sessions.retain(|_, session| now.duration_since(session.last_activity) < timeout);
    }

    /// Get all active players
    pub async fn get_active_players(&self) -> Vec<PeerId> {
        self.sessions.read().await.keys().copied().collect()
    }
}
