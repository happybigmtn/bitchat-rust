//! Token economics for BitCraps
//! 
//! This module implements the CRAP token system including:
//! - Token ledger and balance management
//! - Proof-of-relay mining rewards
//! - Treasury management and liquidity provision
//! - Transaction validation and consensus
//! - Staking and reward distribution

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use crate::protocol::{PeerId, GameId, CrapTokens};
use crate::gaming::TREASURY_ADDRESS;
use crate::crypto::BitchatSignature;
use crate::error::{Error, Result};

/// Token transaction types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Transfer { from: PeerId, to: PeerId, amount: u64 },
    GameBet { player: PeerId, game_id: GameId, amount: u64, bet_type: u8 },
    GamePayout { winner: PeerId, game_id: GameId, amount: u64 },
    RelayReward { relayer: PeerId, amount: u64, proof: RelayProof },
    TreasuryDeposit { from: PeerId, amount: u64 },
    TreasuryWithdraw { to: PeerId, amount: u64 },
    Mint { to: PeerId, amount: u64, reason: String },
}

/// Proof-of-relay evidence for mining rewards
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelayProof {
    pub relayer: PeerId,
    pub packet_hash: [u8; 32],
    pub source: PeerId,
    pub destination: PeerId,
    pub timestamp: u64,
    pub hop_count: u8,
    pub signature: BitchatSignature,
}

/// Token transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenTransaction {
    pub id: [u8; 32],
    pub transaction_type: TransactionType,
    pub timestamp: u64,
    pub nonce: u64,
    pub fee: u64,
    pub signature: Option<BitchatSignature>,
    pub confirmations: u32,
}

/// Account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub peer_id: PeerId,
    pub balance: u64,
    pub staked_amount: u64,
    pub pending_rewards: u64,
    pub transaction_count: u64,
    pub reputation: f64,
    pub last_activity: u64,
}

/// Staking position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingPosition {
    pub staker: PeerId,
    pub amount: u64,
    pub staked_at: u64,
    pub lock_duration: Duration,
    pub reward_rate: f64,
    pub accumulated_rewards: u64,
}

/// Token ledger managing all CRAP tokens
#[allow(dead_code)]
pub struct TokenLedger {
    accounts: Arc<RwLock<HashMap<PeerId, Account>>>,
    transactions: Arc<RwLock<Vec<TokenTransaction>>>,
    staking_positions: Arc<RwLock<HashMap<PeerId, StakingPosition>>>,
    pending_transactions: Arc<RwLock<HashMap<[u8; 32], TokenTransaction>>>,
    total_supply: Arc<RwLock<u64>>,
    treasury_balance: Arc<RwLock<u64>>,
    mining_config: MiningConfig,
    event_sender: mpsc::UnboundedSender<TokenEvent>,
}

/// Mining configuration
#[derive(Debug, Clone)]
pub struct MiningConfig {
    pub base_reward: u64,
    pub difficulty_adjustment_interval: Duration,
    pub target_block_time: Duration,
    pub max_supply: u64,
    pub halving_interval: u64, // Number of transactions
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            base_reward: CrapTokens::from_crap(0.1).amount(), // 0.1 CRAP per relay
            difficulty_adjustment_interval: Duration::from_secs(3600), // 1 hour
            target_block_time: Duration::from_secs(60), // 1 minute average
            max_supply: CrapTokens::from_crap(21_000_000.0).amount(), // 21M CRAP total
            halving_interval: 210_000, // Halve rewards every 210k transactions
        }
    }
}

/// Token events
#[derive(Debug, Clone)]
pub enum TokenEvent {
    TransactionSubmitted { tx_id: [u8; 32], transaction: TokenTransaction },
    TransactionConfirmed { tx_id: [u8; 32] },
    RewardMinted { recipient: PeerId, amount: u64, reason: String },
    BalanceUpdated { peer_id: PeerId, old_balance: u64, new_balance: u64 },
    StakingPositionCreated { staker: PeerId, amount: u64 },
    RewardsDistributed { total_amount: u64, recipient_count: usize },
}

/// Proof-of-relay mining system
#[allow(dead_code)]
pub struct ProofOfRelay {
    ledger: Arc<TokenLedger>,
    relay_log: Arc<RwLock<HashMap<[u8; 32], RelayEntry>>>,
    mining_stats: Arc<RwLock<MiningStats>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RelayEntry {
    relayer: PeerId,
    packet_hash: [u8; 32],
    relay_path: Vec<PeerId>,
    timestamp: u64,
    reward_claimed: bool,
}

#[derive(Debug, Clone)]
pub struct MiningStats {
    pub total_relays: u64,
    pub total_rewards_distributed: u64,
    pub active_relayers: u64,
    pub average_relay_time: Duration,
}

impl TokenLedger {
    pub fn new() -> Self {
        let (event_sender, _) = mpsc::unbounded_channel();
        
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(Vec::new())),
            staking_positions: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            total_supply: Arc::new(RwLock::new(0)),
            treasury_balance: Arc::new(RwLock::new(0)),
            mining_config: MiningConfig::default(),
            event_sender,
        }
    }
    
    /// Get account balance
    pub async fn get_balance(&self, peer_id: &PeerId) -> u64 {
        if let Some(account) = self.accounts.read().await.get(peer_id) {
            account.balance
        } else {
            0
        }
    }
    
    /// Create new account
    pub async fn create_account(&self, peer_id: PeerId) -> Result<()> {
        let mut accounts = self.accounts.write().await;
        
        if accounts.contains_key(&peer_id) {
            return Err(Error::Protocol("Account already exists".to_string()));
        }
        
        let account = Account {
            peer_id,
            balance: 0,
            staked_amount: 0,
            pending_rewards: 0,
            transaction_count: 0,
            reputation: 0.5, // Start with neutral reputation
            last_activity: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        accounts.insert(peer_id, account);
        
        // Initialize treasury if this is treasury address
        if peer_id == TREASURY_ADDRESS {
            let initial_supply = CrapTokens::from_crap(1_000_000.0).amount(); // 1M CRAP for treasury
            accounts.get_mut(&peer_id).unwrap().balance = initial_supply;
            *self.total_supply.write().await = initial_supply;
            *self.treasury_balance.write().await = initial_supply;
        }
        
        log::info!("Created account for peer: {:?}", peer_id);
        Ok(())
    }
    
    /// Process game bet transaction
    pub async fn process_game_bet(
        &self,
        player: PeerId,
        amount: u64,
        game_id: GameId,
        bet_type: u8,
    ) -> Result<[u8; 32]> {
        let mut accounts = self.accounts.write().await;
        
        // Check player balance
        let account = accounts.get_mut(&player)
            .ok_or_else(|| Error::Protocol("Player account not found".to_string()))?;
        
        if account.balance < amount {
            return Err(Error::Protocol("Insufficient balance for bet".to_string()));
        }
        
        // Deduct bet amount from player
        account.balance -= amount;
        account.transaction_count += 1;
        
        // Create transaction record
        let transaction = TokenTransaction {
            id: self.generate_transaction_id(),
            transaction_type: TransactionType::GameBet {
                player,
                game_id,
                amount,
                bet_type,
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            nonce: account.transaction_count,
            fee: 0,
            signature: None, // Game bets are pre-authorized
            confirmations: 1,
        };
        
        let tx_id = transaction.id;
        self.transactions.write().await.push(transaction);
        
        log::info!("Game bet: {} CRAP from {:?} for game {:?}", 
                  CrapTokens::new(amount).to_crap(), player, game_id);
        
        Ok(tx_id)
    }
    
    /// Process game payout
    pub async fn process_game_payout(
        &self,
        winner: PeerId,
        game_id: GameId,
        amount: u64,
    ) -> Result<[u8; 32]> {
        let mut accounts = self.accounts.write().await;
        
        // Add winnings to player account
        let account = accounts.entry(winner).or_insert_with(|| Account {
            peer_id: winner,
            balance: 0,
            staked_amount: 0,
            pending_rewards: 0,
            transaction_count: 0,
            reputation: 0.5,
            last_activity: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        });
        
        account.balance += amount;
        
        // Create transaction record
        let transaction = TokenTransaction {
            id: self.generate_transaction_id(),
            transaction_type: TransactionType::GamePayout {
                winner,
                game_id,
                amount,
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            nonce: 0, // Payouts don't use account nonces
            fee: 0,
            signature: None,
            confirmations: 1,
        };
        
        let tx_id = transaction.id;
        self.transactions.write().await.push(transaction);
        
        log::info!("Game payout: {} CRAP to {:?} from game {:?}", 
                  CrapTokens::new(amount).to_crap(), winner, game_id);
        
        Ok(tx_id)
    }
    
    /// Generate unique transaction ID
    fn generate_transaction_id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_be_bytes());
        hasher.update(&rand::random::<[u8; 16]>());
        
        let result = hasher.finalize();
        let mut tx_id = [0u8; 32];
        tx_id.copy_from_slice(&result);
        tx_id
    }
    
    /// Get ledger statistics
    pub async fn get_stats(&self) -> LedgerStats {
        let accounts = self.accounts.read().await;
        let transactions = self.transactions.read().await;
        let positions = self.staking_positions.read().await;
        
        LedgerStats {
            total_accounts: accounts.len(),
            total_transactions: transactions.len(),
            total_supply: *self.total_supply.read().await,
            treasury_balance: *self.treasury_balance.read().await,
            total_staked: positions.values().map(|p| p.amount).sum(),
            active_stakers: positions.len(),
        }
    }
    
    /// Distribute staking rewards (simplified)
    pub async fn distribute_staking_rewards(&self) -> Result<()> {
        // Simplified implementation - in production would be more complex
        Ok(())
    }
}

impl ProofOfRelay {
    pub fn new(ledger: Arc<TokenLedger>) -> Self {
        Self {
            ledger,
            relay_log: Arc::new(RwLock::new(HashMap::new())),
            mining_stats: Arc::new(RwLock::new(MiningStats {
                total_relays: 0,
                total_rewards_distributed: 0,
                active_relayers: 0,
                average_relay_time: Duration::from_secs(0),
            })),
        }
    }
    
    /// Get mining statistics
    pub async fn get_stats(&self) -> MiningStats {
        self.mining_stats.read().await.clone()
    }
}

/// Ledger statistics
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_accounts: usize,
    pub total_transactions: usize,
    pub total_supply: u64,
    pub treasury_balance: u64,
    pub total_staked: u64,
    pub active_stakers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_ledger_creation() {
        let ledger = TokenLedger::new();
        
        // Create treasury account
        ledger.create_account(TREASURY_ADDRESS).await.unwrap();
        
        let treasury_balance = ledger.get_balance(&TREASURY_ADDRESS).await;
        assert!(treasury_balance > 0);
        
        let stats = ledger.get_stats().await;
        assert_eq!(stats.total_accounts, 1); // Just treasury
    }
}