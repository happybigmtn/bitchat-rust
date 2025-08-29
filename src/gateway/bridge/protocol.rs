//! Bridge Protocol Definitions
//! 
//! Defines the protocol structures and types for bridging between
//! BLE mesh networks and internet protocols.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Supported protocol types for bridging
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProtocolType {
    /// Bluetooth Low Energy mesh protocol
    BleMesh,
    /// Transmission Control Protocol
    Tcp,
    /// User Datagram Protocol
    Udp,
    /// WebSocket protocol
    WebSocket,
    /// QUIC protocol
    Quic,
    /// HTTP/HTTPS protocol
    Http,
}

/// Bridge protocol implementation
#[derive(Debug, Clone)]
pub struct BridgeProtocol {
    /// Protocol type
    pub protocol_type: ProtocolType,
    /// Protocol-specific configuration
    pub config: ProtocolConfig,
    /// Maximum transmission unit
    pub mtu: usize,
    /// Protocol capabilities
    pub capabilities: ProtocolCapabilities,
}

/// Protocol configuration for different transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolConfig {
    BleMesh {
        service_uuid: String,
        characteristic_uuid: String,
        max_packet_size: usize,
        connection_interval: u16,
    },
    Tcp {
        keep_alive: bool,
        no_delay: bool,
        read_timeout: Option<std::time::Duration>,
        write_timeout: Option<std::time::Duration>,
    },
    Udp {
        broadcast: bool,
        multicast_addr: Option<std::net::IpAddr>,
        buffer_size: usize,
    },
    WebSocket {
        subprotocol: Option<String>,
        max_frame_size: usize,
        ping_interval: std::time::Duration,
    },
    Quic {
        max_streams: u32,
        keep_alive_interval: std::time::Duration,
        idle_timeout: std::time::Duration,
    },
    Http {
        version: HttpVersion,
        keep_alive: bool,
        timeout: std::time::Duration,
    },
}

/// HTTP version support
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpVersion {
    Http1_1,
    Http2,
    Http3,
}

/// Protocol capabilities
#[derive(Debug, Clone)]
pub struct ProtocolCapabilities {
    /// Supports reliable delivery
    pub reliable: bool,
    /// Supports ordered delivery
    pub ordered: bool,
    /// Supports flow control
    pub flow_control: bool,
    /// Supports multiplexing
    pub multiplexing: bool,
    /// Supports bidirectional communication
    pub bidirectional: bool,
    /// Maximum message size
    pub max_message_size: usize,
}

/// Bridge message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMessage {
    /// Message header
    pub header: BridgeHeader,
    /// Message payload
    pub payload: Vec<u8>,
    /// Message metadata
    pub metadata: MessageMetadata,
}

/// Bridge message header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeHeader {
    /// Message ID for deduplication
    pub message_id: u64,
    /// Source protocol type
    pub source_protocol: ProtocolType,
    /// Destination protocol type
    pub destination_protocol: ProtocolType,
    /// Message type
    pub message_type: MessageType,
    /// Message flags
    pub flags: MessageFlags,
    /// Payload length
    pub payload_length: u32,
    /// Checksum for integrity
    pub checksum: u32,
}

/// Message types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    /// Regular data message
    Data,
    /// Control message
    Control,
    /// Heartbeat/keepalive message
    Heartbeat,
    /// Error message
    Error,
    /// Protocol negotiation message
    Negotiation,
    /// Authentication message
    Authentication,
}

/// Message flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFlags {
    /// Message requires reliable delivery
    pub reliable: bool,
    /// Message requires ordered delivery
    pub ordered: bool,
    /// Message is compressed
    pub compressed: bool,
    /// Message is encrypted
    pub encrypted: bool,
    /// Message is fragmented
    pub fragmented: bool,
    /// Message is a fragment continuation
    pub more_fragments: bool,
}

impl Default for MessageFlags {
    fn default() -> Self {
        Self {
            reliable: false,
            ordered: false,
            compressed: false,
            encrypted: false,
            fragmented: false,
            more_fragments: false,
        }
    }
}

/// Message metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Timestamp when message was created
    pub timestamp: u64,
    /// Time-to-live in seconds
    pub ttl: u32,
    /// Priority level (0 = highest, 255 = lowest)
    pub priority: u8,
    /// Quality of service requirements
    pub qos: QualityOfService,
    /// Routing hints
    pub routing_hints: Vec<String>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

/// Quality of Service requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityOfService {
    /// Maximum acceptable latency
    pub max_latency: Option<std::time::Duration>,
    /// Minimum required bandwidth
    pub min_bandwidth: Option<f64>,
    /// Maximum acceptable packet loss
    pub max_packet_loss: Option<f64>,
    /// Delivery guarantee level
    pub delivery_guarantee: DeliveryGuarantee,
}

/// Delivery guarantee levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// Best effort delivery (UDP-like)
    BestEffort,
    /// At least once delivery
    AtLeastOnce,
    /// Exactly once delivery
    ExactlyOnce,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ttl: 300, // 5 minutes
            priority: 128, // Medium priority
            qos: QualityOfService {
                max_latency: Some(std::time::Duration::from_secs(30)),
                min_bandwidth: None,
                max_packet_loss: Some(0.01), // 1%
                delivery_guarantee: DeliveryGuarantee::BestEffort,
            },
            routing_hints: Vec::new(),
            attributes: HashMap::new(),
        }
    }
}

/// Protocol errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Unsupported protocol: {0:?}")]
    UnsupportedProtocol(ProtocolType),
    #[error("Protocol mismatch: expected {expected:?}, got {actual:?}")]
    ProtocolMismatch { expected: ProtocolType, actual: ProtocolType },
    #[error("Message too large: {size} bytes exceeds limit of {limit} bytes")]
    MessageTooLarge { size: usize, limit: usize },
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u32, actual: u32 },
    #[error("Fragmentation error: {0}")]
    FragmentationError(String),
    #[error("Protocol negotiation failed: {0}")]
    NegotiationFailed(String),
}

impl BridgeProtocol {
    /// Create new bridge protocol
    pub fn new(protocol_type: ProtocolType, config: ProtocolConfig) -> Self {
        let capabilities = Self::get_capabilities(protocol_type);
        let mtu = Self::get_mtu(protocol_type);
        
        Self {
            protocol_type,
            config,
            mtu,
            capabilities,
        }
    }
    
    /// Get protocol capabilities
    fn get_capabilities(protocol_type: ProtocolType) -> ProtocolCapabilities {
        match protocol_type {
            ProtocolType::BleMesh => ProtocolCapabilities {
                reliable: false,
                ordered: false,
                flow_control: false,
                multiplexing: false,
                bidirectional: true,
                max_message_size: 512, // BLE characteristic limit
            },
            ProtocolType::Tcp => ProtocolCapabilities {
                reliable: true,
                ordered: true,
                flow_control: true,
                multiplexing: false,
                bidirectional: true,
                max_message_size: 65536, // 64KB
            },
            ProtocolType::Udp => ProtocolCapabilities {
                reliable: false,
                ordered: false,
                flow_control: false,
                multiplexing: false,
                bidirectional: true,
                max_message_size: 1472, // Typical UDP payload size
            },
            ProtocolType::WebSocket => ProtocolCapabilities {
                reliable: true,
                ordered: true,
                flow_control: true,
                multiplexing: false,
                bidirectional: true,
                max_message_size: 1024 * 1024, // 1MB
            },
            ProtocolType::Quic => ProtocolCapabilities {
                reliable: true,
                ordered: false, // Per-stream ordering
                flow_control: true,
                multiplexing: true,
                bidirectional: true,
                max_message_size: 1024 * 1024, // 1MB
            },
            ProtocolType::Http => ProtocolCapabilities {
                reliable: true,
                ordered: true,
                flow_control: true,
                multiplexing: true, // HTTP/2+
                bidirectional: false, // Request-response pattern
                max_message_size: 10 * 1024 * 1024, // 10MB
            },
        }
    }
    
    /// Get MTU for protocol
    fn get_mtu(protocol_type: ProtocolType) -> usize {
        match protocol_type {
            ProtocolType::BleMesh => 244, // BLE 4.2 default
            ProtocolType::Tcp => 1460, // Ethernet MTU - headers
            ProtocolType::Udp => 1472, // Ethernet MTU - UDP header
            ProtocolType::WebSocket => 1460, // TCP-based
            ProtocolType::Quic => 1472, // UDP-based
            ProtocolType::Http => 1460, // TCP-based
        }
    }
    
    /// Check if protocols are compatible for bridging
    pub fn is_compatible(&self, other: &BridgeProtocol) -> bool {
        // Define compatibility matrix
        match (self.protocol_type, other.protocol_type) {
            // BLE mesh can bridge to any internet protocol
            (ProtocolType::BleMesh, _) | (_, ProtocolType::BleMesh) => true,
            
            // TCP can bridge to TCP, WebSocket, HTTP
            (ProtocolType::Tcp, ProtocolType::Tcp) |
            (ProtocolType::Tcp, ProtocolType::WebSocket) |
            (ProtocolType::Tcp, ProtocolType::Http) => true,
            
            // UDP can bridge to UDP, QUIC
            (ProtocolType::Udp, ProtocolType::Udp) |
            (ProtocolType::Udp, ProtocolType::Quic) => true,
            
            // WebSocket can bridge to TCP, HTTP
            (ProtocolType::WebSocket, ProtocolType::Tcp) |
            (ProtocolType::WebSocket, ProtocolType::Http) => true,
            
            // QUIC can bridge to UDP
            (ProtocolType::Quic, ProtocolType::Udp) => true,
            
            // HTTP can bridge to TCP, WebSocket
            (ProtocolType::Http, ProtocolType::Tcp) |
            (ProtocolType::Http, ProtocolType::WebSocket) => true,
            
            _ => false,
        }
    }
    
    /// Get required transformations for bridging
    pub fn get_transformations(&self, target: &BridgeProtocol) -> Vec<Transformation> {
        let mut transformations = Vec::new();
        
        // Reliability transformation
        if self.capabilities.reliable != target.capabilities.reliable {
            if target.capabilities.reliable {
                transformations.push(Transformation::AddReliability);
            } else {
                transformations.push(Transformation::RemoveReliability);
            }
        }
        
        // Ordering transformation
        if self.capabilities.ordered != target.capabilities.ordered {
            if target.capabilities.ordered {
                transformations.push(Transformation::AddOrdering);
            } else {
                transformations.push(Transformation::RemoveOrdering);
            }
        }
        
        // MTU transformation
        if self.mtu != target.mtu {
            if self.mtu > target.mtu {
                transformations.push(Transformation::Fragment);
            } else {
                transformations.push(Transformation::Defragment);
            }
        }
        
        // Flow control transformation
        if self.capabilities.flow_control != target.capabilities.flow_control {
            if target.capabilities.flow_control {
                transformations.push(Transformation::AddFlowControl);
            } else {
                transformations.push(Transformation::RemoveFlowControl);
            }
        }
        
        transformations
    }
}

/// Protocol transformations required for bridging
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transformation {
    /// Add reliability guarantees
    AddReliability,
    /// Remove reliability guarantees
    RemoveReliability,
    /// Add message ordering
    AddOrdering,
    /// Remove message ordering
    RemoveOrdering,
    /// Fragment large messages
    Fragment,
    /// Defragment message pieces
    Defragment,
    /// Add flow control
    AddFlowControl,
    /// Remove flow control
    RemoveFlowControl,
    /// Compress payload
    Compress,
    /// Decompress payload
    Decompress,
    /// Encrypt payload
    Encrypt,
    /// Decrypt payload
    Decrypt,
}

impl BridgeMessage {
    /// Create new bridge message
    pub fn new(
        source_protocol: ProtocolType,
        destination_protocol: ProtocolType,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Self {
        let header = BridgeHeader {
            message_id: Self::generate_message_id(),
            source_protocol,
            destination_protocol,
            message_type,
            flags: MessageFlags::default(),
            payload_length: payload.len() as u32,
            checksum: Self::calculate_checksum(&payload),
        };
        
        Self {
            header,
            payload,
            metadata: MessageMetadata::default(),
        }
    }
    
    /// Generate unique message ID
    fn generate_message_id() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
    
    /// Calculate payload checksum
    fn calculate_checksum(payload: &[u8]) -> u32 {
        // Simple CRC-32 implementation
        crc32fast::hash(payload)
    }
    
    /// Verify message integrity
    pub fn verify_checksum(&self) -> bool {
        self.header.checksum == Self::calculate_checksum(&self.payload)
    }
    
    /// Check if message has expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        now > self.metadata.timestamp + self.metadata.ttl as u64
    }
    
    /// Serialize message for transmission
    pub fn serialize(&self) -> Result<Vec<u8>, ProtocolError> {
        bincode::serialize(self)
            .map_err(|e| ProtocolError::InvalidFormat(e.to_string()))
    }
    
    /// Deserialize message from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, ProtocolError> {
        bincode::deserialize(data)
            .map_err(|e| ProtocolError::InvalidFormat(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_protocol_capabilities() {
        let tcp = BridgeProtocol::new(
            ProtocolType::Tcp,
            ProtocolConfig::Tcp {
                keep_alive: true,
                no_delay: true,
                read_timeout: None,
                write_timeout: None,
            },
        );
        
        assert!(tcp.capabilities.reliable);
        assert!(tcp.capabilities.ordered);
        assert!(tcp.capabilities.flow_control);
        assert!(!tcp.capabilities.multiplexing);
    }
    
    #[test]
    fn test_protocol_compatibility() {
        let ble = BridgeProtocol::new(
            ProtocolType::BleMesh,
            ProtocolConfig::BleMesh {
                service_uuid: "test".to_string(),
                characteristic_uuid: "test".to_string(),
                max_packet_size: 512,
                connection_interval: 100,
            },
        );
        
        let tcp = BridgeProtocol::new(
            ProtocolType::Tcp,
            ProtocolConfig::Tcp {
                keep_alive: true,
                no_delay: true,
                read_timeout: None,
                write_timeout: None,
            },
        );
        
        assert!(ble.is_compatible(&tcp));
        assert!(tcp.is_compatible(&ble));
    }
    
    #[test]
    fn test_bridge_message() {
        let message = BridgeMessage::new(
            ProtocolType::BleMesh,
            ProtocolType::Tcp,
            MessageType::Data,
            b"Hello, World!".to_vec(),
        );
        
        assert!(message.verify_checksum());
        assert!(!message.is_expired());
        
        let serialized = message.serialize().unwrap();
        let deserialized = BridgeMessage::deserialize(&serialized).unwrap();
        
        assert_eq!(message.header.message_id, deserialized.header.message_id);
        assert_eq!(message.payload, deserialized.payload);
    }
}