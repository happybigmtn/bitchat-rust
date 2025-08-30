use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Initializing,
    Handshaking,
    Active,
    Rekeying,
    Suspended,
    Terminated,
}

impl SessionState {
    pub fn can_send_data(&self) -> bool {
        matches!(self, SessionState::Active)
    }

    pub fn can_receive_data(&self) -> bool {
        matches!(self, SessionState::Active | SessionState::Rekeying)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionState::Terminated)
    }

    pub fn requires_handshake(&self) -> bool {
        matches!(self, SessionState::Initializing | SessionState::Handshaking)
    }
}
