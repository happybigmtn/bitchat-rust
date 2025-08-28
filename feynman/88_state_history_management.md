# Chapter 88: State History Management - Time Travel for Your Data

## Introduction: The Memory of Systems

In 1949, Maurice Wilkes was debugging the EDSAC computer at Cambridge. As he watched the machine execute instructions, he had a profound realization: "A good part of the remainder of my life was going to be spent finding mistakes in my own programs." If only he could see what the machine had done in the past, debugging would be so much easier.

This desire to understand not just what a system is doing now, but what it did then, drives the field of state history management. It's the difference between a doctor who only knows your current symptoms and one who has your complete medical history. In distributed systems like BitCraps, where thousands of events happen per second across multiple nodes, understanding history isn't just helpful - it's essential for debugging, auditing, and recovery.

This chapter explores how to build systems that remember everything, forget nothing, and can reconstruct any moment in their past. We'll cover event sourcing, audit logging, time-travel debugging, and the fascinating challenge of managing infinite history with finite resources.

## The Philosophy of Event Sourcing

Traditional systems store current state. Event-sourced systems store how we got here:

### State-Based Approach (Traditional)
```
Current Balance: $100
```

### Event-Based Approach (Event Sourcing)
```
1. Account Opened: +$0
2. Deposit: +$50
3. Deposit: +$75
4. Withdrawal: -$25
Current Balance: $100 (derived)
```

The second approach tells a story. And stories are powerful.

## Building an Event Store

Let's implement a production-ready event store for BitCraps:

```rust
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Core event trait that all events must implement
pub trait Event: Send + Sync + Clone + Serialize + for<'de> Deserialize<'de> {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> Uuid;
    fn version(&self) -> u64;
}

/// Metadata attached to every event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub causation_id: Option<Uuid>,  // What caused this event
    pub correlation_id: Option<Uuid>, // What request/saga this belongs to
    pub user_id: Option<String>,
    pub source: String,
    pub version: u64,
}

/// A stored event with its metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent<E: Event> {
    pub event: E,
    pub metadata: EventMetadata,
    pub sequence_number: u64,  // Global sequence number
}

/// The event store interface
pub trait EventStore: Send + Sync {
    type Event: Event;
    type Error: std::error::Error;
    
    /// Append events to the store
    async fn append_events(
        &self,
        aggregate_id: Uuid,
        events: Vec<Self::Event>,
        expected_version: Option<u64>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, Self::Error>;
    
    /// Get all events for an aggregate
    async fn get_events(
        &self,
        aggregate_id: Uuid,
        from_version: u64,
        to_version: Option<u64>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, Self::Error>;
    
    /// Get events by time range
    async fn get_events_by_time(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StoredEvent<Self::Event>>, Self::Error>;
    
    /// Subscribe to real-time events
    async fn subscribe(&self) -> Box<dyn Stream<Item = StoredEvent<Self::Event>>>;
    
    /// Create a snapshot of current state
    async fn create_snapshot(
        &self,
        aggregate_id: Uuid,
        snapshot: impl Serialize,
    ) -> Result<(), Self::Error>;
    
    /// Get the latest snapshot
    async fn get_snapshot(
        &self,
        aggregate_id: Uuid,
    ) -> Result<Option<(u64, Box<dyn Any>)>, Self::Error>;
}
```

## SQLite Event Store Implementation

Here's a production implementation using SQLite:

```rust
use rusqlite::{Connection, params, OptionalExtension};
use tokio::sync::Mutex;

pub struct SqliteEventStore {
    conn: Arc<Mutex<Connection>>,
    subscribers: Arc<RwLock<Vec<mpsc::UnboundedSender<StoredEvent<GameEvent>>>>>,
}

impl SqliteEventStore {
    pub fn new(path: &str) -> Result<Self, EventStoreError> {
        let conn = Connection::open(path)?;
        
        // Create tables
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS events (
                sequence_number INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                aggregate_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                metadata TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                version INTEGER NOT NULL,
                
                INDEX idx_aggregate_id (aggregate_id, version),
                INDEX idx_timestamp (timestamp),
                INDEX idx_event_type (event_type)
            );
            
            CREATE TABLE IF NOT EXISTS snapshots (
                aggregate_id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                snapshot_data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS projections (
                projection_name TEXT PRIMARY KEY,
                last_sequence_number INTEGER NOT NULL,
                checkpoint_data TEXT,
                updated_at TEXT NOT NULL
            );
        "#)?;
        
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    async fn append_events_impl(
        &self,
        aggregate_id: Uuid,
        events: Vec<GameEvent>,
        expected_version: Option<u64>,
    ) -> Result<Vec<StoredEvent<GameEvent>>, EventStoreError> {
        let mut conn = self.conn.lock().await;
        let tx = conn.transaction()?;
        
        // Check expected version (optimistic concurrency control)
        if let Some(expected) = expected_version {
            let current: Option<u64> = tx
                .query_row(
                    "SELECT MAX(version) FROM events WHERE aggregate_id = ?",
                    params![aggregate_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            
            if current != Some(expected) {
                return Err(EventStoreError::ConcurrencyConflict {
                    expected,
                    actual: current.unwrap_or(0),
                });
            }
        }
        
        let mut stored_events = Vec::new();
        
        for event in events {
            let event_id = Uuid::new_v4();
            let metadata = EventMetadata {
                event_id,
                timestamp: Utc::now(),
                causation_id: None,
                correlation_id: None,
                user_id: None,
                source: "bitcraps".to_string(),
                version: event.version(),
            };
            
            let event_json = serde_json::to_string(&event)?;
            let metadata_json = serde_json::to_string(&metadata)?;
            
            tx.execute(
                r#"
                INSERT INTO events (
                    event_id, aggregate_id, event_type, event_data,
                    metadata, timestamp, version
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    event_id.to_string(),
                    aggregate_id.to_string(),
                    event.event_type(),
                    event_json,
                    metadata_json,
                    metadata.timestamp.to_rfc3339(),
                    event.version() as i64,
                ],
            )?;
            
            let sequence_number = tx.last_insert_rowid() as u64;
            
            let stored_event = StoredEvent {
                event,
                metadata,
                sequence_number,
            };
            
            stored_events.push(stored_event.clone());
        }
        
        tx.commit()?;
        
        // Notify subscribers
        self.notify_subscribers(&stored_events).await;
        
        Ok(stored_events)
    }
    
    async fn notify_subscribers(&self, events: &[StoredEvent<GameEvent>]) {
        let mut subscribers = self.subscribers.write().await;
        
        // Remove disconnected subscribers
        subscribers.retain(|sender| {
            for event in events {
                if sender.send(event.clone()).is_err() {
                    return false;
                }
            }
            true
        });
    }
}
```

## Event Projection: Deriving Current State

Events are the source of truth, but we need current state for queries:

```rust
/// A projection builds read models from events
pub trait Projection: Send + Sync {
    type Event: Event;
    type State: Clone + Send + Sync;
    type Error: std::error::Error;
    
    /// Process an event and update state
    fn apply_event(&mut self, event: &Self::Event) -> Result<(), Self::Error>;
    
    /// Get current state
    fn get_state(&self) -> Self::State;
    
    /// Reset to initial state
    fn reset(&mut self);
}

/// Game state projection
pub struct GameStateProjection {
    games: HashMap<Uuid, GameState>,
    player_stats: HashMap<PlayerId, PlayerStats>,
    leaderboard: BTreeMap<i64, Vec<PlayerId>>,  // Score -> Players
}

impl Projection for GameStateProjection {
    type Event = GameEvent;
    type State = GameView;
    type Error = ProjectionError;
    
    fn apply_event(&mut self, event: &GameEvent) -> Result<(), ProjectionError> {
        match event {
            GameEvent::GameStarted { game_id, players, initial_pot } => {
                let game = GameState {
                    id: *game_id,
                    players: players.clone(),
                    pot: *initial_pot,
                    phase: GamePhase::Betting,
                    started_at: Utc::now(),
                };
                self.games.insert(*game_id, game);
            }
            
            GameEvent::BetPlaced { game_id, player, amount } => {
                if let Some(game) = self.games.get_mut(game_id) {
                    game.pot += amount;
                    self.update_player_stats(player, |stats| {
                        stats.total_bet += amount;
                        stats.games_played += 1;
                    });
                }
            }
            
            GameEvent::DiceRolled { game_id, player, result } => {
                if let Some(game) = self.games.get_mut(game_id) {
                    game.last_roll = Some(*result);
                    self.update_player_stats(player, |stats| {
                        stats.rolls.push(*result);
                    });
                }
            }
            
            GameEvent::GameWon { game_id, winner, payout } => {
                if let Some(game) = self.games.get_mut(game_id) {
                    game.phase = GamePhase::Completed;
                    game.winner = Some(*winner);
                    
                    self.update_player_stats(winner, |stats| {
                        stats.total_won += payout;
                        stats.games_won += 1;
                        self.update_leaderboard(winner, stats.total_won - stats.total_bet);
                    });
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    fn get_state(&self) -> GameView {
        GameView {
            games: self.games.clone(),
            player_stats: self.player_stats.clone(),
            leaderboard: self.get_leaderboard_view(),
        }
    }
}

/// Projection runner that keeps projections up to date
pub struct ProjectionRunner<P: Projection> {
    projection: Arc<RwLock<P>>,
    event_store: Arc<dyn EventStore<Event = P::Event>>,
    last_sequence: AtomicU64,
}

impl<P: Projection> ProjectionRunner<P> {
    pub async fn run(&self) {
        let mut event_stream = self.event_store.subscribe().await;
        
        while let Some(stored_event) = event_stream.next().await {
            // Update projection
            let mut projection = self.projection.write().await;
            if let Err(e) = projection.apply_event(&stored_event.event) {
                error!("Projection error: {}", e);
                // Decide whether to retry, skip, or halt
            }
            
            // Update checkpoint
            self.last_sequence.store(
                stored_event.sequence_number,
                Ordering::SeqCst
            );
        }
    }
    
    pub async fn rebuild(&self) -> Result<(), ProjectionError> {
        // Reset projection
        self.projection.write().await.reset();
        
        // Replay all events
        let events = self.event_store
            .get_events_by_time(DateTime::MIN_UTC, Utc::now())
            .await?;
        
        let mut projection = self.projection.write().await;
        for event in events {
            projection.apply_event(&event.event)?;
        }
        
        Ok(())
    }
}
```

## Audit Logging: Compliance and Security

Audit logs track who did what when:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub user_id: String,
    pub action: AuditAction,
    pub resource: String,
    pub result: AuditResult,
    pub details: serde_json::Value,
    pub ip_address: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
    PermissionGrant,
    PermissionRevoke,
    ConfigChange,
    SecurityEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failure { reason: String },
    PartialSuccess { details: String },
}

pub struct AuditLogger {
    store: Arc<dyn AuditStore>,
    encryption_key: Option<Vec<u8>>,
    retention_policy: RetentionPolicy,
}

impl AuditLogger {
    pub async fn log(&self, entry: AuditLog) -> Result<(), AuditError> {
        // Validate entry
        self.validate_entry(&entry)?;
        
        // Encrypt sensitive fields if configured
        let entry = if let Some(key) = &self.encryption_key {
            self.encrypt_entry(entry, key)?
        } else {
            entry
        };
        
        // Store with retry logic
        let mut attempts = 0;
        loop {
            match self.store.append(entry.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) if attempts < 3 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100 * attempts)).await;
                }
                Err(e) => {
                    // Log to fallback location
                    self.log_to_fallback(&entry)?;
                    return Err(e);
                }
            }
        }
    }
    
    pub async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditLog>, AuditError> {
        // Check permissions
        self.check_query_permission(&filter)?;
        
        // Apply retention policy
        let filter = self.apply_retention_filter(filter);
        
        // Query store
        let results = self.store.query(filter).await?;
        
        // Decrypt if needed
        if let Some(key) = &self.encryption_key {
            results.into_iter()
                .map(|entry| self.decrypt_entry(entry, key))
                .collect()
        } else {
            Ok(results)
        }
    }
}

// Tamper-proof audit log using blockchain-style hashing
pub struct TamperProofAuditLog {
    entries: Vec<AuditBlock>,
    current_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuditBlock {
    pub index: u64,
    pub timestamp: DateTime<Utc>,
    pub entry: AuditLog,
    pub previous_hash: [u8; 32],
    pub hash: [u8; 32],
    pub signature: Option<Vec<u8>>,  // Digital signature for non-repudiation
}

impl TamperProofAuditLog {
    pub fn append(&mut self, entry: AuditLog, private_key: Option<&[u8]>) -> Result<(), AuditError> {
        let block = AuditBlock {
            index: self.entries.len() as u64,
            timestamp: Utc::now(),
            entry,
            previous_hash: self.current_hash,
            hash: [0; 32],  // Will calculate
            signature: None,
        };
        
        // Calculate hash
        let mut hasher = Sha256::new();
        hasher.update(&block.index.to_le_bytes());
        hasher.update(block.timestamp.to_rfc3339().as_bytes());
        hasher.update(&serde_json::to_vec(&block.entry)?);
        hasher.update(&block.previous_hash);
        
        let hash = hasher.finalize().into();
        let mut block = block;
        block.hash = hash;
        
        // Sign if key provided
        if let Some(key) = private_key {
            block.signature = Some(self.sign_block(&block, key)?);
        }
        
        self.entries.push(block);
        self.current_hash = hash;
        
        Ok(())
    }
    
    pub fn verify_integrity(&self) -> Result<bool, AuditError> {
        if self.entries.is_empty() {
            return Ok(true);
        }
        
        let mut previous_hash = [0; 32];
        
        for block in &self.entries {
            // Verify hash chain
            if block.previous_hash != previous_hash {
                return Ok(false);
            }
            
            // Verify block hash
            let calculated_hash = self.calculate_hash(block)?;
            if calculated_hash != block.hash {
                return Ok(false);
            }
            
            // Verify signature if present
            if let Some(signature) = &block.signature {
                if !self.verify_signature(block, signature)? {
                    return Ok(false);
                }
            }
            
            previous_hash = block.hash;
        }
        
        Ok(true)
    }
}
```

## Time-Travel Debugging

The ability to reconstruct any past state is invaluable for debugging:

```rust
pub struct TimeMachine {
    event_store: Arc<dyn EventStore<Event = GameEvent>>,
    snapshot_store: Arc<dyn SnapshotStore>,
}

impl TimeMachine {
    /// Reconstruct state at a specific point in time
    pub async fn travel_to(&self, timestamp: DateTime<Utc>) -> Result<GameState, TimeError> {
        // Find nearest snapshot before timestamp
        let snapshot = self.snapshot_store
            .find_before(timestamp)
            .await?;
        
        let mut state = if let Some((snapshot_time, snapshot_state)) = snapshot {
            // Start from snapshot
            snapshot_state
        } else {
            // Start from beginning
            GameState::default()
        };
        
        // Replay events from snapshot to target time
        let events = self.event_store
            .get_events_by_time(
                snapshot.map(|(t, _)| t).unwrap_or(DateTime::MIN_UTC),
                timestamp,
            )
            .await?;
        
        for event in events {
            state.apply_event(&event.event)?;
        }
        
        Ok(state)
    }
    
    /// Find when a condition first became true
    pub async fn find_when<F>(&self, condition: F) -> Result<Option<DateTime<Utc>>, TimeError>
    where
        F: Fn(&GameState) -> bool,
    {
        let mut state = GameState::default();
        let events = self.event_store
            .get_events_by_time(DateTime::MIN_UTC, Utc::now())
            .await?;
        
        for event in events {
            state.apply_event(&event.event)?;
            if condition(&state) {
                return Ok(Some(event.metadata.timestamp));
            }
        }
        
        Ok(None)
    }
    
    /// Replay events with inspection
    pub async fn debug_replay<F>(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        mut inspector: F,
    ) -> Result<(), TimeError>
    where
        F: FnMut(&GameState, &StoredEvent<GameEvent>),
    {
        let events = self.event_store.get_events_by_time(from, to).await?;
        let mut state = self.travel_to(from).await?;
        
        for event in events {
            println!("┌─ Event: {} at {}", event.event.event_type(), event.metadata.timestamp);
            println!("│  ID: {}", event.metadata.event_id);
            
            let state_before = state.clone();
            state.apply_event(&event.event)?;
            
            // Let inspector examine the transition
            inspector(&state, &event);
            
            // Show what changed
            self.print_state_diff(&state_before, &state);
            
            println!("└─────────────────────────────────");
        }
        
        Ok(())
    }
    
    fn print_state_diff(&self, before: &GameState, after: &GameState) {
        // Compare and print differences
        if before.pot != after.pot {
            println!("│  Pot: {} → {}", before.pot, after.pot);
        }
        if before.phase != after.phase {
            println!("│  Phase: {:?} → {:?}", before.phase, after.phase);
        }
        // ... more comparisons
    }
}

// Interactive debugging session
pub struct DebugSession {
    time_machine: Arc<TimeMachine>,
    current_time: DateTime<Utc>,
    breakpoints: Vec<EventBreakpoint>,
}

impl DebugSession {
    pub async fn run_interactive(&mut self) {
        loop {
            print!("(timedb) ");
            std::io::stdout().flush().unwrap();
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            
            let parts: Vec<&str> = input.trim().split_whitespace().collect();
            
            match parts.get(0) {
                Some(&"step") => self.step_forward().await,
                Some(&"back") => self.step_backward().await,
                Some(&"goto") => {
                    if let Some(time_str) = parts.get(1) {
                        self.goto_time(time_str).await;
                    }
                }
                Some(&"watch") => {
                    if let Some(expr) = parts.get(1) {
                        self.add_watch(expr);
                    }
                }
                Some(&"break") => {
                    if let Some(event_type) = parts.get(1) {
                        self.add_breakpoint(event_type);
                    }
                }
                Some(&"continue") => self.continue_to_breakpoint().await,
                Some(&"print") => self.print_current_state().await,
                Some(&"quit") => break,
                _ => println!("Unknown command"),
            }
        }
    }
}
```

## Snapshot Management

Snapshots speed up state reconstruction:

```rust
pub struct SnapshotManager {
    store: Arc<dyn SnapshotStore>,
    strategy: SnapshotStrategy,
}

#[derive(Debug, Clone)]
pub enum SnapshotStrategy {
    /// Snapshot every N events
    EventCount { threshold: u64 },
    
    /// Snapshot every time period
    TimeBased { interval: Duration },
    
    /// Snapshot when reconstruction would be slow
    PerformanceBased { max_events_to_replay: u64 },
    
    /// Hybrid approach
    Adaptive {
        min_interval: Duration,
        max_interval: Duration,
        event_threshold: u64,
    },
}

impl SnapshotManager {
    pub async fn maybe_snapshot(
        &self,
        aggregate_id: Uuid,
        current_version: u64,
        state: &impl Serialize,
    ) -> Result<bool, SnapshotError> {
        let should_snapshot = match &self.strategy {
            SnapshotStrategy::EventCount { threshold } => {
                current_version % threshold == 0
            }
            
            SnapshotStrategy::TimeBased { interval } => {
                let last_snapshot = self.store.get_last_snapshot_time(aggregate_id).await?;
                match last_snapshot {
                    Some(time) => Utc::now() - time > *interval,
                    None => true,
                }
            }
            
            SnapshotStrategy::PerformanceBased { max_events_to_replay } => {
                let events_since_snapshot = self.store
                    .events_since_last_snapshot(aggregate_id)
                    .await?;
                events_since_snapshot > *max_events_to_replay
            }
            
            SnapshotStrategy::Adaptive { min_interval, max_interval, event_threshold } => {
                // Complex logic balancing multiple factors
                self.adaptive_decision(aggregate_id, current_version).await?
            }
        };
        
        if should_snapshot {
            self.store.save_snapshot(aggregate_id, current_version, state).await?;
            
            // Clean up old snapshots
            self.cleanup_old_snapshots(aggregate_id).await?;
            
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    async fn cleanup_old_snapshots(&self, aggregate_id: Uuid) -> Result<(), SnapshotError> {
        // Keep only the last N snapshots
        const MAX_SNAPSHOTS: usize = 10;
        
        let snapshots = self.store.list_snapshots(aggregate_id).await?;
        
        if snapshots.len() > MAX_SNAPSHOTS {
            let to_delete = snapshots.len() - MAX_SNAPSHOTS;
            for snapshot in snapshots.iter().take(to_delete) {
                self.store.delete_snapshot(snapshot.id).await?;
            }
        }
        
        Ok(())
    }
}
```

## CQRS: Command Query Responsibility Segregation

Separate write and read models for optimal performance:

```rust
/// Commands modify state
pub trait Command: Send + Sync {
    type Aggregate: Aggregate;
    type Event: Event;
    type Error: std::error::Error;
    
    fn validate(&self, state: &Self::Aggregate) -> Result<(), Self::Error>;
    fn execute(self, state: &Self::Aggregate) -> Result<Vec<Self::Event>, Self::Error>;
}

/// Queries read state
pub trait Query: Send + Sync {
    type Result: Send;
    type Error: std::error::Error;
    
    fn execute(&self, store: &dyn QueryStore) -> Result<Self::Result, Self::Error>;
}

/// CQRS system coordinator
pub struct CqrsSystem {
    command_bus: Arc<CommandBus>,
    query_bus: Arc<QueryBus>,
    event_store: Arc<dyn EventStore<Event = DomainEvent>>,
    projections: Vec<Arc<dyn Projection>>,
}

impl CqrsSystem {
    pub async fn handle_command<C: Command>(&self, command: C) -> Result<(), CqrsError> {
        // Load aggregate
        let aggregate_id = command.aggregate_id();
        let events = self.event_store.get_events(aggregate_id, 0, None).await?;
        
        let mut aggregate = C::Aggregate::default();
        for event in events {
            aggregate.apply_event(&event.event)?;
        }
        
        // Validate command
        command.validate(&aggregate)?;
        
        // Execute command to get events
        let new_events = command.execute(&aggregate)?;
        
        // Store events
        self.event_store.append_events(
            aggregate_id,
            new_events,
            Some(aggregate.version()),
        ).await?;
        
        Ok(())
    }
    
    pub async fn handle_query<Q: Query>(&self, query: Q) -> Result<Q::Result, CqrsError> {
        self.query_bus.execute(query).await
    }
}

// Example: Place bet command
pub struct PlaceBetCommand {
    pub game_id: Uuid,
    pub player_id: PlayerId,
    pub amount: u64,
}

impl Command for PlaceBetCommand {
    type Aggregate = Game;
    type Event = GameEvent;
    type Error = GameError;
    
    fn validate(&self, game: &Game) -> Result<(), GameError> {
        // Check game is accepting bets
        if game.phase != GamePhase::Betting {
            return Err(GameError::BettingClosed);
        }
        
        // Check player has sufficient balance
        if !game.player_has_balance(&self.player_id, self.amount) {
            return Err(GameError::InsufficientBalance);
        }
        
        // Check bet limits
        if self.amount < game.min_bet || self.amount > game.max_bet {
            return Err(GameError::InvalidBetAmount);
        }
        
        Ok(())
    }
    
    fn execute(self, game: &Game) -> Result<Vec<GameEvent>, GameError> {
        Ok(vec![
            GameEvent::BetPlaced {
                game_id: self.game_id,
                player: self.player_id,
                amount: self.amount,
            }
        ])
    }
}
```

## Memory Management for Infinite History

Managing unbounded history requires clever strategies:

```rust
pub struct HistoryManager {
    hot_storage: Arc<dyn EventStore>,   // Recent events (fast)
    cold_storage: Arc<dyn EventStore>,  // Old events (slow, cheap)
    archive: Arc<dyn ArchiveStore>,     // Very old events (glacial)
    config: HistoryConfig,
}

#[derive(Debug, Clone)]
pub struct HistoryConfig {
    pub hot_duration: Duration,      // Keep in hot storage
    pub warm_duration: Duration,     // Keep in cold storage
    pub compression_after: Duration, // Compress old events
    pub archive_after: Duration,     // Move to archive
    pub delete_after: Option<Duration>, // Optional deletion
}

impl HistoryManager {
    pub async fn age_events(&self) -> Result<(), HistoryError> {
        let now = Utc::now();
        
        // Move from hot to cold
        let hot_cutoff = now - self.config.hot_duration;
        let events_to_cool = self.hot_storage
            .get_events_before(hot_cutoff)
            .await?;
        
        for batch in events_to_cool.chunks(1000) {
            self.cold_storage.append_batch(batch).await?;
            self.hot_storage.delete_batch(batch.iter().map(|e| e.id)).await?;
        }
        
        // Compress cold storage
        let compress_cutoff = now - self.config.compression_after;
        self.compress_old_events(compress_cutoff).await?;
        
        // Archive very old events
        let archive_cutoff = now - self.config.archive_after;
        self.archive_events(archive_cutoff).await?;
        
        // Optional: Delete ancient events
        if let Some(delete_duration) = self.config.delete_after {
            let delete_cutoff = now - delete_duration;
            self.delete_ancient_events(delete_cutoff).await?;
        }
        
        Ok(())
    }
    
    async fn compress_old_events(&self, cutoff: DateTime<Utc>) -> Result<(), HistoryError> {
        let events = self.cold_storage.get_events_before(cutoff).await?;
        
        // Group events by aggregate for better compression
        let mut by_aggregate: HashMap<Uuid, Vec<StoredEvent>> = HashMap::new();
        for event in events {
            by_aggregate.entry(event.event.aggregate_id())
                .or_default()
                .push(event);
        }
        
        for (aggregate_id, events) in by_aggregate {
            // Serialize and compress
            let serialized = bincode::serialize(&events)?;
            let compressed = zstd::encode_all(&serialized[..], 3)?;
            
            // Store compressed
            self.cold_storage.store_compressed(aggregate_id, compressed).await?;
            
            // Delete uncompressed
            for event in events {
                self.cold_storage.delete(event.metadata.event_id).await?;
            }
        }
        
        Ok(())
    }
}
```

## Practical Exercises

### Exercise 1: Implement Event Replay
Build a system that can replay events with filters:

```rust
pub struct EventReplayer {
    // Your implementation
}

impl EventReplayer {
    async fn replay_with_filter<F>(&self, filter: F) -> Result<State, Error>
    where
        F: Fn(&Event) -> bool,
    {
        // Your task: Replay only events matching filter
        // Handle snapshots correctly
        // Maintain consistency
        todo!("Implement filtered replay")
    }
}
```

### Exercise 2: Build Compensating Transactions
Implement undo functionality using compensating events:

```rust
trait Compensatable: Event {
    fn compensate(&self) -> Option<Self>;
}

impl Compensatable for GameEvent {
    fn compensate(&self) -> Option<Self> {
        // Your task: Generate reverse event
        // Not all events can be compensated
        match self {
            GameEvent::BetPlaced { game_id, player, amount } => {
                Some(GameEvent::BetCancelled { game_id, player, amount })
            }
            _ => None
        }
    }
}
```

### Exercise 3: Create a Branch History
Like Git for your event store:

```rust
pub struct BranchingEventStore {
    // Your implementation
}

impl BranchingEventStore {
    async fn create_branch(&self, from: Uuid, name: String) -> Result<Branch, Error> {
        // Your task: Create alternate history branch
        // Allow exploring "what if" scenarios
        // Merge branches back together
        todo!("Implement event store branching")
    }
}
```

## Common Pitfalls and Solutions

### 1. Event Schema Evolution
Events are immutable, but requirements change:

```rust
// Use versioned events
#[derive(Serialize, Deserialize)]
#[serde(tag = "version")]
enum GameEventVersioned {
    #[serde(rename = "1")]
    V1(GameEventV1),
    #[serde(rename = "2")]
    V2(GameEventV2),
}

// Provide upgrade path
impl From<GameEventV1> for GameEventV2 {
    fn from(v1: GameEventV1) -> Self {
        // Upgrade logic
        GameEventV2 {
            // Map fields...
        }
    }
}
```

### 2. Eventually Consistent Projections
Projections lag behind events:

```rust
// Include version in queries
pub struct QueryResult<T> {
    pub data: T,
    pub version: u64,
    pub as_of: DateTime<Utc>,
    pub is_stale: bool,
}
```

### 3. Event Store Performance
Unbounded growth kills performance:

```rust
// Use partitioning
pub struct PartitionedEventStore {
    partitions: HashMap<String, Box<dyn EventStore>>,
    strategy: PartitionStrategy,
}

enum PartitionStrategy {
    ByAggregate,
    ByTime { interval: Duration },
    BySize { max_events: u64 },
}
```

## Conclusion: The Power of History

State history management transforms debugging from archaeology to time travel. Instead of wondering "how did we get here?", you can simply look. Instead of trying to reproduce bugs, you can replay them. Instead of losing data, you keep everything.

In BitCraps, where money and trust are on the line, this isn't just convenient - it's essential. Every bet, every roll, every payout is recorded forever. Not just the what, but the when, who, and why.

Key principles to remember:

1. **Events are facts** - They happened, they're immutable
2. **State is derived** - Current state is just where the events led us
3. **Time is a feature** - Being able to see any point in time is powerful
4. **Storage is cheap** - But organize it well or retrieval is expensive
5. **History enables features** - Undo, audit, debugging, analytics all come free

The next time you debug a production issue at 3 AM, you'll thank yourself for building a system that remembers everything.

## Additional Resources

- **Event Sourcing** by Martin Fowler - The definitive introduction
- **CQRS Journey** by Microsoft - Practical implementation guide  
- **Domain-Driven Design** by Eric Evans - The theory behind the practice
- **Building Event-Driven Microservices** - Modern patterns and practices

Remember: Those who cannot remember the past are condemned to repeat it. With proper state history management, you can remember everything.