# Chapter 38: Repository Pattern Walkthrough

## Introduction

The repository pattern implementation provides a clean abstraction layer between the business logic and data persistence layers in BitCraps. This module implements specialized repositories for users, games, transactions, and statistics, demonstrating proper separation of concerns and database abstraction in a production gaming system.

## Computer Science Foundations

### Repository Pattern

The repository pattern encapsulates data access logic:

```rust
pub struct UserRepository {
    pool: Arc<DatabasePool>,
}

impl UserRepository {
    pub async fn create_user(&self, id: &str, username: &str, public_key: &[u8]) -> Result<()>
    pub async fn get_user(&self, id: &str) -> Result<Option<User>>
    pub async fn update_reputation(&self, id: &str, reputation: f64) -> Result<()>
    pub async fn list_users(&self, limit: usize) -> Result<Vec<User>>
}
```

**Pattern Benefits:**
- Centralized query logic
- Testable data access
- Database abstraction
- Consistent error handling

### Domain Model Design

Each repository manages its domain entity:

```rust
pub struct User {
    pub id: String,
    pub username: String,
    pub public_key: Vec<u8>,
    pub reputation: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct Game {
    pub id: String,
    pub state: serde_json::Value,
    pub pot_size: i64,
    pub phase: String,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub winner_id: Option<String>,
}
```

**Domain Characteristics:**
- Self-contained entities
- Clear boundaries
- Serialization support
- Audit trail fields

## Implementation Analysis

### User Repository

User management with reputation tracking:

```rust
impl UserRepository {
    pub async fn create_user(&self, id: &str, username: &str, public_key: &[u8]) -> Result<()> {
        self.pool.with_connection(|conn| {
            conn.execute(
                "INSERT INTO users (id, username, public_key, created_at, updated_at) 
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    id,
                    username,
                    public_key,
                    chrono::Utc::now().timestamp(),
                    chrono::Utc::now().timestamp(),
                ],
            ).map_err(|e| Error::Database(format!("Failed to create user: {}", e)))?;
            Ok(())
        }).await
    }
    
    pub async fn update_reputation(&self, id: &str, reputation: f64) -> Result<()> {
        self.pool.with_connection(|conn| {
            conn.execute(
                "UPDATE users SET reputation = ?, updated_at = ? WHERE id = ?",
                params![reputation, chrono::Utc::now().timestamp(), id],
            ).map_err(|e| Error::Database(format!("Failed to update reputation: {}", e)))?;
            Ok(())
        }).await
    }
}
```

**Features:**
- Cryptographic identity storage
- Reputation system integration
- Timestamp tracking
- Atomic operations

### Game Repository

Game state persistence with JSON flexibility:

```rust
impl GameRepository {
    pub async fn create_game(&self, game: &Game) -> Result<()> {
        self.pool.with_connection(|conn| {
            conn.execute(
                "INSERT INTO games (id, state, pot_size, phase, created_at) 
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    &game.id,
                    serde_json::to_string(&game.state)?,
                    game.pot_size,
                    &game.phase,
                    game.created_at,
                ],
            )
        }).await
    }
    
    pub async fn list_active_games(&self, limit: usize) -> Result<Vec<Game>> {
        self.pool.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, state, pot_size, phase, created_at, completed_at, winner_id 
                 FROM games WHERE completed_at IS NULL 
                 ORDER BY created_at DESC LIMIT ?"
            )?;
            
            let games = stmt.query_map(params![limit], |row| {
                let state_json: String = row.get(1)?;
                Ok(Game {
                    id: row.get(0)?,
                    state: serde_json::from_str(&state_json)?,
                    pot_size: row.get(2)?,
                    phase: row.get(3)?,
                    created_at: row.get(4)?,
                    completed_at: row.get(5)?,
                    winner_id: row.get(6)?,
                })
            })?;
            
            games.collect()
        }).await
    }
}
```

**Design Choices:**
- Flexible state storage via JSON
- Phase-based filtering
- Winner tracking
- Active game queries

### Transaction Repository

Financial transaction tracking with balance calculation:

```rust
impl TransactionRepository {
    pub async fn create_transaction(&self, tx: &Transaction) -> Result<()> {
        self.pool.with_connection(|conn| {
            conn.execute(
                "INSERT INTO transactions 
                 (id, from_user_id, to_user_id, amount, transaction_type, status, created_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![
                    &tx.id,
                    &tx.from_user_id,
                    &tx.to_user_id,
                    tx.amount,
                    &tx.transaction_type,
                    &tx.status,
                    tx.created_at,
                ],
            )
        }).await
    }
    
    pub async fn get_balance(&self, user_id: &str) -> Result<i64> {
        self.pool.with_connection(|conn| {
            // Calculate balance from transactions
            let received: i64 = conn.query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM transactions 
                 WHERE to_user_id = ? AND status = 'confirmed'",
                params![user_id],
                |row| row.get(0),
            )?;
            
            let sent: i64 = conn.query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM transactions 
                 WHERE from_user_id = ? AND status = 'confirmed'",
                params![user_id],
                |row| row.get(0),
            )?;
            
            Ok(received - sent)
        }).await
    }
}
```

**Financial Features:**
- Transaction atomicity
- Balance derivation
- Status tracking
- Confirmation timestamps

### Statistics Repository

Analytics and leaderboard functionality:

```rust
impl StatsRepository {
    pub async fn update_game_stats(&self, game_id: &str, stats: &GameStats) -> Result<()> {
        self.pool.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO game_statistics 
                 (game_id, total_bets, total_wagered, total_won, house_edge, 
                  duration_seconds, player_count, max_pot_size, created_at) 
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    game_id,
                    stats.total_bets,
                    stats.total_wagered,
                    stats.total_won,
                    stats.house_edge,
                    stats.duration_seconds,
                    stats.player_count,
                    stats.max_pot_size,
                    chrono::Utc::now().timestamp(),
                ],
            )
        }).await
    }
    
    pub async fn get_leaderboard(&self, limit: usize) -> Result<Vec<LeaderboardEntry>> {
        self.pool.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT u.username, p.games_won, p.total_won, p.win_rate 
                 FROM player_statistics p 
                 JOIN users u ON p.player_id = u.id 
                 ORDER BY p.total_won DESC LIMIT ?"
            )?;
            
            let entries = stmt.query_map(params![limit], |row| {
                Ok(LeaderboardEntry {
                    username: row.get(0)?,
                    games_won: row.get(1)?,
                    total_won: row.get(2)?,
                    win_rate: row.get(3)?,
                })
            })?;
            
            entries.collect()
        }).await
    }
}
```

**Analytics Features:**
- Game metrics tracking
- Player performance
- House edge calculation
- Leaderboard generation

## Connection Pool Integration

All repositories share a connection pool:

```rust
self.pool.with_connection(|conn| {
    // Database operations
    conn.execute(sql, params)?;
    Ok(result)
}).await
```

**Pool Benefits:**
- Connection reuse
- Concurrency control
- Transaction support
- Error propagation

## Error Handling

Consistent error transformation:

```rust
conn.execute(sql, params)
    .map_err(|e| Error::Database(format!("Failed to create user: {}", e)))?;

stmt.query_row(params, |row| {...})
    .optional()
    .map_err(|e| Error::Database(e.to_string()))?;
```

**Error Strategy:**
- Database errors wrapped
- Context preservation
- Optional results
- Type safety

## Query Patterns

### COALESCE for Null Handling

```sql
SELECT COALESCE(SUM(amount), 0) FROM transactions
```

### INSERT OR REPLACE

```sql
INSERT OR REPLACE INTO game_statistics ...
```

### JOIN for Related Data

```sql
SELECT u.username, p.games_won 
FROM player_statistics p 
JOIN users u ON p.player_id = u.id
```

### Conditional Filtering

```sql
WHERE completed_at IS NULL 
ORDER BY created_at DESC 
LIMIT ?
```

## Performance Considerations

### Index Strategy
- Primary keys on IDs
- Composite indexes for queries
- Status field indexing
- Timestamp indexes

### Query Optimization
- Prepared statements
- Batch operations
- Limited result sets
- Efficient joins

### Connection Management
- Pool size limits
- Connection timeout
- Retry logic
- Circuit breaking

## Security Considerations

### SQL Injection Prevention
- Parameterized queries throughout
- No string concatenation
- Type-safe parameters
- Input validation

### Data Integrity
- Foreign key constraints
- Check constraints
- NOT NULL enforcement
- Unique constraints

## Testing Strategy

The repository pattern facilitates testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_user_creation() {
        let pool = create_test_pool();
        let repo = UserRepository::new(pool);
        
        repo.create_user("test_id", "test_user", b"public_key").await.unwrap();
        let user = repo.get_user("test_id").await.unwrap();
        
        assert_eq!(user.unwrap().username, "test_user");
    }
}
```

## Known Limitations

1. **No ORM:**
   - Manual SQL writing
   - Manual mapping
   - Migration management

2. **Single Database:**
   - No sharding
   - No read replicas
   - Scaling constraints

3. **Synchronous Pool:**
   - Blocking operations
   - Thread pool dependency

## Future Enhancements

1. **Query Builder:**
   - Type-safe SQL
   - Dynamic queries
   - Compile-time validation

2. **Migration System:**
   - Version control
   - Rollback support
   - Automated deployment

3. **Caching Layer:**
   - Query result caching
   - Entity caching
   - Cache invalidation

## Senior Engineering Review

**Strengths:**
- Clean separation of concerns
- Consistent error handling
- Good use of repository pattern
- Type-safe operations

**Concerns:**
- Manual SQL maintenance
- No query optimization hints
- Limited connection pool features

**Production Readiness:** 8.6/10
- Pattern implementation solid
- Needs migration system
- Good for medium-scale deployments

## Conclusion

The repository pattern implementation provides a clean, maintainable data access layer for BitCraps. The separation of concerns between repositories, consistent error handling, and type-safe operations create a solid foundation for the gaming platform's data persistence needs. While lacking some advanced ORM features, the simplicity and directness of the implementation make it suitable for production use.

---

*Next: [Chapter 39: Database Pool →](39_database_pool_walkthrough.md)*
*Previous: [Chapter 37: Backup and Recovery ←](37_backup_recovery_walkthrough.md)*