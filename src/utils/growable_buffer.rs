use std::cmp;

/// A growable buffer that starts small and expands on demand, with automatic shrinking
/// to optimize memory usage for network operations.
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
    pub fn with_initial_capacity(capacity: usize) -> Self {
        let initial_capacity = cmp::min(capacity, Self::MAX_SIZE);
        Self {
            buffer: vec![0u8; initial_capacity],
            high_water_mark: 0,
            initial_capacity,
        }
    }

    /// Get a mutable slice of the buffer with the requested minimum size
    /// The buffer will grow if necessary to accommodate the request
    pub fn get_mut(&mut self, min_size: usize) -> &mut [u8] {
        let required_size = cmp::min(min_size, Self::MAX_SIZE);
        
        if self.buffer.len() < required_size {
            self.grow_to(required_size);
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
        self.buffer.len()
    }

    /// Get the high water mark (maximum bytes used)
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark
    }

    /// Clear the buffer and reset the high water mark
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.high_water_mark = 0;
    }

    /// Reset to initial capacity and clear high water mark
    pub fn reset(&mut self) {
        self.buffer.resize(self.initial_capacity, 0);
        self.high_water_mark = 0;
    }

    /// Force grow the buffer to the specified size
    fn grow_to(&mut self, new_size: usize) {
        let target_size = cmp::min(new_size, Self::MAX_SIZE);
        if target_size > self.buffer.len() {
            self.buffer.resize(target_size, 0);
        }
    }

    /// Shrink the buffer to an optimal size based on usage patterns
    fn shrink(&mut self) {
        // Shrink to 1.5x the high water mark or initial capacity, whichever is larger
        let optimal_size = cmp::max(
            self.high_water_mark + (self.high_water_mark / 2),
            self.initial_capacity
        );
        
        if optimal_size < self.buffer.len() {
            self.buffer.resize(optimal_size, 0);
        }
    }
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
    fn test_initial_capacity() {
        let buffer = GrowableBuffer::new();
        assert_eq!(buffer.capacity(), GrowableBuffer::MTU_SIZE);
        assert_eq!(buffer.high_water_mark(), 0);
    }

    #[test]
    fn test_growth() {
        let mut buffer = GrowableBuffer::new();
        let large_slice = buffer.get_mut(8192);
        assert_eq!(large_slice.len(), 8192);
        assert!(buffer.capacity() >= 8192);
    }

    #[test]
    fn test_high_water_mark_tracking() {
        let mut buffer = GrowableBuffer::new();
        buffer.mark_used(2048);
        assert_eq!(buffer.high_water_mark(), 2048);
        
        buffer.mark_used(1024); // Should not decrease
        assert_eq!(buffer.high_water_mark(), 2048);
        
        buffer.mark_used(4096); // Should increase
        assert_eq!(buffer.high_water_mark(), 4096);
    }

    #[test]
    fn test_shrinking() {
        let mut buffer = GrowableBuffer::new();
        
        // Grow to large size
        buffer.get_mut(32768);
        assert!(buffer.capacity() >= 32768);
        
        // Mark small usage
        buffer.mark_used(1024);
        
        // Should shrink since capacity >> high_water_mark
        assert!(buffer.capacity() < 32768);
    }

    #[test]
    fn test_max_size_limit() {
        let mut buffer = GrowableBuffer::new();
        let max_slice = buffer.get_mut(GrowableBuffer::MAX_SIZE + 1000);
        assert_eq!(max_slice.len(), GrowableBuffer::MAX_SIZE);
    }

    #[test]
    fn test_reset() {
        let mut buffer = GrowableBuffer::new();
        buffer.get_mut(8192);
        buffer.mark_used(4096);
        
        buffer.reset();
        assert_eq!(buffer.capacity(), GrowableBuffer::MTU_SIZE);
        assert_eq!(buffer.high_water_mark(), 0);
    }
}