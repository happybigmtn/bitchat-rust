use crate::crypto::BitchatKeypair;
use snow::{Builder, HandshakeState, TransportState};

#[derive(Debug, Clone)]
pub enum NoiseRole {
    Initiator,
    Responder,
}

#[derive(Debug)]
pub enum NoiseSessionState {
    Uninitialized,
    HandshakeInProgress {
        handshake_state: Box<HandshakeState>,
    },
    TransportReady {
        transport_state: Box<TransportState>,
    },
    Terminated,
}

pub struct NoiseSession {
    pub role: NoiseRole,
    pub state: NoiseSessionState,
    pub local_ephemeral: Option<BitchatKeypair>,
    pub remote_static: Option<[u8; 32]>,
    pub handshake_hash: Option<[u8; 32]>,
}

impl NoiseSession {
    pub fn new_initiator(local_keypair: &BitchatKeypair) -> Result<Self, snow::Error> {
        let params = "Noise_XX_25519_ChaChaPoly_SHA256".parse()?;
        let builder = Builder::new(params);
        let handshake = builder
            .local_private_key(&local_keypair.secret_key_bytes())?
            .build_initiator()?;

        Ok(Self {
            role: NoiseRole::Initiator,
            state: NoiseSessionState::HandshakeInProgress {
                handshake_state: Box::new(handshake),
            },
            local_ephemeral: Some(local_keypair.clone()),
            remote_static: None,
            handshake_hash: None,
        })
    }

    pub fn new_responder(local_keypair: &BitchatKeypair) -> Result<Self, snow::Error> {
        let params = "Noise_XX_25519_ChaChaPoly_SHA256".parse()?;
        let builder = Builder::new(params);
        let handshake = builder
            .local_private_key(&local_keypair.secret_key_bytes())?
            .build_responder()?;

        Ok(Self {
            role: NoiseRole::Responder,
            state: NoiseSessionState::HandshakeInProgress {
                handshake_state: Box::new(handshake),
            },
            local_ephemeral: Some(local_keypair.clone()),
            remote_static: None,
            handshake_hash: None,
        })
    }

    pub fn write_handshake_message(&mut self, payload: &[u8]) -> Result<Vec<u8>, snow::Error> {
        match &mut self.state {
            NoiseSessionState::HandshakeInProgress { handshake_state } => {
                let mut buffer = vec![0u8; 65535];
                let len = handshake_state.write_message(payload, &mut buffer)?;
                buffer.truncate(len);
                Ok(buffer)
            }
            _ => Err(snow::Error::Input),
        }
    }

    pub fn read_handshake_message(&mut self, message: &[u8]) -> Result<Vec<u8>, snow::Error> {
        match &mut self.state {
            NoiseSessionState::HandshakeInProgress { handshake_state } => {
                let mut buffer = vec![0u8; 65535];
                let len = handshake_state.read_message(message, &mut buffer)?;
                buffer.truncate(len);

                if handshake_state.is_handshake_finished() {
                    let hash_slice = handshake_state.get_handshake_hash();
                    let mut hash_array = [0u8; 32];
                    hash_array.copy_from_slice(&hash_slice[..32]);
                    self.handshake_hash = Some(hash_array);

                    // Take ownership by replacing with a placeholder state temporarily
                    let old_state =
                        std::mem::replace(&mut self.state, NoiseSessionState::Terminated);
                    if let NoiseSessionState::HandshakeInProgress { handshake_state } = old_state {
                        let transport = handshake_state.into_transport_mode()?;
                        self.state = NoiseSessionState::TransportReady {
                            transport_state: Box::new(transport),
                        };
                    }
                }

                Ok(buffer)
            }
            _ => Err(snow::Error::Input),
        }
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, snow::Error> {
        match &mut self.state {
            NoiseSessionState::TransportReady { transport_state } => {
                let mut buffer = vec![0u8; plaintext.len() + 16];
                let len = transport_state.write_message(plaintext, &mut buffer)?;
                buffer.truncate(len);
                Ok(buffer)
            }
            _ => Err(snow::Error::Input),
        }
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, snow::Error> {
        match &mut self.state {
            NoiseSessionState::TransportReady { transport_state } => {
                let mut buffer = vec![0u8; ciphertext.len()];
                let len = transport_state.read_message(ciphertext, &mut buffer)?;
                buffer.truncate(len);
                Ok(buffer)
            }
            _ => Err(snow::Error::Input),
        }
    }

    pub fn is_handshake_finished(&self) -> bool {
        matches!(self.state, NoiseSessionState::TransportReady { .. })
    }

    pub fn terminate(&mut self) {
        self.state = NoiseSessionState::Terminated;
    }
}
