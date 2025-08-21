# BitChat Security Enhancements

This document outlines critical security implementations for the BitChat decentralized messaging protocol, focusing on practical attack mitigation with concrete Rust code examples.

## 1. Eclipse Attack Mitigation - Redundant Routing

Eclipse attacks isolate nodes by controlling their peer connections. We implement redundant path discovery with minimum 5+ diverse routing paths.

```rust
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::time::{Duration, Interval};
use rand::{seq::SliceRandom, thread_rng};

pub struct EclipseMitigation {
    peer_diversity_score: HashMap<PeerId, f64>,
    routing_paths: Vec<Vec<PeerId>>,
    bootstrap_nodes: Vec<PeerId>,
    path_verification_interval: Interval,
    min_paths: usize,
}

impl EclipseMitigation {
    pub fn new() -> Self {
        Self {
            peer_diversity_score: HashMap::new(),
            routing_paths: Vec::new(),
            bootstrap_nodes: Vec::new(),
            path_verification_interval: tokio::time::interval(Duration::from_secs(30)),
            min_paths: 5,
        }
    }

    /// Calculate peer diversity based on IP ranges, ASN, geographic distribution
    pub fn calculate_diversity_score(&mut self, peer_id: &PeerId, peer_info: &PeerInfo) -> f64 {
        let mut score = 1.0;
        
        // IP diversity (different /24 subnets)
        let subnet = peer_info.ip.octets()[..3].to_vec();
        let subnet_peers = self.count_peers_in_subnet(&subnet);
        score *= (10.0 - subnet_peers as f64).max(1.0) / 10.0;
        
        // ASN diversity
        if let Some(asn) = peer_info.asn {
            let asn_peers = self.count_peers_in_asn(asn);
            score *= (5.0 - asn_peers as f64).max(1.0) / 5.0;
        }
        
        // Geographic diversity
        if let Some(geo) = &peer_info.geographic_info {
            let geo_peers = self.count_peers_in_region(&geo.country);
            score *= (8.0 - geo_peers as f64).max(1.0) / 8.0;
        }
        
        self.peer_diversity_score.insert(*peer_id, score);
        score
    }

    /// Discover multiple independent routing paths
    pub async fn discover_redundant_paths(&mut self, target: PeerId) -> Result<Vec<Vec<PeerId>>> {
        let mut paths = Vec::new();
        let mut used_intermediates = HashSet::new();
        
        for _ in 0..self.min_paths {
            if let Some(path) = self.find_diverse_path(target, &used_intermediates).await? {
                // Mark intermediate nodes as used to force path diversity
                for node in &path[1..path.len()-1] {
                    used_intermediates.insert(*node);
                }
                paths.push(path);
            }
        }
        
        self.routing_paths = paths.clone();
        Ok(paths)
    }

    async fn find_diverse_path(
        &self, 
        target: PeerId, 
        excluded: &HashSet<PeerId>
    ) -> Result<Option<Vec<PeerId>>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent = HashMap::new();
        
        // Start with highest diversity score peers
        let mut start_peers: Vec<_> = self.peer_diversity_score
            .iter()
            .filter(|(id, _)| !excluded.contains(id))
            .collect();
        start_peers.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        
        for (peer_id, _) in start_peers.iter().take(3) {
            queue.push_back(**peer_id);
            visited.insert(**peer_id);
        }
        
        while let Some(current) = queue.pop_front() {
            if current == target {
                return Ok(Some(self.reconstruct_path(target, &parent)));
            }
            
            let neighbors = self.get_peer_neighbors(&current).await?;
            for neighbor in neighbors {
                if !visited.contains(&neighbor) && !excluded.contains(&neighbor) {
                    visited.insert(neighbor);
                    parent.insert(neighbor, current);
                    queue.push_back(neighbor);
                }
            }
        }
        
        Ok(None)
    }

    /// Verify path liveness and switch if paths are compromised
    pub async fn verify_and_heal_paths(&mut self) -> Result<()> {
        let mut healthy_paths = Vec::new();
        
        for path in &self.routing_paths {
            if self.verify_path_liveness(path).await? {
                healthy_paths.push(path.clone());
            }
        }
        
        // If we have fewer than minimum paths, discover new ones
        if healthy_paths.len() < self.min_paths {
            tracing::warn!("Eclipse attack detected: only {} healthy paths", healthy_paths.len());
            self.emergency_path_discovery().await?;
        }
        
        Ok(())
    }

    async fn emergency_path_discovery(&mut self) -> Result<()> {
        // Connect to bootstrap nodes with highest diversity
        for bootstrap in &self.bootstrap_nodes.clone() {
            if let Ok(peers) = self.discover_peers_via_bootstrap(*bootstrap).await {
                for peer in peers {
                    self.calculate_diversity_score(&peer.id, &peer.info);
                }
            }
        }
        Ok(())
    }
}
```

## 2. Sybil Prevention - Progressive Proof-of-Work with Argon2id

Prevent fake identity creation through computational cost that scales with identity creation rate.

```rust
use argon2::{Argon2, Config, Variant, Version};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SybilPreventionEngine {
    base_difficulty: u32,
    difficulty_scaling: f64,
    identity_creation_history: HashMap<IpAddr, Vec<u64>>,
    pow_config: Config,
    argon2: Argon2<'static>,
}

impl SybilPreventionEngine {
    pub fn new() -> Self {
        let pow_config = Config {
            variant: Variant::Argon2id,
            version: Version::Version13,
            mem_cost: 65536,      // 64 MB memory
            time_cost: 10,        // 10 iterations
            lanes: 4,            // 4 parallel threads
            thread_mode: argon2::ThreadMode::Parallel,
            secret: &[],
            ad: &[],
            hash_length: 32,
        };

        Self {
            base_difficulty: 20,  // ~1 second on modern CPU
            difficulty_scaling: 1.5,
            identity_creation_history: HashMap::new(),
            pow_config,
            argon2: Argon2::default(),
        }
    }

    /// Calculate required PoW difficulty based on creation history
    pub fn calculate_required_difficulty(&mut self, ip: &IpAddr) -> u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Clean old entries (older than 1 hour)
        if let Some(history) = self.identity_creation_history.get_mut(ip) {
            history.retain(|&timestamp| now - timestamp < 3600);
            
            // Progressive scaling: each identity from same IP increases difficulty
            let recent_count = history.len();
            let difficulty_multiplier = self.difficulty_scaling.powi(recent_count as i32);
            
            (self.base_difficulty as f64 * difficulty_multiplier) as u32
        } else {
            self.base_difficulty
        }
    }

    /// Verify identity creation PoW with Argon2id
    pub fn verify_identity_proof(
        &self,
        identity_pubkey: &[u8],
        nonce: u64,
        difficulty: u32,
        timestamp: u64,
    ) -> Result<bool> {
        // Combine identity public key, nonce, and timestamp
        let mut input = Vec::new();
        input.extend_from_slice(identity_pubkey);
        input.extend_from_slice(&nonce.to_le_bytes());
        input.extend_from_slice(&timestamp.to_le_bytes());
        
        // Generate salt from public key
        let salt = Sha256::digest(identity_pubkey);
        
        // Compute Argon2id hash
        let hash = self.argon2.hash_password_into(&input, &salt[..16], &mut [0u8; 32])
            .map_err(|e| anyhow::anyhow!("Argon2 error: {}", e))?;
        
        // Check if hash meets difficulty requirement
        let leading_zeros = self.count_leading_zeros(&hash);
        Ok(leading_zeros >= difficulty)
    }

    /// Generate identity PoW (for legitimate users)
    pub async fn generate_identity_proof(
        &self,
        identity_pubkey: &[u8],
        difficulty: u32,
    ) -> Result<(u64, u64)> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut nonce = 0u64;
        let salt = Sha256::digest(identity_pubkey);
        
        loop {
            let mut input = Vec::new();
            input.extend_from_slice(identity_pubkey);
            input.extend_from_slice(&nonce.to_le_bytes());
            input.extend_from_slice(&timestamp.to_le_bytes());
            
            let mut hash = [0u8; 32];
            if self.argon2.hash_password_into(&input, &salt[..16], &mut hash).is_ok() {
                if self.count_leading_zeros(&hash) >= difficulty {
                    return Ok((nonce, timestamp));
                }
            }
            
            nonce += 1;
            
            // Yield occasionally to prevent blocking
            if nonce % 1000 == 0 {
                tokio::task::yield_now().await;
            }
        }
    }

    fn count_leading_zeros(&self, hash: &[u8]) -> u32 {
        let mut zeros = 0;
        for &byte in hash {
            if byte == 0 {
                zeros += 8;
            } else {
                zeros += byte.leading_zeros();
                break;
            }
        }
        zeros
    }

    /// Record successful identity creation for rate limiting
    pub fn record_identity_creation(&mut self, ip: &IpAddr) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        self.identity_creation_history
            .entry(*ip)
            .or_insert_with(Vec::new)
            .push(now);
    }
}
```

## 3. Collusion Detection - Zero-Knowledge Proofs & Statistical Monitoring

Detect coordinated attacks through behavioral analysis and cryptographic commitment schemes.

```rust
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek::scalar::Scalar;
use std::collections::BTreeMap;

pub struct CollusionDetector {
    message_timing_patterns: HashMap<PeerId, VecDeque<u64>>,
    vote_correlations: BTreeMap<(PeerId, PeerId), f64>,
    behavioral_scores: HashMap<PeerId, BehavioralProfile>,
    bp_gens: BulletproofGens,
    pc_gens: PedersenGens,
}

#[derive(Clone, Debug)]
pub struct BehavioralProfile {
    pub message_intervals: Vec<f64>,
    pub response_times: Vec<f64>,
    pub vote_patterns: Vec<u8>,
    pub online_periods: Vec<(u64, u64)>,
    pub suspicion_score: f64,
}

impl CollusionDetector {
    pub fn new() -> Self {
        Self {
            message_timing_patterns: HashMap::new(),
            vote_correlations: BTreeMap::new(),
            behavioral_scores: HashMap::new(),
            bp_gens: BulletproofGens::new(64, 1),
            pc_gens: PedersenGens::default(),
        }
    }

    /// Generate zero-knowledge proof of honest behavior without revealing actions
    pub fn generate_honesty_proof(
        &self,
        peer_actions: &[u64],
        honest_threshold: u64,
    ) -> Result<(RangeProof, Vec<u8>)> {
        let mut transcript = merlin::Transcript::new(b"honesty_proof");
        
        // Prove that sum of honest actions >= threshold without revealing individual actions
        let honest_sum: u64 = peer_actions.iter().sum();
        let value = Scalar::from(honest_sum);
        let blinding = Scalar::random(&mut rand::thread_rng());
        
        let (proof, commitment) = RangeProof::prove_single(
            &self.bp_gens,
            &self.pc_gens,
            &mut transcript,
            value,
            &blinding,
            32, // 32-bit range
        ).map_err(|e| anyhow::anyhow!("Proof generation failed: {:?}", e))?;
        
        Ok((proof, commitment.compress().as_bytes().to_vec()))
    }

    /// Verify zero-knowledge honesty proof
    pub fn verify_honesty_proof(
        &self,
        proof: &RangeProof,
        commitment_bytes: &[u8],
        min_threshold: u64,
    ) -> Result<bool> {
        let mut transcript = merlin::Transcript::new(b"honesty_proof");
        
        let commitment = curve25519_dalek::ristretto::CompressedRistretto::from_slice(commitment_bytes)
            .decompress()
            .ok_or_else(|| anyhow::anyhow!("Invalid commitment"))?;
        
        // Verify the range proof
        let verification = proof.verify_single(
            &self.bp_gens,
            &self.pc_gens,
            &mut transcript,
            &commitment,
            32,
        );
        
        Ok(verification.is_ok())
    }

    /// Detect coordinated timing patterns indicating collusion
    pub fn detect_timing_collusion(&mut self, peer_id: PeerId, message_time: u64) -> f64 {
        let entry = self.message_timing_patterns.entry(peer_id).or_insert_with(VecDeque::new);
        
        // Keep sliding window of last 100 message times
        if entry.len() >= 100 {
            entry.pop_front();
        }
        entry.push_back(message_time);
        
        if entry.len() < 10 {
            return 0.0; // Not enough data
        }
        
        // Calculate autocorrelation to detect artificial timing patterns
        let intervals: Vec<f64> = entry.windows(2)
            .map(|w| (w[1] - w[0]) as f64)
            .collect();
        
        let correlation = self.calculate_autocorrelation(&intervals);
        
        // Update behavioral profile
        self.behavioral_scores.entry(peer_id)
            .or_insert_with(|| BehavioralProfile {
                message_intervals: Vec::new(),
                response_times: Vec::new(),
                vote_patterns: Vec::new(),
                online_periods: Vec::new(),
                suspicion_score: 0.0,
            })
            .message_intervals = intervals;
        
        correlation
    }

    /// Calculate vote correlation between peers to detect coordinated voting
    pub fn analyze_vote_correlation(&mut self, votes: &HashMap<PeerId, u8>) -> Vec<(PeerId, PeerId, f64)> {
        let mut suspicious_pairs = Vec::new();
        
        for (peer1, vote1) in votes {
            for (peer2, vote2) in votes {
                if peer1 >= peer2 {
                    continue;
                }
                
                let correlation = self.vote_correlations
                    .entry((*peer1, *peer2))
                    .or_insert(0.0);
                
                // Exponential moving average of vote agreement
                let agreement = if vote1 == vote2 { 1.0 } else { 0.0 };
                *correlation = 0.1 * agreement + 0.9 * *correlation;
                
                // Flag highly correlated pairs
                if *correlation > 0.85 {
                    suspicious_pairs.push((*peer1, *peer2, *correlation));
                }
            }
        }
        
        suspicious_pairs
    }

    fn calculate_autocorrelation(&self, data: &[f64]) -> f64 {
        if data.len() < 3 {
            return 0.0;
        }
        
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / data.len() as f64;
        
        if variance == 0.0 {
            return 1.0; // Perfect correlation (suspicious)
        }
        
        // Calculate lag-1 autocorrelation
        let lag1_covariance = data.windows(2)
            .map(|w| (w[0] - mean) * (w[1] - mean))
            .sum::<f64>() / (data.len() - 1) as f64;
        
        (lag1_covariance / variance).abs()
    }

    /// Update suspicion scores based on multiple factors
    pub fn update_suspicion_scores(&mut self) {
        for (peer_id, profile) in &mut self.behavioral_scores {
            let mut score = 0.0;
            
            // Timing pattern suspicion
            if !profile.message_intervals.is_empty() {
                let timing_correlation = self.calculate_autocorrelation(&profile.message_intervals);
                score += timing_correlation * 0.3;
            }
            
            // Vote correlation suspicion
            let peer_correlations: f64 = self.vote_correlations
                .iter()
                .filter(|((p1, p2), _)| p1 == peer_id || p2 == peer_id)
                .map(|(_, correlation)| *correlation)
                .filter(|&c| c > 0.7)
                .sum();
            
            score += (peer_correlations * 0.4).min(1.0);
            
            // Response time analysis (too fast = bot, too consistent = scripted)
            if profile.response_times.len() > 5 {
                let avg_response = profile.response_times.iter().sum::<f64>() / profile.response_times.len() as f64;
                let std_dev = (profile.response_times.iter()
                    .map(|x| (x - avg_response).powi(2))
                    .sum::<f64>() / profile.response_times.len() as f64).sqrt();
                
                // Suspicion for unusually fast or consistent responses
                if avg_response < 100.0 || std_dev < 50.0 {
                    score += 0.3;
                }
            }
            
            profile.suspicion_score = score.min(1.0);
        }
    }

    /// Get peers above suspicion threshold for potential exclusion
    pub fn get_suspicious_peers(&self, threshold: f64) -> Vec<(PeerId, f64)> {
        self.behavioral_scores
            .iter()
            .filter(|(_, profile)| profile.suspicion_score > threshold)
            .map(|(peer_id, profile)| (*peer_id, profile.suspicion_score))
            .collect()
    }
}
```

## 4. Byzantine Tolerance - PBFT Consensus for Game State

Implement Practical Byzantine Fault Tolerance for critical game state consistency across 3f+1 nodes.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PbftMessage {
    Request(GameStateUpdate),
    PrePrepare { view: u64, sequence: u64, digest: [u8; 32], update: GameStateUpdate },
    Prepare { view: u64, sequence: u64, digest: [u8; 32], node_id: NodeId },
    Commit { view: u64, sequence: u64, digest: [u8; 32], node_id: NodeId },
    ViewChange { new_view: u64, node_id: NodeId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateUpdate {
    pub player_id: PeerId,
    pub action: GameAction,
    pub timestamp: u64,
    pub round: u64,
}

pub struct PbftConsensus {
    node_id: NodeId,
    view: u64,
    sequence_number: u64,
    replica_count: usize,
    f: usize, // maximum byzantine failures tolerated
    
    // Message logs
    prepare_log: HashMap<u64, HashMap<NodeId, [u8; 32]>>,
    commit_log: HashMap<u64, HashMap<NodeId, [u8; 32]>>,
    
    // Current consensus state
    pending_requests: HashMap<u64, GameStateUpdate>,
    committed_updates: Vec<GameStateUpdate>,
    
    // Network channels
    message_sender: mpsc::UnboundedSender<(NodeId, PbftMessage)>,
    message_receiver: mpsc::UnboundedReceiver<(NodeId, PbftMessage)>,
}

impl PbftConsensus {
    pub fn new(
        node_id: NodeId, 
        replica_count: usize,
        message_sender: mpsc::UnboundedSender<(NodeId, PbftMessage)>,
        message_receiver: mpsc::UnboundedReceiver<(NodeId, PbftMessage)>,
    ) -> Self {
        let f = (replica_count - 1) / 3; // Byzantine fault tolerance
        
        Self {
            node_id,
            view: 0,
            sequence_number: 0,
            replica_count,
            f,
            prepare_log: HashMap::new(),
            commit_log: HashMap::new(),
            pending_requests: HashMap::new(),
            committed_updates: Vec::new(),
            message_sender,
            message_receiver,
        }
    }

    /// Main consensus loop processing PBFT messages
    pub async fn run_consensus(&mut self) -> Result<()> {
        loop {
            if let Some((sender, message)) = self.message_receiver.recv().await {
                match message {
                    PbftMessage::Request(update) => {
                        self.handle_request(update).await?;
                    },
                    PbftMessage::PrePrepare { view, sequence, digest, update } => {
                        self.handle_pre_prepare(sender, view, sequence, digest, update).await?;
                    },
                    PbftMessage::Prepare { view, sequence, digest, node_id } => {
                        self.handle_prepare(view, sequence, digest, node_id).await?;
                    },
                    PbftMessage::Commit { view, sequence, digest, node_id } => {
                        self.handle_commit(view, sequence, digest, node_id).await?;
                    },
                    PbftMessage::ViewChange { new_view, node_id } => {
                        self.handle_view_change(new_view, node_id).await?;
                    },
                }
            }
        }
    }

    async fn handle_request(&mut self, update: GameStateUpdate) -> Result<()> {
        if self.is_primary() {
            // Primary node initiates consensus
            self.sequence_number += 1;
            let digest = self.compute_digest(&update);
            
            self.pending_requests.insert(self.sequence_number, update.clone());
            
            // Send pre-prepare to all replicas
            let pre_prepare = PbftMessage::PrePrepare {
                view: self.view,
                sequence: self.sequence_number,
                digest,
                update,
            };
            
            self.broadcast_message(pre_prepare).await?;
        } else {
            // Backup nodes forward request to primary
            let primary_id = self.get_primary_id();
            self.send_to_node(primary_id, PbftMessage::Request(update)).await?;
        }
        
        Ok(())
    }

    async fn handle_pre_prepare(
        &mut self,
        sender: NodeId,
        view: u64,
        sequence: u64,
        digest: [u8; 32],
        update: GameStateUpdate,
    ) -> Result<()> {
        // Validate pre-prepare message
        if view != self.view || sender != self.get_primary_id() {
            return Ok(()); // Invalid pre-prepare
        }
        
        if self.compute_digest(&update) != digest {
            return Ok(()); // Digest mismatch
        }
        
        // Accept pre-prepare and send prepare
        self.pending_requests.insert(sequence, update);
        
        let prepare = PbftMessage::Prepare {
            view,
            sequence,
            digest,
            node_id: self.node_id,
        };
        
        self.broadcast_message(prepare).await?;
        
        // Initialize prepare log entry
        self.prepare_log.entry(sequence).or_insert_with(HashMap::new);
        
        Ok(())
    }

    async fn handle_prepare(
        &mut self,
        view: u64,
        sequence: u64,
        digest: [u8; 32],
        node_id: NodeId,
    ) -> Result<()> {
        if view != self.view {
            return Ok(());
        }
        
        // Record prepare vote
        self.prepare_log
            .entry(sequence)
            .or_insert_with(HashMap::new)
            .insert(node_id, digest);
        
        // Check if we have enough prepare messages (2f)
        if let Some(prepares) = self.prepare_log.get(&sequence) {
            let matching_prepares = prepares.values()
                .filter(|&&d| d == digest)
                .count();
            
            if matching_prepares >= 2 * self.f {
                // Send commit message
                let commit = PbftMessage::Commit {
                    view,
                    sequence,
                    digest,
                    node_id: self.node_id,
                };
                
                self.broadcast_message(commit).await?;
                
                // Initialize commit log entry
                self.commit_log.entry(sequence).or_insert_with(HashMap::new);
            }
        }
        
        Ok(())
    }

    async fn handle_commit(
        &mut self,
        view: u64,
        sequence: u64,
        digest: [u8; 32],
        node_id: NodeId,
    ) -> Result<()> {
        if view != self.view {
            return Ok(());
        }
        
        // Record commit vote
        self.commit_log
            .entry(sequence)
            .or_insert_with(HashMap::new)
            .insert(node_id, digest);
        
        // Check if we have enough commit messages (2f+1)
        if let Some(commits) = self.commit_log.get(&sequence) {
            let matching_commits = commits.values()
                .filter(|&&d| d == digest)
                .count();
            
            if matching_commits >= 2 * self.f + 1 {
                // Execute the game state update
                if let Some(update) = self.pending_requests.remove(&sequence) {
                    self.apply_game_state_update(update).await?;
                }
            }
        }
        
        Ok(())
    }

    async fn handle_view_change(&mut self, new_view: u64, node_id: NodeId) -> Result<()> {
        if new_view > self.view {
            self.view = new_view;
            // Clear current consensus state and restart
            self.prepare_log.clear();
            self.commit_log.clear();
            tracing::info!("View changed to {} due to node {}", new_view, node_id);
        }
        Ok(())
    }

    async fn apply_game_state_update(&mut self, update: GameStateUpdate) -> Result<()> {
        // Validate update against game rules
        if !self.validate_game_action(&update) {
            return Err(anyhow::anyhow!("Invalid game action"));
        }
        
        // Apply update to local game state
        self.committed_updates.push(update.clone());
        
        tracing::info!("Applied game update: {:?}", update);
        Ok(())
    }

    fn validate_game_action(&self, update: &GameStateUpdate) -> bool {
        // Implement game-specific validation logic
        // Check if player can perform this action, validate timing, etc.
        true // Simplified for example
    }

    fn compute_digest(&self, update: &GameStateUpdate) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let serialized = bincode::serialize(update).unwrap();
        Sha256::digest(&serialized).into()
    }

    fn is_primary(&self) -> bool {
        self.node_id == self.get_primary_id()
    }

    fn get_primary_id(&self) -> NodeId {
        NodeId((self.view % self.replica_count as u64) as u32)
    }

    async fn broadcast_message(&self, message: PbftMessage) -> Result<()> {
        for i in 0..self.replica_count {
            let node_id = NodeId(i as u32);
            if node_id != self.node_id {
                self.send_to_node(node_id, message.clone()).await?;
            }
        }
        Ok(())
    }

    async fn send_to_node(&self, node_id: NodeId, message: PbftMessage) -> Result<()> {
        self.message_sender.send((node_id, message))
            .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;
        Ok(())
    }
}
```

## 5. Economic Security - Stake Slashing & Incentive Alignment

Implement economic incentives to discourage malicious behavior through stake-based penalties and rewards.

```rust
use std::collections::BTreeMap;
use rust_decimal::Decimal;

pub struct EconomicSecurity {
    stakes: HashMap<PeerId, StakeInfo>,
    slash_conditions: Vec<SlashCondition>,
    reward_pool: Decimal,
    min_stake_requirement: Decimal,
    slash_rate: Decimal,
}

#[derive(Debug, Clone)]
pub struct StakeInfo {
    pub amount: Decimal,
    pub locked_until: u64,
    pub slashing_history: Vec<SlashEvent>,
    pub last_reward_claim: u64,
    pub reputation_score: f64,
}

#[derive(Debug, Clone)]
pub struct SlashEvent {
    pub timestamp: u64,
    pub amount: Decimal,
    pub reason: SlashReason,
    pub evidence: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlashReason {
    DoubleVoting,
    InvalidStateTransition,
    MessageSpamming,
    EclipseAttackParticipation,
    CollusionDetected,
    ByzantineFailure,
}

#[derive(Debug, Clone)]
pub struct SlashCondition {
    pub condition_type: SlashReason,
    pub evidence_threshold: usize,
    pub slash_percentage: Decimal,
    pub cooldown_period: u64,
}

impl EconomicSecurity {
    pub fn new() -> Self {
        Self {
            stakes: HashMap::new(),
            slash_conditions: Self::default_slash_conditions(),
            reward_pool: Decimal::from(1000000), // 1M tokens initial pool
            min_stake_requirement: Decimal::from(1000),
            slash_rate: Decimal::from_str("0.1").unwrap(), // 10% base slash rate
        }
    }

    fn default_slash_conditions() -> Vec<SlashCondition> {
        vec![
            SlashCondition {
                condition_type: SlashReason::DoubleVoting,
                evidence_threshold: 1,
                slash_percentage: Decimal::from_str("0.5").unwrap(),
                cooldown_period: 86400, // 24 hours
            },
            SlashCondition {
                condition_type: SlashReason::CollusionDetected,
                evidence_threshold: 3,
                slash_percentage: Decimal::from_str("0.3").unwrap(),
                cooldown_period: 3600, // 1 hour
            },
            SlashCondition {
                condition_type: SlashReason::ByzantineFailure,
                evidence_threshold: 2,
                slash_percentage: Decimal::from_str("0.25").unwrap(),
                cooldown_period: 1800, // 30 minutes
            },
            SlashCondition {
                condition_type: SlashReason::MessageSpamming,
                evidence_threshold: 5,
                slash_percentage: Decimal::from_str("0.1").unwrap(),
                cooldown_period: 300, // 5 minutes
            },
        ]
    }

    /// Stake tokens to participate in the network
    pub fn stake_tokens(&mut self, peer_id: PeerId, amount: Decimal) -> Result<()> {
        if amount < self.min_stake_requirement {
            return Err(anyhow::anyhow!("Stake amount below minimum requirement"));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let stake_info = StakeInfo {
            amount,
            locked_until: now + 604800, // 7 days lock period
            slashing_history: Vec::new(),
            last_reward_claim: now,
            reputation_score: 1.0, // Start with neutral reputation
        };

        self.stakes.insert(peer_id, stake_info);
        tracing::info!("Peer {} staked {} tokens", peer_id, amount);
        Ok(())
    }

    /// Process slash condition and execute punishment if warranted
    pub async fn process_slash_condition(
        &mut self,
        peer_id: PeerId,
        reason: SlashReason,
        evidence: Vec<u8>,
    ) -> Result<Option<Decimal>> {
        let stake_info = match self.stakes.get_mut(&peer_id) {
            Some(info) => info,
            None => return Err(anyhow::anyhow!("Peer not staked")),
        };

        // Find matching slash condition
        let condition = self.slash_conditions
            .iter()
            .find(|c| std::mem::discriminant(&c.condition_type) == std::mem::discriminant(&reason))
            .ok_or_else(|| anyhow::anyhow!("Unknown slash condition"))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check cooldown period
        let recent_slashes = stake_info.slashing_history
            .iter()
            .filter(|event| {
                now - event.timestamp < condition.cooldown_period &&
                std::mem::discriminant(&event.reason) == std::mem::discriminant(&reason)
            })
            .count();

        if recent_slashes >= condition.evidence_threshold {
            // Execute slashing
            let slash_amount = stake_info.amount * condition.slash_percentage;
            stake_info.amount -= slash_amount;

            // Record slash event
            let slash_event = SlashEvent {
                timestamp: now,
                amount: slash_amount,
                reason: reason.clone(),
                evidence,
            };
            stake_info.slashing_history.push(slash_event);

            // Update reputation score
            stake_info.reputation_score *= 0.8; // Decrease reputation by 20%

            // Add slashed tokens to reward pool
            self.reward_pool += slash_amount;

            tracing::warn!(
                "Slashed {} tokens from peer {} for {:?}", 
                slash_amount, peer_id, reason
            );

            // Remove stake if amount falls below minimum
            if stake_info.amount < self.min_stake_requirement {
                self.stakes.remove(&peer_id);
                tracing::info!("Peer {} unstaked due to insufficient balance", peer_id);
            }

            Ok(Some(slash_amount))
        } else {
            Ok(None) // No slashing executed
        }
    }

    /// Calculate and distribute rewards to honest participants
    pub fn distribute_rewards(&mut self) -> Result<HashMap<PeerId, Decimal>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut rewards = HashMap::new();
        let eligible_peers: Vec<_> = self.stakes
            .iter()
            .filter(|(_, info)| {
                info.reputation_score > 0.5 && // Good reputation
                now - info.last_reward_claim > 3600 && // Min 1 hour between claims
                info.amount >= self.min_stake_requirement
            })
            .collect();

        if eligible_peers.is_empty() {
            return Ok(rewards);
        }

        // Calculate total weighted stake for distribution
        let total_weighted_stake: Decimal = eligible_peers
            .iter()
            .map(|(_, info)| info.amount * Decimal::from_f64_retain(info.reputation_score).unwrap_or(Decimal::ZERO))
            .sum();

        if total_weighted_stake.is_zero() {
            return Ok(rewards);
        }

        // Distribute rewards proportionally
        let available_rewards = self.reward_pool * Decimal::from_str("0.01").unwrap(); // 1% of pool per distribution
        
        for (peer_id, stake_info) in eligible_peers {
            let weighted_stake = stake_info.amount * Decimal::from_f64_retain(stake_info.reputation_score).unwrap_or(Decimal::ZERO);
            let reward = available_rewards * (weighted_stake / total_weighted_stake);
            
            rewards.insert(*peer_id, reward);
            
            // Update stake info
            if let Some(info) = self.stakes.get_mut(peer_id) {
                info.amount += reward;
                info.last_reward_claim = now;
                info.reputation_score = (info.reputation_score * 0.99 + 0.01).min(1.0); // Gradual reputation recovery
            }
        }

        self.reward_pool -= available_rewards;
        Ok(rewards)
    }

    /// Check if peer has sufficient stake for participation
    pub fn validate_participation(&self, peer_id: &PeerId) -> bool {
        if let Some(stake_info) = self.stakes.get(peer_id) {
            stake_info.amount >= self.min_stake_requirement && 
            stake_info.reputation_score > 0.3 // Minimum reputation threshold
        } else {
            false
        }
    }

    /// Calculate voting weight based on stake and reputation
    pub fn calculate_voting_weight(&self, peer_id: &PeerId) -> Decimal {
        if let Some(stake_info) = self.stakes.get(peer_id) {
            let base_weight = (stake_info.amount / self.min_stake_requirement).sqrt(); // Diminishing returns
            let reputation_modifier = Decimal::from_f64_retain(stake_info.reputation_score).unwrap_or(Decimal::ZERO);
            base_weight * reputation_modifier
        } else {
            Decimal::ZERO
        }
    }

    /// Implement gradual stake unlocking to prevent sudden exits during attacks
    pub fn request_unstake(&mut self, peer_id: PeerId, amount: Decimal) -> Result<u64> {
        let stake_info = self.stakes.get_mut(&peer_id)
            .ok_or_else(|| anyhow::anyhow!("Peer not staked"))?;

        if amount > stake_info.amount {
            return Err(anyhow::anyhow!("Insufficient staked amount"));
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Extend lock period based on recent slashing events
        let slash_penalty = stake_info.slashing_history
            .iter()
            .filter(|event| now - event.timestamp < 86400) // Last 24 hours
            .count() as u64 * 86400; // Add 1 day per recent slash

        let unlock_time = now + 604800 + slash_penalty; // Base 7 days + penalties
        stake_info.locked_until = unlock_time.max(stake_info.locked_until);

        Ok(unlock_time)
    }

    /// Get current stake statistics for monitoring
    pub fn get_stake_statistics(&self) -> HashMap<String, Decimal> {
        let mut stats = HashMap::new();
        
        let total_staked: Decimal = self.stakes.values()
            .map(|info| info.amount)
            .sum();
        
        let active_stakes = self.stakes.len();
        let avg_reputation: f64 = self.stakes.values()
            .map(|info| info.reputation_score)
            .sum::<f64>() / active_stakes as f64;
        
        stats.insert("total_staked".to_string(), total_staked);
        stats.insert("active_stakes".to_string(), Decimal::from(active_stakes));
        stats.insert("reward_pool".to_string(), self.reward_pool);
        stats.insert("avg_reputation".to_string(), Decimal::from_f64_retain(avg_reputation).unwrap_or(Decimal::ZERO));
        
        stats
    }
}
```

## Integration & Monitoring

These security components should be integrated with your main BitChat protocol:

```rust
pub struct SecureProtocolEngine {
    eclipse_mitigation: EclipseMitigation,
    sybil_prevention: SybilPreventionEngine,
    collusion_detector: CollusionDetector,
    pbft_consensus: PbftConsensus,
    economic_security: EconomicSecurity,
}

impl SecureProtocolEngine {
    /// Initialize all security components
    pub fn new(node_id: NodeId, replica_count: usize) -> Self {
        // Component initialization with cross-integration
        Self {
            eclipse_mitigation: EclipseMitigation::new(),
            sybil_prevention: SybilPreventionEngine::new(),
            collusion_detector: CollusionDetector::new(),
            pbft_consensus: PbftConsensus::new(node_id, replica_count, tx, rx),
            economic_security: EconomicSecurity::new(),
        }
    }

    /// Comprehensive security validation pipeline
    pub async fn validate_peer_action(&mut self, peer_id: PeerId, action: &PeerAction) -> Result<bool> {
        // 1. Check economic participation eligibility
        if !self.economic_security.validate_participation(&peer_id) {
            return Ok(false);
        }

        // 2. Update behavioral monitoring
        match action {
            PeerAction::SendMessage { timestamp, .. } => {
                let timing_score = self.collusion_detector.detect_timing_collusion(peer_id, *timestamp);
                if timing_score > 0.8 {
                    self.economic_security.process_slash_condition(
                        peer_id,
                        SlashReason::CollusionDetected,
                        vec![], // Add timing evidence
                    ).await?;
                }
            },
            PeerAction::Vote { choice, .. } => {
                // Analyze vote patterns for collusion
                let mut votes = HashMap::new();
                votes.insert(peer_id, *choice as u8);
                let suspicious_pairs = self.collusion_detector.analyze_vote_correlation(&votes);
                
                for (peer1, peer2, correlation) in suspicious_pairs {
                    if correlation > 0.9 {
                        self.economic_security.process_slash_condition(
                            peer1,
                            SlashReason::CollusionDetected,
                            bincode::serialize(&correlation).unwrap(),
                        ).await?;
                    }
                }
            },
            _ => {}
        }

        // 3. Verify through PBFT consensus for critical actions
        if action.is_critical() {
            // Route through Byzantine consensus
            let game_update = GameStateUpdate {
                player_id: peer_id,
                action: action.to_game_action(),
                timestamp: chrono::Utc::now().timestamp() as u64,
                round: self.pbft_consensus.sequence_number,
            };
            
            // This will go through full PBFT pipeline
            self.pbft_consensus.handle_request(game_update).await?;
        }

        Ok(true)
    }
}
```

This implementation provides comprehensive security coverage for BitChat with practical Rust code that can be directly integrated into your existing protocol. Each component is designed to work independently while providing synergistic protection when combined.