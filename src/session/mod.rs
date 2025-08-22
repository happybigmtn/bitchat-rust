//! Session management for BitCraps
//! 
//! This module implements simplified session management including:
//! - Basic session lifecycle management  
//! - Simple encrypted channel communication
//! - Session persistence and recovery (simplified)
//! - Noise protocol integration
//! - Forward secrecy with key rotation

pub mod noise;
pub mod state;
pub mod lifecycle;
pub mod forward_secrecy;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

use crate::protocol::PeerId;
use crate::crypto::BitchatKeypair;
use crate::error::{Error, Result};

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
    NearExpiry { reason: ExpiryReason, remaining: Duration },
    Expired { reason: ExpiryReason },
    Renewed { old_session_id: SessionId },
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
    event_sender: mpsc::UnboundedSender<SessionEvent>,
}

/// Session events
#[derive(Debug, Clone)]
pub enum SessionEvent {
    SessionEstablished { session_id: SessionId, peer_id: PeerId },
    SessionExpired { session_id: SessionId, reason: ExpiryReason },
    SessionRenewed { old_session_id: SessionId, new_session_id: SessionId },
    MessageReceived { session_id: SessionId, data: Vec<u8> },
    KeyRotated { session_id: SessionId },
    HandshakeCompleted { session_id: SessionId },
    SessionError { session_id: SessionId, error: String },
}

impl BitchatSession {
    /// Create new session (simplified)
    pub fn new_initiator(
        peer_id: PeerId,
        local_keypair: BitchatKeypair,
    ) -> Result<Self> {
        let session_id = Self::generate_session_id();
        
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
        })
    }
    
    /// Encrypt and send message (simplified)
    pub fn encrypt_message(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Simplified encryption - just return the plaintext with a header
        // In production, would use proper encryption
        let mut result = Vec::new();
        result.extend_from_slice(b"ENCRYPTED:");
        result.extend_from_slice(plaintext);
        
        self.metrics.message_count += 1;
        self.metrics.bytes_sent += result.len() as u64;
        self.metrics.last_activity = Instant::now();
        
        Ok(result)
    }
    
    /// Decrypt received message (simplified)
    pub fn decrypt_message(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        // Simplified decryption - just remove the header
        if ciphertext.starts_with(b"ENCRYPTED:") {
            let plaintext = ciphertext[10..].to_vec();
            self.metrics.bytes_received += ciphertext.len() as u64;
            self.metrics.last_activity = Instant::now();
            Ok(plaintext)
        } else {
            Err(Error::Crypto("Invalid encrypted message".to_string()))
        }
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
                    ((limits.max_message_count - self.metrics.message_count) * 60) as u64
                ),
            };
        }
        
        SessionStatus::Active
    }
    
    /// Generate session ID
    fn generate_session_id() -> SessionId {
        let mut session_id = [0u8; 16];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut session_id);
        session_id
    }
}

impl SessionManager {
    pub fn new(limits: SessionLimits) -> Self {
        let (event_sender, _) = mpsc::unbounded_channel();
        
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
        let session = sessions.get_mut(session_id)
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
        let session = sessions.get_mut(session_id)
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
            let _ = self.event_sender.send(SessionEvent::SessionExpired {
                session_id,
                reason,
            });
        }
    }
    
    /// Get session statistics
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let active_sessions = sessions.len();
        
        let total_messages: u32 = sessions.values()
            .map(|s| s.metrics.message_count)
            .sum();
        
        let total_bytes_sent: u64 = sessions.values()
            .map(|s| s.metrics.bytes_sent)
            .sum();
        
        let total_bytes_received: u64 = sessions.values()
            .map(|s| s.metrics.bytes_received)
            .sum();
        
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
}