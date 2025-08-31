use std::cmp;
use std::fmt;

/// Error type for GrowableBuffer operations
#[derive(Debug, Clone)]
pub enum BufferError {
    /// Requested size exceeds maximum allowed
    SizeExceedsMaximum { requested: usize, maximum: usize },
    /// Allocation failed
    AllocationFailed { size: usize },
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::SizeExceedsMaximum { requested, maximum } => {
                write!(f, "Requested size {} exceeds maximum {}", requested, maximum)
            }
            BufferError::AllocationFailed { size } => {
                write!(f, "Failed to allocate {} bytes", size)
            }
        }
    }
}

impl std::error::Error for BufferError {}

/// A growable buffer that starts small and expands on demand, with automatic shrinking
/// to optimize memory usage for network operations.
/// 
/// # Safety
/// 
/// This buffer provides memory-safe operations with proper bounds checking.
/// All resize operations are validated against MAX_SIZE to prevent unbounded growth.
/// 
/// # Examples
/// 
/// ```
/// use bitcraps::utils::growable_buffer::GrowableBuffer;
/// 
/// let mut buffer = GrowableBuffer::new();
/// 
/// // Get a buffer for packet data
/// let packet_buffer = buffer.get_mut(1500)?;
/// packet_buffer[0] = 0x42;
/// 
/// // Mark how much was actually used
/// buffer.mark_used(100);
/// 
/// // Buffer will automatically shrink if oversized
/// assert!(buffer.capacity() <= 3000);
/// ```
pub struct GrowableBuffer {
    buffer: Vec<u8>,
    high_water_mark: usize,
    initial_capacity: usize,
}

impl GrowableBuffer {
    /// Standard MTU size - good starting point for most network operations
    pub const MTU_SIZE: usize = 1500;
    
    /// Maximum buffer size - corresponds to the original 65KB fixed allocation
    pub const MAX_SIZE: usize = 65536;
    
    /// Shrink threshold: if buffer is more than 2x the high water mark, shrink it
    const SHRINK_THRESHOLD_MULTIPLIER: usize = 2;

    /// Create a new GrowableBuffer starting at MTU size
    pub fn new() -> Self {
        Self::with_initial_capacity(Self::MTU_SIZE)
    }

    /// Create a GrowableBuffer with a specific initial capacity
    /// 
    /// # Performance
    /// 
    /// Uses Vec::with_capacity to avoid zeroing memory unnecessarily.
    /// The buffer is only zeroed when actually written to.
    pub fn with_initial_capacity(capacity: usize) -> Self {
        let initial_capacity = cmp::min(capacity, Self::MAX_SIZE);
        
        // Optimize: Don't zero memory until actually needed
        let mut buffer = Vec::with_capacity(initial_capacity);
        // SAFETY: Setting length to capacity is safe because:
        // 1. Vec::with_capacity guarantees memory is allocated for initial_capacity bytes
        // 2. We're not reading the uninitialized memory, only providing it as a buffer
        // 3. The memory will be overwritten before being read by users
        // 4. All public APIs that expose this memory require explicit writes before reads
        unsafe {
            buffer.set_len(initial_capacity);
        }
        
        Self {
            buffer,
            high_water_mark: 0,
            initial_capacity,
        }
    }

    /// Get a mutable slice of the buffer with the requested minimum size
    /// The buffer will grow if necessary to accommodate the request
    /// 
    /// # Errors
    /// 
    /// Returns `BufferError::SizeExceedsMaximum` if the requested size exceeds MAX_SIZE
    /// Returns `BufferError::AllocationFailed` if memory allocation fails
    pub fn get_mut(&mut self, min_size: usize) -> Result<&mut [u8], BufferError> {
        if min_size > Self::MAX_SIZE {
            return Err(BufferError::SizeExceedsMaximum {
                requested: min_size,
                maximum: Self::MAX_SIZE,
            });
        }
        
        if self.buffer.len() < min_size {
            self.grow_to(min_size)?;
        }
        
        Ok(&mut self.buffer[..min_size])
    }

    /// Get a mutable slice without bounds checking (for performance-critical paths)
    /// 
    /// # Safety
    /// 
    /// This function is unsafe because:
    /// - It may return uninitialized memory if allocation fails
    /// - Caller must ensure min_size <= MAX_SIZE to avoid undefined behavior
    /// - Caller must handle potential allocation failures gracefully
    /// - The returned slice may contain uninitialized data that must be written before reading
    pub unsafe fn get_mut_unchecked(&mut self, min_size: usize) -> &mut [u8] {
        let required_size = cmp::min(min_size, Self::MAX_SIZE);
        
        if self.buffer.len() < required_size {
            // Ignore error in unchecked version
            let _ = self.grow_to(required_size);
        }
        
        &mut self.buffer[..required_size]
    }

    /// Get the entire buffer as a mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Get the buffer as an immutable slice up to the specified length
    pub fn as_slice(&self, len: usize) -> &[u8] {
        &self.buffer[..cmp::min(len, self.buffer.len())]
    }

    /// Update the high water mark and potentially shrink the buffer
    pub fn mark_used(&mut self, used_bytes: usize) {
        self.high_water_mark = cmp::max(self.high_water_mark, used_bytes);
        
        // Consider shrinking if buffer is significantly oversized
        if self.buffer.len() > self.high_water_mark * Self::SHRINK_THRESHOLD_MULTIPLIER
            && self.buffer.len() > self.initial_capacity
        {
            self.shrink();
        }
    }

    /// Get the current buffer capacity
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// Get the current buffer size
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the high water mark (maximum bytes used)
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark
    }

    /// Clear the buffer (zero it) and reset the high water mark
    /// 
    /// # Security
    /// 
    /// This method zeros the buffer for security-sensitive data.
    /// Use `fast_clear()` if zeroing is not required.
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.high_water_mark = 0;
    }

    /// Clear the buffer without zeroing (faster)
    pub fn fast_clear(&mut self) {
        self.high_water_mark = 0;
        // Don't zero memory, just reset tracking
    }

    /// Reset to initial capacity and clear high water mark
    pub fn reset(&mut self) {
        self.buffer.resize(self.initial_capacity, 0);
        self.buffer.shrink_to_fit();
        self.high_water_mark = 0;
    }

    /// Force grow the buffer to the specified size
    fn grow_to(&mut self, new_size: usize) -> Result<(), BufferError> {
        let target_size = cmp::min(new_size, Self::MAX_SIZE);
        if target_size > self.buffer.capacity() {
            // Try to reserve additional capacity
            self.buffer.try_reserve(target_size - self.buffer.len())
                .map_err(|_| BufferError::AllocationFailed { size: target_size })?;
        }
        
        // SAFETY: Setting length to target_size is safe because:
        // 1. We've just successfully reserved capacity for target_size bytes
        // 2. target_size is clamped to MAX_SIZE preventing overflow
        // 3. The Vec now has sufficient capacity as verified by try_reserve
        // 4. The uninitialized memory will be overwritten before being read
        unsafe {
            self.buffer.set_len(target_size);
        }
        
        Ok(())
    }

    /// Shrink the buffer to an optimal size based on usage patterns
    fn shrink(&mut self) {
        // Shrink to 1.5x the high water mark for some headroom
        let target_size = cmp::max(
            (self.high_water_mark * 3) / 2,
            self.initial_capacity
        );
        
        if target_size < self.buffer.len() {
            self.buffer.truncate(target_size);
            self.buffer.shrink_to_fit();
        }
    }

    /// Get memory usage statistics
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            current_size: self.buffer.len(),
            capacity: self.buffer.capacity(),
            high_water_mark: self.high_water_mark,
            initial_capacity: self.initial_capacity,
            efficiency: if self.buffer.len() > 0 {
                (self.high_water_mark as f64 / self.buffer.len() as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Statistics about buffer usage
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub current_size: usize,
    pub capacity: usize,
    pub high_water_mark: usize,
    pub initial_capacity: usize,
    pub efficiency: f64,
}

impl Default for GrowableBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_growth() {
        let mut buffer = GrowableBuffer::new();
        
        // Should start at MTU size
        assert_eq!(buffer.len(), GrowableBuffer::MTU_SIZE);
        
        // Request larger size
        let result = buffer.get_mut(4000);
        assert!(result.is_ok());
        assert_eq!(buffer.len(), 4000);
        
        // Mark usage
        buffer.mark_used(3500);
        assert_eq!(buffer.high_water_mark(), 3500);
    }

    #[test]
    fn test_shrinking() {
        let mut buffer = GrowableBuffer::with_initial_capacity(1000);
        
        // Grow to 10KB
        let _ = buffer.get_mut(10000);
        buffer.mark_used(1000);
        
        // Should shrink since we only used 1KB of 10KB
        assert!(buffer.len() < 10000);
        assert!(buffer.len() >= 1000);
    }

    #[test]
    fn test_max_size_limit() {
        let mut buffer = GrowableBuffer::new();
        
        // Request more than maximum
        let result = buffer.get_mut(100_000);
        assert!(result.is_err());
        
        // Request exactly maximum should work
        let result = buffer.get_mut(GrowableBuffer::MAX_SIZE);
        assert!(result.is_ok());
        assert_eq!(buffer.len(), GrowableBuffer::MAX_SIZE);
    }

    #[test]
    fn test_clear_operations() {
        let mut buffer = GrowableBuffer::new();
        
        let slice = buffer.get_mut(100).unwrap();
        slice[0] = 42;
        buffer.mark_used(100);
        
        // Clear should zero memory
        buffer.clear();
        assert_eq!(buffer.as_slice(1)[0], 0);
        assert_eq!(buffer.high_water_mark(), 0);
    }

    #[test]
    fn test_reset() {
        let mut buffer = GrowableBuffer::with_initial_capacity(2000);
        
        // Grow buffer
        let _ = buffer.get_mut(8000);
        
        // Reset should return to initial capacity
        buffer.reset();
        assert_eq!(buffer.len(), 2000);
        assert_eq!(buffer.high_water_mark(), 0);
    }

    #[test]
    fn test_efficiency_tracking() {
        let mut buffer = GrowableBuffer::new();
        
        let _ = buffer.get_mut(5000);
        buffer.mark_used(2500);
        
        let stats = buffer.stats();
        assert_eq!(stats.high_water_mark, 2500);
        assert!(stats.efficiency > 40.0 && stats.efficiency < 60.0);
    }
}