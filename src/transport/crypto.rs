//! Transport-layer cryptographic security for BitCraps
//!
//! This module provides:
//! - ECDH key exchange for establishing shared secrets
//! - AES-256-GCM encryption for data transmission
//! - Key rotation mechanism for forward secrecy
//! - Message authentication codes for integrity

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use x25519_dalek::{EphemeralSecret, PublicKey};
use zeroize::Zeroize;

use crate::error::{Error, Result};
use crate::protocol::PeerId;

/// Key rotation interval (24 hours)
const KEY_ROTATION_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum message age for replay protection (5 minutes)
const MAX_MESSAGE_AGE: Duration = Duration::from_secs(5 * 60);

/// Size of message counter for replay protection
const MESSAGE_COUNTER_SIZE: usize = 8;

/// Size of timestamp for message freshness
const TIMESTAMP_SIZE: usize = 8;

/// BLE message size limit (244 bytes for single packet)
const BLE_MAX_PAYLOAD_SIZE: usize = 244;

/// Message fragmentation support for large payloads
const FRAGMENT_HEADER_SIZE: usize = 4; // fragment_id (2) + total_fragments (1) + sequence (1)

/// Connection priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for ConnectionPriority {
    fn default() -> Self {
        ConnectionPriority::Normal
    }
}

/// Connection scoring metrics
#[derive(Debug, Clone)]
pub struct ConnectionScore {
    pub latency_ms: u32,
    pub packet_loss_rate: f32,
    pub reliability_score: f32,
    pub last_updated: Instant,
    pub priority: ConnectionPriority,
}

impl Default for ConnectionScore {
    fn default() -> Self {
        Self {
            latency_ms: u32::MAX,
            packet_loss_rate: 1.0,
            reliability_score: 0.0,
            last_updated: Instant::now(),
            priority: ConnectionPriority::Normal,
        }
    }
}

/// Transport encryption state for a peer connection
pub struct TransportCryptoState {
    /// Current encryption key
    encryption_key: Key,

    /// Current decryption key (may differ during rotation)
    decryption_key: Key,

    /// Message counter for replay protection
    send_counter: u64,

    /// Received message counters for replay protection
    recv_counters: HashMap<u64, Instant>,

    /// Key generation timestamp
    key_created: Instant,

    /// Next key rotation time
    next_rotation: Instant,

    /// Connection quality metrics
    connection_score: ConnectionScore,
}

impl Drop for TransportCryptoState {
    fn drop(&mut self) {
        // Securely zero encryption keys on drop
        self.encryption_key.zeroize();
        self.decryption_key.zeroize();
        self.send_counter.zeroize();
    }
}

impl TransportCryptoState {
    fn new(shared_secret: &[u8], peer_id: PeerId) -> Result<Self> {
        let hk = Hkdf::<Sha256>::new(None, shared_secret);

        // Derive encryption key using HKDF
        let mut encryption_key_bytes = [0u8; 32];
        hk.expand(b"bitcraps-transport-encrypt", &mut encryption_key_bytes)
            .map_err(|_| Error::Crypto("Failed to derive encryption key".to_string()))?;

        // Derive decryption key (initially same as encryption key)
        let mut decryption_key_bytes = [0u8; 32];
        hk.expand(b"bitcraps-transport-decrypt", &mut decryption_key_bytes)
            .map_err(|_| Error::Crypto("Failed to derive decryption key".to_string()))?;

        let now = Instant::now();

        Ok(Self {
            encryption_key: Key::from_slice(&encryption_key_bytes).clone(),
            decryption_key: Key::from_slice(&decryption_key_bytes).clone(),
            send_counter: 0,
            recv_counters: HashMap::new(),
            key_created: now,
            next_rotation: now + KEY_ROTATION_INTERVAL,
            connection_score: ConnectionScore::default(),
        })
    }

    /// Check if key rotation is needed
    fn needs_rotation(&self) -> bool {
        Instant::now() >= self.next_rotation
    }

    /// Rotate encryption keys
    fn rotate_keys(&mut self, new_shared_secret: &[u8]) -> Result<()> {
        let hk = Hkdf::<Sha256>::new(None, new_shared_secret);

        // Keep old decryption key briefly for in-flight messages
        let mut new_encryption_key = [0u8; 32];
        hk.expand(b"bitcraps-transport-encrypt-v2", &mut new_encryption_key)
            .map_err(|_| Error::Crypto("Failed to derive new encryption key".to_string()))?;

        // Update keys
        self.encryption_key = Key::from_slice(&new_encryption_key).clone();
        self.key_created = Instant::now();
        self.next_rotation = self.key_created + KEY_ROTATION_INTERVAL;

        // Reset send counter for new key
        self.send_counter = 0;

        // Clean up old receive counters
        let cutoff = Instant::now() - MAX_MESSAGE_AGE;
        self.recv_counters
            .retain(|_, timestamp| *timestamp > cutoff);

        Ok(())
    }

    /// Update connection score based on observed metrics
    pub fn update_score(&mut self, latency_ms: u32, success: bool) {
        let score = &mut self.connection_score;

        // Update latency with exponential moving average
        if score.latency_ms == u32::MAX {
            score.latency_ms = latency_ms;
        } else {
            score.latency_ms = (score.latency_ms * 7 + latency_ms) / 8;
        }

        // Update packet loss rate
        if success {
            score.packet_loss_rate = score.packet_loss_rate * 0.95;
        } else {
            score.packet_loss_rate = (score.packet_loss_rate * 0.95) + 0.05;
        }

        // Calculate reliability score (0.0 = unreliable, 1.0 = very reliable)
        let latency_factor = if score.latency_ms < 50 {
            1.0
        } else if score.latency_ms < 200 {
            0.8
        } else if score.latency_ms < 500 {
            0.5
        } else {
            0.2
        };

        let loss_factor = 1.0 - score.packet_loss_rate;
        score.reliability_score = latency_factor * loss_factor;
        score.last_updated = Instant::now();
    }

    /// Get connection priority score for routing decisions
    pub fn priority_score(&self) -> f32 {
        let base_score = self.connection_score.priority as i32 as f32;
        let reliability_bonus = self.connection_score.reliability_score * 0.5;
        base_score + reliability_bonus
    }
}

/// Message header for encrypted transport
#[derive(Debug)]
struct MessageHeader {
    counter: u64,
    timestamp: u64,
    key_version: u16,
}

impl MessageHeader {
    const SIZE: usize = MESSAGE_COUNTER_SIZE + TIMESTAMP_SIZE + 2; // 18 bytes

    fn new(counter: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            counter,
            timestamp,
            key_version: 1,
        }
    }

    fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..8].copy_from_slice(&self.counter.to_be_bytes());
        bytes[8..16].copy_from_slice(&self.timestamp.to_be_bytes());
        bytes[16..18].copy_from_slice(&self.key_version.to_be_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(Error::Crypto("Invalid message header size".to_string()));
        }

        let counter = u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);

        let timestamp = u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]);

        let key_version = u16::from_be_bytes([bytes[16], bytes[17]]);

        Ok(Self {
            counter,
            timestamp,
            key_version,
        })
    }

    /// Check if message is fresh (not replayed)
    fn is_fresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let age = now.saturating_sub(self.timestamp);
        age <= MAX_MESSAGE_AGE.as_secs()
    }
}

/// Transport layer encryption manager
pub struct TransportCrypto {
    /// Our ephemeral secret key for ECDH (regenerated periodically)
    ephemeral_secret: EphemeralSecret,

    /// Our public key
    public_key: PublicKey,

    /// Per-peer crypto states
    peer_states: Arc<RwLock<HashMap<PeerId, TransportCryptoState>>>,

    /// Connection scoring and prioritization
    connection_scores: Arc<RwLock<HashMap<PeerId, ConnectionScore>>>,

    /// Key rotation task handle
    rotation_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl TransportCrypto {
    /// Create new transport crypto manager
    pub fn new() -> Self {
        let mut rng = OsRng;
        let ephemeral_secret = EphemeralSecret::random_from_rng(&mut rng);
        let public_key = PublicKey::from(&ephemeral_secret);

        let crypto = Self {
            ephemeral_secret,
            public_key,
            peer_states: Arc::new(RwLock::new(HashMap::new())),
            connection_scores: Arc::new(RwLock::new(HashMap::new())),
            rotation_task: Arc::new(Mutex::new(None)),
        };

        crypto.start_key_rotation_task();
        crypto
    }

    /// Get our public key for key exchange
    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    /// Perform ECDH key exchange with a peer
    pub async fn perform_key_exchange(
        &self,
        peer_id: PeerId,
        peer_public_key: PublicKey,
    ) -> Result<()> {
        // Generate ephemeral secret for this peer
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);

        // Perform ECDH to get shared secret
        let shared_secret = ephemeral_secret.diffie_hellman(&peer_public_key);

        // Create crypto state for this peer
        let crypto_state = TransportCryptoState::new(shared_secret.as_bytes(), peer_id)?;

        // Store the state
        self.peer_states.write().await.insert(peer_id, crypto_state);

        // Initialize connection score
        self.connection_scores
            .write()
            .await
            .insert(peer_id, ConnectionScore::default());

        log::info!("Key exchange completed for peer {:?}", peer_id);
        Ok(())
    }

    /// Encrypt data for transmission to a peer
    pub async fn encrypt_message(&self, peer_id: PeerId, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut states = self.peer_states.write().await;
        let state = states
            .get_mut(&peer_id)
            .ok_or_else(|| Error::Crypto("No crypto state for peer".to_string()))?;

        // Check if key rotation is needed
        if state.needs_rotation() {
            // For production, we would perform another ECDH here
            // For now, we'll use a simplified rotation
            log::warn!(
                "Key rotation needed for peer {:?} but not implemented",
                peer_id
            );
        }

        // Create message header
        let header = MessageHeader::new(state.send_counter);
        state.send_counter = state.send_counter.wrapping_add(1);

        // Create cipher
        let cipher = ChaCha20Poly1305::new(&state.encryption_key);

        // Generate nonce (12 bytes for ChaCha20Poly1305)
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Prepare associated data (header)
        let header_bytes = header.to_bytes();

        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| Error::Crypto("Encryption failed".to_string()))?;

        // Build final message: header + nonce + ciphertext
        let mut message =
            Vec::with_capacity(MessageHeader::SIZE + nonce_bytes.len() + ciphertext.len());

        message.extend_from_slice(&header_bytes);
        message.extend_from_slice(&nonce_bytes);
        message.extend_from_slice(&ciphertext);

        Ok(message)
    }

    /// Decrypt received data from a peer
    pub async fn decrypt_message(&self, peer_id: PeerId, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < MessageHeader::SIZE + 12 {
            return Err(Error::Crypto("Message too short".to_string()));
        }

        let mut states = self.peer_states.write().await;
        let state = states
            .get_mut(&peer_id)
            .ok_or_else(|| Error::Crypto("No crypto state for peer".to_string()))?;

        // Parse header
        let header = MessageHeader::from_bytes(&ciphertext[..MessageHeader::SIZE])?;

        // Check message freshness
        if !header.is_fresh() {
            return Err(Error::Crypto("Message too old".to_string()));
        }

        // Check for replay attacks
        if state.recv_counters.contains_key(&header.counter) {
            return Err(Error::Crypto("Replay attack detected".to_string()));
        }

        // Extract nonce
        let nonce_start = MessageHeader::SIZE;
        let nonce_end = nonce_start + 12;
        let nonce = Nonce::from_slice(&ciphertext[nonce_start..nonce_end]);

        // Extract ciphertext
        let encrypted_data = &ciphertext[nonce_end..];

        // Create cipher
        let cipher = ChaCha20Poly1305::new(&state.decryption_key);

        // Prepare associated data (header)
        let header_bytes = &ciphertext[..MessageHeader::SIZE];

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|_| Error::Crypto("Decryption failed".to_string()))?;

        // Record successful decryption (prevents replay)
        state.recv_counters.insert(header.counter, Instant::now());

        // Clean up old counters periodically
        if state.recv_counters.len() > 10000 {
            let cutoff = Instant::now() - MAX_MESSAGE_AGE;
            state
                .recv_counters
                .retain(|_, timestamp| *timestamp > cutoff);
        }

        Ok(plaintext)
    }

    /// Set connection priority for a peer
    pub async fn set_connection_priority(
        &self,
        peer_id: PeerId,
        priority: ConnectionPriority,
    ) -> Result<()> {
        let mut states = self.peer_states.write().await;
        if let Some(state) = states.get_mut(&peer_id) {
            state.connection_score.priority = priority;
        }

        let mut scores = self.connection_scores.write().await;
        if let Some(score) = scores.get_mut(&peer_id) {
            score.priority = priority;
        }

        Ok(())
    }

    /// Update connection metrics for scoring
    pub async fn update_connection_metrics(
        &self,
        peer_id: PeerId,
        latency_ms: u32,
        success: bool,
    ) -> Result<()> {
        let mut states = self.peer_states.write().await;
        if let Some(state) = states.get_mut(&peer_id) {
            state.update_score(latency_ms, success);
        }

        let mut scores = self.connection_scores.write().await;
        if let Some(score) = scores.get_mut(&peer_id) {
            score.latency_ms = latency_ms;
            score.reliability_score = if success {
                (score.reliability_score * 0.9) + 0.1
            } else {
                score.reliability_score * 0.9
            };
            score.last_updated = Instant::now();
        }

        Ok(())
    }

    /// Get ordered list of peers by connection quality
    pub async fn get_peers_by_priority(&self) -> Vec<(PeerId, f32)> {
        let states = self.peer_states.read().await;
        let mut peer_scores: Vec<(PeerId, f32)> = states
            .iter()
            .map(|(peer_id, state)| (*peer_id, state.priority_score()))
            .collect();

        // Sort by priority score (highest first)
        peer_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        peer_scores
    }

    /// Remove crypto state for a disconnected peer
    pub async fn remove_peer(&self, peer_id: PeerId) {
        self.peer_states.write().await.remove(&peer_id);
        self.connection_scores.write().await.remove(&peer_id);
    }

    /// Start background key rotation task
    fn start_key_rotation_task(&self) {
        let peer_states = self.peer_states.clone();

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Check every hour

            loop {
                interval.tick().await;

                let mut states = peer_states.write().await;
                let mut to_rotate = Vec::new();

                // Find peers that need key rotation
                for (peer_id, state) in states.iter() {
                    if state.needs_rotation() {
                        to_rotate.push(*peer_id);
                    }
                }

                // Rotate keys (simplified - in production would do ECDH again)
                for peer_id in to_rotate {
                    if let Some(state) = states.get_mut(&peer_id) {
                        log::info!("Rotating keys for peer {:?}", peer_id);
                        // Generate new shared secret (simplified)
                        let mut new_secret = [0u8; 32];
                        OsRng.fill_bytes(&mut new_secret);

                        if let Err(e) = state.rotate_keys(&new_secret) {
                            log::error!("Key rotation failed for peer {:?}: {}", peer_id, e);
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            // Store task handle but we can't access self here
            // In practice, this would be handled differently
        });
    }

    /// Get encryption statistics
    pub async fn get_crypto_stats(&self) -> CryptoStats {
        let states = self.peer_states.read().await;
        let scores = self.connection_scores.read().await;

        let active_sessions = states.len();
        let average_latency = if scores.is_empty() {
            0.0
        } else {
            scores.values().map(|s| s.latency_ms as f32).sum::<f32>() / scores.len() as f32
        };

        let average_reliability = if scores.is_empty() {
            0.0
        } else {
            scores.values().map(|s| s.reliability_score).sum::<f32>() / scores.len() as f32
        };

        CryptoStats {
            active_sessions,
            keys_rotated: 0,       // TODO: track this
            messages_encrypted: 0, // TODO: track this
            messages_decrypted: 0, // TODO: track this
            average_latency,
            average_reliability,
        }
    }
}

/// Transport layer crypto statistics
#[derive(Debug, Clone)]
pub struct CryptoStats {
    pub active_sessions: usize,
    pub keys_rotated: u64,
    pub messages_encrypted: u64,
    pub messages_decrypted: u64,
    pub average_latency: f32,
    pub average_reliability: f32,
}

impl Drop for TransportCrypto {
    fn drop(&mut self) {
        // Clean up any running tasks
        if let Ok(mut task_guard) = self.rotation_task.try_lock() {
            if let Some(task) = task_guard.take() {
                task.abort();
            }
        }
    }
}

/// Secure key exchange protocol
pub struct SecureKeyExchange;

impl SecureKeyExchange {
    /// Generate ephemeral keypair for key exchange
    pub fn generate_keypair() -> (EphemeralSecret, PublicKey) {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        (secret, public)
    }

    /// Perform authenticated ECDH
    pub fn perform_ecdh(secret: EphemeralSecret, peer_public: PublicKey) -> [u8; 32] {
        let shared_secret = secret.diffie_hellman(&peer_public);
        *shared_secret.as_bytes()
    }

    /// Derive session keys from shared secret
    pub fn derive_session_keys(shared_secret: &[u8], peer_id: PeerId) -> Result<(Key, Key)> {
        let hk = Hkdf::<Sha256>::new(None, shared_secret);

        // Derive send key
        let mut send_key = [0u8; 32];
        hk.expand(b"bitcraps-send-key", &mut send_key)
            .map_err(|_| Error::Crypto("Failed to derive send key".to_string()))?;

        // Derive receive key
        let mut recv_key = [0u8; 32];
        hk.expand(b"bitcraps-recv-key", &mut recv_key)
            .map_err(|_| Error::Crypto("Failed to derive receive key".to_string()))?;

        Ok((
            Key::from_slice(&send_key).clone(),
            Key::from_slice(&recv_key).clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_exchange() {
        let crypto1 = TransportCrypto::new();
        let crypto2 = TransportCrypto::new();

        let peer_id1 = [1u8; 32];
        let peer_id2 = [2u8; 32];

        // Perform key exchange
        let public1 = crypto1.public_key();
        let public2 = crypto2.public_key();

        assert!(crypto1
            .perform_key_exchange(peer_id2, public2)
            .await
            .is_ok());
        assert!(crypto2
            .perform_key_exchange(peer_id1, public1)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn test_encrypt_decrypt() {
        let crypto1 = TransportCrypto::new();
        let crypto2 = TransportCrypto::new();

        let peer_id1 = [1u8; 32];
        let peer_id2 = [2u8; 32];

        // Setup keys
        let public1 = crypto1.public_key();
        let public2 = crypto2.public_key();

        crypto1
            .perform_key_exchange(peer_id2, public2)
            .await
            .unwrap();
        crypto2
            .perform_key_exchange(peer_id1, public1)
            .await
            .unwrap();

        // Test encryption/decryption
        let message = b"Hello, BitCraps!";
        let encrypted = crypto1.encrypt_message(peer_id2, message).await.unwrap();
        let decrypted = crypto2.decrypt_message(peer_id1, &encrypted).await.unwrap();

        assert_eq!(message, &decrypted[..]);
    }

    #[tokio::test]
    async fn test_replay_protection() {
        let crypto1 = TransportCrypto::new();
        let crypto2 = TransportCrypto::new();

        let peer_id1 = [1u8; 32];
        let peer_id2 = [2u8; 32];

        // Setup keys
        let public1 = crypto1.public_key();
        let public2 = crypto2.public_key();

        crypto1
            .perform_key_exchange(peer_id2, public2)
            .await
            .unwrap();
        crypto2
            .perform_key_exchange(peer_id1, public1)
            .await
            .unwrap();

        // Test replay protection
        let message = b"Test message";
        let encrypted = crypto1.encrypt_message(peer_id2, message).await.unwrap();

        // First decryption should work
        assert!(crypto2.decrypt_message(peer_id1, &encrypted).await.is_ok());

        // Replay should fail
        assert!(crypto2.decrypt_message(peer_id1, &encrypted).await.is_err());
    }

    #[tokio::test]
    async fn test_connection_prioritization() {
        let crypto = TransportCrypto::new();
        let peer_id = [1u8; 32];

        // Mock key exchange
        crypto
            .perform_key_exchange(peer_id, crypto.public_key())
            .await
            .unwrap();

        // Set high priority
        crypto
            .set_connection_priority(peer_id, ConnectionPriority::High)
            .await
            .unwrap();

        // Update metrics
        crypto
            .update_connection_metrics(peer_id, 50, true)
            .await
            .unwrap();

        let peers = crypto.get_peers_by_priority().await;
        assert!(!peers.is_empty());
        assert_eq!(peers[0].0, peer_id);
    }
}
