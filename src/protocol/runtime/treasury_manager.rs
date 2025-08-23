//! Treasury Manager
//! 
//! Manages the treasury balance, rake collection, and payouts.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::protocol::{GameId, PeerId};
use crate::error::{Error, Result};

/// Manages treasury operations
pub struct TreasuryManager {
    /// Treasury balance
    balance: Arc<RwLock<u64>>,
    
    /// Game pots
    game_pots: Arc<RwLock<HashMap<GameId, u64>>>,
    
    /// Rake percentage (e.g., 0.01 for 1%)
    rake_percentage: f32,
    
    /// Total rake collected
    total_rake_collected: Arc<RwLock<u64>>,
    
    /// Pending payouts
    pending_payouts: Arc<RwLock<HashMap<PeerId, u64>>>,
}

impl TreasuryManager {
    /// Create a new treasury manager
    pub fn new(rake_percentage: f32) -> Self {
        Self {
            balance: Arc::new(RwLock::new(1_000_000_000)), // 1B initial
            game_pots: Arc::new(RwLock::new(HashMap::new())),
            rake_percentage,
            total_rake_collected: Arc::new(RwLock::new(0)),
            pending_payouts: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Add funds to a game pot
    pub async fn add_to_pot(&self, game_id: GameId, amount: u64) -> Result<()> {
        let mut pots = self.game_pots.write().await;
        let pot = pots.entry(game_id).or_insert(0);
        *pot = (*pot).saturating_add(amount);
        Ok(())
    }
    
    /// Calculate and collect rake from a pot
    pub async fn collect_rake(&self, game_id: GameId) -> Result<u64> {
        let mut pots = self.game_pots.write().await;
        let pot = pots.get_mut(&game_id)
            .ok_or_else(|| Error::GameNotFound)?;
        
        let rake = (*pot as f32 * self.rake_percentage) as u64;
        *pot = pot.saturating_sub(rake);
        
        // Add to treasury
        let mut balance = self.balance.write().await;
        *balance = balance.saturating_add(rake);
        
        // Track total rake
        let mut total = self.total_rake_collected.write().await;
        *total = total.saturating_add(rake);
        
        Ok(rake)
    }
    
    /// Process payouts for a game
    pub async fn process_payouts(
        &self,
        game_id: GameId,
        winners: Vec<(PeerId, u64)>,
    ) -> Result<()> {
        // Collect rake first
        let rake = self.collect_rake(game_id).await?;
        
        // Get remaining pot
        let mut pots = self.game_pots.write().await;
        let pot = pots.remove(&game_id).unwrap_or(0);
        
        // Validate total payouts don't exceed pot
        let total_payouts: u64 = winners.iter().map(|(_, amount)| amount).sum();
        if total_payouts > pot {
            return Err(Error::InvalidState(
                format!("Payouts {} exceed pot {}", total_payouts, pot)
            ));
        }
        
        // Queue payouts
        let mut pending = self.pending_payouts.write().await;
        for (player, amount) in winners {
            let balance = pending.entry(player).or_insert(0);
            *balance = balance.saturating_add(amount);
        }
        
        // Return excess to treasury
        let excess = pot.saturating_sub(total_payouts);
        if excess > 0 {
            let mut balance = self.balance.write().await;
            *balance = balance.saturating_add(excess);
        }
        
        log::info!("Processed payouts for game {:?}: rake={}, total_payouts={}, excess={}", 
                  game_id, rake, total_payouts, excess);
        
        Ok(())
    }
    
    /// Claim pending payouts for a player
    pub async fn claim_payouts(&self, player: PeerId) -> Result<u64> {
        let mut pending = self.pending_payouts.write().await;
        Ok(pending.remove(&player).unwrap_or(0))
    }
    
    /// Get treasury balance
    pub async fn get_balance(&self) -> u64 {
        *self.balance.read().await
    }
    
    /// Get total rake collected
    pub async fn get_total_rake(&self) -> u64 {
        *self.total_rake_collected.read().await
    }
    
    /// Get game pot
    pub async fn get_pot(&self, game_id: GameId) -> u64 {
        self.game_pots.read().await.get(&game_id).copied().unwrap_or(0)
    }
    
    /// Check if treasury can cover a payout
    pub async fn can_cover_payout(&self, amount: u64) -> bool {
        *self.balance.read().await >= amount
    }
    
    /// Emergency fund injection (for testing or recovery)
    pub async fn inject_funds(&self, amount: u64) -> Result<()> {
        let mut balance = self.balance.write().await;
        *balance = balance.saturating_add(amount);
        Ok(())
    }
}