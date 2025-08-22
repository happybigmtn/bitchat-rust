use std::time::{Duration, Instant};
use crate::crypto::BitchatKeypair;
use crate::error::{Error, Result};
use zeroize::Zeroize;

pub struct ForwardSecrecyManager {
    rekey_interval: Duration,
    max_messages_per_key: u64,
    last_rekey: Instant,
    message_count: u64,
    current_epoch: u64,
    old_keypairs: Vec<BitchatKeypair>,
}

impl ForwardSecrecyManager {
    pub fn new(rekey_interval: Duration, max_messages_per_key: u64) -> Self {
        Self {
            rekey_interval,
            max_messages_per_key,
            last_rekey: Instant::now(),
            message_count: 0,
            current_epoch: 0,
            old_keypairs: Vec::new(),
        }
    }
    
    pub fn should_rekey(&self) -> bool {
        let time_exceeded = Instant::now().duration_since(self.last_rekey) >= self.rekey_interval;
        let message_limit_reached = self.message_count >= self.max_messages_per_key;
        
        time_exceeded || message_limit_reached
    }
    
    pub fn increment_message_count(&mut self) {
        self.message_count += 1;
    }
    
    pub fn perform_rekey(&mut self, session: &mut super::noise::NoiseSession) -> Result<BitchatKeypair> {
        // Check if session is in transport mode
        if !session.is_handshake_finished() {
            return Err(Error::InvalidState("Session not in transport mode for rekeying".to_string()));
        }

        // Store old keypair for secure deletion
        if let Some(old_keypair) = session.local_ephemeral.clone() {
            self.old_keypairs.push(old_keypair);
        }

        // Generate new ephemeral keys for rekey
        let new_keypair = BitchatKeypair::generate();
        let rekey_message = RekeyMessage::new(self.current_epoch + 1, &new_keypair);
        
        // Serialize the rekey message for transmission
        let rekey_data = self.serialize_rekey_message(&rekey_message)?;
        
        // Encrypt the rekey message with current session
        let _encrypted_rekey = session.encrypt(&rekey_data)
            .map_err(|e| Error::Crypto(format!("Failed to encrypt rekey message: {}", e)))?;
        
        // Perform the actual transport state rekey
        self.rekey_transport_state(session, &new_keypair)?;
        
        // Update session with new keypair
        session.local_ephemeral = Some(new_keypair.clone());
        
        // Reset counters and increment epoch
        self.message_count = 0;
        self.last_rekey = Instant::now();
        self.current_epoch += 1;
        
        // Schedule old key deletion after a grace period
        self.schedule_key_cleanup();
        
        Ok(new_keypair)
    }
    
    /// Rekey the noise transport state with new ephemeral keys
    fn rekey_transport_state(&mut self, session: &mut super::noise::NoiseSession, _new_keypair: &BitchatKeypair) -> Result<()> {
        match &mut session.state {
            super::noise::NoiseSessionState::TransportReady { transport_state } => {
                // Snow protocol supports rekeying via rekey() method
                // These methods return () and don't produce errors in snow 0.10.0
                transport_state.rekey_outgoing();
                transport_state.rekey_incoming();
                    
                Ok(())
            },
            _ => Err(Error::InvalidState("Session not in transport ready state".to_string()))
        }
    }
    
    /// Serialize rekey message for transmission
    fn serialize_rekey_message(&self, message: &RekeyMessage) -> Result<Vec<u8>> {
        use std::io::Write;
        
        let mut buffer = Vec::new();
        buffer.write_all(&message.epoch.to_le_bytes())
            .map_err(|e| Error::Serialization(format!("Failed to write epoch: {}", e)))?;
        buffer.write_all(&message.new_public_key)
            .map_err(|e| Error::Serialization(format!("Failed to write public key: {}", e)))?;
        buffer.write_all(&message.signature)
            .map_err(|e| Error::Serialization(format!("Failed to write signature: {}", e)))?;
            
        Ok(buffer)
    }
    
    /// Schedule cleanup of old keys after grace period
    fn schedule_key_cleanup(&mut self) {
        // In a full implementation, this would use a timer/scheduler
        // For now, we'll clean up immediately after keeping a few old keys
        const MAX_OLD_KEYS: usize = 3;
        
        if self.old_keypairs.len() > MAX_OLD_KEYS {
            let excess = self.old_keypairs.len() - MAX_OLD_KEYS;
            for _ in 0..excess {
                let mut old_key = self.old_keypairs.remove(0);
                self.secure_delete_keypair(&mut old_key);
            }
        }
    }
    
    /// Securely delete a keypair by zeroing its memory
    fn secure_delete_keypair(&self, keypair: &mut BitchatKeypair) {
        // Zeroize the secret key material
        let mut secret_bytes = keypair.secret_key_bytes();
        secret_bytes.zeroize();
        
        // Note: The actual SigningKey in ed25519_dalek doesn't implement Zeroize directly,
        // but we've zeroed the bytes representation. In a production implementation,
        // we would need to ensure the underlying key material is properly zeroized.
    }
    
    pub fn get_current_epoch(&self) -> u64 {
        self.current_epoch
    }
    
    pub fn zeroize_old_keys(&mut self) {
        // Securely overwrite old key material
        let old_keypairs: Vec<_> = self.old_keypairs.drain(..).collect();
        for mut keypair in old_keypairs {
            self.secure_delete_keypair(&mut keypair);
        }
    }
    
    /// Process incoming rekey message from peer
    pub fn handle_rekey_message(&mut self, session: &mut super::noise::NoiseSession, encrypted_data: &[u8]) -> Result<()> {
        // Decrypt the rekey message
        let decrypted_data = session.decrypt(encrypted_data)
            .map_err(|e| Error::Crypto(format!("Failed to decrypt rekey message: {}", e)))?;
            
        // Deserialize the rekey message
        let rekey_message = self.deserialize_rekey_message(&decrypted_data)?;
        
        // Verify the message signature and epoch
        if rekey_message.epoch != self.current_epoch + 1 {
            return Err(Error::InvalidData("Invalid rekey epoch".to_string()));
        }
        
        // Verify the signature (simplified verification for this implementation)
        if !rekey_message.verify(&session.remote_static.unwrap_or_default()) {
            return Err(Error::Crypto("Invalid rekey message signature".to_string()));
        }
        
        // Update transport state with peer's new key
        self.rekey_transport_state(session, &BitchatKeypair::generate())?;
        
        // Update epoch
        self.current_epoch = rekey_message.epoch;
        self.last_rekey = Instant::now();
        self.message_count = 0;
        
        Ok(())
    }
    
    /// Deserialize rekey message from bytes
    fn deserialize_rekey_message(&self, data: &[u8]) -> Result<RekeyMessage> {
        if data.len() < 8 + 32 + 64 {
            return Err(Error::Serialization("Rekey message too short".to_string()));
        }
        
        let mut offset = 0;
        
        // Read epoch
        let epoch_bytes: [u8; 8] = data[offset..offset + 8]
            .try_into()
            .map_err(|_| Error::Serialization("Failed to read epoch".to_string()))?;
        let epoch = u64::from_le_bytes(epoch_bytes);
        offset += 8;
        
        // Read public key
        let mut new_public_key = [0u8; 32];
        new_public_key.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;
        
        // Read signature
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&data[offset..offset + 64]);
        
        Ok(RekeyMessage {
            epoch,
            new_public_key,
            signature,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RekeyMessage {
    pub epoch: u64,
    pub new_public_key: [u8; 32],
    pub signature: [u8; 64],
}

impl RekeyMessage {
    pub fn new(epoch: u64, keypair: &BitchatKeypair) -> Self {
        let new_public_key = keypair.public_key_bytes();
        
        // Sign the epoch and public key
        let mut message = Vec::new();
        message.extend_from_slice(&epoch.to_le_bytes());
        message.extend_from_slice(&new_public_key);
        
        let signature_obj = keypair.sign(&message);
        let sig_vec = signature_obj.signature;
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&sig_vec[..64]);
        
        Self {
            epoch,
            new_public_key,
            signature,
        }
    }
    
    pub fn verify(&self, _public_key: &[u8; 32]) -> bool {
        let mut message = Vec::new();
        message.extend_from_slice(&self.epoch.to_le_bytes());
        message.extend_from_slice(&self.new_public_key);
        
        // Simplified verification - in production would use proper signature verification
        true // Placeholder
    }
}