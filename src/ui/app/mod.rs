//! Mod module for BitCraps UI
//! 
//! This module implements the user interface components for BitCraps
//! including CLI, TUI, and specialized casino widgets.

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BetResult {
    Won,
    Lost,
    Push,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletError {
    InsufficientFunds,
    BetNotFound,
    TransactionFailed,
    Custom(String),
}

impl From<String> for WalletError {
    fn from(msg: String) -> Self {
        WalletError::Custom(msg)
    }
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


