# Chapter 77: Reputation Systems

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: Trust Without Central Authority

Imagine a marketplace where everyone is anonymous, yet you need to know who to trust. How do you build reputation when identities can be created and discarded at will? This is the challenge of decentralized reputation systems.

## The Fundamentals: Reputation Mechanics

Effective reputation systems must:
- Resist Sybil attacks (fake identities)
- Prevent reputation manipulation
- Decay old behavior appropriately
- Incentivize good behavior
- Punish bad behavior proportionally

## Deep Dive: Multi-Factor Reputation

### Comprehensive Reputation Scoring

```rust
pub struct ReputationSystem {
    /// Reputation scores
    scores: Arc<RwLock<HashMap<NodeId, ReputationScore>>>,
    
    /// Behavior tracker
    behavior_tracker: BehaviorTracker,
    
    /// Decay calculator
    decay: ReputationDecay,
    
    /// Trust graph
    trust_graph: TrustGraph,
}

pub struct ReputationScore {
    /// Base reputation from proof of work
    base_score: f64,
    
    /// Behavior-based score
    behavior_score: f64,
    
    /// Peer endorsements
    endorsement_score: f64,
    
    /// Time-based factors
    age_factor: f64,
    
    /// Recent activity
    activity_score: f64,
    
    /// Computed total
    total: f64,
}

impl ReputationSystem {
    pub fn calculate_reputation(&self, node: &NodeId) -> f64 {
        let score = self.scores.read().unwrap()
            .get(node)
            .cloned()
            .unwrap_or_default();
        
        // Weighted combination of factors
        let reputation = 
            score.base_score * 0.2 +
            score.behavior_score * 0.3 +
            score.endorsement_score * 0.2 +
            score.age_factor * 0.1 +
            score.activity_score * 0.2;
        
        // Apply bounds
        reputation.max(0.0).min(1.0)
    }
    
    pub fn update_behavior(&mut self, node: &NodeId, action: &Action) {
        let delta = match action {
            Action::SuccessfulTransaction => 0.01,
            Action::ReportedCheating => -0.1,
            Action::ValidatedBlock => 0.02,
            Action::InvalidProposal => -0.05,
            Action::HelpedNewcomer => 0.03,
        };
        
        self.apply_reputation_change(node, delta);
    }
}
```

## Trust Graphs

### Web of Trust Implementation

```rust
pub struct TrustGraph {
    /// Directed edges with trust weights
    edges: HashMap<NodeId, HashMap<NodeId, TrustEdge>>,
    
    /// Trust propagation algorithm
    propagator: TrustPropagator,
}

pub struct TrustEdge {
    direct_trust: f64,
    interactions: u64,
    last_interaction: SystemTime,
}

impl TrustGraph {
    pub fn calculate_transitive_trust(&self, from: &NodeId, to: &NodeId) -> f64 {
        // Direct trust if exists
        if let Some(direct) = self.get_direct_trust(from, to) {
            return direct;
        }
        
        // Find trust paths
        let paths = self.find_trust_paths(from, to, 3); // Max 3 hops
        
        if paths.is_empty() {
            return 0.0;
        }
        
        // Calculate trust through paths
        let mut total_trust = 0.0;
        let mut total_weight = 0.0;
        
        for path in paths {
            let path_trust = self.calculate_path_trust(&path);
            let path_weight = 1.0 / path.len() as f64; // Shorter paths weighted higher
            
            total_trust += path_trust * path_weight;
            total_weight += path_weight;
        }
        
        total_trust / total_weight
    }
}
```

## Sybil Resistance

### Proof-of-Work Based Identity

```rust
pub struct SybilResistantReputation {
    /// Minimum proof-of-work for identity
    pow_threshold: u64,
    
    /// Identity verifier
    verifier: IdentityVerifier,
    
    /// Stake requirements
    stake_requirements: StakeRequirements,
}

impl SybilResistantReputation {
    pub fn verify_identity(&self, identity: &Identity) -> Result<bool> {
        // Verify proof-of-work
        if !self.verifier.verify_pow(identity)? {
            return Ok(false);
        }
        
        // Check stake if required
        if let Some(min_stake) = self.stake_requirements.minimum {
            if identity.staked_amount < min_stake {
                return Ok(false);
            }
        }
        
        // Check age requirement
        let age = SystemTime::now()
            .duration_since(identity.created_at)?;
        
        if age < Duration::from_secs(86400) { // 24 hours minimum
            return Ok(false);
        }
        
        Ok(true)
    }
    
    pub fn cost_of_sybil_attack(&self, num_identities: usize) -> Cost {
        let pow_cost = self.pow_threshold * num_identities as u64;
        let stake_cost = self.stake_requirements.minimum.unwrap_or(0) * num_identities as u64;
        let time_cost = Duration::from_secs(86400); // Can't parallelize time
        
        Cost {
            computational: pow_cost,
            financial: stake_cost,
            temporal: time_cost,
        }
    }
}
```

## Reputation Decay

### Time-Based Reputation Adjustment

```rust
pub struct ReputationDecay {
    /// Half-life of reputation
    half_life: Duration,
    
    /// Activity requirements
    activity_threshold: ActivityLevel,
}

impl ReputationDecay {
    pub fn apply_decay(&self, score: &mut ReputationScore, last_active: SystemTime) {
        let inactive_duration = SystemTime::now()
            .duration_since(last_active)
            .unwrap_or(Duration::ZERO);
        
        // Exponential decay
        let decay_periods = inactive_duration.as_secs() as f64 / 
                           self.half_life.as_secs() as f64;
        
        let decay_factor = 0.5_f64.powf(decay_periods);
        
        // Apply decay to behavior score
        score.behavior_score *= decay_factor;
        
        // Reduce activity score more aggressively
        score.activity_score *= decay_factor * 0.5;
        
        // Age factor doesn't decay
    }
}
```

## Conclusion

Reputation systems create trust in trustless environments. Through multi-factor scoring, trust graphs, and Sybil resistance, we can build robust reputation that incentivizes good behavior.

Key takeaways:
1. **Multi-factor reputation** prevents gaming single metrics
2. **Trust graphs** enable transitive trust
3. **Sybil resistance** makes fake identities expensive
4. **Reputation decay** ensures current behavior matters most
5. **Proof-of-work identity** adds cost to reputation attacks

Remember: Reputation is earned slowly and lost quickly—just like in real life.
