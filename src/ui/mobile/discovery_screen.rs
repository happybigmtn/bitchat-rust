//! Peer discovery visualization screen
//! 
//! Provides visual representation of nearby peers and network topology

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::mesh::PeerId;
use crate::protocol::CrapTokens;
use super::screen_base::{Screen, ScreenElement, Theme, TouchEvent, RenderContext, ScreenTransition};

/// Discovery screen for finding and connecting to peers
pub struct DiscoveryScreen {
    peers: Arc<RwLock<HashMap<PeerId, PeerNode>>>,
    connections: Arc<RwLock<Vec<Connection>>>,
    local_peer_id: PeerId,
    discovery_state: Arc<RwLock<DiscoveryState>>,
    animation_state: Arc<RwLock<RadarAnimation>>,
    selected_peer: Arc<RwLock<Option<PeerId>>>,
    network_stats: Arc<RwLock<NetworkStats>>,
}

/// Visual representation of a peer
#[derive(Debug, Clone)]
pub struct PeerNode {
    pub id: PeerId,
    pub name: String,
    pub position: (f32, f32),
    pub signal_strength: f32,
    pub last_seen: SystemTime,
    pub is_connected: bool,
    pub is_discovering: bool,
    pub game_count: u32,
    pub reputation: f32,
    pub avatar_color: (u8, u8, u8),
    pub animation_offset: f32,
}

/// Connection between peers
#[derive(Debug, Clone)]
struct Connection {
    from: PeerId,
    to: PeerId,
    strength: f32,
    latency_ms: u32,
    packet_loss: f32,
}

/// Discovery state
#[derive(Debug, Clone)]
enum DiscoveryState {
    Idle,
    Scanning,
    Connecting(PeerId),
    Connected,
    Error(String),
}

/// Radar animation for discovery
#[derive(Debug, Clone)]
struct RadarAnimation {
    sweep_angle: f32,
    pulse_radius: f32,
    pulse_opacity: f32,
    discovered_peers: Vec<(PeerId, f32)>, // peer and animation progress
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_peers: usize,
    pub connected_peers: usize,
    pub discovery_duration: Duration,
    pub average_latency: f32,
    pub network_health: f32,
}

impl DiscoveryScreen {
    /// Create a new discovery screen
    pub fn new(local_peer_id: PeerId) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(Vec::new())),
            local_peer_id,
            discovery_state: Arc::new(RwLock::new(DiscoveryState::Idle)),
            animation_state: Arc::new(RwLock::new(RadarAnimation {
                sweep_angle: 0.0,
                pulse_radius: 0.0,
                pulse_opacity: 1.0,
                discovered_peers: Vec::new(),
            })),
            selected_peer: Arc::new(RwLock::new(None)),
            network_stats: Arc::new(RwLock::new(NetworkStats {
                total_peers: 0,
                connected_peers: 0,
                discovery_duration: Duration::from_secs(0),
                average_latency: 0.0,
                network_health: 1.0,
            })),
        }
    }
    
    /// Start peer discovery
    pub async fn start_discovery(&self) -> Result<(), String> {
        *self.discovery_state.write().await = DiscoveryState::Scanning;
        
        // Reset animation
        let mut anim = self.animation_state.write().await;
        anim.sweep_angle = 0.0;
        anim.pulse_radius = 0.0;
        anim.discovered_peers.clear();
        
        // In real implementation, this would start Bluetooth/network discovery
        Ok(())
    }
    
    /// Stop peer discovery
    pub async fn stop_discovery(&self) {
        *self.discovery_state.write().await = DiscoveryState::Idle;
    }
    
    /// Add a discovered peer
    pub async fn add_peer(&self, peer: PeerNode) {
        let mut peers = self.peers.write().await;
        
        // Add to discovered animation
        let mut anim = self.animation_state.write().await;
        anim.discovered_peers.push((peer.id.clone(), 0.0));
        
        // Calculate position based on signal strength and angle
        let angle = (peers.len() as f32) * std::f32::consts::PI / 4.0;
        let distance = 0.2 + (1.0 - peer.signal_strength) * 0.3;
        let position = (
            0.5 + distance * angle.cos(),
            0.5 + distance * angle.sin(),
        );
        
        let mut peer = peer;
        peer.position = position;
        peers.insert(peer.id.clone(), peer);
        
        // Update stats
        let mut stats = self.network_stats.write().await;
        stats.total_peers = peers.len();
    }
    
    /// Connect to a peer
    pub async fn connect_to_peer(&self, peer_id: &PeerId) -> Result<(), String> {
        *self.discovery_state.write().await = DiscoveryState::Connecting(peer_id.clone());
        
        // Simulate connection delay
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Update peer state
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(peer_id) {
            peer.is_connected = true;
            
            // Add connection line
            let mut connections = self.connections.write().await;
            connections.push(Connection {
                from: self.local_peer_id.clone(),
                to: peer_id.clone(),
                strength: peer.signal_strength,
                latency_ms: ((1.0 - peer.signal_strength) * 100.0) as u32,
                packet_loss: (1.0 - peer.signal_strength) * 0.1,
            });
            
            *self.discovery_state.write().await = DiscoveryState::Connected;
            
            // Update stats
            let mut stats = self.network_stats.write().await;
            stats.connected_peers += 1;
            stats.average_latency = connections.iter()
                .map(|c| c.latency_ms as f32)
                .sum::<f32>() / connections.len() as f32;
            
            Ok(())
        } else {
            Err("Peer not found".to_string())
        }
    }
    
    /// Update radar animation
    pub async fn update_animation(&self, delta_time: Duration) {
        let mut anim = self.animation_state.write().await;
        
        // Update radar sweep
        anim.sweep_angle += delta_time.as_secs_f32() * 60.0; // 60 degrees per second
        if anim.sweep_angle >= 360.0 {
            anim.sweep_angle -= 360.0;
        }
        
        // Update pulse
        anim.pulse_radius += delta_time.as_secs_f32() * 0.3;
        if anim.pulse_radius > 1.0 {
            anim.pulse_radius = 0.0;
            anim.pulse_opacity = 1.0;
        } else {
            anim.pulse_opacity = 1.0 - anim.pulse_radius;
        }
        
        // Update discovered peer animations
        for (_, progress) in &mut anim.discovered_peers {
            *progress = (*progress + delta_time.as_secs_f32() * 2.0).min(1.0);
        }
        
        // Update peer floating animations
        let mut peers = self.peers.write().await;
        for peer in peers.values_mut() {
            peer.animation_offset += delta_time.as_secs_f32() * 2.0;
        }
    }
    
    /// Get peer at position
    fn get_peer_at(&self, x: f32, y: f32) -> Option<PeerId> {
        // In real implementation, check which peer node was tapped
        None
    }
    
    /// Calculate network topology positions
    async fn calculate_topology(&self) {
        let peers = self.peers.read().await;
        let connections = self.connections.read().await;
        
        // Force-directed graph layout
        // In real implementation, use spring-force algorithm
        // to position nodes based on connections
    }
}

impl Screen for DiscoveryScreen {
    fn render(&self, ctx: &mut RenderContext) {
        // Render background
        self.render_background(ctx);
        
        // Render radar effect
        self.render_radar(ctx);
        
        // Render connection lines
        self.render_connections(ctx);
        
        // Render peer nodes
        self.render_peers(ctx);
        
        // Render local node (center)
        self.render_local_node(ctx);
        
        // Render stats overlay
        self.render_stats(ctx);
        
        // Render controls
        self.render_controls(ctx);
    }
    
    fn handle_touch(&mut self, event: TouchEvent) -> Option<ScreenTransition> {
        match event {
            TouchEvent::Tap { x, y } => {
                // Check if discovery button was tapped
                if x >= 0.35 && x <= 0.65 && y >= 0.85 && y <= 0.95 {
                    // Toggle discovery
                    // In real implementation, handle async properly
                }
                
                // Check if a peer was tapped
                if let Some(peer_id) = self.get_peer_at(x, y) {
                    // Select/connect to peer
                }
            }
            TouchEvent::DoubleTap { x, y } => {
                // Quick connect to peer
                if let Some(peer_id) = self.get_peer_at(x, y) {
                    // Connect immediately
                }
            }
            TouchEvent::Pinch { scale, .. } => {
                // Zoom network view
            }
            _ => {}
        }
        
        None
    }
    
    fn update(&mut self, delta_time: Duration) {
        // Update is handled by async methods
    }
}

// Rendering helper methods
impl DiscoveryScreen {
    fn render_background(&self, ctx: &mut RenderContext) {
        // Dark gradient background
        ctx.fill_gradient(
            0.0, 0.0, 1.0, 1.0,
            (10, 10, 30, 255),
            (20, 20, 50, 255),
        );
        
        // Grid pattern
        let grid_spacing = 0.05;
        for i in 0..20 {
            let pos = i as f32 * grid_spacing;
            ctx.draw_line(pos, 0.0, pos, 1.0, (30, 30, 60, 50), 1.0);
            ctx.draw_line(0.0, pos, 1.0, pos, (30, 30, 60, 50), 1.0);
        }
    }
    
    fn render_radar(&self, ctx: &mut RenderContext) {
        // Render radar sweep effect
        // In real implementation, use animation state
        
        // Radar circle
        ctx.draw_circle(0.5, 0.5, 0.4, (0, 255, 0, 30), 2.0);
        ctx.draw_circle(0.5, 0.5, 0.3, (0, 255, 0, 20), 1.0);
        ctx.draw_circle(0.5, 0.5, 0.2, (0, 255, 0, 20), 1.0);
        ctx.draw_circle(0.5, 0.5, 0.1, (0, 255, 0, 20), 1.0);
        
        // Sweep line (would be animated)
        ctx.draw_line(0.5, 0.5, 0.9, 0.5, (0, 255, 0, 100), 2.0);
        
        // Pulse effect (would be animated)
        ctx.draw_circle(0.5, 0.5, 0.2, (0, 255, 0, 50), 3.0);
    }
    
    fn render_connections(&self, ctx: &mut RenderContext) {
        // Draw connection lines between peers
        // In real implementation, read from connections state
    }
    
    fn render_peers(&self, ctx: &mut RenderContext) {
        // Render each discovered peer
        // In real implementation, iterate through peers map
        
        // Example peer node
        ctx.fill_circle(0.7, 0.3, 0.04, (100, 150, 255, 255));
        ctx.draw_text("Peer 1", 0.7, 0.35, 10.0, (255, 255, 255, 255));
        
        ctx.fill_circle(0.3, 0.6, 0.04, (255, 150, 100, 255));
        ctx.draw_text("Peer 2", 0.3, 0.65, 10.0, (255, 255, 255, 255));
    }
    
    fn render_local_node(&self, ctx: &mut RenderContext) {
        // Render the local peer (center)
        ctx.fill_circle(0.5, 0.5, 0.05, (0, 255, 0, 255));
        ctx.draw_circle(0.5, 0.5, 0.06, (0, 255, 0, 100), 2.0);
        ctx.draw_text("You", 0.5, 0.56, 12.0, (255, 255, 255, 255));
    }
    
    fn render_stats(&self, ctx: &mut RenderContext) {
        // Network stats overlay
        ctx.fill_rect(0.02, 0.02, 0.3, 0.15, (0, 0, 0, 180));
        ctx.draw_text("Network Status", 0.17, 0.05, 14.0, (255, 255, 255, 255));
        ctx.draw_text("Peers: 2", 0.05, 0.08, 12.0, (200, 200, 200, 255));
        ctx.draw_text("Connected: 1", 0.05, 0.11, 12.0, (200, 200, 200, 255));
        ctx.draw_text("Latency: 25ms", 0.05, 0.14, 12.0, (200, 200, 200, 255));
    }
    
    fn render_controls(&self, ctx: &mut RenderContext) {
        // Discovery button
        ctx.fill_rect(0.35, 0.85, 0.3, 0.1, (0, 150, 0, 255));
        ctx.draw_text("START DISCOVERY", 0.5, 0.9, 14.0, (255, 255, 255, 255));
    }
}

// Additional visualization helpers
impl DiscoveryScreen {
    /// Generate random color for peer avatar
    pub fn generate_peer_color(peer_id: &PeerId) -> (u8, u8, u8) {
        // Generate consistent color from peer ID
        let hash = peer_id.as_bytes().iter().fold(0u32, |acc, &b| {
            acc.wrapping_add(b as u32).wrapping_mul(31)
        });
        
        let hue = (hash % 360) as f32;
        let saturation = 0.7;
        let value = 0.9;
        
        // HSV to RGB conversion
        let c = value * saturation;
        let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
        let m = value - c;
        
        let (r, g, b) = match (hue / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        
        (
            ((r + m) * 255.0) as u8,
            ((g + m) * 255.0) as u8,
            ((b + m) * 255.0) as u8,
        )
    }
    
    /// Calculate signal strength from distance
    pub fn calculate_signal_strength(distance: f32) -> f32 {
        // Inverse square law with cutoff
        let max_range = 10.0; // meters
        if distance > max_range {
            return 0.0;
        }
        
        1.0 - (distance / max_range).powi(2)
    }
}