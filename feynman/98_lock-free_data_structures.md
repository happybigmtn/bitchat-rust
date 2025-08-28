# Chapter 98: Lock-Free Data Structures

*Maurice Herlihy's groundbreaking 1991 paper "Wait-free synchronization" introduced the world to a radical idea: data structures that could handle concurrent access without locks, mutexes, or any blocking synchronization. It seemed impossible—how can multiple threads safely modify shared data without coordination? The answer lay in the fundamental properties of atomic operations and the careful choreography of memory ordering.*

## The Lock-Free Revolution

Before lock-free data structures, concurrent programming meant locks, and locks meant problems: deadlock, priority inversion, convoy effects, and performance bottlenecks. In 1974, Butler Lampson and David Redell observed that "the use of locks leads to all sorts of difficulties." But it wasn't until the advent of compare-and-swap (CAS) operations in hardware that true alternatives became possible.

The theoretical foundation came from Herlihy's hierarchy of consensus numbers—atomic operations ranked by their ability to solve the consensus problem among n concurrent processes. Load-link/store-conditional and compare-and-swap have infinite consensus numbers, making them universal building blocks for lock-free algorithms.

## Understanding Lock-Free Programming

Lock-free programming is fundamentally different from traditional concurrent programming. Instead of preventing access through mutual exclusion, lock-free algorithms ensure progress through careful coordination of atomic operations and retry loops.

```rust
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::ptr;
use std::mem;

/// Memory ordering explanation:
/// - Relaxed: No ordering guarantees except for this specific atomic variable
/// - Acquire: Prevents memory reordering of the read-acquire with any read or write after it
/// - Release: Prevents memory reordering of the write-release with any read or write before it
/// - AcqRel: Both acquire and release semantics
/// - SeqCst: Sequential consistency - total order on all SeqCst operations

/// Lock-free stack using Treiber's algorithm
pub struct LockFreeStack<T> {
    head: AtomicPtr<Node<T>>,
}

struct Node<T> {
    data: T,
    next: *mut Node<T>,
}

impl<T> LockFreeStack<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }
    
    /// Push an element onto the stack
    /// This operation is lock-free: at least one thread makes progress
    pub fn push(&self, data: T) {
        let new_node = Box::into_raw(Box::new(Node {
            data,
            next: ptr::null_mut(),
        }));
        
        // Classic lock-free retry loop
        loop {
            let head = self.head.load(Ordering::Acquire);
            unsafe {
                (*new_node).next = head;
            }
            
            // The critical compare-and-swap operation
            match self.head.compare_exchange_weak(
                head,
                new_node,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break, // Success!
                Err(_) => {
                    // Another thread modified head, retry
                    // The "weak" variant can fail spuriously on some architectures
                    continue;
                }
            }
        }
    }
    
    /// Pop an element from the stack
    /// Returns None if stack is empty
    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            
            if head.is_null() {
                return None;
            }
            
            let next = unsafe { (*head).next };
            
            // Attempt to update head to next
            match self.head.compare_exchange_weak(
                head,
                next,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully updated head
                    let data = unsafe { Box::from_raw(head).data };
                    return Some(data);
                }
                Err(_) => {
                    // Another thread modified head, retry
                    continue;
                }
            }
        }
    }
    
    /// Check if stack is empty
    /// Note: This is just a snapshot and may be stale immediately
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Acquire).is_null()
    }
}

impl<T> Drop for LockFreeStack<T> {
    fn drop(&mut self) {
        // Clean up remaining nodes
        while self.pop().is_some() {}
    }
}

// Safety: LockFreeStack can be safely shared between threads
unsafe impl<T: Send> Send for LockFreeStack<T> {}
unsafe impl<T: Send> Sync for LockFreeStack<T> {}
```

## The ABA Problem and Solutions

The ABA problem is the most notorious challenge in lock-free programming. It occurs when a value changes from A to B and back to A, making a CAS operation succeed even though the data structure's state has changed.

```rust
use std::sync::atomic::{AtomicU64, AtomicPtr, Ordering};
use std::marker::PhantomData;

/// Hazard pointer for safe memory reclamation
/// This prevents the ABA problem by ensuring freed memory isn't reused
pub struct HazardPointer<T> {
    pointer: AtomicPtr<T>,
    hazard: AtomicPtr<T>,
    _phantom: PhantomData<T>,
}

impl<T> HazardPointer<T> {
    pub fn new() -> Self {
        Self {
            pointer: AtomicPtr::new(ptr::null_mut()),
            hazard: AtomicPtr::new(ptr::null_mut()),
            _phantom: PhantomData,
        }
    }
    
    /// Load a pointer safely, protecting it from reclamation
    pub fn load(&self) -> *mut T {
        loop {
            let ptr = self.pointer.load(Ordering::Acquire);
            self.hazard.store(ptr, Ordering::Release);
            
            // Verify the pointer hasn't changed
            if self.pointer.load(Ordering::Acquire) == ptr {
                return ptr;
            }
            // If it changed, retry
        }
    }
    
    /// Clear the hazard pointer
    pub fn clear(&self) {
        self.hazard.store(ptr::null_mut(), Ordering::Release);
    }
}

/// Tagged pointer to solve ABA problem with generation counting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaggedPtr<T> {
    ptr: *mut T,
    tag: usize,
}

impl<T> TaggedPtr<T> {
    pub fn new(ptr: *mut T, tag: usize) -> Self {
        Self { ptr, tag }
    }
    
    pub fn null() -> Self {
        Self {
            ptr: ptr::null_mut(),
            tag: 0,
        }
    }
    
    pub fn ptr(&self) -> *mut T {
        self.ptr
    }
    
    pub fn tag(&self) -> usize {
        self.tag
    }
    
    pub fn with_incremented_tag(&self) -> Self {
        Self {
            ptr: self.ptr,
            tag: self.tag.wrapping_add(1),
        }
    }
}

/// Lock-free stack with ABA protection using tagged pointers
pub struct ABAFreeStack<T> {
    head: AtomicU64,  // Packed pointer and tag
}

impl<T> ABAFreeStack<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
        }
    }
    
    fn pack_ptr(ptr: *mut Node<T>, tag: u32) -> u64 {
        // Pack pointer and tag into 64 bits
        // Assumes 48-bit pointers (x86_64)
        let ptr_bits = ptr as u64;
        let tag_bits = (tag as u64) << 48;
        ptr_bits | tag_bits
    }
    
    fn unpack_ptr(packed: u64) -> (*mut Node<T>, u32) {
        let ptr = (packed & 0x0000_ffff_ffff_ffff) as *mut Node<T>;
        let tag = (packed >> 48) as u32;
        (ptr, tag)
    }
    
    pub fn push(&self, data: T) {
        let new_node = Box::into_raw(Box::new(Node {
            data,
            next: ptr::null_mut(),
        }));
        
        loop {
            let head_packed = self.head.load(Ordering::Acquire);
            let (head_ptr, tag) = Self::unpack_ptr(head_packed);
            
            unsafe {
                (*new_node).next = head_ptr;
            }
            
            let new_head = Self::pack_ptr(new_node, tag.wrapping_add(1));
            
            match self.head.compare_exchange_weak(
                head_packed,
                new_head,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }
    
    pub fn pop(&self) -> Option<T> {
        loop {
            let head_packed = self.head.load(Ordering::Acquire);
            let (head_ptr, tag) = Self::unpack_ptr(head_packed);
            
            if head_ptr.is_null() {
                return None;
            }
            
            let next = unsafe { (*head_ptr).next };
            let new_head = Self::pack_ptr(next, tag.wrapping_add(1));
            
            match self.head.compare_exchange_weak(
                head_packed,
                new_head,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let data = unsafe { Box::from_raw(head_ptr).data };
                    return Some(data);
                }
                Err(_) => continue,
            }
        }
    }
}
```

## Lock-Free Queue Implementation

The Michael & Scott queue is the gold standard for lock-free FIFO queues. It's significantly more complex than the stack due to the need to maintain both head and tail pointers.

```rust
use std::sync::atomic::{AtomicPtr, Ordering};
use std::ptr;

/// Lock-free queue using Michael & Scott algorithm
pub struct LockFreeQueue<T> {
    head: AtomicPtr<QueueNode<T>>,
    tail: AtomicPtr<QueueNode<T>>,
}

struct QueueNode<T> {
    data: Option<T>,
    next: AtomicPtr<QueueNode<T>>,
}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        // Create a dummy node
        let dummy = Box::into_raw(Box::new(QueueNode {
            data: None,
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        
        Self {
            head: AtomicPtr::new(dummy),
            tail: AtomicPtr::new(dummy),
        }
    }
    
    pub fn enqueue(&self, data: T) {
        let new_node = Box::into_raw(Box::new(QueueNode {
            data: Some(data),
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*tail).next.load(Ordering::Acquire) };
            
            // Check if tail is still the same
            if tail == self.tail.load(Ordering::Acquire) {
                if next.is_null() {
                    // tail->next is null, try to link new node
                    match unsafe {
                        (*tail).next.compare_exchange_weak(
                            next,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                    } {
                        Ok(_) => {
                            // Successfully linked, now try to swing tail
                            let _ = self.tail.compare_exchange_weak(
                                tail,
                                new_node,
                                Ordering::Release,
                                Ordering::Relaxed,
                            );
                            break;
                        }
                        Err(_) => continue,
                    }
                } else {
                    // tail->next is not null, try to swing tail forward
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                }
            }
        }
    }
    
    pub fn dequeue(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*head).next.load(Ordering::Acquire) };
            
            // Check if head is consistent
            if head == self.head.load(Ordering::Acquire) {
                if head == tail {
                    if next.is_null() {
                        // Queue is empty
                        return None;
                    }
                    
                    // Head and tail pointing to same node but there's a next
                    // Try to advance tail
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                } else {
                    // Head and tail are different
                    if next.is_null() {
                        // This shouldn't happen in correct implementation
                        continue;
                    }
                    
                    // Read data before CAS to avoid data races
                    let data = unsafe { (*next).data.take() };
                    
                    // Try to swing head to next node
                    match self.head.compare_exchange_weak(
                        head,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            // Successfully moved head, free old head node
                            unsafe { Box::from_raw(head) };
                            return data;
                        }
                        Err(_) => {
                            // Restore data if CAS failed
                            if let Some(d) = data {
                                unsafe { (*next).data = Some(d) };
                            }
                            continue;
                        }
                    }
                }
            }
        }
    }
    
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        let next = unsafe { (*head).next.load(Ordering::Acquire) };
        
        head == tail && next.is_null()
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        // Clean up all remaining nodes
        while self.dequeue().is_some() {}
        
        // Clean up the dummy node
        let head = *self.head.get_mut();
        if !head.is_null() {
            unsafe { Box::from_raw(head) };
        }
    }
}

unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeQueue<T> {}
```

## Lock-Free Hash Table

Hash tables present unique challenges for lock-free implementation due to the need to handle collisions and resizing.

```rust
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// Lock-free hash table with linear probing
pub struct LockFreeHashMap<K, V> {
    buckets: Vec<AtomicPtr<Bucket<K, V>>>,
    size: AtomicUsize,
    capacity: usize,
}

struct Bucket<K, V> {
    key: K,
    value: V,
    hash: u64,
    deleted: bool,
}

const LOAD_FACTOR_THRESHOLD: f64 = 0.75;

impl<K, V> LockFreeHashMap<K, V> 
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(initial_capacity: usize) -> Self {
        let capacity = initial_capacity.next_power_of_two().max(16);
        let mut buckets = Vec::with_capacity(capacity);
        
        for _ in 0..capacity {
            buckets.push(AtomicPtr::new(ptr::null_mut()));
        }
        
        Self {
            buckets,
            size: AtomicUsize::new(0),
            capacity,
        }
    }
    
    fn hash_key(&self, key: &K) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
    
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        let hash = self.hash_key(&key);
        let mut index = (hash as usize) % self.capacity;
        
        loop {
            let bucket_ptr = self.buckets[index].load(Ordering::Acquire);
            
            if bucket_ptr.is_null() {
                // Empty slot, try to claim it
                let new_bucket = Box::into_raw(Box::new(Bucket {
                    key: key.clone(),
                    value: value.clone(),
                    hash,
                    deleted: false,
                }));
                
                match self.buckets[index].compare_exchange_weak(
                    bucket_ptr,
                    new_bucket,
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        self.size.fetch_add(1, Ordering::Relaxed);
                        return None;
                    }
                    Err(_) => {
                        // Another thread claimed this slot
                        unsafe { Box::from_raw(new_bucket) };
                        continue;
                    }
                }
            } else {
                // Slot occupied, check if it's our key
                let bucket = unsafe { &*bucket_ptr };
                
                if bucket.hash == hash && bucket.key == key && !bucket.deleted {
                    // Found existing key, this is more complex in practice
                    // as we'd need to atomically update the value
                    // For simplicity, we'll treat this as an error case
                    return Some(bucket.value.clone());
                }
            }
            
            // Linear probing
            index = (index + 1) % self.capacity;
        }
    }
    
    pub fn get(&self, key: &K) -> Option<V> {
        let hash = self.hash_key(key);
        let mut index = (hash as usize) % self.capacity;
        
        for _ in 0..self.capacity {
            let bucket_ptr = self.buckets[index].load(Ordering::Acquire);
            
            if bucket_ptr.is_null() {
                return None;
            }
            
            let bucket = unsafe { &*bucket_ptr };
            if bucket.hash == hash && bucket.key == *key && !bucket.deleted {
                return Some(bucket.value.clone());
            }
            
            index = (index + 1) % self.capacity;
        }
        
        None
    }
    
    pub fn remove(&self, key: &K) -> Option<V> {
        let hash = self.hash_key(key);
        let mut index = (hash as usize) % self.capacity;
        
        for _ in 0..self.capacity {
            let bucket_ptr = self.buckets[index].load(Ordering::Acquire);
            
            if bucket_ptr.is_null() {
                return None;
            }
            
            let bucket = unsafe { &mut *bucket_ptr };
            if bucket.hash == hash && bucket.key == *key && !bucket.deleted {
                // Mark as deleted (tombstone)
                bucket.deleted = true;
                self.size.fetch_sub(1, Ordering::Relaxed);
                return Some(bucket.value.clone());
            }
            
            index = (index + 1) % self.capacity;
        }
        
        None
    }
    
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
    
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
```

## Memory Reclamation Strategies

Safe memory reclamation is the biggest challenge in lock-free programming. Several strategies exist to solve this problem.

```rust
use std::sync::atomic::{AtomicPtr, AtomicUsize, AtomicBool, Ordering};
use std::sync::Arc;
use std::ptr;
use std::mem;

/// Epoch-based memory reclamation
pub struct EpochManager {
    global_epoch: AtomicUsize,
    participants: Vec<AtomicUsize>,
}

impl EpochManager {
    pub fn new(max_threads: usize) -> Self {
        let mut participants = Vec::with_capacity(max_threads);
        for _ in 0..max_threads {
            participants.push(AtomicUsize::new(0));
        }
        
        Self {
            global_epoch: AtomicUsize::new(1),
            participants,
        }
    }
    
    /// Enter a new epoch for the calling thread
    pub fn pin(&self, thread_id: usize) {
        let global = self.global_epoch.load(Ordering::Acquire);
        self.participants[thread_id].store(global, Ordering::Release);
    }
    
    /// Exit the current epoch
    pub fn unpin(&self, thread_id: usize) {
        self.participants[thread_id].store(0, Ordering::Release);
    }
    
    /// Try to advance the global epoch
    pub fn try_advance(&self) -> bool {
        let current_epoch = self.global_epoch.load(Ordering::Acquire);
        let next_epoch = current_epoch + 1;
        
        // Check if all participants are caught up
        for participant in &self.participants {
            let epoch = participant.load(Ordering::Acquire);
            if epoch != 0 && epoch != current_epoch {
                return false; // Someone is still in an old epoch
            }
        }
        
        // All caught up, advance epoch
        self.global_epoch.compare_exchange(
            current_epoch,
            next_epoch,
            Ordering::Release,
            Ordering::Relaxed,
        ).is_ok()
    }
    
    /// Get current global epoch
    pub fn current_epoch(&self) -> usize {
        self.global_epoch.load(Ordering::Acquire)
    }
}

/// Deferred deletion with epoch protection
pub struct DeferredReclamation<T> {
    epoch_manager: Arc<EpochManager>,
    garbage: Vec<AtomicPtr<EpochNode<T>>>,
}

struct EpochNode<T> {
    data: T,
    epoch: usize,
    next: AtomicPtr<EpochNode<T>>,
}

impl<T> DeferredReclamation<T> {
    pub fn new(epoch_manager: Arc<EpochManager>) -> Self {
        Self {
            epoch_manager,
            garbage: Vec::new(),
        }
    }
    
    /// Schedule an object for deletion
    pub fn defer_delete(&self, data: T, thread_id: usize) {
        let current_epoch = self.epoch_manager.current_epoch();
        let node = Box::into_raw(Box::new(EpochNode {
            data,
            epoch: current_epoch,
            next: AtomicPtr::new(ptr::null_mut()),
        }));
        
        // Add to garbage list (simplified - would need thread-local lists in practice)
        // This is a simplified implementation
    }
    
    /// Collect garbage from old epochs
    pub fn collect_garbage(&self) {
        let current_epoch = self.epoch_manager.current_epoch();
        
        // Objects from 2 epochs ago are safe to delete
        let safe_epoch = current_epoch.saturating_sub(2);
        
        // Collect and free objects from safe epochs
        // Implementation details omitted for brevity
    }
}

/// Reference counting based reclamation (simplified)
pub struct RcuPointer<T> {
    ptr: AtomicPtr<RcuNode<T>>,
}

struct RcuNode<T> {
    data: T,
    ref_count: AtomicUsize,
}

impl<T> RcuPointer<T> {
    pub fn new(data: T) -> Self {
        let node = Box::into_raw(Box::new(RcuNode {
            data,
            ref_count: AtomicUsize::new(1),
        }));
        
        Self {
            ptr: AtomicPtr::new(node),
        }
    }
    
    pub fn load(&self) -> Option<RcuGuard<T>> {
        loop {
            let ptr = self.ptr.load(Ordering::Acquire);
            if ptr.is_null() {
                return None;
            }
            
            let node = unsafe { &*ptr };
            let old_count = node.ref_count.load(Ordering::Relaxed);
            
            if old_count == 0 {
                // Node is being deleted
                continue;
            }
            
            match node.ref_count.compare_exchange_weak(
                old_count,
                old_count + 1,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(RcuGuard { ptr });
                }
                Err(_) => continue,
            }
        }
    }
    
    pub fn store(&self, data: T) {
        let new_node = Box::into_raw(Box::new(RcuNode {
            data,
            ref_count: AtomicUsize::new(1),
        }));
        
        let old_ptr = self.ptr.swap(new_node, Ordering::Release);
        
        if !old_ptr.is_null() {
            unsafe { self.dec_ref(old_ptr) };
        }
    }
    
    unsafe fn dec_ref(&self, ptr: *mut RcuNode<T>) {
        let node = &*ptr;
        let old_count = node.ref_count.fetch_sub(1, Ordering::Release);
        
        if old_count == 1 {
            // Last reference, safe to delete
            Box::from_raw(ptr);
        }
    }
}

pub struct RcuGuard<T> {
    ptr: *mut RcuNode<T>,
}

impl<T> std::ops::Deref for RcuGuard<T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &(*self.ptr).data }
    }
}

impl<T> Drop for RcuGuard<T> {
    fn drop(&mut self) {
        unsafe {
            let node = &*self.ptr;
            let old_count = node.ref_count.fetch_sub(1, Ordering::Release);
            
            if old_count == 1 {
                Box::from_raw(self.ptr);
            }
        }
    }
}
```

## BitCraps Lock-Free Gaming Components

For BitCraps, lock-free data structures enable high-performance concurrent game state management.

```rust
/// Lock-free game state for BitCraps
pub struct LockFreeGameState {
    players: LockFreeQueue<PlayerId>,
    bets: LockFreeHashMap<PlayerId, BetAmount>,
    dice_results: LockFreeStack<DiceRoll>,
    round_number: AtomicUsize,
    phase: AtomicU8,  // 0: Waiting, 1: Betting, 2: Rolling, 3: Payout
}

#[derive(Clone, Copy)]
pub struct DiceRoll {
    die1: u8,
    die2: u8,
    timestamp: u64,
}

impl LockFreeGameState {
    pub fn new() -> Self {
        Self {
            players: LockFreeQueue::new(),
            bets: LockFreeHashMap::new(64),
            dice_results: LockFreeStack::new(),
            round_number: AtomicUsize::new(0),
            phase: AtomicU8::new(0),
        }
    }
    
    /// Add a player to the game (lock-free)
    pub fn add_player(&self, player_id: PlayerId) {
        self.players.enqueue(player_id);
    }
    
    /// Place a bet (lock-free)
    pub fn place_bet(&self, player_id: PlayerId, amount: BetAmount) -> Result<(), GameError> {
        // Only allow betting in betting phase
        if self.phase.load(Ordering::Acquire) != 1 {
            return Err(GameError::WrongPhase);
        }
        
        self.bets.insert(player_id, amount);
        Ok(())
    }
    
    /// Record dice roll result (lock-free)
    pub fn record_dice_roll(&self, die1: u8, die2: u8) {
        let roll = DiceRoll {
            die1,
            die2,
            timestamp: get_timestamp(),
        };
        
        self.dice_results.push(roll);
    }
    
    /// Advance to next phase atomically
    pub fn advance_phase(&self) -> u8 {
        let current = self.phase.load(Ordering::Acquire);
        let next_phase = (current + 1) % 4;
        
        loop {
            match self.phase.compare_exchange_weak(
                current,
                next_phase,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return next_phase,
                Err(actual) => {
                    if actual != current {
                        return actual;  // Phase already changed
                    }
                }
            }
        }
    }
    
    /// Get current phase
    pub fn current_phase(&self) -> GamePhase {
        match self.phase.load(Ordering::Acquire) {
            0 => GamePhase::Waiting,
            1 => GamePhase::Betting,
            2 => GamePhase::Rolling,
            3 => GamePhase::Payout,
            _ => GamePhase::Waiting, // Default
        }
    }
}

/// Lock-free peer connection manager
pub struct LockFreePeerManager {
    active_peers: LockFreeHashMap<PeerId, PeerConnection>,
    connection_count: AtomicUsize,
    max_connections: usize,
}

impl LockFreePeerManager {
    pub fn new(max_connections: usize) -> Self {
        Self {
            active_peers: LockFreeHashMap::new(max_connections * 2),
            connection_count: AtomicUsize::new(0),
            max_connections,
        }
    }
    
    /// Add a peer connection (with connection limit)
    pub fn add_peer(&self, peer_id: PeerId, connection: PeerConnection) -> Result<(), NetworkError> {
        // Check connection limit
        let current_count = self.connection_count.load(Ordering::Acquire);
        if current_count >= self.max_connections {
            return Err(NetworkError::ConnectionLimit);
        }
        
        // Try to add peer
        match self.active_peers.insert(peer_id, connection) {
            None => {
                self.connection_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Some(_) => {
                // Peer already existed, no count change needed
                Ok(())
            }
        }
    }
    
    /// Remove a peer connection
    pub fn remove_peer(&self, peer_id: &PeerId) -> Option<PeerConnection> {
        match self.active_peers.remove(peer_id) {
            Some(connection) => {
                self.connection_count.fetch_sub(1, Ordering::Relaxed);
                Some(connection)
            }
            None => None,
        }
    }
    
    /// Get peer connection
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<PeerConnection> {
        self.active_peers.get(peer_id)
    }
    
    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connection_count.load(Ordering::Acquire)
    }
}

fn get_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
```

## Performance Analysis and Benchmarking

Understanding the performance characteristics of lock-free data structures is crucial for making design decisions.

```rust
use std::time::{Instant, Duration};
use std::thread;
use std::sync::Arc;

/// Benchmark framework for lock-free data structures
pub struct LockFreeBenchmark {
    thread_count: usize,
    operations_per_thread: usize,
    operation_mix: OperationMix,
}

#[derive(Clone)]
pub struct OperationMix {
    pub read_percentage: f32,
    pub write_percentage: f32,
    pub delete_percentage: f32,
}

impl LockFreeBenchmark {
    pub fn new(threads: usize, ops_per_thread: usize, mix: OperationMix) -> Self {
        Self {
            thread_count: threads,
            operations_per_thread: ops_per_thread,
            operation_mix: mix,
        }
    }
    
    /// Benchmark stack performance
    pub fn benchmark_stack<T: Clone + Send + Sync + 'static>(&self, stack: Arc<LockFreeStack<T>>, test_value: T) -> BenchmarkResult {
        let start = Instant::now();
        let mut handles = Vec::new();
        
        // Spawn benchmark threads
        for thread_id in 0..self.thread_count {
            let stack = stack.clone();
            let value = test_value.clone();
            let ops = self.operations_per_thread;
            let mix = self.operation_mix.clone();
            
            let handle = thread::spawn(move || {
                let mut rng = rand::thread_rng();
                let mut operations = 0;
                let thread_start = Instant::now();
                
                for _ in 0..ops {
                    let op_type: f32 = rng.gen();
                    
                    if op_type < mix.read_percentage {
                        // Pop operation
                        let _ = stack.pop();
                    } else {
                        // Push operation
                        stack.push(value.clone());
                    }
                    
                    operations += 1;
                }
                
                ThreadResult {
                    thread_id,
                    operations,
                    duration: thread_start.elapsed(),
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads
        let mut thread_results = Vec::new();
        for handle in handles {
            thread_results.push(handle.join().unwrap());
        }
        
        let total_duration = start.elapsed();
        
        BenchmarkResult::new(thread_results, total_duration)
    }
    
    /// Benchmark queue performance  
    pub fn benchmark_queue<T: Clone + Send + Sync + 'static>(&self, queue: Arc<LockFreeQueue<T>>, test_value: T) -> BenchmarkResult {
        let start = Instant::now();
        let mut handles = Vec::new();
        
        // Pre-populate queue for read operations
        for _ in 0..self.operations_per_thread {
            queue.enqueue(test_value.clone());
        }
        
        for thread_id in 0..self.thread_count {
            let queue = queue.clone();
            let value = test_value.clone();
            let ops = self.operations_per_thread;
            let mix = self.operation_mix.clone();
            
            let handle = thread::spawn(move || {
                let mut rng = rand::thread_rng();
                let mut operations = 0;
                let thread_start = Instant::now();
                
                for _ in 0..ops {
                    let op_type: f32 = rng.gen();
                    
                    if op_type < mix.read_percentage {
                        let _ = queue.dequeue();
                    } else {
                        queue.enqueue(value.clone());
                    }
                    
                    operations += 1;
                }
                
                ThreadResult {
                    thread_id,
                    operations,
                    duration: thread_start.elapsed(),
                }
            });
            
            handles.push(handle);
        }
        
        let mut thread_results = Vec::new();
        for handle in handles {
            thread_results.push(handle.join().unwrap());
        }
        
        let total_duration = start.elapsed();
        BenchmarkResult::new(thread_results, total_duration)
    }
}

pub struct ThreadResult {
    pub thread_id: usize,
    pub operations: usize,
    pub duration: Duration,
}

pub struct BenchmarkResult {
    pub total_operations: usize,
    pub total_duration: Duration,
    pub throughput: f64, // ops per second
    pub thread_results: Vec<ThreadResult>,
}

impl BenchmarkResult {
    fn new(thread_results: Vec<ThreadResult>, total_duration: Duration) -> Self {
        let total_operations: usize = thread_results.iter().map(|r| r.operations).sum();
        let throughput = total_operations as f64 / total_duration.as_secs_f64();
        
        Self {
            total_operations,
            total_duration,
            throughput,
            thread_results,
        }
    }
    
    pub fn print_summary(&self) {
        println!("Benchmark Results:");
        println!("  Total Operations: {}", self.total_operations);
        println!("  Total Duration: {:?}", self.total_duration);
        println!("  Throughput: {:.2} ops/sec", self.throughput);
        println!("  Threads: {}", self.thread_results.len());
        
        let avg_thread_throughput: f64 = self.thread_results.iter()
            .map(|r| r.operations as f64 / r.duration.as_secs_f64())
            .sum::<f64>() / self.thread_results.len() as f64;
            
        println!("  Avg Thread Throughput: {:.2} ops/sec", avg_thread_throughput);
    }
}
```

## Testing Lock-Free Algorithms

Testing lock-free algorithms requires special attention to race conditions and memory ordering issues.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::collections::HashSet;
    
    #[test]
    fn test_stack_basic_operations() {
        let stack = LockFreeStack::new();
        
        // Test empty stack
        assert!(stack.is_empty());
        assert_eq!(stack.pop(), None);
        
        // Test single element
        stack.push(42);
        assert!(!stack.is_empty());
        assert_eq!(stack.pop(), Some(42));
        assert!(stack.is_empty());
        
        // Test multiple elements (LIFO order)
        stack.push(1);
        stack.push(2);
        stack.push(3);
        
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.pop(), None);
    }
    
    #[test]
    fn test_stack_concurrent_push_pop() {
        let stack = Arc::new(LockFreeStack::new());
        let num_threads = 8;
        let items_per_thread = 1000;
        
        let mut handles = Vec::new();
        
        // Spawn producer threads
        for i in 0..num_threads {
            let stack = stack.clone();
            let handle = thread::spawn(move || {
                for j in 0..items_per_thread {
                    stack.push(i * items_per_thread + j);
                }
            });
            handles.push(handle);
        }
        
        // Spawn consumer threads
        for _ in 0..num_threads {
            let stack = stack.clone();
            let handle = thread::spawn(move || {
                let mut popped = Vec::new();
                for _ in 0..items_per_thread {
                    while let Some(value) = stack.pop() {
                        popped.push(value);
                        if popped.len() >= items_per_thread {
                            break;
                        }
                    }
                    thread::yield_now();
                }
                popped
            });
            handles.push(handle);
        }
        
        // Wait for all threads and collect results
        let mut all_values = Vec::new();
        for handle in handles {
            if let Ok(values) = handle.join() {
                if let Ok(values) = values.downcast::<Vec<usize>>() {
                    all_values.extend(*values);
                }
            }
        }
        
        // Verify all values were processed exactly once
        let expected_total = num_threads * items_per_thread;
        assert_eq!(all_values.len(), expected_total);
        
        let unique_values: HashSet<_> = all_values.into_iter().collect();
        assert_eq!(unique_values.len(), expected_total);
    }
    
    #[test]
    fn test_queue_fifo_order() {
        let queue = LockFreeQueue::new();
        
        // Test empty queue
        assert!(queue.is_empty());
        assert_eq!(queue.dequeue(), None);
        
        // Test FIFO order
        queue.enqueue(1);
        queue.enqueue(2);
        queue.enqueue(3);
        
        assert!(!queue.is_empty());
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.dequeue(), None);
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_queue_concurrent_operations() {
        let queue = Arc::new(LockFreeQueue::new());
        let num_producers = 4;
        let num_consumers = 4;
        let items_per_producer = 1000;
        
        let mut handles = Vec::new();
        
        // Spawn producer threads
        for i in 0..num_producers {
            let queue = queue.clone();
            let handle = thread::spawn(move || {
                for j in 0..items_per_producer {
                    queue.enqueue(i * items_per_producer + j);
                }
            });
            handles.push(handle);
        }
        
        // Spawn consumer threads
        let consumed_counts = Arc::new(std::sync::Mutex::new(Vec::new()));
        for _ in 0..num_consumers {
            let queue = queue.clone();
            let counts = consumed_counts.clone();
            let handle = thread::spawn(move || {
                let mut local_count = 0;
                let mut consecutive_empty = 0;
                
                loop {
                    match queue.dequeue() {
                        Some(_) => {
                            local_count += 1;
                            consecutive_empty = 0;
                        }
                        None => {
                            consecutive_empty += 1;
                            if consecutive_empty > 1000 {
                                break; // Assume we're done
                            }
                            thread::yield_now();
                        }
                    }
                }
                
                counts.lock().unwrap().push(local_count);
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify total consumption
        let total_consumed: usize = consumed_counts.lock().unwrap().iter().sum();
        assert_eq!(total_consumed, num_producers * items_per_producer);
    }
    
    #[test]
    fn test_memory_ordering() {
        // Test that memory ordering works correctly
        let stack = Arc::new(LockFreeStack::new());
        let flag = Arc::new(AtomicBool::new(false));
        
        let stack_clone = stack.clone();
        let flag_clone = flag.clone();
        
        let producer = thread::spawn(move || {
            stack_clone.push(42);
            flag_clone.store(true, Ordering::Release);
        });
        
        let consumer = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                thread::yield_now();
            }
            // Due to release-acquire ordering, the stack push
            // must be visible here
            assert_eq!(stack.pop(), Some(42));
        });
        
        producer.join().unwrap();
        consumer.join().unwrap();
    }
}
```

## Common Pitfalls and Solutions

1. **ABA Problem**: Always use tagged pointers or hazard pointers
2. **Memory Ordering**: Understanding acquire-release semantics is crucial
3. **Memory Leaks**: Implement proper reclamation strategies
4. **Starvation**: Ensure lock-free doesn't become wait-free by accident
5. **Performance**: Lock-free isn't always faster—measure carefully

## Practical Exercises

1. **Implement Lock-Free Set**: Build a lock-free hash set
2. **Memory Reclamation**: Implement epoch-based reclamation
3. **Compare Performance**: Benchmark against mutex-based alternatives
4. **Handle ABA**: Implement and demonstrate ABA problem solutions
5. **BitCraps Integration**: Use lock-free structures in game logic

## Conclusion

Lock-free data structures represent one of the most challenging but rewarding areas of concurrent programming. They enable unprecedented scalability and performance, but at the cost of significant complexity. The key is understanding that lock-free programming is fundamentally different from traditional concurrent programming—it requires thinking in terms of atomic operations, memory ordering, and careful coordination rather than mutual exclusion.

In the context of BitCraps and distributed gaming, lock-free data structures provide the foundation for high-performance, low-latency systems that can handle thousands of concurrent players without traditional synchronization bottlenecks.

Remember: lock-free programming is not just about removing locks—it's about fundamentally rethinking how concurrent algorithms work.

## Additional Resources

- "The Art of Multiprocessor Programming" by Herlihy and Shavit
- "Is Parallel Programming Hard?" by Paul McKenney
- "Memory Barriers: a Hardware View for Software Hackers" by Paul McKenney
- Intel Threading Building Blocks (TBB) Documentation
- Rust Atomics and Locks by Mara Bos

---

*Next Chapter: [99: Production Deployment Strategies](./99_production_deployment_strategies.md)*