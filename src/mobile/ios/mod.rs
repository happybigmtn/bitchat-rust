//! iOS-specific mobile implementation
//!
//! This module provides the iOS-specific implementation for CoreBluetooth
//! integration, state management, and FFI bridges to Swift/Objective-C.

pub mod ble_peripheral;
pub mod ffi;
pub mod memory_bridge;
pub mod state_manager;

// Re-export the main types
pub use ble_peripheral::*;
pub use ffi::*;
pub use memory_bridge::*;
pub use state_manager::*;
