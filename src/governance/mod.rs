//! # Decentralized Governance Framework
//!
//! Complete DAO (Decentralized Autonomous Organization) implementation for BitCraps protocol
//! governance, including voting mechanisms, proposal systems, and treasury management.
//!
//! ## Governance Architecture
//!
//! - **Token-Based Voting**: CRAP token holders participate in governance
//! - **Proposal System**: On-chain proposals for protocol changes
//! - **Quadratic Voting**: Prevents plutocracy through quadratic cost scaling
//! - **Treasury Management**: Community-controlled protocol treasury

pub mod dao;
pub mod voting;
pub mod proposals;
pub mod treasury_governance;
pub mod delegation;
pub mod emergency;

pub use dao::{Dao, DaoConfig, DaoMember, MembershipTier, DaoStats};
pub use voting::{
    VotingMechanism, VotingPower, VotingRecord, VotingResult, 
    QuadraticVoting, DelegatedVoting, VotingThreshold
};
pub use proposals::{
    Proposal, ProposalType, ProposalStatus, ProposalManager,
    ProtocolUpgradeProposal, TreasuryProposal, ParameterChangeProposal
};
pub use treasury_governance::{TreasuryManager, TreasuryProposal as TreasuryGovProposal, TreasuryAllocation};
pub use delegation::{VotingDelegate, DelegationManager, DelegationRecord};
pub use emergency::{EmergencyGovernance, EmergencyProposal, EmergencyAction};

use crate::{Error, Result, PeerId, CrapTokens};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

/// Overall governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// DAO configuration
    pub dao_config: DaoConfig,
    /// Voting system parameters
    pub voting_config: VotingConfig,
    /// Proposal system parameters
    pub proposal_config: ProposalConfig,
    /// Treasury governance parameters
    pub treasury_config: TreasuryGovernanceConfig,
    /// Emergency governance parameters
    pub emergency_config: EmergencyConfig,
}

/// Voting system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingConfig {
    /// Minimum tokens required to vote
    pub minimum_voting_tokens: CrapTokens,
    /// Voting power calculation method
    pub voting_power_method: VotingPowerMethod,
    /// Enable quadratic voting
    pub enable_quadratic_voting: bool,
    /// Enable vote delegation
    pub enable_delegation: bool,
    /// Vote privacy settings
    pub vote_privacy: VotePrivacy,
    /// Snapshot block for token holdings
    pub snapshot_delay_blocks: u64,
}

/// Method for calculating voting power
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VotingPowerMethod {
    /// Linear: 1 token = 1 vote
    Linear,
    /// Square root: voting power = sqrt(tokens)
    SquareRoot,
    /// Quadratic: cost increases quadratically
    Quadratic,
    /// Logarithmic: diminishing returns
    Logarithmic,
}

/// Vote privacy options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VotePrivacy {
    /// All votes are public
    Public,
    /// Votes are private until proposal concludes
    Private,
    /// Anonymous voting with zero-knowledge proofs
    Anonymous,
}

/// Proposal system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalConfig {
    /// Minimum tokens required to create proposal
    pub minimum_proposal_tokens: CrapTokens,
    /// Proposal deposit amount (returned if proposal passes)
    pub proposal_deposit: CrapTokens,
    /// Voting period duration
    pub voting_period: Duration,
    /// Discussion period before voting
    pub discussion_period: Duration,
    /// Execution delay after successful vote
    pub execution_delay: Duration,
    /// Minimum quorum percentage
    pub minimum_quorum: f64,
    /// Minimum approval percentage
    pub minimum_approval: f64,
}

/// Treasury governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryGovernanceConfig {
    /// Maximum percentage of treasury that can be allocated in one proposal
    pub max_allocation_percentage: f64,
    /// Minimum time between treasury proposals
    pub proposal_cooldown: Duration,
    /// Required approval threshold for treasury proposals
    pub treasury_approval_threshold: f64,
    /// Multi-signature requirements
    pub multisig_threshold: u8,
    pub multisig_signers: Vec<PeerId>,
}

/// Emergency governance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyConfig {
    /// Enable emergency governance mechanisms
    pub enabled: bool,
    /// Emergency council members
    pub emergency_council: Vec<PeerId>,
    /// Threshold of council members required for emergency action
    pub council_threshold: u8,
    /// Maximum duration of emergency measures
    pub max_emergency_duration: Duration,
    /// Actions that can be taken in emergency
    pub allowed_emergency_actions: Vec<EmergencyActionType>,
}

/// Types of emergency actions allowed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmergencyActionType {
    /// Pause all protocol operations
    PauseProtocol,
    /// Pause specific contract or module
    PauseModule,
    /// Emergency treasury allocation
    EmergencyAllocation,
    /// Parameter adjustment
    EmergencyParameterChange,
    /// Blacklist malicious actors
    BlacklistAddress,
}

/// Governance participation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMetrics {
    /// Total active voters in last period
    pub active_voters: u32,
    /// Average voter turnout percentage
    pub average_turnout: f64,
    /// Total proposals submitted
    pub total_proposals: u32,
    /// Proposals passed vs failed
    pub proposals_passed: u32,
    /// Total governance tokens participating
    pub tokens_participating: CrapTokens,
    /// Governance participation rate
    pub participation_rate: f64,
}

/// Central governance coordinator
#[derive(Debug)]
pub struct GovernanceCoordinator {
    /// Configuration
    config: GovernanceConfig,
    /// DAO instance
    dao: Dao,
    /// Proposal manager
    proposal_manager: ProposalManager,
    /// Treasury manager
    treasury_manager: TreasuryManager,
    /// Delegation manager
    delegation_manager: DelegationManager,
    /// Emergency governance
    emergency_governance: Option<EmergencyGovernance>,
    /// Governance metrics
    metrics: GovernanceMetrics,
}

impl GovernanceCoordinator {
    /// Create new governance coordinator
    pub async fn new(config: GovernanceConfig) -> Result<Self> {
        let dao = Dao::new(config.dao_config.clone()).await?;
        let proposals_config = proposals::ProposalConfig {
            minimum_proposal_tokens: config.proposal_config.minimum_proposal_tokens,
            proposal_deposit: config.proposal_config.proposal_deposit,
            discussion_period: config.proposal_config.discussion_period,
            voting_period: config.proposal_config.voting_period,
            execution_delay: config.proposal_config.execution_delay,
        };
        let proposal_manager = ProposalManager::new(proposals_config).await?;
        let treasury_manager = TreasuryManager::new(config.treasury_config.clone()).await?;
        let delegation_manager = DelegationManager::new().await?;
        
        let emergency_governance = if config.emergency_config.enabled {
            Some(EmergencyGovernance::new(config.emergency_config.clone()).await?)
        } else {
            None
        };

        let metrics = GovernanceMetrics {
            active_voters: 0,
            average_turnout: 0.0,
            total_proposals: 0,
            proposals_passed: 0,
            tokens_participating: CrapTokens::zero(),
            participation_rate: 0.0,
        };

        Ok(Self {
            config,
            dao,
            proposal_manager,
            treasury_manager,
            delegation_manager,
            emergency_governance,
            metrics,
        })
    }

    /// Register user as DAO member
    pub async fn register_member(
        &mut self,
        peer_id: PeerId,
        token_balance: CrapTokens,
        reputation_score: u32,
    ) -> Result<()> {
        self.dao.add_member(peer_id, token_balance, reputation_score).await?;
        
        // Update metrics
        self.metrics.active_voters += 1;
        self.metrics.tokens_participating += token_balance;
        
        Ok(())
    }

    /// Submit new proposal
    pub async fn submit_proposal(
        &mut self,
        proposer: PeerId,
        proposal_type: ProposalType,
        title: String,
        description: String,
        execution_payload: Option<Vec<u8>>,
    ) -> Result<String> {
        // Verify proposer eligibility
        let member = self.dao.get_member(proposer).await?
            .ok_or_else(|| Error::ValidationError("Proposer not a DAO member".to_string()))?;

        if member.token_balance < self.config.proposal_config.minimum_proposal_tokens {
            return Err(Error::ValidationError("Insufficient tokens to propose".to_string()));
        }

        // Create and submit proposal
        let proposal_id = self.proposal_manager.create_proposal(
            proposer,
            proposal_type,
            title,
            description,
            execution_payload,
        ).await?;

        // Take proposal deposit
        self.dao.lock_tokens(proposer, self.config.proposal_config.proposal_deposit).await?;

        // Update metrics
        self.metrics.total_proposals += 1;

        Ok(proposal_id)
    }

    /// Cast vote on proposal
    pub async fn vote_on_proposal(
        &mut self,
        voter: PeerId,
        proposal_id: String,
        support: bool,
        voting_power: Option<CrapTokens>, // For quadratic voting
        reason: Option<String>,
    ) -> Result<()> {
        // Verify voter eligibility
        let member = self.dao.get_member(voter).await?
            .ok_or_else(|| Error::ValidationError("Voter not a DAO member".to_string()))?;

        if member.token_balance < self.config.voting_config.minimum_voting_tokens {
            return Err(Error::ValidationError("Insufficient tokens to vote".to_string()));
        }

        // Calculate actual voting power
        let actual_voting_power = match self.config.voting_config.voting_power_method {
            VotingPowerMethod::Linear => member.token_balance,
            VotingPowerMethod::SquareRoot => {
                CrapTokens::from_inner((member.token_balance.inner() as f64).sqrt() as u64)
            },
            VotingPowerMethod::Logarithmic => {
                CrapTokens::from_inner((member.token_balance.inner() as f64).ln().max(1.0) as u64)
            },
            VotingPowerMethod::Quadratic => {
                if let Some(power) = voting_power {
                    // Verify quadratic cost
                    let cost = self.calculate_quadratic_cost(power);
                    if member.token_balance >= cost {
                        self.dao.lock_tokens(voter, cost).await?;
                        power
                    } else {
                        return Err(Error::ValidationError("Insufficient tokens for quadratic vote".to_string()));
                    }
                } else {
                    return Err(Error::ValidationError("Quadratic voting requires power specification".to_string()));
                }
            }
        };

        // Submit vote
        self.proposal_manager.vote(
            proposal_id.clone(),
            voter,
            support,
            actual_voting_power,
            reason,
        ).await?;

        Ok(())
    }

    /// Delegate voting power to another member
    pub async fn delegate_voting_power(
        &mut self,
        delegator: PeerId,
        delegate: PeerId,
        power_percentage: f64,
    ) -> Result<()> {
        if !self.config.voting_config.enable_delegation {
            return Err(Error::ValidationError("Delegation not enabled".to_string()));
        }

        self.delegation_manager.delegate_power(delegator, delegate, power_percentage).await?;
        Ok(())
    }

    /// Execute approved proposal
    pub async fn execute_proposal(&mut self, proposal_id: String) -> Result<()> {
        let proposal = self.proposal_manager.get_proposal(&proposal_id).await?;
        
        // Verify proposal is ready for execution
        if !proposal.is_ready_for_execution() {
            return Err(Error::ValidationError("Proposal not ready for execution".to_string()));
        }

        // Execute based on proposal type
        match &proposal.proposal_type {
            ProposalType::ProtocolUpgrade { .. } => {
                self.execute_protocol_upgrade(&proposal).await?;
            },
            ProposalType::TreasuryAllocation { .. } => {
                self.execute_treasury_allocation(&proposal).await?;
            },
            ProposalType::ParameterChange { .. } => {
                self.execute_parameter_change(&proposal).await?;
            },
            ProposalType::MembershipChange { .. } => {
                self.execute_membership_change(&proposal).await?;
            },
        }

        // Mark proposal as executed
        self.proposal_manager.mark_executed(proposal_id).await?;

        // Update metrics
        self.metrics.proposals_passed += 1;

        Ok(())
    }

    /// Get current governance metrics
    pub fn get_governance_metrics(&self) -> GovernanceMetrics {
        self.metrics.clone()
    }

    /// Get DAO statistics
    pub async fn get_dao_stats(&self) -> Result<DaoStats> {
        self.dao.get_stats().await
    }

    /// Handle emergency governance action
    pub async fn handle_emergency_action(
        &mut self,
        council_member: PeerId,
        action_type: EmergencyActionType,
        target: Option<String>,
        duration: Option<Duration>,
    ) -> Result<String> {
        if let Some(ref mut emergency_gov) = self.emergency_governance {
            emergency_gov.propose_emergency_action(
                council_member,
                action_type,
                target,
                duration,
            ).await
        } else {
            Err(Error::ValidationError("Emergency governance not enabled".to_string()))
        }
    }

    /// Calculate quadratic voting cost
    fn calculate_quadratic_cost(&self, voting_power: CrapTokens) -> CrapTokens {
        // Cost = power^2
        let power = voting_power.inner() as f64;
        CrapTokens::from_inner((power * power) as u64)
    }

    /// Execute protocol upgrade proposal
    async fn execute_protocol_upgrade(&mut self, proposal: &Proposal) -> Result<()> {
        // In production, would trigger protocol upgrade mechanisms
        println!("Executing protocol upgrade: {}", proposal.title);
        Ok(())
    }

    /// Execute treasury allocation proposal
    async fn execute_treasury_allocation(&mut self, proposal: &Proposal) -> Result<()> {
        if let ProposalType::TreasuryAllocation { recipient, amount } = &proposal.proposal_type {
            self.treasury_manager.allocate_funds(*recipient, *amount).await?;
        }
        Ok(())
    }

    /// Execute parameter change proposal
    async fn execute_parameter_change(&mut self, proposal: &Proposal) -> Result<()> {
        // In production, would update protocol parameters
        println!("Executing parameter change: {}", proposal.title);
        Ok(())
    }

    /// Execute membership change proposal
    async fn execute_membership_change(&mut self, proposal: &Proposal) -> Result<()> {
        // In production, would update DAO membership
        println!("Executing membership change: {}", proposal.title);
        Ok(())
    }
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            dao_config: DaoConfig::default(),
            voting_config: VotingConfig {
                minimum_voting_tokens: CrapTokens::from_inner(100),
                voting_power_method: VotingPowerMethod::SquareRoot,
                enable_quadratic_voting: true,
                enable_delegation: true,
                vote_privacy: VotePrivacy::Private,
                snapshot_delay_blocks: 100,
            },
            proposal_config: ProposalConfig {
                minimum_proposal_tokens: CrapTokens::from_inner(1000),
                proposal_deposit: CrapTokens::from_inner(500),
                voting_period: Duration::days(7),
                discussion_period: Duration::days(3),
                execution_delay: Duration::days(2),
                minimum_quorum: 0.1, // 10%
                minimum_approval: 0.51, // 51%
            },
            treasury_config: TreasuryGovernanceConfig {
                max_allocation_percentage: 0.1, // 10%
                proposal_cooldown: Duration::days(30),
                treasury_approval_threshold: 0.6, // 60%
                multisig_threshold: 3,
                multisig_signers: Vec::new(),
            },
            emergency_config: EmergencyConfig {
                enabled: true,
                emergency_council: Vec::new(),
                council_threshold: 3,
                max_emergency_duration: Duration::days(7),
                allowed_emergency_actions: vec![
                    EmergencyActionType::PauseProtocol,
                    EmergencyActionType::PauseModule,
                    EmergencyActionType::BlacklistAddress,
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_governance_config_default() {
        let config = GovernanceConfig::default();
        assert!(config.voting_config.enable_quadratic_voting);
        assert!(config.voting_config.enable_delegation);
        assert_eq!(config.voting_config.voting_power_method, VotingPowerMethod::SquareRoot);
    }

    #[test]
    fn test_voting_power_methods() {
        let tokens = CrapTokens::from_inner(100);
        
        // Linear would be 100
        // Square root would be 10
        // Log would be ln(100) ≈ 4.6
        
        assert!(VotingPowerMethod::Linear != VotingPowerMethod::SquareRoot);
    }

    #[test]
    fn test_governance_metrics_initialization() {
        let metrics = GovernanceMetrics {
            active_voters: 0,
            average_turnout: 0.0,
            total_proposals: 0,
            proposals_passed: 0,
            tokens_participating: CrapTokens::zero(),
            participation_rate: 0.0,
        };
        
        assert_eq!(metrics.active_voters, 0);
        assert_eq!(metrics.tokens_participating, CrapTokens::zero());
    }
}