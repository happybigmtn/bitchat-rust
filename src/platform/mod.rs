//! Platform-specific integrations for BitCraps
//! 
//! This module provides native bindings for mobile platforms,
//! allowing the Rust core to be embedded in Android and iOS apps.

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "ios")]  
pub mod ios;

// Re-export platform-specific functionality
#[cfg(target_os = "android")]
pub use android::*;

#[cfg(target_os = "ios")]
pub use ios::*;