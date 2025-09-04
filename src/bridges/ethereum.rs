//! Ethereum Bridge Implementation
//!
//! Feynman Explanation: This is our "Ethereum translator" - it speaks both BitCraps and Ethereum.
//! Think of it like a currency exchange that can:
//! 1. Lock CRAP tokens on our network when someone wants to move them to Ethereum
//! 2. Mint wrapped CRAP (wCRAP) tokens on Ethereum
//! 3. Burn wCRAP tokens when someone wants to move back to BitCraps
//! 4. Release the original CRAP tokens back to the user
//!
//! The bridge uses smart contracts on Ethereum to handle the wrapping/unwrapping process
//! and multi-signature validation to prevent fraud and ensure security.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::error::{Error, Result};
use crate::protocol::{Hash256, PeerId, CrapTokens};
use crate::crypto::BitchatKeypair;
use crate::utils::spawn_tracked;
use crate::utils::task::TaskType;

use super::{
    Bridge, BridgeConfig, BridgeTransaction, BridgeTransactionStatus, 
    BridgeEvent, ValidatorSignature, ChainId
};

/// Ethereum-specific bridge configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumBridgeConfig {
    /// Ethereum RPC endpoint URL
    pub rpc_endpoint: String,
    /// Bridge contract address on Ethereum
    pub bridge_contract_address: String,
    /// CRAP token contract address on Ethereum
    pub token_contract_address: String,
    /// Private key for bridge operations (in production, use HSM)
    pub bridge_private_key: String,
    /// Gas price for transactions (in Wei)
    pub gas_price: u64,
    /// Gas limit for bridge transactions
    pub gas_limit: u64,
    /// Number of confirmations required
    pub confirmation_blocks: u32,
    /// Chain ID (1 for mainnet, others for testnets)
    pub chain_id: u64,
    /// Maximum retry attempts for failed transactions
    pub max_retries: u32,
    /// Retry delay in seconds
    pub retry_delay: u64,
}

impl Default for EthereumBridgeConfig {
    fn default() -> Self {
        Self {
            rpc_endpoint: "https://mainnet.infura.io/v3/YOUR-PROJECT-ID".to_string(),
            bridge_contract_address: "0x0000000000000000000000000000000000000000".to_string(),
            token_contract_address: "0x0000000000000000000000000000000000000000".to_string(),
            bridge_private_key: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            gas_price: 20_000_000_000, // 20 gwei
            gas_limit: 300_000,
            confirmation_blocks: 12,
            chain_id: 1, // Ethereum mainnet
            max_retries: 3,
            retry_delay: 60,
        }
    }
}

/// Ethereum smart contract interfaces
#[derive(Debug, Clone)]
pub struct EthereumContracts {
    /// Bridge contract for locking/unlocking operations
    pub bridge_contract: ContractInterface,
    /// ERC-20 token contract for CRAP tokens
    pub token_contract: ContractInterface,
}

/// Smart contract interface abstraction
#[derive(Debug, Clone)]
pub struct ContractInterface {
    /// Contract address on Ethereum
    pub address: String,
    /// Contract ABI (Application Binary Interface)
    pub abi: String,
    /// Gas optimization settings
    pub gas_settings: GasSettings,
}

/// Gas optimization strategies for Ethereum transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasSettings {
    /// Base gas price (in Wei)
    pub base_gas_price: u64,
    /// Maximum gas price willing to pay
    pub max_gas_price: u64,
    /// Gas price increase percentage per retry
    pub gas_price_increase: f64,
    /// EIP-1559 support flag
    pub eip1559_enabled: bool,
    /// Maximum fee per gas (EIP-1559)
    pub max_fee_per_gas: Option<u64>,
    /// Maximum priority fee per gas (EIP-1559)
    pub max_priority_fee: Option<u64>,
}

impl Default for GasSettings {
    fn default() -> Self {
        Self {
            base_gas_price: 20_000_000_000, // 20 gwei
            max_gas_price: 100_000_000_000,  // 100 gwei
            gas_price_increase: 1.125, // 12.5% increase per retry
            eip1559_enabled: true,
            max_fee_per_gas: Some(50_000_000_000), // 50 gwei
            max_priority_fee: Some(2_000_000_000),  // 2 gwei
        }
    }
}

/// Ethereum transaction details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthereumTransaction {
    /// Transaction hash on Ethereum
    pub hash: String,
    /// Block number where transaction was mined
    pub block_number: Option<u64>,
    /// Transaction status (pending, success, failed)
    pub status: EthereumTxStatus,
    /// Gas used by the transaction
    pub gas_used: Option<u64>,
    /// Gas price used (in Wei)
    pub gas_price: u64,
    /// Transaction nonce
    pub nonce: u64,
    /// Number of confirmations
    pub confirmations: u32,
    /// Transaction creation timestamp
    pub created_at: u64,
    /// Last status check timestamp
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EthereumTxStatus {
    Pending,
    Success,
    Failed,
    Cancelled,
}

/// Main Ethereum bridge implementation
pub struct EthereumBridge {
    config: Arc<RwLock<EthereumBridgeConfig>>,
    contracts: Arc<RwLock<Option<EthereumContracts>>>,
    pending_transactions: Arc<RwLock<HashMap<Hash256, EthereumTransaction>>>,
    nonce_tracker: Arc<RwLock<u64>>,
    bridge_config: Arc<RwLock<Option<BridgeConfig>>>,
}

impl EthereumBridge {
    /// Create new Ethereum bridge instance
    pub fn new(config: EthereumBridgeConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            contracts: Arc::new(RwLock::new(None)),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
            nonce_tracker: Arc::new(RwLock::new(0)),
            bridge_config: Arc::new(RwLock::new(None)),
        }
    }

    /// Deploy CRAP token contract on Ethereum
    pub async fn deploy_token_contract(
        &self,
        token_name: &str,
        token_symbol: &str,
        initial_supply: u64,
        decimals: u8,
    ) -> Result<String> {
        let config = self.config.read().await;
        
        // In production, this would:
        // 1. Compile the ERC-20 contract
        // 2. Estimate deployment gas
        // 3. Deploy with constructor parameters
        // 4. Wait for confirmation
        // 5. Verify contract on Etherscan

        let deployment_data = self.create_token_deployment_data(
            token_name, 
            token_symbol, 
            initial_supply, 
            decimals
        ).await?;

        let contract_address = self.simulate_contract_deployment(&deployment_data).await?;

        log::info!(
            "Deployed {} ({}) token contract on Ethereum at {}",
            token_name, token_symbol, contract_address
        );

        // Update our config with the new contract address
        let mut config_guard = self.config.write().await;
        config_guard.token_contract_address = contract_address.clone();
        drop(config_guard);

        Ok(contract_address)
    }

    /// Deploy bridge contract on Ethereum
    pub async fn deploy_bridge_contract(&self, validator_addresses: Vec<String>) -> Result<String> {
        let config = self.config.read().await;
        
        let deployment_data = self.create_bridge_deployment_data(&validator_addresses).await?;
        let contract_address = self.simulate_contract_deployment(&deployment_data).await?;

        log::info!(
            "Deployed bridge contract on Ethereum at {} with {} validators",
            contract_address, validator_addresses.len()
        );

        // Update our config with the new contract address
        let mut config_guard = self.config.write().await;
        config_guard.bridge_contract_address = contract_address.clone();
        drop(config_guard);

        Ok(contract_address)
    }

    /// Lock tokens on Ethereum side of the bridge
    pub async fn lock_tokens(
        &self,
        token_address: &str,
        amount: u64,
        recipient: &str,
        target_chain: &ChainId,
    ) -> Result<String> {
        let config = self.config.read().await;
        
        // Create lock transaction data
        let tx_data = self.create_lock_transaction_data(
            token_address,
            amount,
            recipient,
            target_chain,
        ).await?;

        // Estimate gas for the transaction
        let gas_estimate = self.estimate_gas(&tx_data).await?;
        
        // Get current gas price
        let gas_price = self.get_optimal_gas_price().await?;

        // Submit transaction
        let tx_hash = self.submit_transaction(&tx_data, gas_estimate, gas_price).await?;

        // Track pending transaction
        let eth_tx = EthereumTransaction {
            hash: tx_hash.clone(),
            block_number: None,
            status: EthereumTxStatus::Pending,
            gas_used: None,
            gas_price,
            nonce: self.get_next_nonce().await,
            confirmations: 0,
            created_at: Self::current_timestamp(),
            updated_at: Self::current_timestamp(),
        };

        // Store transaction for monitoring
        let tx_id = self.transaction_hash_to_bridge_id(&tx_hash);
        self.pending_transactions.write().await.insert(tx_id, eth_tx);

        log::info!("Locked {} tokens on Ethereum, tx: {}", amount, tx_hash);
        Ok(tx_hash)
    }

    /// Release tokens on Ethereum (mint wCRAP tokens)
    pub async fn release_tokens(
        &self,
        recipient: &str,
        amount: u64,
        source_tx_hash: &Hash256,
        validator_signatures: &[ValidatorSignature],
    ) -> Result<String> {
        // Verify we have enough validator signatures
        let bridge_config = self.bridge_config.read().await;
        if let Some(config) = bridge_config.as_ref() {
            if validator_signatures.len() < config.required_signatures as usize {
                return Err(Error::InvalidData(
                    "Insufficient validator signatures".to_string()
                ));
            }
        }

        // Create release transaction data
        let tx_data = self.create_release_transaction_data(
            recipient,
            amount,
            source_tx_hash,
            validator_signatures,
        ).await?;

        // Estimate gas and submit
        let gas_estimate = self.estimate_gas(&tx_data).await?;
        let gas_price = self.get_optimal_gas_price().await?;
        let tx_hash = self.submit_transaction(&tx_data, gas_estimate, gas_price).await?;

        log::info!("Released {} tokens to {} on Ethereum, tx: {}", amount, recipient, tx_hash);
        Ok(tx_hash)
    }

    /// Get current token balance on Ethereum
    pub async fn get_token_balance(&self, address: &str, token_contract: &str) -> Result<u64> {
        // Create balance query transaction data
        let balance_data = self.create_balance_query_data(address, token_contract).await?;
        
        // Call contract (read-only, no gas cost)
        let balance_result = self.call_contract(&balance_data).await?;
        
        // Parse balance from result
        let balance = self.parse_balance_result(&balance_result)?;
        
        Ok(balance)
    }

    /// Monitor Ethereum blockchain for bridge events
    pub async fn start_event_monitoring(&self) -> Result<()> {
        let config = self.config.read().await;
        let bridge_contract_address = config.bridge_contract_address.clone();
        let token_contract_address = config.token_contract_address.clone();
        drop(config);

        let pending_transactions = Arc::clone(&self.pending_transactions);

        spawn_tracked("ethereum_event_monitor", TaskType::Network, async move {
            let mut monitoring_interval = tokio::time::interval(Duration::from_secs(15));

            loop {
                tokio::select! {
                    _ = monitoring_interval.tick() => {
                        // Monitor for TokensLocked events
                        if let Err(e) = Self::monitor_tokens_locked_events(&bridge_contract_address).await {
                            log::warn!("Failed to monitor TokensLocked events: {}", e);
                        }

                        // Monitor for TokensReleased events  
                        if let Err(e) = Self::monitor_tokens_released_events(&bridge_contract_address).await {
                            log::warn!("Failed to monitor TokensReleased events: {}", e);
                        }

                        // Update pending transaction statuses
                        if let Err(e) = Self::update_pending_transaction_status(&pending_transactions).await {
                            log::warn!("Failed to update pending transactions: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    // Helper methods for contract interaction

    async fn create_token_deployment_data(
        &self,
        name: &str,
        symbol: &str,
        supply: u64,
        decimals: u8,
    ) -> Result<Vec<u8>> {
        // In production, this would create actual ERC-20 deployment bytecode
        let mut data = Vec::new();
        
        // Constructor parameters (mock encoding)
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(symbol.as_bytes());
        data.extend_from_slice(&supply.to_be_bytes());
        data.extend_from_slice(&[decimals]);
        
        Ok(data)
    }

    async fn create_bridge_deployment_data(&self, validators: &[String]) -> Result<Vec<u8>> {
        // Mock bridge contract deployment data
        let mut data = Vec::new();
        
        // Add validator addresses
        for validator in validators {
            data.extend_from_slice(validator.as_bytes());
        }
        
        // Add required signatures threshold
        data.extend_from_slice(&(validators.len() as u8 * 2 / 3 + 1).to_be_bytes());
        
        Ok(data)
    }

    async fn simulate_contract_deployment(&self, _deployment_data: &[u8]) -> Result<String> {
        // In production, this would actually deploy to Ethereum
        // For now, generate a mock contract address
        Ok(format!("0x{:040x}", rand::random::<u128>()))
    }

    async fn create_lock_transaction_data(
        &self,
        token_address: &str,
        amount: u64,
        recipient: &str,
        target_chain: &ChainId,
    ) -> Result<Vec<u8>> {
        // Create transaction data for locking tokens
        let mut data = Vec::new();
        
        // Function selector for lockTokens(address,uint256,string,uint256)
        data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]); // Mock selector
        
        // Parameters
        data.extend_from_slice(token_address.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(recipient.as_bytes());
        data.extend_from_slice(&(target_chain.chain_id_number()).to_be_bytes());
        
        Ok(data)
    }

    async fn create_release_transaction_data(
        &self,
        recipient: &str,
        amount: u64,
        source_tx_hash: &Hash256,
        signatures: &[ValidatorSignature],
    ) -> Result<Vec<u8>> {
        // Create transaction data for releasing tokens
        let mut data = Vec::new();
        
        // Function selector for releaseTokens
        data.extend_from_slice(&[0x87, 0x65, 0x43, 0x21]); // Mock selector
        
        // Parameters
        data.extend_from_slice(recipient.as_bytes());
        data.extend_from_slice(&amount.to_be_bytes());
        data.extend_from_slice(source_tx_hash);
        
        // Add validator signatures
        for sig in signatures {
            data.extend_from_slice(&sig.signature);
        }
        
        Ok(data)
    }

    async fn create_balance_query_data(&self, address: &str, token_contract: &str) -> Result<Vec<u8>> {
        // Create data for balanceOf(address) call
        let mut data = Vec::new();
        
        // Function selector for balanceOf(address)
        data.extend_from_slice(&[0x70, 0xa0, 0x82, 0x31]); // Actual ERC-20 balanceOf selector
        
        // Address parameter (32 bytes, left-padded)
        let mut address_bytes = [0u8; 32];
        let addr_bytes = address.as_bytes();
        if addr_bytes.len() <= 32 {
            address_bytes[32 - addr_bytes.len()..].copy_from_slice(addr_bytes);
        }
        data.extend_from_slice(&address_bytes);
        
        Ok(data)
    }

    async fn estimate_gas(&self, _tx_data: &[u8]) -> Result<u64> {
        // In production, this would call eth_estimateGas
        // For now, return a reasonable estimate
        Ok(300_000) // 300k gas for complex bridge operations
    }

    async fn get_optimal_gas_price(&self) -> Result<u64> {
        let config = self.config.read().await;
        let gas_settings = GasSettings::default(); // In production, load from config
        
        // In production, this would:
        // 1. Check current network gas prices
        // 2. Apply EIP-1559 pricing if enabled
        // 3. Consider user preferences and urgency
        
        if gas_settings.eip1559_enabled {
            Ok(gas_settings.max_fee_per_gas.unwrap_or(gas_settings.base_gas_price))
        } else {
            Ok(gas_settings.base_gas_price)
        }
    }

    async fn get_next_nonce(&self) -> u64 {
        let mut nonce = self.nonce_tracker.write().await;
        *nonce += 1;
        *nonce
    }

    async fn submit_transaction(&self, _tx_data: &[u8], _gas_limit: u64, _gas_price: u64) -> Result<String> {
        // In production, this would:
        // 1. Sign the transaction with the private key
        // 2. Submit via eth_sendRawTransaction
        // 3. Return the transaction hash
        
        // For now, generate a mock transaction hash
        Ok(format!("0x{:064x}", rand::random::<u64>()))
    }

    async fn call_contract(&self, _call_data: &[u8]) -> Result<Vec<u8>> {
        // In production, this would use eth_call
        // For now, return mock balance data
        let mock_balance = 1000000u64; // 1 CRAP
        Ok(mock_balance.to_be_bytes().to_vec())
    }

    async fn parse_balance_result(&self, result: &[u8]) -> Result<u64> {
        if result.len() >= 8 {
            Ok(u64::from_be_bytes([
                result[0], result[1], result[2], result[3],
                result[4], result[5], result[6], result[7],
            ]))
        } else {
            Err(Error::InvalidData("Invalid balance result".to_string()))
        }
    }

    fn transaction_hash_to_bridge_id(&self, tx_hash: &str) -> Hash256 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(tx_hash.as_bytes());
        
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);
        id
    }

    async fn monitor_tokens_locked_events(contract_address: &str) -> Result<()> {
        // In production, this would:
        // 1. Query recent TokensLocked events from the contract
        // 2. Parse event data to extract lock details
        // 3. Create BridgeEvent::TokensLocked events
        // 4. Store in bridge state manager
        
        log::debug!("Monitoring TokensLocked events for contract {}", contract_address);
        Ok(())
    }

    async fn monitor_tokens_released_events(contract_address: &str) -> Result<()> {
        // In production, this would monitor TokensReleased events
        log::debug!("Monitoring TokensReleased events for contract {}", contract_address);
        Ok(())
    }

    async fn update_pending_transaction_status(
        pending_transactions: &Arc<RwLock<HashMap<Hash256, EthereumTransaction>>>,
    ) -> Result<()> {
        let mut transactions = pending_transactions.write().await;
        
        for (tx_id, eth_tx) in transactions.iter_mut() {
            if eth_tx.status == EthereumTxStatus::Pending {
                // In production, this would:
                // 1. Call eth_getTransactionReceipt
                // 2. Check if transaction is mined
                // 3. Update status and confirmation count
                
                // For now, simulate some transactions completing
                if rand::random::<f64>() < 0.1 { // 10% chance per check
                    eth_tx.status = EthereumTxStatus::Success;
                    eth_tx.block_number = Some(rand::random::<u64>() % 1000000);
                    eth_tx.confirmations = 1;
                    eth_tx.updated_at = Self::current_timestamp();
                    
                    log::info!("Ethereum transaction {} confirmed", hex::encode(&tx_id[..8]));
                }
            }
        }
        
        Ok(())
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[async_trait]
impl Bridge for EthereumBridge {
    async fn initialize(&self, config: BridgeConfig) -> Result<()> {
        *self.bridge_config.write().await = Some(config);
        
        // Initialize Ethereum contracts
        let eth_config = self.config.read().await;
        
        let bridge_contract = ContractInterface {
            address: eth_config.bridge_contract_address.clone(),
            abi: self.get_bridge_contract_abi(),
            gas_settings: GasSettings::default(),
        };
        
        let token_contract = ContractInterface {
            address: eth_config.token_contract_address.clone(),
            abi: self.get_erc20_contract_abi(),
            gas_settings: GasSettings::default(),
        };
        
        let contracts = EthereumContracts {
            bridge_contract,
            token_contract,
        };
        
        *self.contracts.write().await = Some(contracts);
        
        // Start event monitoring
        self.start_event_monitoring().await?;
        
        log::info!("Ethereum bridge initialized with RPC: {}", eth_config.rpc_endpoint);
        Ok(())
    }

    async fn is_token_supported(&self, token: &str, chain: &ChainId) -> Result<bool> {
        // Check if token is supported on Ethereum
        if *chain != ChainId::Ethereum {
            return Ok(false);
        }
        
        // In production, this would check against a whitelist of supported tokens
        // For now, support our CRAP token and some common tokens
        let supported_tokens = ["CRAP", "USDC", "USDT", "DAI", "WETH"];
        Ok(supported_tokens.contains(&token))
    }

    async fn calculate_bridge_fee(&self, amount: u64, _source_chain: &ChainId, _target_chain: &ChainId) -> Result<u64> {
        let bridge_config = self.bridge_config.read().await;
        if let Some(config) = bridge_config.as_ref() {
            let fee = (amount as f64 * config.fee_percentage) as u64;
            Ok(fee.max(1000)) // Minimum fee of 1000 units
        } else {
            Err(Error::InvalidData("Bridge not initialized".to_string()))
        }
    }

    async fn initiate_bridge(&self, transaction: &BridgeTransaction) -> Result<Hash256> {
        // Validate transaction is for Ethereum
        if transaction.source_chain != ChainId::Ethereum && transaction.target_chain != ChainId::Ethereum {
            return Err(Error::InvalidData("Transaction does not involve Ethereum".to_string()));
        }

        if transaction.source_chain == ChainId::Ethereum {
            // Locking tokens on Ethereum to bridge out
            let tx_hash = self.lock_tokens(
                &transaction.token_address,
                transaction.amount,
                &transaction.recipient,
                &transaction.target_chain,
            ).await?;
            
            // Convert Ethereum tx hash to bridge tx ID
            Ok(self.transaction_hash_to_bridge_id(&tx_hash))
        } else {
            // Releasing tokens on Ethereum (bridging in)
            let tx_hash = self.release_tokens(
                &transaction.recipient,
                transaction.amount,
                &transaction.source_tx_hash,
                &transaction.validator_signatures,
            ).await?;
            
            Ok(self.transaction_hash_to_bridge_id(&tx_hash))
        }
    }

    async fn submit_validator_signature(
        &self,
        tx_id: &Hash256,
        signature: &ValidatorSignature,
    ) -> Result<()> {
        // Store validator signature for later use in release transaction
        log::info!(
            "Received validator signature from {} for transaction {}",
            hex::encode(&signature.validator_id),
            hex::encode(&tx_id[..8])
        );
        
        // In production, this would:
        // 1. Validate the signature
        // 2. Store it in a database
        // 3. Check if we have enough signatures to proceed
        // 4. Automatically submit release transaction if threshold met
        
        Ok(())
    }

    async fn get_transaction_status(&self, tx_id: &Hash256) -> Result<BridgeTransactionStatus> {
        let pending_transactions = self.pending_transactions.read().await;
        
        if let Some(eth_tx) = pending_transactions.get(tx_id) {
            match eth_tx.status {
                EthereumTxStatus::Pending => Ok(BridgeTransactionStatus::Initiated),
                EthereumTxStatus::Success => {
                    let bridge_config = self.bridge_config.read().await;
                    let required_confirmations = if let Some(config) = bridge_config.as_ref() {
                        config.confirmation_requirements
                            .get(&ChainId::Ethereum)
                            .cloned()
                            .unwrap_or(12)
                    } else {
                        12
                    };
                    
                    if eth_tx.confirmations >= required_confirmations {
                        Ok(BridgeTransactionStatus::Completed)
                    } else {
                        Ok(BridgeTransactionStatus::SourceConfirmed)
                    }
                }
                EthereumTxStatus::Failed => Ok(BridgeTransactionStatus::Failed("Transaction failed on Ethereum".to_string())),
                EthereumTxStatus::Cancelled => Ok(BridgeTransactionStatus::Cancelled),
            }
        } else {
            Err(Error::InvalidData("Transaction not found".to_string()))
        }
    }

    async fn get_transaction(&self, tx_id: &Hash256) -> Result<Option<BridgeTransaction>> {
        // In production, this would query the full transaction details
        // For now, return None as we don't store full BridgeTransaction objects
        Ok(None)
    }

    async fn cancel_transaction(&self, tx_id: &Hash256, _canceller: &str) -> Result<()> {
        let mut pending_transactions = self.pending_transactions.write().await;
        
        if let Some(eth_tx) = pending_transactions.get_mut(tx_id) {
            if eth_tx.status == EthereumTxStatus::Pending {
                eth_tx.status = EthereumTxStatus::Cancelled;
                eth_tx.updated_at = Self::current_timestamp();
                Ok(())
            } else {
                Err(Error::InvalidData("Transaction cannot be cancelled".to_string()))
            }
        } else {
            Err(Error::InvalidData("Transaction not found".to_string()))
        }
    }

    async fn get_supported_chains(&self) -> Result<Vec<ChainId>> {
        Ok(vec![
            ChainId::Ethereum,
            ChainId::BinanceSmartChain, // Can bridge to other EVM chains
            ChainId::Polygon,
            ChainId::BitCraps, // Our native chain
        ])
    }

    async fn emergency_pause(&self) -> Result<()> {
        // In production, this would call the emergency pause function on the smart contract
        log::warn!("Emergency pause activated for Ethereum bridge");
        Ok(())
    }

    async fn resume_operations(&self) -> Result<()> {
        // In production, this would resume operations on the smart contract
        log::info!("Ethereum bridge operations resumed");
        Ok(())
    }
}

impl EthereumBridge {
    fn get_bridge_contract_abi(&self) -> String {
        // In production, this would be the actual bridge contract ABI
        r#"[
            {
                "inputs": [
                    {"name": "_token", "type": "address"},
                    {"name": "_amount", "type": "uint256"},
                    {"name": "_recipient", "type": "string"},
                    {"name": "_targetChain", "type": "uint256"}
                ],
                "name": "lockTokens",
                "outputs": [],
                "stateMutability": "nonpayable",
                "type": "function"
            },
            {
                "inputs": [
                    {"name": "_recipient", "type": "address"},
                    {"name": "_amount", "type": "uint256"},
                    {"name": "_sourceTxHash", "type": "bytes32"},
                    {"name": "_signatures", "type": "bytes[]"}
                ],
                "name": "releaseTokens",
                "outputs": [],
                "stateMutability": "nonpayable",
                "type": "function"
            }
        ]"#.to_string()
    }

    fn get_erc20_contract_abi(&self) -> String {
        // Standard ERC-20 ABI
        r#"[
            {
                "inputs": [{"name": "_owner", "type": "address"}],
                "name": "balanceOf",
                "outputs": [{"name": "balance", "type": "uint256"}],
                "stateMutability": "view",
                "type": "function"
            },
            {
                "inputs": [
                    {"name": "_to", "type": "address"},
                    {"name": "_value", "type": "uint256"}
                ],
                "name": "transfer",
                "outputs": [{"name": "", "type": "bool"}],
                "stateMutability": "nonpayable",
                "type": "function"
            }
        ]"#.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_ethereum_bridge_initialization() {
        let eth_config = EthereumBridgeConfig::default();
        let bridge = EthereumBridge::new(eth_config);
        
        let bridge_config = BridgeConfig {
            min_amount: 1000,
            max_amount: 1000000,
            fee_percentage: 0.001,
            required_signatures: 2,
            timeout_duration: Duration::from_secs(3600),
            confirmation_requirements: HashMap::new(),
            supported_tokens: HashMap::new(),
            validator_keys: Vec::new(),
            emergency_pause: false,
        };

        let result = bridge.initialize(bridge_config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_token_support() {
        let eth_config = EthereumBridgeConfig::default();
        let bridge = EthereumBridge::new(eth_config);
        
        let supported = bridge.is_token_supported("CRAP", &ChainId::Ethereum).await.unwrap();
        assert!(supported);
        
        let not_supported = bridge.is_token_supported("UNKNOWN", &ChainId::Bitcoin).await.unwrap();
        assert!(!not_supported);
    }

    #[tokio::test]
    async fn test_fee_calculation() {
        let eth_config = EthereumBridgeConfig::default();
        let bridge = EthereumBridge::new(eth_config);
        
        let bridge_config = BridgeConfig {
            min_amount: 1000,
            max_amount: 1000000,
            fee_percentage: 0.001, // 0.1%
            required_signatures: 2,
            timeout_duration: Duration::from_secs(3600),
            confirmation_requirements: HashMap::new(),
            supported_tokens: HashMap::new(),
            validator_keys: Vec::new(),
            emergency_pause: false,
        };

        bridge.initialize(bridge_config).await.unwrap();
        
        let fee = bridge.calculate_bridge_fee(100000, &ChainId::Ethereum, &ChainId::BitCraps).await.unwrap();
        assert_eq!(fee, 1000); // Max of calculated fee (100) and minimum fee (1000)
    }
}