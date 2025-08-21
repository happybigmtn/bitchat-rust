// src/crypto/noise.rs
use snow::{Builder, HandshakeState, TransportState, params::NoiseParams};
use std::collections::HashMap;
use super::keys::{BitchatIdentity, NoiseKeyPair};
use crate::protocol::{PeerId, ProtocolError, ProtocolResult};

/// Noise protocol pattern: Noise_XX_25519_ChaChaPoly_SHA256
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";

#[derive(Debug)]
pub enum NoiseSessionState {
    /// Handshake in progress
    Handshaking(Option<HandshakeState>),
    /// Established transport state
    Transport(TransportState),
}

pub struct NoiseSession {
    pub state: NoiseSessionState,
    pub remote_peer_id: Option<PeerId>,
    pub is_initiator: bool,
}

pub struct NoiseEncryptionService {
    identity: BitchatIdentity,
    sessions: HashMap<PeerId, NoiseSession>,
    params: NoiseParams,
}

impl NoiseEncryptionService {
    pub fn new(identity: BitchatIdentity) -> ProtocolResult<Self> {
        let params = NOISE_PATTERN.parse()
            .map_err(|e| ProtocolError::CryptographicError(format!("Invalid noise params: {}", e)))?;
            
        Ok(Self {
            identity,
            sessions: HashMap::new(),
            params,
        })
    }
    
    /// Initiate a handshake with a remote peer
    pub fn initiate_handshake(&mut self, remote_peer_id: PeerId) -> ProtocolResult<Vec<u8>> {
        let builder = Builder::new(self.params.clone());
        let static_key = self.identity.noise_keypair.private_bytes();
        
        let mut handshake = builder
            .local_private_key(&static_key)
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to set private key: {}", e)))?
            .build_initiator()
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to build initiator: {}", e)))?;
        
        let mut buffer = vec![0u8; 65536]; // Large buffer for handshake
        let len = handshake
            .write_message(&[], &mut buffer)
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to write handshake message: {}", e)))?;
        
        buffer.truncate(len);
        
        // Store the handshake state
        let session = NoiseSession {
            state: NoiseSessionState::Handshaking(Some(handshake)),
            remote_peer_id: Some(remote_peer_id),
            is_initiator: true,
        };
        
        self.sessions.insert(remote_peer_id, session);
        
        Ok(buffer)
    }
    
    /// Respond to a handshake initiation
    pub fn respond_to_handshake(&mut self, message: &[u8]) -> ProtocolResult<(Vec<u8>, PeerId)> {
        let builder = Builder::new(self.params.clone());
        let static_key = self.identity.noise_keypair.private_bytes();
        
        let mut handshake = builder
            .local_private_key(&static_key)
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to set private key: {}", e)))?
            .build_responder()
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to build responder: {}", e)))?;
        
        // Read the incoming handshake message
        let mut payload = vec![0u8; 65536];
        let len = handshake
            .read_message(message, &mut payload)
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to read handshake message: {}", e)))?;
        
        // Generate response
        let mut response = vec![0u8; 65536];
        let response_len = handshake
            .write_message(&[], &mut response)
            .map_err(|e| ProtocolError::CryptographicError(format!("Failed to write handshake response: {}", e)))?;
        
        response.truncate(response_len);
        
        // Extract remote static key to determine peer ID
        let remote_static = handshake
            .get_remote_static()
            .ok_or_else(|| ProtocolError::CryptographicError("No remote static key".to_string()))?;
        
        let remote_peer_id = PeerId::from_public_key(
            &remote_static.try_into().map_err(|_| 
                ProtocolError::CryptographicError("Invalid remote static key length".to_string())
            )?
        );
        
        // Store the handshake state
        let session = NoiseSession {
            state: NoiseSessionState::Handshaking(Some(handshake)),
            remote_peer_id: Some(remote_peer_id),
            is_initiator: false,
        };
        
        self.sessions.insert(remote_peer_id, session);
        
        Ok((response, remote_peer_id))
    }
    
    /// Complete handshake (for initiator receiving response)
    pub fn complete_handshake(&mut self, peer_id: PeerId, message: &[u8]) -> ProtocolResult<()> {
        let session = self.sessions.get_mut(&peer_id)
            .ok_or_else(|| ProtocolError::CryptographicError("No handshake session found".to_string()))?;
        
        match &mut session.state {
            NoiseSessionState::Handshaking(handshake_opt) => {
                if let Some(mut handshake) = handshake_opt.take() {
                    // Read the final handshake message
                    let mut payload = vec![0u8; 65536];
                    let _len = handshake
                        .read_message(message, &mut payload)
                        .map_err(|e| ProtocolError::CryptographicError(format!("Failed to complete handshake: {}", e)))?;
                    
                    // Convert to transport mode
                    let transport = handshake
                        .into_transport_mode()
                        .map_err(|e| ProtocolError::CryptographicError(format!("Failed to enter transport mode: {}", e)))?;
                    
                    session.state = NoiseSessionState::Transport(transport);
                    Ok(())
                } else {
                    Err(ProtocolError::CryptographicError("Handshake already consumed".to_string()))
                }
            }
            NoiseSessionState::Transport(_) => {
                Err(ProtocolError::CryptographicError("Session already in transport mode".to_string()))
            }
        }
    }
    
    /// Encrypt a message for a specific peer
    pub fn encrypt(&mut self, peer_id: PeerId, plaintext: &[u8]) -> ProtocolResult<Vec<u8>> {
        let session = self.sessions.get_mut(&peer_id)
            .ok_or_else(|| ProtocolError::CryptographicError("No session found for peer".to_string()))?;
        
        match &mut session.state {
            NoiseSessionState::Transport(transport) => {
                let mut buffer = vec![0u8; plaintext.len() + 16]; // Add space for auth tag
                let len = transport
                    .write_message(plaintext, &mut buffer)
                    .map_err(|e| ProtocolError::CryptographicError(format!("Encryption failed: {}", e)))?;
                
                buffer.truncate(len);
                Ok(buffer)
            }
            NoiseSessionState::Handshaking(_) => {
                Err(ProtocolError::CryptographicError("Cannot encrypt during handshake".to_string()))
            }
        }
    }
    
    /// Decrypt a message from a specific peer
    pub fn decrypt(&mut self, peer_id: PeerId, ciphertext: &[u8]) -> ProtocolResult<Vec<u8>> {
        let session = self.sessions.get_mut(&peer_id)
            .ok_or_else(|| ProtocolError::CryptographicError("No session found for peer".to_string()))?;
        
        match &mut session.state {
            NoiseSessionState::Transport(transport) => {
                let mut buffer = vec![0u8; ciphertext.len()];
                let len = transport
                    .read_message(ciphertext, &mut buffer)
                    .map_err(|e| ProtocolError::CryptographicError(format!("Decryption failed: {}", e)))?;
                
                buffer.truncate(len);
                Ok(buffer)
            }
            NoiseSessionState::Handshaking(_) => {
                Err(ProtocolError::CryptographicError("Cannot decrypt during handshake".to_string()))
            }
        }
    }
    
    /// Check if we have an established session with a peer
    pub fn has_session(&self, peer_id: &PeerId) -> bool {
        self.sessions.get(peer_id)
            .map(|s| matches!(s.state, NoiseSessionState::Transport(_)))
            .unwrap_or(false)
    }
    
    /// Remove a session (for cleanup)
    pub fn remove_session(&mut self, peer_id: &PeerId) {
        self.sessions.remove(peer_id);
    }
    
    /// Get our public identity
    pub fn get_identity(&self) -> &BitchatIdentity {
        &self.identity
    }
}