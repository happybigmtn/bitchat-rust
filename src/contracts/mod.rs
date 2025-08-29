//! Smart Contract Integration Module
//! 
//! This module provides interfaces and implementations for integrating with
//! various blockchain smart contracts, enabling cross-chain operations and
//! bridging for the BitCraps ecosystem:
//! - ERC-20/BEP-20 token contract interfaces
//! - Staking contract integration with automated rewards
//! - Treasury contract management and governance
//! - Cross-chain bridge contracts for multi-chain operations
//! - Automated market maker contract integration
//! - Oracle price feeds and external data sources

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::protocol::{CrapTokens, Hash256};
use crate::error::{Error, Result};
use crate::treasury::TreasuryManager;
use crate::economics::TokenEconomics;

pub mod token_contracts;
pub mod staking_contracts;
pub mod bridge_contracts;
pub mod oracle_integration;

/// Supported blockchain networks
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum BlockchainNetwork {
    Ethereum,
    BinanceSmartChain,
    Polygon,
    Arbitrum,
    Optimism,
    Avalanche,
    Bitcoin,    // For Bitcoin bridge
    BitcoinCash,
    Litecoin,
}

impl BlockchainNetwork {
    pub fn chain_id(&self) -> u64 {
        match self {
            BlockchainNetwork::Ethereum => 1,
            BlockchainNetwork::BinanceSmartChain => 56,
            BlockchainNetwork::Polygon => 137,
            BlockchainNetwork::Arbitrum => 42161,
            BlockchainNetwork::Optimism => 10,
            BlockchainNetwork::Avalanche => 43114,
            BlockchainNetwork::Bitcoin => 0,
            BlockchainNetwork::BitcoinCash => 0,
            BlockchainNetwork::Litecoin => 0,
        }
    }
    
    pub fn native_currency(&self) -> &'static str {
        match self {
            BlockchainNetwork::Ethereum => "ETH",
            BlockchainNetwork::BinanceSmartChain => "BNB",
            BlockchainNetwork::Polygon => "MATIC",
            BlockchainNetwork::Arbitrum => "ETH",
            BlockchainNetwork::Optimism => "ETH",
            BlockchainNetwork::Avalanche => "AVAX",
            BlockchainNetwork::Bitcoin => "BTC",
            BlockchainNetwork::BitcoinCash => "BCH",
            BlockchainNetwork::Litecoin => "LTC",
        }
    }
}

/// Smart contract interface definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInterface {
    pub contract_address: String,
    pub network: BlockchainNetwork,
    pub abi: Value, // Contract ABI in JSON format
    pub bytecode: Option<String>,
    pub constructor_args: Option<Vec<Value>>,
    pub deployed_block: Option<u64>,
    pub verification_status: VerificationStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Formal, // Formal verification completed
}

/// Token contract interface for ERC-20/BEP-20 tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenContract {
    pub interface: ContractInterface,
    pub token_name: String,
    pub token_symbol: String,
    pub decimals: u8,
    pub total_supply: u64,
    pub mint_capability: bool,
    pub burn_capability: bool,
    pub pause_capability: bool,
    pub upgrade_capability: bool,
    pub owner: Option<String>,
    pub admin_roles: Vec<String>,
}

/// Staking contract interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingContract {
    pub interface: ContractInterface,
    pub staking_token: String,
    pub reward_token: String,
    pub min_stake_amount: u64,
    pub lock_periods: Vec<u64>,
    pub reward_rates: HashMap<u64, f64>, // lock_period -> APY
    pub penalty_rates: HashMap<u64, f64>, // lock_period -> penalty
    pub compound_frequency: u64,
    pub total_staked: u64,
    pub total_rewards_distributed: u64,
    pub emergency_withdrawal: bool,
}

/// Bridge contract for cross-chain operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeContract {
    pub interface: ContractInterface,
    pub source_network: BlockchainNetwork,
    pub target_networks: Vec<BlockchainNetwork>,
    pub supported_tokens: Vec<String>,
    pub bridge_fee: f64,
    pub min_bridge_amount: u64,
    pub max_bridge_amount: u64,
    pub confirmation_blocks: HashMap<BlockchainNetwork, u64>,
    pub validator_threshold: u8,
    pub validator_addresses: Vec<String>,
    pub pause_status: bool,
}

/// Oracle price feed integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePriceFeed {
    pub oracle_address: String,
    pub network: BlockchainNetwork,
    pub price_pair: String, // e.g., "CRAP/USD"
    pub decimals: u8,
    pub update_frequency: Duration,
    pub last_update: u64,
    pub price_deviation_threshold: f64,
    pub heartbeat_interval: Duration,
    pub aggregation_method: AggregationMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationMethod {
    Median,
    Average,
    Weighted,
    TWAP, // Time-Weighted Average Price
}

/// Contract transaction for cross-chain operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTransaction {
    pub tx_id: Hash256,
    pub network: BlockchainNetwork,
    pub contract_address: String,
    pub function_name: String,
    pub parameters: Vec<Value>,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub value: u64, // ETH/BNB value sent
    pub nonce: u64,
    pub status: TransactionStatus,
    pub confirmations: u64,
    pub block_number: Option<u64>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Cancelled,
}

/// Bridge operation for cross-chain transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeOperation {
    pub operation_id: Hash256,
    pub source_network: BlockchainNetwork,
    pub target_network: BlockchainNetwork,
    pub source_tx: Hash256,
    pub target_tx: Option<Hash256>,
    pub token_address: String,
    pub amount: u64,
    pub sender: String,
    pub recipient: String,
    pub bridge_fee: u64,
    pub status: BridgeStatus,
    pub created_at: u64,
    pub confirmed_at: Option<u64>,
    pub validator_signatures: Vec<ValidatorSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeStatus {
    Initiated,
    Validated,
    Minted,
    Completed,
    Failed,
    Disputed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSignature {
    pub validator: String,
    pub signature: Vec<u8>,
    pub timestamp: u64,
}

/// Main contract integration manager
pub struct ContractManager {
    treasury: Arc<TreasuryManager>,
    economics: Arc<TokenEconomics>,
    token_contracts: Arc<RwLock<HashMap<String, TokenContract>>>,
    staking_contracts: Arc<RwLock<HashMap<String, StakingContract>>>,
    bridge_contracts: Arc<RwLock<HashMap<String, BridgeContract>>>,
    oracle_feeds: Arc<RwLock<HashMap<String, OraclePriceFeed>>>,
    pending_transactions: Arc<RwLock<HashMap<Hash256, ContractTransaction>>>,
    bridge_operations: Arc<RwLock<HashMap<Hash256, BridgeOperation>>>,
    rpc_endpoints: Arc<RwLock<HashMap<BlockchainNetwork, String>>>,
}

impl ContractManager {
    /// Create new contract integration manager
    pub fn new(treasury: Arc<TreasuryManager>, economics: Arc<TokenEconomics>) -> Self {
        Self {
            treasury,
            economics,
            token_contracts: Arc::new(RwLock::new(HashMap::new())),
            staking_contracts: Arc::new(RwLock::new(HashMap::new())),
            bridge_contracts: Arc::new(RwLock::new(HashMap::new())),
            oracle_feeds: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            bridge_operations: Arc::new(RwLock::new(HashMap::new())),
            rpc_endpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Deploy token contract on specified network
    pub async fn deploy_token_contract(
        &self,
        network: BlockchainNetwork,
        token_name: String,
        token_symbol: String,
        initial_supply: u64,
        decimals: u8,
    ) -> Result<String> {
        // In a real implementation, this would:
        // 1. Compile the contract bytecode
        // 2. Estimate gas costs
        // 3. Submit deployment transaction
        // 4. Wait for confirmation
        // 5. Verify contract on block explorer
        
        let contract_address = self.generate_contract_address(&network).await;
        
        let interface = ContractInterface {
            contract_address: contract_address.clone(),
            network: network.clone(),
            abi: self.get_erc20_abi(),
            bytecode: Some(self.get_erc20_bytecode()),
            constructor_args: Some(vec![
                Value::String(token_name.clone()),
                Value::String(token_symbol.clone()),
                Value::Number(serde_json::Number::from(initial_supply)),
                Value::Number(serde_json::Number::from(decimals)),
            ]),
            deployed_block: Some(self.get_current_block_number(&network).await),
            verification_status: VerificationStatus::Unverified,
        };
        
        let token_contract = TokenContract {
            interface,
            token_name: token_name.clone(),
            token_symbol: token_symbol.clone(),
            decimals,
            total_supply: initial_supply,
            mint_capability: true,
            burn_capability: true,
            pause_capability: true,
            upgrade_capability: false, // Immutable contract
            owner: Some("0x1234...".to_string()), // Treasury multisig
            admin_roles: vec!["0x5678...".to_string()], // Additional admins
        };
        
        self.token_contracts.write().await.insert(contract_address.clone(), token_contract);
        
        log::info!("Deployed {} token contract on {:?} at address {}",
                  token_name, network, contract_address);
        
        Ok(contract_address)
    }
    
    /// Create staking contract with reward mechanisms
    pub async fn deploy_staking_contract(
        &self,
        network: BlockchainNetwork,
        staking_token: String,
        reward_token: String,
        reward_rates: HashMap<u64, f64>,
    ) -> Result<String> {
        let contract_address = self.generate_contract_address(&network).await;
        
        let interface = ContractInterface {
            contract_address: contract_address.clone(),
            network: network.clone(),
            abi: self.get_staking_abi(),
            bytecode: Some(self.get_staking_bytecode()),
            constructor_args: Some(vec![
                Value::String(staking_token.clone()),
                Value::String(reward_token.clone()),
            ]),
            deployed_block: Some(self.get_current_block_number(&network).await),
            verification_status: VerificationStatus::Unverified,
        };
        
        let mut penalty_rates = HashMap::new();
        for &lock_period in reward_rates.keys() {
            // Higher rewards = higher penalties for early withdrawal
            let penalty = match lock_period {
                86400..=604800 => 0.05,      // 5% for 1-7 days
                604801..=2592000 => 0.10,    // 10% for 1 week - 1 month
                2592001..=31536000 => 0.15,  // 15% for 1 month - 1 year
                _ => 0.20,                   // 20% for over 1 year
            };
            penalty_rates.insert(lock_period, penalty);
        }
        
        let staking_contract = StakingContract {
            interface,
            staking_token,
            reward_token,
            min_stake_amount: 1_000_000_000, // 1 CRAP minimum
            lock_periods: reward_rates.keys().copied().collect(),
            reward_rates,
            penalty_rates,
            compound_frequency: 86400, // Daily compounding
            total_staked: 0,
            total_rewards_distributed: 0,
            emergency_withdrawal: false,
        };
        
        self.staking_contracts.write().await.insert(contract_address.clone(), staking_contract);
        
        log::info!("Deployed staking contract on {:?} at address {}",
                  network, contract_address);
        
        Ok(contract_address)
    }
    
    /// Create bridge contract for cross-chain transfers
    pub async fn deploy_bridge_contract(
        &self,
        source_network: BlockchainNetwork,
        target_networks: Vec<BlockchainNetwork>,
        supported_tokens: Vec<String>,
        validator_addresses: Vec<String>,
    ) -> Result<String> {
        let contract_address = self.generate_contract_address(&source_network).await;
        
        let interface = ContractInterface {
            contract_address: contract_address.clone(),
            network: source_network.clone(),
            abi: self.get_bridge_abi(),
            bytecode: Some(self.get_bridge_bytecode()),
            constructor_args: Some(vec![
                Value::Array(validator_addresses.iter().map(|v| Value::String(v.clone())).collect()),
                Value::Number(serde_json::Number::from(2)), // 2/3 threshold
            ]),
            deployed_block: Some(self.get_current_block_number(&source_network).await),
            verification_status: VerificationStatus::Unverified,
        };
        
        let mut confirmation_blocks = HashMap::new();
        for network in &target_networks {
            let confirmations = match network {
                BlockchainNetwork::Ethereum => 12,
                BlockchainNetwork::BinanceSmartChain => 15,
                BlockchainNetwork::Polygon => 20,
                BlockchainNetwork::Bitcoin => 6,
                _ => 10,
            };
            confirmation_blocks.insert(network.clone(), confirmations);
        }
        
        let bridge_contract = BridgeContract {
            interface,
            source_network: source_network.clone(),
            target_networks,
            supported_tokens,
            bridge_fee: 0.001, // 0.1% bridge fee
            min_bridge_amount: 1_000_000,   // 0.001 CRAP minimum
            max_bridge_amount: 1_000_000_000_000, // 1M CRAP maximum
            confirmation_blocks,
            validator_threshold: ((validator_addresses.len() * 2) / 3) as u8 + 1,
            validator_addresses,
            pause_status: false,
        };
        
        let target_networks_len = bridge_contract.target_networks.len();
        self.bridge_contracts.write().await.insert(contract_address.clone(), bridge_contract);
        
        log::info!("Deployed bridge contract on {:?} at address {} supporting {} networks",
                  source_network, contract_address, target_networks_len);
        
        Ok(contract_address)
    }
    
    /// Initiate cross-chain bridge transfer
    pub async fn bridge_tokens(
        &self,
        bridge_contract_address: String,
        target_network: BlockchainNetwork,
        token_address: String,
        amount: u64,
        recipient: String,
    ) -> Result<Hash256> {
        let bridge_contracts = self.bridge_contracts.read().await;
        let bridge_contract = bridge_contracts.get(&bridge_contract_address)
            .ok_or_else(|| Error::InvalidData("Bridge contract not found".to_string()))?;
        
        // Validate bridge operation
        if amount < bridge_contract.min_bridge_amount {
            return Err(Error::InvalidData("Amount below minimum".to_string()));
        }
        
        if amount > bridge_contract.max_bridge_amount {
            return Err(Error::InvalidData("Amount above maximum".to_string()));
        }
        
        if !bridge_contract.target_networks.contains(&target_network) {
            return Err(Error::InvalidData("Target network not supported".to_string()));
        }
        
        let operation_id = self.generate_operation_id();
        let bridge_fee = (amount as f64 * bridge_contract.bridge_fee) as u64;
        
        // Create contract transaction to lock tokens
        let lock_tx = ContractTransaction {
            tx_id: self.generate_tx_id(),
            network: bridge_contract.source_network.clone(),
            contract_address: bridge_contract_address.clone(),
            function_name: "lockTokens".to_string(),
            parameters: vec![
                Value::String(token_address.clone()),
                Value::Number(serde_json::Number::from(amount)),
                Value::String(recipient.clone()),
                Value::String(format!("{:?}", target_network)),
            ],
            gas_limit: 300000,
            gas_price: 20_000_000_000, // 20 gwei
            value: 0,
            nonce: 0, // Would be fetched from wallet
            status: TransactionStatus::Pending,
            confirmations: 0,
            block_number: None,
            timestamp: Self::current_timestamp(),
        };
        
        // Create bridge operation
        let bridge_operation = BridgeOperation {
            operation_id,
            source_network: bridge_contract.source_network.clone(),
            target_network: target_network.clone(),
            source_tx: lock_tx.tx_id,
            target_tx: None,
            token_address,
            amount: amount - bridge_fee,
            sender: "user_address".to_string(), // Would be provided by user
            recipient,
            bridge_fee,
            status: BridgeStatus::Initiated,
            created_at: Self::current_timestamp(),
            confirmed_at: None,
            validator_signatures: Vec::new(),
        };
        
        // Store pending transaction and bridge operation
        self.pending_transactions.write().await.insert(lock_tx.tx_id, lock_tx);
        self.bridge_operations.write().await.insert(operation_id, bridge_operation);
        
        log::info!("Initiated bridge operation {} for {} tokens to {:?}",
                  hex::encode(&operation_id[..8]),
                  CrapTokens::from(amount).to_crap(),
                  target_network);
        
        Ok(operation_id)
    }
    
    /// Set up oracle price feed
    pub async fn setup_oracle_feed(
        &self,
        network: BlockchainNetwork,
        oracle_address: String,
        price_pair: String,
        update_frequency: Duration,
    ) -> Result<()> {
        let feed = OraclePriceFeed {
            oracle_address: oracle_address.clone(),
            network: network.clone(),
            price_pair: price_pair.clone(),
            decimals: 18, // Standard for price feeds
            update_frequency,
            last_update: 0,
            price_deviation_threshold: 0.05, // 5% deviation triggers update
            heartbeat_interval: Duration::from_secs(3600), // 1 hour heartbeat
            aggregation_method: AggregationMethod::Median,
        };
        
        self.oracle_feeds.write().await.insert(oracle_address.clone(), feed);
        
        log::info!("Set up oracle price feed for {} on {:?} at {}",
                  price_pair, network, oracle_address);
        
        Ok(())
    }
    
    /// Get latest price from oracle
    pub async fn get_oracle_price(&self, oracle_address: &str) -> Result<(f64, u64)> {
        let feeds = self.oracle_feeds.read().await;
        let feed = feeds.get(oracle_address)
            .ok_or_else(|| Error::InvalidData("Oracle feed not found".to_string()))?;
        
        // In a real implementation, this would call the oracle contract
        let mock_price = self.fetch_mock_price(&feed.price_pair).await;
        let timestamp = Self::current_timestamp();
        
        Ok((mock_price, timestamp))
    }
    
    /// Get comprehensive contract statistics
    pub async fn get_contract_stats(&self) -> ContractStats {
        let token_contracts = self.token_contracts.read().await;
        let staking_contracts = self.staking_contracts.read().await;
        let bridge_contracts = self.bridge_contracts.read().await;
        let oracle_feeds = self.oracle_feeds.read().await;
        let bridge_operations = self.bridge_operations.read().await;
        
        let total_staked: u64 = staking_contracts.values()
            .map(|s| s.total_staked)
            .sum();
        
        let completed_bridges = bridge_operations.values()
            .filter(|op| matches!(op.status, BridgeStatus::Completed))
            .count();
        
        let bridge_volume: u64 = bridge_operations.values()
            .map(|op| op.amount)
            .sum();
        
        let networks_supported: std::collections::HashSet<_> = token_contracts.values()
            .map(|t| &t.interface.network)
            .chain(staking_contracts.values().map(|s| &s.interface.network))
            .chain(bridge_contracts.values().map(|b| &b.source_network))
            .collect();
        
        ContractStats {
            total_token_contracts: token_contracts.len(),
            total_staking_contracts: staking_contracts.len(),
            total_bridge_contracts: bridge_contracts.len(),
            total_oracle_feeds: oracle_feeds.len(),
            total_staked_value: CrapTokens::from(total_staked),
            completed_bridge_operations: completed_bridges,
            total_bridge_volume: CrapTokens::from(bridge_volume),
            networks_supported: networks_supported.len(),
        }
    }
    
    // Helper functions
    
    async fn generate_contract_address(&self, network: &BlockchainNetwork) -> String {
        // In a real implementation, this would use the proper address derivation
        format!("0x{:040x}", rand::random::<u64>())
    }
    
    async fn get_current_block_number(&self, _network: &BlockchainNetwork) -> u64 {
        // Mock implementation - would call RPC endpoint
        rand::random::<u64>() % 20_000_000 + 15_000_000
    }
    
    fn generate_operation_id(&self) -> Hash256 {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"bridge_operation");
        hasher.update(Self::current_timestamp().to_be_bytes());
        hasher.update(rand::random::<[u8; 16]>());
        
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }
    
    fn generate_tx_id(&self) -> Hash256 {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"contract_tx");
        hasher.update(Self::current_timestamp().to_be_bytes());
        hasher.update(rand::random::<[u8; 16]>());
        
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }
    
    async fn fetch_mock_price(&self, price_pair: &str) -> f64 {
        // Mock price feed - in production, would fetch from real oracles
        match price_pair {
            "CRAP/USD" => 0.15 + (rand::random::<f64>() - 0.5) * 0.02, // $0.14-$0.16
            "ETH/USD" => 2000.0 + (rand::random::<f64>() - 0.5) * 100.0,
            "BTC/USD" => 45000.0 + (rand::random::<f64>() - 0.5) * 2000.0,
            _ => 1.0,
        }
    }
    
    fn get_erc20_abi(&self) -> Value {
        serde_json::json!([
            {
                "constant": false,
                "inputs": [{"name": "_to", "type": "address"}, {"name": "_value", "type": "uint256"}],
                "name": "transfer",
                "outputs": [{"name": "", "type": "bool"}],
                "type": "function"
            }
        ])
    }
    
    fn get_erc20_bytecode(&self) -> String {
        "0x608060405234801561001057600080fd5b50...".to_string() // Truncated for brevity
    }
    
    fn get_staking_abi(&self) -> Value {
        serde_json::json!([
            {
                "constant": false,
                "inputs": [{"name": "_amount", "type": "uint256"}],
                "name": "stake",
                "outputs": [],
                "type": "function"
            }
        ])
    }
    
    fn get_staking_bytecode(&self) -> String {
        "0x608060405234801561001057600080fd5b50...".to_string()
    }
    
    fn get_bridge_abi(&self) -> Value {
        serde_json::json!([
            {
                "constant": false,
                "inputs": [{"name": "_token", "type": "address"}, {"name": "_amount", "type": "uint256"}],
                "name": "lockTokens",
                "outputs": [],
                "type": "function"
            }
        ])
    }
    
    fn get_bridge_bytecode(&self) -> String {
        "0x608060405234801561001057600080fd5b50...".to_string()
    }
    
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Contract integration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractStats {
    pub total_token_contracts: usize,
    pub total_staking_contracts: usize,
    pub total_bridge_contracts: usize,
    pub total_oracle_feeds: usize,
    pub total_staked_value: CrapTokens,
    pub completed_bridge_operations: usize,
    pub total_bridge_volume: CrapTokens,
    pub networks_supported: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenLedger;
    use crate::treasury::TreasuryManager;
    use crate::economics::TokenEconomics;
    
    #[tokio::test]
    async fn test_contract_manager_creation() {
        let ledger = Arc::new(TokenLedger::new());
        let treasury = Arc::new(TreasuryManager::new(ledger.clone()));
        let economics = Arc::new(TokenEconomics::new(ledger));
        
        let contract_manager = ContractManager::new(treasury, economics);
        let stats = contract_manager.get_contract_stats().await;
        
        assert_eq!(stats.total_token_contracts, 0);
        assert_eq!(stats.networks_supported, 0);
    }
    
    #[tokio::test]
    async fn test_token_contract_deployment() {
        let ledger = Arc::new(TokenLedger::new());
        let treasury = Arc::new(TreasuryManager::new(ledger.clone()));
        let economics = Arc::new(TokenEconomics::new(ledger));
        let contract_manager = ContractManager::new(treasury, economics);
        
        let contract_address = contract_manager.deploy_token_contract(
            BlockchainNetwork::Ethereum,
            "BitCraps Token".to_string(),
            "CRAP".to_string(),
            21_000_000_000_000,
            12,
        ).await.unwrap();
        
        assert!(contract_address.starts_with("0x"));
        
        let stats = contract_manager.get_contract_stats().await;
        assert_eq!(stats.total_token_contracts, 1);
    }
}