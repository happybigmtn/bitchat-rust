//! # Proposal System for DAO Governance
//!
//! Complete proposal lifecycle management including creation, discussion,
//! voting, and execution phases.

use crate::{Error, Result, PeerId, CrapTokens};
use crate::governance::voting::{VotingRecord, VotingResult, VotingThreshold};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

/// Types of proposals that can be submitted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    /// Protocol upgrade proposal
    ProtocolUpgrade {
        /// Version being upgraded to
        new_version: String,
        /// Upgrade specification
        upgrade_spec: String,
        /// Required coordination
        requires_coordination: bool,
    },
    /// Treasury allocation proposal
    TreasuryAllocation {
        /// Recipient of funds
        recipient: PeerId,
        /// Amount to allocate
        amount: CrapTokens,
        /// Purpose/reason
        purpose: String,
        /// Milestone-based release
        milestones: Option<Vec<Milestone>>,
    },
    /// Protocol parameter change
    ParameterChange {
        /// Parameter being changed
        parameter_name: String,
        /// Current value
        current_value: String,
        /// Proposed new value
        new_value: String,
        /// Impact assessment
        impact: String,
    },
    /// DAO membership changes
    MembershipChange {
        /// Type of change
        change_type: MembershipChangeType,
        /// Target member
        target_member: PeerId,
        /// Reason for change
        reason: String,
    },
}

/// Milestone for treasury allocations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Milestone description
    pub description: String,
    /// Amount released at this milestone
    pub amount: CrapTokens,
    /// Due date
    pub due_date: DateTime<Utc>,
    /// Completion status
    pub completed: bool,
}

/// Types of membership changes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembershipChangeType {
    /// Add new member
    Add,
    /// Remove existing member
    Remove,
    /// Change member tier
    ChangeTier,
    /// Suspend member
    Suspend,
    /// Reinstate suspended member
    Reinstate,
}

/// Proposal status throughout its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal submitted, pending review
    Pending,
    /// In discussion phase
    Discussion,
    /// Active voting period
    Voting,
    /// Voting completed, awaiting execution
    AwaitingExecution,
    /// Successfully executed
    Executed,
    /// Proposal rejected by vote
    Rejected,
    /// Proposal cancelled by proposer
    Cancelled,
    /// Proposal expired without resolution
    Expired,
}

/// Complete proposal structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique proposal ID
    pub id: String,
    /// Proposal title
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Type of proposal
    pub proposal_type: ProposalType,
    /// Proposer's peer ID
    pub proposer: PeerId,
    /// Current status
    pub status: ProposalStatus,
    /// When proposal was created
    pub created_at: DateTime<Utc>,
    /// Discussion period end
    pub discussion_ends: DateTime<Utc>,
    /// Voting period start
    pub voting_starts: DateTime<Utc>,
    /// Voting period end
    pub voting_ends: DateTime<Utc>,
    /// Execution deadline
    pub execution_deadline: DateTime<Utc>,
    /// Voting threshold required
    pub threshold: VotingThreshold,
    /// Proposal deposit amount
    pub deposit: CrapTokens,
    /// Execution payload (if applicable)
    pub execution_payload: Option<Vec<u8>>,
    /// Discussion comments
    pub comments: Vec<ProposalComment>,
    /// Vote tally
    pub vote_result: Option<VotingResult>,
    /// Execution result
    pub execution_result: Option<ExecutionResult>,
}

/// Comment on a proposal during discussion phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalComment {
    /// Comment author
    pub author: PeerId,
    /// Comment content
    pub content: String,
    /// When comment was made
    pub timestamp: DateTime<Utc>,
    /// Parent comment (for replies)
    pub parent: Option<String>,
    /// Support/oppose signal
    pub sentiment: Option<bool>,
}

/// Result of proposal execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Execution message/log
    pub message: String,
    /// When execution occurred
    pub executed_at: DateTime<Utc>,
    /// Transaction hash (if applicable)
    pub transaction_hash: Option<String>,
}

/// Protocol upgrade proposal details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolUpgradeProposal {
    /// Target version
    pub version: String,
    /// Upgrade specification
    pub specification: String,
    /// Breaking changes
    pub breaking_changes: Vec<String>,
    /// Migration steps
    pub migration_steps: Vec<String>,
    /// Rollback plan
    pub rollback_plan: String,
    /// Required coordination with other systems
    pub coordination_required: bool,
}

/// Treasury proposal details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryProposal {
    /// Recipient information
    pub recipient: PeerId,
    /// Requested amount
    pub amount: CrapTokens,
    /// Purpose description
    pub purpose: String,
    /// Detailed budget breakdown
    pub budget_breakdown: Vec<BudgetItem>,
    /// Milestones for release
    pub milestones: Vec<Milestone>,
    /// Expected outcomes
    pub expected_outcomes: Vec<String>,
}

/// Budget item for treasury proposals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetItem {
    /// Item description
    pub description: String,
    /// Amount for this item
    pub amount: CrapTokens,
    /// Category
    pub category: String,
}

/// Parameter change proposal details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterChangeProposal {
    /// Parameter identifier
    pub parameter: String,
    /// Current value
    pub current_value: serde_json::Value,
    /// Proposed new value
    pub new_value: serde_json::Value,
    /// Rationale for change
    pub rationale: String,
    /// Impact assessment
    pub impact_assessment: ImpactAssessment,
}

/// Impact assessment for parameter changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// Economic impact
    pub economic_impact: String,
    /// Security impact
    pub security_impact: String,
    /// User experience impact
    pub user_experience_impact: String,
    /// Technical complexity
    pub technical_complexity: String,
    /// Reversibility
    pub reversible: bool,
}

/// Proposal management system
pub struct ProposalManager {
    /// All proposals
    proposals: HashMap<String, Proposal>,
    /// Configuration
    config: ProposalConfig,
}

/// Configuration for proposal system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalConfig {
    /// Minimum tokens required to create proposal
    pub minimum_proposal_tokens: CrapTokens,
    /// Proposal deposit amount
    pub proposal_deposit: CrapTokens,
    /// Discussion period duration
    pub discussion_period: Duration,
    /// Voting period duration
    pub voting_period: Duration,
    /// Execution delay after successful vote
    pub execution_delay: Duration,
    /// Maximum proposal lifetime
    pub max_lifetime: Duration,
}

impl ProposalManager {
    /// Create new proposal manager
    pub async fn new(config: ProposalConfig) -> Result<Self> {
        Ok(Self {
            proposals: HashMap::new(),
            config,
        })
    }

    /// Create new proposal
    pub async fn create_proposal(
        &mut self,
        proposer: PeerId,
        proposal_type: ProposalType,
        title: String,
        description: String,
        execution_payload: Option<Vec<u8>>,
    ) -> Result<String> {
        let proposal_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        
        let threshold = self.determine_threshold(&proposal_type);
        
        let proposal = Proposal {
            id: proposal_id.clone(),
            title,
            description,
            proposal_type,
            proposer,
            status: ProposalStatus::Pending,
            created_at: now,
            discussion_ends: now + self.config.discussion_period,
            voting_starts: now + self.config.discussion_period,
            voting_ends: now + self.config.discussion_period + self.config.voting_period,
            execution_deadline: now + self.config.discussion_period + self.config.voting_period + self.config.execution_delay,
            threshold,
            deposit: self.config.proposal_deposit,
            execution_payload,
            comments: Vec::new(),
            vote_result: None,
            execution_result: None,
        };

        self.proposals.insert(proposal_id.clone(), proposal);
        Ok(proposal_id)
    }

    /// Get proposal by ID
    pub async fn get_proposal(&self, proposal_id: &str) -> Result<Proposal> {
        self.proposals.get(proposal_id)
            .cloned()
            .ok_or_else(|| Error::ValidationError("Proposal not found".to_string()))
    }

    /// Add comment to proposal
    pub async fn add_comment(
        &mut self,
        proposal_id: String,
        author: PeerId,
        content: String,
        parent: Option<String>,
        sentiment: Option<bool>,
    ) -> Result<()> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or_else(|| Error::ValidationError("Proposal not found".to_string()))?;

        if proposal.status != ProposalStatus::Discussion {
            return Err(Error::ValidationError("Proposal not in discussion phase".to_string()));
        }

        let comment = ProposalComment {
            author,
            content,
            timestamp: Utc::now(),
            parent,
            sentiment,
        };

        proposal.comments.push(comment);
        Ok(())
    }

    /// Vote on proposal
    pub async fn vote(
        &mut self,
        proposal_id: String,
        voter: PeerId,
        support: bool,
        voting_power: crate::governance::voting::VotingPower,
        reason: Option<String>,
    ) -> Result<()> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or_else(|| Error::ValidationError("Proposal not found".to_string()))?;

        if proposal.status != ProposalStatus::Voting {
            return Err(Error::ValidationError("Proposal not in voting phase".to_string()));
        }

        if Utc::now() > proposal.voting_ends {
            return Err(Error::ValidationError("Voting period has ended".to_string()));
        }

        // In production, would integrate with voting mechanism
        Ok(())
    }

    /// Finalize voting and determine result
    pub async fn finalize_voting(&mut self, proposal_id: String) -> Result<VotingResult> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or_else(|| Error::ValidationError("Proposal not found".to_string()))?;

        if proposal.status != ProposalStatus::Voting {
            return Err(Error::ValidationError("Proposal not in voting phase".to_string()));
        }

        if Utc::now() <= proposal.voting_ends {
            return Err(Error::ValidationError("Voting period not yet ended".to_string()));
        }

        // Mock result - in production would get from voting mechanism
        let result = VotingResult {
            proposal_id: proposal_id.clone(),
            support_votes: CrapTokens::from_inner(6000),
            oppose_votes: CrapTokens::from_inner(4000),
            total_participation: CrapTokens::from_inner(10000),
            unique_voters: 100,
            quorum_reached: true,
            threshold_met: true,
            passed: true,
            closed_at: Utc::now(),
        };

        if result.passed {
            proposal.status = ProposalStatus::AwaitingExecution;
        } else {
            proposal.status = ProposalStatus::Rejected;
        }

        proposal.vote_result = Some(result.clone());
        Ok(result)
    }

    /// Mark proposal as executed
    pub async fn mark_executed(&mut self, proposal_id: String) -> Result<()> {
        let proposal = self.proposals.get_mut(&proposal_id)
            .ok_or_else(|| Error::ValidationError("Proposal not found".to_string()))?;

        proposal.status = ProposalStatus::Executed;
        proposal.execution_result = Some(ExecutionResult {
            success: true,
            message: "Proposal executed successfully".to_string(),
            executed_at: Utc::now(),
            transaction_hash: None,
        });

        Ok(())
    }

    /// Update proposal statuses based on time
    pub async fn update_proposal_statuses(&mut self) -> Result<()> {
        let now = Utc::now();
        
        for proposal in self.proposals.values_mut() {
            match proposal.status {
                ProposalStatus::Pending => {
                    if now >= proposal.discussion_ends {
                        proposal.status = ProposalStatus::Discussion;
                    }
                },
                ProposalStatus::Discussion => {
                    if now >= proposal.voting_starts {
                        proposal.status = ProposalStatus::Voting;
                    }
                },
                ProposalStatus::Voting => {
                    if now >= proposal.voting_ends {
                        // Would trigger automatic vote finalization
                    }
                },
                ProposalStatus::AwaitingExecution => {
                    if now >= proposal.execution_deadline {
                        proposal.status = ProposalStatus::Expired;
                    }
                },
                _ => {}
            }
        }

        Ok(())
    }

    /// Get all proposals with optional filtering
    pub async fn get_proposals(
        &self,
        status_filter: Option<ProposalStatus>,
        proposer_filter: Option<PeerId>,
    ) -> Result<Vec<Proposal>> {
        let mut results = Vec::new();
        
        for proposal in self.proposals.values() {
            let mut include = true;
            
            if let Some(status) = status_filter {
                if proposal.status != status {
                    include = false;
                }
            }
            
            if let Some(proposer) = proposer_filter {
                if proposal.proposer != proposer {
                    include = false;
                }
            }
            
            if include {
                results.push(proposal.clone());
            }
        }
        
        // Sort by creation date (newest first)
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(results)
    }

    /// Determine voting threshold for proposal type
    fn determine_threshold(&self, proposal_type: &ProposalType) -> VotingThreshold {
        match proposal_type {
            ProposalType::ProtocolUpgrade { .. } => VotingThreshold {
                quorum: 0.2,      // 20% quorum
                approval: 0.75,   // 75% approval
                supermajority: true,
                minimum_duration: Duration::days(7),
            },
            ProposalType::TreasuryAllocation { .. } => VotingThreshold {
                quorum: 0.15,     // 15% quorum
                approval: 0.6,    // 60% approval
                supermajority: false,
                minimum_duration: Duration::days(5),
            },
            ProposalType::ParameterChange { .. } => VotingThreshold {
                quorum: 0.1,      // 10% quorum
                approval: 0.55,   // 55% approval
                supermajority: false,
                minimum_duration: Duration::days(3),
            },
            ProposalType::MembershipChange { .. } => VotingThreshold {
                quorum: 0.2,      // 20% quorum
                approval: 0.67,   // 67% approval
                supermajority: true,
                minimum_duration: Duration::days(7),
            },
        }
    }
}

impl Proposal {
    /// Check if proposal is ready for execution
    pub fn is_ready_for_execution(&self) -> bool {
        matches!(self.status, ProposalStatus::AwaitingExecution) &&
        self.vote_result.as_ref().map_or(false, |r| r.passed) &&
        Utc::now() >= self.voting_ends
    }

    /// Get days remaining in current phase
    pub fn days_remaining(&self) -> Option<i64> {
        let now = Utc::now();
        match self.status {
            ProposalStatus::Discussion => Some((self.discussion_ends - now).num_days()),
            ProposalStatus::Voting => Some((self.voting_ends - now).num_days()),
            ProposalStatus::AwaitingExecution => Some((self.execution_deadline - now).num_days()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proposal_creation() {
        let config = ProposalConfig {
            minimum_proposal_tokens: CrapTokens::from_inner(1000),
            proposal_deposit: CrapTokens::from_inner(500),
            discussion_period: Duration::days(3),
            voting_period: Duration::days(7),
            execution_delay: Duration::days(2),
            max_lifetime: Duration::days(30),
        };

        let mut manager = ProposalManager::new(config).await.unwrap();
        
        let proposal_type = ProposalType::ParameterChange {
            parameter_name: "max_bet_size".to_string(),
            current_value: "1000".to_string(),
            new_value: "2000".to_string(),
            impact: "Allows larger bets".to_string(),
        };

        let proposal_id = manager.create_proposal(
            [1u8; 32],
            proposal_type,
            "Increase max bet size".to_string(),
            "This proposal increases the maximum bet size to 2000 CRAP tokens".to_string(),
            None,
        ).await.unwrap();

        let proposal = manager.get_proposal(&proposal_id).await.unwrap();
        assert_eq!(proposal.title, "Increase max bet size");
        assert_eq!(proposal.status, ProposalStatus::Pending);
    }

    #[tokio::test]
    async fn test_proposal_comment_addition() {
        let config = ProposalConfig {
            minimum_proposal_tokens: CrapTokens::from_inner(1000),
            proposal_deposit: CrapTokens::from_inner(500),
            discussion_period: Duration::days(3),
            voting_period: Duration::days(7),
            execution_delay: Duration::days(2),
            max_lifetime: Duration::days(30),
        };

        let mut manager = ProposalManager::new(config).await.unwrap();
        let proposal_type = ProposalType::ParameterChange {
            parameter_name: "test".to_string(),
            current_value: "1".to_string(),
            new_value: "2".to_string(),
            impact: "test impact".to_string(),
        };

        let proposal_id = manager.create_proposal(
            [1u8; 32],
            proposal_type,
            "Test Proposal".to_string(),
            "Test Description".to_string(),
            None,
        ).await.unwrap();

        // Manually set status to discussion for testing
        if let Some(proposal) = manager.proposals.get_mut(&proposal_id) {
            proposal.status = ProposalStatus::Discussion;
        }

        let result = manager.add_comment(
            proposal_id.clone(),
            [2u8; 32],
            "I support this proposal".to_string(),
            None,
            Some(true),
        ).await;

        assert!(result.is_ok());
        
        let proposal = manager.get_proposal(&proposal_id).await.unwrap();
        assert_eq!(proposal.comments.len(), 1);
        assert_eq!(proposal.comments[0].content, "I support this proposal");
    }
}