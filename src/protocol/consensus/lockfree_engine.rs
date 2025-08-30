//! Lock-free consensus engine for high-performance game state management

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use crossbeam_epoch::{self as epoch, Atomic, Owned};
use rustc_hash::FxHashMap;
use serde::{Serialize, Deserialize};

use crate::protocol::{PeerId, GameId, Hash256, Signature};
use crate::protocol::craps::CrapTokens;
use crate::error::Result;

use super::{ProposalId, StateHash};
use super::GameConsensusState;
use super::GameProposal;
use super::GameOperation;

/// Lock-free consensus metrics
#[derive(Debug, Default)]
pub struct LockFreeMetrics {
    pub state_transitions: AtomicU64,
    pub successful_cas: AtomicU64,
    pub failed_cas: AtomicU64,
    pub consensus_latency_ns: AtomicU64,
}

/// Immutable state snapshot for lock-free operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub state: GameConsensusState,
    pub version: u64,
    pub timestamp: u64,
}

/// Lock-free consensus engine using atomic operations
pub struct LockFreeConsensusEngine {
    /// Current state using atomic pointer for lock-free updates
    current_state: Atomic<StateSnapshot>,
    
    /// Version counter for optimistic concurrency control
    version_counter: AtomicU64,
    
    /// Game ID
    game_id: GameId,
    
    /// Local peer ID
    local_peer_id: PeerId,
    
    /// Pending proposals (using crossbeam's lock-free map would be better)
    pending_proposals: Arc<parking_lot::RwLock<FxHashMap<ProposalId, GameProposal>>>,
    
    /// Consensus participants
    _participants: Vec<PeerId>,
    
    /// Performance metrics
    metrics: Arc<LockFreeMetrics>,
    
    /// Engine active flag
    active: AtomicBool,
}

impl LockFreeConsensusEngine {
    /// Create new lock-free consensus engine
    pub fn new(
        game_id: GameId,
        local_peer_id: PeerId,
        participants: Vec<PeerId>,
        initial_state: GameConsensusState,
    ) -> Self {
        let initial_snapshot = StateSnapshot {
            state: initial_state,
            version: 0,
            timestamp: current_timestamp(),
        };
        
        Self {
            current_state: Atomic::new(initial_snapshot),
            version_counter: AtomicU64::new(1),
            game_id,
            local_peer_id,
            pending_proposals: Arc::new(parking_lot::RwLock::new(FxHashMap::default())),
            _participants: participants,
            metrics: Arc::new(LockFreeMetrics::default()),
            active: AtomicBool::new(true),
        }
    }
    
    /// Apply operation using lock-free compare-and-swap
    pub fn apply_operation(&self, operation: &GameOperation) -> Result<StateSnapshot> {
        let guard = &epoch::pin();
        let start_time = std::time::Instant::now();
        
        loop {
            // Load current state
            let current_shared = self.current_state.load(Ordering::Acquire, guard);
            
            // SAFETY: Use crossbeam epoch guard to safely dereference
            // The epoch-based protection ensures the memory remains valid
            let current = match unsafe { current_shared.as_ref() } {
                Some(state) => state,
                None => {
                    return Err(crate::error::Error::InvalidState("Null state pointer".to_string()));
                }
            };
            
            // Create new state based on current
            let mut new_state = current.state.clone();
            
            // Apply operation
            self.apply_operation_to_state(&mut new_state, operation)?;
            
            // Create new snapshot with incremented version
            let new_version = self.version_counter.fetch_add(1, Ordering::SeqCst);
            let new_snapshot = StateSnapshot {
                state: new_state,
                version: new_version,
                timestamp: current_timestamp(),
            };
            
            // Attempt compare-and-swap
            let new_owned = Owned::new(new_snapshot.clone());
            
            match self.current_state.compare_exchange(
                current_shared,
                new_owned,
                Ordering::Release,
                Ordering::Acquire,
                guard,
            ) {
                Ok(_) => {
                    // Success! Update metrics
                    self.metrics.successful_cas.fetch_add(1, Ordering::Relaxed);
                    self.metrics.state_transitions.fetch_add(1, Ordering::Relaxed);
                    
                    let latency = start_time.elapsed().as_nanos() as u64;
                    self.metrics.consensus_latency_ns.store(latency, Ordering::Relaxed);
                    
                    // SAFETY: Defer cleanup of old state using crossbeam epoch
                    // current_shared was obtained from atomic load and is valid
                    // Epoch-based deferred destruction ensures no use-after-free
                    unsafe {
                        guard.defer_destroy(current_shared);
                    }
                    
                    return Ok(new_snapshot);
                }
                Err(_) => {
                    // CAS failed, another thread updated state
                    // Retry with new state
                    self.metrics.failed_cas.fetch_add(1, Ordering::Relaxed);
                    
                    // Add small backoff to reduce contention
                    std::hint::spin_loop();
                }
            }
        }
    }
    
    /// Apply operation to state (pure function)
    fn apply_operation_to_state(&self, state: &mut GameConsensusState, operation: &GameOperation) -> Result<()> {
        state.sequence_number += 1;
        state.timestamp = current_timestamp();
        
        match operation {
            GameOperation::PlaceBet { player, bet, .. } => {
                // Apply bet
                if let Some(balance) = state.player_balances.get_mut(player) {
                    if balance.0 >= bet.amount.0 {
                        *balance = CrapTokens::new_unchecked(balance.0 - bet.amount.0);
                    } else {
                        return Err(crate::error::Error::InsufficientBalance);
                    }
                }
            }
            GameOperation::ProcessRoll { dice_roll, .. } => {
                // Process dice roll
                let _resolutions = state.game_state.process_roll(*dice_roll);
            }
            GameOperation::UpdateBalances { changes, .. } => {
                // Update balances
                for (player, change) in changes {
                    if let Some(balance) = state.player_balances.get_mut(player) {
                        if let Some(new_balance) = balance.checked_add(*change) {
                            *balance = new_balance;
                        }
                    }
                }
            }
            _ => {
                // Handle other operations
            }
        }
        
        // Recalculate state hash
        state.state_hash = self.calculate_state_hash(state)?;
        
        Ok(())
    }
    
    /// Get current state snapshot (lock-free read)
    pub fn get_current_state(&self) -> Result<StateSnapshot> {
        let guard = &epoch::pin();
        let current = self.current_state.load(Ordering::Acquire, guard);
        
        // SAFETY: Use crossbeam epoch guard to safely dereference
        let snapshot = match unsafe { current.as_ref() } {
            Some(state) => state,
            None => {
                return Err(crate::error::Error::InvalidState("Null state pointer".to_string()));
            }
        };
        Ok(snapshot.clone())
    }
    
    /// Propose new operation (still uses minimal locking for proposal storage)
    pub fn propose_operation(&self, operation: GameOperation) -> Result<ProposalId> {
        // Generate proposal ID
        let proposal_id = self.generate_proposal_id(&operation);
        
        // Get current state
        let current_state = self.get_current_state()?;
        
        // Apply operation to get proposed state
        let mut proposed_state = current_state.state.clone();
        self.apply_operation_to_state(&mut proposed_state, &operation)?;
        
        // Create proposal
        let proposal = GameProposal {
            id: proposal_id,
            proposer: self.local_peer_id,
            previous_state_hash: current_state.state.state_hash,
            proposed_state,
            operation,
            timestamp: current_timestamp(),
            signature: Signature([0u8; 64]),  // Would use real signature
        };
        
        // Store proposal (minimal locking)
        {
            let mut proposals = self.pending_proposals.write();
            proposals.insert(proposal_id, proposal);
        }
        
        Ok(proposal_id)
    }
    
    /// Check if a state transition is valid (lock-free)
    pub fn validate_transition(&self, from_state: &StateHash, _to_state: &StateHash) -> bool {
        let guard = &epoch::pin();
        let current = self.current_state.load(Ordering::Acquire, guard);
        
        // SAFETY: Use crossbeam epoch guard to safely dereference
        let snapshot = match unsafe { current.as_ref() } {
            Some(state) => state,
            None => return false,
        };
        
        // Simple validation: current state must match from_state
        snapshot.state.state_hash == *from_state
    }
    
    /// Optimistic update with validation
    pub fn optimistic_update<F>(&self, update_fn: F) -> Result<StateSnapshot>
    where
        F: Fn(&GameConsensusState) -> Result<GameConsensusState>,
    {
        let guard = &epoch::pin();
        let max_retries = 10;
        
        for _ in 0..max_retries {
            // Load current state
            let current_shared = self.current_state.load(Ordering::Acquire, guard);
            
            // SAFETY: Use crossbeam epoch guard to safely dereference
            let current = match unsafe { current_shared.as_ref() } {
                Some(state) => state,
                None => {
                    return Err(crate::error::Error::InvalidState("Null state pointer".to_string()));
                }
            };
            
            // Apply update function
            let new_state = update_fn(&current.state)?;
            
            // Create new snapshot
            let new_version = self.version_counter.fetch_add(1, Ordering::SeqCst);
            let new_snapshot = StateSnapshot {
                state: new_state,
                version: new_version,
                timestamp: current_timestamp(),
            };
            
            // Try CAS
            let new_owned = Owned::new(new_snapshot.clone());
            
            if self.current_state.compare_exchange(
                current_shared,
                new_owned,
                Ordering::Release,
                Ordering::Acquire,
                guard,
            ).is_ok() {
                // Success
                self.metrics.successful_cas.fetch_add(1, Ordering::Relaxed);
                
                // SAFETY: Defer cleanup using crossbeam epoch
                // current_shared was loaded atomically and is valid for cleanup
                unsafe {
                    guard.defer_destroy(current_shared);
                }
                
                return Ok(new_snapshot);
            }
            
            // Failed, retry
            self.metrics.failed_cas.fetch_add(1, Ordering::Relaxed);
            std::thread::yield_now();
        }
        
        Err(crate::error::Error::Protocol("Failed to update state after max retries".to_string()))
    }
    
    /// Get metrics
    pub fn get_metrics(&self) -> LockFreeMetrics {
        LockFreeMetrics {
            state_transitions: AtomicU64::new(self.metrics.state_transitions.load(Ordering::Relaxed)),
            successful_cas: AtomicU64::new(self.metrics.successful_cas.load(Ordering::Relaxed)),
            failed_cas: AtomicU64::new(self.metrics.failed_cas.load(Ordering::Relaxed)),
            consensus_latency_ns: AtomicU64::new(self.metrics.consensus_latency_ns.load(Ordering::Relaxed)),
        }
    }
    
    /// Calculate state hash
    fn calculate_state_hash(&self, state: &GameConsensusState) -> Result<Hash256> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(state.game_id);
        hasher.update(state.sequence_number.to_le_bytes());
        hasher.update(state.timestamp.to_le_bytes());
        
        // Add game state data
        hasher.update(format!("{:?}", state.game_state.phase));
        
        Ok(hasher.finalize().into())
    }
    
    /// Generate proposal ID
    fn generate_proposal_id(&self, operation: &GameOperation) -> ProposalId {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        
        hasher.update(self.game_id);
        hasher.update(self.local_peer_id);
        hasher.update(current_timestamp().to_le_bytes());
        
        match operation {
            GameOperation::PlaceBet { player, bet, nonce } => {
                hasher.update(b"place_bet");
                hasher.update(player);
                hasher.update(bet.amount.0.to_le_bytes());
                hasher.update(nonce.to_le_bytes());
            }
            _ => {
                hasher.update(b"other_operation");
            }
        }
        
        hasher.finalize().into()
    }
    
    /// Shutdown engine
    pub fn shutdown(&self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::Arc;
    use crate::protocol::craps::CrapsGame;
    
    #[test]
    fn test_lock_free_consensus() {
        let game_id = [0u8; 16];
        let peer_id = [1u8; 32];
        
        let initial_state = GameConsensusState {
            game_id,
            state_hash: [0u8; 32],
            sequence_number: 0,
            timestamp: 0,
            game_state: CrapsGame::new(game_id, peer_id),
            player_balances: FxHashMap::default(),
            last_proposer: peer_id,
            confirmations: 0,
            is_finalized: false,
        };
        
        let engine = Arc::new(LockFreeConsensusEngine::new(
            game_id,
            peer_id,
            vec![peer_id],
            initial_state,
        ));
        
        // Test concurrent updates
        let mut handles = vec![];
        
        for i in 0..10 {
            let engine_clone = engine.clone();
            let handle = thread::spawn(move || {
                let mut changes = FxHashMap::default();
                changes.insert(peer_id, CrapTokens::new(i as u64));
                let operation = GameOperation::UpdateBalances {
                    changes,
                    reason: format!("Test update {}", i),
                };
                
                engine_clone.apply_operation(&operation).unwrap();
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Check metrics
        let metrics = engine.get_metrics();
        assert_eq!(metrics.state_transitions.load(Ordering::Relaxed), 10);
        
        // Check final state
        let final_state = engine.get_current_state().unwrap();
        assert!(final_state.version > 0);
    }
    
    #[test]
    fn test_optimistic_update() {
        let game_id = [0u8; 16];
        let peer_id = [1u8; 32];
        
        let initial_state = GameConsensusState {
            game_id,
            state_hash: [0u8; 32],
            sequence_number: 0,
            timestamp: 0,
            game_state: CrapsGame::new(game_id, peer_id),
            player_balances: FxHashMap::default(),
            last_proposer: peer_id,
            confirmations: 0,
            is_finalized: false,
        };
        
        let engine = LockFreeConsensusEngine::new(
            game_id,
            peer_id,
            vec![peer_id],
            initial_state,
        );
        
        // Test optimistic update
        let result = engine.optimistic_update(|state| {
            let mut new_state = state.clone();
            new_state.sequence_number += 1;
            Ok(new_state)
        }).unwrap();
        
        assert_eq!(result.state.sequence_number, 1);
    }
}