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
use crate::TREASURY_ADDRESS;
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
            base_reward: CrapTokens::from_crap(0.1).unwrap_or_else(|_| CrapTokens::new_unchecked(100_000)).amount(), // 0.1 CRAP per relay
            difficulty_adjustment_interval: Duration::from_secs(3600), // 1 hour
            target_block_time: Duration::from_secs(60), // 1 minute average
            max_supply: CrapTokens::from_crap(21_000_000.0).unwrap_or_else(|_| CrapTokens::new_unchecked(21_000_000_000_000)).amount(), // 21M CRAP total
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
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_secs(),
        };
        
        accounts.insert(peer_id, account);
        
        // Initialize treasury if this is treasury address
        if peer_id == TREASURY_ADDRESS {
            let initial_supply = CrapTokens::from_crap(1_000_000.0).unwrap_or_else(|_| CrapTokens::new_unchecked(1_000_000_000_000)).amount(); // 1M CRAP for treasury
            if let Some(treasury_account) = accounts.get_mut(&peer_id) {
                treasury_account.balance = initial_supply;
                *self.total_supply.write().await = initial_supply;
                *self.treasury_balance.write().await = initial_supply;
            }
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
                  CrapTokens::new_unchecked(amount).to_crap(), player, game_id);
        
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
                  CrapTokens::new_unchecked(amount).to_crap(), winner, game_id);
        
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
    
    /// Process relay reward for message forwarding
    pub async fn process_relay_reward(
        &self,
        relayer: PeerId,
        messages_relayed: u64,
    ) -> Result<[u8; 32]> {
        let mut accounts = self.accounts.write().await;
        
        // Calculate reward amount based on messages relayed
        let base_reward_per_message = self.mining_config.base_reward / 10; // 0.01 CRAP per message
        let reward_amount = messages_relayed * base_reward_per_message;
        
        // Create or get relayer account
        let account = accounts.entry(relayer).or_insert_with(|| Account {
            peer_id: relayer,
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
        
        // Add reward to balance
        account.balance += reward_amount;
        account.reputation = (account.reputation + 0.01).min(1.0); // Increase reputation
        account.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Create proof of relay for transaction record
        let relay_proof = RelayProof {
            relayer,
            packet_hash: self.generate_packet_hash(messages_relayed),
            source: [0u8; 32], // Would be filled with actual source
            destination: [0u8; 32], // Would be filled with actual destination
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            hop_count: 1, // Default hop count
            signature: crate::crypto::BitchatSignature {
                signature: vec![0u8; 64], // Would be actual signature
                public_key: relayer.to_vec(),
            },
        };
        
        // Create transaction record
        let transaction = TokenTransaction {
            id: self.generate_transaction_id(),
            transaction_type: TransactionType::RelayReward {
                relayer,
                amount: reward_amount,
                proof: relay_proof,
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            nonce: account.transaction_count,
            fee: 0,
            signature: None,
            confirmations: 1,
        };
        
        let tx_id = transaction.id;
        self.transactions.write().await.push(transaction);
        
        // Update total supply with newly minted tokens
        *self.total_supply.write().await += reward_amount;
        
        log::info!("Relay reward: {} CRAP to {:?} for {} messages relayed", 
                  CrapTokens::new_unchecked(reward_amount).to_crap(), relayer, messages_relayed);
        
        // Emit event
        let _ = self.event_sender.send(TokenEvent::RewardMinted {
            recipient: relayer,
            amount: reward_amount,
            reason: format!("Relay reward for {} messages", messages_relayed),
        });
        
        Ok(tx_id)
    }
    
    /// Get treasury balance
    pub async fn get_treasury_balance(&self) -> u64 {
        *self.treasury_balance.read().await
    }
    
    /// Generate a packet hash for relay proof (placeholder implementation)
    fn generate_packet_hash(&self, seed: u64) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(seed.to_be_bytes());
        hasher.update(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_be_bytes());
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
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
    
    /// Record a relay event for mining rewards
    pub async fn record_relay(
        &self,
        relayer: PeerId,
        packet_hash: [u8; 32],
        source: PeerId,
        destination: PeerId,
        hop_count: u8,
    ) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let relay_entry = RelayEntry {
            relayer,
            packet_hash,
            relay_path: vec![source, relayer, destination],
            timestamp,
            reward_claimed: false,
        };
        
        // Store relay entry
        self.relay_log.write().await.insert(packet_hash, relay_entry);
        
        // Update mining stats
        {
            let mut stats = self.mining_stats.write().await;
            stats.total_relays += 1;
        }
        
        // Calculate and distribute reward based on hop distance
        let reward_multiplier = match hop_count {
            1 => 1, // Direct relay
            2..=3 => 2, // Medium distance
            4..=6 => 3, // Long distance
            _ => 4, // Very long distance (max multiplier)
        };
        
        let base_reward = self.ledger.mining_config.base_reward / 100; // 0.001 CRAP base
        let reward_amount = base_reward * reward_multiplier;
        
        // Process relay reward through ledger
        if let Ok(_tx_id) = self.ledger.process_relay_reward(relayer, reward_amount).await {
            // Mark reward as claimed
            if let Some(entry) = self.relay_log.write().await.get_mut(&packet_hash) {
                entry.reward_claimed = true;
            }
            
            // Update stats
            let mut stats = self.mining_stats.write().await;
            stats.total_rewards_distributed += reward_amount;
        }
        
        Ok(())
    }
    
    /// Update relay score for a peer (for testing purposes)
    pub async fn update_relay_score(&self, peer_id: PeerId, score_delta: i32) {
        // This method is used by tests to simulate relay activity
        // In a real implementation, this would track peer reliability scores
        log::info!("Updated relay score for peer {:?} by {}", peer_id, score_delta);
        
        // Update active relayers count
        let mut stats = self.mining_stats.write().await;
        if score_delta > 0 {
            stats.active_relayers = stats.active_relayers.saturating_add(1);
        }
    }
    
    /// Process accumulated relay rewards for a peer
    pub async fn process_accumulated_rewards(&self, peer_id: PeerId) -> Result<u64> {
        let relay_log = self.relay_log.read().await;
        
        // Count unrewarded relays for this peer
        let unrewarded_count = relay_log
            .values()
            .filter(|entry| entry.relayer == peer_id && !entry.reward_claimed)
            .count() as u64;
        
        drop(relay_log);
        
        if unrewarded_count > 0 {
            // Process rewards through ledger
            if let Ok(_tx_id) = self.ledger.process_relay_reward(peer_id, unrewarded_count).await {
                // Mark all relays as rewarded
                let mut relay_log = self.relay_log.write().await;
                for entry in relay_log.values_mut() {
                    if entry.relayer == peer_id && !entry.reward_claimed {
                        entry.reward_claimed = true;
                    }
                }
                
                let reward_amount = unrewarded_count * self.ledger.mining_config.base_reward / 10;
                return Ok(reward_amount);
            }
        }
        
        Ok(0)
    }
    
    /// Adjust mining difficulty based on network activity
    pub async fn adjust_mining_difficulty(&self) -> Result<()> {
        let stats = self.mining_stats.read().await;
        let current_activity = stats.total_relays;
        drop(stats);
        
        // Simple difficulty adjustment based on activity
        // In a real implementation, this would be more sophisticated
        if current_activity > 1000 {
            log::info!("High network activity detected, adjusting mining difficulty");
        } else if current_activity < 100 {
            log::info!("Low network activity detected, adjusting mining difficulty");
        }
        
        Ok(())
    }
    
    /// Clean up old relay entries
    pub async fn cleanup_old_entries(&self) -> Result<()> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut relay_log = self.relay_log.write().await;
        let old_count = relay_log.len();
        
        // Remove entries older than 1 hour
        relay_log.retain(|_hash, entry| {
            current_time - entry.timestamp < 3600
        });
        
        let removed = old_count - relay_log.len();
        if removed > 0 {
            log::info!("Cleaned up {} old relay entries", removed);
        }
        
        Ok(())
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