use super::noise::{NoiseRole, NoiseSession};
use super::state::SessionState;
use crate::error::Result;
use crate::protocol::PeerId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct SessionLifecycleManager {
    sessions: Arc<RwLock<HashMap<PeerId, ManagedSession>>>,
    config: SessionConfig,
}

pub struct SessionConfig {
    pub session_timeout: Duration,
    pub handshake_timeout: Duration,
    pub max_sessions: usize,
    pub enable_persistence: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            session_timeout: Duration::from_secs(3600),
            handshake_timeout: Duration::from_secs(30),
            max_sessions: 1000,
            enable_persistence: true,
        }
    }
}

pub struct ManagedSession {
    pub session: NoiseSession,
    pub state: SessionState,
    pub peer_id: PeerId,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub rekey_counter: u64,
}

impl SessionLifecycleManager {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn create_session(
        &self,
        peer_id: PeerId,
        role: NoiseRole,
        keypair: &crate::crypto::BitchatKeypair,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if sessions.len() >= self.config.max_sessions {
            self.cleanup_stale_sessions(&mut sessions);
        }

        let noise_session = match role {
            NoiseRole::Initiator => NoiseSession::new_initiator(keypair)?,
            NoiseRole::Responder => NoiseSession::new_responder(keypair)?,
        };

        let managed_session = ManagedSession {
            session: noise_session,
            state: SessionState::Initializing,
            peer_id,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
            rekey_counter: 0,
        };

        sessions.insert(peer_id, managed_session);
        Ok(())
    }

    pub async fn get_session_ref(
        &self,
        peer_id: &PeerId,
    ) -> Option<tokio::sync::RwLockReadGuard<'_, std::collections::HashMap<PeerId, ManagedSession>>>
    {
        let sessions = self.sessions.read().await;
        if sessions.contains_key(peer_id) {
            Some(sessions)
        } else {
            None
        }
    }

    /// Get session mutably for cryptographic operations that modify state
    pub async fn get_session_mut(
        &self,
        peer_id: &PeerId,
    ) -> Option<tokio::sync::RwLockWriteGuard<'_, std::collections::HashMap<PeerId, ManagedSession>>>
    {
        let sessions = self.sessions.write().await;
        if sessions.contains_key(peer_id) {
            Some(sessions)
        } else {
            None
        }
    }

    pub async fn update_activity(&self, peer_id: &PeerId) {
        if let Some(session) = self.sessions.write().await.get_mut(peer_id) {
            session.last_activity = Instant::now();
        }
    }

    pub async fn transition_state(&self, peer_id: &PeerId, new_state: SessionState) -> Result<()> {
        if let Some(session) = self.sessions.write().await.get_mut(peer_id) {
            // Validate state transition
            let valid_transition = match (&session.state, &new_state) {
                (SessionState::Initializing, SessionState::Handshaking) => true,
                (SessionState::Handshaking, SessionState::Active) => true,
                (SessionState::Active, SessionState::Rekeying) => true,
                (SessionState::Rekeying, SessionState::Active) => true,
                (_, SessionState::Terminated) => true,
                _ => false,
            };

            if valid_transition {
                session.state = new_state;
                Ok(())
            } else {
                Err(crate::error::Error::InvalidState(format!(
                    "Invalid state transition from {:?} to {:?}",
                    session.state, new_state
                )))
            }
        } else {
            Err(crate::error::Error::SessionNotFound)
        }
    }

    pub async fn terminate_session(&self, peer_id: &PeerId) -> Result<()> {
        if let Some(mut session) = self.sessions.write().await.remove(peer_id) {
            session.session.terminate();
            Ok(())
        } else {
            Err(crate::error::Error::SessionNotFound)
        }
    }

    fn cleanup_stale_sessions(&self, sessions: &mut HashMap<PeerId, ManagedSession>) {
        let now = Instant::now();
        sessions.retain(|_, session| {
            now.duration_since(session.last_activity) < self.config.session_timeout
        });
    }

    pub async fn cleanup_expired(&self) {
        let mut sessions = self.sessions.write().await;
        self.cleanup_stale_sessions(&mut sessions);
    }
}
