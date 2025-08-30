//! Memory management bridge between Rust and iOS
//!
//! This module provides safe memory management utilities for passing data
//! between Rust and Swift/Objective-C, handling ownership transfer and
//! memory lifecycle management.

use log::{debug, error, warn};
use std::ffi::{c_char, c_void, CStr, CString};
use std::mem;
use std::ptr;
use std::slice;

use crate::mobile::BitCrapsError;

/// Managed buffer for data transfer between Rust and iOS
#[repr(C)]
pub struct ManagedBuffer {
    /// Pointer to the data
    pub data: *mut u8,
    /// Length of the data
    pub length: usize,
    /// Capacity of the buffer
    pub capacity: usize,
    /// Whether this buffer is owned by Rust
    pub owned_by_rust: bool,
}

/// String wrapper for safe transfer to iOS
#[repr(C)]
pub struct ManagedString {
    /// C-style string pointer
    pub ptr: *mut c_char,
    /// Length of the string (excluding null terminator)
    pub length: usize,
    /// Whether this string is owned by Rust
    pub owned_by_rust: bool,
}

/// iOS callback function pointer types
pub type IosEventCallback = extern "C" fn(*const c_char, *const c_void, u32);
pub type IosErrorCallback = extern "C" fn(*const c_char);
pub type IosDataCallback = extern "C" fn(*const u8, u32);
pub type IosStringCallback = extern "C" fn(*const c_char);

impl ManagedBuffer {
    /// Create a new managed buffer from Rust data
    pub fn new_from_rust(data: Vec<u8>) -> Self {
        let mut boxed_data = data.into_boxed_slice();
        let ptr = boxed_data.as_mut_ptr();
        let length = boxed_data.len();
        let capacity = boxed_data.len();

        // Transfer ownership to avoid deallocation
        std::mem::forget(boxed_data);

        Self {
            data: ptr,
            length,
            capacity,
            owned_by_rust: true,
        }
    }

    /// Create a managed buffer from iOS-provided data (non-owning)
    pub fn new_from_ios(data_ptr: *const u8, length: usize) -> Result<Self, BitCrapsError> {
        if data_ptr.is_null() || length == 0 {
            return Err(BitCrapsError::InvalidInput {
                reason: "Null or empty data pointer from iOS".to_string(),
            });
        }

        Ok(Self {
            data: data_ptr as *mut u8,
            length,
            capacity: length,
            owned_by_rust: false,
        })
    }

    /// Get a slice view of the buffer data
    pub fn as_slice(&self) -> Result<&[u8], BitCrapsError> {
        if self.data.is_null() || self.length == 0 {
            return Err(BitCrapsError::InvalidInput {
                reason: "Invalid buffer data".to_string(),
            });
        }

        // SAFETY INVARIANTS:
        // 1. self.data must point to valid memory of at least self.length bytes
        // 2. Memory must remain valid for the lifetime of the returned slice
        // 3. No mutable aliasing - caller must not modify buffer while slice exists
        let slice = unsafe { slice::from_raw_parts(self.data, self.length) };
        Ok(slice)
    }

    /// Get a mutable slice view of the buffer data (only if owned by Rust)
    pub fn as_mut_slice(&mut self) -> Result<&mut [u8], BitCrapsError> {
        if !self.owned_by_rust {
            return Err(BitCrapsError::InvalidInput {
                reason: "Cannot get mutable slice of non-Rust owned buffer".to_string(),
            });
        }

        if self.data.is_null() || self.length == 0 {
            return Err(BitCrapsError::InvalidInput {
                reason: "Invalid buffer data".to_string(),
            });
        }

        // SAFETY INVARIANTS:
        // 1. self.data must point to valid, mutable memory of at least self.length bytes
        // 2. Memory must remain valid and not be accessed by other threads during slice lifetime
        // 3. owned_by_rust guarantees we have exclusive access to this memory
        let slice = unsafe { slice::from_raw_parts_mut(self.data, self.length) };
        Ok(slice)
    }

    /// Clone the buffer data into a Rust Vec
    pub fn to_vec(&self) -> Result<Vec<u8>, BitCrapsError> {
        let slice = self.as_slice()?;
        Ok(slice.to_vec())
    }

    /// Transfer ownership to iOS (mark as non-Rust owned)
    pub fn transfer_to_ios(&mut self) {
        self.owned_by_rust = false;
        debug!(
            "Buffer ownership transferred to iOS (ptr: {:?}, len: {})",
            self.data, self.length
        );
    }

    /// Take ownership back from iOS (mark as Rust owned)
    pub fn take_from_ios(&mut self) {
        self.owned_by_rust = true;
        debug!(
            "Buffer ownership taken from iOS (ptr: {:?}, len: {})",
            self.data, self.length
        );
    }
}

impl Drop for ManagedBuffer {
    fn drop(&mut self) {
        if self.owned_by_rust && !self.data.is_null() {
            // SAFETY INVARIANTS for Box reconstruction:
            // 1. self.data must be the original pointer from Box::into_raw or similar
            // 2. self.capacity must match the original allocation size
            // 3. Memory must not have been freed elsewhere
            // 4. No other references to this memory must exist
            let boxed_slice =
                unsafe { Box::from_raw(slice::from_raw_parts_mut(self.data, self.capacity)) };
            drop(boxed_slice);

            debug!(
                "Dropped Rust-owned buffer (ptr: {:?}, len: {})",
                self.data, self.length
            );
        }
    }
}

impl ManagedString {
    /// Create a new managed string from Rust String
    pub fn new_from_rust(s: String) -> Result<Self, BitCrapsError> {
        let c_string = CString::new(s).map_err(|_| BitCrapsError::InvalidInput {
            reason: "String contains null bytes".to_string(),
        })?;

        let length = c_string.as_bytes().len();
        let ptr = c_string.into_raw(); // Transfer ownership

        Ok(Self {
            ptr,
            length,
            owned_by_rust: true,
        })
    }

    /// Create a managed string from iOS-provided C string (non-owning)
    pub fn new_from_ios(c_str_ptr: *const c_char) -> Result<Self, BitCrapsError> {
        if c_str_ptr.is_null() {
            return Err(BitCrapsError::InvalidInput {
                reason: "Null C string pointer from iOS".to_string(),
            });
        }

        let c_str = unsafe { CStr::from_ptr(c_str_ptr) };
        let length = c_str.to_bytes().len();

        Ok(Self {
            ptr: c_str_ptr as *mut c_char,
            length,
            owned_by_rust: false,
        })
    }

    /// Get the string as a Rust &str
    pub fn as_str(&self) -> Result<&str, BitCrapsError> {
        if self.ptr.is_null() {
            return Err(BitCrapsError::InvalidInput {
                reason: "Null string pointer".to_string(),
            });
        }

        let c_str = unsafe { CStr::from_ptr(self.ptr) };
        c_str.to_str().map_err(|e| BitCrapsError::InvalidInput {
            reason: format!("Invalid UTF-8 in string: {}", e),
        })
    }

    /// Clone the string data into a Rust String
    pub fn to_string(&self) -> Result<String, BitCrapsError> {
        Ok(self.as_str()?.to_string())
    }

    /// Get the C string pointer (for passing to iOS)
    pub fn as_c_ptr(&self) -> *const c_char {
        self.ptr
    }

    /// Transfer ownership to iOS
    pub fn transfer_to_ios(&mut self) {
        self.owned_by_rust = false;
        debug!(
            "String ownership transferred to iOS (ptr: {:?}, len: {})",
            self.ptr, self.length
        );
    }

    /// Take ownership back from iOS
    pub fn take_from_ios(&mut self) {
        self.owned_by_rust = true;
        debug!(
            "String ownership taken from iOS (ptr: {:?}, len: {})",
            self.ptr, self.length
        );
    }
}

impl Drop for ManagedString {
    fn drop(&mut self) {
        if self.owned_by_rust && !self.ptr.is_null() {
            // Reconstruct the CString to properly deallocate
            let _c_string = unsafe { CString::from_raw(self.ptr) };
            debug!(
                "Dropped Rust-owned string (ptr: {:?}, len: {})",
                self.ptr, self.length
            );
        }
    }
}

// MARK: - C FFI Memory Management Functions

/// Allocate a buffer on the Rust side for iOS to use
#[no_mangle]
pub extern "C" fn ios_alloc_buffer(size: usize) -> *mut ManagedBuffer {
    if size == 0 {
        error!("Cannot allocate zero-sized buffer");
        return ptr::null_mut();
    }

    let data = vec![0u8; size];
    let buffer = ManagedBuffer::new_from_rust(data);
    let boxed_buffer = Box::new(buffer);

    debug!(
        "Allocated buffer for iOS (size: {}, ptr: {:?})",
        size, boxed_buffer.data
    );
    Box::into_raw(boxed_buffer)
}

/// Free a buffer allocated by Rust
#[no_mangle]
pub extern "C" fn ios_free_buffer(buffer_ptr: *mut ManagedBuffer) {
    if buffer_ptr.is_null() {
        error!("Attempt to free null buffer pointer");
        return;
    }

    let buffer = unsafe { Box::from_raw(buffer_ptr) };
    debug!(
        "Freeing buffer (ptr: {:?}, len: {}, owned: {})",
        buffer.data, buffer.length, buffer.owned_by_rust
    );

    // Buffer will be properly dropped by Box destructor
}

/// Allocate a string on the Rust side for iOS to use
#[no_mangle]
pub extern "C" fn ios_alloc_string(rust_str: *const c_char) -> *mut ManagedString {
    if rust_str.is_null() {
        error!("Cannot allocate string from null pointer");
        return ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(rust_str) };
    let string = match c_str.to_str() {
        Ok(s) => s.to_string(),
        Err(e) => {
            error!("Invalid UTF-8 in string allocation: {}", e);
            return ptr::null_mut();
        }
    };

    let managed_string = match ManagedString::new_from_rust(string) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create managed string: {}", e);
            return ptr::null_mut();
        }
    };

    let boxed_string = Box::new(managed_string);
    debug!(
        "Allocated string for iOS (ptr: {:?}, len: {})",
        boxed_string.ptr, boxed_string.length
    );

    Box::into_raw(boxed_string)
}

/// Free a string allocated by Rust
#[no_mangle]
pub extern "C" fn ios_free_string(string_ptr: *mut ManagedString) {
    if string_ptr.is_null() {
        error!("Attempt to free null string pointer");
        return;
    }

    let string = unsafe { Box::from_raw(string_ptr) };
    debug!(
        "Freeing string (ptr: {:?}, len: {}, owned: {})",
        string.ptr, string.length, string.owned_by_rust
    );

    // String will be properly dropped by Box destructor
}

/// Copy data from a managed buffer to a new iOS-managed buffer
#[no_mangle]
pub extern "C" fn ios_copy_buffer_data(
    buffer_ptr: *const ManagedBuffer,
    out_data: *mut *mut u8,
    out_length: *mut usize,
) -> i32 {
    if buffer_ptr.is_null() || out_data.is_null() || out_length.is_null() {
        error!("Null pointers in ios_copy_buffer_data");
        return 0;
    }

    let buffer = unsafe { &*buffer_ptr };

    let data_slice = match buffer.as_slice() {
        Ok(slice) => slice,
        Err(e) => {
            error!("Failed to get buffer slice: {}", e);
            return 0;
        }
    };

    // Allocate new memory for iOS to own
    let copied_data = data_slice.to_vec().into_boxed_slice();
    let data_ptr = copied_data.as_ptr() as *mut u8;
    let data_len = copied_data.len();

    // Transfer ownership to iOS
    mem::forget(copied_data);

    unsafe {
        *out_data = data_ptr;
        *out_length = data_len;
    }

    debug!(
        "Copied buffer data for iOS (ptr: {:?}, len: {})",
        data_ptr, data_len
    );
    1
}

/// Copy string data from a managed string to a new iOS-managed C string
#[no_mangle]
pub extern "C" fn ios_copy_string_data(
    string_ptr: *const ManagedString,
    out_c_str: *mut *mut c_char,
) -> i32 {
    if string_ptr.is_null() || out_c_str.is_null() {
        error!("Null pointers in ios_copy_string_data");
        return 0;
    }

    let string = unsafe { &*string_ptr };

    let rust_str = match string.as_str() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to get string data: {}", e);
            return 0;
        }
    };

    // Create new CString for iOS to own
    let c_string = match CString::new(rust_str) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create CString: {}", e);
            return 0;
        }
    };

    let c_str_ptr = c_string.into_raw();

    unsafe {
        *out_c_str = c_str_ptr;
    }

    debug!(
        "Copied string data for iOS (ptr: {:?}, content: {:?})",
        c_str_ptr, rust_str
    );
    1
}

/// Create a callback-safe data structure for iOS events
#[repr(C)]
pub struct IosEventData {
    pub event_type: *const c_char,
    pub peer_id: *const c_char,
    pub data_ptr: *const u8,
    pub data_len: u32,
    pub timestamp: u64,
}

/// Create iOS event data structure (to be freed by ios_free_event_data)
#[no_mangle]
pub extern "C" fn ios_create_event_data(
    event_type: *const c_char,
    peer_id: *const c_char,
    data_ptr: *const u8,
    data_len: u32,
) -> *mut IosEventData {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let event_data = IosEventData {
        event_type,
        peer_id,
        data_ptr,
        data_len,
        timestamp,
    };

    let boxed_event = Box::new(event_data);
    debug!("Created iOS event data (timestamp: {})", timestamp);

    Box::into_raw(boxed_event)
}

/// Free iOS event data structure
#[no_mangle]
pub extern "C" fn ios_free_event_data(event_ptr: *mut IosEventData) {
    if event_ptr.is_null() {
        error!("Attempt to free null event data pointer");
        return;
    }

    let event_data = unsafe { Box::from_raw(event_ptr) };
    debug!("Freed iOS event data (timestamp: {})", event_data.timestamp);

    // Event data will be properly dropped by Box destructor
}

/// Validate a memory pointer and size (for debugging)
/// SECURITY NOTE: This function has been made safe by removing arbitrary memory access
#[no_mangle]
pub extern "C" fn ios_validate_memory(ptr: *const c_void, size: usize) -> i32 {
    if ptr.is_null() || size == 0 {
        warn!("Memory validation failed: ptr={:?}, size={}", ptr, size);
        return 0;
    }

    // SECURITY: Removed unsafe memory access that could crash or expose memory
    // We can only perform basic pointer validation without dereferencing

    // Check for obviously invalid pointers (basic heuristics)
    let ptr_addr = ptr as usize;

    // Check alignment for common pointer types (heuristic)
    if ptr_addr % std::mem::align_of::<*const c_void>() != 0 {
        warn!("Memory validation failed: unaligned pointer {:p}", ptr);
        return 0;
    }

    // Check for obviously invalid address ranges (platform-specific heuristics)
    #[cfg(target_pointer_width = "64")]
    {
        // On 64-bit systems, valid user space is typically limited
        if ptr_addr < 0x1000 || ptr_addr > 0x7FFF_FFFF_FFFF {
            warn!(
                "Memory validation failed: suspicious address range {:p}",
                ptr
            );
            return 0;
        }
    }

    #[cfg(target_pointer_width = "32")]
    {
        // On 32-bit systems, check basic range
        if ptr_addr < 0x1000 || ptr_addr > 0xFFFF_0000 {
            warn!(
                "Memory validation failed: suspicious address range {:p}",
                ptr
            );
            return 0;
        }
    }

    // Check for size overflow
    if ptr_addr.saturating_add(size) < ptr_addr {
        warn!("Memory validation failed: size causes address overflow");
        return 0;
    }

    debug!("Memory validation passed: ptr={:p}, size={}", ptr, size);
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_managed_buffer_creation() {
        let data = vec![1, 2, 3, 4, 5];
        let buffer = ManagedBuffer::new_from_rust(data.clone());

        assert!(buffer.owned_by_rust);
        assert_eq!(buffer.length, 5);
        assert_eq!(buffer.capacity, 5);

        let slice = buffer.as_slice().unwrap();
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_managed_string_creation() {
        let test_string = "Hello, iOS!".to_string();
        let managed_str = ManagedString::new_from_rust(test_string.clone()).unwrap();

        assert!(managed_str.owned_by_rust);
        assert_eq!(managed_str.length, test_string.len());

        let rust_str = managed_str.as_str().unwrap();
        assert_eq!(rust_str, "Hello, iOS!");
    }

    #[test]
    fn test_buffer_ownership_transfer() {
        let data = vec![1, 2, 3];
        let mut buffer = ManagedBuffer::new_from_rust(data);

        assert!(buffer.owned_by_rust);

        buffer.transfer_to_ios();
        assert!(!buffer.owned_by_rust);

        buffer.take_from_ios();
        assert!(buffer.owned_by_rust);
    }

    #[test]
    fn test_string_ownership_transfer() {
        let test_string = "Test".to_string();
        let mut managed_str = ManagedString::new_from_rust(test_string).unwrap();

        assert!(managed_str.owned_by_rust);

        managed_str.transfer_to_ios();
        assert!(!managed_str.owned_by_rust);

        managed_str.take_from_ios();
        assert!(managed_str.owned_by_rust);
    }
}
