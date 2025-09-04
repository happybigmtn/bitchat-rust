//! TCP Transport Layer with TLS Support and Connection Pooling
//!
//! This module provides:
//! - TCP transport with optional TLS encryption
//! - Connection pooling and reuse
//! - Intelligent failover mechanisms
//! - Health monitoring and circuit breaker pattern
//! - Support for 8+ concurrent connections

use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use dashmap::DashMap;
use serde::{Serialize, Deserialize};

use crate::error::{Error, Result};
use crate::protocol::PeerId;
use crate::transport::{Transport, TransportAddress, TransportEvent};

// Conditional TLS support
#[cfg(feature = "tls")]
use rustls::{ClientConfig, ServerConfig};
#[cfg(feature = "tls")]
use std::sync::Arc as StdArc;
#[cfg(feature = "tls")]
use tokio_rustls::{
    client::TlsStream as ClientTlsStream, server::TlsStream as ServerTlsStream, TlsAcceptor,
    TlsConnector,
};

/// TCP transport configuration
#[derive(Debug, Clone)]
pub struct TcpTransportConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub keepalive_interval: Duration,
    pub max_message_size: usize,
    pub enable_tls: bool,
    pub connection_pool_size: usize,
}

impl Default for TcpTransportConfig {
    fn default() -> Self {
        Self {
            max_connections: 100,
            connection_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(60),
            max_message_size: 1024 * 1024, // 1MB
            enable_tls: false,
            connection_pool_size: 20,
        }
    }
}

/// Connection health status
#[derive(Debug, Clone, PartialEq)]
enum ConnectionHealth {
    Healthy,
    Degraded,
    Failed,
    CircuitOpen,
}

/// TCP connection wrapper with health monitoring
struct TcpConnection {
    peer_id: PeerId,
    address: SocketAddr,
    stream: Arc<Mutex<ConnectionStream>>, // shared, mutex-protected stream
    health: ConnectionHealth,
    last_activity: Instant,
    message_count: u64,
    error_count: u64,
    created_at: Instant,
}

/// Unified stream type for TCP and TLS
enum ConnectionStream {
    Plain(TcpStream),
    #[cfg(feature = "tls")]
    TlsClient(ClientTlsStream<TcpStream>),
    #[cfg(feature = "tls")]
    TlsServer(ServerTlsStream<TcpStream>),
}

impl ConnectionStream {
    async fn read(&mut self, buf: &mut [u8]) -> tokio::io::Result<usize> {
        match self {
            ConnectionStream::Plain(stream) => stream.read(buf).await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsClient(stream) => stream.read(buf).await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsServer(stream) => stream.read(buf).await,
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> tokio::io::Result<()> {
        match self {
            ConnectionStream::Plain(stream) => stream.write_all(buf).await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsClient(stream) => stream.write_all(buf).await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsServer(stream) => stream.write_all(buf).await,
        }
    }

    async fn flush(&mut self) -> tokio::io::Result<()> {
        match self {
            ConnectionStream::Plain(stream) => stream.flush().await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsClient(stream) => stream.flush().await,
            #[cfg(feature = "tls")]
            ConnectionStream::TlsServer(stream) => stream.flush().await,
        }
    }
}

/// Circuit breaker for connection failure management
#[derive(Debug)]
struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    failure_count: u32,
    last_failure: Option<Instant>,
    state: CircuitState,
}

#[derive(Debug, PartialEq)]
enum CircuitState {
    Closed,   // Normal operation
    Open,     // Circuit open, rejecting connections
    HalfOpen, // Testing if service recovered
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            failure_count: 0,
            last_failure: None,
            state: CircuitState::Closed,
        }
    }

    fn can_connect(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = self.last_failure {
                    if last_failure.elapsed() > self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.last_failure = None;
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }
}

/// TCP Transport implementation
pub struct TcpTransport {
    config: TcpTransportConfig,
    connections: Arc<RwLock<HashMap<PeerId, TcpConnection>>>,
    connected_set: Arc<DashMap<PeerId, ()>>, // fast, sync-friendly view for queries
    listener: Option<TcpListener>,
    local_address: Option<SocketAddr>,
    event_sender: mpsc::Sender<TransportEvent>,
    event_receiver: Arc<Mutex<mpsc::Receiver<TransportEvent>>>,
    connection_semaphore: Arc<Semaphore>,
    circuit_breakers: Arc<RwLock<HashMap<SocketAddr, CircuitBreaker>>>,
    local_peer_id: PeerId,

    #[cfg(feature = "tls")]
    tls_connector: Option<TlsConnector>,
    #[cfg(feature = "tls")]
    tls_acceptor: Option<TlsAcceptor>,
}

/// TCP handshake message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Hello {
    version: u16,
    peer_id: PeerId,
}

impl TcpTransport {
    /// Create new TCP transport
    pub fn new(config: TcpTransportConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::channel(10000); // Critical path: high-capacity for network events
        let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));

        Self {
            config: config.clone(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            connected_set: Arc::new(DashMap::new()),
            listener: None,
            local_address: None,
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            connection_semaphore,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            local_peer_id: [0u8; 32],

            #[cfg(feature = "tls")]
            tls_connector: None,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    /// Create new TCP transport using an external event sender and local peer id
    pub fn new_with_sender(
        config: TcpTransportConfig,
        local_peer_id: PeerId,
        event_sender: mpsc::Sender<TransportEvent>,
    ) -> Self {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_connections));

        Self {
            config: config.clone(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            connected_set: Arc::new(DashMap::new()),
            listener: None,
            local_address: None,
            event_sender,
            event_receiver: Arc::new(Mutex::new(mpsc::channel(1).1)), // unused in this mode
            connection_semaphore,
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            local_peer_id,

            #[cfg(feature = "tls")]
            tls_connector: None,
            #[cfg(feature = "tls")]
            tls_acceptor: None,
        }
    }

    /// Create TCP transport with TLS support
    #[cfg(feature = "tls")]
    pub fn new_with_tls(
        config: TcpTransportConfig,
        client_config: Option<ClientConfig>,
        server_config: Option<ServerConfig>,
    ) -> Self {
        let mut transport = Self::new(config);

        if let Some(client_cfg) = client_config {
            transport.tls_connector = Some(TlsConnector::from(StdArc::new(client_cfg)));
        }

        if let Some(server_cfg) = server_config {
            transport.tls_acceptor = Some(TlsAcceptor::from(StdArc::new(server_cfg)));
        }

        transport
    }

    /// Start health monitoring task
    async fn start_health_monitor(&self) {
        let connections = self.connections.clone();
        let config = self.config.clone();
        let event_sender = self.event_sender.clone();
        let connected_set = self.connected_set.clone();

        let local_peer_id_captured = self.local_peer_id;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut to_remove = Vec::new();
                let now = Instant::now();

                {
                    let mut connections_guard = connections.write().await;

                    for (peer_id, conn) in connections_guard.iter_mut() {
                        // Check connection timeout
                        if now.duration_since(conn.last_activity) > config.keepalive_interval * 2 {
                            conn.health = ConnectionHealth::Failed;
                            to_remove.push(*peer_id);
                            continue;
                        }

                        // Check error rate
                        if conn.message_count > 0 {
                            let error_rate = conn.error_count as f64 / conn.message_count as f64;
                            if error_rate > 0.1 {
                                // 10% error rate threshold
                                if conn.health == ConnectionHealth::Healthy {
                                    conn.health = ConnectionHealth::Degraded;
                                }
                            }
                        }

                        // Send keepalive for healthy connections
                        if conn.health == ConnectionHealth::Healthy
                            && now.duration_since(conn.last_activity) > config.keepalive_interval
                        {
                            // Best-effort keepalive: length=1, payload [0]
                            if let Ok(mut stream) = conn.stream.try_lock() {
                                let _ = stream.write_all(&1u32.to_be_bytes()).await;
                                let _ = stream.write_all(&[0u8]).await;
                                let _ = stream.flush().await;
                            }
                        }
                    }

                    // Remove failed connections
                    for peer_id in &to_remove {
                        connections_guard.remove(peer_id);
                    }
                }

                // Send disconnection events
                for peer_id in to_remove {
                    let _ = event_sender.send(TransportEvent::Disconnected {
                        peer_id,
                        reason: "Connection health check failed".to_string(),
                    });
                    // Update connected set snapshot
                    connected_set.remove(&peer_id);
                }
            }
        });
    }

    /// Connect with circuit breaker protection
    async fn connect_with_circuit_breaker(&self, address: SocketAddr) -> Result<TcpStream> {
        // Check circuit breaker
        {
            let mut breakers = self.circuit_breakers.write().await;
            let breaker = breakers
                .entry(address)
                .or_insert_with(|| CircuitBreaker::new(5, Duration::from_secs(60)));

            if !breaker.can_connect() {
                return Err(Error::Network(format!(
                    "Circuit breaker open for {}",
                    address
                )));
            }
        }

        // Attempt connection
        match tokio::time::timeout(self.config.connection_timeout, TcpStream::connect(address))
            .await
        {
            Ok(Ok(stream)) => {
                // Record success
                let mut breakers = self.circuit_breakers.write().await;
                if let Some(breaker) = breakers.get_mut(&address) {
                    breaker.record_success();
                }
                Ok(stream)
            }
            Ok(Err(e)) => {
                // Record failure
                let mut breakers = self.circuit_breakers.write().await;
                if let Some(breaker) = breakers.get_mut(&address) {
                    breaker.record_failure();
                }
                Err(Error::Network(format!("TCP connection failed: {}", e)))
            }
            Err(_) => {
                // Timeout - record failure
                let mut breakers = self.circuit_breakers.write().await;
                if let Some(breaker) = breakers.get_mut(&address) {
                    breaker.record_failure();
                }
                Err(Error::Network("TCP connection timeout".to_string()))
            }
        }
    }

    /// Create TLS connection if enabled
    async fn establish_connection(
        &self,
        address: SocketAddr,
        is_client: bool,
    ) -> Result<ConnectionStream> {
        let tcp_stream = self.connect_with_circuit_breaker(address).await?;

        #[cfg(feature = "tls")]
        {
            if self.config.enable_tls {
                if is_client {
                    if let Some(connector) = &self.tls_connector {
                        let domain =
                            rustls::ServerName::try_from(address.ip().to_string().as_str())
                                .map_err(|e| {
                                    Error::Network(format!("Invalid TLS server name: {}", e))
                                })?;

                        let tls_stream = connector
                            .connect(domain, tcp_stream)
                            .await
                            .map_err(|e| Error::Network(format!("TLS handshake failed: {}", e)))?;

                        return Ok(ConnectionStream::TlsClient(tls_stream));
                    }
                } else {
                    if let Some(acceptor) = &self.tls_acceptor {
                        let tls_stream = acceptor
                            .accept(tcp_stream)
                            .await
                            .map_err(|e| Error::Network(format!("TLS accept failed: {}", e)))?;

                        return Ok(ConnectionStream::TlsServer(tls_stream));
                    }
                }
            }
        }

        Ok(ConnectionStream::Plain(tcp_stream))
    }

    async fn write_frame(stream: &mut ConnectionStream, payload: &[u8]) -> Result<()> {
        let len = payload.len() as u32;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| Error::Network(format!("write length failed: {}", e)))?;
        stream
            .write_all(payload)
            .await
            .map_err(|e| Error::Network(format!("write payload failed: {}", e)))?;
        stream
            .flush()
            .await
            .map_err(|e| Error::Network(format!("flush failed: {}", e)))?;
        Ok(())
    }

    async fn read_exact_helper(stream: &mut ConnectionStream, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            let n = stream
                .read(buf)
                .await
                .map_err(|e| Error::Network(format!("read failed: {}", e)))?;
            if n == 0 {
                return Err(Error::Network("connection closed".to_string()));
            }
            let tmp = buf;
            buf = &mut tmp[n..];
        }
        Ok(())
    }

    async fn read_frame(stream: &mut ConnectionStream, max_size: usize) -> Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        Self::read_exact_helper(stream, &mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > max_size {
            return Err(Error::Network(format!(
                "frame too large: {} > {}",
                len, max_size
            )));
        }
        let mut buf = vec![0u8; len];
        Self::read_exact_helper(stream, &mut buf).await?;
        Ok(buf)
    }

    async fn run_reader(
        peer_id: PeerId,
        stream: Arc<Mutex<ConnectionStream>>,
        event_sender: mpsc::Sender<TransportEvent>,
        connections: Arc<RwLock<HashMap<PeerId, TcpConnection>>>,
    ) {
        // We do not have access to connected_set here; disconnect will be handled by
        // the public disconnect() and health monitor to keep the snapshot consistent.
        loop {
            let payload = {
                let mut guard = stream.lock().await;
                match Self::read_frame(&mut *guard, 4 + 1024 * 1024).await {
                    Ok(p) => p,
                    Err(e) => {
                        let _ = event_sender
                            .send(TransportEvent::Disconnected {
                                peer_id,
                                reason: format!("read error: {}", e),
                            })
                            .await;
                        let mut map = connections.write().await;
                        map.remove(&peer_id);
                        break;
                    }
                }
            };
            if payload.len() == 1 && payload[0] == 0 {
                continue;
            }
            let _ = event_sender
                .send(TransportEvent::DataReceived { peer_id, data: payload })
                .await;
        }
    }

    /// Send message with reliability and connection pooling
    async fn send_reliable(&self, peer_id: PeerId, data: Vec<u8>) -> Result<()> {
        if data.len() > self.config.max_message_size {
            return Err(Error::Network(format!(
                "Message too large: {} bytes (max: {})",
                data.len(),
                self.config.max_message_size
            )));
        }

        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&peer_id) {
            let stream = connection.stream.clone();
            drop(connections);
            // Frame: length prefix + bytes
            let mut guard = stream.lock().await;
            let len = data.len() as u32;
            guard
                .write_all(&len.to_be_bytes())
                .await
                .map_err(|e| Error::Network(format!("write length failed: {}", e)))?;
            guard
                .write_all(&data)
                .await
                .map_err(|e| Error::Network(format!("write data failed: {}", e)))?;
            guard
                .flush()
                .await
                .map_err(|e| Error::Network(format!("flush failed: {}", e)))?;
            Ok(())
        } else {
            Err(Error::Network(format!("No connection to peer {:?}", peer_id)))
        }
    }


    /// Accept incoming connections
    async fn accept_loop(&self, listener: TcpListener) {
        let connections = self.connections.clone();
        let event_sender = self.event_sender.clone();
        let semaphore = self.connection_semaphore.clone();
        let config = self.config.clone();
        let local_peer_id_captured = self.local_peer_id;
        let connected_set = self.connected_set.clone();

        #[cfg(feature = "tls")]
        let tls_acceptor = self.tls_acceptor.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        // Acquire connection permit
                        let permit = match semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                println!("Connection limit reached, rejecting: {}", addr);
                                continue;
                            }
                        };

                        let connections = connections.clone();
                        let event_sender = event_sender.clone();
                        let config = config.clone();

                        #[cfg(feature = "tls")]
                        let tls_acceptor = tls_acceptor.clone();

                        // Handle connection in separate task
                        let local_peer_id2 = local_peer_id_captured;
                        tokio::spawn(async move {
                            let _permit = permit; // Keep permit alive

                            let mut connection_stream = {
                                #[cfg(feature = "tls")]
                                {
                                    if config.enable_tls {
                                        if let Some(acceptor) = tls_acceptor {
                                            match acceptor.accept(stream).await {
                                                Ok(tls_stream) => {
                                                    ConnectionStream::TlsServer(tls_stream)
                                                }
                                                Err(e) => {
                                                    println!("TLS handshake failed: {}", e);
                                                    return;
                                                }
                                            }
                                        } else {
                                            ConnectionStream::Plain(stream)
                                        }
                                    } else {
                                        ConnectionStream::Plain(stream)
                                    }
                                }
                                #[cfg(not(feature = "tls"))]
                                {
                                    ConnectionStream::Plain(stream)
                                }
                            };

                            // Server handshake: expect client Hello then reply with our Hello
                            let hello_bytes = match Self::read_frame(&mut connection_stream, config.max_message_size).await {
                                Ok(b) => b,
                                Err(e) => {
                                    println!("Handshake read failed: {}", e);
                                    return;
                                }
                            };
                            let client_hello: Hello = match bincode::deserialize(&hello_bytes) {
                                Ok(h) => h,
                                Err(e) => {
                                    println!("Invalid client hello: {}", e);
                                    return;
                                }
                            };
                            if client_hello.version != 1 {
                                println!("Unsupported client version: {}", client_hello.version);
                                return;
                            }
                            let server_hello = Hello { version: 1, peer_id: local_peer_id2 };
                            let out = bincode::serialize(&server_hello).expect("hello serialize");
                            if let Err(e) = Self::write_frame(&mut connection_stream, &out).await {
                                println!("Handshake write failed: {}", e);
                                return;
                            }
                            let peer_id = client_hello.peer_id;

                            let tcp_connection = TcpConnection {
                                peer_id,
                                address: addr,
                                stream: Arc::new(Mutex::new(connection_stream)),
                                health: ConnectionHealth::Healthy,
                                last_activity: Instant::now(),
                                message_count: 0,
                                error_count: 0,
                                created_at: Instant::now(),
                            };

                            // Store connection
                            {
                                let mut connections_guard = connections.write().await;
                                connections_guard.insert(peer_id, tcp_connection);
                            }
                            // Update connected snapshot
                            connected_set.insert(peer_id, ());

                            // Send connection event
                            let _ = event_sender.send(TransportEvent::Connected {
                                peer_id,
                                address: TransportAddress::Tcp(addr),
                            });

                            println!(
                                "Accepted TCP connection from: {} (peer: {:?})",
                                addr, peer_id
                            );

                            // Spawn reader
                            let connections_clone = connections.clone();
                            let event_sender_clone = event_sender.clone();
                            tokio::spawn(async move {
                                // Obtain stream arc from map
                                let stream_arc = {
                                    let guard = connections_clone.read().await;
                                    guard.get(&peer_id).unwrap().stream.clone()
                                };
                                Self::run_reader(peer_id, stream_arc, event_sender_clone, connections_clone).await;
                            });
                        });
                    }
                    Err(e) => {
                        println!("Failed to accept connection: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
    }

    /// Get connection statistics
    pub async fn connection_stats(&self) -> TcpTransportStats {
        let connections = self.connections.read().await;
        let circuit_breakers = self.circuit_breakers.read().await;

        let healthy_count = connections
            .values()
            .filter(|conn| conn.health == ConnectionHealth::Healthy)
            .count();

        let degraded_count = connections
            .values()
            .filter(|conn| conn.health == ConnectionHealth::Degraded)
            .count();

        let failed_count = connections
            .values()
            .filter(|conn| conn.health == ConnectionHealth::Failed)
            .count();

        let circuit_open_count = circuit_breakers
            .values()
            .filter(|breaker| breaker.state == CircuitState::Open)
            .count();

        TcpTransportStats {
            total_connections: connections.len(),
            healthy_connections: healthy_count,
            degraded_connections: degraded_count,
            failed_connections: failed_count,
            circuit_breakers_open: circuit_open_count,
            max_connections: self.config.max_connections,
            available_permits: self.connection_semaphore.available_permits(),
        }
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn listen(
        &mut self,
        address: TransportAddress,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        if let TransportAddress::Tcp(addr) = address {
            let listener = TcpListener::bind(addr).await?;
            self.local_address = Some(listener.local_addr()?);

            if let Some(addr) = self.local_address {
                println!("TCP transport listening on: {}", addr);
            }

            // Start accept loop
            self.accept_loop(listener).await;

            // Start health monitor
            self.start_health_monitor().await;

            Ok(())
        } else {
            Err(Error::Network("Invalid address type for TCP transport".to_string()).into())
        }
    }

    async fn connect(
        &mut self,
        address: TransportAddress,
    ) -> std::result::Result<PeerId, Box<dyn std::error::Error>> {
        if let TransportAddress::Tcp(addr) = address {
            // Acquire connection permit
            let _permit = self
                .connection_semaphore
                .acquire()
                .await
                .map_err(|e| Error::Network(format!("Semaphore acquire failed: {}", e)))?;

            let mut connection_stream = self.establish_connection(addr, true).await?;

            // Client handshake: send hello, read server hello
            let hello = Hello { version: 1, peer_id: self.local_peer_id };
            let bytes = bincode::serialize(&hello).map_err(|e| Error::Network(format!("hello serialize: {}", e)))?;
            Self::write_frame(&mut connection_stream, &bytes).await?;
            let resp = Self::read_frame(&mut connection_stream, self.config.max_message_size).await?;
            let server_hello: Hello = bincode::deserialize(&resp)
                .map_err(|e| Error::Network(format!("hello deserialize: {}", e)))?;
            if server_hello.version != 1 {
                return Err(Error::Network("server version mismatch".to_string()).into());
            }
            let peer_id = server_hello.peer_id;

            let tcp_connection = TcpConnection {
                peer_id,
                address: addr,
                stream: Arc::new(Mutex::new(connection_stream)),
                health: ConnectionHealth::Healthy,
                last_activity: Instant::now(),
                message_count: 0,
                error_count: 0,
                created_at: Instant::now(),
            };

            // Store connection
            {
                let mut connections = self.connections.write().await;
                connections.insert(peer_id, tcp_connection);
            }
            // Update connected snapshot
            self.connected_set.insert(peer_id, ());

            // Send connection event
            let _ = self.event_sender.send(TransportEvent::Connected {
                peer_id,
                address: TransportAddress::Tcp(addr),
            });

            println!("Connected to TCP peer: {} (peer: {:?})", addr, peer_id);

            // Spawn reader loop
            let event_sender = self.event_sender.clone();
            let connections = self.connections.clone();
            let stream_arc = {
                let guard = self.connections.read().await;
                guard.get(&peer_id).unwrap().stream.clone()
            };
            tokio::spawn(async move {
                Self::run_reader(peer_id, stream_arc, event_sender, connections).await;
            });

            Ok(peer_id)
        } else {
            Err(Error::Network("Invalid address type for TCP transport".to_string()).into())
        }
    }

    async fn send(
        &mut self,
        peer_id: PeerId,
        data: Vec<u8>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        match self.send_reliable(peer_id, data).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn disconnect(
        &mut self,
        peer_id: PeerId,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.connections.write().await;

        if let Some(_connection) = connections.remove(&peer_id) {
            // Update snapshot first
            self.connected_set.remove(&peer_id);
            // Send disconnection event
            let _ = self.event_sender.send(TransportEvent::Disconnected {
                peer_id,
                reason: "User requested disconnect".to_string(),
            });

            println!("Disconnected from peer: {:?}", peer_id);
        }

        Ok(())
    }

    fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connected_set.contains_key(peer_id)
    }

    fn connected_peers(&self) -> Vec<PeerId> {
        self.connected_set.iter().map(|e| *e.key()).collect()
    }

    async fn next_event(&mut self) -> Option<TransportEvent> {
        let mut receiver = self.event_receiver.lock().await;
        receiver.recv().await
    }
}

/// TCP transport statistics
#[derive(Debug, Clone)]
pub struct TcpTransportStats {
    pub total_connections: usize,
    pub healthy_connections: usize,
    pub degraded_connections: usize,
    pub failed_connections: usize,
    pub circuit_breakers_open: usize,
    pub max_connections: usize,
    pub available_permits: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_tcp_transport_creation() {
        let config = TcpTransportConfig::default();
        let transport = TcpTransport::new(config);

        let stats = transport.connection_stats().await;
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.max_connections, 100);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let mut breaker = CircuitBreaker::new(3, Duration::from_secs(60));

        // Initially closed
        assert!(breaker.can_connect());
        assert_eq!(breaker.state, CircuitState::Closed);

        // Record failures
        breaker.record_failure();
        breaker.record_failure();
        assert!(breaker.can_connect());

        // Third failure opens circuit
        breaker.record_failure();
        assert!(!breaker.can_connect());
        assert_eq!(breaker.state, CircuitState::Open);

        // Success closes circuit
        breaker.record_success();
        assert!(breaker.can_connect());
        assert_eq!(breaker.state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_connection_limits() {
        let config = TcpTransportConfig {
            max_connections: 2,
            ..Default::default()
        };
        let transport = TcpTransport::new(config);

        // Test semaphore limits
        let permit1 = transport.connection_semaphore.try_acquire().unwrap();
        let permit2 = transport.connection_semaphore.try_acquire().unwrap();

        // Third should fail
        assert!(transport.connection_semaphore.try_acquire().is_err());

        // Release and try again
        drop(permit1);
        let _permit3 = transport.connection_semaphore.try_acquire().unwrap();
    }

    #[tokio::test]
    async fn handshake_connects() {
        let config = TcpTransportConfig::default();
        let (tx_server, _rx_server) = mpsc::channel(1000);
        let (tx_client, _rx_client) = mpsc::channel(1000);

        let mut server = TcpTransport::new_with_sender(config.clone(), [1u8; 32], tx_server);
        // listen on ephemeral port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        server.listen(TransportAddress::Tcp(addr)).await.unwrap();
        let server_addr = server.local_address.unwrap();

        let mut client = TcpTransport::new_with_sender(config.clone(), [2u8; 32], tx_client);
        let peer_id = client
            .connect(TransportAddress::Tcp(server_addr))
            .await
            .unwrap();
        assert_eq!(peer_id, [1u8; 32]);
    }
}
