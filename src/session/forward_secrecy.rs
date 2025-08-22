use std::time::{Duration, Instant};
use crate::crypto::BitchatKeypair;
use crate::error::Result;

pub struct ForwardSecrecyManager {
    rekey_interval: Duration,
    max_messages_per_key: u64,
    last_rekey: Instant,
    message_count: u64,
    current_epoch: u64,
}

impl ForwardSecrecyManager {
    pub fn new(rekey_interval: Duration, max_messages_per_key: u64) -> Self {
        Self {
            rekey_interval,
            max_messages_per_key,
            last_rekey: Instant::now(),
            message_count: 0,
            current_epoch: 0,
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
    
    pub fn perform_rekey(&mut self, _session: &mut super::noise::NoiseSession) -> Result<BitchatKeypair> {
        // Generate new ephemeral keys
        let new_keypair = BitchatKeypair::generate();
        
        // Reset counters
        self.message_count = 0;
        self.last_rekey = Instant::now();
        self.current_epoch += 1;
        
        // In a real implementation, this would:
        // 1. Negotiate new keys with peer
        // 2. Update the Noise transport state
        // 3. Securely delete old keys
        
        Ok(new_keypair)
    }
    
    pub fn get_current_epoch(&self) -> u64 {
        self.current_epoch
    }
    
    pub fn zeroize_old_keys(&mut self) {
        // Securely overwrite old key material
        // This would use zeroize crate in production
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