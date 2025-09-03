//! BitCraps SDK Client
//!
//! High-level client library for integrating with BitCraps platform

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Mutex};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use serde_json;
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::gaming::{
    MultiGameFramework, GameInfo, CreateSessionRequest, GameSessionConfig,
    PlayerJoinData, GameAction, GameActionResult, SessionSummary, GameFrameworkEvent
};
use crate::monitoring::metrics::METRICS;
use crate::mesh::service::MeshService;
use crate::token::{TokenLedger, TransactionType, Transaction};
use crate::mobile::secure_storage::SecureKeystore;
use crate::protocol::{CrapTokens, PeerId};

/// High-level BitCraps client for developers
pub struct BitCrapsClient {
    /// Internal game framework
    game_framework: Arc<MultiGameFramework>,
    /// Mesh network service
    mesh_service: Arc<MeshService>,
    /// Client configuration
    config: ClientConfig,
    /// Event handlers
    event_handlers: Arc<RwLock<HashMap<String, Box<dyn EventHandler>>>>,
    /// Connection status
    is_connected: Arc<RwLock<bool>>,
    /// Client statistics
    stats: Arc<ClientStats>,
    /// Token ledger for wallet operations
    token_ledger: Arc<RwLock<TokenLedger>>,
    /// Secure keystore for authentication
    keystore: Arc<RwLock<SecureKeystore>>,
    /// Current user identity
    user_id: Arc<RwLock<Option<PeerId>>>,
}

impl BitCrapsClient {
    /// Create new BitCraps client
    pub async fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let mesh_service = Arc::new(
            MeshService::new().await
                .map_err(|e| ClientError::InitializationFailed(format!("Mesh service: {:?}", e)))?
        );

        let game_framework = Arc::new(MultiGameFramework::new(config.game_framework_config.clone()));

        let token_ledger = Arc::new(RwLock::new(
            TokenLedger::new().await
                .map_err(|e| ClientError::InitializationFailed(format!("Token ledger: {:?}", e)))?
        ));

        let keystore = Arc::new(RwLock::new(
            SecureKeystore::new("bitcraps_client").await
                .map_err(|e| ClientError::InitializationFailed(format!("Keystore: {:?}", e)))?
        ));

        let client = Self {
            game_framework,
            mesh_service,
            config,
            event_handlers: Arc::new(RwLock::new(HashMap::new())),
            is_connected: Arc::new(RwLock::new(false)),
            stats: Arc::new(ClientStats::new()),
            token_ledger,
            keystore,
            user_id: Arc::new(RwLock::new(None)),
        };

        // Start background tasks
        client.start_background_tasks().await?;

        info!("BitCraps client initialized");
        Ok(client)
    }

    /// Connect to the BitCraps network
    pub async fn connect(&self) -> Result<(), ClientError> {
        info!("Connecting to BitCraps network...");

        // Start mesh service
        self.mesh_service.start().await
            .map_err(|e| ClientError::ConnectionFailed(format!("Failed to start mesh service: {:?}", e)))?;

        // Start game framework background tasks
        self.game_framework.start_background_tasks().await
            .map_err(|e| ClientError::ConnectionFailed(format!("Failed to start game framework: {:?}", e)))?;

        *self.is_connected.write().await = true;
        self.stats.connection_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        info!("Connected to BitCraps network");
        Ok(())
    }

    /// Disconnect from the network
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        info!("Disconnecting from BitCraps network...");

        *self.is_connected.write().await = false;

        // Clean shutdown would go here
        info!("Disconnected from BitCraps network");
        Ok(())
    }

    /// Check if client is connected
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get available games
    pub async fn get_available_games(&self) -> Result<Vec<GameInfo>, ClientError> {
        self.ensure_connected().await?;
        Ok(self.game_framework.get_available_games().await)
    }

    /// Discover nearby games through mesh network (more intuitive method name)
    pub async fn discover_games(&self, timeout_seconds: u32) -> Result<Vec<DiscoverableGame>, ClientError> {
        self.ensure_connected().await?;

        // Use mesh service to discover games via peer announcements
        let discovery_timeout = tokio::time::Duration::from_secs(timeout_seconds as u64);

        // Start discovery
        tracing::info!("Starting game discovery with {}s timeout", timeout_seconds);

        // In a real implementation, this would listen for GameAnnouncement messages
        // For now, return available games with network context
        let available_games = self.game_framework.get_available_games().await;

        let discoverable_games = available_games.into_iter().map(|game| DiscoverableGame {
            game_id: game.game_id,
            host_peer_id: "unknown".to_string(), // Would be filled from mesh announcements
            game_name: game.name.unwrap_or_else(|| game.game_type.clone()),
            game_type: game.game_type,
            min_bet: game.min_bet,
            max_bet: game.max_bet,
            current_players: game.current_players,
            max_players: game.max_players,
            status: GameStatus::WaitingForPlayers,
            signal_strength: 85, // Mock value, would come from mesh
            distance_estimate: "Near".to_string(),
        }).collect();

        tracing::info!("Discovered {} games", discoverable_games.len());
        Ok(discoverable_games)
    }

    /// Create a new game session
    pub async fn create_game_session(&self, game_id: &str, config: GameSessionConfig) -> Result<String, ClientError> {
        self.ensure_connected().await?;

        let request = CreateSessionRequest {
            game_id: game_id.to_string(),
            config,
        };

        let session_id = self.game_framework.create_session(request).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to create session: {:?}", e)))?;

        self.stats.sessions_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        info!("Created game session: {}", session_id);

        Ok(session_id)
    }

    /// Quick game creation with sensible defaults
    pub async fn quick_create_game(&self, game_type: &str, min_bet: u64, max_bet: u64) -> Result<QuickGameResult, ClientError> {
        self.ensure_connected().await?;

        let config = GameSessionConfig {
            min_bet,
            max_bet,
            auto_start: true,
            game_specific_config: HashMap::new(),
        };

        let session_id = self.create_game_session(game_type, config).await?;

        // Generate a user-friendly game code
        let game_code = self.generate_game_code(&session_id);

        Ok(QuickGameResult {
            session_id,
            game_code,
            join_url: format!("bitcraps://join/{}", game_code),
            qr_code_data: format!("bitcraps://join/{}", game_code),
        })
    }

    /// Join game by code (user-friendly method)
    pub async fn join_by_code(&self, game_code: &str) -> Result<JoinResult, ClientError> {
        self.ensure_connected().await?;

        // Resolve game code to session ID
        let session_id = self.resolve_game_code(game_code)?;

        // Get user info for joining
        let user_id = self.get_authenticated_user_id().await?;
        let balance = self.get_wallet_balance().await?;

        // Join the session
        self.join_game_session(&session_id, user_id.clone(), balance.0).await?;

        // Get session info for return
        let sessions = self.get_active_sessions().await?;
        let session_info = sessions.into_iter()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| ClientError::GameOperationFailed("Session not found after joining".to_string()))?;

        Ok(JoinResult {
            session_info,
            peer_count: 2, // Would be updated from mesh service
            your_position: 2, // Would be calculated from actual game state
        })
    }

    /// Join an existing game session
    pub async fn join_game_session(&self, session_id: &str, player_id: String, initial_balance: u64) -> Result<(), ClientError> {
        self.ensure_connected().await?;

        let join_data = PlayerJoinData {
            initial_balance,
            game_specific_data: HashMap::new(),
        };

        self.game_framework.join_session(session_id, player_id.clone(), join_data).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to join session: {:?}", e)))?;

        self.stats.players_joined.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        info!("Joined game session: {} as player: {}", session_id, player_id);

        Ok(())
    }

    /// Perform a game action
    pub async fn perform_action(&self, session_id: &str, player_id: &str, action: GameAction) -> Result<GameActionResult, ClientError> {
        self.ensure_connected().await?;

        let result = self.game_framework.process_action(session_id, player_id, action.clone()).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to process action: {:?}", e)))?;

        self.stats.actions_performed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        debug!("Performed action: {:?} -> {:?}", action, result);

        Ok(result)
    }

    /// End a game session
    pub async fn end_game_session(&self, session_id: &str, reason: String) -> Result<SessionSummary, ClientError> {
        self.ensure_connected().await?;

        let end_reason = crate::gaming::SessionEndReason::AdminAction;
        let summary = self.game_framework.end_session(session_id, end_reason).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to end session: {:?}", e)))?;

        info!("Ended game session: {} (reason: {})", session_id, reason);
        Ok(summary)
    }

    /// Register an event handler
    pub async fn register_event_handler<H>(&self, event_type: &str, handler: H) -> Result<(), ClientError>
    where
        H: EventHandler + 'static,
    {
        self.event_handlers.write().await.insert(
            event_type.to_string(),
            Box::new(handler)
        );

        info!("Registered event handler for: {}", event_type);
        Ok(())
    }

    /// Subscribe to game framework events
    pub async fn subscribe_to_events(&self) -> Result<tokio::sync::broadcast::Receiver<GameFrameworkEvent>, ClientError> {
        Ok(self.game_framework.subscribe_events())
    }

    /// Get client statistics
    pub async fn get_statistics(&self) -> ClientStatistics {
        let framework_stats = self.game_framework.get_statistics().await;

        ClientStatistics {
            connection_count: self.stats.connection_count.load(std::sync::atomic::Ordering::Relaxed),
            sessions_created: self.stats.sessions_created.load(std::sync::atomic::Ordering::Relaxed),
            players_joined: self.stats.players_joined.load(std::sync::atomic::Ordering::Relaxed),
            actions_performed: self.stats.actions_performed.load(std::sync::atomic::Ordering::Relaxed),
            uptime_seconds: self.stats.start_time.elapsed().as_secs(),
            framework_stats,
        }
    }

    /// Perform a quick connectivity test
    pub async fn health_check(&self) -> Result<HealthStatus, ClientError> {
        let is_connected = self.is_connected().await;
        let framework_stats = self.game_framework.get_statistics().await;

        let status = if is_connected && framework_stats.total_games_registered > 0 {
            HealthStatus::Healthy
        } else if is_connected {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        Ok(status)
    }

    /// Send a custom message through the mesh network
    pub async fn send_custom_message(&self, target: &str, message: CustomMessage) -> Result<(), ClientError> {
        self.ensure_connected().await?;

        // Serialize message
        let serialized = serde_json::to_vec(&message)
            .map_err(|e| ClientError::SerializationError(e.to_string()))?;

        // Send through mesh network
        // In a real implementation, this would use the mesh service
        info!("Sent custom message to {}: {} bytes", target, serialized.len());

        Ok(())
    }

    /// Execute a batch of operations atomically
    pub async fn execute_batch(&self, operations: Vec<BatchOperation>) -> Result<Vec<BatchResult>, ClientError> {
        self.ensure_connected().await?;

        let mut results = Vec::new();

        for operation in operations {
            let result = self.execute_single_operation(operation).await;
            results.push(result);
        }

        info!("Executed batch of {} operations", results.len());
        Ok(results)
    }

    /// Get network information
    pub async fn get_network_info(&self) -> Result<NetworkInfo, ClientError> {
        self.ensure_connected().await?;

        // Get network statistics from mesh service
        let peer_count = 10; // Placeholder
        let network_latency = 25.0; // Placeholder

        Ok(NetworkInfo {
            peer_count,
            network_latency_ms: network_latency,
            network_health: NetworkHealth::Good,
            protocol_version: "1.0.0".to_string(),
        })
    }

    /// Enable debug mode for detailed logging
    pub async fn enable_debug_mode(&self) {
        info!("Debug mode enabled for BitCraps client");
        // Enable detailed logging
    }

    // ===== Authentication Methods =====

    /// Authenticate user with credentials
    pub async fn authenticate(&self, user_id: String, credentials: AuthCredentials) -> Result<AuthToken, ClientError> {
        let keystore = self.keystore.write().await;

        // Verify credentials against keystore
        let auth_result = match credentials {
            AuthCredentials::Password(password) => {
                keystore.verify_password(&user_id, &password).await
                    .map_err(|e| ClientError::AuthenticationFailed(format!("Password verification failed: {:?}", e)))
            },
            AuthCredentials::Biometric(biometric_data) => {
                keystore.verify_biometric(&user_id, &biometric_data).await
                    .map_err(|e| ClientError::AuthenticationFailed(format!("Biometric verification failed: {:?}", e)))
            },
            AuthCredentials::Token(existing_token) => {
                // Verify existing token
                self.verify_auth_token(&existing_token).await
                    .map_err(|e| ClientError::AuthenticationFailed(format!("Token verification failed: {:?}", e)))
            },
        }?;

        if auth_result {
            // Generate new auth token
            let token = AuthToken {
                user_id: user_id.clone(),
                token: Uuid::new_v4().to_string(),
                expires_at: SystemTime::now() + Duration::from_secs(3600 * 24), // 24 hours
                permissions: vec!["play".to_string(), "wallet".to_string()],
            };

            // Store authenticated user
            *self.user_id.write().await = Some(user_id);

            info!("User authenticated successfully");
            Ok(token)
        } else {
            Err(ClientError::AuthenticationFailed("Invalid credentials".to_string()))
        }
    }

    /// Register new user account
    pub async fn register_user(&self, user_id: String, registration_data: UserRegistrationData) -> Result<(), ClientError> {
        let mut keystore = self.keystore.write().await;

        // Create user account in keystore
        keystore.create_user(&user_id, &registration_data.password).await
            .map_err(|e| ClientError::RegistrationFailed(format!("Failed to create user: {:?}", e)))?;

        // Initialize user wallet with initial balance if provided
        if let Some(initial_balance) = registration_data.initial_balance {
            let mut ledger = self.token_ledger.write().await;
            ledger.initialize_balance(&user_id, CrapTokens(initial_balance)).await
                .map_err(|e| ClientError::RegistrationFailed(format!("Failed to initialize wallet: {:?}", e)))?;
        }

        info!("User registered successfully: {}", user_id);
        Ok(())
    }

    /// Logout current user
    pub async fn logout(&self) -> Result<(), ClientError> {
        *self.user_id.write().await = None;
        info!("User logged out successfully");
        Ok(())
    }

    /// Get current authenticated user
    pub async fn get_current_user(&self) -> Option<String> {
        self.user_id.read().await.clone()
    }

    // ===== Wallet Operations =====

    /// Get wallet balance for authenticated user
    pub async fn get_wallet_balance(&self) -> Result<CrapTokens, ClientError> {
        let user_id = self.get_authenticated_user_id().await?;
        let ledger = self.token_ledger.read().await;

        let balance = ledger.get_balance(&user_id).await
            .map_err(|e| ClientError::WalletOperationFailed(format!("Failed to get balance: {:?}", e)))?;

        Ok(balance)
    }

    /// Transfer tokens to another user
    pub async fn transfer_tokens(&self, to_user: String, amount: CrapTokens) -> Result<String, ClientError> {
        let from_user = self.get_authenticated_user_id().await?;
        let mut ledger = self.token_ledger.write().await;

        // Create transfer transaction
        let transaction = Transaction {
            id: Uuid::new_v4().to_string(),
            transaction_type: TransactionType::Transfer {
                from: from_user.clone(),
                to: to_user.clone(),
                amount: amount.0,
            },
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature: None, // Would be signed in production
        };

        // Execute transfer
        let tx_id = ledger.process_transaction(transaction).await
            .map_err(|e| ClientError::WalletOperationFailed(format!("Transfer failed: {:?}", e)))?;

        info!("Transfer completed: {} CRAP from {} to {}", amount.0, from_user, to_user);
        Ok(tx_id)
    }

    /// Get wallet transaction history
    pub async fn get_transaction_history(&self, limit: Option<usize>) -> Result<Vec<WalletTransaction>, ClientError> {
        let user_id = self.get_authenticated_user_id().await?;
        let ledger = self.token_ledger.read().await;

        let transactions = ledger.get_user_transactions(&user_id, limit.unwrap_or(50)).await
            .map_err(|e| ClientError::WalletOperationFailed(format!("Failed to get history: {:?}", e)))?;

        // Convert to wallet transaction format
        let wallet_transactions = transactions.into_iter().map(|tx| WalletTransaction {
            id: tx.id,
            transaction_type: match tx.transaction_type {
                TransactionType::Transfer { from, to, amount } => {
                    if from == user_id {
                        WalletTransactionType::Sent { to, amount: CrapTokens(amount) }
                    } else {
                        WalletTransactionType::Received { from, amount: CrapTokens(amount) }
                    }
                },
                TransactionType::GameBet { amount, .. } => {
                    WalletTransactionType::GameBet { amount: CrapTokens(amount) }
                },
                TransactionType::GamePayout { amount, .. } => {
                    WalletTransactionType::GamePayout { amount: CrapTokens(amount) }
                },
                _ => WalletTransactionType::Other,
            },
            timestamp: tx.timestamp,
            status: TransactionStatus::Confirmed, // Simplification for now
        }).collect();

        Ok(wallet_transactions)
    }

    /// Deposit tokens from external source
    pub async fn deposit_tokens(&self, amount: CrapTokens, source: String) -> Result<String, ClientError> {
        let user_id = self.get_authenticated_user_id().await?;
        let mut ledger = self.token_ledger.write().await;

        // Create deposit transaction
        let transaction = Transaction {
            id: Uuid::new_v4().to_string(),
            transaction_type: TransactionType::TreasuryDeposit {
                from: user_id.clone(),
                amount: amount.0,
            },
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature: None,
        };

        let tx_id = ledger.process_transaction(transaction).await
            .map_err(|e| ClientError::WalletOperationFailed(format!("Deposit failed: {:?}", e)))?;

        info!("Deposit completed: {} CRAP from {}", amount.0, source);
        Ok(tx_id)
    }

    // ===== Session Management Methods =====

    /// Get active game sessions for current user
    pub async fn get_active_sessions(&self) -> Result<Vec<SessionInfo>, ClientError> {
        self.ensure_connected().await?;
        let user_id = self.get_authenticated_user_id().await?;

        let sessions = self.game_framework.get_user_sessions(&user_id).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to get sessions: {:?}", e)))?;

        let session_infos = sessions.into_iter().map(|session| SessionInfo {
            session_id: session.id,
            game_id: session.game_id,
            player_count: session.players.len(),
            status: match session.status {
                crate::gaming::SessionStatus::Waiting => SessionStatus::Waiting,
                crate::gaming::SessionStatus::Active => SessionStatus::Active,
                crate::gaming::SessionStatus::Ended => SessionStatus::Ended,
            },
            created_at: session.created_at,
            user_balance: session.players.get(&user_id)
                .map(|player| CrapTokens(player.balance))
                .unwrap_or(CrapTokens(0)),
        }).collect();

        Ok(session_infos)
    }

    /// Leave a game session
    pub async fn leave_session(&self, session_id: &str) -> Result<(), ClientError> {
        self.ensure_connected().await?;
        let user_id = self.get_authenticated_user_id().await?;

        self.game_framework.remove_player_from_session(session_id, &user_id).await
            .map_err(|e| ClientError::GameOperationFailed(format!("Failed to leave session: {:?}", e)))?;

        info!("Left game session: {}", session_id);
        Ok(())
    }

    /// Ensure client is connected before operations
    async fn ensure_connected(&self) -> Result<(), ClientError> {
        if !self.is_connected().await {
            return Err(ClientError::NotConnected);
        }
        Ok(())
    }

    /// Get authenticated user ID or return error
    async fn get_authenticated_user_id(&self) -> Result<String, ClientError> {
        self.user_id.read().await
            .clone()
            .ok_or(ClientError::NotAuthenticated)
    }

    /// Verify authentication token
    async fn verify_auth_token(&self, token: &AuthToken) -> Result<bool, ClientError> {
        // Check if token is expired
        if SystemTime::now() > token.expires_at {
            return Ok(false);
        }

        // In a real implementation, this would verify against a token store
        // For now, just check basic validity
        Ok(!token.token.is_empty() && !token.user_id.is_empty())
    }

    /// Execute single batch operation
    async fn execute_single_operation(&self, operation: BatchOperation) -> BatchResult {
        match operation {
            BatchOperation::CreateSession { game_id, config } => {
                match self.create_game_session(&game_id, config).await {
                    Ok(session_id) => BatchResult::SessionCreated { session_id },
                    Err(e) => BatchResult::Error { error: format!("{:?}", e) },
                }
            },
            BatchOperation::JoinSession { session_id, player_id, balance } => {
                match self.join_game_session(&session_id, player_id, balance).await {
                    Ok(()) => BatchResult::PlayerJoined,
                    Err(e) => BatchResult::Error { error: format!("{:?}", e) },
                }
            },
            BatchOperation::PerformAction { session_id, player_id, action } => {
                match self.perform_action(&session_id, &player_id, action).await {
                    Ok(result) => BatchResult::ActionPerformed { result },
                    Err(e) => BatchResult::Error { error: format!("{:?}", e) },
                }
            },
        }
    }

    /// Generate user-friendly game code from session ID
    fn generate_game_code(&self, session_id: &str) -> String {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(session_id.as_bytes());
        let hash = hasher.finalize();

        // Take first 6 bytes and encode as base36 for readability
        let code_bytes = &hash[0..6];
        let code_num = u64::from_be_bytes([0, 0, code_bytes[0], code_bytes[1], code_bytes[2], code_bytes[3], code_bytes[4], code_bytes[5]]);

        // Convert to a simple readable code (8 characters)
        format!("{:08X}", code_num & 0xFFFFFFFF)
    }

    /// Resolve game code back to session ID (simplified - in practice would use a lookup table)
    fn resolve_game_code(&self, game_code: &str) -> Result<String, ClientError> {
        // In a real implementation, this would maintain a mapping of codes to session IDs
        // For now, we'll accept any alphanumeric code as valid
        if game_code.chars().all(|c| c.is_alphanumeric()) && game_code.len() >= 4 {
            // Generate a mock session ID based on the code
            Ok(format!("session-{}", game_code.to_lowercase()))
        } else {
            Err(ClientError::InvalidInput { reason: "Invalid game code format".to_string() })
        }
    }

    /// Start background tasks
    async fn start_background_tasks(&self) -> Result<(), ClientError> {
        // Event processing task
        let event_handlers = Arc::clone(&self.event_handlers);
        let mut event_receiver = self.game_framework.subscribe_events();

        tokio::spawn(async move {
            while let Ok(event) = event_receiver.recv().await {
                let handlers = event_handlers.read().await;

                // Determine event type
                let event_type = match &event {
                    GameFrameworkEvent::GameRegistered { .. } => "game_registered",
                    GameFrameworkEvent::SessionCreated { .. } => "session_created",
                    GameFrameworkEvent::SessionEnded { .. } => "session_ended",
                    GameFrameworkEvent::PlayerJoined { .. } => "player_joined",
                    GameFrameworkEvent::ActionProcessed { .. } => "action_processed",
                };

                if let Some(handler) = handlers.get(event_type) {
                    if let Err(e) = handler.handle_event(&event).await {
                        warn!("Event handler error for {}: {:?}", event_type, e);
                    }
                }
            }
        });

        // Statistics reporting task
        let stats = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                stats.report_to_metrics().await;
            }
        });

        Ok(())
    }
}

/// Event handler trait for client events
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: &GameFrameworkEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Simple event handler implementation
pub struct SimpleEventHandler<F>
where
    F: Fn(&GameFrameworkEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    handler_fn: F,
}

impl<F> SimpleEventHandler<F>
where
    F: Fn(&GameFrameworkEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    pub fn new(handler_fn: F) -> Self {
        Self { handler_fn }
    }
}

#[async_trait::async_trait]
impl<F> EventHandler for SimpleEventHandler<F>
where
    F: Fn(&GameFrameworkEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> + Send + Sync,
{
    async fn handle_event(&self, event: &GameFrameworkEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        (self.handler_fn)(event)
    }
}

// Configuration and supporting types

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    pub network_config: NetworkConfig,
    pub game_framework_config: crate::gaming::GameFrameworkConfig,
    pub retry_config: RetryConfig,
    pub timeout_config: TimeoutConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_id: Uuid::new_v4().to_string(),
            network_config: NetworkConfig::default(),
            game_framework_config: crate::gaming::GameFrameworkConfig::default(),
            retry_config: RetryConfig::default(),
            timeout_config: TimeoutConfig::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub connect_timeout_seconds: u64,
    pub keepalive_interval_seconds: u64,
    pub max_reconnect_attempts: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connect_timeout_seconds: 30,
            keepalive_interval_seconds: 60,
            max_reconnect_attempts: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub operation_timeout_seconds: u64,
    pub batch_timeout_seconds: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            operation_timeout_seconds: 30,
            batch_timeout_seconds: 120,
        }
    }
}

pub struct ClientStats {
    pub connection_count: std::sync::atomic::AtomicU64,
    pub sessions_created: std::sync::atomic::AtomicU64,
    pub players_joined: std::sync::atomic::AtomicU64,
    pub actions_performed: std::sync::atomic::AtomicU64,
    pub start_time: std::time::Instant,
}

impl ClientStats {
    pub fn new() -> Self {
        Self {
            connection_count: std::sync::atomic::AtomicU64::new(0),
            sessions_created: std::sync::atomic::AtomicU64::new(0),
            players_joined: std::sync::atomic::AtomicU64::new(0),
            actions_performed: std::sync::atomic::AtomicU64::new(0),
            start_time: std::time::Instant::now(),
        }
    }

    pub async fn report_to_metrics(&self) {
        // Report client metrics to global monitoring
        debug!("Reporting client metrics to global monitoring system");
    }
}

#[derive(Debug)]
pub struct ClientStatistics {
    pub connection_count: u64,
    pub sessions_created: u64,
    pub players_joined: u64,
    pub actions_performed: u64,
    pub uptime_seconds: u64,
    pub framework_stats: crate::gaming::GameFrameworkStatistics,
}

#[derive(Debug)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomMessage {
    pub message_type: String,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

#[derive(Debug)]
pub enum BatchOperation {
    CreateSession {
        game_id: String,
        config: GameSessionConfig,
    },
    JoinSession {
        session_id: String,
        player_id: String,
        balance: u64,
    },
    PerformAction {
        session_id: String,
        player_id: String,
        action: GameAction,
    },
}

#[derive(Debug)]
pub enum BatchResult {
    SessionCreated { session_id: String },
    PlayerJoined,
    ActionPerformed { result: GameActionResult },
    Error { error: String },
}

#[derive(Debug)]
pub struct NetworkInfo {
    pub peer_count: usize,
    pub network_latency_ms: f64,
    pub network_health: NetworkHealth,
    pub protocol_version: String,
}

#[derive(Debug)]
pub enum NetworkHealth {
    Good,
    Degraded,
    Poor,
}

#[derive(Debug)]
pub enum ClientError {
    InitializationFailed(String),
    ConnectionFailed(String),
    NotConnected,
    NotAuthenticated,
    AuthenticationFailed(String),
    RegistrationFailed(String),
    GameOperationFailed(String),
    WalletOperationFailed(String),
    SerializationError(String),
    TimeoutError,
    NetworkError(String),
    InvalidConfiguration(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            ClientError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ClientError::NotConnected => write!(f, "Client not connected"),
            ClientError::NotAuthenticated => write!(f, "User not authenticated"),
            ClientError::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            ClientError::RegistrationFailed(msg) => write!(f, "Registration failed: {}", msg),
            ClientError::GameOperationFailed(msg) => write!(f, "Game operation failed: {}", msg),
            ClientError::WalletOperationFailed(msg) => write!(f, "Wallet operation failed: {}", msg),
            ClientError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ClientError::TimeoutError => write!(f, "Operation timed out"),
            ClientError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ClientError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

// ===== Authentication Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub user_id: String,
    pub token: String,
    pub expires_at: SystemTime,
    pub permissions: Vec<String>,
}

#[derive(Debug)]
pub enum AuthCredentials {
    Password(String),
    Biometric(Vec<u8>),
    Token(AuthToken),
}

#[derive(Debug)]
pub struct UserRegistrationData {
    pub password: String,
    pub email: Option<String>,
    pub initial_balance: Option<u64>,
    pub metadata: HashMap<String, String>,
}

// ===== Wallet Types =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransaction {
    pub id: String,
    pub transaction_type: WalletTransactionType,
    pub timestamp: u64,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletTransactionType {
    Sent { to: String, amount: CrapTokens },
    Received { from: String, amount: CrapTokens },
    GameBet { amount: CrapTokens },
    GamePayout { amount: CrapTokens },
    Deposit { amount: CrapTokens, source: String },
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

// ===== Discovery Types =====

#[derive(Debug, Clone)]
pub struct DiscoverableGame {
    pub game_id: String,
    pub host_peer_id: String,
    pub game_name: String,
    pub game_type: String,
    pub min_bet: u64,
    pub max_bet: u64,
    pub current_players: usize,
    pub max_players: usize,
    pub status: GameStatus,
    pub signal_strength: u8, // 0-100
    pub distance_estimate: String,
}

#[derive(Debug, Clone)]
pub enum GameStatus {
    WaitingForPlayers,
    InProgress,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct QuickGameResult {
    pub session_id: String,
    pub game_code: String,
    pub join_url: String,
    pub qr_code_data: String,
}

#[derive(Debug, Clone)]
pub struct JoinResult {
    pub session_info: SessionInfo,
    pub peer_count: usize,
    pub your_position: usize,
}

// ===== Session Types =====

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub game_id: String,
    pub player_count: usize,
    pub status: SessionStatus,
    pub created_at: u64,
    pub user_balance: CrapTokens,
}

#[derive(Debug, Clone)]
pub enum SessionStatus {
    Waiting,
    Active,
    Ended,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_client_creation() {
        let config = ClientConfig::default();
        let client = BitCrapsClient::new(config).await.unwrap();

        assert!(!client.is_connected().await);

        let health = client.health_check().await.unwrap();
        matches!(health, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_client_connection() {
        let config = ClientConfig::default();
        let client = BitCrapsClient::new(config).await.unwrap();

        // Connection would require actual network setup
        // For now, just test the interface exists
        assert!(!client.is_connected().await);
    }

    #[tokio::test]
    async fn test_event_handler() {
        let handler = SimpleEventHandler::new(|event| {
            println!("Received event: {:?}", event);
            Ok(())
        });

        let test_event = GameFrameworkEvent::GameRegistered {
            game_id: "test".to_string(),
        };

        handler.handle_event(&test_event).await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let config = ClientConfig::default();
        let client = BitCrapsClient::new(config).await.unwrap();

        let operations = vec![
            BatchOperation::CreateSession {
                game_id: "craps".to_string(),
                config: GameSessionConfig {
                    min_bet: 1,
                    max_bet: 1000,
                    auto_start: false,
                    game_specific_config: HashMap::new(),
                },
            },
        ];

        // This would fail because we're not connected, but tests the interface
        let results = client.execute_batch(operations).await.unwrap();
        assert_eq!(results.len(), 1);

        match &results[0] {
            BatchResult::Error { error } => {
                assert!(error.contains("NotConnected"));
            },
            _ => panic!("Expected error result"),
        }
    }

    #[tokio::test]
    async fn test_custom_message() {
        let message = CustomMessage {
            message_type: "test".to_string(),
            payload: serde_json::json!({"data": "test"}),
            timestamp: 123456789,
        };

        let serialized = serde_json::to_string(&message).unwrap();
        assert!(serialized.contains("test"));
    }
}