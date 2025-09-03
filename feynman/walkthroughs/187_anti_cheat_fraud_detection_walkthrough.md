# Chapter 75: Anti-Cheat & Fraud Detection

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: Guardians of Fair Play

Imagine running a casino where some players have X-ray vision, can manipulate dice rolls, or create counterfeit chips. You need sophisticated systems to detect and prevent cheating while maintaining a smooth experience for honest players. This is the challenge of anti-cheat and fraud detection in distributed gaming systems.

In BitCraps, anti-cheat isn't just about catching cheaters—it's about maintaining trust in a trustless environment where every peer could potentially be malicious.

## The Fundamentals: Understanding Distributed Cheating

### Types of Cheating in P2P Gaming

In a distributed casino, cheating can take many forms:

1. **Time Manipulation**: Exploiting network delays to change bets after seeing results
2. **State Manipulation**: Modifying local game state to gain unfair advantages
3. **Collusion**: Multiple players working together to defraud the system
4. **Statistical Manipulation**: Biasing random number generation
5. **Consensus Attacks**: Attempting to influence consensus decisions
6. **Economic Attacks**: Exploiting tokenomics for unfair gain

```rust
// From src/protocol/p2p_messages.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheatType {
    /// Invalid dice values outside 1-6 range
    InvalidDiceValue,
    
    /// Betting more than available balance
    OverBetting,
    
    /// Too many operations too quickly
    RateLimitViolation,
    
    /// Operations with invalid timestamps
    TimeManipulation,
    
    /// Suspicious win patterns
    StatisticalAnomaly,
    
    /// Multiple identities from same source
    SybilAttack,
    
    /// Invalid cryptographic signatures
    SignatureForgery,
    
    /// Attempting to double-spend tokens
    DoubleSpending,
    
    /// Manipulating consensus proposals
    ConsensusManipulation,
    
    /// Players working together
    Collusion,
}
```

## Deep Dive: BitCraps Anti-Cheat Implementation

### The Anti-Cheat Configuration

BitCraps uses a comprehensive configuration system to define cheating thresholds:

```rust
// From src/protocol/anti_cheat.rs
pub struct AntiCheatConfig {
    /// Maximum allowed time skew for operations (30 seconds)
    pub max_time_skew: Duration,
    
    /// Minimum time between operations from same peer (100ms)
    pub min_operation_interval: Duration,
    
    /// Maximum bet amount relative to balance (100%)
    pub max_bet_ratio: f64,
    
    /// Suspicious behavior threshold (3 strikes)
    pub suspicion_threshold: u32,
    
    /// Evidence retention period (1 hour)
    pub evidence_retention: Duration,
    
    /// Dice value bounds (1-6)
    pub min_dice_value: u8,
    pub max_dice_value: u8,
    
    /// Statistical anomaly threshold (0.1% probability)
    pub anomaly_threshold: f64,
}
```

This configuration creates a framework where:
- Players can't manipulate timestamps beyond 30 seconds
- Rate limiting prevents spam attacks (minimum 100ms between actions)
- Bets are capped at available balance
- Three suspicious activities trigger investigation
- Evidence is retained for one hour for verification

### Evidence Collection System

When cheating is detected, BitCraps collects cryptographically verifiable evidence:

```rust
// From src/protocol/anti_cheat.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheatEvidence {
    /// Unique evidence identifier
    pub evidence_id: [u8; 32],
    
    /// The suspected cheater's peer ID
    pub suspect: PeerId,
    
    /// Type of cheat detected
    pub cheat_type: CheatType,
    
    /// Raw evidence data (serialized proof)
    pub evidence_data: Vec<u8>,
    
    /// Unix timestamp when detected
    pub detected_at: u64,
    
    /// Other peers who witnessed the cheat
    pub witnesses: Vec<PeerId>,
    
    /// Severity score (0.0 to 1.0)
    pub severity: f64,
    
    /// Related consensus operation ID
    pub related_operation: Option<ProposalId>,
}

impl CheatEvidence {
    pub fn new(suspect: PeerId, cheat_type: CheatType, data: Vec<u8>) -> Self {
        let mut evidence_id = [0u8; 32];
        let mut hasher = blake3::Hasher::new();
        hasher.update(&suspect);
        hasher.update(&bincode::serialize(&cheat_type).unwrap());
        hasher.update(&data);
        evidence_id.copy_from_slice(hasher.finalize().as_bytes());
        
        Self {
            evidence_id,
            suspect,
            cheat_type,
            evidence_data: data,
            detected_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            witnesses: Vec::new(),
            severity: Self::calculate_severity(&cheat_type),
            related_operation: None,
        }
    }
    
    fn calculate_severity(cheat_type: &CheatType) -> f64 {
        match cheat_type {
            CheatType::SignatureForgery => 1.0,  // Critical
            CheatType::DoubleSpending => 1.0,    // Critical
            CheatType::ConsensusManipulation => 0.9,
            CheatType::Collusion => 0.8,
            CheatType::SybilAttack => 0.7,
            CheatType::StatisticalAnomaly => 0.6,
            CheatType::TimeManipulation => 0.5,
            CheatType::OverBetting => 0.4,
            CheatType::InvalidDiceValue => 0.3,
            CheatType::RateLimitViolation => 0.2,
        }
    }
}
```

## Statistical Anomaly Detection

### Chi-Square Test for Randomness

BitCraps uses statistical analysis to detect biased dice rolls:

```rust
// From src/protocol/anti_cheat.rs
pub struct RandomnessValidator {
    dice_outcomes: HashMap<u8, u64>,
    total_rolls: u64,
    
    pub fn validate_randomness(&self) -> Result<bool> {
        if self.total_rolls < 100 {
            return Ok(true); // Not enough data
        }
        
        let expected_frequency = self.total_rolls as f64 / 6.0;
        let mut chi_square = 0.0;
        
        for face in 1..=6 {
            let observed = *self.dice_outcomes.get(&face).unwrap_or(&0) as f64;
            let diff = observed - expected_frequency;
            chi_square += (diff * diff) / expected_frequency;
        }
        
        // Degrees of freedom = 6 - 1 = 5
        // Critical value at 0.001 significance = 20.515
        const CRITICAL_VALUE: f64 = 20.515;
        
        if chi_square > CRITICAL_VALUE {
            // Probability of this distribution < 0.1%
            return Ok(false); // Likely biased
        }
        
        Ok(true) // Appears random
    }
}

// Example: Detecting biased dice
fn detect_biased_dice(rolls: &[DiceRoll]) -> bool {
    let mut validator = RandomnessValidator::new();
    
    for roll in rolls {
        validator.record_roll(roll.die1);
        validator.record_roll(roll.die2);
    }
    
    !validator.validate_randomness().unwrap_or(true)
}
```

## Time-Based Attack Prevention

### Detecting Time Manipulation

```rust
// From src/protocol/anti_cheat.rs
impl AntiCheatEngine {
    pub fn validate_operation_timing(
        &self,
        operation: &GameOperation,
        sender: &PeerId,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        
        // Check for future timestamps
        if operation.timestamp > now {
            let skew = operation.timestamp - now;
            if skew > self.config.max_time_skew.as_secs() {
                return Err(Error::TimeManipulation {
                    peer: *sender,
                    skew_seconds: skew,
                });
            }
        }
        
        // Check for old timestamps (replay attacks)
        if now > operation.timestamp {
            let age = now - operation.timestamp;
            if age > self.config.max_time_skew.as_secs() {
                return Err(Error::StaleOperation {
                    peer: *sender,
                    age_seconds: age,
                });
            }
        }
        
        // Check rate limiting
        if let Some(profile) = self.peer_profiles.get(sender) {
            let elapsed = profile.last_operation_time.elapsed();
            if elapsed < self.config.min_operation_interval {
                return Err(Error::RateLimitExceeded {
                    peer: *sender,
                    interval: elapsed,
                });
            }
        }
        
        Ok(())
    }
}
```

## Behavioral Pattern Analysis

### Building Peer Behavior Profiles

```rust
// From src/protocol/anti_cheat.rs
#[derive(Debug, Clone)]
pub struct PeerBehaviorProfile {
    peer_id: PeerId,
    operations_count: u64,
    last_operation_time: Instant,
    total_bets_placed: CrapTokens,
    total_winnings: CrapTokens,
    dice_rolls_witnessed: Vec<DiceRoll>,
    suspicious_activities: Vec<CheatType>,
    trust_score: f64, // 0.0 to 1.0
    statistical_anomalies: u32,
}

impl PeerBehaviorProfile {
    pub fn calculate_trust_score(&self) -> f64 {
        let mut score = 1.0;
        
        // Reduce score for each suspicious activity
        for _ in &self.suspicious_activities {
            score *= 0.8;
        }
        
        // Reduce score for statistical anomalies
        score *= 0.95_f64.powi(self.statistical_anomalies as i32);
        
        // Consider win/loss ratio
        if self.total_bets_placed.0 > 0 {
            let win_ratio = self.total_winnings.0 as f64 / 
                          self.total_bets_placed.0 as f64;
            
            // House edge in craps is ~1.4%
            // Winning significantly more is suspicious
            if win_ratio > 1.1 {
                score *= 0.7;
            } else if win_ratio > 1.05 {
                score *= 0.85;
            }
        }
        
        score.max(0.0).min(1.0)
    }
    
    pub fn is_suspicious(&self) -> bool {
        self.trust_score < 0.5 || 
        self.suspicious_activities.len() >= 3
    }
}
```

## Consensus-Based Cheat Detection

### Distributed Verification

```rust
// From src/protocol/anti_cheat.rs
pub struct ConsensusAntiCheat {
    /// Evidence awaiting consensus
    pending_evidence: Arc<RwLock<HashMap<[u8; 32], CheatEvidence>>>,
    
    /// Confirmed cheats
    confirmed_cheats: Arc<RwLock<HashMap<PeerId, Vec<CheatEvidence>>>>,
    
    /// Reputation scores
    reputation_scores: Arc<RwLock<HashMap<PeerId, f64>>>,
}

impl ConsensusAntiCheat {
    pub async fn report_cheat(
        &self,
        evidence: CheatEvidence,
        reporter: PeerId,
    ) -> Result<()> {
        // Add reporter as witness
        let mut evidence = evidence;
        evidence.witnesses.push(reporter);
        
        // Store pending evidence
        self.pending_evidence.write().await
            .insert(evidence.evidence_id, evidence.clone());
        
        // Broadcast to network for consensus
        self.broadcast_cheat_report(evidence).await?;
        
        Ok(())
    }
    
    pub async fn vote_on_evidence(
        &self,
        evidence_id: [u8; 32],
        voter: PeerId,
        verdict: bool,
    ) -> Result<()> {
        let mut pending = self.pending_evidence.write().await;
        
        if let Some(evidence) = pending.get_mut(&evidence_id) {
            if verdict {
                evidence.witnesses.push(voter);
                
                // Check if we have consensus (>2/3 of active peers)
                let active_peers = self.get_active_peer_count().await;
                let required_witnesses = (active_peers * 2) / 3;
                
                if evidence.witnesses.len() >= required_witnesses {
                    // Consensus reached - confirm the cheat
                    self.confirm_cheat(evidence.clone()).await?;
                    pending.remove(&evidence_id);
                }
            }
        }
        
        Ok(())
    }
    
    async fn confirm_cheat(&self, evidence: CheatEvidence) -> Result<()> {
        // Update confirmed cheats
        self.confirmed_cheats.write().await
            .entry(evidence.suspect)
            .or_insert_with(Vec::new)
            .push(evidence.clone());
        
        // Update reputation
        let mut reputation = self.reputation_scores.write().await;
        let current = reputation.entry(evidence.suspect)
            .or_insert(1.0);
        
        // Reduce reputation based on severity
        *current *= 1.0 - evidence.severity;
        
        // Ban if reputation too low
        if *current < 0.1 {
            self.ban_peer(evidence.suspect).await?;
        }
        
        Ok(())
    }
}
```

## Real-World Anti-Cheat Patterns

### Martingale Detection

The Martingale betting system (doubling bets after losses) can be detected:

```rust
pub fn detect_martingale(bets: &[Bet]) -> bool {
    if bets.len() < 3 {
        return false;
    }
    
    let mut martingale_pattern = 0;
    
    for window in bets.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        
        // Check if current bet is ~2x previous after a loss
        if prev.outcome == Some(BetOutcome::Lost) {
            let ratio = curr.amount.0 as f64 / prev.amount.0 as f64;
            if (ratio - 2.0).abs() < 0.1 {
                martingale_pattern += 1;
            }
        }
    }
    
    // Suspicious if pattern appears frequently
    martingale_pattern >= 3
}
```

### Collusion Detection

Detecting players working together:

```rust
pub fn detect_collusion(
    game_sessions: &[GameSession],
    players: &[PeerId],
) -> Vec<(PeerId, PeerId)> {
    let mut suspicious_pairs = Vec::new();
    
    for i in 0..players.len() {
        for j in i+1..players.len() {
            let p1 = players[i];
            let p2 = players[j];
            
            // Count games where both players participated
            let shared_games = game_sessions.iter()
                .filter(|s| s.has_player(p1) && s.has_player(p2))
                .count();
            
            // Check win/loss patterns between them
            let mut p1_wins_from_p2 = 0;
            let mut p2_wins_from_p1 = 0;
            
            for session in game_sessions {
                if session.has_player(p1) && session.has_player(p2) {
                    // Check if one consistently loses to the other
                    if let Some(winner) = session.winner {
                        if winner == p1 && session.has_loser(p2) {
                            p1_wins_from_p2 += 1;
                        } else if winner == p2 && session.has_loser(p1) {
                            p2_wins_from_p1 += 1;
                        }
                    }
                }
            }
            
            // Suspicious if one player consistently loses to another
            let total_shared = p1_wins_from_p2 + p2_wins_from_p1;
            if total_shared > 5 {
                let win_ratio = p1_wins_from_p2.max(p2_wins_from_p1) as f64 / 
                              total_shared as f64;
                
                if win_ratio > 0.8 {  // 80% wins is suspicious
                    suspicious_pairs.push((p1, p2));
                }
            }
        }
    }
    
    suspicious_pairs
}
```

## Testing Anti-Cheat Systems

### Comprehensive Test Suite

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_biased_dice_detection() {
        let mut validator = RandomnessValidator::new();
        
        // Simulate heavily biased dice (always rolling 6)
        for _ in 0..1000 {
            validator.record_roll(6);
        }
        
        assert!(!validator.validate_randomness().unwrap());
    }
    
    #[test]
    fn test_time_manipulation_detection() {
        let config = AntiCheatConfig::default();
        let engine = AntiCheatEngine::new(config);
        
        // Create operation with future timestamp
        let future_op = GameOperation {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() + 60, // 1 minute in future
            ..Default::default()
        };
        
        assert!(engine.validate_operation_timing(
            &future_op, 
            &PeerId::random()
        ).is_err());
    }
    
    #[tokio::test]
    async fn test_consensus_based_banning() {
        let anti_cheat = ConsensusAntiCheat::new();
        
        // Create evidence of cheating
        let cheater = PeerId::random();
        let evidence = CheatEvidence::new(
            cheater,
            CheatType::DoubleSpending,
            vec![1, 2, 3], // Proof data
        );
        
        // Multiple peers report the same cheat
        for i in 0..10 {
            anti_cheat.vote_on_evidence(
                evidence.evidence_id,
                PeerId::random(),
                true, // Agree it's cheating
            ).await.unwrap();
        }
        
        // Check that cheater was banned
        let reputation = anti_cheat.reputation_scores
            .read().await
            .get(&cheater)
            .copied()
            .unwrap_or(1.0);
        
        assert!(reputation < 0.1); // Should be banned
    }
}
```

## Production Deployment Considerations

### Performance Impact

Anti-cheat systems must balance security with performance:

```rust
pub struct OptimizedAntiCheat {
    /// Sampling rate for expensive checks
    sampling_rate: f64,
    
    /// Async validation queue
    validation_queue: Arc<SegQueue<ValidationTask>>,
    
    /// Background validator
    validator_handle: JoinHandle<()>,
}

impl OptimizedAntiCheat {
    pub async fn validate_operation(&self, op: GameOperation) -> Result<()> {
        // Always do cheap checks
        self.validate_basic(&op)?;
        
        // Sample expensive checks
        if rand::random::<f64>() < self.sampling_rate {
            // Queue for async validation
            self.validation_queue.push(ValidationTask {
                operation: op,
                timestamp: Instant::now(),
            });
        }
        
        Ok(())
    }
}
```

### Privacy Considerations

Balance cheat detection with player privacy:

```rust
pub struct PrivacyPreservingAntiCheat {
    /// Zero-knowledge proofs for validation
    zk_verifier: ZkVerifier,
    
    /// Homomorphic encryption for statistics
    he_processor: HomomorphicProcessor,
}

impl PrivacyPreservingAntiCheat {
    pub fn verify_bet_validity(&self, proof: ZkProof) -> bool {
        // Verify bet is valid without seeing amount
        self.zk_verifier.verify_range_proof(proof)
    }
    
    pub fn update_statistics(&self, encrypted_data: EncryptedStats) {
        // Update stats without decrypting individual data
        self.he_processor.aggregate(encrypted_data);
    }
}
```

## Conclusion

Anti-cheat and fraud detection in BitCraps represents a sophisticated blend of statistical analysis, behavioral monitoring, and consensus-based verification. The system demonstrates how distributed networks can maintain fairness without central authority.

Key takeaways from the implementation:

1. **Multi-layered detection**: Combining time validation, statistical analysis, and behavioral patterns
2. **Evidence-based approach**: Cryptographically verifiable proof of cheating
3. **Consensus verification**: Using Byzantine fault tolerance for cheat confirmation
4. **Reputation systems**: Long-term trust tracking with gradual decay
5. **Privacy preservation**: Detecting cheats without compromising player privacy
6. **Performance optimization**: Sampling and async validation for efficiency

Remember: In distributed gaming, the goal isn't to make cheating impossible—it's to make it unprofitable. By requiring consensus for penalties and maintaining reputation scores, BitCraps ensures that the cost of cheating exceeds any potential gain.
