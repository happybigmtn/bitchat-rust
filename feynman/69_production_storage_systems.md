# Chapter 69: Production Storage Systems

## Introduction: The Memory of Distributed Systems

Imagine running a massive library where millions of books arrive every second, readers demand instant access to any book ever written, and you must never lose a single page—even if the building catches fire. This is the challenge of production storage systems in distributed applications.

In BitCraps, storage isn't just about persisting data; it's about managing game state across crashes, enabling instant recovery, supporting time-travel debugging, and maintaining perfect consistency while serving thousands of concurrent requests. This chapter explores how we build storage systems that are fast, reliable, and scalable.

## The Fundamentals: Storage Hierarchy

### Understanding Storage Tiers

Production systems use multiple storage layers, each optimized for different access patterns:

```rust
pub enum StorageTier {
    /// In-memory cache (nanoseconds)
    L1Cache { size: usize, ttl: Duration },
    
    /// SSD-backed cache (microseconds)
    L2Cache { path: PathBuf, max_size: u64 },
    
    /// Primary database (milliseconds)
    Database { connection_pool: Arc<ConnectionPool> },
    
    /// Cold storage archive (seconds)
    Archive { bucket: String, compression: CompressionType },
}

pub struct TieredStorage {
    tiers: Vec<Box<dyn StorageTier>>,
    promotion_policy: PromotionPolicy,
    eviction_policy: EvictionPolicy,
}
```

## Deep Dive: Write-Ahead Logging (WAL)

### Implementing Durability

```rust
pub struct WriteAheadLog {
    /// Active log file
    active_segment: Arc<RwLock<LogSegment>>,
    
    /// Completed segments awaiting compression
    sealed_segments: Arc<RwLock<Vec<LogSegment>>>,
    
    /// Configuration
    config: WalConfig,
    
    /// Checksums for integrity
    checksum_engine: ChecksumEngine,
}

pub struct LogSegment {
    file: File,
    path: PathBuf,
    size: AtomicU64,
    entries: AtomicU32,
    first_index: u64,
    last_index: AtomicU64,
}

impl WriteAheadLog {
    pub async fn append(&self, entry: &LogEntry) -> Result<u64> {
        // Serialize entry with checksum
        let mut buffer = Vec::with_capacity(entry.estimated_size());
        buffer.write_u32(entry.size())?;
        buffer.write_u32(self.checksum_engine.calculate(entry))?;
        bincode::serialize_into(&mut buffer, entry)?;
        
        // Append to active segment
        let mut segment = self.active_segment.write().await;
        
        // Check if rotation needed
        if segment.should_rotate(&self.config) {
            self.rotate_segment().await?;
            segment = self.active_segment.write().await;
        }
        
        // Write with fsync for durability
        segment.file.write_all(&buffer)?;
        segment.file.sync_data()?;
        
        let index = segment.last_index.fetch_add(1, Ordering::SeqCst);
        segment.size.fetch_add(buffer.len() as u64, Ordering::Relaxed);
        
        Ok(index)
    }
    
    pub async fn recover(&self) -> Result<Vec<LogEntry>> {
        let mut entries = Vec::new();
        
        // Read all segments in order
        for segment_path in self.list_segments()? {
            let mut file = File::open(segment_path)?;
            
            loop {
                // Read entry header
                let mut header = [0u8; 8];
                match file.read_exact(&mut header) {
                    Ok(_) => {},
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into()),
                }
                
                let size = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
                let expected_checksum = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
                
                // Read entry data
                let mut data = vec![0u8; size as usize];
                file.read_exact(&mut data)?;
                
                // Verify checksum
                let actual_checksum = self.checksum_engine.calculate(&data);
                if actual_checksum != expected_checksum {
                    return Err(Error::CorruptedEntry);
                }
                
                // Deserialize entry
                let entry: LogEntry = bincode::deserialize(&data)?;
                entries.push(entry);
            }
        }
        
        Ok(entries)
    }
}
```

## Multi-Version Concurrency Control (MVCC)

### Implementing Snapshot Isolation

```rust
pub struct MvccStorage {
    /// Current version counter
    version_counter: AtomicU64,
    
    /// Version history
    versions: Arc<RwLock<BTreeMap<u64, VersionSnapshot>>>,
    
    /// Active transactions
    transactions: Arc<RwLock<HashMap<TransactionId, Transaction>>>,
    
    /// Garbage collector
    gc: GarbageCollector,
}

pub struct VersionSnapshot {
    version: u64,
    timestamp: SystemTime,
    data: Arc<HashMap<Key, VersionedValue>>,
    tombstones: HashSet<Key>,
}

pub struct VersionedValue {
    value: Vec<u8>,
    created_version: u64,
    deleted_version: Option<u64>,
}

impl MvccStorage {
    pub async fn begin_transaction(&self) -> Transaction {
        let tx_id = TransactionId::new();
        let snapshot_version = self.version_counter.load(Ordering::Acquire);
        
        let transaction = Transaction {
            id: tx_id,
            snapshot_version,
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            state: TransactionState::Active,
        };
        
        self.transactions.write().await.insert(tx_id, transaction.clone());
        transaction
    }
    
    pub async fn get(&self, tx: &Transaction, key: &Key) -> Result<Option<Vec<u8>>> {
        // Read from transaction's write set first
        if let Some(value) = tx.write_set.get(key) {
            return Ok(Some(value.clone()));
        }
        
        // Read from snapshot
        let versions = self.versions.read().await;
        
        // Find the right version for this transaction
        for (version, snapshot) in versions.range(..=tx.snapshot_version).rev() {
            if let Some(versioned) = snapshot.data.get(key) {
                if versioned.created_version <= tx.snapshot_version &&
                   versioned.deleted_version.map_or(true, |v| v > tx.snapshot_version) {
                    return Ok(Some(versioned.value.clone()));
                }
            }
        }
        
        Ok(None)
    }
    
    pub async fn commit(&self, mut tx: Transaction) -> Result<()> {
        // Validate transaction
        if !self.validate_transaction(&tx).await? {
            return Err(Error::TransactionConflict);
        }
        
        // Acquire new version
        let commit_version = self.version_counter.fetch_add(1, Ordering::SeqCst) + 1;
        
        // Create new snapshot
        let mut new_data = HashMap::new();
        for (key, value) in tx.write_set {
            new_data.insert(key, VersionedValue {
                value,
                created_version: commit_version,
                deleted_version: None,
            });
        }
        
        let snapshot = VersionSnapshot {
            version: commit_version,
            timestamp: SystemTime::now(),
            data: Arc::new(new_data),
            tombstones: tx.delete_set,
        };
        
        // Add to version history
        self.versions.write().await.insert(commit_version, snapshot);
        
        // Mark transaction as committed
        tx.state = TransactionState::Committed;
        self.transactions.write().await.insert(tx.id, tx);
        
        // Trigger garbage collection if needed
        self.gc.maybe_collect(commit_version).await;
        
        Ok(())
    }
}
```

## Compression and Deduplication

### Space-Efficient Storage

```rust
pub struct CompressedStorage {
    /// Compression algorithm
    compressor: Box<dyn Compressor>,
    
    /// Deduplication index
    dedup_index: Arc<RwLock<HashMap<Hash, BlockLocation>>>,
    
    /// Block storage
    blocks: BlockStorage,
    
    /// Statistics
    stats: CompressionStats,
}

pub trait Compressor: Send + Sync {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decompress(&self, compressed: &[u8]) -> Result<Vec<u8>>;
    fn compression_ratio(&self, original: usize, compressed: usize) -> f64 {
        compressed as f64 / original as f64
    }
}

pub struct LZ4Compressor {
    level: u32,
}

impl Compressor for LZ4Compressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        lz4::block::compress(data, Some(lz4::block::CompressionMode::HIGHCOMPRESSION(self.level)), true)
            .map_err(Into::into)
    }
    
    fn decompress(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        lz4::block::decompress(compressed, None)
            .map_err(Into::into)
    }
}

impl CompressedStorage {
    pub async fn store(&self, key: &Key, value: &[u8]) -> Result<()> {
        // Calculate content hash for deduplication
        let hash = blake3::hash(value);
        
        // Check if content already exists
        if let Some(location) = self.dedup_index.read().await.get(&hash) {
            // Content already stored, just update reference
            self.blocks.add_reference(location, key).await?;
            return Ok(());
        }
        
        // Compress data
        let compressed = self.compressor.compress(value)?;
        let ratio = self.compressor.compression_ratio(value.len(), compressed.len());
        
        // Store compressed block
        let location = self.blocks.store_block(&compressed).await?;
        
        // Update deduplication index
        self.dedup_index.write().await.insert(hash, location.clone());
        
        // Update statistics
        self.stats.record_compression(value.len(), compressed.len(), ratio);
        
        Ok(())
    }
    
    pub async fn retrieve(&self, key: &Key) -> Result<Vec<u8>> {
        // Find block location
        let location = self.blocks.find_block(key).await?
            .ok_or(Error::KeyNotFound)?;
        
        // Read compressed block
        let compressed = self.blocks.read_block(&location).await?;
        
        // Decompress
        let decompressed = self.compressor.decompress(&compressed)?;
        
        Ok(decompressed)
    }
}
```

## Horizontal Scaling with Sharding

### Distributed Storage Architecture

```rust
pub struct ShardedStorage {
    /// Shard configuration
    shards: Vec<Shard>,
    
    /// Shard assignment strategy
    partitioner: Box<dyn Partitioner>,
    
    /// Replication manager
    replication: ReplicationManager,
    
    /// Coordinator for distributed operations
    coordinator: DistributedCoordinator,
}

pub struct Shard {
    id: ShardId,
    range: KeyRange,
    primary: NodeId,
    replicas: Vec<NodeId>,
    storage: Arc<dyn Storage>,
}

pub trait Partitioner: Send + Sync {
    fn assign_shard(&self, key: &Key) -> ShardId;
    fn rebalance(&self, shards: &[Shard]) -> Vec<ShardMigration>;
}

pub struct ConsistentHashPartitioner {
    ring: ConsistentHashRing,
    virtual_nodes: u32,
}

impl Partitioner for ConsistentHashPartitioner {
    fn assign_shard(&self, key: &Key) -> ShardId {
        let hash = xxhash::xxh3::xxh3_64(key);
        self.ring.find_node(hash)
    }
    
    fn rebalance(&self, shards: &[Shard]) -> Vec<ShardMigration> {
        let mut migrations = Vec::new();
        
        // Calculate ideal distribution
        let total_keys = shards.iter().map(|s| s.key_count()).sum::<usize>();
        let ideal_per_shard = total_keys / shards.len();
        
        // Find overloaded and underloaded shards
        let mut overloaded = Vec::new();
        let mut underloaded = Vec::new();
        
        for shard in shards {
            let count = shard.key_count();
            if count > ideal_per_shard * 120 / 100 { // 20% over
                overloaded.push(shard);
            } else if count < ideal_per_shard * 80 / 100 { // 20% under
                underloaded.push(shard);
            }
        }
        
        // Plan migrations
        for source in &overloaded {
            for target in &underloaded {
                let keys_to_move = (source.key_count() - ideal_per_shard) / 2;
                migrations.push(ShardMigration {
                    source: source.id,
                    target: target.id,
                    key_range: source.select_keys_for_migration(keys_to_move),
                });
            }
        }
        
        migrations
    }
}
```

## ACID Compliance in Distributed Storage

### Implementing Distributed Transactions

```rust
pub struct DistributedTransactionManager {
    /// Two-phase commit coordinator
    coordinator: TwoPhaseCommitCoordinator,
    
    /// Transaction log
    transaction_log: WriteAheadLog,
    
    /// Lock manager
    lock_manager: DistributedLockManager,
    
    /// Deadlock detector
    deadlock_detector: DeadlockDetector,
}

pub struct TwoPhaseCommitCoordinator {
    /// Participant nodes
    participants: HashMap<NodeId, ParticipantConnection>,
    
    /// Transaction states
    transactions: Arc<RwLock<HashMap<TransactionId, DistributedTransaction>>>,
    
    /// Timeout configuration
    prepare_timeout: Duration,
    commit_timeout: Duration,
}

impl TwoPhaseCommitCoordinator {
    pub async fn execute_transaction<F>(&self, f: F) -> Result<()>
    where
        F: Fn(&mut DistributedTransaction) -> Result<()>,
    {
        let tx_id = TransactionId::new();
        let mut tx = DistributedTransaction::new(tx_id);
        
        // Phase 1: Prepare
        f(&mut tx)?;
        
        // Send prepare to all participants
        let prepare_futures: Vec<_> = tx.participants
            .iter()
            .map(|p| self.send_prepare(p, &tx))
            .collect();
        
        let prepare_results = futures::future::join_all(prepare_futures).await;
        
        // Check all prepares succeeded
        for result in prepare_results {
            match result {
                Ok(PrepareResponse::Ready) => continue,
                Ok(PrepareResponse::Abort) | Err(_) => {
                    // Abort transaction
                    self.abort_transaction(tx_id).await?;
                    return Err(Error::TransactionAborted);
                }
            }
        }
        
        // Phase 2: Commit
        let commit_futures: Vec<_> = tx.participants
            .iter()
            .map(|p| self.send_commit(p, tx_id))
            .collect();
        
        let commit_results = futures::future::join_all(commit_futures).await;
        
        // Verify all commits succeeded
        for result in commit_results {
            if result.is_err() {
                // Log error but transaction is committed
                tracing::error!("Commit notification failed: {:?}", result);
            }
        }
        
        Ok(())
    }
    
    async fn abort_transaction(&self, tx_id: TransactionId) -> Result<()> {
        let abort_futures: Vec<_> = self.participants
            .values()
            .map(|p| p.send_abort(tx_id))
            .collect();
        
        futures::future::join_all(abort_futures).await;
        Ok(())
    }
}
```

## Cache Coherency

### Distributed Cache Management

```rust
pub struct DistributedCache {
    /// Local cache
    local: Arc<RwLock<LruCache<Key, CachedValue>>>,
    
    /// Cache invalidation bus
    invalidation_bus: InvalidationBus,
    
    /// Cache coherency protocol
    coherency: CacheCoherencyProtocol,
    
    /// Remote cache peers
    peers: Vec<CachePeer>,
}

pub enum CacheCoherencyProtocol {
    /// Modified, Exclusive, Shared, Invalid
    MESI,
    
    /// Modified, Owned, Exclusive, Shared, Invalid  
    MOESI,
    
    /// Dragon protocol
    Dragon,
}

pub struct CachedValue {
    data: Vec<u8>,
    state: CacheState,
    version: u64,
    last_accessed: Instant,
}

#[derive(Clone, Copy, Debug)]
pub enum CacheState {
    Modified,   // Local modifications not yet written back
    Exclusive,  // Only copy in the system
    Shared,     // Multiple read copies exist
    Invalid,    // Stale data
}

impl DistributedCache {
    pub async fn get(&self, key: &Key) -> Result<Option<Vec<u8>>> {
        // Check local cache first
        if let Some(cached) = self.local.read().await.get(key) {
            match cached.state {
                CacheState::Modified | CacheState::Exclusive | CacheState::Shared => {
                    return Ok(Some(cached.data.clone()));
                }
                CacheState::Invalid => {
                    // Need to fetch fresh copy
                }
            }
        }
        
        // Request from peers
        let value = self.fetch_from_peers(key).await?;
        
        if let Some(data) = &value {
            // Update local cache
            self.local.write().await.insert(key.clone(), CachedValue {
                data: data.clone(),
                state: CacheState::Shared,
                version: self.get_version(key).await?,
                last_accessed: Instant::now(),
            });
        }
        
        Ok(value)
    }
    
    pub async fn put(&self, key: &Key, value: Vec<u8>) -> Result<()> {
        // Acquire exclusive access
        self.coherency.acquire_exclusive(key).await?;
        
        // Update local cache
        self.local.write().await.insert(key.clone(), CachedValue {
            data: value.clone(),
            state: CacheState::Modified,
            version: self.increment_version(key).await?,
            last_accessed: Instant::now(),
        });
        
        // Invalidate other caches
        self.invalidation_bus.broadcast_invalidation(key).await?;
        
        Ok(())
    }
}
```

## Time-Series Storage

### Optimized for Sequential Data

```rust
pub struct TimeSeriesStorage {
    /// Active write buffer
    write_buffer: Arc<RwLock<TimeSeriesBuffer>>,
    
    /// Immutable segments
    segments: Arc<RwLock<Vec<TimeSeriesSegment>>>,
    
    /// Downsampling engine
    downsampler: Downsampler,
    
    /// Retention policy
    retention: RetentionPolicy,
}

pub struct TimeSeriesBuffer {
    points: Vec<DataPoint>,
    start_time: SystemTime,
    end_time: SystemTime,
    capacity: usize,
}

pub struct DataPoint {
    timestamp: u64,
    value: f64,
    tags: HashMap<String, String>,
}

impl TimeSeriesStorage {
    pub async fn write(&self, metric: &str, value: f64, tags: HashMap<String, String>) {
        let point = DataPoint {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            value,
            tags,
        };
        
        let mut buffer = self.write_buffer.write().await;
        buffer.points.push(point);
        
        // Flush if buffer is full
        if buffer.points.len() >= buffer.capacity {
            self.flush_buffer().await.unwrap();
        }
    }
    
    pub async fn query(
        &self,
        metric: &str,
        start: SystemTime,
        end: SystemTime,
        aggregation: AggregationFunction,
    ) -> Result<Vec<AggregatedPoint>> {
        let mut results = Vec::new();
        
        // Query segments in time range
        let segments = self.segments.read().await;
        for segment in segments.iter() {
            if segment.overlaps(start, end) {
                let points = segment.query(metric, start, end)?;
                results.extend(points);
            }
        }
        
        // Apply aggregation
        Ok(self.aggregate(results, aggregation))
    }
}
```

## Testing Storage Systems

### Chaos Testing for Durability

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_crash_recovery() {
        let storage = TieredStorage::new(test_config());
        
        // Write data
        for i in 0..1000 {
            storage.put(&format!("key_{}", i), &format!("value_{}", i)).await.unwrap();
        }
        
        // Simulate crash
        drop(storage);
        
        // Recover
        let recovered = TieredStorage::recover(test_config()).await.unwrap();
        
        // Verify all data recovered
        for i in 0..1000 {
            let value = recovered.get(&format!("key_{}", i)).await.unwrap();
            assert_eq!(value, format!("value_{}", i).as_bytes());
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_transactions() {
        let storage = MvccStorage::new();
        
        // Start multiple concurrent transactions
        let handles: Vec<_> = (0..100)
            .map(|i| {
                let storage = storage.clone();
                tokio::spawn(async move {
                    let tx = storage.begin_transaction().await;
                    
                    // Read-modify-write
                    let key = format!("counter_{}", i % 10);
                    let current = storage.get(&tx, &key).await.unwrap()
                        .and_then(|v| String::from_utf8(v).ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    
                    storage.put(&tx, &key, (current + 1).to_string()).await.unwrap();
                    storage.commit(tx).await
                })
            })
            .collect();
        
        // Wait for all transactions
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // Verify consistency
        for i in 0..10 {
            let key = format!("counter_{}", i);
            let value = storage.get_latest(&key).await.unwrap();
            assert_eq!(value, b"10");
        }
    }
}
```

## Conclusion

Production storage systems are the foundation upon which all distributed applications stand. Through BitCraps' implementation, we've explored the complex interplay between durability, performance, consistency, and scalability that defines modern storage architectures.

Key insights from this chapter:

1. **Write-ahead logging** ensures durability even during crashes
2. **MVCC** enables concurrent access without locking
3. **Compression and deduplication** maximize storage efficiency
4. **Sharding** enables horizontal scaling
5. **Cache coherency** maintains consistency in distributed caches
6. **Time-series optimization** handles sequential data efficiently

Remember: Storage is not just about persisting bytes—it's about preserving the state and history of your entire system while providing fast, reliable access at any scale.