//! Consensus API
//!
//! High-level API for distributed consensus operations including proposals,
//! voting, and Byzantine fault tolerance.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    types::*,
    rest::RestClient,
    SDKContext,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Consensus management API
#[derive(Debug)]
pub struct ConsensusAPI {
    context: Arc<SDKContext>,
    rest_client: RestClient,
    active_proposals: Arc<RwLock<HashMap<ProposalId, ConsensusProposal>>>,
    voting_history: Arc<RwLock<Vec<VotingRecord>>>,
}

impl ConsensusAPI {
    /// Create a new consensus API instance
    pub fn new(context: Arc<SDKContext>) -> Self {
        let rest_client = RestClient::new(&context.config)
            .expect("Failed to create REST client");
        
        Self {
            context,
            rest_client,
            active_proposals: Arc::new(RwLock::new(HashMap::new())),
            voting_history: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Submit a new consensus proposal
    pub async fn propose(&self, game_id: &GameId, action: GameAction) -> SDKResult<ProposalId> {
        let request = CreateProposalRequest {
            game_id: game_id.clone(),
            proposer: self.get_current_player_id().await?,
            action,
            timeout_seconds: 300, // 5 minutes default
        };
        
        let response: CreateProposalResponse = self.rest_client
            .post("consensus/proposals", request)
            .await?;
        
        // Cache the proposal locally
        {
            let mut proposals = self.active_proposals.write().await;
            proposals.insert(response.proposal_id.clone(), response.proposal.clone());
        }
        
        // Update metrics
        {
            let mut metrics = self.context.metrics.write().await;
            metrics.consensus_operations += 1;
        }
        
        Ok(response.proposal_id)
    }
    
    /// Vote on a consensus proposal
    pub async fn vote(&self, proposal_id: &ProposalId, vote: Vote) -> SDKResult<()> {
        let request = SubmitVoteRequest {
            proposal_id: proposal_id.clone(),
            voter: self.get_current_player_id().await?,
            vote,
            signature: self.sign_vote(proposal_id, vote).await?,
        };
        
        let _: serde_json::Value = self.rest_client
            .post(&format!("consensus/proposals/{}/votes", proposal_id), request)
            .await?;
        
        // Record voting history
        {
            let mut history = self.voting_history.write().await;
            history.push(VotingRecord {
                proposal_id: proposal_id.clone(),
                vote,
                voted_at: chrono::Utc::now(),
                voter: request.voter,
            });
        }
        
        Ok(())
    }
    
    /// Get proposal details
    pub async fn get_proposal(&self, proposal_id: &ProposalId) -> SDKResult<ConsensusProposal> {
        // Check local cache first
        {
            let proposals = self.active_proposals.read().await;
            if let Some(proposal) = proposals.get(proposal_id) {
                return Ok(proposal.clone());
            }
        }
        
        // Fetch from server
        let proposal: ConsensusProposal = self.rest_client
            .get(&format!("consensus/proposals/{}", proposal_id))
            .await?;
        
        // Update cache
        {
            let mut proposals = self.active_proposals.write().await;
            proposals.insert(proposal_id.clone(), proposal.clone());
        }
        
        Ok(proposal)
    }
    
    /// List active proposals
    pub async fn list_proposals(&self, game_id: Option<&GameId>) -> SDKResult<Vec<ConsensusProposal>> {
        let path = if let Some(game_id) = game_id {
            format!("consensus/proposals?game_id={}", game_id)
        } else {
            "consensus/proposals".to_string()
        };
        
        let proposals: Vec<ConsensusProposal> = self.rest_client
            .get(&path)
            .await?;
        
        // Update local cache
        {
            let mut active_proposals = self.active_proposals.write().await;
            for proposal in &proposals {
                active_proposals.insert(proposal.id.clone(), proposal.clone());
            }
        }
        
        Ok(proposals)
    }
    
    /// Get consensus status for a game
    pub async fn get_game_consensus_status(&self, game_id: &GameId) -> SDKResult<ConsensusStatus> {
        let status: ConsensusStatus = self.rest_client
            .get(&format!("consensus/games/{}/status", game_id))
            .await?;
        
        Ok(status)
    }
    
    /// Get voting statistics
    pub async fn get_voting_stats(&self, game_id: &GameId) -> SDKResult<VotingStatistics> {
        let stats: VotingStatistics = self.rest_client
            .get(&format!("consensus/games/{}/stats", game_id))
            .await?;
        
        Ok(stats)
    }
    
    /// Create a proposal builder
    pub fn create_proposal(&self, game_id: &GameId) -> ProposalBuilder {
        ProposalBuilder::new(game_id.clone(), self.rest_client.clone())
    }
    
    /// Get player's voting history
    pub async fn get_voting_history(&self) -> Vec<VotingRecord> {
        self.voting_history.read().await.clone()
    }
    
    /// Cancel a proposal (only by proposer)
    pub async fn cancel_proposal(&self, proposal_id: &ProposalId) -> SDKResult<()> {
        let _: serde_json::Value = self.rest_client
            .delete(&format!("consensus/proposals/{}", proposal_id))
            .await?;
        
        // Remove from cache
        {
            let mut proposals = self.active_proposals.write().await;
            proposals.remove(proposal_id);
        }
        
        Ok(())
    }
    
    /// Get consensus rules for a game
    pub async fn get_consensus_rules(&self, game_id: &GameId) -> SDKResult<ConsensusRules> {
        let rules: ConsensusRules = self.rest_client
            .get(&format!("games/{}/consensus/rules", game_id))
            .await?;
        
        Ok(rules)
    }
    
    /// Submit a batch of votes (for multiple proposals)
    pub async fn batch_vote(&self, votes: Vec<(ProposalId, Vote)>) -> SDKResult<BatchVoteResult> {
        let request = BatchVoteRequest {
            votes: votes.iter().map(|(proposal_id, vote)| VoteItem {
                proposal_id: proposal_id.clone(),
                vote: *vote,
                signature: None, // Would be computed in real implementation
            }).collect(),
            voter: self.get_current_player_id().await?,
        };
        
        let result: BatchVoteResult = self.rest_client
            .post("consensus/batch-vote", request)
            .await?;
        
        Ok(result)
    }
    
    // Private helper methods
    
    async fn get_current_player_id(&self) -> SDKResult<PlayerId> {
        // In a real implementation, this would come from authentication context
        Ok("current_player_id".to_string())
    }
    
    async fn sign_vote(&self, _proposal_id: &ProposalId, _vote: Vote) -> SDKResult<String> {
        // In a real implementation, this would create a cryptographic signature
        Ok("signature_placeholder".to_string())
    }
}

/// Proposal builder for creating complex consensus proposals
#[derive(Debug)]
pub struct ProposalBuilder {
    game_id: GameId,
    action: Option<GameAction>,
    timeout_seconds: u64,
    required_votes: Option<u32>,
    description: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
    rest_client: RestClient,
}

impl ProposalBuilder {
    /// Create a new proposal builder
    pub fn new(game_id: GameId, rest_client: RestClient) -> Self {
        Self {
            game_id,
            action: None,
            timeout_seconds: 300, // 5 minutes default
            required_votes: None,
            description: None,
            metadata: HashMap::new(),
            rest_client,
        }
    }
    
    /// Set the action to propose
    pub fn action(mut self, action: GameAction) -> Self {
        self.action = Some(action);
        self
    }
    
    /// Set proposal timeout
    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }
    
    /// Set required number of votes
    pub fn required_votes(mut self, votes: u32) -> Self {
        self.required_votes = Some(votes);
        self
    }
    
    /// Set proposal description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }
    
    /// Add metadata
    pub fn metadata<K: Into<String>, V: serde::Serialize>(mut self, key: K, value: V) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), json_value);
        }
        self
    }
    
    /// Submit the proposal
    pub async fn submit(self) -> SDKResult<ProposalId> {
        let action = self.action
            .ok_or_else(|| SDKError::ValidationError {
                message: "Action is required for proposal".to_string(),
                field: Some("action".to_string()),
                invalid_value: None,
            })?;
        
        let request = CreateProposalRequest {
            game_id: self.game_id,
            proposer: "current_player_id".to_string(), // Would come from context
            action,
            timeout_seconds: self.timeout_seconds,
        };
        
        let response: CreateProposalResponse = self.rest_client
            .post("consensus/proposals", request)
            .await?;
        
        Ok(response.proposal_id)
    }
}

/// Request/Response structures
#[derive(Debug, Serialize)]
struct CreateProposalRequest {
    game_id: GameId,
    proposer: PlayerId,
    action: GameAction,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct CreateProposalResponse {
    proposal_id: ProposalId,
    proposal: ConsensusProposal,
}

#[derive(Debug, Serialize)]
struct SubmitVoteRequest {
    proposal_id: ProposalId,
    voter: PlayerId,
    vote: Vote,
    signature: String,
}

#[derive(Debug, Serialize)]
struct BatchVoteRequest {
    votes: Vec<VoteItem>,
    voter: PlayerId,
}

#[derive(Debug, Serialize)]
struct VoteItem {
    proposal_id: ProposalId,
    vote: Vote,
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchVoteResult {
    pub successful_votes: u32,
    pub failed_votes: Vec<FailedVote>,
    pub total_votes: u32,
}

#[derive(Debug, Deserialize)]
pub struct FailedVote {
    pub proposal_id: ProposalId,
    pub error: String,
}

/// Consensus status for a game
#[derive(Debug, Deserialize)]
pub struct ConsensusStatus {
    pub active_proposals: u32,
    pub pending_votes: u32,
    pub consensus_health: ConsensusHealth,
    pub last_consensus: Option<chrono::DateTime<chrono::Utc>>,
    pub byzantine_faults_detected: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum ConsensusHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Voting statistics
#[derive(Debug, Deserialize)]
pub struct VotingStatistics {
    pub total_proposals: u32,
    pub approved_proposals: u32,
    pub rejected_proposals: u32,
    pub timed_out_proposals: u32,
    pub average_voting_time: f64,
    pub participation_rate: f64,
    pub consensus_efficiency: f64,
}

/// Voting record
#[derive(Debug, Clone)]
pub struct VotingRecord {
    pub proposal_id: ProposalId,
    pub vote: Vote,
    pub voted_at: chrono::DateTime<chrono::Utc>,
    pub voter: PlayerId,
}

/// Consensus rules for a game
#[derive(Debug, Deserialize)]
pub struct ConsensusRules {
    pub required_majority: f64, // e.g., 0.67 for 2/3 majority
    pub proposal_timeout: u64,
    pub max_active_proposals: u32,
    pub byzantine_threshold: f64, // e.g., 0.33 for 1/3 Byzantine tolerance
    pub voting_power_distribution: HashMap<PlayerId, f64>,
}

/// Preset proposal types
pub struct ProposalPresets;

impl ProposalPresets {
    /// Create a proposal to start the game
    pub fn start_game(game_id: GameId, rest_client: RestClient) -> ProposalBuilder {
        ProposalBuilder::new(game_id, rest_client)
            .action(GameAction::Custom {
                action_type: "start_game".to_string(),
                data: serde_json::json!({}),
            })
            .description("Proposal to start the game")
            .timeout(60) // Quick timeout for game start
            .required_votes(2) // Minimum consensus
    }
    
    /// Create a proposal to pause the game
    pub fn pause_game(game_id: GameId, reason: String, rest_client: RestClient) -> ProposalBuilder {
        ProposalBuilder::new(game_id, rest_client)
            .action(GameAction::Custom {
                action_type: "pause_game".to_string(),
                data: serde_json::json!({"reason": reason}),
            })
            .description(&format!("Proposal to pause game: {}", reason))
            .timeout(120)
    }
    
    /// Create a proposal to kick a player
    pub fn kick_player(game_id: GameId, player_id: PlayerId, reason: String, rest_client: RestClient) -> ProposalBuilder {
        ProposalBuilder::new(game_id, rest_client)
            .action(GameAction::Custom {
                action_type: "kick_player".to_string(),
                data: serde_json::json!({"player_id": player_id, "reason": reason}),
            })
            .description(&format!("Proposal to kick player {}: {}", player_id, reason))
            .timeout(300)
            .required_votes(3) // Higher consensus required for kicking
    }
    
    /// Create a proposal to change betting limits
    pub fn change_betting_limits(
        game_id: GameId, 
        min_bet: u64, 
        max_bet: u64, 
        rest_client: RestClient
    ) -> ProposalBuilder {
        ProposalBuilder::new(game_id, rest_client)
            .action(GameAction::Custom {
                action_type: "change_betting_limits".to_string(),
                data: serde_json::json!({"min_bet": min_bet, "max_bet": max_bet}),
            })
            .description(&format!("Proposal to change betting limits: {} - {}", min_bet, max_bet))
            .timeout(180)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_v2::{config::{Config, Environment}, init};
    
    #[tokio::test]
    async fn test_consensus_api_creation() {
        let config = Config::builder()
            .api_key("test-key")
            .environment(Environment::Testing)
            .build()
            .unwrap();
            
        let context = init(config).await.unwrap();
        let consensus_api = ConsensusAPI::new(context);
        
        // Test that the API was created successfully
        assert_eq!(consensus_api.get_voting_history().await.len(), 0);
    }
    
    #[test]
    fn test_proposal_builder_validation() {
        let builder = ProposalBuilder::new(
            "test_game".to_string(),
            RestClient::new(&Default::default()).unwrap()
        );
        
        // Should fail without action
        let result = tokio_test::block_on(builder.submit());
        assert!(result.is_err());
    }
    
    #[test]
    fn test_proposal_presets() {
        let rest_client = RestClient::new(&Default::default()).unwrap();
        
        let start_proposal = ProposalPresets::start_game("game1".to_string(), rest_client.clone());
        assert_eq!(start_proposal.timeout_seconds, 60);
        
        let pause_proposal = ProposalPresets::pause_game(
            "game1".to_string(),
            "Technical issue".to_string(),
            rest_client
        );
        assert_eq!(pause_proposal.timeout_seconds, 120);
    }
}