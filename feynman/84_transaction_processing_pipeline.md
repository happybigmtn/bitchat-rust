# Chapter 84: Transaction Processing Pipeline - Moving Money Safely

## Understanding Database Transactions Through BitCraps
*"A transaction is like a marriage proposal - it's all or nothing. Either everything works perfectly, or nothing happens at all."*

---

## Part I: What Is a Transaction?

Imagine you're at a bank, transferring $100 from your checking account to your savings account. This simple action actually involves multiple steps:

1. **Check** if you have $100 in checking
2. **Subtract** $100 from checking
3. **Add** $100 to savings  
4. **Record** the transaction in your statement
5. **Update** account balances in the bank's systems

What happens if step 3 fails because the savings account database is down? You'd lose $100! This is why we need transactions - mechanisms that ensure either ALL steps complete successfully, or NONE of them do.

In BitCraps, when Alice bets 50 tokens on the dice roll, we need:
- Her token balance to decrease by 50
- The bet to be recorded in the game state
- All other players to see the bet
- The game history to be updated

If any step fails, the entire bet should be cancelled. No partial states allowed.

## Part II: The BitCraps Transaction Architecture

### Core Transaction Processing

```rust
// From src/database/mod.rs (extended for transactions)
pub struct TransactionProcessor {
    connection_pool: Arc<DatabasePool>,
    transaction_log: TransactionLog,
    lock_manager: LockManager,
    recovery_manager: RecoveryManager,
}

impl TransactionProcessor {
    pub async fn execute_transaction<F, T>(&self, 
        transaction_fn: F
    ) -> Result<T, TransactionError> 
    where
        F: FnOnce(&mut Transaction) -> BoxFuture<'_, Result<T, TransactionError>>,
    {
        // Get connection from pool
        let mut conn = self.connection_pool.get().await?;
        
        // Start transaction
        let mut transaction = conn.begin().await?;
        
        // Generate unique transaction ID
        let tx_id = TransactionId::generate();
        
        // Log transaction start
        self.transaction_log.log_start(tx_id).await?;
        
        // Execute user's transaction logic
        let result = match transaction_fn(&mut transaction).await {
            Ok(result) => {
                // Success - commit all changes
                match transaction.commit().await {
                    Ok(_) => {
                        self.transaction_log.log_commit(tx_id).await?;
                        Ok(result)
                    }
                    Err(e) => {
                        self.transaction_log.log_rollback(tx_id, e.to_string()).await?;
                        Err(TransactionError::CommitFailed(e))
                    }
                }
            }
            Err(e) => {
                // Error occurred - rollback all changes
                transaction.rollback().await?;
                self.transaction_log.log_rollback(tx_id, e.to_string()).await?;
                Err(e)
            }
        };
        
        result
    }
    
    // Process a bet transaction
    pub async fn process_bet_transaction(&self,
        player_id: PlayerId,
        bet_type: BetType,
        amount: u64
    ) -> Result<BetTransactionResult, TransactionError> {
        
        self.execute_transaction(|tx| async move {
            // Step 1: Check if player has sufficient tokens
            let current_balance = self.get_player_balance(tx, player_id).await?;
            
            if current_balance < amount {
                return Err(TransactionError::InsufficientFunds);
            }
            
            // Step 2: Reserve tokens (prevents double-spending)
            let reservation_id = self.reserve_tokens(tx, player_id, amount).await?;
            
            // Step 3: Validate bet against current game state
            let game_state = self.get_current_game_state(tx).await?;
            
            if !bet_type.is_valid_for_game_phase(game_state.phase) {
                // Release reservation since bet is invalid
                self.release_token_reservation(tx, reservation_id).await?;
                return Err(TransactionError::InvalidBetForGamePhase);
            }
            
            // Step 4: Create bet record
            let bet_id = self.create_bet_record(tx, BetRecord {
                id: BetId::generate(),
                player_id,
                bet_type,
                amount,
                game_state_version: game_state.version,
                timestamp: Utc::now(),
            }).await?;
            
            // Step 5: Update player balance
            let new_balance = current_balance - amount;
            self.update_player_balance(tx, player_id, new_balance).await?;
            
            // Step 6: Convert token reservation to actual deduction
            self.confirm_token_reservation(tx, reservation_id).await?;
            
            // Step 7: Update game state
            let mut updated_game_state = game_state;
            updated_game_state.add_bet(bet_id, player_id, bet_type, amount)?;
            updated_game_state.increment_version();
            
            self.update_game_state(tx, &updated_game_state).await?;
            
            // Step 8: Log transaction for audit trail
            self.log_bet_transaction(tx, BetTransactionLog {
                bet_id,
                player_id,
                amount,
                previous_balance: current_balance,
                new_balance,
                game_state_version: updated_game_state.version,
            }).await?;
            
            Ok(BetTransactionResult {
                bet_id,
                new_balance,
                game_state_version: updated_game_state.version,
            })
            
        }.boxed()).await
    }
    
    // Process payout when dice are rolled
    pub async fn process_payout_transaction(&self,
        dice_result: DiceResult,
        game_state: &GameState
    ) -> Result<PayoutTransactionResult, TransactionError> {
        
        self.execute_transaction(|tx| async move {
            let mut total_paid_out = 0u64;
            let mut payout_records = Vec::new();
            
            // Calculate payouts for all bets
            for (bet_id, bet) in &game_state.active_bets {
                let payout_amount = bet.calculate_payout(&dice_result);
                
                if payout_amount > 0 {
                    // Step 1: Get current player balance
                    let current_balance = self.get_player_balance(tx, bet.player_id).await?;
                    
                    // Step 2: Add winnings to balance
                    let new_balance = current_balance + payout_amount;
                    self.update_player_balance(tx, bet.player_id, new_balance).await?;
                    
                    // Step 3: Record payout
                    let payout_record = PayoutRecord {
                        payout_id: PayoutId::generate(),
                        bet_id: *bet_id,
                        player_id: bet.player_id,
                        amount: payout_amount,
                        dice_result: dice_result.clone(),
                        timestamp: Utc::now(),
                    };
                    
                    self.create_payout_record(tx, &payout_record).await?;
                    payout_records.push(payout_record);
                    
                    total_paid_out += payout_amount;
                }
            }
            
            // Step 4: Clear all active bets (they're now resolved)
            self.clear_active_bets(tx, game_state.id).await?;
            
            // Step 5: Update game state
            let mut updated_game_state = game_state.clone();
            updated_game_state.resolve_bets_with_dice_result(dice_result.clone());
            updated_game_state.increment_version();
            
            self.update_game_state(tx, &updated_game_state).await?;
            
            // Step 6: Verify token conservation
            let total_tokens_before = self.calculate_total_tokens_in_system(tx).await?;
            // Note: total_tokens_before should equal total_tokens_after 
            // because payouts come from losing bets
            
            Ok(PayoutTransactionResult {
                payout_records,
                total_paid_out,
                game_state_version: updated_game_state.version,
            })
            
        }.boxed()).await
    }
}
```

### Transaction Locking and Concurrency

When multiple players try to bet simultaneously, we need locking to prevent conflicts:

```rust
pub struct LockManager {
    locks: Arc<Mutex<HashMap<LockKey, LockInfo>>>,
    lock_timeouts: HashMap<LockKey, Instant>,
}

#[derive(Hash, Eq, PartialEq)]
pub enum LockKey {
    PlayerBalance(PlayerId),
    GameState(GameId),
    BetRecord(BetId),
    GlobalTokenCount,
}

impl LockManager {
    pub async fn acquire_locks(&self, 
        keys: Vec<LockKey>, 
        timeout: Duration
    ) -> Result<LockGuard, LockError> {
        
        let deadline = Instant::now() + timeout;
        let mut acquired_locks = Vec::new();
        
        // Sort keys to prevent deadlock (always acquire in same order)
        let mut sorted_keys = keys;
        sorted_keys.sort_by_key(|k| format!("{:?}", k));
        
        for key in sorted_keys {
            // Try to acquire lock with timeout
            let lock_acquired = self.try_acquire_lock_with_timeout(key.clone(), deadline).await?;
            
            if lock_acquired {
                acquired_locks.push(key);
            } else {
                // Failed to acquire - release all locks we got so far
                for acquired_key in acquired_locks {
                    self.release_lock(acquired_key).await;
                }
                return Err(LockError::Timeout);
            }
        }
        
        Ok(LockGuard::new(acquired_locks, self))
    }
    
    async fn try_acquire_lock_with_timeout(&self, 
        key: LockKey, 
        deadline: Instant
    ) -> Result<bool, LockError> {
        
        loop {
            {
                let mut locks = self.locks.lock().await;
                
                // Check if lock is available
                if !locks.contains_key(&key) {
                    // Lock is available - acquire it
                    locks.insert(key.clone(), LockInfo {
                        holder: std::thread::current().id(),
                        acquired_at: Instant::now(),
                    });
                    return Ok(true);
                }
                
                // Check if existing lock is expired
                if let Some(lock_info) = locks.get(&key) {
                    if lock_info.acquired_at.elapsed() > Duration::from_secs(30) {
                        // Lock is stale - take it over
                        locks.insert(key.clone(), LockInfo {
                            holder: std::thread::current().id(),
                            acquired_at: Instant::now(),
                        });
                        return Ok(true);
                    }
                }
            }
            
            // Lock not available - check if we've timed out
            if Instant::now() >= deadline {
                return Ok(false);
            }
            
            // Wait a bit and try again
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

// Use locks in transaction processing
impl TransactionProcessor {
    pub async fn process_bet_with_locking(&self,
        player_id: PlayerId,
        bet_type: BetType,
        amount: u64
    ) -> Result<BetTransactionResult, TransactionError> {
        
        // Acquire necessary locks
        let locks = self.lock_manager.acquire_locks(vec![
            LockKey::PlayerBalance(player_id),
            LockKey::GameState(self.current_game_id()),
        ], Duration::from_secs(5)).await?;
        
        // Process transaction while holding locks
        let result = self.process_bet_transaction(player_id, bet_type, amount).await;
        
        // Locks are automatically released when guard is dropped
        drop(locks);
        
        result
    }
}
```

## Part III: Write-Ahead Logging for Durability

BitCraps uses write-ahead logging to ensure no transactions are lost, even if the system crashes:

```rust
pub struct WriteAheadLog {
    log_file: Arc<Mutex<File>>,
    log_sequence_number: Arc<AtomicU64>,
    checkpoint_manager: CheckpointManager,
}

impl WriteAheadLog {
    pub async fn log_transaction_start(&self, tx_id: TransactionId) -> Result<LSN, WALError> {
        let lsn = self.get_next_lsn();
        
        let log_entry = WALEntry {
            lsn,
            transaction_id: tx_id,
            entry_type: WALEntryType::TransactionStart,
            data: vec![], // No data for start record
            timestamp: Utc::now(),
        };
        
        self.write_log_entry(log_entry).await?;
        
        Ok(lsn)
    }
    
    pub async fn log_operation(&self, 
        tx_id: TransactionId,
        operation: DatabaseOperation
    ) -> Result<LSN, WALError> {
        let lsn = self.get_next_lsn();
        
        let log_entry = WALEntry {
            lsn,
            transaction_id: tx_id,
            entry_type: WALEntryType::Operation,
            data: bincode::serialize(&operation)?,
            timestamp: Utc::now(),
        };
        
        self.write_log_entry(log_entry).await?;
        
        Ok(lsn)
    }
    
    pub async fn log_transaction_commit(&self, tx_id: TransactionId) -> Result<LSN, WALError> {
        let lsn = self.get_next_lsn();
        
        let log_entry = WALEntry {
            lsn,
            transaction_id: tx_id,
            entry_type: WALEntryType::TransactionCommit,
            data: vec![],
            timestamp: Utc::now(),
        };
        
        self.write_log_entry(log_entry).await?;
        
        // Force log to disk before returning (durability guarantee)
        self.force_log_to_disk().await?;
        
        Ok(lsn)
    }
    
    async fn write_log_entry(&self, entry: WALEntry) -> Result<(), WALError> {
        let mut file = self.log_file.lock().await;
        
        // Serialize log entry
        let serialized = bincode::serialize(&entry)?;
        let entry_size = serialized.len() as u32;
        
        // Write size prefix (for reading back)
        file.write_all(&entry_size.to_le_bytes()).await?;
        
        // Write entry data
        file.write_all(&serialized).await?;
        
        Ok(())
    }
    
    // Recovery: replay log entries after crash
    pub async fn recover_transactions(&self) -> Result<Vec<RecoveredTransaction>, WALError> {
        let mut file = File::open(&self.log_file_path).await?;
        let mut recovered_transactions = HashMap::new();
        
        loop {
            // Try to read next entry
            let mut size_bytes = [0u8; 4];
            match file.read_exact(&mut size_bytes).await {
                Ok(_) => {
                    let entry_size = u32::from_le_bytes(size_bytes);
                    
                    let mut entry_data = vec![0u8; entry_size as usize];
                    file.read_exact(&mut entry_data).await?;
                    
                    let entry: WALEntry = bincode::deserialize(&entry_data)?;
                    
                    // Process log entry
                    match entry.entry_type {
                        WALEntryType::TransactionStart => {
                            recovered_transactions.insert(entry.transaction_id, RecoveredTransaction {
                                id: entry.transaction_id,
                                status: TransactionStatus::InProgress,
                                operations: vec![],
                            });
                        }
                        
                        WALEntryType::Operation => {
                            let operation: DatabaseOperation = bincode::deserialize(&entry.data)?;
                            
                            if let Some(tx) = recovered_transactions.get_mut(&entry.transaction_id) {
                                tx.operations.push(operation);
                            }
                        }
                        
                        WALEntryType::TransactionCommit => {
                            if let Some(tx) = recovered_transactions.get_mut(&entry.transaction_id) {
                                tx.status = TransactionStatus::Committed;
                            }
                        }
                        
                        WALEntryType::TransactionAbort => {
                            if let Some(tx) = recovered_transactions.get_mut(&entry.transaction_id) {
                                tx.status = TransactionStatus::Aborted;
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of file - normal termination
                    break;
                }
                Err(e) => return Err(WALError::IOError(e)),
            }
        }
        
        Ok(recovered_transactions.into_values().collect())
    }
}
```

## Part IV: ACID Properties in Action

BitCraps ensures all transactions have ACID properties:

### Atomicity - All or Nothing

```rust
pub async fn transfer_tokens_atomically(&self,
    from_player: PlayerId,
    to_player: PlayerId,
    amount: u64
) -> Result<TransferResult, TransactionError> {
    
    self.execute_transaction(|tx| async move {
        // All these operations succeed together or fail together
        
        // 1. Check source has enough tokens
        let from_balance = self.get_player_balance(tx, from_player).await?;
        if from_balance < amount {
            return Err(TransactionError::InsufficientFunds);
        }
        
        // 2. Subtract from source
        let new_from_balance = from_balance - amount;
        self.update_player_balance(tx, from_player, new_from_balance).await?;
        
        // 3. Add to destination
        let to_balance = self.get_player_balance(tx, to_player).await?;
        let new_to_balance = to_balance + amount;
        self.update_player_balance(tx, to_player, new_to_balance).await?;
        
        // 4. Log transfer
        self.log_transfer(tx, TransferRecord {
            from_player,
            to_player,
            amount,
            timestamp: Utc::now(),
        }).await?;
        
        // If ANY step fails, ALL steps are undone automatically
        
        Ok(TransferResult {
            from_balance: new_from_balance,
            to_balance: new_to_balance,
        })
        
    }.boxed()).await
}
```

### Consistency - Valid States Only

```rust
impl TransactionProcessor {
    async fn validate_game_state_consistency(&self, 
        tx: &mut Transaction,
        proposed_state: &GameState
    ) -> Result<(), ConsistencyError> {
        
        // Rule 1: Total tokens in system must be conserved
        let total_tokens_in_balances = self.sum_all_player_balances(tx).await?;
        let total_tokens_in_bets = self.sum_all_active_bets(tx).await?;
        let expected_total = self.get_initial_token_supply().await?;
        
        if total_tokens_in_balances + total_tokens_in_bets != expected_total {
            return Err(ConsistencyError::TokenConservationViolated);
        }
        
        // Rule 2: Game phase transitions must be valid
        let current_state = self.get_current_game_state(tx).await?;
        if !proposed_state.phase.is_valid_transition_from(&current_state.phase) {
            return Err(ConsistencyError::InvalidPhaseTransition);
        }
        
        // Rule 3: Player balances cannot be negative
        for (player_id, balance) in &proposed_state.player_balances {
            if *balance < 0 {
                return Err(ConsistencyError::NegativeBalance(*player_id));
            }
        }
        
        // Rule 4: Bets must be valid for current game phase
        for bet in &proposed_state.active_bets {
            if !bet.bet_type.is_valid_for_phase(&proposed_state.phase) {
                return Err(ConsistencyError::InvalidBetForPhase);
            }
        }
        
        Ok(())
    }
}
```

### Isolation - Transactions Don't Interfere

```rust
pub enum IsolationLevel {
    ReadUncommitted, // Dirty reads allowed
    ReadCommitted,   // Only committed data visible
    RepeatableRead,  // Same reads return same results
    Serializable,    // Transactions appear to run sequentially
}

impl TransactionProcessor {
    pub async fn execute_transaction_with_isolation<F, T>(&self,
        isolation_level: IsolationLevel,
        transaction_fn: F
    ) -> Result<T, TransactionError>
    where
        F: FnOnce(&mut Transaction) -> BoxFuture<'_, Result<T, TransactionError>>,
    {
        match isolation_level {
            IsolationLevel::Serializable => {
                // Highest isolation - acquire all necessary locks upfront
                let locks = self.acquire_serializable_locks().await?;
                let result = self.execute_transaction(transaction_fn).await?;
                drop(locks); // Release locks
                Ok(result)
            }
            
            IsolationLevel::RepeatableRead => {
                // Take snapshot of data at transaction start
                let snapshot = self.create_transaction_snapshot().await?;
                let result = self.execute_transaction_with_snapshot(transaction_fn, snapshot).await?;
                Ok(result)
            }
            
            IsolationLevel::ReadCommitted => {
                // Default - see committed changes from other transactions
                self.execute_transaction(transaction_fn).await
            }
            
            IsolationLevel::ReadUncommitted => {
                // Lowest isolation - might see uncommitted changes
                self.execute_transaction_dirty_read(transaction_fn).await
            }
        }
    }
    
    // Detect conflicts between concurrent transactions
    async fn detect_serialization_conflicts(&self,
        tx1_id: TransactionId,
        tx2_id: TransactionId
    ) -> Result<bool, TransactionError> {
        
        let tx1_ops = self.get_transaction_operations(tx1_id).await?;
        let tx2_ops = self.get_transaction_operations(tx2_id).await?;
        
        // Check for read-write conflicts
        for op1 in &tx1_ops {
            for op2 in &tx2_ops {
                match (op1, op2) {
                    // TX1 reads what TX2 writes - conflict!
                    (DatabaseOperation::Read { key: k1 }, DatabaseOperation::Write { key: k2, .. }) 
                    if k1 == k2 => {
                        return Ok(true);
                    }
                    
                    // TX1 writes what TX2 reads - conflict!
                    (DatabaseOperation::Write { key: k1, .. }, DatabaseOperation::Read { key: k2 }) 
                    if k1 == k2 => {
                        return Ok(true);
                    }
                    
                    // TX1 writes what TX2 writes - conflict!
                    (DatabaseOperation::Write { key: k1, .. }, DatabaseOperation::Write { key: k2, .. }) 
                    if k1 == k2 => {
                        return Ok(true);
                    }
                    
                    _ => {} // No conflict
                }
            }
        }
        
        Ok(false) // No conflicts found
    }
}
```

### Durability - Changes Survive Crashes

```rust
impl TransactionProcessor {
    pub async fn ensure_durability(&self, tx_id: TransactionId) -> Result<(), DurabilityError> {
        // 1. Write transaction to write-ahead log
        self.wal.log_transaction_commit(tx_id).await?;
        
        // 2. Force log to persistent storage
        self.wal.force_log_to_disk().await?;
        
        // 3. Create checkpoint periodically
        if self.should_create_checkpoint().await {
            self.create_checkpoint().await?;
        }
        
        Ok(())
    }
    
    async fn create_checkpoint(&self) -> Result<(), CheckpointError> {
        // A checkpoint saves current database state to disk
        // so we don't have to replay all log entries from the beginning
        
        let checkpoint_id = CheckpointId::generate();
        
        // 1. Pause new transactions temporarily
        let _pause_guard = self.pause_new_transactions().await;
        
        // 2. Wait for active transactions to complete
        self.wait_for_active_transactions().await?;
        
        // 3. Write current state to checkpoint file
        let current_state = self.get_full_database_state().await?;
        let checkpoint_file = format!("checkpoint_{}.db", checkpoint_id);
        
        let mut file = File::create(&checkpoint_file).await?;
        let serialized_state = bincode::serialize(&current_state)?;
        file.write_all(&serialized_state).await?;
        file.sync_all().await?; // Force to disk
        
        // 4. Update checkpoint metadata
        self.update_checkpoint_metadata(checkpoint_id, checkpoint_file).await?;
        
        // 5. Truncate old log entries (they're now in the checkpoint)
        self.wal.truncate_log_before_checkpoint(checkpoint_id).await?;
        
        Ok(())
    }
}
```

## Part V: Distributed Transaction Coordination

When BitCraps runs across multiple nodes, we need distributed transactions:

```rust
pub struct DistributedTransactionCoordinator {
    node_id: NodeId,
    participant_nodes: Vec<NodeId>,
    transaction_state: HashMap<TransactionId, DistributedTransactionState>,
}

impl DistributedTransactionCoordinator {
    // Two-Phase Commit Protocol
    pub async fn execute_distributed_transaction(&self,
        tx_id: TransactionId,
        operations: Vec<(NodeId, DatabaseOperation)>
    ) -> Result<(), DistributedTransactionError> {
        
        // Phase 1: Prepare - Ask all nodes if they can commit
        let mut prepare_responses = Vec::new();
        
        for (node_id, operation) in &operations {
            let prepare_msg = PrepareMessage {
                transaction_id: tx_id,
                operation: operation.clone(),
                coordinator: self.node_id,
            };
            
            let response = self.send_prepare_message(*node_id, prepare_msg).await?;
            prepare_responses.push(response);
        }
        
        // Check if all nodes voted to commit
        let all_voted_commit = prepare_responses.iter()
            .all(|r| matches!(r.vote, PrepareVote::Commit));
        
        // Phase 2: Commit or Abort based on votes
        if all_voted_commit {
            // All nodes ready - send commit messages
            for (node_id, _) in &operations {
                let commit_msg = CommitMessage {
                    transaction_id: tx_id,
                };
                
                self.send_commit_message(*node_id, commit_msg).await?;
            }
            
            Ok(())
        } else {
            // At least one node voted abort - send abort to all
            for (node_id, _) in &operations {
                let abort_msg = AbortMessage {
                    transaction_id: tx_id,
                };
                
                self.send_abort_message(*node_id, abort_msg).await?;
            }
            
            Err(DistributedTransactionError::TransactionAborted)
        }
    }
    
    // Handle prepare message from coordinator
    pub async fn handle_prepare_message(&self, msg: PrepareMessage) -> Result<PrepareResponse, TransactionError> {
        // Try to prepare the transaction locally
        let local_result = self.prepare_local_transaction(
            msg.transaction_id,
            msg.operation
        ).await;
        
        match local_result {
            Ok(_) => {
                // We can commit - vote yes
                Ok(PrepareResponse {
                    transaction_id: msg.transaction_id,
                    node_id: self.node_id,
                    vote: PrepareVote::Commit,
                })
            }
            Err(e) => {
                // We cannot commit - vote no
                Ok(PrepareResponse {
                    transaction_id: msg.transaction_id,
                    node_id: self.node_id,
                    vote: PrepareVote::Abort(e.to_string()),
                })
            }
        }
    }
    
    async fn prepare_local_transaction(&self,
        tx_id: TransactionId,
        operation: DatabaseOperation
    ) -> Result<(), TransactionError> {
        
        // 1. Acquire necessary locks
        let locks = self.acquire_locks_for_operation(&operation).await?;
        
        // 2. Validate operation can be performed
        if !self.can_perform_operation(&operation).await? {
            return Err(TransactionError::OperationInvalid);
        }
        
        // 3. Write prepare record to log (but don't commit yet)
        self.wal.log_prepare(tx_id, operation.clone()).await?;
        
        // 4. Keep locks until commit/abort message arrives
        self.hold_locks_for_transaction(tx_id, locks).await;
        
        Ok(())
    }
}
```

## Part VI: Transaction Performance Optimization

High-performance transaction processing requires careful optimization:

```rust
pub struct TransactionOptimizer {
    batch_processor: BatchProcessor,
    connection_pool: ConnectionPool,
    prefetch_engine: PrefetchEngine,
}

impl TransactionOptimizer {
    // Batch multiple transactions together
    pub async fn batch_process_bets(&self, 
        bets: Vec<BetRequest>
    ) -> Result<Vec<BetResult>, BatchError> {
        
        // Group bets by the locks they need
        let mut lock_groups: HashMap<Vec<LockKey>, Vec<BetRequest>> = HashMap::new();
        
        for bet in bets {
            let required_locks = vec![
                LockKey::PlayerBalance(bet.player_id),
                LockKey::GameState(bet.game_id),
            ];
            
            lock_groups.entry(required_locks).or_insert_with(Vec::new).push(bet);
        }
        
        let mut all_results = Vec::new();
        
        // Process each group in a single transaction
        for (locks, group_bets) in lock_groups {
            let group_results = self.process_bet_group(locks, group_bets).await?;
            all_results.extend(group_results);
        }
        
        Ok(all_results)
    }
    
    async fn process_bet_group(&self,
        required_locks: Vec<LockKey>,
        bets: Vec<BetRequest>
    ) -> Result<Vec<BetResult>, BatchError> {
        
        // Acquire locks once for entire group
        let lock_guard = self.lock_manager.acquire_locks(required_locks, Duration::from_secs(10)).await?;
        
        // Process all bets in single transaction
        let results = self.execute_transaction(|tx| async move {
            let mut results = Vec::new();
            
            for bet in bets {
                let result = self.process_single_bet_in_transaction(tx, bet).await?;
                results.push(result);
            }
            
            Ok(results)
        }.boxed()).await?;
        
        drop(lock_guard); // Release locks
        
        Ok(results)
    }
    
    // Prefetch data that transactions will likely need
    pub async fn prefetch_for_game(&self, game_id: GameId) -> Result<(), PrefetchError> {
        // Prefetch player balances for active players
        let active_players = self.get_active_players_for_game(game_id).await?;
        
        for player_id in active_players {
            self.prefetch_engine.prefetch_player_balance(player_id).await?;
        }
        
        // Prefetch current game state
        self.prefetch_engine.prefetch_game_state(game_id).await?;
        
        // Prefetch recent transaction history (for validation)
        self.prefetch_engine.prefetch_recent_transactions(game_id).await?;
        
        Ok(())
    }
}
```

## Part VII: Practical Transaction Exercise

Let's implement a simple banking system with transactions:

**Exercise: Multi-Account Transfer**

```rust
pub struct BankingSystem {
    accounts: HashMap<AccountId, Arc<Mutex<Account>>>,
    transaction_log: Vec<TransactionRecord>,
}

impl BankingSystem {
    pub async fn transfer_money(&mut self,
        from_account: AccountId,
        to_account: AccountId,
        amount: Decimal
    ) -> Result<TransferReceipt, BankingError> {
        
        // Start transaction
        let tx_id = TransactionId::generate();
        self.log_transaction_start(tx_id).await;
        
        // Acquire locks in consistent order (prevent deadlock)
        let (first_lock, second_lock) = if from_account < to_account {
            (from_account, to_account)
        } else {
            (to_account, from_account)
        };
        
        let first_account = self.accounts.get(&first_lock)
            .ok_or(BankingError::AccountNotFound)?
            .clone();
        
        let second_account = self.accounts.get(&second_lock)
            .ok_or(BankingError::AccountNotFound)?
            .clone();
        
        // Acquire locks
        let _lock1 = first_account.lock().await;
        let _lock2 = second_account.lock().await;
        
        // Now safely get mutable references
        let from_account_ref = self.accounts.get(&from_account).unwrap();
        let to_account_ref = self.accounts.get(&to_account).unwrap();
        
        let mut from_acc = from_account_ref.lock().await;
        let mut to_acc = to_account_ref.lock().await;
        
        // Validate transfer
        if from_acc.balance < amount {
            self.log_transaction_abort(tx_id, "Insufficient funds").await;
            return Err(BankingError::InsufficientFunds);
        }
        
        // Perform transfer
        let old_from_balance = from_acc.balance;
        let old_to_balance = to_acc.balance;
        
        from_acc.balance -= amount;
        to_acc.balance += amount;
        
        // Log operations
        self.log_operation(tx_id, Operation::Debit {
            account: from_account,
            amount,
            old_balance: old_from_balance,
            new_balance: from_acc.balance,
        }).await;
        
        self.log_operation(tx_id, Operation::Credit {
            account: to_account,
            amount,
            old_balance: old_to_balance,
            new_balance: to_acc.balance,
        }).await;
        
        // Commit transaction
        self.log_transaction_commit(tx_id).await;
        
        Ok(TransferReceipt {
            transaction_id: tx_id,
            from_account,
            to_account,
            amount,
            from_new_balance: from_acc.balance,
            to_new_balance: to_acc.balance,
        })
    }
    
    // Rollback transaction if something fails
    pub async fn rollback_transaction(&mut self, tx_id: TransactionId) -> Result<(), BankingError> {
        // Find all operations for this transaction
        let tx_operations: Vec<_> = self.transaction_log.iter()
            .filter(|record| record.transaction_id == tx_id)
            .cloned()
            .collect();
        
        // Reverse operations in reverse order
        for record in tx_operations.into_iter().rev() {
            match record.operation {
                Operation::Debit { account, amount, old_balance, .. } => {
                    // Undo debit by crediting back
                    let account_ref = self.accounts.get(&account).unwrap();
                    let mut acc = account_ref.lock().await;
                    acc.balance = old_balance; // Restore old balance
                }
                
                Operation::Credit { account, amount, old_balance, .. } => {
                    // Undo credit by debiting back
                    let account_ref = self.accounts.get(&account).unwrap();
                    let mut acc = account_ref.lock().await;
                    acc.balance = old_balance; // Restore old balance
                }
            }
        }
        
        self.log_transaction_abort(tx_id, "Manual rollback").await;
        
        Ok(())
    }
}

#[tokio::test]
async fn test_concurrent_transfers() {
    let mut bank = BankingSystem::new();
    
    // Create accounts
    bank.create_account(AccountId(1), Decimal::from(1000)).await;
    bank.create_account(AccountId(2), Decimal::from(500)).await;
    
    // Concurrent transfers
    let bank_ref = Arc::new(Mutex::new(bank));
    
    let transfer1 = {
        let bank = bank_ref.clone();
        tokio::spawn(async move {
            let mut b = bank.lock().await;
            b.transfer_money(AccountId(1), AccountId(2), Decimal::from(100)).await
        })
    };
    
    let transfer2 = {
        let bank = bank_ref.clone();
        tokio::spawn(async move {
            let mut b = bank.lock().await;
            b.transfer_money(AccountId(2), AccountId(1), Decimal::from(50)).await
        })
    };
    
    let (result1, result2) = tokio::join!(transfer1, transfer2);
    
    // Both transfers should succeed
    assert!(result1.unwrap().is_ok());
    assert!(result2.unwrap().is_ok());
    
    // Check final balances are consistent
    let bank = bank_ref.lock().await;
    let account1_balance = bank.get_balance(AccountId(1)).await.unwrap();
    let account2_balance = bank.get_balance(AccountId(2)).await.unwrap();
    
    // Total money should be conserved
    assert_eq!(account1_balance + account2_balance, Decimal::from(1500));
}
```

## Conclusion: Transactions as Trust Infrastructure

Transaction processing is the foundation of trust in any system that handles valuable assets. In BitCraps, when players bet real tokens, they're trusting that:

1. **Their bets will be processed correctly** - Or not at all if something goes wrong
2. **No tokens will be lost** - Every token transfer is atomic and logged
3. **The game state stays consistent** - No impossible states, no token duplication
4. **Recovery is possible** - System crashes don't lose money

The key insights for transaction design:

1. **ACID properties aren't optional** - They're what make money systems trustworthy
2. **Locking prevents races** - But design carefully to avoid deadlocks
3. **Logging enables recovery** - Write-ahead logs let you survive crashes
4. **Batching improves performance** - Process similar transactions together
5. **Distributed transactions are hard** - Two-phase commit has its complexities

Remember: In financial systems, correctness is more important than performance. It's better to be slow and right than fast and wrong. BitCraps players trust the system with real value - transaction processing is what makes that trust possible.