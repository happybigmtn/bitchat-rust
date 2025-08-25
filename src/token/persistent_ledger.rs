//! Persistent Token Ledger with Synchronization
//! 
//! This module provides persistent storage for the CRAP token ledger,
//! ensuring that token balances survive application restarts and
//! enabling synchronization between peers.

use std::collections::{HashMap, BTreeMap};
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use crate::protocol::{PeerId, CrapTokens, Hash256};
use crate::error::Error;
use crate::crypto::GameCrypto;

/// Ledger version for upgrade compatibility
pub const LEDGER_VERSION: u32 = 1;

/// Maximum transaction history per peer
pub const MAX_TRANSACTION_HISTORY: usize = 1000;

/// Checkpoint interval (every N transactions)
pub const CHECKPOINT_INTERVAL: u64 = 100;

/// Transaction types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    /// Initial token allocation
    Genesis { amount: CrapTokens },
    /// Mining reward
    Mining { amount: CrapTokens, block_height: u64 },
    /// Bet placed
    BetPlaced { amount: CrapTokens, game_id: [u8; 16] },
    /// Bet won
    BetWon { amount: CrapTokens, game_id: [u8; 16] },
    /// Bet lost
    BetLost { amount: CrapTokens, game_id: [u8; 16] },
    /// Transfer between peers
    Transfer { from: PeerId, to: PeerId, amount: CrapTokens },
    /// Treasury interaction
    TreasuryDeposit { amount: CrapTokens },
    TreasuryWithdraw { amount: CrapTokens },
}

/// Transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub id: Hash256,
    /// Transaction type and details
    pub tx_type: TransactionType,
    /// Timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Block height when included
    pub block_height: Option<u64>,
    /// Digital signature
    pub signature: Option<Vec<u8>>,
    /// Previous transaction hash (for chain)
    pub prev_hash: Hash256,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(tx_type: TransactionType, prev_hash: Hash256) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut tx = Self {
            id: [0; 32],
            tx_type,
            timestamp,
            block_height: None,
            signature: None,
            prev_hash,
        };
        
        tx.id = tx.calculate_hash();
        tx
    }
    
    /// Calculate transaction hash
    pub fn calculate_hash(&self) -> Hash256 {
        let mut data = Vec::new();
        data.extend_from_slice(&bincode::serialize(&self.tx_type).unwrap());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(&self.prev_hash);
        GameCrypto::hash(&data)
    }
    
    /// Verify transaction integrity
    pub fn verify(&self) -> bool {
        self.id == self.calculate_hash()
    }
}

/// Ledger checkpoint for fast recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerCheckpoint {
    /// Checkpoint version
    pub version: u32,
    /// Block height at checkpoint
    pub block_height: u64,
    /// Timestamp of checkpoint
    pub timestamp: u64,
    /// Balances at checkpoint
    pub balances: HashMap<PeerId, CrapTokens>,
    /// Treasury balance at checkpoint
    pub treasury_balance: CrapTokens,
    /// Merkle root of all transactions up to this point
    pub merkle_root: Hash256,
    /// Signature of checkpoint (for consensus)
    pub signatures: Vec<(Vec<u8>, Vec<u8>)>, // (public_key, signature)
}

/// Persistent token ledger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentLedger {
    /// Current version
    pub version: u32,
    
    /// Current balances
    pub balances: HashMap<PeerId, CrapTokens>,
    
    /// Treasury balance
    pub treasury_balance: CrapTokens,
    
    /// Transaction history (limited per peer)
    pub transactions: BTreeMap<u64, Vec<Transaction>>,
    
    /// Latest checkpoint
    pub last_checkpoint: Option<LedgerCheckpoint>,
    
    /// Current block height
    pub block_height: u64,
    
    /// Total tokens in circulation
    pub total_supply: CrapTokens,
    
    /// Merkle tree root of current state
    pub state_root: Hash256,
}

impl Default for PersistentLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentLedger {
    /// Create a new ledger
    pub fn new() -> Self {
        Self {
            version: LEDGER_VERSION,
            balances: HashMap::new(),
            treasury_balance: CrapTokens::new_unchecked(crate::protocol::treasury::INITIAL_TREASURY_BALANCE),
            transactions: BTreeMap::new(),
            last_checkpoint: None,
            block_height: 0,
            total_supply: CrapTokens::new_unchecked(crate::protocol::treasury::INITIAL_TREASURY_BALANCE),
            state_root: [0; 32],
        }
    }
    
    /// Load ledger from disk
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let data = fs::read(path).map_err(|e| Error::IoError(e.to_string()))?;
        let ledger: Self = bincode::deserialize(&data)
            .map_err(|e| Error::DeserializationError(e.to_string()))?;
        
        // Verify integrity
        if !ledger.verify_integrity() {
            return Err(Error::InvalidState("Ledger integrity check failed".into()));
        }
        
        Ok(ledger)
    }
    
    /// Save ledger to disk
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        let data = bincode::serialize(self)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        
        // Write to temporary file first
        let temp_path = path.as_ref().with_extension("tmp");
        fs::write(&temp_path, data).map_err(|e| Error::IoError(e.to_string()))?;
        
        // Atomic rename
        fs::rename(temp_path, path).map_err(|e| Error::IoError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Apply a transaction to the ledger
    pub fn apply_transaction(&mut self, tx: Transaction) -> Result<(), Error> {
        // Verify transaction
        if !tx.verify() {
            return Err(Error::InvalidTransaction("Transaction verification failed".into()));
        }
        
        // Apply based on type
        match &tx.tx_type {
            TransactionType::Genesis { amount } => {
                // Only allowed at block 0
                if self.block_height != 0 {
                    return Err(Error::InvalidTransaction("Genesis only allowed at block 0".into()));
                }
                self.total_supply = self.total_supply.checked_add(*amount)
                    .ok_or_else(|| Error::InvalidData("Total supply overflow".to_string()))?;
            },
            TransactionType::Mining { amount, .. } => {
                // Mint new tokens
                self.total_supply = self.total_supply.checked_add(*amount)
                    .ok_or_else(|| Error::InvalidData("Total supply overflow".to_string()))?;
            },
            TransactionType::Transfer { from, to, amount } => {
                // Check balance
                let from_balance = self.balances.get(from).copied()
                    .unwrap_or(CrapTokens::new_unchecked(0));
                if from_balance < *amount {
                    return Err(Error::InsufficientFunds(format!(
                        "Balance {} insufficient for transfer of {}", 
                        from_balance.0, amount.0
                    )));
                }
                
                // Execute transfer
                *self.balances.entry(*from).or_insert(CrapTokens::new_unchecked(0)) = 
                    from_balance.checked_sub(*amount)
                        .ok_or_else(|| Error::InsufficientFunds("Insufficient balance".into()))?;
                let current = self.balances.entry(*to).or_insert(CrapTokens::new_unchecked(0));
                *current = current.checked_add(*amount)
                    .ok_or_else(|| Error::InvalidData("Balance overflow".to_string()))?;
            },
            TransactionType::BetPlaced { amount, .. } => {
                // Move to treasury
                self.treasury_balance = self.treasury_balance.checked_add(*amount)
                    .ok_or_else(|| Error::InvalidData("Treasury balance overflow".to_string()))?;
            },
            TransactionType::BetWon { amount, .. } => {
                // Pay from treasury
                if self.treasury_balance < *amount {
                    return Err(Error::InsufficientFunds("Treasury balance insufficient".into()));
                }
                self.treasury_balance = self.treasury_balance.checked_sub(*amount)
                    .ok_or_else(|| Error::InsufficientFunds("Treasury balance insufficient".into()))?;
            },
            TransactionType::BetLost { .. } => {
                // Treasury keeps the funds
            },
            TransactionType::TreasuryDeposit { amount } => {
                self.treasury_balance = self.treasury_balance.checked_add(*amount)
                    .ok_or_else(|| Error::InvalidData("Treasury balance overflow".to_string()))?;
            },
            TransactionType::TreasuryWithdraw { amount } => {
                if self.treasury_balance < *amount {
                    return Err(Error::InsufficientFunds("Treasury balance insufficient".into()));
                }
                self.treasury_balance = self.treasury_balance.checked_sub(*amount)
                    .ok_or_else(|| Error::InsufficientFunds("Treasury balance insufficient".into()))?;
            },
        }
        
        // Store transaction
        self.transactions.entry(self.block_height)
            .or_default()
            .push(tx);
        
        // Update state root
        self.update_state_root();
        
        // Check if we need a checkpoint
        if self.block_height % CHECKPOINT_INTERVAL == 0 {
            self.create_checkpoint();
        }
        
        Ok(())
    }
    
    /// Create a checkpoint
    pub fn create_checkpoint(&mut self) {
        let checkpoint = LedgerCheckpoint {
            version: self.version,
            block_height: self.block_height,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            balances: self.balances.clone(),
            treasury_balance: self.treasury_balance,
            merkle_root: self.calculate_merkle_root(),
            signatures: Vec::new(), // Will be filled by consensus
        };
        
        self.last_checkpoint = Some(checkpoint);
        
        // Prune old transactions (keep only recent history)
        let cutoff = self.block_height.saturating_sub(MAX_TRANSACTION_HISTORY as u64);
        self.transactions = self.transactions.split_off(&cutoff);
    }
    
    /// Calculate Merkle root of all transactions
    fn calculate_merkle_root(&self) -> Hash256 {
        let mut hashes: Vec<Hash256> = Vec::new();
        
        for txs in self.transactions.values() {
            for tx in txs {
                hashes.push(tx.id);
            }
        }
        
        // Simple Merkle tree implementation
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in hashes.chunks(2) {
                let mut data = Vec::new();
                data.extend_from_slice(&chunk[0]);
                if chunk.len() > 1 {
                    data.extend_from_slice(&chunk[1]);
                } else {
                    data.extend_from_slice(&chunk[0]); // Duplicate if odd
                }
                next_level.push(GameCrypto::hash(&data));
            }
            hashes = next_level;
        }
        
        hashes.first().copied().unwrap_or([0; 32])
    }
    
    /// Update state root
    fn update_state_root(&mut self) {
        let mut data = Vec::new();
        
        // Include all balances
        for (peer, balance) in &self.balances {
            data.extend_from_slice(peer);
            data.extend_from_slice(&balance.amount().to_le_bytes());
        }
        
        // Include treasury
        data.extend_from_slice(&self.treasury_balance.amount().to_le_bytes());
        
        // Include block height
        data.extend_from_slice(&self.block_height.to_le_bytes());
        
        self.state_root = GameCrypto::hash(&data);
    }
    
    /// Verify ledger integrity
    pub fn verify_integrity(&self) -> bool {
        // Verify total supply matches sum of balances + treasury
        let total_in_wallets: u64 = self.balances.values()
            .map(|t| t.amount())
            .sum();
        let total = CrapTokens::new_unchecked(
            total_in_wallets.saturating_add(self.treasury_balance.amount())
        );
        
        if total.amount() != self.total_supply.amount() {
            return false;
        }
        
        // Verify transactions chain
        let mut prev_hash = [0; 32];
        for txs in self.transactions.values() {
            for tx in txs {
                if tx.prev_hash != prev_hash {
                    return false;
                }
                if !tx.verify() {
                    return false;
                }
                prev_hash = tx.id;
            }
        }
        
        true
    }
    
    /// Get balance for a peer
    pub fn get_balance(&self, peer: &PeerId) -> CrapTokens {
        self.balances.get(peer).copied()
            .unwrap_or(CrapTokens::new_unchecked(0))
    }
    
    /// Get transaction history for a peer
    pub fn get_peer_transactions(&self, peer: &PeerId) -> Vec<&Transaction> {
        let mut result = Vec::new();
        
        for txs in self.transactions.values() {
            for tx in txs {
                let involves_peer = match &tx.tx_type {
                    TransactionType::Transfer { from, to, .. } => from == peer || to == peer,
                    _ => false,
                };
                
                if involves_peer {
                    result.push(tx);
                }
            }
        }
        
        result
    }
}

/// Ledger synchronization protocol
pub struct LedgerSync {
    /// Local ledger
    ledger: Arc<RwLock<PersistentLedger>>,
    
    /// Storage path
    storage_path: PathBuf,
}

impl LedgerSync {
    /// Create new ledger sync
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Self {
        let ledger = if storage_path.as_ref().exists() {
            PersistentLedger::load(&storage_path).unwrap_or_else(|_| PersistentLedger::new())
        } else {
            PersistentLedger::new()
        };
        
        Self {
            ledger: Arc::new(RwLock::new(ledger)),
            storage_path: storage_path.as_ref().to_path_buf(),
        }
    }
    
    /// Get current state for synchronization
    pub fn get_sync_state(&self) -> (u64, Hash256) {
        let ledger = self.ledger.read().unwrap();
        (ledger.block_height, ledger.state_root)
    }
    
    /// Request missing blocks from peer
    pub fn get_blocks_since(&self, height: u64) -> Vec<Transaction> {
        let ledger = self.ledger.read().unwrap();
        let mut result = Vec::new();
        
        for (&block_height, txs) in ledger.transactions.range(height..) {
            if block_height > height {
                result.extend(txs.clone());
            }
        }
        
        result
    }
    
    /// Apply blocks received from peer
    pub fn apply_peer_blocks(&self, transactions: Vec<Transaction>) -> Result<(), Error> {
        let mut ledger = self.ledger.write().unwrap();
        
        for tx in transactions {
            ledger.apply_transaction(tx)?;
        }
        
        // Save to disk
        ledger.save(&self.storage_path)?;
        
        Ok(())
    }
    
    /// Persist current state
    pub fn persist(&self) -> Result<(), Error> {
        let ledger = self.ledger.read().unwrap();
        ledger.save(&self.storage_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new(
            TransactionType::Transfer {
                from: [1; 32],
                to: [2; 32],
                amount: CrapTokens::from(100),
            },
            [0; 32],
        );
        
        assert!(tx.verify());
        assert_eq!(tx.prev_hash, [0; 32]);
    }
    
    #[test]
    fn test_ledger_operations() {
        let mut ledger = PersistentLedger::new();
        
        // Initial state
        assert_eq!(ledger.treasury_balance, CrapTokens::from(crate::protocol::treasury::INITIAL_TREASURY_BALANCE));
        assert!(ledger.verify_integrity());
        
        // Apply a transfer
        let from = [1; 32];
        let to = [2; 32];
        
        // First give sender some balance
        ledger.balances.insert(from, CrapTokens::from(1000));
        ledger.total_supply = ledger.total_supply.saturating_add(CrapTokens::from(1000));
        
        let tx = Transaction::new(
            TransactionType::Transfer {
                from,
                to,
                amount: CrapTokens::from(500),
            },
            [0; 32],
        );
        
        assert!(ledger.apply_transaction(tx).is_ok());
        assert_eq!(ledger.get_balance(&from), CrapTokens::from(500));
        assert_eq!(ledger.get_balance(&to), CrapTokens::from(500));
        assert!(ledger.verify_integrity());
    }
    
    #[test]
    fn test_checkpoint_creation() {
        let mut ledger = PersistentLedger::new();
        
        ledger.block_height = CHECKPOINT_INTERVAL;
        ledger.create_checkpoint();
        
        assert!(ledger.last_checkpoint.is_some());
        let checkpoint = ledger.last_checkpoint.as_ref().unwrap();
        assert_eq!(checkpoint.block_height, CHECKPOINT_INTERVAL);
        assert_eq!(checkpoint.treasury_balance, ledger.treasury_balance);
    }
}