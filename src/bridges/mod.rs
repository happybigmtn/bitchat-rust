//! Cross-Chain Bridge Infrastructure
//!
//! Feynman Explanation: This is the "universal translator" for blockchain communication.
//! Think of it like an international banking system where different banks (blockchains)
//! use different currencies and protocols, but customers can still transfer money between them.
//!
//! The bridge system consists of:
//! - Core abstraction layer: Universal interface for all bridge operations
//! - Security layer: Multi-signature validation and fraud prevention
//! - State management: Tracking cross-chain transactions and settlements
//! - Event monitoring: Real-time detection of blockchain events
//!
//! Each bridge type (Ethereum, Bitcoin, Universal) implements the common interface
//! while handling the specific protocols and security requirements of their networks.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Notify};
use tokio::time::{interval, sleep};

use crate::error::{Error, Result};
use crate::protocol::{Hash256, PeerId, CrapTokens};
use crate::crypto::BitchatKeypair;
use crate::validation::InputValidator;
use crate::security::{SecurityManager, SecurityEvent};
use crate::utils::spawn_tracked;
use crate::utils::task::TaskType;
use tokio::task::JoinHandle;

pub mod ethereum;
pub mod bitcoin;
pub mod universal;

/// Supported blockchain networks for bridging
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChainId {
    Ethereum,
    BinanceSmartChain,
    Polygon,
    Arbitrum,
    Optimism,
    Avalanche,
    Bitcoin,
    BitcoinCash,
    Litecoin,
    BitCraps, // Our native mesh network
}

impl ChainId {
    /// Get the native currency symbol for this chain
    pub fn native_currency(&self) -> &'static str {
        match self {
            ChainId::Ethereum => "ETH",
            ChainId::BinanceSmartChain => "BNB", 
            ChainId::Polygon => "MATIC",
            ChainId::Arbitrum => "ETH",
            ChainId::Optimism => "ETH",
            ChainId::Avalanche => "AVAX",
            ChainId::Bitcoin => "BTC",
            ChainId::BitcoinCash => "BCH",
            ChainId::Litecoin => "LTC",
            ChainId::BitCraps => "CRAP",
        }
    }

    /// Get the chain ID number (0 for UTXO chains)
    pub fn chain_id_number(&self) -> u64 {
        match self {
            ChainId::Ethereum => 1,
            ChainId::BinanceSmartChain => 56,
            ChainId::Polygon => 137,
            ChainId::Arbitrum => 42161,
            ChainId::Optimism => 10,
            ChainId::Avalanche => 43114,
            ChainId::Bitcoin => 0,
            ChainId::BitcoinCash => 0,
            ChainId::Litecoin => 0,
            ChainId::BitCraps => 0,
        }
    }

    /// Check if this is an EVM-compatible chain
    pub fn is_evm_compatible(&self) -> bool {
        matches!(
            self,
            ChainId::Ethereum | ChainId::BinanceSmartChain | ChainId::Polygon | 
            ChainId::Arbitrum | ChainId::Optimism | ChainId::Avalanche
        )
    }

    /// Check if this is a UTXO-based chain
    pub fn is_utxo_based(&self) -> bool {
        matches!(
            self,
            ChainId::Bitcoin | ChainId::BitcoinCash | ChainId::Litecoin
        )
    }
}

/// Bridge transaction status tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BridgeTransactionStatus {
    /// Transaction initiated on source chain
    Initiated,
    /// Source chain transaction confirmed
    SourceConfirmed,
    /// Validators have signed the transaction
    ValidatorsSigned,
    /// Minting/release transaction submitted to target chain
    TargetSubmitted,
    /// Bridge operation completed successfully
    Completed,
    /// Bridge operation failed
    Failed(String),
    /// Bridge operation disputed by validators
    Disputed,
    /// Bridge operation timed out
    TimedOut,
    /// Bridge operation cancelled by user
    Cancelled,
}

/// Bridge transaction details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTransaction {
    /// Unique transaction identifier
    pub tx_id: Hash256,
    /// Source chain identifier
    pub source_chain: ChainId,
    /// Target chain identifier
    pub target_chain: ChainId,
    /// Source chain transaction hash
    pub source_tx_hash: Hash256,
    /// Target chain transaction hash (once minted)
    pub target_tx_hash: Option<Hash256>,
    /// Token contract address on source chain
    pub token_address: String,
    /// Amount being bridged (in smallest unit)
    pub amount: u64,
    /// Bridge fee (in smallest unit)
    pub bridge_fee: u64,
    /// Sender address on source chain
    pub sender: String,
    /// Recipient address on target chain
    pub recipient: String,
    /// Current status of bridge operation
    pub status: BridgeTransactionStatus,
    /// Required validator confirmations
    pub required_confirmations: u8,
    /// Current validator confirmations
    pub current_confirmations: u8,
    /// Validator signatures
    pub validator_signatures: Vec<ValidatorSignature>,
    /// Transaction creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Transaction expiration timestamp
    pub expires_at: u64,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Validator signature for bridge transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSignature {
    /// Validator identifier
    pub validator_id: PeerId,
    /// Cryptographic signature
    pub signature: Vec<u8>,
    /// Signature timestamp
    pub timestamp: u64,
    /// Validator's public key
    pub public_key: Vec<u8>,
}

/// Bridge event types for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeEvent {
    /// Token lock event on source chain
    TokensLocked {
        tx_id: Hash256,
        chain: ChainId,
        token: String,
        amount: u64,
        sender: String,
        recipient: String,
    },
    /// Token mint/release event on target chain
    TokensMinted {
        tx_id: Hash256,
        chain: ChainId,
        token: String,
        amount: u64,
        recipient: String,
    },
    /// Validator signature received
    ValidatorSigned {
        tx_id: Hash256,
        validator: PeerId,
        signature_count: u8,
        required_count: u8,
    },
    /// Bridge transaction completed
    TransactionCompleted {
        tx_id: Hash256,
        source_chain: ChainId,
        target_chain: ChainId,
        amount: u64,
    },
    /// Bridge transaction failed
    TransactionFailed {
        tx_id: Hash256,
        reason: String,
    },
    /// Security alert
    SecurityAlert {
        alert_type: String,
        details: String,
        severity: u8,
    },
}

/// Bridge operation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Minimum bridge amount
    pub min_amount: u64,
    /// Maximum bridge amount  
    pub max_amount: u64,
    /// Bridge fee percentage (0.0-1.0)
    pub fee_percentage: f64,
    /// Required validator signatures
    pub required_signatures: u8,
    /// Transaction timeout duration
    pub timeout_duration: Duration,
    /// Minimum confirmations per chain
    pub confirmation_requirements: HashMap<ChainId, u32>,
    /// Supported token pairs
    pub supported_tokens: HashMap<String, Vec<ChainId>>,
    /// Validator public keys
    pub validator_keys: Vec<Vec<u8>>,
    /// Emergency pause flag
    pub emergency_pause: bool,
}

/// Bridge abstraction trait - all bridge implementations must implement this
#[async_trait]
pub trait Bridge: Send + Sync {
    /// Initialize the bridge connection
    async fn initialize(&self, config: BridgeConfig) -> Result<()>;

    /// Check if a token is supported for bridging
    async fn is_token_supported(&self, token: &str, chain: &ChainId) -> Result<bool>;

    /// Get bridge fee for a specific amount
    async fn calculate_bridge_fee(&self, amount: u64, source_chain: &ChainId, target_chain: &ChainId) -> Result<u64>;

    /// Initiate a bridge transaction
    async fn initiate_bridge(&self, transaction: &BridgeTransaction) -> Result<Hash256>;

    /// Submit validator signature for a transaction
    async fn submit_validator_signature(
        &self, 
        tx_id: &Hash256, 
        signature: &ValidatorSignature
    ) -> Result<()>;

    /// Check the status of a bridge transaction
    async fn get_transaction_status(&self, tx_id: &Hash256) -> Result<BridgeTransactionStatus>;

    /// Get transaction details
    async fn get_transaction(&self, tx_id: &Hash256) -> Result<Option<BridgeTransaction>>;

    /// Cancel a pending bridge transaction
    async fn cancel_transaction(&self, tx_id: &Hash256, canceller: &str) -> Result<()>;

    /// Get supported chains for this bridge
    async fn get_supported_chains(&self) -> Result<Vec<ChainId>>;

    /// Emergency pause all bridge operations
    async fn emergency_pause(&self) -> Result<()>;

    /// Resume bridge operations after pause
    async fn resume_operations(&self) -> Result<()>;
}

/// Bridge security manager for fraud prevention and validation
pub struct BridgeSecurityManager {
    security_manager: Arc<SecurityManager>,
    validator: Arc<InputValidator>,
    fraud_detection_rules: Arc<RwLock<HashMap<String, FraudDetectionRule>>>,
    security_events: Arc<RwLock<Vec<SecurityEvent>>>,
}

impl BridgeSecurityManager {
    pub fn new(security_manager: Arc<SecurityManager>) -> Self {
        Self {
            security_manager,
            validator: Arc::new(InputValidator::new(Default::default()).unwrap_or_else(|_| {
                // TODO: Handle InputValidator initialization error gracefully
                panic!("Failed to create InputValidator")
            })),
            fraud_detection_rules: Arc::new(RwLock::new(HashMap::new())),
            security_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Validate bridge transaction for security issues
    pub async fn validate_bridge_transaction(&self, tx: &BridgeTransaction) -> Result<()> {
        // Input validation
        self.validator.validate_address(&tx.sender)?;
        self.validator.validate_address(&tx.recipient)?;
        self.validator.validate_address(&tx.token_address)?;

        // Amount validation
        if tx.amount == 0 {
            return Err(Error::InvalidData("Bridge amount cannot be zero".to_string()));
        }

        if tx.amount > 1_000_000_000_000_000 { // 1M CRAP max
            return Err(Error::InvalidData("Bridge amount exceeds maximum".to_string()));
        }

        // Chain validation
        if tx.source_chain == tx.target_chain {
            return Err(Error::InvalidData("Source and target chains cannot be the same".to_string()));
        }

        // Fraud detection
        self.run_fraud_detection(tx).await?;

        Ok(())
    }

    /// Run fraud detection rules on bridge transaction
    async fn run_fraud_detection(&self, tx: &BridgeTransaction) -> Result<()> {
        let rules = self.fraud_detection_rules.read().await;
        
        for (rule_name, rule) in rules.iter() {
            if !(rule.evaluate)(tx) {
                let security_event = SecurityEvent::GameIntegrityViolation {
                    game_id: [0u8; 16], // Bridge transactions don't have game IDs
                    player_id: [0u8; 32], // Use default for bridge transactions
                    violation_type: "fraud_detection_triggered".to_string(),
                    details: format!("Rule '{}' triggered for transaction {}", rule_name, hex::encode(&tx.tx_id)),
                };

                self.security_events.write().await.push(security_event);

                if rule.severity >= 8 {
                    return Err(Error::SecurityViolation(
                        format!("High-severity fraud rule triggered: {}", rule_name)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate validator signature
    pub async fn validate_validator_signature(&self, signature: &ValidatorSignature, tx_id: &Hash256) -> Result<()> {
        use ed25519_dalek::{VerifyingKey as PublicKey, Signature, Verifier};

        // Reconstruct message to verify
        let message = self.create_signature_message(tx_id);

        // Parse public key
        let public_key_bytes: [u8; 32] = signature.public_key[..32].try_into()
            .map_err(|_| Error::InvalidData("Invalid public key length".to_string()))?;
        let public_key = PublicKey::from_bytes(&public_key_bytes)
            .map_err(|e| Error::InvalidData(format!("Invalid public key: {}", e)))?;

        // Parse signature
        let sig_bytes: [u8; 64] = signature.signature[..64].try_into()
            .map_err(|_| Error::InvalidData("Invalid signature length".to_string()))?;
        let sig = Signature::from_bytes(&sig_bytes);

        // Verify signature
        public_key.verify(&message, &sig)
            .map_err(|e| Error::SecurityViolation(format!("Signature verification failed: {}", e)))?;

        Ok(())
    }

    /// Add fraud detection rule
    pub async fn add_fraud_detection_rule(&self, name: String, rule: FraudDetectionRule) {
        self.fraud_detection_rules.write().await.insert(name, rule);
    }

    fn create_signature_message(&self, tx_id: &Hash256) -> Vec<u8> {
        let mut message = Vec::new();
        message.extend_from_slice(b"BitCraps Bridge Transaction: ");
        message.extend_from_slice(tx_id);
        message.extend_from_slice(&Self::current_timestamp().to_be_bytes());
        message
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Fraud detection rule for bridge security
#[derive(Debug, Clone)]
pub struct FraudDetectionRule {
    /// Rule name/identifier
    pub name: String,
    /// Rule description
    pub description: String,
    /// Rule severity (1-10)
    pub severity: u8,
    /// Rule evaluation function
    pub evaluate: fn(&BridgeTransaction) -> bool,
}

/// Bridge state manager for tracking transactions and operations
pub struct BridgeStateManager {
    transactions: Arc<RwLock<HashMap<Hash256, BridgeTransaction>>>,
    events: Arc<RwLock<Vec<BridgeEvent>>>,
    config: Arc<RwLock<BridgeConfig>>,
    cleanup_notify: Arc<Notify>,
}

impl BridgeStateManager {
    pub fn new(config: BridgeConfig) -> Self {
        let state_manager = Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(config)),
            cleanup_notify: Arc::new(Notify::new()),
        };

        // Start background cleanup task
        state_manager.start_cleanup_task();

        state_manager
    }

    /// Store bridge transaction
    pub async fn store_transaction(&self, tx: BridgeTransaction) -> Result<()> {
        self.transactions.write().await.insert(tx.tx_id, tx);
        Ok(())
    }

    /// Update transaction status
    pub async fn update_transaction_status(
        &self, 
        tx_id: &Hash256, 
        status: BridgeTransactionStatus
    ) -> Result<()> {
        let mut transactions = self.transactions.write().await;
        if let Some(tx) = transactions.get_mut(tx_id) {
            tx.status = status;
            tx.updated_at = Self::current_timestamp();
        }
        Ok(())
    }

    /// Add validator signature to transaction
    pub async fn add_validator_signature(
        &self,
        tx_id: &Hash256,
        signature: ValidatorSignature,
    ) -> Result<()> {
        let mut transactions = self.transactions.write().await;
        if let Some(tx) = transactions.get_mut(tx_id) {
            // Check if validator already signed
            if tx.validator_signatures.iter().any(|s| s.validator_id == signature.validator_id) {
                return Err(Error::InvalidData("Validator already signed this transaction".to_string()));
            }

            tx.validator_signatures.push(signature);
            tx.current_confirmations = tx.validator_signatures.len() as u8;
            tx.updated_at = Self::current_timestamp();

            // Check if we have enough signatures
            if tx.current_confirmations >= tx.required_confirmations {
                tx.status = BridgeTransactionStatus::ValidatorsSigned;
            }
        }
        Ok(())
    }

    /// Get transaction by ID
    pub async fn get_transaction(&self, tx_id: &Hash256) -> Option<BridgeTransaction> {
        self.transactions.read().await.get(tx_id).cloned()
    }

    /// Get all transactions with a specific status
    pub async fn get_transactions_by_status(&self, status: BridgeTransactionStatus) -> Vec<BridgeTransaction> {
        self.transactions
            .read()
            .await
            .values()
            .filter(|tx| tx.status == status)
            .cloned()
            .collect()
    }

    /// Record bridge event
    pub async fn record_event(&self, event: BridgeEvent) {
        self.events.write().await.push(event);
    }

    /// Get recent events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<BridgeEvent> {
        let events = self.events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get bridge configuration
    pub async fn get_config(&self) -> BridgeConfig {
        self.config.read().await.clone()
    }

    /// Update bridge configuration
    pub async fn update_config(&self, config: BridgeConfig) {
        *self.config.write().await = config;
    }

    /// Clean up expired transactions
    async fn cleanup_expired_transactions(&self) {
        let current_time = Self::current_timestamp();
        let mut transactions = self.transactions.write().await;

        transactions.retain(|_, tx| {
            if current_time > tx.expires_at && 
               !matches!(tx.status, BridgeTransactionStatus::Completed | BridgeTransactionStatus::Failed(_)) {
                false // Remove expired transaction
            } else {
                true // Keep transaction
            }
        });
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let transactions = Arc::clone(&self.transactions);
        let events = Arc::clone(&self.events);
        let cleanup_notify = Arc::clone(&self.cleanup_notify);

        spawn_tracked("bridge_state_cleanup", TaskType::Background, async move {
            let mut cleanup_interval = interval(Duration::from_secs(300)); // 5 minutes

            loop {
                tokio::select! {
                    _ = cleanup_interval.tick() => {
                        // Clean expired transactions
                        let current_time = Self::current_timestamp();
                        let mut tx_guard = transactions.write().await;
                        tx_guard.retain(|_, tx| current_time <= tx.expires_at + 86400); // Keep for 24h after expiry
                        drop(tx_guard);

                        // Clean old events (keep last 1000)
                        let mut events_guard = events.write().await;
                        if events_guard.len() > 1000 {
                            let len = events_guard.len();
                            events_guard.drain(0..len - 1000);
                        }
                        drop(events_guard);
                    }
                    _ = cleanup_notify.notified() => {
                        // Manual cleanup requested
                        break;
                    }
                }
            }
        });
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

/// Bridge event monitor for real-time blockchain event detection
pub struct BridgeEventMonitor {
    state_manager: Arc<BridgeStateManager>,
    security_manager: Arc<BridgeSecurityManager>,
    bridge_implementations: HashMap<ChainId, Arc<dyn Bridge>>,
    monitoring_tasks: Vec<JoinHandle<()>>,
}

impl BridgeEventMonitor {
    pub fn new(
        state_manager: Arc<BridgeStateManager>,
        security_manager: Arc<BridgeSecurityManager>,
    ) -> Self {
        Self {
            state_manager,
            security_manager,
            bridge_implementations: HashMap::new(),
            monitoring_tasks: Vec::new(),
        }
    }

    /// Register bridge implementation for monitoring
    pub fn register_bridge(&mut self, chain: ChainId, bridge: Arc<dyn Bridge>) {
        self.bridge_implementations.insert(chain, bridge);
    }

    /// Start monitoring all registered bridges
    pub async fn start_monitoring(&mut self) -> Result<()> {
        for (chain_id, bridge) in &self.bridge_implementations {
            let chain_id = chain_id.clone();
            let bridge = Arc::clone(bridge);
            let state_manager = Arc::clone(&self.state_manager);
            let security_manager = Arc::clone(&self.security_manager);

            let handle = tokio::spawn(async move {
                let mut monitor_interval = interval(Duration::from_secs(10));

                loop {
                    tokio::select! {
                        _ = monitor_interval.tick() => {
                            // Monitor pending transactions for this chain
                            let pending_txs = state_manager
                                .get_transactions_by_status(BridgeTransactionStatus::Initiated)
                                .await;

                            for tx in pending_txs {
                                if tx.source_chain == chain_id || tx.target_chain == chain_id {
                                    if let Err(e) = Self::monitor_transaction(&bridge, &tx, &state_manager).await {
                                        log::warn!("Failed to monitor transaction {:?}: {}", tx.tx_id, e);
                                    }
                                }
                            }
                        }
                    }
                }
            });

            self.monitoring_tasks.push(handle);
        }

        Ok(())
    }

    async fn monitor_transaction(
        bridge: &Arc<dyn Bridge>,
        tx: &BridgeTransaction,
        state_manager: &Arc<BridgeStateManager>,
    ) -> Result<()> {
        match bridge.get_transaction_status(&tx.tx_id).await? {
            BridgeTransactionStatus::SourceConfirmed => {
                state_manager
                    .update_transaction_status(&tx.tx_id, BridgeTransactionStatus::SourceConfirmed)
                    .await?;
            }
            BridgeTransactionStatus::Completed => {
                state_manager
                    .update_transaction_status(&tx.tx_id, BridgeTransactionStatus::Completed)
                    .await?;
                
                state_manager
                    .record_event(BridgeEvent::TransactionCompleted {
                        tx_id: tx.tx_id,
                        source_chain: tx.source_chain.clone(),
                        target_chain: tx.target_chain.clone(),
                        amount: tx.amount,
                    })
                    .await;
            }
            BridgeTransactionStatus::Failed(reason) => {
                state_manager
                    .update_transaction_status(&tx.tx_id, BridgeTransactionStatus::Failed(reason.clone()))
                    .await?;
                
                state_manager
                    .record_event(BridgeEvent::TransactionFailed {
                        tx_id: tx.tx_id,
                        reason,
                    })
                    .await;
            }
            _ => {}
        }

        Ok(())
    }

    /// Stop all monitoring tasks
    pub async fn stop_monitoring(&mut self) {
        for handle in self.monitoring_tasks.drain(..) {
            handle.abort();
        }
    }
}

/// Bridge statistics and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatistics {
    /// Total number of bridge transactions
    pub total_transactions: u64,
    /// Completed transactions
    pub completed_transactions: u64,
    /// Failed transactions
    pub failed_transactions: u64,
    /// Total volume bridged (in CRAP)
    pub total_volume: CrapTokens,
    /// Average bridge fee
    pub average_fee: f64,
    /// Average completion time (seconds)
    pub average_completion_time: u64,
    /// Supported chains count
    pub supported_chains: u8,
    /// Active validators count
    pub active_validators: u8,
    /// Security events in last 24h
    pub recent_security_events: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_id_properties() {
        assert_eq!(ChainId::Ethereum.native_currency(), "ETH");
        assert_eq!(ChainId::Bitcoin.native_currency(), "BTC");
        assert_eq!(ChainId::BitCraps.native_currency(), "CRAP");
        
        assert!(ChainId::Ethereum.is_evm_compatible());
        assert!(!ChainId::Bitcoin.is_evm_compatible());
        
        assert!(ChainId::Bitcoin.is_utxo_based());
        assert!(!ChainId::Ethereum.is_utxo_based());
    }

    #[tokio::test]
    async fn test_bridge_state_manager() {
        let config = BridgeConfig {
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

        let state_manager = BridgeStateManager::new(config);
        
        let tx = BridgeTransaction {
            tx_id: [1u8; 32],
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::BitCraps,
            source_tx_hash: [2u8; 32],
            target_tx_hash: None,
            token_address: "0x1234".to_string(),
            amount: 1000000,
            bridge_fee: 1000,
            sender: "sender".to_string(),
            recipient: "recipient".to_string(),
            status: BridgeTransactionStatus::Initiated,
            required_confirmations: 2,
            current_confirmations: 0,
            validator_signatures: Vec::new(),
            created_at: 1234567890,
            updated_at: 1234567890,
            expires_at: 1234567890 + 3600,
            metadata: HashMap::new(),
        };

        state_manager.store_transaction(tx.clone()).await.unwrap();
        
        let retrieved = state_manager.get_transaction(&tx.tx_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().amount, 1000000);
    }
}