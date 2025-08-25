//! Treasury System - Counterparty to All Bets
//! 
//! The treasury acts as the house/bank in the decentralized casino,
//! serving as the counterparty to all player bets. This ensures that:
//! - Players always have a counterparty to bet against
//! - Payouts are guaranteed from a common pool
//! - The house edge maintains treasury solvency
//! - No player can refuse to pay out winnings

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use crate::protocol::{CrapTokens, GameId, Hash256};
use crate::error::Error;
use crate::crypto::GameCrypto;

/// Treasury configuration constants
pub const INITIAL_TREASURY_BALANCE: u64 = 1_000_000_000; // 1 billion CRAP tokens
pub const MIN_TREASURY_RESERVE: u64 = 100_000_000; // 100 million minimum reserve
pub const MAX_BET_RATIO: f64 = 0.01; // Max single bet is 1% of treasury
pub const HOUSE_EDGE: f64 = 0.014; // 1.4% house edge (standard craps)
pub const TREASURY_FEE: f64 = 0.001; // 0.1% transaction fee

/// Treasury state - the decentralized bank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Treasury {
    /// Current treasury balance
    pub balance: CrapTokens,
    
    /// Locked funds for active bets (potential payouts)
    pub locked_funds: CrapTokens,
    
    /// Historical profit/loss tracking
    pub total_wagered: CrapTokens,
    pub total_paid_out: CrapTokens,
    pub total_fees_collected: CrapTokens,
    
    /// Per-game locked funds
    pub game_locks: HashMap<GameId, CrapTokens>,
    
    /// Treasury state hash for consensus
    pub state_hash: Hash256,
    
    /// Treasury version for upgrade compatibility
    pub version: u32,
}

impl Default for Treasury {
    fn default() -> Self {
        Self::new()
    }
}

impl Treasury {
    /// Create a new treasury with initial balance
    pub fn new() -> Self {
        let mut treasury = Self {
            balance: CrapTokens::from(INITIAL_TREASURY_BALANCE),
            locked_funds: CrapTokens::from(0),
            total_wagered: CrapTokens::from(0),
            total_paid_out: CrapTokens::from(0),
            total_fees_collected: CrapTokens::from(0),
            game_locks: HashMap::new(),
            state_hash: [0; 32],
            version: 1,
        };
        treasury.update_state_hash();
        treasury
    }
    
    /// Calculate maximum allowed bet based on treasury balance
    pub fn max_bet_amount(&self) -> CrapTokens {
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        let max_amount = (available.0 as f64 * MAX_BET_RATIO) as u64;
        let reserve_adjusted = available.0.saturating_sub(MIN_TREASURY_RESERVE);
        CrapTokens::from(max_amount.min(reserve_adjusted))
    }
    
    /// Lock funds for a potential payout
    pub fn lock_funds(&mut self, game_id: GameId, amount: CrapTokens, max_payout: CrapTokens) -> Result<(), Error> {
        // Check if treasury can cover the maximum payout
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        if max_payout > available {
            return Err(Error::InsufficientFunds(
                format!("Treasury cannot cover max payout of {} CRAP", max_payout.0)
            ));
        }
        
        // Check if bet exceeds maximum allowed
        let max_bet = self.max_bet_amount();
        if amount > max_bet {
            return Err(Error::InvalidBet(
                format!("Bet of {} exceeds max allowed bet of {}", amount.0, max_bet.0)
            ));
        }
        
        // Lock the maximum potential payout
        self.locked_funds = self.locked_funds.checked_add(max_payout)
            .ok_or_else(|| Error::InvalidData("Overflow in locked funds".to_string()))?;
        let current = self.game_locks.get(&game_id).copied().unwrap_or(CrapTokens::from(0));
        *self.game_locks.entry(game_id).or_insert(CrapTokens::from(0)) = 
            current.checked_add(max_payout)
                .ok_or_else(|| Error::InvalidData("Overflow in game locks".to_string()))?;
        
        // Track wagered amount
        self.total_wagered = self.total_wagered.checked_add(amount)
            .ok_or_else(|| Error::InvalidData("Overflow in total wagered".to_string()))?;
        
        // Collect treasury fee
        let fee = CrapTokens::from((amount.0 as f64 * TREASURY_FEE) as u64);
        self.balance = self.balance.checked_add(fee)
            .ok_or_else(|| Error::InvalidData("Overflow in balance".to_string()))?;
        self.total_fees_collected = self.total_fees_collected.checked_add(fee)
            .ok_or_else(|| Error::InvalidData("Overflow in fees collected".to_string()))?;
        
        self.update_state_hash();
        Ok(())
    }
    
    /// Release locked funds and process payout
    pub fn settle_bet(&mut self, game_id: GameId, locked_amount: CrapTokens, actual_payout: CrapTokens) -> Result<(), Error> {
        // Get locked amount for this game
        let game_locked = self.game_locks.get(&game_id).copied()
            .unwrap_or(CrapTokens::from(0));
        if locked_amount > game_locked {
            return Err(Error::InvalidState(
                format!("Attempted to release {} but only {} locked for game", 
                    locked_amount.0, game_locked.0)
            ));
        }
        
        // Release the locked funds
        self.locked_funds = self.locked_funds.checked_sub(locked_amount)
            .unwrap_or(CrapTokens::from(0));
        if let Some(lock) = self.game_locks.get_mut(&game_id) {
            *lock = lock.checked_sub(locked_amount)
                .unwrap_or(CrapTokens::from(0));
            if lock.0 == 0 {
                self.game_locks.remove(&game_id);
            }
        }
        
        // Process the actual payout
        if actual_payout.0 > 0 {
            if actual_payout > self.balance {
                return Err(Error::InsufficientFunds(
                    format!("Treasury balance {} insufficient for payout {}", 
                        self.balance.0, actual_payout.0)
                ));
            }
            self.balance = self.balance.checked_sub(actual_payout)
                .ok_or_else(|| Error::InvalidData("Underflow in balance".to_string()))?;
            self.total_paid_out = self.total_paid_out.checked_add(actual_payout)
                .ok_or_else(|| Error::InvalidData("Overflow in total paid out".to_string()))?;
        } else {
            // House wins - add the bet amount to treasury
            let house_win = locked_amount.checked_sub(actual_payout)
                .unwrap_or(locked_amount);
            self.balance = self.balance.checked_add(house_win)
                .ok_or_else(|| Error::InvalidData("Overflow in balance".to_string()))?;
        }
        
        self.update_state_hash();
        Ok(())
    }
    
    /// Release all locks for a game (e.g., on game cancellation)
    pub fn release_game_locks(&mut self, game_id: GameId) {
        if let Some(locked) = self.game_locks.remove(&game_id) {
            self.locked_funds = self.locked_funds.checked_sub(locked)
                .unwrap_or(CrapTokens::from(0));
            self.update_state_hash();
        }
    }
    
    /// Calculate house edge for a specific bet type
    pub fn calculate_house_edge(&self, bet_type: &crate::protocol::BetType) -> f64 {
        use crate::protocol::BetType;
        
        // Standard craps house edges
        match bet_type {
            BetType::Pass | BetType::DontPass => 0.014,
            BetType::Come | BetType::DontCome => 0.014,
            BetType::Field => 0.027,
            BetType::Yes6 | BetType::Yes8 => 0.015,  // Place 6/8 equivalent
            BetType::Yes5 | BetType::Yes9 => 0.040,  // Place 5/9 equivalent 
            BetType::Yes4 | BetType::Yes10 => 0.067, // Place 4/10 equivalent
            BetType::Hard4 | BetType::Hard10 => 0.111,
            BetType::Hard6 | BetType::Hard8 => 0.091,
            BetType::Next7 => 0.167,  // Any Seven equivalent
            BetType::Next2 | BetType::Next3 | BetType::Next11 | BetType::Next12 => 0.111, // Any Craps equivalent
            _ => HOUSE_EDGE,
        }
    }
    
    /// Get treasury health metrics
    pub fn get_health(&self) -> TreasuryHealth {
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        let utilization = if self.balance.0 > 0 {
            (self.locked_funds.0 as f64 / self.balance.0 as f64) * 100.0
        } else {
            0.0
        };
        
        let profit = self.total_wagered.0 as i64 - self.total_paid_out.0 as i64 
            + self.total_fees_collected.0 as i64;
        
        TreasuryHealth {
            balance: self.balance,
            available_balance: available,
            locked_funds: self.locked_funds,
            utilization_percent: utilization,
            total_profit: profit,
            is_solvent: available.0 >= MIN_TREASURY_RESERVE,
            max_single_bet: self.max_bet_amount(),
            active_games: self.game_locks.len(),
        }
    }
    
    /// Update the state hash for consensus verification
    fn update_state_hash(&mut self) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.balance.0.to_le_bytes());
        data.extend_from_slice(&self.locked_funds.0.to_le_bytes());
        data.extend_from_slice(&self.total_wagered.0.to_le_bytes());
        data.extend_from_slice(&self.total_paid_out.0.to_le_bytes());
        data.extend_from_slice(&self.total_fees_collected.0.to_le_bytes());
        data.extend_from_slice(&self.version.to_le_bytes());
        
        self.state_hash = GameCrypto::hash(&data);
    }
    
    /// Verify treasury state integrity
    pub fn verify_integrity(&self) -> bool {
        // Check basic invariants
        if self.locked_funds > self.balance {
            return false;
        }
        
        // Verify game locks sum matches total locked
        let total_game_locks: u64 = self.game_locks.values()
            .map(|t| t.0)
            .sum();
        if total_game_locks != self.locked_funds.0 {
            return false;
        }
        
        // Verify state hash
        let mut temp = self.clone();
        temp.update_state_hash();
        temp.state_hash == self.state_hash
    }
}

/// Treasury health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryHealth {
    pub balance: CrapTokens,
    pub available_balance: CrapTokens,
    pub locked_funds: CrapTokens,
    pub utilization_percent: f64,
    pub total_profit: i64,
    pub is_solvent: bool,
    pub max_single_bet: CrapTokens,
    pub active_games: usize,
}

/// Treasury manager with thread-safe access
pub struct TreasuryManager {
    treasury: Arc<RwLock<Treasury>>,
}

impl TreasuryManager {
    /// Create a new treasury manager
    pub fn new() -> Self {
        Self {
            treasury: Arc::new(RwLock::new(Treasury::new())),
        }
    }
    
    /// Get a read-only reference to the treasury
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, Treasury> {
        self.treasury.read().unwrap()
    }
    
    /// Get a mutable reference to the treasury
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, Treasury> {
        self.treasury.write().unwrap()
    }
    
    /// Process a bet with automatic locking
    pub fn process_bet(&self, game_id: GameId, bet_amount: CrapTokens, max_payout: CrapTokens) -> Result<(), Error> {
        let mut treasury = self.write();
        treasury.lock_funds(game_id, bet_amount, max_payout)
    }
    
    /// Settle a bet with automatic unlocking
    pub fn settle_bet(&self, game_id: GameId, locked_amount: CrapTokens, actual_payout: CrapTokens) -> Result<(), Error> {
        let mut treasury = self.write();
        treasury.settle_bet(game_id, locked_amount, actual_payout)
    }
    
    /// Get current treasury health
    pub fn get_health(&self) -> TreasuryHealth {
        self.read().get_health()
    }
    
    /// Verify treasury integrity
    pub fn verify_integrity(&self) -> bool {
        self.read().verify_integrity()
    }
}

impl Default for TreasuryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_treasury_creation() {
        let treasury = Treasury::new();
        assert_eq!(treasury.balance.amount(), INITIAL_TREASURY_BALANCE);
        assert_eq!(treasury.locked_funds.amount(), 0);
        assert!(treasury.verify_integrity());
    }
    
    #[test]
    fn test_lock_and_release() {
        let mut treasury = Treasury::new();
        let game_id = [1; 16];
        
        // Lock funds for a bet
        let bet_amount = CrapTokens::new_unchecked(1000);
        let max_payout = CrapTokens::new_unchecked(2000);
        assert!(treasury.lock_funds(game_id, bet_amount, max_payout).is_ok());
        assert_eq!(treasury.locked_funds.amount(), 2000);
        assert!(treasury.balance.amount() > INITIAL_TREASURY_BALANCE); // Fee added
        
        // Settle with payout
        let payout = CrapTokens::new_unchecked(1500);
        assert!(treasury.settle_bet(game_id, max_payout, payout).is_ok());
        assert_eq!(treasury.locked_funds.amount(), 0);
        assert!(treasury.balance.amount() < INITIAL_TREASURY_BALANCE);
        
        assert!(treasury.verify_integrity());
    }
    
    #[test]
    fn test_max_bet_limits() {
        let treasury = Treasury::new();
        let max_bet = treasury.max_bet_amount();
        
        assert!(max_bet.amount() > 0);
        assert!(max_bet.amount() < treasury.balance.amount());
        assert!(max_bet.amount() <= (treasury.balance.amount() as f64 * MAX_BET_RATIO) as u64);
    }
    
    #[test]
    fn test_treasury_health() {
        let mut treasury = Treasury::new();
        let health = treasury.get_health();
        
        assert!(health.is_solvent);
        assert_eq!(health.utilization_percent, 0.0);
        assert_eq!(health.active_games, 0);
        
        // Lock some funds
        let game_id = [1; 16];
        let bet_amount = CrapTokens::new_unchecked(1000);
        let max_payout = CrapTokens::new_unchecked(5000);
        treasury.lock_funds(game_id, bet_amount, max_payout).unwrap();
        
        let health = treasury.get_health();
        assert!(health.utilization_percent > 0.0);
        assert_eq!(health.active_games, 1);
    }
}