//! # Voting Delegation System
//!
//! Allows users to delegate their voting power to other trusted members.

use crate::{Error, Result, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Voting delegation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRecord {
    /// Delegator (person giving power)
    pub delegator: PeerId,
    /// Delegate (person receiving power)
    pub delegate: PeerId,
    /// Percentage of power delegated (0.0 - 1.0)
    pub power_percentage: f64,
    /// When delegation was created
    pub created_at: DateTime<Utc>,
    /// When delegation expires (if applicable)
    pub expires_at: Option<DateTime<Utc>>,
}

/// Voting delegate information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingDelegate {
    /// Delegate's peer ID
    pub peer_id: PeerId,
    /// Total delegated power
    pub total_delegated_power: f64,
    /// Number of delegators
    pub delegator_count: u32,
    /// Delegation records
    pub delegations: Vec<DelegationRecord>,
}

/// Delegation management system
pub struct DelegationManager {
    /// Active delegations
    delegations: HashMap<PeerId, DelegationRecord>,
}

impl DelegationManager {
    /// Create new delegation manager
    pub async fn new() -> Result<Self> {
        Ok(Self {
            delegations: HashMap::new(),
        })
    }

    /// Delegate voting power
    pub async fn delegate_power(
        &mut self,
        delegator: PeerId,
        delegate: PeerId,
        power_percentage: f64,
    ) -> Result<()> {
        if power_percentage <= 0.0 || power_percentage > 1.0 {
            return Err(Error::ValidationError("Invalid power percentage".to_string()));
        }

        let record = DelegationRecord {
            delegator,
            delegate,
            power_percentage,
            created_at: Utc::now(),
            expires_at: None,
        };

        self.delegations.insert(delegator, record);
        Ok(())
    }

    /// Remove delegation
    pub async fn remove_delegation(&mut self, delegator: PeerId) -> Result<()> {
        self.delegations.remove(&delegator);
        Ok(())
    }
}