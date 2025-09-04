//! # Decentralized Autonomous Organization (DAO) Implementation
//!
//! Core DAO functionality including membership management, token-based governance,
//! and decentralized decision making.

use crate::{Error, Result, PeerId, CrapTokens};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// DAO configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaoConfig {
    /// Minimum tokens required for membership
    pub minimum_membership_tokens: CrapTokens,
    /// Reputation system enabled
    pub enable_reputation: bool,
    /// Membership tiers configuration
    pub membership_tiers: Vec<MembershipTierConfig>,
    /// Maximum number of active members
    pub max_members: Option<u32>,
    /// Membership fees
    pub membership_fee: CrapTokens,
    /// Governance participation rewards
    pub participation_rewards: bool,
}

/// Membership tier configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipTierConfig {
    /// Tier name
    pub name: String,
    /// Minimum token requirement
    pub min_tokens: CrapTokens,
    /// Minimum reputation requirement  
    pub min_reputation: u32,
    /// Voting power multiplier
    pub voting_multiplier: f64,
    /// Special privileges
    pub privileges: Vec<MemberPrivilege>,
}

/// Special privileges for members
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberPrivilege {
    /// Can create proposals
    CreateProposals,
    /// Can veto emergency actions
    EmergencyVeto,
    /// Access to private governance channels
    PrivateChannels,
    /// Enhanced voting power
    EnhancedVoting,
    /// Treasury access
    TreasuryAccess,
}

/// DAO member information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaoMember {
    /// Member's peer ID
    pub peer_id: PeerId,
    /// Token balance
    pub token_balance: CrapTokens,
    /// Reputation score
    pub reputation_score: u32,
    /// Membership tier
    pub membership_tier: MembershipTier,
    /// Join date
    pub joined_at: DateTime<Utc>,
    /// Voting activity
    pub voting_activity: VotingActivity,
    /// Locked tokens (for proposals, etc.)
    pub locked_tokens: CrapTokens,
    /// Member status
    pub status: MemberStatus,
}

/// Membership tiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MembershipTier {
    /// Basic member
    Basic = 1,
    /// Active contributor
    Contributor = 2,
    /// Senior member
    Senior = 3,
    /// DAO council member
    Council = 4,
    /// Founding member
    Founder = 5,
}

/// Member voting activity tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingActivity {
    /// Total votes cast
    pub votes_cast: u32,
    /// Proposals created
    pub proposals_created: u32,
    /// Participation rate (0-100)
    pub participation_rate: f64,
    /// Last vote timestamp
    pub last_vote: Option<DateTime<Utc>>,
}

/// Member status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberStatus {
    /// Active member
    Active,
    /// Inactive member
    Inactive,
    /// Suspended member
    Suspended,
    /// Expelled member
    Expelled,
}

/// DAO statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaoStats {
    /// Total number of members
    pub total_members: u32,
    /// Active members
    pub active_members: u32,
    /// Total tokens held by members
    pub total_member_tokens: CrapTokens,
    /// Average reputation score
    pub average_reputation: f64,
    /// Members by tier
    pub members_by_tier: HashMap<MembershipTier, u32>,
    /// Governance participation rate
    pub participation_rate: f64,
}

/// DAO implementation
pub struct Dao {
    /// Configuration
    config: DaoConfig,
    /// Member registry
    members: HashMap<PeerId, DaoMember>,
    /// Statistics
    stats: DaoStats,
}

impl Dao {
    /// Create new DAO
    pub async fn new(config: DaoConfig) -> Result<Self> {
        let stats = DaoStats {
            total_members: 0,
            active_members: 0,
            total_member_tokens: CrapTokens::zero(),
            average_reputation: 0.0,
            members_by_tier: HashMap::new(),
            participation_rate: 0.0,
        };

        Ok(Self {
            config,
            members: HashMap::new(),
            stats,
        })
    }

    /// Add new member to DAO
    pub async fn add_member(
        &mut self,
        peer_id: PeerId,
        token_balance: CrapTokens,
        reputation_score: u32,
    ) -> Result<()> {
        // Check minimum requirements
        if token_balance < self.config.minimum_membership_tokens {
            return Err(Error::ValidationError("Insufficient tokens for membership".to_string()));
        }

        // Determine membership tier
        let membership_tier = self.determine_membership_tier(token_balance, reputation_score);

        let member = DaoMember {
            peer_id,
            token_balance,
            reputation_score,
            membership_tier,
            joined_at: Utc::now(),
            voting_activity: VotingActivity {
                votes_cast: 0,
                proposals_created: 0,
                participation_rate: 0.0,
                last_vote: None,
            },
            locked_tokens: CrapTokens::zero(),
            status: MemberStatus::Active,
        };

        self.members.insert(peer_id, member);
        self.update_stats().await;

        Ok(())
    }

    /// Get member information
    pub async fn get_member(&self, peer_id: PeerId) -> Result<Option<DaoMember>> {
        Ok(self.members.get(&peer_id).cloned())
    }

    /// Lock tokens for member (for proposals, voting, etc.)
    pub async fn lock_tokens(&mut self, peer_id: PeerId, amount: CrapTokens) -> Result<()> {
        let member = self.members.get_mut(&peer_id)
            .ok_or_else(|| Error::ValidationError("Member not found".to_string()))?;

        if member.token_balance < amount {
            return Err(Error::ValidationError("Insufficient token balance".to_string()));
        }

        member.token_balance -= amount;
        member.locked_tokens += amount;

        Ok(())
    }

    /// Unlock tokens for member
    pub async fn unlock_tokens(&mut self, peer_id: PeerId, amount: CrapTokens) -> Result<()> {
        let member = self.members.get_mut(&peer_id)
            .ok_or_else(|| Error::ValidationError("Member not found".to_string()))?;

        if member.locked_tokens < amount {
            return Err(Error::ValidationError("Insufficient locked tokens".to_string()));
        }

        member.locked_tokens -= amount;
        member.token_balance += amount;

        Ok(())
    }

    /// Update member voting activity
    pub async fn record_vote(&mut self, peer_id: PeerId) -> Result<()> {
        let member = self.members.get_mut(&peer_id)
            .ok_or_else(|| Error::ValidationError("Member not found".to_string()))?;

        member.voting_activity.votes_cast += 1;
        member.voting_activity.last_vote = Some(Utc::now());

        // Recalculate participation rate
        Self::calculate_participation_rate(member);

        Ok(())
    }

    /// Update member reputation
    pub async fn update_reputation(&mut self, peer_id: PeerId, new_reputation: u32) -> Result<()> {
        // First get the member info we need without holding a mutable borrow
        let token_balance = {
            let member = self.members.get(&peer_id)
                .ok_or_else(|| Error::ValidationError("Member not found".to_string()))?;
            member.token_balance
        };
        
        // Calculate new tier before getting mutable access
        let new_tier = self.determine_membership_tier(token_balance, new_reputation);
        
        // Now update the member with mutable access
        let member = self.members.get_mut(&peer_id)
            .ok_or_else(|| Error::ValidationError("Member not found".to_string()))?;

        member.reputation_score = new_reputation;
        
        // Check if tier should be updated
        if new_tier != member.membership_tier {
            member.membership_tier = new_tier;
        }

        self.update_stats().await;

        Ok(())
    }

    /// Get DAO statistics
    pub async fn get_stats(&self) -> Result<DaoStats> {
        Ok(self.stats.clone())
    }

    /// Remove member from DAO
    pub async fn remove_member(&mut self, peer_id: PeerId) -> Result<()> {
        self.members.remove(&peer_id);
        self.update_stats().await;
        Ok(())
    }

    /// Determine membership tier based on tokens and reputation
    fn determine_membership_tier(&self, tokens: CrapTokens, reputation: u32) -> MembershipTier {
        for tier_config in &self.config.membership_tiers {
            if tokens >= tier_config.min_tokens && reputation >= tier_config.min_reputation {
                return match tier_config.name.as_str() {
                    "Founder" => MembershipTier::Founder,
                    "Council" => MembershipTier::Council,
                    "Senior" => MembershipTier::Senior,
                    "Contributor" => MembershipTier::Contributor,
                    _ => MembershipTier::Basic,
                };
            }
        }
        MembershipTier::Basic
    }

    /// Calculate participation rate for a member
    fn calculate_participation_rate(member: &mut DaoMember) {
        // Simplified calculation - would be more sophisticated in production
        let days_since_join = Utc::now().signed_duration_since(member.joined_at).num_days();
        if days_since_join > 0 {
            let expected_votes = days_since_join as f64 / 7.0; // Assume 1 vote per week expected
            member.voting_activity.participation_rate = 
                (member.voting_activity.votes_cast as f64 / expected_votes).min(1.0) * 100.0;
        }
    }

    /// Update DAO statistics
    async fn update_stats(&mut self) {
        self.stats.total_members = self.members.len() as u32;
        
        let mut active_count = 0;
        let mut total_tokens = CrapTokens::zero();
        let mut total_reputation = 0u64;
        let mut members_by_tier = HashMap::new();

        for member in self.members.values() {
            if member.status == MemberStatus::Active {
                active_count += 1;
            }

            total_tokens += member.token_balance + member.locked_tokens;
            total_reputation += member.reputation_score as u64;

            *members_by_tier.entry(member.membership_tier).or_insert(0) += 1;
        }

        self.stats.active_members = active_count;
        self.stats.total_member_tokens = total_tokens;
        self.stats.average_reputation = if self.stats.total_members > 0 {
            total_reputation as f64 / self.stats.total_members as f64
        } else {
            0.0
        };
        self.stats.members_by_tier = members_by_tier;

        // Calculate overall participation rate
        let total_votes: u32 = self.members.values()
            .map(|m| m.voting_activity.votes_cast)
            .sum();
        let expected_total_votes = self.stats.total_members * 10; // Assume 10 votes expected per member
        self.stats.participation_rate = if expected_total_votes > 0 {
            (total_votes as f64 / expected_total_votes as f64) * 100.0
        } else {
            0.0
        };
    }
}

impl Default for DaoConfig {
    fn default() -> Self {
        Self {
            minimum_membership_tokens: CrapTokens::from_inner(1000),
            enable_reputation: true,
            membership_tiers: vec![
                MembershipTierConfig {
                    name: "Basic".to_string(),
                    min_tokens: CrapTokens::from_inner(1000),
                    min_reputation: 0,
                    voting_multiplier: 1.0,
                    privileges: vec![],
                },
                MembershipTierConfig {
                    name: "Contributor".to_string(),
                    min_tokens: CrapTokens::from_inner(5000),
                    min_reputation: 100,
                    voting_multiplier: 1.2,
                    privileges: vec![MemberPrivilege::CreateProposals],
                },
                MembershipTierConfig {
                    name: "Senior".to_string(),
                    min_tokens: CrapTokens::from_inner(10000),
                    min_reputation: 500,
                    voting_multiplier: 1.5,
                    privileges: vec![
                        MemberPrivilege::CreateProposals,
                        MemberPrivilege::EnhancedVoting,
                    ],
                },
            ],
            max_members: Some(10000),
            membership_fee: CrapTokens::from_inner(100),
            participation_rewards: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dao_creation() {
        let config = DaoConfig::default();
        let dao = Dao::new(config).await.unwrap();
        
        let stats = dao.get_stats().await.unwrap();
        assert_eq!(stats.total_members, 0);
    }

    #[tokio::test]
    async fn test_member_addition() {
        let config = DaoConfig::default();
        let mut dao = Dao::new(config).await.unwrap();
        
        let peer_id = [1u8; 32];
        let tokens = CrapTokens::from_inner(5000);
        let reputation = 150;
        
        dao.add_member(peer_id, tokens, reputation).await.unwrap();
        
        let member = dao.get_member(peer_id).await.unwrap().unwrap();
        assert_eq!(member.peer_id, peer_id);
        assert_eq!(member.token_balance, tokens);
        assert_eq!(member.membership_tier, MembershipTier::Contributor);
    }

    #[tokio::test]
    async fn test_insufficient_tokens() {
        let config = DaoConfig::default();
        let mut dao = Dao::new(config).await.unwrap();
        
        let peer_id = [1u8; 32];
        let tokens = CrapTokens::from_inner(100); // Below minimum
        let reputation = 0;
        
        let result = dao.add_member(peer_id, tokens, reputation).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_token_locking() {
        let config = DaoConfig::default();
        let mut dao = Dao::new(config).await.unwrap();
        
        let peer_id = [1u8; 32];
        let tokens = CrapTokens::from_inner(5000);
        dao.add_member(peer_id, tokens, 0).await.unwrap();
        
        let lock_amount = CrapTokens::from_inner(1000);
        dao.lock_tokens(peer_id, lock_amount).await.unwrap();
        
        let member = dao.get_member(peer_id).await.unwrap().unwrap();
        assert_eq!(member.locked_tokens, lock_amount);
        assert_eq!(member.token_balance, tokens - lock_amount);
    }
}