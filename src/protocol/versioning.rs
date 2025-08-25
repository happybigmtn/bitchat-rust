//! Protocol versioning and compatibility layer
//!
//! This module provides version negotiation, backward compatibility,
//! and seamless upgrades for the BitCraps protocol.

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Protocol version identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl ProtocolVersion {
    /// Current protocol version
    pub const CURRENT: Self = Self {
        major: 1,
        minor: 0,
        patch: 0,
    };
    
    /// Minimum supported version
    pub const MIN_SUPPORTED: Self = Self {
        major: 1,
        minor: 0,
        patch: 0,
    };
    
    /// Create new version
    pub fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }
    
    /// Check if this version is compatible with another
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        // Major version must match
        if self.major != other.major {
            return false;
        }
        
        // Minor version can differ within same major
        // Newer minor versions are forward compatible
        // Older minor versions may not support new features
        self >= &Self::MIN_SUPPORTED && other >= &Self::MIN_SUPPORTED
    }
    
    /// Check if this version supports a specific feature
    pub fn supports_feature(&self, feature: ProtocolFeature) -> bool {
        feature.min_version() <= *self
    }
    
    /// Get version as 3-byte array for serialization
    pub fn as_bytes(&self) -> [u8; 3] {
        [self.major, self.minor, self.patch]
    }
    
    /// Create from 3-byte array
    pub fn from_bytes(bytes: [u8; 3]) -> Self {
        Self {
            major: bytes[0],
            minor: bytes[1],
            patch: bytes[2],
        }
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Protocol features with version requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtocolFeature {
    BasicMesh,           // v1.0.0+
    GatewayNodes,        // v1.1.0+
    EnhancedRouting,     // v1.2.0+
    CompressionV2,       // v1.3.0+
    ProofOfRelay,        // v1.4.0+
    CrossChainBridge,    // v2.0.0+
}

impl ProtocolFeature {
    /// Get minimum version that supports this feature
    pub fn min_version(&self) -> ProtocolVersion {
        match self {
            Self::BasicMesh => ProtocolVersion::new(1, 0, 0),
            Self::GatewayNodes => ProtocolVersion::new(1, 1, 0),
            Self::EnhancedRouting => ProtocolVersion::new(1, 2, 0),
            Self::CompressionV2 => ProtocolVersion::new(1, 3, 0),
            Self::ProofOfRelay => ProtocolVersion::new(1, 4, 0),
            Self::CrossChainBridge => ProtocolVersion::new(2, 0, 0),
        }
    }
}

/// Version negotiation result
#[derive(Debug, Clone)]
pub struct VersionNegotiation {
    pub local_version: ProtocolVersion,
    pub remote_version: ProtocolVersion,
    pub negotiated_version: ProtocolVersion,
    pub supported_features: Vec<ProtocolFeature>,
    pub compatibility_mode: CompatibilityMode,
}

/// Compatibility modes for different version mismatches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityMode {
    /// Full compatibility - all features available
    Full,
    /// Limited compatibility - some features disabled
    Limited,
    /// Legacy mode - minimal feature set
    Legacy,
    /// Incompatible versions
    Incompatible,
}

/// Protocol compatibility manager
pub struct ProtocolCompatibility {
    version_handlers: HashMap<ProtocolVersion, Box<dyn VersionHandler>>,
    feature_adapters: HashMap<ProtocolFeature, Box<dyn FeatureAdapter>>,
}

/// Handler for specific protocol versions
pub trait VersionHandler: Send + Sync {
    fn can_handle(&self, version: ProtocolVersion) -> bool;
    fn adapt_message(&self, message: &[u8], target_version: ProtocolVersion) -> Result<Vec<u8>>;
    fn supported_features(&self) -> Vec<ProtocolFeature>;
}

/// Adapter for protocol features
pub trait FeatureAdapter: Send + Sync {
    fn feature(&self) -> ProtocolFeature;
    fn encode_with_fallback(&self, data: &[u8], target_version: ProtocolVersion) -> Result<Vec<u8>>;
    fn decode_with_fallback(&self, data: &[u8], source_version: ProtocolVersion) -> Result<Vec<u8>>;
}

impl Default for ProtocolCompatibility {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolCompatibility {
    /// Create new compatibility manager
    pub fn new() -> Self {
        let mut manager = Self {
            version_handlers: HashMap::new(),
            feature_adapters: HashMap::new(),
        };
        
        // Register default handlers
        manager.register_default_handlers();
        manager
    }
    
    /// Register version handler
    pub fn register_version_handler(&mut self, version: ProtocolVersion, handler: Box<dyn VersionHandler>) {
        self.version_handlers.insert(version, handler);
    }
    
    /// Register feature adapter
    pub fn register_feature_adapter(&mut self, adapter: Box<dyn FeatureAdapter>) {
        let feature = adapter.feature();
        self.feature_adapters.insert(feature, adapter);
    }
    
    /// Negotiate protocol version with remote peer
    pub fn negotiate_version(&self, local: ProtocolVersion, remote: ProtocolVersion) -> VersionNegotiation {
        // Find the highest common version
        let negotiated = if local.is_compatible_with(&remote) {
            std::cmp::min(local, remote)
        } else {
            // Try to find compatible version
            if local.major == remote.major {
                ProtocolVersion::new(local.major, std::cmp::min(local.minor, remote.minor), 0)
            } else {
                ProtocolVersion::MIN_SUPPORTED
            }
        };
        
        // Determine supported features
        let supported_features = self.get_supported_features(&negotiated);
        
        // Determine compatibility mode
        let compatibility_mode = if negotiated == std::cmp::max(local, remote) {
            CompatibilityMode::Full
        } else if negotiated >= ProtocolVersion::new(1, 0, 0) {
            if local.is_compatible_with(&remote) {
                CompatibilityMode::Limited
            } else {
                CompatibilityMode::Legacy
            }
        } else {
            CompatibilityMode::Incompatible
        };
        
        VersionNegotiation {
            local_version: local,
            remote_version: remote,
            negotiated_version: negotiated,
            supported_features,
            compatibility_mode,
        }
    }
    
    /// Get supported features for a version
    pub fn get_supported_features(&self, version: &ProtocolVersion) -> Vec<ProtocolFeature> {
        [
            ProtocolFeature::BasicMesh,
            ProtocolFeature::GatewayNodes,
            ProtocolFeature::EnhancedRouting,
            ProtocolFeature::CompressionV2,
            ProtocolFeature::ProofOfRelay,
            ProtocolFeature::CrossChainBridge,
        ]
        .into_iter()
        .filter(|feature| version.supports_feature(*feature))
        .collect()
    }
    
    /// Adapt message for target version
    pub fn adapt_message(&self, message: &[u8], source_version: ProtocolVersion, target_version: ProtocolVersion) -> Result<Vec<u8>> {
        if source_version == target_version {
            return Ok(message.to_vec());
        }
        
        // Find appropriate handler
        if let Some(handler) = self.version_handlers.get(&source_version) {
            handler.adapt_message(message, target_version)
        } else if let Some(handler) = self.version_handlers.get(&target_version) {
            // Try reverse adaptation
            handler.adapt_message(message, source_version)
        } else {
            // No specific handler, try generic adaptation
            self.generic_adapt_message(message, source_version, target_version)
        }
    }
    
    /// Generic message adaptation
    fn generic_adapt_message(&self, message: &[u8], _source_version: ProtocolVersion, _target_version: ProtocolVersion) -> Result<Vec<u8>> {
        // For now, just pass through
        // In a real implementation, this would handle field mapping,
        // default values for new fields, etc.
        Ok(message.to_vec())
    }
    
    /// Register default version handlers
    fn register_default_handlers(&mut self) {
        // V1.0 handler
        self.register_version_handler(
            ProtocolVersion::new(1, 0, 0),
            Box::new(V1Handler::new())
        );
    }
}

/// Version handler for Protocol v1.x
struct V1Handler {
    version: ProtocolVersion,
}

impl V1Handler {
    fn new() -> Self {
        Self {
            version: ProtocolVersion::new(1, 0, 0),
        }
    }
}

impl VersionHandler for V1Handler {
    fn can_handle(&self, version: ProtocolVersion) -> bool {
        version.major == 1
    }
    
    fn adapt_message(&self, message: &[u8], target_version: ProtocolVersion) -> Result<Vec<u8>> {
        if self.can_handle(target_version) {
            // Same major version, minimal adaptation needed
            Ok(message.to_vec())
        } else {
            Err(Error::Protocol(format!(
                "Cannot adapt v{} message to v{}",
                self.version, target_version
            )))
        }
    }
    
    fn supported_features(&self) -> Vec<ProtocolFeature> {
        vec![
            ProtocolFeature::BasicMesh,
            ProtocolFeature::GatewayNodes,
            ProtocolFeature::EnhancedRouting,
        ]
    }
}

/// Protocol message with version header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedMessage {
    pub version: ProtocolVersion,
    pub message_type: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl VersionedMessage {
    /// Create new versioned message
    pub fn new(message_type: u8, payload: Vec<u8>) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            message_type,
            flags: 0,
            payload,
        }
    }
    
    /// Serialize message with version header
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        
        // Version (3 bytes)
        buffer.extend_from_slice(&self.version.as_bytes());
        
        // Message type (1 byte)
        buffer.push(self.message_type);
        
        // Flags (1 byte)
        buffer.push(self.flags);
        
        // Payload length (4 bytes, big endian)
        buffer.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        
        // Payload
        buffer.extend_from_slice(&self.payload);
        
        Ok(buffer)
    }
    
    /// Deserialize message with version header
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < 9 {  // 3 + 1 + 1 + 4 = 9 bytes minimum
            return Err(Error::InvalidData("Message too short for version header".to_string()));
        }
        
        // Extract version
        let version = ProtocolVersion::from_bytes([data[0], data[1], data[2]]);
        
        // Extract message type and flags
        let message_type = data[3];
        let flags = data[4];
        
        // Extract payload length
        let payload_len = u32::from_be_bytes([data[5], data[6], data[7], data[8]]) as usize;
        
        // Check bounds
        if data.len() < 9 + payload_len {
            return Err(Error::InvalidData("Message shorter than declared payload length".to_string()));
        }
        
        // Extract payload
        let payload = data[9..9 + payload_len].to_vec();
        
        Ok(Self {
            version,
            message_type,
            flags,
            payload,
        })
    }
    
    /// Check if this message is compatible with local version
    pub fn is_compatible(&self) -> bool {
        self.version.is_compatible_with(&ProtocolVersion::CURRENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_compatibility() {
        let v1_0_0 = ProtocolVersion::new(1, 0, 0);
        let v1_1_0 = ProtocolVersion::new(1, 1, 0);
        let v1_2_0 = ProtocolVersion::new(1, 2, 0);
        let v2_0_0 = ProtocolVersion::new(2, 0, 0);
        
        // Same major version should be compatible
        assert!(v1_0_0.is_compatible_with(&v1_1_0));
        assert!(v1_1_0.is_compatible_with(&v1_2_0));
        
        // Different major version should not be compatible
        assert!(!v1_0_0.is_compatible_with(&v2_0_0));
        assert!(!v2_0_0.is_compatible_with(&v1_0_0));
    }
    
    #[test]
    fn test_feature_support() {
        let v1_0_0 = ProtocolVersion::new(1, 0, 0);
        let v1_2_0 = ProtocolVersion::new(1, 2, 0);
        
        assert!(v1_0_0.supports_feature(ProtocolFeature::BasicMesh));
        assert!(!v1_0_0.supports_feature(ProtocolFeature::EnhancedRouting));
        
        assert!(v1_2_0.supports_feature(ProtocolFeature::BasicMesh));
        assert!(v1_2_0.supports_feature(ProtocolFeature::EnhancedRouting));
    }
    
    #[test]
    fn test_version_negotiation() {
        let compatibility = ProtocolCompatibility::new();
        
        let v1_0 = ProtocolVersion::new(1, 0, 0);
        let v1_1 = ProtocolVersion::new(1, 1, 0);
        
        let negotiation = compatibility.negotiate_version(v1_1, v1_0);
        
        assert_eq!(negotiation.negotiated_version, v1_0);
        assert_eq!(negotiation.compatibility_mode, CompatibilityMode::Limited);
    }
    
    #[test]
    fn test_versioned_message_serialization() {
        let message = VersionedMessage::new(42, vec![1, 2, 3, 4, 5]);
        let serialized = message.serialize().expect("Serialization should succeed");
        
        let deserialized = VersionedMessage::deserialize(&serialized)
            .expect("Deserialization should succeed");
        
        assert_eq!(message.version, deserialized.version);
        assert_eq!(message.message_type, deserialized.message_type);
        assert_eq!(message.payload, deserialized.payload);
    }
}