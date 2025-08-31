//! Session management for BitCraps
//!
//! This module implements simplified session management including:
//! - Basic session lifecycle management  
//! - Simple encrypted channel communication
//! - Session persistence and recovery (simplified)
//! - Noise protocol integration
//! - Forward secrecy with key rotation

pub mod forward_secrecy;
pub mod lifecycle;
pub mod noise;
pub mod state;

use chacha20poly1305::{AeadInPlace, ChaCha20Poly1305, Key, KeyInit, Nonce};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

use crate::crypto::BitchatKeypair;
use crate::error::{Error, Result};
use crate::protocol::PeerId;

/// Session identifier
pub type SessionId = [u8; 16];

/// Session state machine (simplified)
#[derive(Debug, Clone)]
pub enum SessionState {
    Active,
    Expired,
}

/// BitCraps session (simplified)
#[derive(Debug, Clone)]
pub struct BitchatSession {
    pub session_id: SessionId,
    pub peer_id: PeerId,
    pub local_keypair: BitchatKeypair,
    pub state: SessionState,
    pub metrics: SessionMetrics,
    encryption_key: [u8; 32],
    nonce_counter: u64,
}

/// Session metrics and limits
#[derive(Debug, Clone)]
pub struct SessionMetrics {
    pub created_at: Instant,
    pub last_activity: Instant,
    pub message_count: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Session limits configuration
#[derive(Debug, Clone)]
pub struct SessionLimits {
    pub max_duration: Duration,
    pub max_message_count: u32,
    pub warning_threshold: f32,
}

impl Default for SessionLimits {
    fn default() -> Self {
        Self {
            max_duration: Duration::from_secs(3600), // 1 hour
            max_message_count: 1000,
            warning_threshold: 0.8, // 80%
        }
    }
}

/// Session status
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Active,
    NearExpiry {
        reason: ExpiryReason,
        remaining: Duration,
    },
    Expired {
        reason: ExpiryReason,
    },
    Renewed {
        old_session_id: SessionId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpiryReason {
    TimeLimit,
    MessageLimit,
    IdleTimeout,
}

/// Session manager coordinating all active sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, BitchatSession>>>,
    limits: SessionLimits,
    event_sender: mpsc::Sender<SessionEvent>,
}

/// Session events
#[derive(Debug, Clone)]
pub enum SessionEvent {
    SessionEstablished {
        session_id: SessionId,
        peer_id: PeerId,
    },
    SessionExpired {
        session_id: SessionId,
        reason: ExpiryReason,
    },
    SessionRenewed {
        old_session_id: SessionId,
        new_session_id: SessionId,
    },
    MessageReceived {
        session_id: SessionId,
        data: Vec<u8>,
    },
    KeyRotated {
        session_id: SessionId,
    },
    HandshakeCompleted {
        session_id: SessionId,
    },
    SessionError {
        session_id: SessionId,
        error: String,
    },
}

impl BitchatSession {
    /// Create new session (simplified)
    pub fn new_initiator(peer_id: PeerId, local_keypair: BitchatKeypair) -> Result<Self> {
        let session_id = Self::generate_session_id();

        // Generate session encryption key from keypair
        let encryption_key = Self::derive_encryption_key(&local_keypair, &peer_id);

        Ok(Self {
            session_id,
            peer_id,
            local_keypair,
            state: SessionState::Active,
            metrics: SessionMetrics {
                created_at: Instant::now(),
                last_activity: Instant::now(),
                message_count: 0,
                bytes_sent: 0,
                bytes_received: 0,
            },
            encryption_key,
            nonce_counter: 0,
        })
    }

    /// Encrypt and send message using ChaCha20-Poly1305
    pub fn encrypt_message(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Input validation
        if plaintext.is_empty() {
            return Err(Error::InvalidData(
                "Cannot encrypt empty message".to_string(),
            ));
        }
        if plaintext.len() > 1024 * 1024 {
            return Err(Error::InvalidData(
                "Message too large for encryption".to_string(),
            ));
        }

        // Create cipher with session key
        let key = Key::from_slice(&self.encryption_key);
        let cipher = ChaCha20Poly1305::new(key);

        // Generate unique nonce for this message
        self.nonce_counter += 1;
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[..8].copy_from_slice(&self.nonce_counter.to_le_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt plaintext
        let mut buffer = plaintext.to_vec();
        let tag = cipher
            .encrypt_in_place_detached(nonce, b"", &mut buffer)
            .map_err(|e| Error::Crypto(format!("Encryption failed: {}", e)))?;

        // Construct result: nonce (12 bytes) + tag (16 bytes) + ciphertext
        let mut result = Vec::with_capacity(12 + 16 + buffer.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&tag);
        result.extend_from_slice(&buffer);

        self.metrics.message_count += 1;
        self.metrics.bytes_sent += result.len() as u64;
        self.metrics.last_activity = Instant::now();

        Ok(result)
    }

    /// Decrypt received message using ChaCha20-Poly1305
    pub fn decrypt_message(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // Input validation
        if ciphertext.len() < 28 {
            return Err(Error::Crypto(
                "Ciphertext too short (minimum 28 bytes: 12 nonce + 16 tag)".to_string(),
            ));
        }
        if ciphertext.len() > 1024 * 1024 + 28 {
            return Err(Error::Crypto("Ciphertext too large".to_string()));
        }

        // Extract nonce, tag, and encrypted data
        let nonce_bytes = &ciphertext[..12];
        let tag_bytes = &ciphertext[12..28];
        let encrypted_data = &ciphertext[28..];

        // Create cipher with session key
        let key = Key::from_slice(&self.encryption_key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt data
        let mut buffer = encrypted_data.to_vec();
        cipher
            .decrypt_in_place_detached(nonce, b"", &mut buffer, tag_bytes.into())
            .map_err(|e| Error::Crypto(format!("Decryption failed: {}", e)))?;

        self.metrics.bytes_received += ciphertext.len() as u64;
        self.metrics.last_activity = Instant::now();

        Ok(buffer)
    }

    /// Check session health
    pub fn check_health(&self, limits: &SessionLimits) -> SessionStatus {
        let now = Instant::now();
        let age = now.duration_since(self.metrics.created_at);

        // Check time limit
        if age >= limits.max_duration {
            return SessionStatus::Expired {
                reason: ExpiryReason::TimeLimit,
            };
        }

        // Check message count limit
        if self.metrics.message_count >= limits.max_message_count {
            return SessionStatus::Expired {
                reason: ExpiryReason::MessageLimit,
            };
        }

        // Check for near expiry warnings
        let time_progress = age.as_secs_f32() / limits.max_duration.as_secs_f32();
        let msg_progress = self.metrics.message_count as f32 / limits.max_message_count as f32;

        if time_progress >= limits.warning_threshold {
            return SessionStatus::NearExpiry {
                reason: ExpiryReason::TimeLimit,
                remaining: limits.max_duration - age,
            };
        }

        if msg_progress >= limits.warning_threshold {
            return SessionStatus::NearExpiry {
                reason: ExpiryReason::MessageLimit,
                remaining: Duration::from_secs(
                    ((limits.max_message_count - self.metrics.message_count) * 60) as u64,
                ),
            };
        }

        SessionStatus::Active
    }

    /// Generate session ID using cryptographic randomness
    fn generate_session_id() -> SessionId {
        let mut session_id = [0u8; 16];
        use rand::{rngs::OsRng, RngCore};
        let mut rng = OsRng;
        rng.fill_bytes(&mut session_id);
        session_id
    }

    /// Derive encryption key from keypair and peer ID
    fn derive_encryption_key(keypair: &BitchatKeypair, peer_id: &PeerId) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(b"BITCRAPS_SESSION_KEY_V1");
        hasher.update(keypair.secret_key_bytes());
        hasher.update(peer_id);

        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
}

impl SessionManager {
    pub fn new(limits: SessionLimits) -> Self {
        let (event_sender, _) = mpsc::channel(1000); // Moderate traffic for session events

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            limits,
            event_sender,
        }
    }

    /// Add new session
    pub async fn add_session(&self, session: BitchatSession) {
        let session_id = session.session_id;
        let peer_id = session.peer_id;

        self.sessions.write().await.insert(session_id, session);

        let _ = self.event_sender.send(SessionEvent::SessionEstablished {
            session_id,
            peer_id,
        });
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &SessionId) -> Option<BitchatSession> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// Send encrypted message through session
    pub async fn send_encrypted_message(
        &self,
        session_id: &SessionId,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::Protocol("Session not found".to_string()))?;

        let encrypted = session.encrypt_message(plaintext)?;

        Ok(encrypted)
    }

    /// Process received encrypted message
    pub async fn process_encrypted_message(
        &self,
        session_id: &SessionId,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| Error::Protocol("Session not found".to_string()))?;

        let plaintext = session.decrypt_message(ciphertext)?;

        let _ = self.event_sender.send(SessionEvent::MessageReceived {
            session_id: *session_id,
            data: plaintext.clone(),
        });

        Ok(plaintext)
    }

    /// Check all sessions for expiry
    pub async fn check_session_health(&self) {
        let mut sessions = self.sessions.write().await;
        let mut expired_sessions = Vec::new();

        for (session_id, session) in sessions.iter() {
            match session.check_health(&self.limits) {
                SessionStatus::Expired { reason } => {
                    expired_sessions.push((*session_id, reason));
                }
                SessionStatus::NearExpiry { .. } => {
                    // Could send warning events here
                }
                _ => {}
            }
        }

        // Remove expired sessions
        for (session_id, reason) in expired_sessions {
            sessions.remove(&session_id);
            let _ = self
                .event_sender
                .send(SessionEvent::SessionExpired { session_id, reason });
        }
    }

    /// Get session statistics
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let active_sessions = sessions.len();

        let total_messages: u32 = sessions.values().map(|s| s.metrics.message_count).sum();

        let total_bytes_sent: u64 = sessions.values().map(|s| s.metrics.bytes_sent).sum();

        let total_bytes_received: u64 = sessions.values().map(|s| s.metrics.bytes_received).sum();

        SessionStats {
            active_sessions,
            total_messages,
            total_bytes_sent,
            total_bytes_received,
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub active_sessions: usize,
    pub total_messages: u32,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::BitchatKeypair;

    #[test]
    fn test_session_creation() {
        let keypair = BitchatKeypair::generate();
        let peer_id = [1u8; 32];

        let session = BitchatSession::new_initiator(peer_id, keypair).unwrap();

        assert_eq!(session.peer_id, peer_id);
        assert!(matches!(session.state, SessionState::Active));
    }

    #[test]
    fn test_encryption_decryption() {
        let keypair = BitchatKeypair::generate();
        let peer_id = [1u8; 32];
        let mut session = BitchatSession::new_initiator(peer_id, keypair).unwrap();

        let plaintext = b"Hello, secure world!";
        let ciphertext = session.encrypt_message(plaintext).unwrap();
        let decrypted = session.decrypt_message(&ciphertext).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encryption_input_validation() {
        let keypair = BitchatKeypair::generate();
        let peer_id = [1u8; 32];
        let mut session = BitchatSession::new_initiator(peer_id, keypair).unwrap();

        // Test empty message
        assert!(session.encrypt_message(&[]).is_err());

        // Test oversized message
        let large_message = vec![0u8; 1024 * 1024 + 1];
        assert!(session.encrypt_message(&large_message).is_err());
    }

    #[tokio::test]
    async fn test_session_manager() {
        let manager = SessionManager::new(SessionLimits::default());

        let keypair = BitchatKeypair::generate();
        let peer_id = [1u8; 32];
        let session = BitchatSession::new_initiator(peer_id, keypair).unwrap();
        let session_id = session.session_id;

        manager.add_session(session).await;

        let retrieved = manager.get_session(&session_id).await;
        assert!(retrieved.is_some());

        let stats = manager.get_stats().await;
        assert_eq!(stats.active_sessions, 1);
    }

    #[tokio::test]
    async fn test_encrypted_message_through_manager() {
        let manager = SessionManager::new(SessionLimits::default());

        let keypair = BitchatKeypair::generate();
        let peer_id = [1u8; 32];
        let session = BitchatSession::new_initiator(peer_id, keypair).unwrap();
        let session_id = session.session_id;

        manager.add_session(session).await;

        let plaintext = b"Test message through manager";
        let ciphertext = manager
            .send_encrypted_message(&session_id, plaintext)
            .await
            .unwrap();
        let decrypted = manager
            .process_encrypted_message(&session_id, &ciphertext)
            .await
            .unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }
}
