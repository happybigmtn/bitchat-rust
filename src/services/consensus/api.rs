//! Consensus Service API
//!
//! API endpoints for the consensus service.

use super::service::ConsensusService;
use super::types::*;
use crate::error::Result;
use crate::protocol::PeerId;

/// Consensus API implementation
pub struct ConsensusApi {
    // Implementation will be added as needed
}

impl ConsensusApi {
    pub fn new() -> Self {
        Self {}
    }

    /// Add an active validator (admin)
    pub async fn add_validator(&self, service: &ConsensusService, peer: PeerId, stake: Option<u64>) -> Result<UpdateValidatorResponse> {
        service
            .update_validator(UpdateValidatorRequest {
                peer_id: peer,
                action: ValidatorUpdateAction::Add,
                stake,
            })
            .await
    }

    /// Remove a validator (admin)
    pub async fn remove_validator(&self, service: &ConsensusService, peer: PeerId) -> Result<UpdateValidatorResponse> {
        service
            .update_validator(UpdateValidatorRequest {
                peer_id: peer,
                action: ValidatorUpdateAction::Remove,
                stake: None,
            })
            .await
    }
}
