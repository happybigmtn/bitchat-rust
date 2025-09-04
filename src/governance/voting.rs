//! # Voting Mechanisms for DAO Governance
//!
//! Implements various voting systems including quadratic voting, delegated voting,
//! and privacy-preserving ballot systems.

use crate::{Error, Result, PeerId, CrapTokens};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Voting power calculation and distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingPower {
    /// Base voting power from token holdings
    pub base_power: CrapTokens,
    /// Adjusted power after applying voting mechanism
    pub effective_power: CrapTokens,
    /// Multipliers applied (from reputation, delegation, etc.)
    pub multipliers: Vec<VotingMultiplier>,
    /// Total cost for this voting power (relevant for quadratic voting)
    pub cost: CrapTokens,
}

/// Voting power multiplier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingMultiplier {
    /// Source of the multiplier
    pub source: String,
    /// Multiplier value
    pub value: f64,
    /// Reason/description
    pub reason: String,
}

/// Individual vote record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingRecord {
    /// Voter's peer ID
    pub voter: PeerId,
    /// Proposal being voted on
    pub proposal_id: String,
    /// Vote choice (true = support, false = oppose)
    pub support: bool,
    /// Voting power used
    pub voting_power: VotingPower,
    /// Vote timestamp
    pub timestamp: DateTime<Utc>,
    /// Optional reason/comment
    pub reason: Option<String>,
    /// Whether vote was delegated
    pub delegated: bool,
    /// Original voter if this is a delegated vote
    pub delegator: Option<PeerId>,
}

/// Aggregated voting results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingResult {
    /// Proposal ID
    pub proposal_id: String,
    /// Total support votes
    pub support_votes: CrapTokens,
    /// Total oppose votes
    pub oppose_votes: CrapTokens,
    /// Total voting power participated
    pub total_participation: CrapTokens,
    /// Number of unique voters
    pub unique_voters: u32,
    /// Quorum reached
    pub quorum_reached: bool,
    /// Required threshold met
    pub threshold_met: bool,
    /// Final result
    pub passed: bool,
    /// Vote closed timestamp
    pub closed_at: DateTime<Utc>,
}

/// Voting thresholds for different types of proposals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingThreshold {
    /// Minimum quorum percentage (0.0 - 1.0)
    pub quorum: f64,
    /// Required approval percentage (0.0 - 1.0)
    pub approval: f64,
    /// Whether supermajority is required
    pub supermajority: bool,
    /// Minimum participation time
    pub minimum_duration: chrono::Duration,
}

/// Voting mechanism trait
pub trait VotingMechanism {
    /// Calculate voting power for a voter
    fn calculate_voting_power(
        &self,
        voter: PeerId,
        token_balance: CrapTokens,
        desired_power: Option<CrapTokens>,
    ) -> Result<VotingPower>;

    /// Cast a vote
    fn cast_vote(
        &mut self,
        voter: PeerId,
        proposal_id: String,
        support: bool,
        voting_power: VotingPower,
        reason: Option<String>,
    ) -> Result<VotingRecord>;

    /// Tally votes for a proposal
    fn tally_votes(&self, proposal_id: &str) -> Result<VotingResult>;

    /// Check if voting thresholds are met
    fn check_thresholds(&self, result: &VotingResult, threshold: &VotingThreshold) -> bool;
}

/// Linear voting implementation (1 token = 1 vote)
pub struct LinearVoting {
    votes: HashMap<String, Vec<VotingRecord>>,
}

impl LinearVoting {
    pub fn new() -> Self {
        Self {
            votes: HashMap::new(),
        }
    }
}

impl VotingMechanism for LinearVoting {
    fn calculate_voting_power(
        &self,
        _voter: PeerId,
        token_balance: CrapTokens,
        _desired_power: Option<CrapTokens>,
    ) -> Result<VotingPower> {
        Ok(VotingPower {
            base_power: token_balance,
            effective_power: token_balance,
            multipliers: vec![],
            cost: CrapTokens::zero(),
        })
    }

    fn cast_vote(
        &mut self,
        voter: PeerId,
        proposal_id: String,
        support: bool,
        voting_power: VotingPower,
        reason: Option<String>,
    ) -> Result<VotingRecord> {
        let record = VotingRecord {
            voter,
            proposal_id: proposal_id.clone(),
            support,
            voting_power,
            timestamp: Utc::now(),
            reason,
            delegated: false,
            delegator: None,
        };

        self.votes.entry(proposal_id).or_insert_with(Vec::new).push(record.clone());
        Ok(record)
    }

    fn tally_votes(&self, proposal_id: &str) -> Result<VotingResult> {
        let empty_votes = vec![];
        let votes = self.votes.get(proposal_id).unwrap_or(&empty_votes);
        
        let mut support_votes = CrapTokens::zero();
        let mut oppose_votes = CrapTokens::zero();
        let mut total_participation = CrapTokens::zero();
        let mut unique_voters = std::collections::HashSet::new();

        for vote in votes {
            unique_voters.insert(vote.voter);
            total_participation += vote.voting_power.effective_power;
            
            if vote.support {
                support_votes += vote.voting_power.effective_power;
            } else {
                oppose_votes += vote.voting_power.effective_power;
            }
        }

        Ok(VotingResult {
            proposal_id: proposal_id.to_string(),
            support_votes,
            oppose_votes,
            total_participation,
            unique_voters: unique_voters.len() as u32,
            quorum_reached: false, // Will be determined by caller with threshold
            threshold_met: false,
            passed: false,
            closed_at: Utc::now(),
        })
    }

    fn check_thresholds(&self, result: &VotingResult, threshold: &VotingThreshold) -> bool {
        // Check quorum (simplified - would need total eligible voters)
        let quorum_met = true; // Placeholder
        
        // Check approval threshold
        let total_votes = result.support_votes + result.oppose_votes;
        let approval_rate = if total_votes.inner() > 0 {
            result.support_votes.inner() as f64 / total_votes.inner() as f64
        } else {
            0.0
        };

        let approval_met = if threshold.supermajority {
            approval_rate >= 0.67 // 2/3 majority
        } else {
            approval_rate >= threshold.approval
        };

        quorum_met && approval_met
    }
}

/// Quadratic voting implementation (cost increases quadratically)
pub struct QuadraticVoting {
    votes: HashMap<String, Vec<VotingRecord>>,
    /// Mapping of voter to total credits spent per proposal
    credits_spent: HashMap<(PeerId, String), CrapTokens>,
}

impl QuadraticVoting {
    pub fn new() -> Self {
        Self {
            votes: HashMap::new(),
            credits_spent: HashMap::new(),
        }
    }

    /// Calculate quadratic cost for desired voting power
    fn calculate_quadratic_cost(&self, desired_power: CrapTokens) -> CrapTokens {
        let power = desired_power.inner() as f64;
        CrapTokens::from_inner((power * power) as u64)
    }
}

impl VotingMechanism for QuadraticVoting {
    fn calculate_voting_power(
        &self,
        _voter: PeerId,
        token_balance: CrapTokens,
        desired_power: Option<CrapTokens>,
    ) -> Result<VotingPower> {
        let desired = desired_power.unwrap_or(CrapTokens::from_inner((token_balance.inner() as f64).sqrt() as u64));
        let cost = self.calculate_quadratic_cost(desired);

        if cost > token_balance {
            return Err(Error::ValidationError("Insufficient tokens for desired voting power".to_string()));
        }

        Ok(VotingPower {
            base_power: token_balance,
            effective_power: desired,
            multipliers: vec![],
            cost,
        })
    }

    fn cast_vote(
        &mut self,
        voter: PeerId,
        proposal_id: String,
        support: bool,
        voting_power: VotingPower,
        reason: Option<String>,
    ) -> Result<VotingRecord> {
        // Track credits spent
        let key = (voter, proposal_id.clone());
        let current_spent = self.credits_spent.get(&key).copied().unwrap_or(CrapTokens::zero());
        let new_total = current_spent + voting_power.cost;
        self.credits_spent.insert(key, new_total);

        let record = VotingRecord {
            voter,
            proposal_id: proposal_id.clone(),
            support,
            voting_power,
            timestamp: Utc::now(),
            reason,
            delegated: false,
            delegator: None,
        };

        self.votes.entry(proposal_id).or_insert_with(Vec::new).push(record.clone());
        Ok(record)
    }

    fn tally_votes(&self, proposal_id: &str) -> Result<VotingResult> {
        let empty_votes = vec![];
        let votes = self.votes.get(proposal_id).unwrap_or(&empty_votes);
        
        let mut support_votes = CrapTokens::zero();
        let mut oppose_votes = CrapTokens::zero();
        let mut total_participation = CrapTokens::zero();
        let mut unique_voters = std::collections::HashSet::new();

        for vote in votes {
            unique_voters.insert(vote.voter);
            total_participation += vote.voting_power.effective_power;
            
            if vote.support {
                support_votes += vote.voting_power.effective_power;
            } else {
                oppose_votes += vote.voting_power.effective_power;
            }
        }

        Ok(VotingResult {
            proposal_id: proposal_id.to_string(),
            support_votes,
            oppose_votes,
            total_participation,
            unique_voters: unique_voters.len() as u32,
            quorum_reached: false,
            threshold_met: false,
            passed: false,
            closed_at: Utc::now(),
        })
    }

    fn check_thresholds(&self, result: &VotingResult, threshold: &VotingThreshold) -> bool {
        let total_votes = result.support_votes + result.oppose_votes;
        if total_votes.inner() == 0 {
            return false;
        }

        let approval_rate = result.support_votes.inner() as f64 / total_votes.inner() as f64;
        approval_rate >= threshold.approval
    }
}

/// Delegated voting system
pub struct DelegatedVoting {
    base_mechanism: Box<dyn VotingMechanism>,
    delegations: HashMap<PeerId, PeerId>, // delegator -> delegate
}

impl DelegatedVoting {
    pub fn new(base_mechanism: Box<dyn VotingMechanism>) -> Self {
        Self {
            base_mechanism,
            delegations: HashMap::new(),
        }
    }

    /// Delegate voting power to another voter
    pub fn delegate_power(&mut self, delegator: PeerId, delegate: PeerId) -> Result<()> {
        self.delegations.insert(delegator, delegate);
        Ok(())
    }

    /// Remove delegation
    pub fn remove_delegation(&mut self, delegator: PeerId) -> Result<()> {
        self.delegations.remove(&delegator);
        Ok(())
    }
}

impl VotingMechanism for DelegatedVoting {
    fn calculate_voting_power(
        &self,
        voter: PeerId,
        token_balance: CrapTokens,
        desired_power: Option<CrapTokens>,
    ) -> Result<VotingPower> {
        // If this voter has delegated their power, they can't vote directly
        if self.delegations.contains_key(&voter) {
            return Err(Error::ValidationError("Voting power has been delegated".to_string()));
        }

        // Calculate base power plus any delegated power
        let mut total_balance = token_balance;
        
        // Add delegated tokens
        for (delegator, delegate) in &self.delegations {
            if *delegate == voter {
                // In production, would look up delegator's token balance
                // For now, using placeholder
                total_balance += CrapTokens::from_inner(1000);
            }
        }

        self.base_mechanism.calculate_voting_power(voter, total_balance, desired_power)
    }

    fn cast_vote(
        &mut self,
        voter: PeerId,
        proposal_id: String,
        support: bool,
        voting_power: VotingPower,
        reason: Option<String>,
    ) -> Result<VotingRecord> {
        self.base_mechanism.cast_vote(voter, proposal_id, support, voting_power, reason)
    }

    fn tally_votes(&self, proposal_id: &str) -> Result<VotingResult> {
        self.base_mechanism.tally_votes(proposal_id)
    }

    fn check_thresholds(&self, result: &VotingResult, threshold: &VotingThreshold) -> bool {
        self.base_mechanism.check_thresholds(result, threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_voting_power_calculation() {
        let voting = LinearVoting::new();
        let tokens = CrapTokens::from_inner(1000);
        
        let power = voting.calculate_voting_power([1u8; 32], tokens, None).unwrap();
        assert_eq!(power.effective_power, tokens);
        assert_eq!(power.cost, CrapTokens::zero());
    }

    #[test]
    fn test_quadratic_voting_cost() {
        let voting = QuadraticVoting::new();
        let desired_power = CrapTokens::from_inner(10);
        let cost = voting.calculate_quadratic_cost(desired_power);
        
        assert_eq!(cost, CrapTokens::from_inner(100)); // 10^2 = 100
    }

    #[test]
    fn test_voting_threshold_check() {
        let voting = LinearVoting::new();
        
        let result = VotingResult {
            proposal_id: "test".to_string(),
            support_votes: CrapTokens::from_inner(600),
            oppose_votes: CrapTokens::from_inner(400),
            total_participation: CrapTokens::from_inner(1000),
            unique_voters: 10,
            quorum_reached: true,
            threshold_met: true,
            passed: false,
            closed_at: Utc::now(),
        };

        let threshold = VotingThreshold {
            quorum: 0.1,
            approval: 0.5,
            supermajority: false,
            minimum_duration: chrono::Duration::hours(24),
        };

        assert!(voting.check_thresholds(&result, &threshold));
    }
}