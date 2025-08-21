# Week 4: Session Management & Noise Protocol Integration

## ⚠️ IMPORTANT: Updated Implementation Notes

**Before starting this week, please review `/docs/COMPILATION_FIXES.md` for critical dependency and API updates.**

**Key fixes for Week 4:**
- Add session persistence dependencies: `argon2 = "0.5.3"`, `chrono = "0.4.41"`, `rusqlite`
- Session state machine is already correctly implemented with proper states
- SQLite schemas need proper constraints and indexes
- All session operations should be async where appropriate

## Overview

This week implements the session management layer and full Noise protocol integration for BitChat. We'll build secure session state machines with lifecycle management, implement forward secrecy through key rotation, and add encrypted channel communication with session persistence.

---

## Day 1: Noise Session State Machine

### Goals
- Implement Noise_XX_25519_ChaChaPoly_SHA256 handshake state machine
- Create session state management
- Build handshake message handling
- Implement transport state tracking

### Core Data Structures

```rust
// src/session/noise.rs
use snow::{Builder, HandshakeState, TransportState};
use crate::crypto::keys::{KeyPair, PublicKey};

#[derive(Debug, Clone)]
pub enum NoiseRole {
    Initiator,
    Responder,
}

#[derive(Debug)]
pub enum NoiseSessionState {
    Uninitialized,
    HandshakeInProgress {
        handshake: HandshakeState,
        role: NoiseRole,
        step: u8,
    },
    Transport {
        transport: TransportState,
        role: NoiseRole,
        created_at: Instant,
    },
    Failed {
        error: String,
        timestamp: Instant,
    },
}

pub struct NoiseSession {
    state: NoiseSessionState,
    peer_id: PublicKey,
    local_keypair: KeyPair,
    session_id: [u8; 16],
}
```

### Key Functions

```rust
impl NoiseSession {
    pub fn new_initiator(peer_id: PublicKey, local_keypair: KeyPair) -> Result<Self> {
        let params = snow::params::NoiseParams::new(
            "Noise_XX_25519_ChaChaPoly_SHA256".parse()?
        );
        
        let builder = Builder::new(params)
            .local_private_key(&local_keypair.private_bytes())
            .remote_public_key(&peer_id.to_bytes());
            
        let handshake = builder.build_initiator()?;
        
        Ok(Self {
            state: NoiseSessionState::HandshakeInProgress {
                handshake,
                role: NoiseRole::Initiator,
                step: 0,
            },
            peer_id,
            local_keypair,
            session_id: generate_session_id(),
        })
    }

    pub fn new_responder(local_keypair: KeyPair) -> Result<Self> {
        let params = snow::params::NoiseParams::new(
            "Noise_XX_25519_ChaChaPoly_SHA256".parse()?
        );
        
        let builder = Builder::new(params)
            .local_private_key(&local_keypair.private_bytes());
            
        let handshake = builder.build_responder()?;
        
        Ok(Self {
            state: NoiseSessionState::HandshakeInProgress {
                handshake,
                role: NoiseRole::Responder,
                step: 0,
            },
            peer_id: PublicKey::default(), // Will be set during handshake
            local_keypair,
            session_id: generate_session_id(),
        })
    }

    pub fn process_handshake_message(&mut self, message: &[u8]) -> Result<Option<Vec<u8>>> {
        match &mut self.state {
            NoiseSessionState::HandshakeInProgress { handshake, role, step } => {
                let mut buf = vec![0u8; 4096];
                
                match role {
                    NoiseRole::Initiator => {
                        match *step {
                            0 => {
                                let len = handshake.write_message(&[], &mut buf)?;
                                *step = 1;
                                Ok(Some(buf[..len].to_vec()))
                            }
                            2 => {
                                let len = handshake.read_message(message, &mut buf)?;
                                let transport = handshake.into_transport_mode()?;
                                self.state = NoiseSessionState::Transport {
                                    transport,
                                    role: *role,
                                    created_at: Instant::now(),
                                };
                                Ok(None)
                            }
                            _ => Err(SessionError::InvalidHandshakeStep(*step)),
                        }
                    }
                    NoiseRole::Responder => {
                        match *step {
                            0 => {
                                let _len = handshake.read_message(message, &mut buf)?;
                                let len = handshake.write_message(&[], &mut buf)?;
                                *step = 1;
                                Ok(Some(buf[..len].to_vec()))
                            }
                            1 => {
                                let _len = handshake.read_message(message, &mut buf)?;
                                let transport = handshake.into_transport_mode()?;
                                self.state = NoiseSessionState::Transport {
                                    transport,
                                    role: *role,
                                    created_at: Instant::now(),
                                };
                                Ok(None)
                            }
                            _ => Err(SessionError::InvalidHandshakeStep(*step)),
                        }
                    }
                }
            }
            _ => Err(SessionError::NotInHandshakeState),
        }
    }
}
```

### State Transitions

```rust
// src/session/state.rs
#[derive(Debug, Clone)]
pub enum SessionEvent {
    HandshakeInitiated,
    HandshakeMessageReceived(Vec<u8>),
    HandshakeCompleted,
    MessageSent,
    MessageReceived,
    TimeoutExpired,
    ErrorOccurred(String),
}

impl NoiseSession {
    pub fn handle_event(&mut self, event: SessionEvent) -> Result<Vec<SessionAction>> {
        match (&self.state, event) {
            (NoiseSessionState::Uninitialized, SessionEvent::HandshakeInitiated) => {
                // Start handshake process
                self.initiate_handshake()
            }
            (NoiseSessionState::HandshakeInProgress { .. }, SessionEvent::HandshakeMessageReceived(msg)) => {
                // Process handshake message
                self.process_handshake_message(&msg)
                    .map(|response| {
                        if let Some(resp) = response {
                            vec![SessionAction::SendHandshakeMessage(resp)]
                        } else {
                            vec![SessionAction::HandshakeCompleted]
                        }
                    })
            }
            _ => Ok(vec![]),
        }
    }
}
```

---

## Day 2: Session Lifecycle (1hr/1000msg limits)

### Goals
- Implement session timeout management
- Add message count tracking
- Create automatic session renewal
- Build lifecycle event handling

### Core Data Structures

```rust
// src/session/lifecycle.rs
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SessionLimits {
    pub max_duration: Duration,
    pub max_message_count: u32,
    pub warning_threshold: f32, // 0.8 = 80% threshold
}

impl Default for SessionLimits {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(3600), // 1 hour
            max_message_count: 1000,
            warning_threshold: 0.8,
        }
    }
}

#[derive(Debug)]
pub struct SessionMetrics {
    pub created_at: Instant,
    pub last_activity: Instant,
    pub message_count: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug)]
pub enum SessionStatus {
    Active,
    NearExpiry { reason: ExpiryReason, remaining: Duration },
    Expired { reason: ExpiryReason },
    Renewed { old_session_id: [u8; 16] },
}

#[derive(Debug, Clone)]
pub enum ExpiryReason {
    TimeLimit,
    MessageLimit,
    IdleTimeout,
}

pub struct SessionManager {
    sessions: HashMap<[u8; 16], NoiseSession>,
    metrics: HashMap<[u8; 16], SessionMetrics>,
    limits: SessionLimits,
}
```

### Key Functions

```rust
impl SessionManager {
    pub fn new(limits: SessionLimits) -> Self {
        Self {
            sessions: HashMap::new(),
            metrics: HashMap::new(),
            limits,
        }
    }

    pub fn check_session_health(&self, session_id: &[u8; 16]) -> Option<SessionStatus> {
        let session = self.sessions.get(session_id)?;
        let metrics = self.metrics.get(session_id)?;
        
        let now = Instant::now();
        let age = now.duration_since(metrics.created_at);
        let idle_time = now.duration_since(metrics.last_activity);
        
        // Check time limit
        if age >= self.limits.max_duration {
            return Some(SessionStatus::Expired {
                reason: ExpiryReason::TimeLimit,
            });
        }
        
        // Check message count limit
        if metrics.message_count >= self.limits.max_message_count {
            return Some(SessionStatus::Expired {
                reason: ExpiryReason::MessageLimit,
            });
        }
        
        // Check for near expiry warnings
        let time_progress = age.as_secs_f32() / self.limits.max_duration.as_secs_f32();
        let msg_progress = metrics.message_count as f32 / self.limits.max_message_count as f32;
        
        if time_progress >= self.limits.warning_threshold {
            return Some(SessionStatus::NearExpiry {
                reason: ExpiryReason::TimeLimit,
                remaining: self.limits.max_duration - age,
            });
        }
        
        if msg_progress >= self.limits.warning_threshold {
            return Some(SessionStatus::NearExpiry {
                reason: ExpiryReason::MessageLimit,
                remaining: Duration::from_secs(
                    ((self.limits.max_message_count - metrics.message_count) * 60) as u64
                ),
            });
        }
        
        Some(SessionStatus::Active)
    }

    pub fn renew_session(&mut self, old_session_id: &[u8; 16]) -> Result<[u8; 16]> {
        let old_session = self.sessions.remove(old_session_id)
            .ok_or(SessionError::SessionNotFound)?;
        
        // Create new session with same peer
        let new_session = NoiseSession::new_initiator(
            old_session.peer_id,
            old_session.local_keypair.clone()
        )?;
        
        let new_session_id = new_session.session_id;
        
        // Initialize new metrics
        let new_metrics = SessionMetrics {
            created_at: Instant::now(),
            last_activity: Instant::now(),
            message_count: 0,
            bytes_sent: 0,
            bytes_received: 0,
        };
        
        self.sessions.insert(new_session_id, new_session);
        self.metrics.insert(new_session_id, new_metrics);
        
        // Clean up old metrics
        self.metrics.remove(old_session_id);
        
        Ok(new_session_id)
    }

    pub fn update_activity(&mut self, session_id: &[u8; 16], message_size: usize, is_outbound: bool) {
        if let Some(metrics) = self.metrics.get_mut(session_id) {
            metrics.last_activity = Instant::now();
            metrics.message_count += 1;
            
            if is_outbound {
                metrics.bytes_sent += message_size as u64;
            } else {
                metrics.bytes_received += message_size as u64;
            }
        }
    }
}
```

---

## Day 3: Key Rotation and Forward Secrecy

### Goals
- Implement automatic key rotation
- Add forward secrecy guarantees
- Create key derivation functions
- Build secure key storage
- Add gaming session security for BitCraps
- Implement bet escrow mechanisms
- Create game-specific key rotation

### Core Data Structures

```rust
// src/session/forward_secrecy.rs
use crate::crypto::kdf::derive_key;

#[derive(Debug, Clone)]
pub struct KeyRotationConfig {
    pub rotation_interval: Duration,
    pub max_messages_per_key: u32,
    pub keep_old_keys_for: Duration,
}

#[derive(Debug)]
struct RotatedKey {
    key_material: [u8; 32],
    created_at: Instant,
    message_count: u32,
    last_used: Instant,
}

pub struct ForwardSecretyManager {
    current_send_key: RotatedKey,
    current_recv_key: RotatedKey,
    old_recv_keys: Vec<RotatedKey>,
    config: KeyRotationConfig,
    base_secret: [u8; 32],
    rotation_counter: u64,
}
```

### Key Functions

```rust
impl ForwardSecretyManager {
    pub fn new(initial_secret: [u8; 32], config: KeyRotationConfig) -> Self {
        let now = Instant::now();
        let initial_key = RotatedKey {
            key_material: initial_secret,
            created_at: now,
            message_count: 0,
            last_used: now,
        };

        Self {
            current_send_key: initial_key.clone(),
            current_recv_key: initial_key,
            old_recv_keys: Vec::new(),
            config,
            base_secret: initial_secret,
            rotation_counter: 0,
        }
    }

    pub fn should_rotate_keys(&self) -> bool {
        let now = Instant::now();
        let age = now.duration_since(self.current_send_key.created_at);
        
        age >= self.config.rotation_interval ||
        self.current_send_key.message_count >= self.config.max_messages_per_key
    }

    pub fn rotate_send_key(&mut self) -> Result<[u8; 32]> {
        self.rotation_counter += 1;
        
        // Derive new key material using KDF with counter
        let new_key_material = derive_key(
            &self.base_secret,
            &format!("send_key_{}", self.rotation_counter).as_bytes(),
            32
        )?;

        let new_key = RotatedKey {
            key_material: new_key_material,
            created_at: Instant::now(),
            message_count: 0,
            last_used: Instant::now(),
        };

        // Archive old key for potential delayed messages
        self.old_recv_keys.push(self.current_recv_key.clone());
        self.current_send_key = new_key;
        
        // Clean up old keys
        self.cleanup_old_keys();
        
        Ok(new_key_material)
    }

    pub fn get_decrypt_key(&mut self, timestamp: Instant) -> Option<&[u8; 32]> {
        // Try current key first
        if timestamp >= self.current_recv_key.created_at {
            self.current_recv_key.last_used = Instant::now();
            return Some(&self.current_recv_key.key_material);
        }
        
        // Try old keys for delayed messages
        for key in &mut self.old_recv_keys {
            if timestamp >= key.created_at {
                key.last_used = Instant::now();
                return Some(&key.key_material);
            }
        }
        
        None
    }

    fn cleanup_old_keys(&mut self) {
        let cutoff = Instant::now() - self.config.keep_old_keys_for;
        self.old_recv_keys.retain(|key| key.last_used > cutoff);
    }

    pub fn get_current_send_key(&mut self) -> &[u8; 32] {
        self.current_send_key.message_count += 1;
        self.current_send_key.last_used = Instant::now();
        &self.current_send_key.key_material
    }
}
```

---

## Day 4: Channel Encryption with Argon2id

### Goals
- Implement Argon2id for key derivation
- Add authenticated encryption for channels
- Create secure random nonce generation
- Build encrypted packet format

### Core Data Structures

```rust
// src/session/encryption.rs
use argon2::{Argon2, Config, Variant, Version};
use chacha20poly1305::{XChaCha20Poly1305, Key, Nonce};

#[derive(Debug)]
pub struct ChannelCrypto {
    cipher: XChaCha20Poly1305,
    argon2_config: Config<'static>,
    salt: [u8; 32],
    nonce_counter: AtomicU64,
}

#[derive(Debug)]
pub struct EncryptedPacket {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
    pub timestamp: u64,
}
```

### Key Functions

```rust
impl ChannelCrypto {
    pub fn new(password: &[u8], salt: [u8; 32]) -> Result<Self> {
        let config = Config {
            variant: Variant::Argon2id,
            version: Version::Version13,
            mem_cost: 65536,      // 64 MB
            time_cost: 3,         // 3 iterations
            lanes: 4,             // 4 parallel lanes
            secret: &[],
            ad: &[],
            hash_length: 32,
        };

        let key_bytes = argon2::hash_raw(password, &salt, &config)
            .map_err(|e| SessionError::KeyDerivation(e.to_string()))?;

        let key = Key::from_slice(&key_bytes);
        let cipher = XChaCha20Poly1305::new(key);

        Ok(Self {
            cipher,
            argon2_config: config,
            salt,
            nonce_counter: AtomicU64::new(0),
        })
    }

    pub fn encrypt(&self, plaintext: &[u8], associated_data: &[u8]) -> Result<EncryptedPacket> {
        // Generate unique nonce
        let counter = self.nonce_counter.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let mut nonce_bytes = [0u8; 24];
        nonce_bytes[..8].copy_from_slice(&counter.to_le_bytes());
        nonce_bytes[8..16].copy_from_slice(&timestamp.to_le_bytes());
        nonce_bytes[16..].copy_from_slice(&thread_rng().gen::<[u8; 8]>());

        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher
            .encrypt(nonce, Payload {
                msg: plaintext,
                aad: associated_data,
            })
            .map_err(|e| SessionError::EncryptionFailed(e.to_string()))?;

        // Split ciphertext and tag
        let (ct, tag) = ciphertext.split_at(ciphertext.len() - 16);
        let mut tag_array = [0u8; 16];
        tag_array.copy_from_slice(tag);

        Ok(EncryptedPacket {
            nonce: nonce_bytes,
            ciphertext: ct.to_vec(),
            tag: tag_array,
            timestamp,
        })
    }

    pub fn decrypt(&self, packet: &EncryptedPacket, associated_data: &[u8]) -> Result<Vec<u8>> {
        let nonce = XNonce::from_slice(&packet.nonce);
        
        // Reconstruct full ciphertext with tag
        let mut full_ciphertext = packet.ciphertext.clone();
        full_ciphertext.extend_from_slice(&packet.tag);

        let plaintext = self.cipher
            .decrypt(nonce, Payload {
                msg: &full_ciphertext,
                aad: associated_data,
            })
            .map_err(|e| SessionError::DecryptionFailed(e.to_string()))?;

        Ok(plaintext)
    }

    pub fn derive_channel_key(&self, channel_id: &[u8], session_secret: &[u8]) -> Result<[u8; 32]> {
        let mut input = Vec::new();
        input.extend_from_slice(session_secret);
        input.extend_from_slice(b"bitchat_channel_");
        input.extend_from_slice(channel_id);

        let key = argon2::hash_raw(&input, &self.salt, &self.argon2_config)
            .map_err(|e| SessionError::KeyDerivation(e.to_string()))?;

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&key);
        Ok(key_array)
    }
}
```

---

## Day 5: Session Persistence and Recovery

### Goals
- Implement session state serialization
- Add encrypted session storage
- Create session recovery mechanisms
- Build migration for protocol updates

### Core Data Structures

```rust
// src/session/persistence.rs
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PersistedSession {
    pub session_id: [u8; 16],
    pub peer_id: Vec<u8>,
    pub transport_state: Vec<u8>, // Encrypted serialized transport
    pub created_at: u64,
    pub last_activity: u64,
    pub message_count: u32,
    pub version: u8,
}

pub struct SessionStorage {
    storage_path: PathBuf,
    encryption_key: [u8; 32],
    version: u8,
}
```

### Key Functions

```rust
impl SessionStorage {
    pub fn new(storage_path: PathBuf, master_key: &[u8]) -> Result<Self> {
        let encryption_key = derive_storage_key(master_key)?;
        
        std::fs::create_dir_all(&storage_path)?;
        
        Ok(Self {
            storage_path,
            encryption_key,
            version: 1,
        })
    }

    pub async fn save_session(&self, session: &NoiseSession, metrics: &SessionMetrics) -> Result<()> {
        let transport_bytes = match &session.state {
            NoiseSessionState::Transport { transport, .. } => {
                // Serialize transport state securely
                bincode::serialize(transport)?
            }
            _ => return Err(SessionError::InvalidSessionState),
        };

        let persisted = PersistedSession {
            session_id: session.session_id,
            peer_id: session.peer_id.to_bytes().to_vec(),
            transport_state: self.encrypt_state(&transport_bytes)?,
            created_at: metrics.created_at.elapsed().as_secs(),
            last_activity: metrics.last_activity.elapsed().as_secs(),
            message_count: metrics.message_count,
            version: self.version,
        };

        let session_file = self.storage_path.join(format!("{}.session", 
            hex::encode(&session.session_id)));
        
        let encrypted_data = self.encrypt_session_data(&bincode::serialize(&persisted)?)?;
        tokio::fs::write(session_file, encrypted_data).await?;
        
        Ok(())
    }

    pub async fn load_session(&self, session_id: &[u8; 16]) -> Result<(NoiseSession, SessionMetrics)> {
        let session_file = self.storage_path.join(format!("{}.session", hex::encode(session_id)));
        
        let encrypted_data = tokio::fs::read(session_file).await?;
        let session_data = self.decrypt_session_data(&encrypted_data)?;
        let persisted: PersistedSession = bincode::deserialize(&session_data)?;

        // Verify version compatibility
        if persisted.version != self.version {
            return self.migrate_session(persisted).await;
        }

        let transport_bytes = self.decrypt_state(&persisted.transport_state)?;
        let transport: TransportState = bincode::deserialize(&transport_bytes)?;

        let session = NoiseSession {
            state: NoiseSessionState::Transport {
                transport,
                role: NoiseRole::Initiator, // TODO: persist role
                created_at: Instant::now() - Duration::from_secs(persisted.created_at),
            },
            peer_id: PublicKey::from_bytes(&persisted.peer_id)?,
            local_keypair: KeyPair::generate(), // TODO: persist safely
            session_id: persisted.session_id,
        };

        let metrics = SessionMetrics {
            created_at: Instant::now() - Duration::from_secs(persisted.created_at),
            last_activity: Instant::now() - Duration::from_secs(persisted.last_activity),
            message_count: persisted.message_count,
            bytes_sent: 0, // TODO: persist metrics
            bytes_received: 0,
        };

        Ok((session, metrics))
    }

    async fn migrate_session(&self, old_session: PersistedSession) -> Result<(NoiseSession, SessionMetrics)> {
        match old_session.version {
            0 => {
                // Migration from version 0 to 1
                // Re-establish handshake due to incompatible changes
                Err(SessionError::SessionMigrationRequired)
            }
            _ => Err(SessionError::UnsupportedVersion(old_session.version)),
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let mut cleaned = 0;
        let max_age = Duration::from_secs(24 * 3600); // 24 hours
        
        let mut dir = tokio::fs::read_dir(&self.storage_path).await?;
        while let Some(entry) = dir.next_entry().await? {
            if let Some(ext) = entry.path().extension() {
                if ext == "session" {
                    let metadata = entry.metadata().await?;
                    if let Ok(modified) = metadata.modified() {
                        let age = SystemTime::now().duration_since(modified)
                            .unwrap_or(Duration::ZERO);
                        
                        if age > max_age {
                            tokio::fs::remove_file(entry.path()).await?;
                            cleaned += 1;
                        }
                    }
                }
            }
        }
        
        Ok(cleaned)
    }
}
```

### Integration Points

```rust
// src/session/mod.rs - Main session module integration
pub struct SessionManager {
    sessions: HashMap<[u8; 16], NoiseSession>,
    metrics: HashMap<[u8; 16], SessionMetrics>,
    forward_secrecy: HashMap<[u8; 16], ForwardSecretyManager>,
    storage: SessionStorage,
    limits: SessionLimits,
}

impl SessionManager {
    pub async fn send_encrypted_message(
        &mut self,
        session_id: &[u8; 16],
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let session = self.sessions.get_mut(session_id)
            .ok_or(SessionError::SessionNotFound)?;
        
        // Check if key rotation is needed
        if let Some(fs_manager) = self.forward_secrecy.get_mut(session_id) {
            if fs_manager.should_rotate_keys() {
                fs_manager.rotate_send_key()?;
            }
        }
        
        // Encrypt with current session state
        match &mut session.state {
            NoiseSessionState::Transport { transport, .. } => {
                let mut buffer = vec![0u8; plaintext.len() + 16];
                let len = transport.write_message(plaintext, &mut buffer)?;
                
                // Update activity tracking
                self.update_activity(session_id, len, true);
                
                Ok(buffer[..len].to_vec())
            }
            _ => Err(SessionError::SessionNotEstablished),
        }
    }
}
```

### Gaming Security Extensions

```rust
// src/session/gaming_security.rs
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use rand::{RngCore, CryptoRng};

/// Gaming-specific session security manager
pub struct GamingSecurityManager {
    game_sessions: RwLock<HashMap<String, GameSessionSecurity>>,
    escrow_keys: RwLock<HashMap<String, EscrowKeySet>>,
    bet_validators: RwLock<HashMap<PeerId, BetValidator>>,
    security_config: GamingSecurityConfig,
}

#[derive(Debug, Clone)]
pub struct GamingSecurityConfig {
    pub escrow_key_rotation_interval: Duration,
    pub bet_validation_timeout: Duration,
    pub max_concurrent_bets: usize,
    pub required_confirmations: usize,
}

impl Default for GamingSecurityConfig {
    fn default() -> Self {
        Self {
            escrow_key_rotation_interval: Duration::from_secs(600), // 10 minutes
            bet_validation_timeout: Duration::from_secs(30),
            max_concurrent_bets: 50,
            required_confirmations: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameSessionSecurity {
    pub session_id: String,
    pub participants: Vec<PeerId>,
    pub escrow_key_id: String,
    pub current_round_key: [u8; 32],
    pub bet_commitment_keys: HashMap<PeerId, [u8; 32]>,
    pub created_at: u64,
    pub last_key_rotation: u64,
}

#[derive(Debug, Clone)]
pub struct EscrowKeySet {
    pub key_id: String,
    pub master_key: [u8; 32],
    pub derived_keys: HashMap<String, [u8; 32]>, // purpose -> key
    pub created_at: u64,
    pub expires_at: u64,
    pub participants: Vec<PeerId>,
}

#[derive(Debug)]
pub struct BetValidator {
    pub peer_id: PeerId,
    pub public_key: [u8; 32],
    pub pending_bets: HashMap<String, PendingBet>,
    pub validated_bets: u64,
    pub failed_validations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBet {
    pub bet_id: String,
    pub player: PeerId,
    pub amount: u64,
    pub bet_hash: [u8; 32],
    pub timestamp: u64,
    pub confirmations: Vec<PeerId>,
    pub escrow_signature: Option<Vec<u8>>,
}

impl GamingSecurityManager {
    pub fn new(config: GamingSecurityConfig) -> Self {
        Self {
            game_sessions: RwLock::new(HashMap::new()),
            escrow_keys: RwLock::new(HashMap::new()),
            bet_validators: RwLock::new(HashMap::new()),
            security_config: config,
        }
    }
    
    /// Create secure gaming session with escrow keys
    pub async fn create_gaming_session(
        &self, 
        session_id: String, 
        participants: Vec<PeerId>
    ) -> Result<GameSessionSecurity, SecurityError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Generate escrow key set
        let escrow_key_id = format!("escrow_{}_{}",session_id, now);
        let master_key = self.generate_escrow_key();
        
        let mut derived_keys = HashMap::new();
        derived_keys.insert("bet_validation".to_string(), derive_key(&master_key, b"bet_validation", 32)?);
        derived_keys.insert("payout_signing".to_string(), derive_key(&master_key, b"payout_signing", 32)?);
        derived_keys.insert("round_sealing".to_string(), derive_key(&master_key, b"round_sealing", 32)?);
        
        let escrow_keys = EscrowKeySet {
            key_id: escrow_key_id.clone(),
            master_key,
            derived_keys,
            created_at: now,
            expires_at: now + self.security_config.escrow_key_rotation_interval.as_secs(),
            participants: participants.clone(),
        };
        
        // Generate per-player commitment keys
        let mut bet_commitment_keys = HashMap::new();
        for participant in &participants {
            let commitment_key = self.generate_commitment_key(*participant, &session_id);
            bet_commitment_keys.insert(*participant, commitment_key);
        }
        
        let game_security = GameSessionSecurity {
            session_id: session_id.clone(),
            participants,
            escrow_key_id: escrow_key_id.clone(),
            current_round_key: self.generate_round_key(&session_id),
            bet_commitment_keys,
            created_at: now,
            last_key_rotation: now,
        };
        
        // Store keys and session security
        self.escrow_keys.write().await.insert(escrow_key_id, escrow_keys);
        self.game_sessions.write().await.insert(session_id, game_security.clone());
        
        Ok(game_security)
    }
    
    /// Validate and escrow a bet with cryptographic proof
    pub async fn validate_and_escrow_bet(
        &self,
        session_id: &str,
        bet: &PendingBet,
    ) -> Result<BetEscrowResult, SecurityError> {
        let sessions = self.game_sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or(SecurityError::SessionNotFound)?;
        
        // Verify bet hash integrity
        let calculated_hash = self.calculate_bet_hash(bet);
        if calculated_hash != bet.bet_hash {
            return Err(SecurityError::BetIntegrityFailure);
        }
        
        // Check if enough confirmations
        if bet.confirmations.len() < self.security_config.required_confirmations {
            return Err(SecurityError::InsufficientConfirmations);
        }
        
        // Verify all confirmations are from valid participants
        for confirmer in &bet.confirmations {
            if !session.participants.contains(confirmer) {
                return Err(SecurityError::UnauthorizedConfirmation);
            }
        }
        
        // Get escrow keys
        let escrow_keys = self.escrow_keys.read().await;
        let keys = escrow_keys.get(&session.escrow_key_id)
            .ok_or(SecurityError::EscrowKeysNotFound)?;
        
        // Create escrow signature
        let validation_key = keys.derived_keys.get("bet_validation")
            .ok_or(SecurityError::ValidationKeyMissing)?;
        
        let escrow_signature = self.sign_bet_escrow(bet, validation_key)?;
        
        Ok(BetEscrowResult {
            bet_id: bet.bet_id.clone(),
            escrowed: true,
            escrow_signature,
            validation_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        })
    }
    
    /// Rotate gaming session keys for forward secrecy
    pub async fn rotate_gaming_keys(&self, session_id: &str) -> Result<(), SecurityError> {
        let mut sessions = self.game_sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or(SecurityError::SessionNotFound)?;
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Check if rotation is needed
        if now - session.last_key_rotation < self.security_config.escrow_key_rotation_interval.as_secs() {
            return Err(SecurityError::RotationNotNeeded);
        }
        
        // Generate new round key
        session.current_round_key = self.generate_round_key(&format!("{}_r{}", session_id, now));
        
        // Generate new commitment keys for all participants
        for participant in &session.participants {
            let new_commitment_key = self.generate_commitment_key(*participant, &format!("{}_r{}", session_id, now));
            session.bet_commitment_keys.insert(*participant, new_commitment_key);
        }
        
        session.last_key_rotation = now;
        
        Ok(())
    }
    
    fn generate_escrow_key(&self) -> [u8; 32] {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }
    
    fn generate_commitment_key(&self, participant: PeerId, session_id: &str) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(participant.as_bytes());
        hasher.update(session_id.as_bytes());
        hasher.update(b"commitment_key");
        
        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);
        key
    }
    
    fn generate_round_key(&self, round_identifier: &str) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(round_identifier.as_bytes());
        hasher.update(b"round_key");
        hasher.update(&SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_be_bytes());
        
        let hash = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&hash);
        key
    }
    
    fn calculate_bet_hash(&self, bet: &PendingBet) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        hasher.update(bet.bet_id.as_bytes());
        hasher.update(bet.player.as_bytes());
        hasher.update(&bet.amount.to_be_bytes());
        hasher.update(&bet.timestamp.to_be_bytes());
        
        let hash = hasher.finalize();
        let mut result = [0u8; 32];
        result.copy_from_slice(&hash);
        result
    }
    
    fn sign_bet_escrow(&self, bet: &PendingBet, key: &[u8; 32]) -> Result<Vec<u8>, SecurityError> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        type HmacSha256 = Hmac<Sha256>;
        
        let mut mac = HmacSha256::new_from_slice(key)
            .map_err(|_| SecurityError::SigningError)?;
        
        mac.update(bet.bet_id.as_bytes());
        mac.update(bet.player.as_bytes());
        mac.update(&bet.amount.to_be_bytes());
        
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

#[derive(Debug, Clone)]
pub struct BetEscrowResult {
    pub bet_id: String,
    pub escrowed: bool,
    pub escrow_signature: Vec<u8>,
    pub validation_timestamp: u64,
}

#[derive(Debug)]
pub enum SecurityError {
    SessionNotFound,
    BetIntegrityFailure,
    InsufficientConfirmations,
    UnauthorizedConfirmation,
    EscrowKeysNotFound,
    ValidationKeyMissing,
    SigningError,
    RotationNotNeeded,
}
```

## Summary

Week 4 delivers a complete session management system with:

- **Noise Protocol**: Full XX handshake with state machine
- **Lifecycle Management**: 1-hour/1000-message limits with automatic renewal
- **Forward Secrecy**: Automatic key rotation and secure key storage
- **Channel Encryption**: Argon2id-based encryption with XChaCha20-Poly1305
- **Persistence**: Encrypted session storage with migration support
- **Gaming Security**: Specialized escrow mechanisms for BitCraps betting
- **Bet Validation**: Cryptographic proof system for secure gambling
- **Game Key Rotation**: Enhanced forward secrecy for gaming sessions

The implementation provides robust security guarantees while maintaining performance for mesh network constraints, with additional gaming-specific security features for casino operations.