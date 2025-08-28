# Chapter 95: Repository Pattern Implementation

*In 1994, Eric Evans introduced Domain-Driven Design to the software world, and with it came one of the most powerful abstractions in modern software architecture: the Repository Pattern. It's a deceptively simple idea—treat data access like a collection of domain objects—but its implications revolutionized how we build maintainable, testable applications.*

## The Birth of Data Abstraction

Before repositories, developers wrote SQL queries directly in their business logic. A simple user registration might have database code scattered across dozens of files. Martin Fowler observed this chaos and recognized that data access needed its own abstraction layer—a pattern that would become fundamental to clean architecture.

The repository pattern emerged from a simple observation: business logic shouldn't care whether data comes from a database, a file, or a web service. By treating data storage as an implementation detail, we could finally achieve true separation of concerns.

## Understanding the Repository Pattern

Think of a repository as a sophisticated collection that happens to be backed by persistent storage. Just as you might have a `Vec<User>` in memory, a repository provides a `UserRepository` that looks and feels like a collection but actually manages complex data persistence behind the scenes.

```rust
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Core repository trait defining standard operations
#[async_trait]
pub trait Repository<T, ID>
where
    T: Send + Sync,
    ID: Send + Sync + Clone,
{
    type Error: std::error::Error + Send + Sync;
    
    /// Find an entity by its ID
    async fn find_by_id(&self, id: &ID) -> Result<Option<T>, Self::Error>;
    
    /// Find all entities matching a specification
    async fn find_all(&self) -> Result<Vec<T>, Self::Error>;
    
    /// Save a new entity or update existing
    async fn save(&self, entity: T) -> Result<T, Self::Error>;
    
    /// Delete an entity by ID
    async fn delete(&self, id: &ID) -> Result<bool, Self::Error>;
    
    /// Check if entity exists
    async fn exists(&self, id: &ID) -> Result<bool, Self::Error>;
    
    /// Count total entities
    async fn count(&self) -> Result<usize, Self::Error>;
}

/// Specification pattern for complex queries
pub trait Specification<T> {
    /// Check if entity satisfies specification
    fn is_satisfied_by(&self, entity: &T) -> bool;
    
    /// Convert to SQL where clause (for SQL backends)
    fn to_sql(&self) -> Option<String> {
        None
    }
    
    /// Combine with another specification using AND
    fn and<S: Specification<T>>(self, other: S) -> AndSpecification<T, Self, S>
    where
        Self: Sized,
    {
        AndSpecification::new(self, other)
    }
    
    /// Combine with another specification using OR
    fn or<S: Specification<T>>(self, other: S) -> OrSpecification<T, Self, S>
    where
        Self: Sized,
    {
        OrSpecification::new(self, other)
    }
    
    /// Negate this specification
    fn not(self) -> NotSpecification<T, Self>
    where
        Self: Sized,
    {
        NotSpecification::new(self)
    }
}

/// AND combination of specifications
pub struct AndSpecification<T, S1: Specification<T>, S2: Specification<T>> {
    spec1: S1,
    spec2: S2,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, S1: Specification<T>, S2: Specification<T>> AndSpecification<T, S1, S2> {
    pub fn new(spec1: S1, spec2: S2) -> Self {
        Self {
            spec1,
            spec2,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, S1: Specification<T>, S2: Specification<T>> Specification<T> 
    for AndSpecification<T, S1, S2> 
{
    fn is_satisfied_by(&self, entity: &T) -> bool {
        self.spec1.is_satisfied_by(entity) && self.spec2.is_satisfied_by(entity)
    }
    
    fn to_sql(&self) -> Option<String> {
        match (self.spec1.to_sql(), self.spec2.to_sql()) {
            (Some(sql1), Some(sql2)) => Some(format!("({}) AND ({})", sql1, sql2)),
            _ => None,
        }
    }
}

/// Repository errors
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Entity not found: {0}")]
    NotFound(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Concurrency conflict: {0}")]
    ConcurrencyConflict(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
}
```

## Query Builder Pattern

The query builder pattern complements repositories by providing a fluent interface for constructing complex queries without exposing SQL details.

```rust
use std::fmt::Write;

/// Fluent query builder for constructing database queries
pub struct QueryBuilder<T> {
    table: String,
    select_columns: Vec<String>,
    where_clauses: Vec<String>,
    order_by: Vec<(String, SortDirection)>,
    limit: Option<usize>,
    offset: Option<usize>,
    joins: Vec<JoinClause>,
    group_by: Vec<String>,
    having: Vec<String>,
    _phantom: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct JoinClause {
    join_type: JoinType,
    table: String,
    on: String,
}

#[derive(Debug, Clone)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl<T> QueryBuilder<T> {
    pub fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            select_columns: vec!["*".to_string()],
            where_clauses: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
            joins: Vec::new(),
            group_by: Vec::new(),
            having: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Select specific columns
    pub fn select(mut self, columns: Vec<impl Into<String>>) -> Self {
        self.select_columns = columns.into_iter().map(|c| c.into()).collect();
        self
    }
    
    /// Add WHERE condition
    pub fn where_clause(mut self, condition: impl Into<String>) -> Self {
        self.where_clauses.push(condition.into());
        self
    }
    
    /// Add WHERE condition with parameter binding
    pub fn where_eq(mut self, column: impl Into<String>, value: impl ToString) -> Self {
        self.where_clauses.push(format!("{} = '{}'", column.into(), value.to_string()));
        self
    }
    
    /// Add WHERE IN condition
    pub fn where_in<V: ToString>(mut self, column: impl Into<String>, values: Vec<V>) -> Self {
        let values_str = values.iter()
            .map(|v| format!("'{}'", v.to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        self.where_clauses.push(format!("{} IN ({})", column.into(), values_str));
        self
    }
    
    /// Add ORDER BY clause
    pub fn order_by(mut self, column: impl Into<String>, direction: SortDirection) -> Self {
        self.order_by.push((column.into(), direction));
        self
    }
    
    /// Set LIMIT
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
    
    /// Set OFFSET
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
    
    /// Add JOIN clause
    pub fn join(
        mut self, 
        join_type: JoinType, 
        table: impl Into<String>, 
        on: impl Into<String>
    ) -> Self {
        self.joins.push(JoinClause {
            join_type,
            table: table.into(),
            on: on.into(),
        });
        self
    }
    
    /// Add GROUP BY clause
    pub fn group_by(mut self, columns: Vec<impl Into<String>>) -> Self {
        self.group_by = columns.into_iter().map(|c| c.into()).collect();
        self
    }
    
    /// Add HAVING clause
    pub fn having(mut self, condition: impl Into<String>) -> Self {
        self.having.push(condition.into());
        self
    }
    
    /// Build the SQL query
    pub fn build(&self) -> String {
        let mut query = String::new();
        
        // SELECT
        write!(&mut query, "SELECT {} FROM {}", 
            self.select_columns.join(", "), 
            self.table
        ).unwrap();
        
        // JOINs
        for join in &self.joins {
            let join_type = match join.join_type {
                JoinType::Inner => "INNER JOIN",
                JoinType::Left => "LEFT JOIN",
                JoinType::Right => "RIGHT JOIN",
                JoinType::Full => "FULL OUTER JOIN",
            };
            write!(&mut query, " {} {} ON {}", join_type, join.table, join.on).unwrap();
        }
        
        // WHERE
        if !self.where_clauses.is_empty() {
            write!(&mut query, " WHERE {}", self.where_clauses.join(" AND ")).unwrap();
        }
        
        // GROUP BY
        if !self.group_by.is_empty() {
            write!(&mut query, " GROUP BY {}", self.group_by.join(", ")).unwrap();
        }
        
        // HAVING
        if !self.having.is_empty() {
            write!(&mut query, " HAVING {}", self.having.join(" AND ")).unwrap();
        }
        
        // ORDER BY
        if !self.order_by.is_empty() {
            let order_clauses: Vec<String> = self.order_by.iter()
                .map(|(col, dir)| {
                    let dir_str = match dir {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                    };
                    format!("{} {}", col, dir_str)
                })
                .collect();
            write!(&mut query, " ORDER BY {}", order_clauses.join(", ")).unwrap();
        }
        
        // LIMIT and OFFSET
        if let Some(limit) = self.limit {
            write!(&mut query, " LIMIT {}", limit).unwrap();
        }
        if let Some(offset) = self.offset {
            write!(&mut query, " OFFSET {}", offset).unwrap();
        }
        
        query
    }
}

/// Paginated query results
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

impl<T> Page<T> {
    pub fn total_pages(&self) -> usize {
        (self.total + self.page_size - 1) / self.page_size
    }
    
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages() - 1
    }
    
    pub fn has_previous(&self) -> bool {
        self.page > 0
    }
}
```

## Concrete Repository Implementation

Let's implement a concrete repository for a User domain model, showing how the pattern works in practice.

```rust
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Domain model for User
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i32, // For optimistic locking
}

/// PostgreSQL implementation of UserRepository
pub struct PostgresUserRepository {
    pool: Arc<Pool<Postgres>>,
    cache: Arc<dyn Cache<Uuid, User>>,
}

impl PostgresUserRepository {
    pub fn new(pool: Arc<Pool<Postgres>>, cache: Arc<dyn Cache<Uuid, User>>) -> Self {
        Self { pool, cache }
    }
    
    /// Internal method to map database row to domain model
    fn map_row(row: &sqlx::postgres::PgRow) -> Result<User, RepositoryError> {
        Ok(User {
            id: row.try_get("id")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
            username: row.try_get("username")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
            email: row.try_get("email")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
            created_at: row.try_get("created_at")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
            updated_at: row.try_get("updated_at")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
            version: row.try_get("version")
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?,
        })
    }
}

#[async_trait]
impl Repository<User, Uuid> for PostgresUserRepository {
    type Error = RepositoryError;
    
    async fn find_by_id(&self, id: &Uuid) -> Result<Option<User>, Self::Error> {
        // Check cache first
        if let Some(user) = self.cache.get(id).await {
            return Ok(Some(user));
        }
        
        // Query database
        let user = sqlx::query(
            "SELECT id, username, email, created_at, updated_at, version 
             FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        .map(|row| Self::map_row(&row))
        .transpose()?;
        
        // Update cache if found
        if let Some(ref user) = user {
            self.cache.set(id.clone(), user.clone()).await;
        }
        
        Ok(user)
    }
    
    async fn find_all(&self) -> Result<Vec<User>, Self::Error> {
        let users = sqlx::query(
            "SELECT id, username, email, created_at, updated_at, version 
             FROM users ORDER BY created_at DESC"
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        .iter()
        .map(Self::map_row)
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(users)
    }
    
    async fn save(&self, mut user: User) -> Result<User, Self::Error> {
        // Check for existing user (optimistic locking)
        let existing = self.find_by_id(&user.id).await?;
        
        if let Some(existing) = existing {
            // Update existing user
            if existing.version != user.version {
                return Err(RepositoryError::ConcurrencyConflict(
                    "Version mismatch".to_string()
                ));
            }
            
            user.version += 1;
            user.updated_at = Utc::now();
            
            sqlx::query(
                "UPDATE users SET username = $2, email = $3, updated_at = $4, version = $5 
                 WHERE id = $1 AND version = $6"
            )
            .bind(&user.id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(&user.updated_at)
            .bind(&user.version)
            .bind(&existing.version)
            .execute(&*self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        } else {
            // Insert new user
            user.created_at = Utc::now();
            user.updated_at = user.created_at;
            user.version = 1;
            
            sqlx::query(
                "INSERT INTO users (id, username, email, created_at, updated_at, version) 
                 VALUES ($1, $2, $3, $4, $5, $6)"
            )
            .bind(&user.id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(&user.created_at)
            .bind(&user.updated_at)
            .bind(&user.version)
            .execute(&*self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        }
        
        // Update cache
        self.cache.set(user.id.clone(), user.clone()).await;
        
        Ok(user)
    }
    
    async fn delete(&self, id: &Uuid) -> Result<bool, Self::Error> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&*self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        
        // Invalidate cache
        self.cache.delete(id).await;
        
        Ok(result.rows_affected() > 0)
    }
    
    async fn exists(&self, id: &Uuid) -> Result<bool, Self::Error> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        
        Ok(count > 0)
    }
    
    async fn count(&self) -> Result<usize, Self::Error> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        
        Ok(count as usize)
    }
}

/// Extended repository with custom queries
impl PostgresUserRepository {
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query(
            "SELECT id, username, email, created_at, updated_at, version 
             FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        .map(|row| Self::map_row(&row))
        .transpose()?;
        
        Ok(user)
    }
    
    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>, RepositoryError> {
        let user = sqlx::query(
            "SELECT id, username, email, created_at, updated_at, version 
             FROM users WHERE username = $1"
        )
        .bind(username)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        .map(|row| Self::map_row(&row))
        .transpose()?;
        
        Ok(user)
    }
    
    pub async fn find_paginated(&self, page: usize, page_size: usize) 
        -> Result<Page<User>, RepositoryError> 
    {
        let offset = page * page_size;
        
        // Get total count
        let total = self.count().await?;
        
        // Get page items
        let users = sqlx::query(
            "SELECT id, username, email, created_at, updated_at, version 
             FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        )
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
        .iter()
        .map(Self::map_row)
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(Page {
            items: users,
            total,
            page,
            page_size,
        })
    }
}
```

## Caching Layer

A sophisticated caching layer is essential for repository performance.

```rust
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::hash::Hash;

/// Generic cache trait
#[async_trait]
pub trait Cache<K, V>: Send + Sync 
where
    K: Send + Sync,
    V: Send + Sync + Clone,
{
    async fn get(&self, key: &K) -> Option<V>;
    async fn set(&self, key: K, value: V);
    async fn delete(&self, key: &K);
    async fn clear(&self);
    async fn size(&self) -> usize;
}

/// LRU cache implementation
pub struct LruCache<K, V> 
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    capacity: usize,
    cache: Arc<RwLock<lru::LruCache<K, V>>>,
}

impl<K, V> LruCache<K, V> 
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: Arc::new(RwLock::new(lru::LruCache::new(capacity))),
        }
    }
}

#[async_trait]
impl<K, V> Cache<K, V> for LruCache<K, V> 
where
    K: Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    async fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.write().await;
        cache.get(key).cloned()
    }
    
    async fn set(&self, key: K, value: V) {
        let mut cache = self.cache.write().await;
        cache.put(key, value);
    }
    
    async fn delete(&self, key: &K) {
        let mut cache = self.cache.write().await;
        cache.pop(key);
    }
    
    async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
    
    async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

/// Time-based expiring cache
pub struct TtlCache<K, V> 
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    cache: Arc<RwLock<HashMap<K, (V, std::time::Instant)>>>,
    ttl: Duration,
}

impl<K, V> TtlCache<K, V> 
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(ttl: Duration) -> Self {
        let cache = Arc::new(RwLock::new(HashMap::new()));
        
        // Start cleanup task
        let cache_clone = cache.clone();
        let ttl_clone = ttl;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ttl_clone / 2);
            loop {
                interval.tick().await;
                let now = std::time::Instant::now();
                let mut cache = cache_clone.write().await;
                cache.retain(|_, (_, expiry)| *expiry > now);
            }
        });
        
        Self { cache, ttl }
    }
}

#[async_trait]
impl<K, V> Cache<K, V> for TtlCache<K, V> 
where
    K: Eq + Hash + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    async fn get(&self, key: &K) -> Option<V> {
        let cache = self.cache.read().await;
        cache.get(key)
            .filter(|(_, expiry)| *expiry > std::time::Instant::now())
            .map(|(value, _)| value.clone())
    }
    
    async fn set(&self, key: K, value: V) {
        let mut cache = self.cache.write().await;
        let expiry = std::time::Instant::now() + self.ttl;
        cache.insert(key, (value, expiry));
    }
    
    async fn delete(&self, key: &K) {
        let mut cache = self.cache.write().await;
        cache.remove(key);
    }
    
    async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
    
    async fn size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

/// Multi-level cache combining memory and Redis
pub struct MultiLevelCache<K, V> 
where
    K: Eq + Hash + Clone + ToString,
    V: Clone + Serialize + for<'de> Deserialize<'de>,
{
    l1_cache: Arc<dyn Cache<K, V>>,
    l2_cache: Arc<RedisCache<K, V>>,
}

pub struct RedisCache<K, V> {
    client: redis::aio::ConnectionManager,
    prefix: String,
    ttl: Duration,
    _phantom: std::marker::PhantomData<(K, V)>,
}

impl<K, V> RedisCache<K, V> 
where
    K: ToString,
    V: Serialize + for<'de> Deserialize<'de>,
{
    pub async fn new(redis_url: &str, prefix: String, ttl: Duration) 
        -> Result<Self, redis::RedisError> 
    {
        let client = redis::Client::open(redis_url)?;
        let connection = client.get_tokio_connection_manager().await?;
        
        Ok(Self {
            client: connection,
            prefix,
            ttl,
            _phantom: std::marker::PhantomData,
        })
    }
    
    fn make_key(&self, key: &K) -> String {
        format!("{}:{}", self.prefix, key.to_string())
    }
}

#[async_trait]
impl<K, V> Cache<K, V> for RedisCache<K, V> 
where
    K: ToString + Send + Sync,
    V: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    async fn get(&self, key: &K) -> Option<V> {
        let redis_key = self.make_key(key);
        let data: Option<Vec<u8>> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut self.client.clone())
            .await
            .ok()?;
        
        data.and_then(|bytes| bincode::deserialize(&bytes).ok())
    }
    
    async fn set(&self, key: K, value: V) {
        let redis_key = self.make_key(&key);
        if let Ok(bytes) = bincode::serialize(&value) {
            let _: Result<(), _> = redis::cmd("SETEX")
                .arg(&redis_key)
                .arg(self.ttl.as_secs())
                .arg(bytes)
                .query_async(&mut self.client.clone())
                .await;
        }
    }
    
    async fn delete(&self, key: &K) {
        let redis_key = self.make_key(key);
        let _: Result<(), _> = redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut self.client.clone())
            .await;
    }
    
    async fn clear(&self) {
        let pattern = format!("{}:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut self.client.clone())
            .await
            .unwrap_or_default();
        
        for key in keys {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg(&key)
                .query_async(&mut self.client.clone())
                .await;
        }
    }
    
    async fn size(&self) -> usize {
        let pattern = format!("{}:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut self.client.clone())
            .await
            .unwrap_or_default();
        
        keys.len()
    }
}
```

## Unit of Work Pattern

The Unit of Work pattern complements repositories by managing transactions across multiple repositories.

```rust
use std::collections::HashMap;
use std::any::{Any, TypeId};

/// Unit of Work for managing transactions
pub struct UnitOfWork {
    transaction: Option<sqlx::Transaction<'static, Postgres>>,
    repositories: HashMap<TypeId, Box<dyn Any>>,
    committed: bool,
}

impl UnitOfWork {
    pub async fn new(pool: &Pool<Postgres>) -> Result<Self, RepositoryError> {
        let transaction = pool.begin().await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        
        Ok(Self {
            transaction: Some(transaction),
            repositories: HashMap::new(),
            committed: false,
        })
    }
    
    /// Register a repository with this unit of work
    pub fn register<R: 'static>(&mut self, repository: R) {
        self.repositories.insert(TypeId::of::<R>(), Box::new(repository));
    }
    
    /// Get a registered repository
    pub fn get_repository<R: 'static>(&self) -> Option<&R> {
        self.repositories
            .get(&TypeId::of::<R>())
            .and_then(|r| r.downcast_ref::<R>())
    }
    
    /// Commit all changes
    pub async fn commit(mut self) -> Result<(), RepositoryError> {
        if let Some(tx) = self.transaction.take() {
            tx.commit().await
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
            self.committed = true;
        }
        Ok(())
    }
    
    /// Rollback all changes
    pub async fn rollback(mut self) -> Result<(), RepositoryError> {
        if let Some(tx) = self.transaction.take() {
            tx.rollback().await
                .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for UnitOfWork {
    fn drop(&mut self) {
        if !self.committed && self.transaction.is_some() {
            // Transaction will be rolled back automatically
            eprintln!("UnitOfWork dropped without commit - rolling back");
        }
    }
}

/// Service layer using Unit of Work
pub struct UserService {
    pool: Arc<Pool<Postgres>>,
}

impl UserService {
    pub async fn transfer_ownership(
        &self,
        from_user_id: Uuid,
        to_user_id: Uuid,
        resource_id: Uuid,
    ) -> Result<(), RepositoryError> {
        let mut uow = UnitOfWork::new(&*self.pool).await?;
        
        // Register repositories
        let user_repo = PostgresUserRepository::new(
            self.pool.clone(),
            Arc::new(LruCache::new(100)),
        );
        let resource_repo = ResourceRepository::new(self.pool.clone());
        
        uow.register(user_repo);
        uow.register(resource_repo);
        
        // Perform operations within transaction
        let from_user = uow.get_repository::<PostgresUserRepository>()
            .unwrap()
            .find_by_id(&from_user_id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound("From user not found".to_string()))?;
        
        let to_user = uow.get_repository::<PostgresUserRepository>()
            .unwrap()
            .find_by_id(&to_user_id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound("To user not found".to_string()))?;
        
        // Update resource ownership
        let mut resource = uow.get_repository::<ResourceRepository>()
            .unwrap()
            .find_by_id(&resource_id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound("Resource not found".to_string()))?;
        
        resource.owner_id = to_user_id;
        uow.get_repository::<ResourceRepository>()
            .unwrap()
            .save(resource)
            .await?;
        
        // Commit transaction
        uow.commit().await?;
        
        Ok(())
    }
}
```

## BitCraps Repository System

For BitCraps, we need specialized repositories for game state and peer management.

```rust
/// Game state repository for BitCraps
pub struct GameStateRepository {
    storage: Arc<dyn Storage>,
    cache: Arc<dyn Cache<GameId, GameState>>,
    event_store: Arc<EventStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub id: GameId,
    pub players: Vec<PlayerId>,
    pub current_shooter: Option<PlayerId>,
    pub point: Option<u8>,
    pub bets: HashMap<PlayerId, Vec<Bet>>,
    pub phase: GamePhase,
    pub round: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    WaitingForPlayers,
    ComeOut,
    Point(u8),
    Resolved,
}

impl GameStateRepository {
    pub async fn find_active_games(&self) -> Result<Vec<GameState>, RepositoryError> {
        let query = QueryBuilder::<GameState>::new("game_states")
            .where_clause("phase != 'Resolved'")
            .order_by("created_at", SortDirection::Desc)
            .build();
        
        self.storage.query(&query).await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    
    pub async fn find_games_by_player(&self, player_id: PlayerId) 
        -> Result<Vec<GameState>, RepositoryError> 
    {
        let query = QueryBuilder::<GameState>::new("game_states")
            .where_clause(format!("'{}' = ANY(players)", player_id))
            .order_by("updated_at", SortDirection::Desc)
            .build();
        
        self.storage.query(&query).await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))
    }
    
    pub async fn save_with_event(&self, game: GameState, event: GameEvent) 
        -> Result<GameState, RepositoryError> 
    {
        // Start transaction
        let tx = self.storage.begin_transaction().await?;
        
        // Save game state
        let saved = self.storage.save(&game).await?;
        
        // Append event
        self.event_store.append(game.id, event).await?;
        
        // Update cache
        self.cache.set(game.id.clone(), saved.clone()).await;
        
        // Commit transaction
        tx.commit().await?;
        
        Ok(saved)
    }
    
    pub async fn replay_events(&self, game_id: GameId) 
        -> Result<GameState, RepositoryError> 
    {
        // Get all events for game
        let events = self.event_store.get_events(game_id).await?;
        
        // Start with initial state
        let mut state = GameState::new(game_id);
        
        // Apply each event
        for event in events {
            state = state.apply_event(event)?;
        }
        
        Ok(state)
    }
}

/// Peer repository for network participants
pub struct PeerRepository {
    storage: Arc<dyn Storage>,
    network: Arc<NetworkManager>,
    cache: Arc<DashMap<PeerId, PeerInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: PeerId,
    pub address: SocketAddr,
    pub public_key: PublicKey,
    pub reputation: f64,
    pub last_seen: DateTime<Utc>,
    pub capabilities: Vec<String>,
    pub latency_ms: Option<u64>,
}

impl PeerRepository {
    pub async fn find_nearby_peers(&self, max_distance: u32) 
        -> Result<Vec<PeerInfo>, RepositoryError> 
    {
        let local_id = self.network.local_peer_id();
        let mut peers = Vec::new();
        
        for entry in self.cache.iter() {
            let peer = entry.value();
            if let Some(distance) = self.network.xor_distance(&local_id, &peer.id) {
                if distance <= max_distance {
                    peers.push(peer.clone());
                }
            }
        }
        
        // Sort by distance
        peers.sort_by_key(|p| self.network.xor_distance(&local_id, &p.id).unwrap_or(u32::MAX));
        
        Ok(peers)
    }
    
    pub async fn find_peers_with_capability(&self, capability: &str) 
        -> Result<Vec<PeerInfo>, RepositoryError> 
    {
        let peers: Vec<PeerInfo> = self.cache
            .iter()
            .filter(|entry| entry.value().capabilities.contains(&capability.to_string()))
            .map(|entry| entry.value().clone())
            .collect();
        
        Ok(peers)
    }
    
    pub async fn update_peer_metrics(&self, peer_id: PeerId, latency: Duration) 
        -> Result<(), RepositoryError> 
    {
        if let Some(mut peer) = self.cache.get_mut(&peer_id) {
            peer.latency_ms = Some(latency.as_millis() as u64);
            peer.last_seen = Utc::now();
            
            // Update reputation based on responsiveness
            if latency.as_millis() < 100 {
                peer.reputation = (peer.reputation * 0.95 + 1.0 * 0.05).min(1.0);
            } else if latency.as_millis() > 1000 {
                peer.reputation = (peer.reputation * 0.95 + 0.5 * 0.05).max(0.0);
            }
        }
        
        Ok(())
    }
}
```

## Testing Repository Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    
    async fn setup_test_db() -> Pool<Postgres> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://test:test@localhost/test_db".to_string());
        
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }
    
    #[tokio::test]
    async fn test_user_repository_crud() {
        let pool = Arc::new(setup_test_db().await);
        let cache = Arc::new(LruCache::new(10));
        let repo = PostgresUserRepository::new(pool, cache);
        
        // Create user
        let user = User {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 0,
        };
        
        let saved = repo.save(user.clone()).await.unwrap();
        assert_eq!(saved.version, 1);
        
        // Find by ID
        let found = repo.find_by_id(&saved.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().username, "testuser");
        
        // Update user
        let mut updated = saved.clone();
        updated.username = "updateduser".to_string();
        let saved_again = repo.save(updated).await.unwrap();
        assert_eq!(saved_again.version, 2);
        
        // Delete user
        let deleted = repo.delete(&saved_again.id).await.unwrap();
        assert!(deleted);
        
        // Verify deletion
        let not_found = repo.find_by_id(&saved_again.id).await.unwrap();
        assert!(not_found.is_none());
    }
    
    #[tokio::test]
    async fn test_query_builder() {
        let query = QueryBuilder::<User>::new("users")
            .select(vec!["id", "username", "email"])
            .where_eq("active", true)
            .where_in("role", vec!["admin", "moderator"])
            .order_by("created_at", SortDirection::Desc)
            .limit(10)
            .build();
        
        let expected = "SELECT id, username, email FROM users WHERE active = 'true' AND role IN ('admin', 'moderator') ORDER BY created_at DESC LIMIT 10";
        assert_eq!(query, expected);
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let cache = LruCache::new(3);
        
        // Set values
        cache.set("key1", "value1").await;
        cache.set("key2", "value2").await;
        cache.set("key3", "value3").await;
        
        // Get values
        assert_eq!(cache.get(&"key1").await, Some("value1"));
        assert_eq!(cache.get(&"key2").await, Some("value2"));
        
        // LRU eviction
        cache.set("key4", "value4").await;
        assert_eq!(cache.get(&"key3").await, None); // Evicted
        assert_eq!(cache.get(&"key4").await, Some("value4"));
        
        // Size check
        assert_eq!(cache.size().await, 3);
    }
    
    #[tokio::test]
    async fn test_unit_of_work() {
        let pool = Arc::new(setup_test_db().await);
        let mut uow = UnitOfWork::new(&*pool).await.unwrap();
        
        // Register repositories
        let user_repo = PostgresUserRepository::new(
            pool.clone(),
            Arc::new(LruCache::new(10)),
        );
        uow.register(user_repo);
        
        // Perform operations
        let user = User {
            id: Uuid::new_v4(),
            username: "uow_test".to_string(),
            email: "uow@test.com".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 0,
        };
        
        uow.get_repository::<PostgresUserRepository>()
            .unwrap()
            .save(user.clone())
            .await
            .unwrap();
        
        // Commit
        uow.commit().await.unwrap();
        
        // Verify commit
        let repo = PostgresUserRepository::new(pool, Arc::new(LruCache::new(10)));
        let found = repo.find_by_id(&user.id).await.unwrap();
        assert!(found.is_some());
    }
}
```

## Common Pitfalls and Solutions

1. **N+1 Query Problem**: Use eager loading or batch fetching
2. **Cache Invalidation**: Implement proper cache invalidation strategies
3. **Optimistic Locking Conflicts**: Implement retry logic with exponential backoff
4. **Repository Bloat**: Keep repositories focused on data access, move business logic to services
5. **Testing with Real Databases**: Use test containers or in-memory databases

## Practical Exercises

1. **Implement a MongoDB Repository**: Create a repository using MongoDB instead of PostgreSQL
2. **Add Full-Text Search**: Implement search capabilities using Elasticsearch
3. **Build Event Sourcing**: Create an event-sourced repository
4. **Implement CQRS**: Separate read and write repositories
5. **Add GraphQL Support**: Build a GraphQL resolver using repositories

## Conclusion

The Repository Pattern is more than just a data access abstraction—it's a fundamental building block of clean architecture. By encapsulating data access logic, providing a consistent interface, and enabling testability, repositories allow us to build maintainable, scalable applications.

In the context of BitCraps and distributed systems, repositories become even more critical. They provide the abstraction needed to handle complex data synchronization, caching strategies, and eventual consistency models while keeping the domain logic clean and focused.

Remember that the best repository is one that makes the simple things easy and the complex things possible.

## Additional Resources

- "Domain-Driven Design" by Eric Evans
- "Patterns of Enterprise Application Architecture" by Martin Fowler
- "Implementing Domain-Driven Design" by Vaughn Vernon
- SQLx Documentation for Rust
- Redis Documentation for Caching Strategies

---

*Next Chapter: [96: Consensus Benchmarking](./96_consensus_benchmarking.md)*