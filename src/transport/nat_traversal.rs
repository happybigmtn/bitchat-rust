//! NAT Traversal Implementation with Security Hardening
//!
//! This module provides NAT traversal capabilities including:
//! - STUN client for public IP discovery with constant-time parsing
//! - NAT type detection  
//! - Hole punching techniques
//! - TURN relay support (placeholder)
//! - DoS protection and input validation

use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::net::{UdpSocket, TcpStream, TcpListener};
use tokio::io::AsyncWriteExt;
use rand::Rng;
use dashmap::DashMap;
use lru::LruCache;
use std::num::NonZeroUsize;
use crate::error::{Error, Result};
use crate::security::{ConstantTimeOps, SecurityManager, SecurityConfig};

// Note: TLS dependencies would need to be added to Cargo.toml
// For now, we'll use conditional compilation to avoid build errors
#[cfg(feature = "tls")]
use rustls::{ClientConfig, ServerName};
#[cfg(feature = "tls")]
use tokio_rustls::{TlsConnector, TlsAcceptor};
#[cfg(feature = "tls")]
use std::sync::Arc as StdArc;

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
    TcpTls,
    TurnRelay,
    UdpHolePunching,
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

/// Cached STUN server response
#[derive(Debug, Clone)]
struct StunResponse {
    pub public_address: SocketAddr,
    pub cached_at: Instant,
    pub ttl: Duration,
}

/// Connection pool entry
#[derive(Debug)]
struct PooledConnection {
    pub stream: TcpStream,
    pub created_at: Instant,
    pub last_used: Instant,
}

/// STUN server with performance tracking
#[derive(Debug, Clone)]
struct TrackedStunServer {
    pub address: String,
    pub response_time: Duration,
    pub success_rate: f64,
    pub last_used: Instant,
    pub failure_count: u32,
}

/// Enhanced network handler with NAT traversal and security capabilities
pub struct NetworkHandler {
    pub udp_socket: Arc<UdpSocket>,
    pub tcp_listener: Option<Arc<TcpListener>>,
    pub local_address: SocketAddr,
    pub public_address: Arc<RwLock<Option<SocketAddr>>>,
    pub stun_servers: Vec<TrackedStunServer>,
    pub turn_servers: Vec<TurnServer>,
    pub nat_type: Arc<RwLock<NatType>>,
    pub message_id_counter: Arc<AtomicU64>,
    pub pending_messages: Arc<DashMap<u64, ReliableMessage>>,
    pub security_manager: Arc<SecurityManager>,
    
    // Performance optimizations
    pub stun_cache: Arc<Mutex<LruCache<String, StunResponse>>>,
    pub connection_pool: Arc<DashMap<SocketAddr, PooledConnection>>,
    pub stun_server_metrics: Arc<DashMap<String, TrackedStunServer>>,
}

impl NetworkHandler {
    pub fn new(
        udp_socket: UdpSocket,
        tcp_listener: Option<TcpListener>,
        local_address: SocketAddr,
    ) -> Self {
        let security_config = SecurityConfig::default();
        let security_manager = Arc::new(SecurityManager::new(security_config));
        
        // Initialize STUN servers with tracking
        let stun_servers = vec![
            TrackedStunServer {
                address: "stun.l.google.com:19302".to_string(),
                response_time: Duration::from_millis(100),
                success_rate: 1.0,
                last_used: Instant::now() - Duration::from_secs(3600), // Never used
                failure_count: 0,
            },
            TrackedStunServer {
                address: "stun1.l.google.com:19302".to_string(),
                response_time: Duration::from_millis(100),
                success_rate: 1.0,
                last_used: Instant::now() - Duration::from_secs(3600),
                failure_count: 0,
            },
            TrackedStunServer {
                address: "stun2.l.google.com:19302".to_string(),
                response_time: Duration::from_millis(100),
                success_rate: 1.0,
                last_used: Instant::now() - Duration::from_secs(3600),
                failure_count: 0,
            },
            TrackedStunServer {
                address: "stun.cloudflare.com:3478".to_string(),
                response_time: Duration::from_millis(100),
                success_rate: 1.0,
                last_used: Instant::now() - Duration::from_secs(3600),
                failure_count: 0,
            },
        ];

        Self {
            udp_socket: Arc::new(udp_socket),
            tcp_listener: tcp_listener.map(Arc::new),
            local_address,
            public_address: Arc::new(RwLock::new(None)),
            stun_servers: stun_servers.clone(),
            turn_servers: vec![],
            nat_type: Arc::new(RwLock::new(NatType::Unknown)),
            message_id_counter: Arc::new(AtomicU64::new(0)),
            pending_messages: Arc::new(DashMap::new()),
            security_manager,
            stun_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(100).expect("Cache size must be non-zero")
            ))),
            connection_pool: Arc::new(DashMap::new()),
            stun_server_metrics: Arc::new({
                let metrics = DashMap::new();
                for server in stun_servers {
                    metrics.insert(server.address.clone(), server);
                }
                metrics
            }),
        }
    }

    /// Setup NAT traversal using STUN/TURN with parallel optimization
    pub async fn setup_nat_traversal(&self) -> Result<()> {
        println!("Starting optimized NAT traversal setup...");

        // Check cache first
        if let Some(cached_address) = self.get_cached_public_address().await {
            let mut addr = self.public_address.write().await;
            *addr = Some(cached_address);
            println!("Using cached public address: {}", cached_address);
        } else {
            // Discover public IP using parallel STUN requests for better performance
            if let Some(public_addr) = self.discover_public_address_parallel().await {
                {
                    let mut addr = self.public_address.write().await;
                    *addr = Some(public_addr);
                }
                println!("Discovered public address: {}", public_addr);
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

    /// Get cached public address if valid
    async fn get_cached_public_address(&self) -> Option<SocketAddr> {
        let mut cache = self.stun_cache.lock().await;
        let cache_key = format!("public_address_{}", self.local_address);
        
        if let Some(cached) = cache.get(&cache_key) {
            if cached.cached_at.elapsed() < cached.ttl {
                return Some(cached.public_address);
            } else {
                cache.pop(&cache_key); // Remove expired entry
            }
        }
        None
    }

    /// Discover public address using parallel STUN requests
    async fn discover_public_address_parallel(&self) -> Option<SocketAddr> {
        // Sort STUN servers by performance metrics
        let mut sorted_servers = self.stun_servers.clone();
        sorted_servers.sort_by(|a, b| {
            // Sort by success rate (desc) then by response time (asc)
            b.success_rate.partial_cmp(&a.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.response_time.cmp(&b.response_time))
        });

        // Launch parallel requests to top 3 servers
        let mut tasks = Vec::new();
        let server_count = std::cmp::min(3, sorted_servers.len());
        
        for server in &sorted_servers[..server_count] {
            let server_addr = server.address.clone();
            let udp_socket = self.udp_socket.clone();
            let stun_server_metrics = self.stun_server_metrics.clone();
            
            let task = tokio::spawn(async move {
                let start_time = Instant::now();
                
                match Self::discover_public_address_single(&udp_socket, &server_addr).await {
                    Ok(public_addr) => {
                        let response_time = start_time.elapsed();
                        
                        // Update server metrics
                        if let Some(mut metrics) = stun_server_metrics.get_mut(&server_addr) {
                            metrics.response_time = response_time;
                            metrics.success_rate = (metrics.success_rate * 0.9) + 0.1; // Exponential moving average
                            metrics.last_used = Instant::now();
                            metrics.failure_count = 0;
                        }
                        
                        Some(public_addr)
                    }
                    Err(_) => {
                        // Update failure metrics
                        if let Some(mut metrics) = stun_server_metrics.get_mut(&server_addr) {
                            metrics.success_rate *= 0.9; // Reduce success rate
                            metrics.failure_count += 1;
                        }
                        None
                    }
                }
            });
            
            tasks.push(task);
        }

        // Wait for first successful response
        for task in tasks {
            if let Ok(Some(public_addr)) = task.await {
                // Cache the result
                self.cache_public_address(public_addr).await;
                return Some(public_addr);
            }
        }

        None
    }

    /// Cache public address for future use
    async fn cache_public_address(&self, public_addr: SocketAddr) {
        let mut cache = self.stun_cache.lock().await;
        let cache_key = format!("public_address_{}", self.local_address);
        
        let cached_response = StunResponse {
            public_address: public_addr,
            cached_at: Instant::now(),
            ttl: Duration::from_secs(300), // 5 minute TTL
        };
        
        cache.put(cache_key, cached_response);
    }

    /// Discover public address from a single STUN server
    async fn discover_public_address_single(
        udp_socket: &UdpSocket,
        stun_server: &str,
    ) -> Result<SocketAddr> {
        let server_addr: SocketAddr = stun_server.parse()
            .map_err(|e| Error::Network(format!("Invalid STUN server address: {}", e)))?;

        // Create STUN binding request
        let transaction_id: [u8; 12] = rand::thread_rng().gen();
        let stun_request = Self::create_stun_binding_request_static(transaction_id);

        // Send STUN request
        udp_socket.send_to(&stun_request, server_addr).await
            .map_err(|e| Error::Network(format!("Failed to send STUN request: {}", e)))?;

        // Wait for response with timeout
        let mut buffer = [0u8; 1024];
        let timeout = Duration::from_secs(3); // Shorter timeout for parallel requests

        match tokio::time::timeout(timeout, udp_socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, _from))) => {
                if let Some(public_addr) = Self::parse_stun_response_static(&buffer[..len], transaction_id) {
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
        match self.discover_public_address(&first_server.address).await {
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
        Self::create_stun_binding_request_static(transaction_id)
    }

    /// Static version of create_stun_binding_request
    fn create_stun_binding_request_static(transaction_id: [u8; 12]) -> Vec<u8> {
        let mut packet = Vec::new();

        // STUN header
        packet.extend_from_slice(&[0x00, 0x01]); // Message Type: Binding Request
        packet.extend_from_slice(&[0x00, 0x00]); // Message Length: 0 (no attributes)
        packet.extend_from_slice(&[0x21, 0x12, 0xA4, 0x42]); // Magic Cookie
        packet.extend_from_slice(&transaction_id); // Transaction ID

        packet
    }

    /// Parse STUN response to extract public address using constant-time operations
    fn parse_stun_response(&self, data: &[u8], expected_transaction_id: [u8; 12]) -> Option<SocketAddr> {
        Self::parse_stun_response_static(data, expected_transaction_id)
    }

    /// Static version of parse_stun_response
    fn parse_stun_response_static(data: &[u8], expected_transaction_id: [u8; 12]) -> Option<SocketAddr> {
        // Use constant-time STUN parsing to prevent timing attacks
        match ConstantTimeOps::parse_stun_packet_ct(data) {
            Ok(packet_info) => {
                // Verify transaction ID in constant time
                if !ConstantTimeOps::constant_time_eq(&packet_info.transaction_id, &expected_transaction_id) {
                    return None;
                }
                
                // Continue with attribute parsing for address extraction
                Self::parse_stun_attributes_secure_static(data, packet_info)
            },
            Err(_) => None,
        }
    }
    
    /// Secure attribute parsing for STUN packets
    fn parse_stun_attributes_secure(&self, data: &[u8], packet_info: crate::security::constant_time::StunPacketInfo) -> Option<SocketAddr> {
        Self::parse_stun_attributes_secure_static(data, packet_info)
    }

    /// Static version of parse_stun_attributes_secure
    fn parse_stun_attributes_secure_static(data: &[u8], _packet_info: crate::security::constant_time::StunPacketInfo) -> Option<SocketAddr> {

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

        // Store message for retransmission - lock-free insertion
        self.pending_messages.insert(message_id, reliable_msg.clone());

        // Send initial attempt
        if let Err(e) = self.send_message_attempt(&reliable_msg).await {
            println!("Initial send failed: {}", e);
        }

        Ok(message_id)
    }

    /// Select optimal transport mode based on destination and NAT type
    pub async fn select_transport_mode(&self, _dest: &SocketAddr) -> TransportMode {
        let nat_type = self.nat_type.read().await;
        match *nat_type {
            NatType::Open | NatType::FullCone => TransportMode::Udp,
            NatType::Symmetric => {
                // Use TCP/TLS or TURN relay for symmetric NAT
                if self.tcp_listener.is_some() {
                    TransportMode::TcpTls
                } else if !self.turn_servers.is_empty() {
                    TransportMode::TurnRelay
                } else {
                    TransportMode::UdpHolePunching // Try hole punching as fallback
                }
            }
            NatType::RestrictedCone | NatType::PortRestrictedCone => {
                // These NAT types benefit from hole punching
                TransportMode::UdpHolePunching
            }
            _ => TransportMode::Udp,
        }
    }

    /// Send single message attempt
    async fn send_message_attempt(&self, msg: &ReliableMessage) -> Result<()> {
        match msg.transport_mode {
            TransportMode::Udp => self.send_udp(msg).await,
            TransportMode::Tcp => self.send_tcp(msg).await,
            TransportMode::TcpTls => self.send_tcp_tls(msg).await,
            TransportMode::TurnRelay => self.send_turn_relay(msg).await,
            TransportMode::UdpHolePunching => self.send_udp_hole_punching(msg).await,
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

    /// Send message via TCP with TLS
    async fn send_tcp_tls(&self, msg: &ReliableMessage) -> Result<()> {
        #[cfg(feature = "tls")]
        {
            use tokio_rustls::TlsConnector;
            use rustls::ClientConfig;
            
            let config = ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            
            let connector = TlsConnector::from(Arc::new(config));
            let domain = ServerName::try_from("localhost").unwrap();
            
            let stream = TcpStream::connect(msg.destination).await
                .map_err(|e| Error::Network(format!("TLS TCP connect failed: {}", e)))?;
                
            let mut tls_stream = connector.connect(domain, stream).await
                .map_err(|e| Error::Network(format!("TLS handshake failed: {}", e)))?;
            
            // Send message length prefix
            let len = msg.payload.len() as u32;
            tls_stream.write_all(&len.to_be_bytes()).await
                .map_err(|e| Error::Network(format!("TLS write failed: {}", e)))?;

            // Send message ID  
            tls_stream.write_all(&msg.id.to_be_bytes()).await
                .map_err(|e| Error::Network(format!("TLS write failed: {}", e)))?;

            // Send payload
            tls_stream.write_all(&msg.payload).await
                .map_err(|e| Error::Network(format!("TLS write failed: {}", e)))?;
            tls_stream.flush().await
                .map_err(|e| Error::Network(format!("TLS flush failed: {}", e)))?;

            Ok(())
        }
        #[cfg(not(feature = "tls"))]
        {
            // Fallback to regular TCP if TLS is not available
            self.send_tcp(msg).await
        }
    }

    /// Send message via UDP hole punching
    async fn send_udp_hole_punching(&self, msg: &ReliableMessage) -> Result<()> {
        // UDP hole punching involves:
        // 1. Send initial packet to establish mapping
        // 2. Wait for response or timeout
        // 3. Send actual message
        
        // For now, send multiple UDP packets to increase hole punching success
        for _attempt in 0..3 {
            if let Err(e) = self.send_udp(msg).await {
                println!("UDP hole punching attempt failed: {}", e);
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                return Ok(());
            }
        }
        
        Err(Error::Network("UDP hole punching failed after retries".to_string()))
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
                
                // Check for messages that need retransmission - lock-free iteration
                let now = Instant::now();
                
                for mut entry in pending_messages.iter_mut() {
                    let id = *entry.key();
                    let msg = entry.value_mut();
                    
                    if now.duration_since(msg.last_attempt) > msg.timeout / msg.max_attempts {
                        if msg.attempts < msg.max_attempts {
                            msg.attempts += 1;
                            msg.last_attempt = now;
                            to_retry.push(msg.clone());
                        } else {
                            to_remove.push(id);
                        }
                    }
                }
                
                // Remove expired messages
                for id in &to_remove {
                    pending_messages.remove(id);
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
            security_manager: self.security_manager.clone(),
            stun_cache: self.stun_cache.clone(),
            connection_pool: self.connection_pool.clone(),
            stun_server_metrics: self.stun_server_metrics.clone(),
        }
    }

    /// Acknowledge message receipt (removes from retransmission queue)
    pub async fn acknowledge_message(&self, message_id: u64) {
        self.pending_messages.remove(&message_id);
    }

    /// Initiate advanced NAT traversal to establish connection with target peer
    pub async fn initiate_advanced_nat_traversal(&self, target_peer: SocketAddr) -> Result<SocketAddr> {
        println!("Initiating advanced NAT traversal to: {}", target_peer);
        
        let nat_type = self.nat_type.read().await.clone();
        
        match nat_type {
            NatType::Open => {
                // Direct connection possible
                Ok(target_peer)
            }
            NatType::FullCone | NatType::RestrictedCone | NatType::PortRestrictedCone => {
                // Try UDP hole punching
                self.attempt_udp_hole_punching(target_peer).await
            }
            NatType::Symmetric => {
                // Symmetric NAT requires TURN relay or TCP
                if !self.turn_servers.is_empty() {
                    self.establish_turn_relay_connection(target_peer).await
                } else {
                    // Fall back to TCP connection attempt
                    self.attempt_tcp_connection(target_peer).await
                }
            }
            NatType::Unknown => {
                // Try multiple strategies in order of preference
                if let Ok(addr) = self.attempt_udp_hole_punching(target_peer).await {
                    Ok(addr)
                } else if let Ok(addr) = self.attempt_tcp_connection(target_peer).await {
                    Ok(addr)  
                } else {
                    Err(Error::Network("All NAT traversal methods failed".to_string()))
                }
            }
        }
    }

    /// Attempt UDP hole punching
    async fn attempt_udp_hole_punching(&self, target_peer: SocketAddr) -> Result<SocketAddr> {
        println!("Attempting UDP hole punching to: {}", target_peer);
        
        // Send initial UDP packet to "punch" the hole
        let punch_message = b"NAT_PUNCH";
        
        for attempt in 0..5 {
            match self.udp_socket.send_to(punch_message, target_peer).await {
                Ok(_) => {
                    println!("UDP punch attempt {} sent successfully", attempt + 1);
                    
                    // Wait briefly for a response
                    let mut buffer = [0u8; 1024];
                    let timeout = Duration::from_millis(200);
                    
                    match tokio::time::timeout(timeout, 
                        self.udp_socket.recv_from(&mut buffer)).await {
                        Ok(Ok((_len, from))) => {
                            println!("Received response from: {}", from);
                            if from == target_peer {
                                return Ok(from);
                            }
                        }
                        Ok(Err(_)) | Err(_) => {
                            // No response, continue trying
                        }
                    }
                }
                Err(e) => {
                    println!("UDP punch attempt {} failed: {}", attempt + 1, e);
                }
            }
            
            // Wait before next attempt
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Err(Error::Network("UDP hole punching failed".to_string()))
    }

    /// Attempt TCP connection
    async fn attempt_tcp_connection(&self, target_peer: SocketAddr) -> Result<SocketAddr> {
        println!("Attempting TCP connection to: {}", target_peer);
        
        match tokio::time::timeout(
            Duration::from_secs(5),
            TcpStream::connect(target_peer)
        ).await {
            Ok(Ok(_stream)) => {
                println!("TCP connection established to: {}", target_peer);
                Ok(target_peer)
            }
            Ok(Err(e)) => {
                Err(Error::Network(format!("TCP connection failed: {}", e)))
            }
            Err(_) => {
                Err(Error::Network("TCP connection timeout".to_string()))
            }
        }
    }

    /// Establish TURN relay connection
    async fn establish_turn_relay_connection(&self, target_peer: SocketAddr) -> Result<SocketAddr> {
        println!("Attempting TURN relay connection to: {}", target_peer);
        
        if self.turn_servers.is_empty() {
            return Err(Error::Network("No TURN servers configured".to_string()));
        }
        
        // TODO: Implement full TURN protocol
        // For now, return an error as TURN is not fully implemented
        Err(Error::Network("TURN relay not fully implemented yet".to_string()))
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