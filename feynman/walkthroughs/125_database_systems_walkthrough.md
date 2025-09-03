# Chapter 11: Database Systems - Persistent State Management

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending

## Understanding `src/database/mod.rs`

*"Data is the new oil, but unlike oil, data is not consumed when used - it multiplies."* - Database Engineer

*"A database is just a sophisticated filing system with superpowers: it finds, sorts, and protects your data even when the power goes out."* - Storage Architect

---

## Part I: Database Systems for Complete Beginners
### A 500+ Line Journey from "What's Data Storage?" to "Production Database Management"

Let me start with a story that shows why databases matter.

In 1970, IBM researcher Edgar F. Codd was frustrated. Companies were storing data in chaotic, interconnected webs where changing one thing could break everything else. A customer's address might be stored in 17 different places, each slightly different. Adding a new field required changing every program that touched the data.

Codd proposed something revolutionary: what if we organized data into simple tables with rows and columns, like a spreadsheet? What if we had a universal language to ask questions about this data? What if the computer handled all the complex linking and optimization automatically?

This became the relational model and SQL, fundamentally changing how humanity stores and retrieves information. But to understand why our casino needs sophisticated database management, we need to start from the very beginning.

### What Is Data Storage, Really?

At its most basic, data storage is about persistence - keeping information even after the program ends:

**Without Persistence**:
```rust
let player_chips = 1000;  // Alice has 1000 chips
// Program ends
// *poof* - Alice's chips are gone forever!
```

**With Persistence**:
```rust
save_to_file("alice_chips.txt", "1000");  // Alice has 1000 chips
// Program ends, restarts later
let player_chips = read_from_file("alice_chips.txt");  // Alice still has 1000 chips!
```

But simple file storage quickly becomes inadequate as complexity grows.

### The Evolution of Data Storage

#### Era 1: Flat Files (1950s-1960s)
Early systems stored data in simple files:

```
players.txt:
Alice,1000
Bob,500
Charlie,2000
```

**Problems**:
- No data validation (Bob could have -500 chips!)
- No concurrent access (what if two people bet simultaneously?)
- No backup/recovery (file corruption = total loss)
- No relationships (how do you link bets to players?)
- No search (finding Alice requires reading entire file)

#### Era 2: Hierarchical Databases (1960s-1970s)
Organized data into tree structures:

```
Casino
├── Players
│   ├── Alice (1000 chips)
│   └── Bob (500 chips)
├── Games
│   ├── Craps Game 1
│   │   ├── Bet 1 (Alice, Pass, 100)
│   │   └── Bet 2 (Bob, Don't Pass, 50)
│   └── Craps Game 2
```

**Better, but still limited**:
- Fixed structure (hard to change)
- Complex navigation (to find Alice's bets, traverse entire tree)
- Data duplication (Alice appears in multiple places)

#### Era 3: Network Databases (1970s)
Allowed complex interconnections:

```
Player ←→ Game ←→ Bet ←→ Result
   ↑         ↑       ↑       ↑
   └─────────┼───────┼───────┘
             └───────┘
```

**Powerful but complex**:
- Required programmers to understand physical storage
- Brittle (changing relationships broke programs)
- Hard to optimize

#### Era 4: Relational Databases (1970s-Present)
Codd's breakthrough - simple tables with mathematical foundations:

**Players Table**:
```
ID | Name    | Chips
1  | Alice   | 1000
2  | Bob     | 500
3  | Charlie | 2000
```

**Bets Table**:
```
ID | PlayerID | GameID | Type      | Amount
1  | 1        | 1      | Pass      | 100
2  | 2        | 1      | Don't Pass| 50
```

**The Magic**: SQL language lets you ask complex questions:
```sql
-- Who has more than 750 chips and has placed a bet?
SELECT p.Name, p.Chips 
FROM Players p 
JOIN Bets b ON p.ID = b.PlayerID 
WHERE p.Chips > 750
```

#### Era 5: NoSQL (2000s-Present)
When web scale broke relational assumptions:

**Document Databases** (MongoDB):
```json
{
  "player": "Alice",
  "chips": 1000,
  "bets": [
    {"game": 1, "type": "Pass", "amount": 100},
    {"game": 2, "type": "Field", "amount": 25}
  ]
}
```

**Key-Value Stores** (Redis):
```
alice:chips -> 1000
alice:bets -> ["bet1", "bet2", "bet3"]
bet1 -> {"type": "Pass", "amount": 100}
```

### Database Fundamentals: The ACID Properties

For systems handling money (like our casino), databases must guarantee ACID properties:

#### Atomicity - All or Nothing
Either the entire transaction succeeds, or nothing happens:

```sql
-- Transfer 100 chips from Alice to Bob
BEGIN TRANSACTION;
  UPDATE Players SET chips = chips - 100 WHERE name = 'Alice';  -- Subtract from Alice
  UPDATE Players SET chips = chips + 100 WHERE name = 'Bob';    -- Add to Bob
COMMIT;  -- Both updates succeed, or both fail
```

**Without Atomicity**:
```
Step 1: Alice loses 100 chips ✓
Step 2: System crashes! 💥
Result: Alice lost chips, Bob gained nothing - money vanished!
```

**With Atomicity**:
```
Step 1: Alice loses 100 chips (tentative)
Step 2: System crashes! 💥
Result: Transaction rolls back - Alice keeps her chips
```

#### Consistency - Rules Are Never Broken
Database never allows invalid states:

```sql
-- Rule: Chips cannot be negative
CONSTRAINT check_chips CHECK (chips >= 0)

-- This would be rejected:
UPDATE Players SET chips = -100 WHERE name = 'Alice';  -- ERROR!
```

#### Isolation - Transactions Don't Interfere
Multiple operations can happen simultaneously without confusion:

```sql
-- Alice and Bob both try to bet their last 100 chips on the same game

-- Transaction A (Alice):
BEGIN;
  balance = SELECT chips FROM Players WHERE name = 'Alice';  -- Sees 100
  -- If balance >= 100, place bet
COMMIT;

-- Transaction B (Bob):  
BEGIN;
  balance = SELECT chips FROM Players WHERE name = 'Bob';    -- Sees 100  
  -- If balance >= 100, place bet
COMMIT;

-- Without isolation: Both see 100, both place bets, but only one should succeed!
-- With isolation: Database ensures only one succeeds
```

#### Durability - Changes Survive Crashes
Once committed, data is permanently stored:

```
Alice bets 100 chips
Database confirms: "Bet recorded" ✓
Power outage! 💥
System restarts
Alice's bet is still there - money didn't vanish
```

### The CAP Theorem: Fundamental Trade-offs

In distributed systems (like our P2P casino), you can only guarantee 2 out of 3:

#### Consistency
All nodes see the same data at the same time
```
Node A: Alice has 900 chips
Node B: Alice has 900 chips  ← Same value
Node C: Alice has 900 chips
```

#### Availability
System remains operational even if some nodes fail
```
Node A: ✓ Responding
Node B: ✗ Failed
Node C: ✓ Still serving requests
System: Still working!
```

#### Partition Tolerance
System continues to operate despite network failures
```
Network split:
[Node A] ←→ [Node B]    [Node C] ←→ [Node D]
     Group 1                Group 2
Both groups keep working despite split
```

**The Trade-off**:
- **CA Systems**: Traditional databases (MySQL, PostgreSQL) - not suitable for distributed networks
- **CP Systems**: Strong consistency, but may become unavailable during partitions
- **AP Systems**: Always available, but data may be temporarily inconsistent

Our casino chooses **CP** - we'd rather pause briefly than allow double-spending!

### Database Architecture Components

#### Storage Engine - How Data Lives on Disk

**B-Trees**: The foundation of most databases
```
Root:      [G]
          /   \
Level 1: [C]   [M,S]
        / |     | | \
Leaves:[A,B][D,E,F][H,I,J][N,O,P][T,U,V]
```

Why B-Trees?
- **Sorted**: Easy to find ranges (all players with 500-1000 chips)
- **Balanced**: Every search takes the same time
- **Page-friendly**: Each node fits in one disk page

**LSM Trees**: For write-heavy workloads
```
Memory:    [New writes go here - fast!]
           ↓ (when full)
Level 0:   [Recent data - unsorted]
           ↓ (compact periodically)  
Level 1:   [Sorted, merged data]
           ↓
Level 2:   [More sorted, merged data]
```

Benefits:
- Extremely fast writes (append-only)
- Good for high-throughput systems
- Used in Cassandra, LevelDB, RocksDB

#### Query Engine - How Questions Get Answered

**Query Planning**:
```sql
SELECT p.name, SUM(b.amount)
FROM players p 
JOIN bets b ON p.id = b.player_id 
WHERE p.chips > 1000
GROUP BY p.name
```

**Optimizer's Job**:
1. **Parse**: Understand the SQL
2. **Plan**: Find efficient execution strategy
3. **Execute**: Run the plan

**Multiple strategies possible**:
```
Strategy 1: Nested loops (slow for big tables)
FOR each player WHERE chips > 1000:
    FOR each bet WHERE player_id = player.id:
        Add to sum

Strategy 2: Hash join (much faster)
1. Build hash table of rich players (chips > 1000)  
2. Scan bets, lookup in hash table
3. Accumulate sums
```

#### Transaction Manager - ACID Guarantees

**Write-Ahead Logging (WAL)**:
```
1. Alice bets 100 chips
2. FIRST: Write "Alice bet 100" to log file
3. THEN: Update database pages
4. If crash occurs, replay log on restart
```

**Lock Management**:
```rust
// Pessimistic locking
let mut alice_account = db.lock_account("Alice");  // Block other transactions
alice_account.chips -= 100;
alice_account.save();
// Lock released

// Optimistic locking  
let alice_v1 = db.get_account("Alice");  // Version 1
alice_v1.chips -= 100;
db.update_if_version(alice_v1, version=1);  // Fails if someone else updated
```

### SQL vs NoSQL: When to Use What

#### SQL Databases (PostgreSQL, MySQL, SQLite)

**Best For**:
- Financial data (ACID critical)
- Complex relationships
- Ad-hoc queries
- Reporting and analytics
- Mature ecosystem

**Our Casino Uses SQL Because**:
- Money requires ACID guarantees
- Complex relationships (players ↔ games ↔ bets ↔ results)
- Audit requirements (complex queries for compliance)
- Proven reliability

#### NoSQL Databases

**Document Stores** (MongoDB, CouchDB):
```json
{
  "_id": "game_123",
  "type": "craps",
  "players": [
    {"name": "Alice", "bets": [...]},
    {"name": "Bob", "bets": [...]}
  ],
  "results": {...}
}
```

**Best For**: Flexible schemas, rapid development, object-like data

**Key-Value Stores** (Redis, DynamoDB):
```
player:alice:chips -> 1000
player:alice:games -> ["game1", "game2", "game3"]
game:123:state -> "waiting_for_players"
```

**Best For**: Caching, sessions, simple lookups, high performance

**Graph Databases** (Neo4j, Amazon Neptune):
```
(Alice)-[BET]->(Game1)-[RESULT]->(Win)
  |                |
  [FRIEND]-------(Bob)-[BET]->(Game1)
```

**Best For**: Social networks, recommendation engines, fraud detection

**Time Series** (InfluxDB, TimescaleDB):
```
timestamp           | player | action  | chips
2023-10-15 14:30:00 | Alice  | bet     | 100
2023-10-15 14:30:01 | Bob    | bet     | 50
2023-10-15 14:30:05 | Alice  | win     | 200
```

**Best For**: Metrics, monitoring, IoT data, financial time series

### Performance and Optimization

#### Indexing - Making Searches Fast

**Without Index**:
```sql
SELECT * FROM players WHERE name = 'Alice';
-- Database must scan EVERY row! O(n) time
-- With 1 million players, checks all 1 million rows
```

**With Index**:
```sql
CREATE INDEX idx_player_name ON players(name);
SELECT * FROM players WHERE name = 'Alice';
-- Database uses index to jump directly to Alice! O(log n) time
-- With 1 million players, checks only ~20 rows (log₂ 1,000,000 ≈ 20)
```

**Types of Indexes**:

**B-Tree Index** (most common):
- Good for: equality, ranges, sorting
- Example: `WHERE chips BETWEEN 500 AND 1500`

**Hash Index**:
- Good for: exact matches only
- Example: `WHERE player_id = 123`

**Bitmap Index**:
- Good for: low-cardinality data (few unique values)
- Example: `WHERE game_type = 'craps'`

**Full-Text Index**:
- Good for: searching text content
- Example: `WHERE chat_message CONTAINS 'good luck'`

#### Query Optimization Techniques

**1. Use EXPLAIN to Understand Plans**:
```sql
EXPLAIN SELECT p.name, SUM(b.amount)
FROM players p 
JOIN bets b ON p.id = b.player_id 
WHERE p.chips > 1000
GROUP BY p.name;

-- Output shows:
-- 1. Index scan on players (chips > 1000)  
-- 2. Nested loop join with bets
-- 3. Hash aggregation for GROUP BY
-- Cost: 1,234.56 (lower is better)
```

**2. Avoid Common Anti-Patterns**:

```sql
-- BAD: Function on indexed column
SELECT * FROM players WHERE UPPER(name) = 'ALICE';

-- GOOD: Store uppercase version or use case-insensitive collation
SELECT * FROM players WHERE name = 'Alice';

-- BAD: Leading wildcard prevents index use
SELECT * FROM players WHERE name LIKE '%lice';  

-- GOOD: Trailing wildcard can use index
SELECT * FROM players WHERE name LIKE 'Ali%';
```

**3. Choose Right Data Types**:
```sql
-- BAD: UUID as VARCHAR (36 bytes, slower comparisons)
CREATE TABLE players (id VARCHAR(36), ...);

-- GOOD: UUID as BINARY (16 bytes, faster comparisons)
CREATE TABLE players (id BINARY(16), ...);

-- BAD: Storing chips as DECIMAL for exact precision, but slow
chips DECIMAL(10,2)

-- GOOD: Store chips as INTEGER (cents/micro-units), fast math
chips BIGINT  -- 1 chip = 1,000,000 micro-chips
```

#### Scaling Strategies

**Vertical Scaling (Scale Up)**:
- Bigger machine: more CPU, RAM, faster SSD
- Pros: Simple, no code changes
- Cons: Expensive, single point of failure, limited ceiling

**Horizontal Scaling (Scale Out)**:

**Read Replicas**:
```
Master DB (writes) → Replica 1 (reads)
                 → Replica 2 (reads)  
                 → Replica 3 (reads)
```
- Distribute read load across multiple servers
- Eventual consistency (replicas may lag slightly)

**Sharding**:
```
Players A-H → Shard 1
Players I-P → Shard 2  
Players Q-Z → Shard 3
```
- Split data across multiple databases
- Complex application logic
- Cross-shard queries are difficult

**Federation**:
```
Users DB     ← User management
Games DB     ← Game state
Analytics DB ← Historical data  
```
- Split by feature/domain
- Each service owns its data

### Database Security

#### Authentication and Authorization

**Database Users**:
```sql
-- Create application user with limited permissions
CREATE USER 'casino_app'@'localhost' IDENTIFIED BY 'strong_password';

-- Grant only necessary permissions
GRANT SELECT, INSERT, UPDATE ON casino.players TO 'casino_app'@'localhost';
GRANT SELECT, INSERT ON casino.bets TO 'casino_app'@'localhost';

-- Never grant:
-- DROP, ALTER, DELETE (except where needed)
-- SUPER, FILE, PROCESS (administrative privileges)
```

**Application-Level Security**:
```rust
// Use parameterized queries to prevent SQL injection
let stmt = conn.prepare("SELECT chips FROM players WHERE id = ?1")?;
let chips: i64 = stmt.query_row([player_id], |row| row.get(0))?;

// NEVER do this (vulnerable to injection):
// let query = format!("SELECT chips FROM players WHERE id = {}", player_id);
```

#### Encryption

**At Rest** (data on disk):
```
Disk: [encrypted_database_file.db]
      ↓ (with key)
Memory: [readable data for queries]
```

**In Transit** (data over network):
```
Application ←[TLS/SSL]→ Database Server
```

**Transparent Data Encryption**:
- Database handles encryption/decryption automatically
- Application code unchanged
- Keys managed by database or external key manager

#### Backup and Recovery

**Backup Types**:

**Full Backup**:
- Complete copy of entire database
- Takes longest time
- Restoration is simple

**Incremental Backup**:
- Only changes since last backup
- Faster to create
- Longer to restore (need full + all incrementals)

**Differential Backup**:
- Changes since last full backup
- Medium time to create/restore
- Need full + latest differential

**Point-in-Time Recovery**:
```
Full Backup    Incremental    Incremental    CRASH!
(Sunday)       (Monday)       (Tuesday)      (Wednesday)
     ↓              ↓              ↓              ↓
[Complete DB]  [Changes]      [Changes]      [Lost data]

Recovery plan:
1. Restore Sunday's full backup
2. Apply Monday's changes  
3. Apply Tuesday's changes
4. Replay Wednesday's transaction log up to crash point
```

### Common Database Mistakes

#### Mistake 1: Not Planning for Scale

```sql
-- BAD: Will be slow when table grows
CREATE TABLE game_history (
    id SERIAL PRIMARY KEY,
    player_name VARCHAR(50),    -- No index!
    game_date DATE,             -- No index!
    bet_amount INTEGER
);

-- GOOD: Indexes and partitioning from the start
CREATE TABLE game_history (
    id SERIAL PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES players(id),
    game_date DATE NOT NULL,
    bet_amount INTEGER NOT NULL
) PARTITION BY RANGE (game_date);

CREATE INDEX idx_game_history_player ON game_history(player_id);
CREATE INDEX idx_game_history_date ON game_history(game_date);
```

#### Mistake 2: Ignoring Database Normalization

```sql
-- BAD: Denormalized, redundant data
CREATE TABLE bets (
    id INTEGER PRIMARY KEY,
    player_name VARCHAR(50),     -- Duplicated data
    player_email VARCHAR(100),   -- What if Alice changes email?
    player_phone VARCHAR(20),    -- Stored everywhere Alice has bets!
    bet_type VARCHAR(20),
    amount INTEGER
);

-- GOOD: Normalized, single source of truth
CREATE TABLE players (
    id INTEGER PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    phone VARCHAR(20)
);

CREATE TABLE bets (
    id INTEGER PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES players(id),
    bet_type VARCHAR(20) NOT NULL,
    amount INTEGER NOT NULL
);
```

#### Mistake 3: Poor Connection Management

```rust
// BAD: New connection per operation (expensive!)
async fn place_bet(bet: &Bet) -> Result<()> {
    let conn = Database::connect().await?;  // Slow!
    conn.execute("INSERT INTO bets ...", &bet).await?;
    conn.close().await?;  // Wasteful!
    Ok(())
}

// GOOD: Connection pooling
lazy_static! {
    static ref DB_POOL: DatabasePool = DatabasePool::new(config).unwrap();
}

async fn place_bet(bet: &Bet) -> Result<()> {
    DB_POOL.with_connection(|conn| {  // Reuse connections
        conn.execute("INSERT INTO bets ...", &bet)
    }).await
}
```

#### Mistake 4: Not Using Transactions for Related Operations

```rust
// BAD: Non-atomic operations
async fn transfer_chips(from: &str, to: &str, amount: i64) -> Result<()> {
    // What if crash happens between these operations?
    db.execute("UPDATE players SET chips = chips - ? WHERE name = ?", &[amount, from]).await?;
    db.execute("UPDATE players SET chips = chips + ? WHERE name = ?", &[amount, to]).await?;
    Ok(())
}

// GOOD: Atomic transaction
async fn transfer_chips(from: &str, to: &str, amount: i64) -> Result<()> {
    db.transaction(|tx| {
        tx.execute("UPDATE players SET chips = chips - ? WHERE name = ?", &[amount, from])?;
        tx.execute("UPDATE players SET chips = chips + ? WHERE name = ?", &[amount, to])?;
        Ok(())  // Both succeed or both fail
    }).await
}
```

### The BitCraps Database Strategy

Our distributed casino needs a database that provides:

1. **ACID Guarantees**: Money requires absolute consistency
2. **Performance**: Handle hundreds of concurrent bets
3. **Reliability**: Automatic backups and corruption detection
4. **Scalability**: Connection pooling and efficient queries
5. **Security**: Encrypted storage and access control
6. **Observability**: Statistics and health monitoring

We choose **SQLite with WAL mode** because:
- **ACID compliance**: Full transaction support
- **Zero configuration**: No separate server process
- **Reliability**: Most tested database in the world
- **Performance**: Fast for our use case (single-node)
- **Portability**: Works everywhere Rust works

For future scaling, we can migrate to PostgreSQL or implement distributed consensus.

---

## Part II: The Code - Complete Walkthrough

Now let's see how BitCraps implements these database concepts in real Rust code, creating a production-grade data management system for our casino.

### The Database Architecture Overview

BitCraps implements a sophisticated database layer with several key components:

```rust
// Lines 31-39
pub struct DatabasePool {
    connections: Arc<RwLock<Vec<DatabaseConnection>>>,  // Connection pool
    config: DatabaseConfig,                             // Configuration
    backup_manager: Arc<BackupManager>,                 // Automated backups
    health_monitor: Arc<HealthMonitor>,                 // Health checking
    shutdown: Arc<AtomicBool>,                          // Graceful shutdown
    background_handles: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>, // Background tasks
}
```

This architecture provides:
- **Connection Pooling**: Reuse expensive database connections
- **Automated Backups**: Regular data protection
- **Health Monitoring**: Detect and handle corruption
- **Graceful Shutdown**: Clean resource cleanup
- **Background Tasks**: Non-blocking maintenance

### Database Connection Management

```rust
// Lines 41-48
pub struct DatabaseConnection {
    conn: Connection,           // The actual SQLite connection
    in_use: bool,              // Prevents concurrent use
    created_at: Instant,       // Age tracking for cleanup
    last_used: Instant,        // Idle detection
    transaction_count: u64,    // Usage statistics
}
```

**Why Track Connection Metadata?**

1. **in_use**: Prevents race conditions where two operations try to use the same connection
2. **created_at/last_used**: Enable connection lifecycle management
3. **transaction_count**: Provides usage statistics for optimization

### Production-Grade Connection Creation

```rust
// Lines 123-151
fn create_connection(config: &DatabaseConfig) -> Result<Connection> {
    let conn = Connection::open(&config.url)
        .map_err(|e| Error::Database(format!("Failed to open database: {}", e)))?;
    
    // Enable WAL mode for better concurrency
    if config.enable_wal {
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))
            .map_err(|e| Error::Database(format!("Failed to enable WAL: {}", e)))?;
    }
    
    // Set optimal pragmas
    conn.execute("PRAGMA synchronous = NORMAL", [])
        .map_err(|e| Error::Database(format!("Failed to set synchronous: {}", e)))?;
    
    conn.execute("PRAGMA cache_size = -64000", []) // 64MB cache
        .map_err(|e| Error::Database(format!("Failed to set cache size: {}", e)))?;
    
    conn.execute("PRAGMA temp_store = MEMORY", [])
        .map_err(|e| Error::Database(format!("Failed to set temp store: {}", e)))?;
    
    conn.execute("PRAGMA mmap_size = 268435456", []) // 256MB mmap
        .map_err(|e| Error::Database(format!("Failed to set mmap size: {}", e)))?;
    
    // Set busy timeout
    conn.busy_timeout(Duration::from_secs(30))
        .map_err(|e| Error::Database(format!("Failed to set busy timeout: {}", e)))?;
}
```

**SQLite Optimization Explained**:

**WAL Mode (Write-Ahead Logging)**:
```
Traditional: [Read] → [Lock] → [Write] → [Unlock]
WAL Mode:    [Read] → [Write to WAL] → [Background merge]
```
- Writers don't block readers
- Multiple readers can work simultaneously
- Better concurrency for our multi-user casino

**Performance Pragmas**:
- `synchronous = NORMAL`: Good balance of safety and speed
- `cache_size = -64000`: 64MB in-memory cache for hot data
- `temp_store = MEMORY`: Keep temporary data in RAM
- `mmap_size = 268MB`: Memory-mapped I/O for large databases
- `busy_timeout = 30s`: Wait up to 30s for locked database

### The Connection Pool Implementation

```rust
// Lines 166-238
pub async fn with_connection<F, R>(&self, f: F) -> Result<R>
where
    F: FnOnce(&mut Connection) -> Result<R>,
{
    let start = Instant::now();
    let timeout = self.config.connection_timeout;
    
    loop {
        {
            let mut connections = self.connections.write().await;
            
            // Find an available connection
            for conn in connections.iter_mut() {
                if !conn.in_use {
                    // Check if connection is still valid
                    if conn.created_at.elapsed() > self.config.idle_timeout {
                        // Recreate stale connection
                        match Self::create_connection(&self.config) {
                            Ok(new_conn) => {
                                conn.conn = new_conn;
                                conn.created_at = Instant::now();
                            }
                            Err(e) => {
                                log::warn!("Failed to recreate connection: {}", e);
                                continue;
                            }
                        }
                    }
                    
                    conn.in_use = true;
                    conn.last_used = Instant::now();
                    conn.transaction_count += 1;
                    
                    // Execute the operation
                    let result = f(&mut conn.conn);
                    
                    // Mark connection as available
                    conn.in_use = false;
                    
                    return result;
                }
            }
            
            // Try to create a new connection if under limit
            if connections.len() < self.config.max_connections as usize {
                match Self::create_connection(&self.config) {
                    Ok(new_conn) => {
                        connections.push(DatabaseConnection {
                            conn: new_conn,
                            in_use: false,
                            created_at: Instant::now(),
                            last_used: Instant::now(),
                            transaction_count: 0,
                        });
                        continue;
                    }
                    Err(e) => {
                        log::warn!("Failed to create new connection: {}", e);
                    }
                }
            }
        }
        
        // Check timeout
        if start.elapsed() > timeout {
            return Err(Error::Database("Connection pool timeout".to_string()));
        }
        
        // Wait briefly before retrying
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
```

**Connection Pool Algorithm**:

1. **Try to find available connection**: O(n) scan through pool
2. **Validate connection health**: Check if connection is stale
3. **Mark as in-use**: Prevent concurrent access
4. **Execute operation**: Run user's database code
5. **Return connection**: Mark as available again
6. **Handle pool exhaustion**: Create new connection if possible
7. **Timeout protection**: Don't wait forever

### Transaction Management with ACID Guarantees

```rust
// Lines 240-261
pub async fn transaction<F, R>(&self, f: F) -> Result<R>
where
    F: FnOnce(&rusqlite::Transaction) -> Result<R>,
{
    self.with_connection(|conn| {
        let tx = conn.transaction()
            .map_err(|e| Error::Database(format!("Failed to begin transaction: {}", e)))?;
        
        match f(&tx) {
            Ok(result) => {
                tx.commit()
                    .map_err(|e| Error::Database(format!("Failed to commit: {}", e)))?;
                Ok(result)
            }
            Err(e) => {
                // Transaction automatically rolls back on drop
                Err(e)
            }
        }
    }).await
}
```

**Transaction Safety**:
- **Automatic rollback**: If any error occurs, transaction rolls back
- **Explicit commit**: Success requires explicit commit
- **RAII pattern**: Rust's ownership ensures cleanup

**Usage Example**:
```rust
// Transfer chips between players atomically
db.transaction(|tx| {
    tx.execute("UPDATE players SET chips = chips - ? WHERE id = ?", 
               [amount, from_player])?;
    tx.execute("UPDATE players SET chips = chips + ? WHERE id = ?", 
               [amount, to_player])?;
    Ok(())  // Both updates succeed or both fail
}).await?;
```

### Automated Backup Management

```rust
// Lines 296-362
pub struct BackupManager {
    backup_dir: PathBuf,           // Where to store backups
    backup_interval: Duration,      // How often to backup
    last_backup: Arc<RwLock<Instant>>, // Track backup timing
    _retention_days: u32,           // How long to keep backups
}

impl BackupManager {
    pub async fn run_backup(&self) -> Result<()> {
        // Create backup directory if it doesn't exist
        std::fs::create_dir_all(&self.backup_dir)
            .map_err(Error::Io)?;
        
        // Generate backup filename with timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_file = self.backup_dir.join(format!("backup_{}.db", timestamp));
        
        // Perform actual backup using file copy
        if let Ok(db_path) = std::env::var("DATABASE_URL") {
            if Path::new(&db_path).exists() {
                std::fs::copy(&db_path, &backup_file)
                    .map_err(|e| Error::Database(format!("Backup failed: {}", e)))?;
                
                log::info!("Database backup created: {:?}", backup_file);
                
                // Clean up old backups
                self.cleanup_old_backups().await?;
            }
        }
        
        // Update last backup time
        *self.last_backup.write().await = Instant::now();
        
        Ok(())
    }
    
    async fn cleanup_old_backups(&self) -> Result<()> {
        let retention_days = self._retention_days as i64;
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
        
        if let Ok(entries) = std::fs::read_dir(&self.backup_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                        if modified_time < cutoff {
                            let _ = std::fs::remove_file(entry.path());
                            log::info!("Removed old backup: {:?}", entry.path());
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}
```

**Backup Strategy**:
1. **Timestamped files**: `backup_20231015_143022.db`
2. **Automatic cleanup**: Remove backups older than retention period
3. **Non-blocking**: Runs in background thread
4. **Error handling**: Continue operation even if backup fails

### Health Monitoring and Corruption Detection

```rust
// Lines 365-395
pub struct HealthMonitor {
    last_check: Arc<RwLock<Instant>>,
    _check_interval: Duration,
    corruption_detected: Arc<RwLock<bool>>,
    _total_transactions: Arc<RwLock<u64>>,
    _failed_transactions: Arc<RwLock<u64>>,
}

impl HealthMonitor {
    async fn check_health(&self) -> Result<()> {
        *self.last_check.write().await = Instant::now();
        
        let corruption_detected = *self.corruption_detected.read().await;
        
        if corruption_detected {
            Err(Error::Database("Database corruption detected".to_string()))
        } else {
            Ok(())
        }
    }
    
    /// Check if a connection is healthy
    pub async fn check_connection(&self, conn: &mut Connection) -> bool {
        conn.execute("SELECT 1", []).is_ok()
    }
}
```

**Health Monitoring Features**:
- **Simple health check**: `SELECT 1` query to verify connection
- **Corruption detection**: Track database integrity
- **Statistics tracking**: Monitor transaction success/failure rates

### Background Task Management

```rust
// Lines 398-456
async fn start_background_tasks(&self) {
    let mut handles = self.background_handles.write().await;
    
    // Start backup task
    if self.config.backup_interval > Duration::ZERO {
        let backup_manager = self.backup_manager.clone();
        let shutdown = self.shutdown.clone();
        let interval = self.config.backup_interval;
        
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            while !shutdown.load(Ordering::Relaxed) {
                ticker.tick().await;
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                let _ = backup_manager.run_backup().await;
            }
        });
        handles.push(handle);
    }
    
    // Start health monitoring task
    let health_monitor = self.health_monitor.clone();
    let shutdown = self.shutdown.clone();
    let check_interval = self.config.checkpoint_interval;
    
    let handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(check_interval);
        while !shutdown.load(Ordering::Relaxed) {
            ticker.tick().await;
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let _ = health_monitor.check_health().await;
        }
    });
    handles.push(handle);
}
```

**Background Tasks**:
1. **Backup task**: Runs periodically based on configuration
2. **Health monitoring**: Checks database health at intervals
3. **Graceful shutdown**: Uses atomic boolean for clean exit
4. **Task tracking**: Store handles for proper cleanup

### Database Statistics and Observability

```rust
// Lines 280-294
pub async fn get_stats(&self) -> Result<DatabaseStats> {
    let connections = self.connections.read().await;
    let active = connections.iter().filter(|c| c.in_use).count();
    let total = connections.len();
    let total_transactions: u64 = connections.iter().map(|c| c.transaction_count).sum();
    
    Ok(DatabaseStats {
        active_connections: active,
        total_connections: total,
        total_transactions,
        corrupted: *self.health_monitor.corruption_detected.read().await,
    })
}
```

**Observable Metrics**:
- **Connection usage**: How many connections are active
- **Transaction volume**: Total transactions processed
- **Health status**: Is the database corrupted?
- **Pool efficiency**: Total vs active connections

### Production Database Operations

```rust
// Lines 271-278
pub async fn checkpoint(&self) -> Result<()> {
    self.with_connection(|conn| {
        conn.execute("PRAGMA wal_checkpoint(TRUNCATE)", [])
            .map_err(|e| Error::Database(format!("Checkpoint failed: {}", e)))?;
        Ok(())
    }).await
}
```

**WAL Checkpointing**:
- **Purpose**: Merge WAL file back into main database
- **TRUNCATE mode**: Removes WAL file after merge
- **Performance**: Prevents WAL from growing too large

---

## Database Design Philosophy

The BitCraps database implementation embodies several key principles:

### 1. **Reliability Over Performance**
- ACID transactions for all money operations
- Automatic backups and corruption detection
- Connection health monitoring

### 2. **Resource Management**
- Connection pooling to limit resource usage
- Automatic cleanup of stale connections
- Graceful shutdown with proper cleanup

### 3. **Observability**
- Comprehensive statistics and metrics
- Health monitoring and alerting
- Transaction tracking and auditing

### 4. **Production Readiness**
- Background task management
- Error handling and recovery
- Configuration-driven behavior

---

## Exercises

### Exercise 1: Add Query Metrics
Implement query performance tracking:

```rust
pub struct QueryMetrics {
    slow_query_threshold: Duration,
    query_counts: HashMap<String, u64>,
    query_times: HashMap<String, Duration>,
}

impl DatabasePool {
    pub async fn with_metrics<F, R>(&self, query_name: &str, f: F) -> Result<R>
    where F: FnOnce(&mut Connection) -> Result<R>
    {
        // Track query execution time and count
        // Log slow queries above threshold
    }
}
```

### Exercise 2: Implement Read Replicas
Add read-only replicas for scaling:

```rust
pub struct DatabaseCluster {
    primary: DatabasePool,    // Write operations
    replicas: Vec<DatabasePool>, // Read operations
}

impl DatabaseCluster {
    pub async fn read<F, R>(&self, f: F) -> Result<R> {
        // Route read to least-loaded replica
    }
    
    pub async fn write<F, R>(&self, f: F) -> Result<R> {
        // Route write to primary
    }
}
```

### Exercise 3: Add Database Migrations
Implement schema versioning:

```rust
pub struct Migration {
    version: u32,
    up_sql: &'static str,
    down_sql: &'static str,
}

pub fn migrate_database(pool: &DatabasePool, target_version: u32) -> Result<()> {
    // Apply migrations to reach target version
    // Track applied migrations in schema_migrations table
}
```

---

## Key Takeaways

1. **Connection Pooling is Essential**: Prevents resource exhaustion and improves performance
2. **ACID Guarantees Protect Money**: Transactions ensure no chips are lost or duplicated
3. **Background Tasks Enable Scale**: Automated backups and monitoring prevent outages
4. **Health Monitoring Prevents Disasters**: Early detection of corruption saves data
5. **Configuration Drives Behavior**: Different environments need different settings
6. **Observability Enables Operations**: Metrics and logs are essential for production
7. **Error Handling is Critical**: Database operations can fail, plan for it
8. **Resource Cleanup Prevents Leaks**: Proper shutdown and connection management
9. **WAL Mode Improves Concurrency**: Multiple readers don't block each other
10. **Backup Strategy is Non-Negotiable**: Regular backups with retention policies

---

## Next Chapter

[Chapter 12: Transport Layer →](./12_transport_layer.md)

Next, we'll explore how our database-backed casino nodes communicate over networks - from Bluetooth to the internet - ensuring reliable message delivery in our distributed system!

---

*Remember: "A database without backups is just a fancy way to lose data permanently."*
