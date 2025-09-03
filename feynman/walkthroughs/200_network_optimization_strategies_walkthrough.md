# Chapter 89: Network Optimization Strategies - Making Every Packet Count

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: The Network Performance Journey

In 1973, Bob Metcalfe was trying to connect computers at Xerox PARC. His network kept failing - collisions everywhere, packets lost, chaos. So he invented Ethernet and a simple principle: when collision is detected, back off exponentially. This elegant solution to network contention became the foundation of modern networking.

Today, BitCraps faces similar challenges but at a different scale. We're not just connecting computers in a building; we're connecting mobile phones across continents, through firewalls, over cellular networks that drop to 2G in tunnels, and WiFi that competes with microwave ovens. Network optimization isn't optional - it's what makes the difference between a game that feels responsive and one that's unplayable.

This chapter dives deep into the art and science of making networks fast, reliable, and efficient. We'll explore everything from TCP tuning to custom protocols, from congestion control to traffic shaping. By the end, you'll understand how BitCraps squeezes every bit of performance from any network condition.

## The Network Stack: Understanding the Layers

Before we optimize, let's understand what we're optimizing. Networks are like parfaits - they have layers:

### Physical Layer: The Reality of Physics
Signals traveling through copper, fiber, or air. Limited by the speed of light (roughly 200,000 km/s in fiber). This gives us our theoretical minimum latency:
- Same city: ~1ms
- Cross-country (US): ~20ms  
- Transatlantic: ~60ms
- Around the world: ~200ms

### Data Link Layer: Local Delivery
Ethernet, WiFi, Bluetooth. This is where packets actually move between directly connected devices.

### Network Layer: Global Routing
IP addressing and routing. How packets find their way across the internet.

### Transport Layer: Reliable Delivery
TCP for reliability, UDP for speed. This is where most of our optimization happens.

### Application Layer: Your Code
Where BitCraps lives. We can be smart here to work around lower layer limitations.

## TCP Optimization: Tuning the Workhorse

TCP is like a careful mail carrier - it ensures every packet arrives, in order, without errors. But this reliability comes at a cost. Let's optimize it:

```rust
use std::net::{TcpStream, TcpListener};
use std::os::unix::io::AsRawFd;
use libc::{c_void, setsockopt, SOL_SOCKET, SO_KEEPALIVE, TCP_NODELAY, 
          TCP_KEEPIDLE, TCP_KEEPINTVL, TCP_KEEPCNT, SOL_TCP};

pub struct OptimizedTcpSocket {
    stream: TcpStream,
    config: TcpConfig,
}

#[derive(Clone)]
pub struct TcpConfig {
    // Disable Nagle's algorithm for low latency
    pub tcp_nodelay: bool,
    
    // Keep-alive settings
    pub keepalive_time: u32,    // Seconds before first probe
    pub keepalive_interval: u32, // Seconds between probes
    pub keepalive_probes: u32,   // Number of failed probes before disconnect
    
    // Buffer sizes
    pub send_buffer_size: usize,
    pub recv_buffer_size: usize,
    
    // TCP congestion control algorithm
    pub congestion_algorithm: CongestionAlgorithm,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            tcp_nodelay: true,        // Low latency for gaming
            keepalive_time: 30,       // Start probing after 30s
            keepalive_interval: 10,   // Probe every 10s
            keepalive_probes: 3,      // Fail after 3 probes
            send_buffer_size: 65536,  // 64KB send buffer
            recv_buffer_size: 65536,  // 64KB receive buffer
            congestion_algorithm: CongestionAlgorithm::Bbr,
        }
    }
}

#[derive(Clone)]
pub enum CongestionAlgorithm {
    Cubic,  // Default on Linux, good for high-bandwidth
    Bbr,    // Google's BBR, good for variable networks
    Vegas,  // Delay-based, good for low latency
}

impl OptimizedTcpSocket {
    pub fn new(stream: TcpStream, config: TcpConfig) -> Result<Self, NetworkError> {
        let socket = Self { stream, config };
        socket.apply_optimizations()?;
        Ok(socket)
    }
    
    fn apply_optimizations(&self) -> Result<(), NetworkError> {
        let fd = self.stream.as_raw_fd();
        
        // Disable Nagle's algorithm for low latency
        if self.config.tcp_nodelay {
            let nodelay = 1i32;
            unsafe {
                setsockopt(
                    fd,
                    SOL_TCP,
                    TCP_NODELAY,
                    &nodelay as *const _ as *const c_void,
                    std::mem::size_of::<i32>() as u32,
                );
            }
        }
        
        // Configure keep-alive
        let keepalive = 1i32;
        unsafe {
            setsockopt(
                fd,
                SOL_SOCKET,
                SO_KEEPALIVE,
                &keepalive as *const _ as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
            
            // Keep-alive time
            setsockopt(
                fd,
                SOL_TCP,
                TCP_KEEPIDLE,
                &self.config.keepalive_time as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
            
            // Keep-alive interval
            setsockopt(
                fd,
                SOL_TCP,
                TCP_KEEPINTVL,
                &self.config.keepalive_interval as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
            
            // Keep-alive probes
            setsockopt(
                fd,
                SOL_TCP,
                TCP_KEEPCNT,
                &self.config.keepalive_probes as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
        
        // Set buffer sizes
        self.set_buffer_sizes()?;
        
        // Set congestion control algorithm (Linux only)
        #[cfg(target_os = "linux")]
        self.set_congestion_algorithm()?;
        
        Ok(())
    }
    
    fn set_buffer_sizes(&self) -> Result<(), NetworkError> {
        use libc::{SO_SNDBUF, SO_RCVBUF};
        
        let fd = self.stream.as_raw_fd();
        
        unsafe {
            // Send buffer
            setsockopt(
                fd,
                SOL_SOCKET,
                SO_SNDBUF,
                &self.config.send_buffer_size as *const _ as *const c_void,
                std::mem::size_of::<usize>() as u32,
            );
            
            // Receive buffer
            setsockopt(
                fd,
                SOL_SOCKET,
                SO_RCVBUF,
                &self.config.recv_buffer_size as *const _ as *const c_void,
                std::mem::size_of::<usize>() as u32,
            );
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn set_congestion_algorithm(&self) -> Result<(), NetworkError> {
        use std::ffi::CString;
        
        let algorithm = match self.config.congestion_algorithm {
            CongestionAlgorithm::Cubic => "cubic",
            CongestionAlgorithm::Bbr => "bbr",
            CongestionAlgorithm::Vegas => "vegas",
        };
        
        let algo_cstring = CString::new(algorithm)?;
        let fd = self.stream.as_raw_fd();
        
        unsafe {
            libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_CONGESTION,
                algo_cstring.as_ptr() as *const c_void,
                algorithm.len() as u32,
            );
        }
        
        Ok(())
    }
}
```

## UDP Optimization: Speed Over Reliability

When TCP's guarantees are overkill, UDP shines. It's like shouting across a room - fast but no guarantees anyone heard you:

```rust
use std::net::{UdpSocket, SocketAddr};
use std::time::{Duration, Instant};

pub struct OptimizedUdpSocket {
    socket: UdpSocket,
    config: UdpConfig,
    stats: SocketStats,
}

pub struct UdpConfig {
    pub recv_buffer_size: usize,
    pub send_buffer_size: usize,
    pub ttl: u32,
    pub multicast_ttl: u32,
    pub multicast_loop: bool,
}

#[derive(Default)]
struct SocketStats {
    packets_sent: u64,
    packets_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
    last_activity: Option<Instant>,
}

impl OptimizedUdpSocket {
    pub fn bind(addr: SocketAddr, config: UdpConfig) -> Result<Self, NetworkError> {
        let socket = UdpSocket::bind(addr)?;
        
        // Apply optimizations
        socket.set_recv_buffer_size(config.recv_buffer_size)?;
        socket.set_send_buffer_size(config.send_buffer_size)?;
        socket.set_ttl(config.ttl)?;
        socket.set_multicast_ttl_v4(config.multicast_ttl)?;
        socket.set_multicast_loop_v4(config.multicast_loop)?;
        
        // Non-blocking for async operations
        socket.set_nonblocking(true)?;
        
        Ok(Self {
            socket,
            config,
            stats: SocketStats::default(),
        })
    }
    
    pub fn send_to(&mut self, buf: &[u8], addr: SocketAddr) -> Result<usize, NetworkError> {
        let bytes = self.socket.send_to(buf, addr)?;
        self.stats.packets_sent += 1;
        self.stats.bytes_sent += bytes as u64;
        self.stats.last_activity = Some(Instant::now());
        Ok(bytes)
    }
    
    pub fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetworkError> {
        let (bytes, addr) = self.socket.recv_from(buf)?;
        self.stats.packets_received += 1;
        self.stats.bytes_received += bytes as u64;
        self.stats.last_activity = Some(Instant::now());
        Ok((bytes, addr))
    }
}
```

## QUIC: The Best of Both Worlds

QUIC is like TCP and UDP had a baby - reliable when needed, fast always, with built-in encryption:

```rust
use quinn::{Endpoint, ServerConfig, ClientConfig, Connection};
use std::sync::Arc;

pub struct QuicManager {
    endpoint: Endpoint,
    connections: Arc<RwLock<HashMap<SocketAddr, Connection>>>,
}

impl QuicManager {
    pub async fn new_server(addr: SocketAddr) -> Result<Self, NetworkError> {
        let (cert, key) = generate_self_signed_cert()?;
        
        let mut server_config = ServerConfig::with_single_cert(vec![cert], key)?;
        
        // Optimize for low latency gaming
        let mut transport = quinn::TransportConfig::default();
        transport.max_idle_timeout(Some(Duration::from_secs(30).try_into()?));
        transport.keep_alive_interval(Some(Duration::from_secs(10)));
        
        // Optimize congestion control
        transport.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        
        // Allow more concurrent streams for game data
        transport.max_concurrent_bidi_streams(256u32.into());
        transport.max_concurrent_uni_streams(256u32.into());
        
        server_config.transport = Arc::new(transport);
        
        let endpoint = Endpoint::server(server_config, addr)?;
        
        Ok(Self {
            endpoint,
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub async fn send_datagram(&self, addr: SocketAddr, data: &[u8]) -> Result<(), NetworkError> {
        let connections = self.connections.read().await;
        
        if let Some(conn) = connections.get(&addr) {
            // QUIC datagrams are unreliable but encrypted
            conn.send_datagram(data.into())?;
        } else {
            // Establish connection first
            drop(connections);
            let conn = self.connect(addr).await?;
            conn.send_datagram(data.into())?;
        }
        
        Ok(())
    }
    
    pub async fn open_stream(&self, addr: SocketAddr) -> Result<quinn::SendStream, NetworkError> {
        let connections = self.connections.read().await;
        
        let conn = if let Some(conn) = connections.get(&addr) {
            conn.clone()
        } else {
            drop(connections);
            self.connect(addr).await?
        };
        
        let stream = conn.open_uni().await?;
        Ok(stream)
    }
}
```

## Congestion Control: Being a Good Network Citizen

Congestion control is like traffic management - push too hard and everyone slows down. Let's implement custom congestion control:

```rust
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct CongestionController {
    algorithm: Box<dyn CongestionAlgorithm>,
    rtt_estimator: RttEstimator,
    loss_detector: LossDetector,
}

trait CongestionAlgorithm: Send + Sync {
    fn on_packet_sent(&mut self, bytes: usize, now: Instant);
    fn on_ack_received(&mut self, bytes: usize, rtt: Duration, now: Instant);
    fn on_loss_detected(&mut self, bytes: usize, now: Instant);
    fn get_congestion_window(&self) -> usize;
    fn get_pacing_rate(&self) -> Option<u64>; // bytes per second
}

// BBR: Bottleneck Bandwidth and Round-trip propagation time
pub struct BbrAlgorithm {
    min_rtt: Duration,
    max_bandwidth: u64,
    congestion_window: usize,
    pacing_gain: f64,
    cwnd_gain: f64,
    
    // BBR state machine
    mode: BbrMode,
    cycle_index: usize,
    
    // Measurements
    bandwidth_samples: VecDeque<(Instant, u64)>,
    rtt_samples: VecDeque<(Instant, Duration)>,
}

#[derive(Debug, Clone)]
enum BbrMode {
    Startup,    // Exponential growth
    Drain,      // Drain the queue
    ProbeBw,    // Probe for bandwidth
    ProbeRtt,   // Probe for RTT
}

impl BbrAlgorithm {
    pub fn new() -> Self {
        Self {
            min_rtt: Duration::from_millis(100),
            max_bandwidth: 1_000_000, // 1 MB/s initial estimate
            congestion_window: 10 * 1500, // 10 packets
            pacing_gain: 2.77, // Startup gain
            cwnd_gain: 2.0,
            mode: BbrMode::Startup,
            cycle_index: 0,
            bandwidth_samples: VecDeque::new(),
            rtt_samples: VecDeque::new(),
        }
    }
    
    fn update_bandwidth_estimate(&mut self, delivered: u64, interval: Duration) {
        if interval.as_micros() == 0 {
            return;
        }
        
        let bandwidth = (delivered as u128 * 1_000_000) / interval.as_micros();
        self.max_bandwidth = self.max_bandwidth.max(bandwidth as u64);
        
        // Keep samples for windowed max
        self.bandwidth_samples.push_back((Instant::now(), bandwidth as u64));
        
        // Remove old samples (> 10 RTTs)
        let cutoff = Instant::now() - (self.min_rtt * 10);
        while let Some(&(time, _)) = self.bandwidth_samples.front() {
            if time < cutoff {
                self.bandwidth_samples.pop_front();
            } else {
                break;
            }
        }
    }
    
    fn update_mode(&mut self) {
        match self.mode {
            BbrMode::Startup => {
                // Exit startup when we stop seeing bandwidth increases
                if self.is_full_pipe() {
                    self.mode = BbrMode::Drain;
                    self.pacing_gain = 1.0 / 2.77; // Drain gain
                    self.cwnd_gain = 1.0;
                }
            }
            BbrMode::Drain => {
                // Exit drain when queue is empty
                if self.congestion_window <= self.bdp() {
                    self.mode = BbrMode::ProbeBw;
                    self.cycle_index = 0;
                }
            }
            BbrMode::ProbeBw => {
                // Cycle through different pacing gains
                self.cycle_index = (self.cycle_index + 1) % 8;
                self.pacing_gain = match self.cycle_index {
                    0 => 1.25,  // Probe up
                    1 => 0.75,  // Probe down
                    _ => 1.0,   // Cruise
                };
                
                // Periodically probe RTT
                if self.should_probe_rtt() {
                    self.mode = BbrMode::ProbeRtt;
                    self.pacing_gain = 1.0;
                    self.cwnd_gain = 1.0;
                }
            }
            BbrMode::ProbeRtt => {
                // Exit after probing
                if self.probe_rtt_done() {
                    self.mode = BbrMode::ProbeBw;
                    self.cycle_index = 0;
                }
            }
        }
    }
    
    fn bdp(&self) -> usize {
        // Bandwidth-Delay Product
        let bdp_bytes = (self.max_bandwidth as u128 * self.min_rtt.as_micros()) / 1_000_000;
        bdp_bytes as usize
    }
    
    fn is_full_pipe(&self) -> bool {
        // Simplified: real BBR has more complex logic
        self.bandwidth_samples.len() >= 3 &&
        self.bandwidth_samples.iter()
            .rev()
            .take(3)
            .map(|(_, bw)| bw)
            .max()
            .map_or(false, |&max_recent| {
                max_recent < self.max_bandwidth * 125 / 100 // Less than 25% increase
            })
    }
    
    fn should_probe_rtt(&self) -> bool {
        // Probe RTT every 10 seconds
        self.rtt_samples.back()
            .map_or(true, |(time, _)| time.elapsed() > Duration::from_secs(10))
    }
    
    fn probe_rtt_done(&self) -> bool {
        // Probe for at least 200ms
        self.rtt_samples.back()
            .map_or(false, |(time, _)| time.elapsed() > Duration::from_millis(200))
    }
}

impl CongestionAlgorithm for BbrAlgorithm {
    fn on_packet_sent(&mut self, bytes: usize, now: Instant) {
        // Track in-flight data
    }
    
    fn on_ack_received(&mut self, bytes: usize, rtt: Duration, now: Instant) {
        // Update RTT estimate
        self.min_rtt = self.min_rtt.min(rtt);
        self.rtt_samples.push_back((now, rtt));
        
        // Update bandwidth estimate
        self.update_bandwidth_estimate(bytes as u64, rtt);
        
        // Update congestion window
        self.congestion_window = ((self.bdp() as f64) * self.cwnd_gain) as usize;
        
        // Update mode
        self.update_mode();
    }
    
    fn on_loss_detected(&mut self, bytes: usize, now: Instant) {
        // BBR doesn't reduce on loss in most modes
        if matches!(self.mode, BbrMode::Startup) {
            // Might have overshot
            self.mode = BbrMode::Drain;
        }
    }
    
    fn get_congestion_window(&self) -> usize {
        self.congestion_window
    }
    
    fn get_pacing_rate(&self) -> Option<u64> {
        Some((self.max_bandwidth as f64 * self.pacing_gain) as u64)
    }
}
```

## Traffic Shaping: Controlling the Flow

Traffic shaping ensures we don't overwhelm the network or exceed limits:

```rust
use std::collections::VecDeque;
use tokio::time::sleep;

pub struct TrafficShaper {
    config: ShaperConfig,
    token_bucket: TokenBucket,
    packet_queue: VecDeque<QueuedPacket>,
}

pub struct ShaperConfig {
    pub max_bandwidth: u64,      // bytes per second
    pub burst_size: usize,       // max burst in bytes
    pub queue_size: usize,       // max queued packets
    pub priority_levels: u8,     // number of priority queues
}

struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

struct QueuedPacket {
    data: Vec<u8>,
    priority: u8,
    queued_at: Instant,
    deadline: Option<Instant>,
}

impl TokenBucket {
    fn new(rate: u64, burst_size: usize) -> Self {
        Self {
            tokens: burst_size as f64,
            max_tokens: burst_size as f64,
            refill_rate: rate as f64,
            last_refill: Instant::now(),
        }
    }
    
    fn try_consume(&mut self, bytes: usize) -> bool {
        self.refill();
        
        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }
    
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
    
    fn time_until_tokens(&self, bytes: usize) -> Duration {
        if self.tokens >= bytes as f64 {
            Duration::ZERO
        } else {
            let needed = bytes as f64 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }
}

impl TrafficShaper {
    pub fn new(config: ShaperConfig) -> Self {
        Self {
            token_bucket: TokenBucket::new(config.max_bandwidth, config.burst_size),
            packet_queue: VecDeque::with_capacity(config.queue_size),
            config,
        }
    }
    
    pub async fn send_shaped<F>(&mut self, data: Vec<u8>, priority: u8, send_fn: F) 
    -> Result<(), NetworkError> 
    where
        F: FnOnce(Vec<u8>) -> Result<(), NetworkError>,
    {
        // Try to send immediately
        if self.packet_queue.is_empty() && self.token_bucket.try_consume(data.len()) {
            return send_fn(data);
        }
        
        // Queue the packet
        if self.packet_queue.len() >= self.config.queue_size {
            return Err(NetworkError::QueueFull);
        }
        
        self.packet_queue.push_back(QueuedPacket {
            data,
            priority,
            queued_at: Instant::now(),
            deadline: None,
        });
        
        // Sort by priority
        self.packet_queue.make_contiguous()
            .sort_by_key(|p| std::cmp::Reverse(p.priority));
        
        // Process queue
        self.process_queue(send_fn).await
    }
    
    async fn process_queue<F>(&mut self, send_fn: F) -> Result<(), NetworkError>
    where
        F: FnOnce(Vec<u8>) -> Result<(), NetworkError>,
    {
        while let Some(packet) = self.packet_queue.front() {
            let wait_time = self.token_bucket.time_until_tokens(packet.data.len());
            
            if wait_time > Duration::ZERO {
                sleep(wait_time).await;
            }
            
            if self.token_bucket.try_consume(packet.data.len()) {
                let packet = self.packet_queue.pop_front().unwrap();
                send_fn(packet.data)?;
            } else {
                break;
            }
        }
        
        Ok(())
    }
}
```

## Bandwidth Estimation: Knowing Your Limits

Understanding available bandwidth helps us adapt:

```rust
pub struct BandwidthEstimator {
    samples: VecDeque<BandwidthSample>,
    max_samples: usize,
    estimation_window: Duration,
}

#[derive(Clone)]
struct BandwidthSample {
    timestamp: Instant,
    bytes: usize,
    duration: Duration,
    bandwidth: f64, // bytes per second
}

impl BandwidthEstimator {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            max_samples: 100,
            estimation_window: Duration::from_secs(10),
        }
    }
    
    pub fn add_sample(&mut self, bytes: usize, duration: Duration) {
        let bandwidth = if duration.as_micros() > 0 {
            (bytes as f64 * 1_000_000.0) / duration.as_micros() as f64
        } else {
            return; // Invalid sample
        };
        
        let sample = BandwidthSample {
            timestamp: Instant::now(),
            bytes,
            duration,
            bandwidth,
        };
        
        self.samples.push_back(sample);
        
        // Remove old samples
        let cutoff = Instant::now() - self.estimation_window;
        while let Some(front) = self.samples.front() {
            if front.timestamp < cutoff {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        
        // Limit sample count
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }
    
    pub fn estimate_bandwidth(&self) -> BandwidthEstimate {
        if self.samples.is_empty() {
            return BandwidthEstimate::default();
        }
        
        // Calculate various statistics
        let bandwidths: Vec<f64> = self.samples.iter()
            .map(|s| s.bandwidth)
            .collect();
        
        let mean = bandwidths.iter().sum::<f64>() / bandwidths.len() as f64;
        
        let variance = bandwidths.iter()
            .map(|&bw| (bw - mean).powi(2))
            .sum::<f64>() / bandwidths.len() as f64;
        
        let std_dev = variance.sqrt();
        
        // Median
        let mut sorted = bandwidths.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = sorted[sorted.len() / 2];
        
        // Exponentially weighted moving average
        let ewma = self.calculate_ewma();
        
        BandwidthEstimate {
            current: bandwidths.last().copied().unwrap_or(0.0),
            mean,
            median,
            std_dev,
            ewma,
            min: sorted.first().copied().unwrap_or(0.0),
            max: sorted.last().copied().unwrap_or(0.0),
            confidence: self.calculate_confidence(),
        }
    }
    
    fn calculate_ewma(&self) -> f64 {
        let alpha = 0.2; // Smoothing factor
        
        self.samples.iter()
            .fold(0.0, |acc, sample| {
                acc * (1.0 - alpha) + sample.bandwidth * alpha
            })
    }
    
    fn calculate_confidence(&self) -> f64 {
        // More samples = higher confidence
        let sample_factor = (self.samples.len() as f64 / 20.0).min(1.0);
        
        // Recent samples = higher confidence
        let recency_factor = if let Some(latest) = self.samples.back() {
            let age = latest.timestamp.elapsed().as_secs_f64();
            (1.0 - (age / 10.0)).max(0.0)
        } else {
            0.0
        };
        
        sample_factor * 0.7 + recency_factor * 0.3
    }
}

#[derive(Debug, Default)]
pub struct BandwidthEstimate {
    pub current: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub ewma: f64,
    pub min: f64,
    pub max: f64,
    pub confidence: f64, // 0.0 to 1.0
}
```

## Connection Multiplexing: One Socket, Many Streams

Multiplexing lets us send multiple logical streams over one connection:

```rust
use tokio::sync::mpsc;
use bytes::BytesMut;

pub struct MultiplexedConnection {
    transport: Box<dyn Transport>,
    streams: HashMap<StreamId, Stream>,
    next_stream_id: StreamId,
    config: MultiplexConfig,
}

pub struct MultiplexConfig {
    pub max_streams: usize,
    pub stream_window_size: usize,
    pub connection_window_size: usize,
    pub priority_levels: u8,
}

type StreamId = u32;

struct Stream {
    id: StreamId,
    priority: u8,
    send_window: usize,
    recv_window: usize,
    send_buffer: BytesMut,
    recv_buffer: BytesMut,
    state: StreamState,
}

#[derive(Debug, Clone)]
enum StreamState {
    Open,
    HalfClosedLocal,
    HalfClosedRemote,
    Closed,
}

impl MultiplexedConnection {
    pub fn open_stream(&mut self, priority: u8) -> Result<StreamId, NetworkError> {
        if self.streams.len() >= self.config.max_streams {
            return Err(NetworkError::TooManyStreams);
        }
        
        let id = self.next_stream_id;
        self.next_stream_id += 2; // Odd for client, even for server
        
        let stream = Stream {
            id,
            priority,
            send_window: self.config.stream_window_size,
            recv_window: self.config.stream_window_size,
            send_buffer: BytesMut::new(),
            recv_buffer: BytesMut::new(),
            state: StreamState::Open,
        };
        
        self.streams.insert(id, stream);
        
        // Send stream open frame
        self.send_frame(Frame::StreamOpen { id, priority })?;
        
        Ok(id)
    }
    
    pub fn send_data(&mut self, stream_id: StreamId, data: &[u8]) -> Result<(), NetworkError> {
        let stream = self.streams.get_mut(&stream_id)
            .ok_or(NetworkError::StreamNotFound)?;
        
        if !matches!(stream.state, StreamState::Open | StreamState::HalfClosedRemote) {
            return Err(NetworkError::StreamClosed);
        }
        
        // Check window
        if data.len() > stream.send_window {
            return Err(NetworkError::WindowExceeded);
        }
        
        // Buffer data
        stream.send_buffer.extend_from_slice(data);
        stream.send_window -= data.len();
        
        // Try to send immediately
        self.flush_stream(stream_id)?;
        
        Ok(())
    }
    
    fn flush_stream(&mut self, stream_id: StreamId) -> Result<(), NetworkError> {
        let stream = self.streams.get_mut(&stream_id)
            .ok_or(NetworkError::StreamNotFound)?;
        
        if stream.send_buffer.is_empty() {
            return Ok(());
        }
        
        // Send what we can
        let to_send = stream.send_buffer.split();
        
        self.send_frame(Frame::Data {
            stream_id,
            data: to_send.freeze(),
        })?;
        
        Ok(())
    }
    
    pub fn receive_data(&mut self, stream_id: StreamId) -> Option<Vec<u8>> {
        self.streams.get_mut(&stream_id)
            .and_then(|stream| {
                if stream.recv_buffer.is_empty() {
                    None
                } else {
                    Some(stream.recv_buffer.split().to_vec())
                }
            })
    }
}
```

## Network Quality Monitoring: Real-Time Adaptation

Monitoring network quality lets us adapt in real-time:

```rust
pub struct NetworkQualityMonitor {
    metrics: Arc<RwLock<NetworkMetrics>>,
    history: VecDeque<NetworkSnapshot>,
    quality_score: Arc<AtomicU32>,
}

#[derive(Default)]
struct NetworkMetrics {
    rtt: Duration,
    jitter: Duration,
    packet_loss: f64,
    bandwidth_up: f64,
    bandwidth_down: f64,
    connection_count: usize,
}

struct NetworkSnapshot {
    timestamp: Instant,
    metrics: NetworkMetrics,
    score: f64,
}

impl NetworkQualityMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(NetworkMetrics::default())),
            history: VecDeque::with_capacity(1000),
            quality_score: Arc::new(AtomicU32::new(100)),
        }
    }
    
    pub async fn update_rtt(&self, rtt: Duration) {
        let mut metrics = self.metrics.write().await;
        
        // Exponentially weighted moving average
        let alpha = 0.125;
        let old_rtt = metrics.rtt.as_secs_f64();
        let new_rtt = rtt.as_secs_f64();
        
        metrics.rtt = Duration::from_secs_f64(
            old_rtt * (1.0 - alpha) + new_rtt * alpha
        );
        
        // Update jitter (variation in RTT)
        let diff = (new_rtt - old_rtt).abs();
        let old_jitter = metrics.jitter.as_secs_f64();
        
        metrics.jitter = Duration::from_secs_f64(
            old_jitter * (1.0 - alpha) + diff * alpha
        );
    }
    
    pub async fn update_packet_loss(&self, sent: u64, lost: u64) {
        let mut metrics = self.metrics.write().await;
        
        if sent > 0 {
            let loss_rate = lost as f64 / sent as f64;
            let alpha = 0.1;
            
            metrics.packet_loss = metrics.packet_loss * (1.0 - alpha) + loss_rate * alpha;
        }
    }
    
    pub async fn calculate_quality_score(&self) -> f64 {
        let metrics = self.metrics.read().await;
        
        // Score components (0-100 each)
        let rtt_score = Self::score_rtt(metrics.rtt);
        let jitter_score = Self::score_jitter(metrics.jitter);
        let loss_score = Self::score_packet_loss(metrics.packet_loss);
        let bandwidth_score = Self::score_bandwidth(metrics.bandwidth_down);
        
        // Weighted average
        let score = rtt_score * 0.3 +
                   jitter_score * 0.2 +
                   loss_score * 0.3 +
                   bandwidth_score * 0.2;
        
        // Store for external access
        self.quality_score.store(score as u32, Ordering::Relaxed);
        
        score
    }
    
    fn score_rtt(rtt: Duration) -> f64 {
        // Scoring: <50ms = 100, 50-150ms = linear, >500ms = 0
        let ms = rtt.as_millis() as f64;
        
        if ms <= 50.0 {
            100.0
        } else if ms <= 150.0 {
            100.0 - (ms - 50.0) * 0.5
        } else if ms <= 500.0 {
            50.0 - (ms - 150.0) * 0.14
        } else {
            0.0
        }
    }
    
    fn score_jitter(jitter: Duration) -> f64 {
        // Scoring: <10ms = 100, 10-50ms = linear, >100ms = 0
        let ms = jitter.as_millis() as f64;
        
        if ms <= 10.0 {
            100.0
        } else if ms <= 50.0 {
            100.0 - (ms - 10.0) * 1.25
        } else if ms <= 100.0 {
            50.0 - (ms - 50.0)
        } else {
            0.0
        }
    }
    
    fn score_packet_loss(loss: f64) -> f64 {
        // Scoring: 0% = 100, 1% = 80, 5% = 20, >10% = 0
        if loss <= 0.0 {
            100.0
        } else if loss <= 0.01 {
            100.0 - loss * 2000.0
        } else if loss <= 0.05 {
            80.0 - (loss - 0.01) * 1500.0
        } else if loss <= 0.10 {
            20.0 - (loss - 0.05) * 400.0
        } else {
            0.0
        }
    }
    
    fn score_bandwidth(bandwidth: f64) -> f64 {
        // Scoring: >10Mbps = 100, 1-10Mbps = linear, <1Mbps = poor
        let mbps = bandwidth / 125000.0; // Convert bytes/s to Mbps
        
        if mbps >= 10.0 {
            100.0
        } else if mbps >= 1.0 {
            55.0 + (mbps - 1.0) * 5.0
        } else {
            mbps * 55.0
        }
    }
}
```

## BitCraps Network Optimization Strategy

Here's how BitCraps combines all these techniques:

```rust
pub struct BitCrapsNetworkOptimizer {
    transport_selector: TransportSelector,
    congestion_controller: CongestionController,
    traffic_shaper: TrafficShaper,
    quality_monitor: NetworkQualityMonitor,
    bandwidth_estimator: BandwidthEstimator,
}

impl BitCrapsNetworkOptimizer {
    pub async fn optimize_for_conditions(&mut self) -> NetworkStrategy {
        let quality = self.quality_monitor.calculate_quality_score().await;
        let bandwidth = self.bandwidth_estimator.estimate_bandwidth();
        
        if quality > 80.0 && bandwidth.current > 1_000_000.0 {
            // Excellent conditions: maximize throughput
            NetworkStrategy {
                transport: Transport::Quic,
                congestion_algorithm: CongestionAlgorithm::Bbr,
                packet_size: 1450, // Near MTU
                batch_size: 16,
                compression: CompressionLevel::Fast,
                redundancy: RedundancyLevel::None,
            }
        } else if quality > 60.0 {
            // Good conditions: balanced approach
            NetworkStrategy {
                transport: Transport::Tcp,
                congestion_algorithm: CongestionAlgorithm::Cubic,
                packet_size: 1200,
                batch_size: 8,
                compression: CompressionLevel::Balanced,
                redundancy: RedundancyLevel::Low,
            }
        } else if quality > 40.0 {
            // Poor conditions: focus on reliability
            NetworkStrategy {
                transport: Transport::Tcp,
                congestion_algorithm: CongestionAlgorithm::Vegas,
                packet_size: 576, // Conservative
                batch_size: 4,
                compression: CompressionLevel::High,
                redundancy: RedundancyLevel::Medium,
            }
        } else {
            // Terrible conditions: survival mode
            NetworkStrategy {
                transport: Transport::Udp, // With custom reliability
                congestion_algorithm: CongestionAlgorithm::Vegas,
                packet_size: 512,
                batch_size: 1,
                compression: CompressionLevel::Maximum,
                redundancy: RedundancyLevel::High,
            }
        }
    }
}
```

## Practical Exercises

### Exercise 1: Implement Packet Pacing
Smooth out traffic to avoid bursts:

```rust
struct PacketPacer {
    rate: u64, // bytes per second
    last_send: Instant,
}

impl PacketPacer {
    async fn pace_packet(&mut self, size: usize) {
        // Your task: Calculate and wait for appropriate interval
        todo!("Implement packet pacing")
    }
}
```

### Exercise 2: Build a Jitter Buffer
Smooth out packet arrival times:

```rust
struct JitterBuffer {
    packets: BTreeMap<u64, Packet>,
    target_delay: Duration,
}

impl JitterBuffer {
    fn insert(&mut self, seq: u64, packet: Packet) {
        // Your task: Insert and manage buffer depth
        todo!("Implement jitter buffer")
    }
    
    fn get_ready_packets(&mut self) -> Vec<Packet> {
        // Your task: Return packets that have waited long enough
        todo!("Get ready packets")
    }
}
```

### Exercise 3: Network Condition Simulator
Test your optimizations:

```rust
struct NetworkSimulator {
    latency: Duration,
    jitter: Duration,
    loss_rate: f64,
    bandwidth: u64,
}

impl NetworkSimulator {
    async fn simulate_send(&self, packet: &[u8]) -> Result<(), NetworkError> {
        // Your task: Simulate network conditions
        // - Add random delay (latency + jitter)
        // - Randomly drop packets (loss_rate)
        // - Limit bandwidth
        todo!("Implement network simulation")
    }
}
```

## Common Pitfalls and Solutions

### 1. The Bufferbloat Problem
Large buffers cause latency:

```rust
// Bad: Huge buffers
socket.set_recv_buffer_size(10_000_000)?; // 10MB!

// Good: Right-sized buffers
let bdp = bandwidth * rtt;
socket.set_recv_buffer_size(bdp * 2)?;
```

### 2. The Head-of-Line Blocking
One slow packet blocks all:

```rust
// Solution: Multiple streams or out-of-order delivery
// Use QUIC or SCTP instead of TCP
```

### 3. The Last Mile Problem
Your optimization means nothing if the user's WiFi is terrible:

```rust
// Adapt to actual conditions, not theoretical maximums
if wifi_signal_strength < -70 {
    // Weak signal: be conservative
    use_smaller_packets();
    increase_redundancy();
}
```

## Conclusion: The Network is the Computer

Sun Microsystems' old slogan "The Network is the Computer" has never been more true. In BitCraps, the network isn't just how we communicate - it defines the user experience. A perfectly coded game is worthless if packets arrive late.

Key takeaways:

1. **Measure, don't assume** - Real networks behave nothing like localhost
2. **Adapt constantly** - Network conditions change by the second
3. **Be a good citizen** - Aggressive optimization can hurt everyone
4. **Plan for the worst** - If it can fail, it will fail
5. **Optimize holistically** - The best protocol depends on your use case

Remember: In distributed systems, the network is both your greatest ally and your worst enemy. Master it, and you master the system.

## Additional Resources

- **High Performance Browser Networking** by Ilya Grigorik
- **TCP/IP Illustrated** by W. Richard Stevens
- **QUIC Working Group** - The future of internet transport
- **Bufferbloat.net** - Understanding modern network problems

The next time someone complains about lag in BitCraps, you'll know it's not just "the internet being slow" - it's a complex dance of packets, protocols, and optimizations, all working together to deliver the best possible experience over an inherently unreliable medium.
