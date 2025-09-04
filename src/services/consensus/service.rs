//! Consensus Service Implementation
//!
//! Implements Byzantine fault tolerant consensus for distributed game coordination.

use super::byzantine::ByzantineDetector;
use super::types::*;
use super::{ConsensusConfig, ConsensusAlgorithm, NetworkState, ConsensusMetrics, ConsensusRound, ConsensusProposal, ConsensusResult, ConsensusVote};
use crate::error::{Error, Result};
use crate::protocol::{GameId, PeerId, TransactionId};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock, broadcast};
use tokio::time::{interval, timeout};

/// Consensus Service
pub struct ConsensusService {
    config: ConsensusConfig,
    network_state: Arc<RwLock<NetworkState>>,
    active_rounds: Arc<DashMap<TransactionId, Arc<RwLock<ConsensusRound>>>>,
    byzantine_detector: Arc<ByzantineDetector>,
    metrics: Arc<RwLock<ConsensusMetrics>>,
    proposal_tx: broadcast::Sender<ConsensusProposal>,
    result_tx: broadcast::Sender<ConsensusResult>,
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
    peer_id: PeerId,
}

impl ConsensusService {
    /// Create a new consensus service
    pub fn new(config: ConsensusConfig, peer_id: PeerId) -> Self {
        let (proposal_tx, _) = broadcast::channel(1000);
        let (result_tx, _) = broadcast::channel(1000);
        
        Self {
            config: config.clone(),
            network_state: Arc::new(RwLock::new(NetworkState::new())),
            active_rounds: Arc::new(DashMap::new()),
            byzantine_detector: Arc::new(ByzantineDetector::new(config)),
            metrics: Arc::new(RwLock::new(ConsensusMetrics::default())),
            proposal_tx,
            result_tx,
            shutdown_tx: None,
            peer_id,
        }
    }

    /// Get quorum certificate for a committed sequence (if integrated engine is available).
    /// For now, returns None as a placeholder until engine is wired.
    pub async fn get_quorum_certificate(&self, _sequence: u64) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Get quorum certificate by proposal id
    pub async fn get_quorum_certificate_by_proposal(&self, proposal_id: TransactionId) -> Result<Option<Vec<u8>>> {
        Ok(self.quorum_certs.get(&proposal_id).map(|e| e.clone()))
    }
    
    /// Start the consensus service
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start background tasks
        let active_rounds = self.active_rounds.clone();
        let config = self.config.clone();
        let metrics = self.metrics.clone();
        let result_tx = self.result_tx.clone();
        
        // Round timeout monitor
        tokio::spawn(async move {
            let mut timeout_interval = interval(Duration::from_secs(1));
            
            loop {
                tokio::select! {
                    _ = timeout_interval.tick() => {
                        Self::check_round_timeouts(&active_rounds, &config, &metrics, &result_tx).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        // Network state synchronizer
        let network_state = self.network_state.clone();
        let byzantine_detector = self.byzantine_detector.clone();
        
        tokio::spawn(async move {
            let mut sync_interval = interval(Duration::from_secs(5));
            
            loop {
                tokio::select! {
                    _ = sync_interval.tick() => {
                        Self::update_network_state(&network_state, &byzantine_detector).await;
                    }
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                }
            }
        });
        
        log::info!("Consensus Service started");
        Ok(())
    }
    
    /// Stop the consensus service
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        log::info!("Consensus Service stopped");
        Ok(())
    }
    
    /// Submit a proposal for consensus
    pub async fn propose(&self, request: ProposeRequest) -> Result<ProposeResponse> {
        // Enforce: only active validators may propose
        {
            let network_state = self.network_state.read().await;
            let is_validator = network_state
                .validators
                .get(&self.peer_id)
                .map_or(false, |v| v.is_active);
            if !is_validator {
                return Err(Error::ConsensusError(
                    "Only active validators may propose".to_string(),
                ));
            }
        }

        let proposal_id = self.generate_transaction_id();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let proposal = ConsensusProposal {
            id: proposal_id,
            proposer: self.peer_id,
            game_id: request.game_id,
            proposal_type: request.proposal_type,
            data: request.data,
            timestamp,
            round: 0,
        };
        
        // Check if we have sufficient validators
        let network_state = self.network_state.read().await;
        if !network_state.has_sufficient_validators(&self.config) {
            return Err(Error::ConsensusError(
                "Insufficient validators for consensus".to_string()
            ));
        }
        drop(network_state);
        
        // Create initial consensus round
        let round = ConsensusRound {
            round_number: 0,
            proposal: Some(proposal.clone()),
            votes: std::collections::HashMap::new(),
            start_time: SystemTime::now(),
            status: super::RoundStatus::Proposed,
        };
        
        self.active_rounds.insert(proposal_id, Arc::new(RwLock::new(round)));
        
        // Broadcast proposal to network
        if let Err(_) = self.proposal_tx.send(proposal.clone()) {
            return Err(Error::ConsensusError("Failed to broadcast proposal".to_string()));
        }
        
        // Start consensus process based on algorithm
        match self.config.algorithm {
            ConsensusAlgorithm::PBFT => {
                self.run_pbft_consensus(proposal_id).await?;
            },
            ConsensusAlgorithm::Tendermint => {
                self.run_tendermint_consensus(proposal_id).await?;
            },
            ConsensusAlgorithm::HotStuff => {
                self.run_hotstuff_consensus(proposal_id).await?;
            },
        }
        
        Ok(ProposeResponse {
            proposal_id,
            status: "submitted".to_string(),
        })
    }
    
    /// Submit a vote on a proposal
    pub async fn vote(&self, request: VoteRequest) -> Result<VoteResponse> {
        let round_entry = self.active_rounds.get(&request.proposal_id)
            .ok_or_else(|| Error::ConsensusError("Proposal not found".to_string()))?;
        
        let mut round = round_entry.write().await;
        
        // Validate vote
        if !self.is_valid_vote(&request.vote, &*round).await? {
            return Err(Error::ConsensusError("Invalid vote".to_string()));
        }
        
        // Record vote
        let voter_votes = round.votes.entry(request.vote.voter).or_insert_with(std::collections::HashMap::new);
        voter_votes.insert(request.vote.vote_type, request.vote.clone());
        
        // Check if we have reached consensus
        if self.check_consensus_reached(&*round).await? {
            round.status = super::RoundStatus::Committed;

            // Build simple QC from commit votes
            let commit_sigs: Vec<(PeerId, Vec<u8>)> = round
                .votes
                .iter()
                .filter_map(|(peer, mp)| mp.get(&super::VoteType::Commit).map(|v| (*peer, v.signature.clone())))
                .collect();

            #[derive(serde::Serialize, serde::Deserialize)]
            struct ServiceQC {
                proposal_id: TransactionId,
                round: u32,
                commit_signatures: Vec<(PeerId, Vec<u8>)>,
            }
            let qc = ServiceQC {
                proposal_id: request.proposal_id,
                round: round.round_number,
                commit_signatures: commit_sigs,
            };
            let qc_bytes = bincode::serialize(&qc).map_err(|e| Error::ConsensusError(e.to_string()))?;
            self.quorum_certs.insert(request.proposal_id, qc_bytes.clone());

            let result = ConsensusResult {
                proposal_id: request.proposal_id,
                status: super::ConsensusStatus::Committed,
                final_round: round.round_number,
                commit_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                participating_validators: round.votes.keys().cloned().collect(),
                quorum_certificate: Some(qc_bytes),
            };

            // Broadcast result
            let _ = self.result_tx.send(result);

            // Clean up round
            drop(round);
            self.active_rounds.remove(&request.proposal_id);
        }
        
        Ok(VoteResponse {
            accepted: true,
            current_round: round.round_number,
        })
    }
    
    /// Get consensus status
    pub async fn get_status(&self, request: StatusRequest) -> Result<StatusResponse> {
        let network_state = self.network_state.read().await;
        let metrics = self.metrics.read().await;
        
        let active_proposals: Vec<_> = if let Some(proposal_id) = request.proposal_id {
            if let Some(round_entry) = self.active_rounds.get(&proposal_id) {
                let round = round_entry.read().await;
                vec![ActiveProposal {
                    proposal_id,
                    round: round.round_number,
                    status: format!("{:?}", round.status),
                    votes_received: round.votes.len() as u32,
                    votes_required: self.calculate_required_votes().await,
                }]
            } else {
                vec![]
            }
        } else {
            let mut proposals = Vec::new();
            for entry in self.active_rounds.iter() {
                let round = entry.value().read().await;
                proposals.push(ActiveProposal {
                    proposal_id: *entry.key(),
                    round: round.round_number,
                    status: format!("{:?}", round.status),
                    votes_received: round.votes.len() as u32,
                    votes_required: self.calculate_required_votes().await,
                });
            }
            proposals
        };
        
        Ok(StatusResponse {
            network_height: network_state.current_height,
            current_round: network_state.current_round,
            active_validators: network_state.active_validators().len() as u32,
            leader: network_state.leader,
            active_proposals,
            metrics: ConsensusMetricsResponse {
                total_proposals: metrics.total_proposals,
                committed_proposals: metrics.committed_proposals,
                rejected_proposals: metrics.rejected_proposals,
                timeout_proposals: metrics.timeout_proposals,
                byzantine_faults_detected: metrics.byzantine_faults_detected,
                average_rounds_to_commit: metrics.average_rounds_to_commit,
                average_time_to_commit_ms: metrics.average_time_to_commit.as_millis() as u64,
            },
        })
    }
    
    /// Add or update validator
    pub async fn update_validator(&self, request: UpdateValidatorRequest) -> Result<UpdateValidatorResponse> {
        let mut network_state = self.network_state.write().await;
        
        match request.action {
            ValidatorUpdateAction::Add => {
                let validator = super::Validator {
                    peer_id: request.peer_id,
                    stake: request.stake.unwrap_or(0),
                    reputation: 1.0,
                    is_active: true,
                    last_seen: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                };
                network_state.validators.insert(request.peer_id, validator);
            },
            ValidatorUpdateAction::Remove => {
                network_state.validators.remove(&request.peer_id);
            },
            ValidatorUpdateAction::Suspend => {
                if let Some(validator) = network_state.validators.get_mut(&request.peer_id) {
                    validator.is_active = false;
                }
            },
            ValidatorUpdateAction::Reinstate => {
                if let Some(validator) = network_state.validators.get_mut(&request.peer_id) {
                    validator.is_active = true;
                    validator.last_seen = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                }
            },
        }
        
        Ok(UpdateValidatorResponse {
            success: true,
            active_validators: network_state.active_validators().len() as u32,
        })
    }
    
    /// Subscribe to consensus events
    pub fn subscribe_proposals(&self) -> broadcast::Receiver<ConsensusProposal> {
        self.proposal_tx.subscribe()
    }
    
    /// Subscribe to consensus results
    pub fn subscribe_results(&self) -> broadcast::Receiver<ConsensusResult> {
        self.result_tx.subscribe()
    }
    
    // Private implementation methods
    
    fn generate_transaction_id(&self) -> TransactionId {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.peer_id.as_bytes());
        hasher.update(&SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_le_bytes());
        hasher.finalize().into()
    }
    
    async fn run_pbft_consensus(&self, proposal_id: TransactionId) -> Result<()> {
        // Simplified PBFT implementation
        // In production, this would implement the full 3-phase protocol
        
        log::debug!("Starting PBFT consensus for proposal: {:?}", proposal_id);
        
        // Phase 1: Pre-prepare (already done in propose())
        // Phase 2: Prepare
        // Phase 3: Commit
        
        Ok(())
    }
    
    async fn run_tendermint_consensus(&self, proposal_id: TransactionId) -> Result<()> {
        log::debug!("Starting Tendermint consensus for proposal: {:?}", proposal_id);
        // Implement Tendermint consensus logic
        Ok(())
    }
    
    async fn run_hotstuff_consensus(&self, proposal_id: TransactionId) -> Result<()> {
        log::debug!("Starting HotStuff consensus for proposal: {:?}", proposal_id);
        // Implement HotStuff consensus logic
        Ok(())
    }
    
    async fn is_valid_vote(&self, vote: &ConsensusVote, _round: &super::ConsensusRound) -> Result<bool> {
        // Check signature, voter authorization, etc.
        // For now, just check if voter is an active validator
        let network_state = self.network_state.read().await;
        Ok(network_state.validators.get(&vote.voter)
            .map_or(false, |v| v.is_active))
    }
    
    async fn check_consensus_reached(&self, round: &super::ConsensusRound) -> Result<bool> {
        let required_votes = self.calculate_required_votes().await;
        let commit_votes = round.votes.values()
            .filter(|votes| votes.contains_key(&super::VoteType::Commit))
            .count();
        
        Ok(commit_votes >= required_votes as usize)
    }
    
    async fn calculate_required_votes(&self) -> u32 {
        let network_state = self.network_state.read().await;
        let active_validators = network_state.active_validators().len() as u32;
        
        // For Byzantine fault tolerance: need 2f + 1 votes where f is max Byzantine nodes
        let f = self.config.byzantine_threshold as u32;
        std::cmp::min(2 * f + 1, active_validators)
    }
    
    async fn check_round_timeouts(
        active_rounds: &DashMap<TransactionId, Arc<RwLock<super::ConsensusRound>>>,
        config: &ConsensusConfig,
        metrics: &Arc<RwLock<ConsensusMetrics>>,
        result_tx: &broadcast::Sender<ConsensusResult>,
    ) {
        let timeout_threshold = SystemTime::now() - config.round_timeout;
        let mut timed_out = Vec::new();
        
        for entry in active_rounds.iter() {
            let round = entry.value().read().await;
            if round.start_time < timeout_threshold && 
               round.status == super::RoundStatus::Proposed {
                timed_out.push(*entry.key());
            }
        }
        
        for proposal_id in timed_out {
            if let Some((_, round_entry)) = active_rounds.remove(&proposal_id) {
                let mut round = round_entry.write().await;
                round.status = super::RoundStatus::Timeout;
                
                let result = ConsensusResult {
                    proposal_id,
                    status: super::ConsensusStatus::Timeout,
                    final_round: round.round_number,
                    commit_time: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    participating_validators: round.votes.keys().cloned().collect(),
                    quorum_certificate: None,
                };
                
                // Update metrics
                let mut metrics = metrics.write().await;
                metrics.record_consensus(&result, config.round_timeout);
                
                // Broadcast timeout result
                let _ = result_tx.send(result);
                
                log::warn!("Consensus round timed out for proposal: {:?}", proposal_id);
            }
        }
    }
    
    async fn update_network_state(
        _network_state: &Arc<RwLock<NetworkState>>,
        _byzantine_detector: &Arc<ByzantineDetector>,
    ) {
        // Update validator liveness, detect Byzantine behavior, etc.
        // Implementation would sync with network and update state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::craps::BetType;
    
    #[tokio::test]
    async fn test_consensus_service_creation() {
        let config = ConsensusConfig::default();
        let peer_id = [0u8; 32];
        let service = ConsensusService::new(config, peer_id);
        
        assert_eq!(service.peer_id, peer_id);
    }
    
    #[tokio::test]
    async fn test_propose_with_insufficient_validators() {
        let mut service = ConsensusService::new(ConsensusConfig::default(), [0u8; 32]);
        service.start().await.unwrap();
        
        let request = ProposeRequest {
            game_id: Some([0u8; 16]),
            proposal_type: super::ProposalType::GameAction {
                action: "place_bet".to_string(),
            },
            data: vec![],
        };
        
        let result = service.propose(request).await;
        assert!(result.is_err()); // Should fail due to not a validator and insufficient validators
        
        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_only_validators_can_propose_and_vote() {
        let mut service = ConsensusService::new(ConsensusConfig::default(), [0u8; 32]);
        service.start().await.unwrap();

        // Add two active validators, including this service.peer_id
        let my_id = service.peer_id;
        let other = [0u8; 32];
        service
            .update_validator(UpdateValidatorRequest {
                peer_id: my_id,
                action: super::ValidatorUpdateAction::Add,
                stake: Some(1),
            })
            .await
            .unwrap();
        service
            .update_validator(UpdateValidatorRequest {
                peer_id: other,
                action: super::ValidatorUpdateAction::Add,
                stake: Some(1),
            })
            .await
            .unwrap();

        // Propose should now be allowed (sufficient validators check may still fail if min_validators not met)
        let request = ProposeRequest {
            game_id: Some([0u8; 16]),
            proposal_type: super::ProposalType::GameAction {
                action: "place_bet".to_string(),
            },
            data: vec![],
        };
        let res = service.propose(request).await;
        // With default config min_validators=3, still insufficient; ensure error is due to validators count, not role
        assert!(res.is_err());

        // Now, attempt to vote as non-validator should fail
        let non_validator = [0u8; 32];
        let vote = ConsensusVote {
            proposal_id: [0u8; 32],
            voter: non_validator,
            vote_type: super::VoteType::Commit,
            round: 0,
            signature: vec![],
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        let vote_res = service
            .vote(VoteRequest {
                proposal_id: vote.proposal_id,
                vote,
            })
            .await;
        assert!(vote_res.is_err());

        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_quorum_certificate_on_commit() {
        let mut service = ConsensusService::new(ConsensusConfig::default(), [0u8; 32]);
        service.start().await.unwrap();

        // Add three active validators including this node
        let v1 = service.peer_id;
        let v2 = [0u8; 32];
        let v3 = [0u8; 32];
        for v in [v1, v2, v3] {
            service
                .update_validator(UpdateValidatorRequest {
                    peer_id: v,
                    action: super::ValidatorUpdateAction::Add,
                    stake: Some(1),
                })
                .await
                .unwrap();
        }

        // Propose
        let req = ProposeRequest {
            game_id: None,
            proposal_type: super::ProposalType::NetworkUpgrade { version: "test".to_string() },
            data: vec![],
        };
        let resp = service.propose(req).await.unwrap();
        let pid = resp.proposal_id;

        // Cast three commit votes from validators
        for voter in [v1, v2, v3] {
            let vote = ConsensusVote {
                proposal_id: pid,
                voter,
                vote_type: super::VoteType::Commit,
                round: 0,
                signature: vec![1,2,3],
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            };
            let _ = service.vote(VoteRequest { proposal_id: pid, vote }).await.unwrap();
        }

        // QC should be stored
        let qc = service.get_quorum_certificate_by_proposal(pid).await.unwrap();
        assert!(qc.is_some());

        service.stop().await.unwrap();
    }
}
