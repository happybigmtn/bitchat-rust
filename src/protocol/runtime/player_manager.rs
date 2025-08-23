//! Player Manager
//! 
//! Manages player balances, sessions, and validation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use crate::protocol::PeerId;
use crate::error::{Error, Result};

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
        self.balances.read().await.get(&peer_id).copied().unwrap_or(0)
    }
    
    /// Get available balance (total - locked)
    pub async fn get_available_balance(&self, peer_id: PeerId) -> u64 {
        let balance = self.get_balance(peer_id).await;
        let locked = self.locked_balances.read().await.get(&peer_id).copied().unwrap_or(0);
        balance.saturating_sub(locked)
    }
    
    /// Deduct balance
    pub async fn deduct_balance(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        let mut balances = self.balances.write().await;
        let balance = balances.get_mut(&peer_id)
            .ok_or(Error::PlayerNotFound)?;
        
        if *balance < amount {
            return Err(Error::InsufficientBalance);
        }
        
        *balance = balance.saturating_sub(amount);
        
        // Update session
        if let Some(session) = self.sessions.write().await.get_mut(&peer_id) {
            session.balance = *balance;
            session.total_wagered = session.total_wagered.saturating_add(amount);
            session.last_activity = Instant::now();
        }
        
        Ok(())
    }
    
    /// Add to balance
    pub async fn add_balance(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        let mut balances = self.balances.write().await;
        let balance = balances.entry(peer_id).or_insert(0);
        *balance = balance.saturating_add(amount);
        
        // Update session
        if let Some(session) = self.sessions.write().await.get_mut(&peer_id) {
            session.balance = *balance;
            session.total_won = session.total_won.saturating_add(amount);
            session.last_activity = Instant::now();
        }
        
        Ok(())
    }
    
    /// Lock funds for a bet
    pub async fn lock_funds(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        // Check available balance
        let available = self.get_available_balance(peer_id).await;
        if available < amount {
            return Err(Error::InsufficientBalance);
        }
        
        // Lock the funds
        let mut locked = self.locked_balances.write().await;
        let locked_balance = locked.entry(peer_id).or_insert(0);
        *locked_balance = locked_balance.saturating_add(amount);
        
        Ok(())
    }
    
    /// Unlock funds
    pub async fn unlock_funds(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        let mut locked = self.locked_balances.write().await;
        let locked_balance = locked.entry(peer_id).or_insert(0);
        *locked_balance = locked_balance.saturating_sub(amount);
        Ok(())
    }
    
    /// Validate a bet
    pub async fn validate_bet(&self, peer_id: PeerId, amount: u64) -> Result<()> {
        let available = self.get_available_balance(peer_id).await;
        if available < amount {
            return Err(Error::InsufficientBalance);
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
        
        sessions.retain(|_, session| {
            now.duration_since(session.last_activity) < timeout
        });
    }
    
    /// Get all active players
    pub async fn get_active_players(&self) -> Vec<PeerId> {
        self.sessions.read().await.keys().copied().collect()
    }
}