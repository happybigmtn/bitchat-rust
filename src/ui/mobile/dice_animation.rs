//! Dice rolling animation with physics simulation
//! 
//! Provides realistic dice rolling animations for the game

use std::time::Duration;

/// 3D dice with physics simulation
#[derive(Debug, Clone)]
pub struct Dice3D {
    // Position in 3D space
    pub position: Vec3,
    // Rotation in Euler angles
    pub rotation: Vec3,
    // Linear velocity
    pub velocity: Vec3,
    // Angular velocity
    pub angular_velocity: Vec3,
    // Current face value (1-6)
    pub value: u8,
    // Animation state
    pub state: DiceState,
    // Bounce count
    bounce_count: u32,
    // Animation time
    animation_time: f32,
}

/// 3D vector for physics calculations
#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
    
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            }
        } else {
            *self
        }
    }
    
    pub fn dot(&self, other: &Vec3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    
    pub fn cross(&self, other: &Vec3) -> Vec3 {
        Vec3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Vec3;
    
    fn add(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;
    
    fn sub(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Vec3;
    
    fn mul(self, scalar: f32) -> Vec3 {
        Vec3 {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

/// Dice animation state
#[derive(Debug, Clone, PartialEq)]
pub enum DiceState {
    Idle,
    Rolling,
    Bouncing,
    Settling,
    Settled,
}

/// Dice physics configuration
pub struct DicePhysics {
    pub gravity: f32,
    pub bounciness: f32,
    pub friction: f32,
    pub angular_damping: f32,
    pub settle_threshold: f32,
    pub table_height: f32,
}

impl Default for DicePhysics {
    fn default() -> Self {
        Self {
            gravity: -9.8,
            bounciness: 0.5,
            friction: 0.3,
            angular_damping: 0.95,
            settle_threshold: 0.1,
            table_height: 0.0,
        }
    }
}

impl Dice3D {
    /// Create a new dice
    pub fn new(value: u8) -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 0.0),
            rotation: Vec3::zero(),
            velocity: Vec3::zero(),
            angular_velocity: Vec3::zero(),
            value,
            state: DiceState::Idle,
            bounce_count: 0,
            animation_time: 0.0,
        }
    }
    
    /// Start rolling animation with initial force
    pub fn roll(&mut self, force: Vec3, spin: Vec3) {
        self.state = DiceState::Rolling;
        self.velocity = force;
        self.angular_velocity = spin;
        self.bounce_count = 0;
        self.animation_time = 0.0;
        
        // Set initial position above table
        self.position = Vec3::new(0.0, 2.0, 0.0);
    }
    
    /// Update physics simulation
    pub fn update(&mut self, delta: Duration, physics: &DicePhysics) {
        if self.state == DiceState::Idle || self.state == DiceState::Settled {
            return;
        }
        
        let dt = delta.as_secs_f32();
        self.animation_time += dt;
        
        // Apply gravity
        self.velocity.y += physics.gravity * dt;
        
        // Update position
        self.position = self.position + self.velocity * dt;
        
        // Update rotation
        self.rotation = self.rotation + self.angular_velocity * dt;
        
        // Check collision with table
        if self.position.y <= physics.table_height {
            self.position.y = physics.table_height;
            
            // Bounce
            if self.velocity.y < 0.0 {
                self.velocity.y *= -physics.bounciness;
                self.bounce_count += 1;
                
                // Apply friction to horizontal velocity
                self.velocity.x *= 1.0 - physics.friction;
                self.velocity.z *= 1.0 - physics.friction;
                
                // Dampen angular velocity
                self.angular_velocity = self.angular_velocity * physics.angular_damping;
                
                self.state = DiceState::Bouncing;
            }
        }
        
        // Check if dice has settled
        if self.state == DiceState::Bouncing {
            let total_velocity = self.velocity.length() + self.angular_velocity.length();
            
            if total_velocity < physics.settle_threshold || self.bounce_count > 10 {
                self.state = DiceState::Settling;
                self.settle_to_face();
            }
        }
        
        // Settling animation
        if self.state == DiceState::Settling {
            // Snap to nearest face orientation
            self.rotation.x = self.lerp_angle(self.rotation.x, self.target_rotation_for_value().x, dt * 5.0);
            self.rotation.y = self.lerp_angle(self.rotation.y, self.target_rotation_for_value().y, dt * 5.0);
            self.rotation.z = self.lerp_angle(self.rotation.z, self.target_rotation_for_value().z, dt * 5.0);
            
            // Check if settled
            let rotation_diff = (self.rotation - self.target_rotation_for_value()).length();
            if rotation_diff < 0.01 {
                self.state = DiceState::Settled;
                self.velocity = Vec3::zero();
                self.angular_velocity = Vec3::zero();
            }
        }
    }
    
    /// Settle dice to show the target value
    fn settle_to_face(&mut self) {
        // Calculate which face should be up based on value
        // This would normally use proper rotation matrices
    }
    
    /// Get target rotation for the dice value
    fn target_rotation_for_value(&self) -> Vec3 {
        // Return the rotation that shows the correct face up
        match self.value {
            1 => Vec3::new(0.0, 0.0, 0.0),
            2 => Vec3::new(90.0_f32.to_radians(), 0.0, 0.0),
            3 => Vec3::new(0.0, 90.0_f32.to_radians(), 0.0),
            4 => Vec3::new(0.0, -90.0_f32.to_radians(), 0.0),
            5 => Vec3::new(-90.0_f32.to_radians(), 0.0, 0.0),
            6 => Vec3::new(180.0_f32.to_radians(), 0.0, 0.0),
            _ => Vec3::zero(),
        }
    }
    
    /// Lerp between angles
    fn lerp_angle(&self, current: f32, target: f32, t: f32) -> f32 {
        let mut diff = target - current;
        
        // Wrap to [-PI, PI]
        while diff > std::f32::consts::PI {
            diff -= 2.0 * std::f32::consts::PI;
        }
        while diff < -std::f32::consts::PI {
            diff += 2.0 * std::f32::consts::PI;
        }
        
        current + diff * t.min(1.0)
    }
    
    /// Get 2D screen position for rendering
    pub fn get_screen_position(&self, screen_width: f32, screen_height: f32) -> (f32, f32) {
        // Simple orthographic projection
        let x = screen_width * 0.5 + self.position.x * 100.0;
        let y = screen_height * 0.5 - self.position.y * 100.0 + self.position.z * 50.0;
        (x, y)
    }
    
    /// Check if animation is complete
    pub fn is_settled(&self) -> bool {
        self.state == DiceState::Settled
    }
}

/// Dual dice animation controller
pub struct DiceRollAnimation {
    pub dice1: Dice3D,
    pub dice2: Dice3D,
    physics: DicePhysics,
    pub camera_shake: f32,
    pub sound_triggers: Vec<SoundTrigger>,
}

#[derive(Debug, Clone)]
pub struct SoundTrigger {
    pub time: f32,
    pub sound_type: DiceSound,
    pub played: bool,
}

#[derive(Debug, Clone)]
pub enum DiceSound {
    Throw,
    Bounce,
    Roll,
    Settle,
}

impl DiceRollAnimation {
    /// Create new dice roll animation
    pub fn new(value1: u8, value2: u8) -> Self {
        Self {
            dice1: Dice3D::new(value1),
            dice2: Dice3D::new(value2),
            physics: DicePhysics::default(),
            camera_shake: 0.0,
            sound_triggers: Vec::new(),
        }
    }
    
    /// Start the roll animation
    pub fn start_roll(&mut self, value1: u8, value2: u8) {
        self.dice1.value = value1;
        self.dice2.value = value2;
        
        // Random initial forces for realistic roll
        let force1 = Vec3::new(
            (rand::random::<f32>() - 0.5) * 4.0,
            5.0 + rand::random::<f32>() * 2.0,
            (rand::random::<f32>() - 0.5) * 2.0,
        );
        
        let force2 = Vec3::new(
            (rand::random::<f32>() - 0.5) * 4.0,
            5.0 + rand::random::<f32>() * 2.0,
            (rand::random::<f32>() - 0.5) * 2.0,
        );
        
        let spin1 = Vec3::new(
            rand::random::<f32>() * 20.0 - 10.0,
            rand::random::<f32>() * 20.0 - 10.0,
            rand::random::<f32>() * 20.0 - 10.0,
        );
        
        let spin2 = Vec3::new(
            rand::random::<f32>() * 20.0 - 10.0,
            rand::random::<f32>() * 20.0 - 10.0,
            rand::random::<f32>() * 20.0 - 10.0,
        );
        
        self.dice1.roll(force1, spin1);
        self.dice2.roll(force2, spin2);
        
        // Start with some offset to avoid overlap
        self.dice1.position.x = -0.5;
        self.dice2.position.x = 0.5;
        
        // Reset sound triggers
        self.sound_triggers.clear();
        self.sound_triggers.push(SoundTrigger {
            time: 0.0,
            sound_type: DiceSound::Throw,
            played: false,
        });
        
        // Initial camera shake
        self.camera_shake = 1.0;
    }
    
    /// Update animation
    pub fn update(&mut self, delta: Duration) -> Vec<DiceSound> {
        let mut triggered_sounds = Vec::new();
        
        // Update dice physics
        let old_state1 = self.dice1.state.clone();
        let old_state2 = self.dice2.state.clone();
        
        self.dice1.update(delta, &self.physics);
        self.dice2.update(delta, &self.physics);
        
        // Check for state changes to trigger sounds
        if old_state1 != self.dice1.state && self.dice1.state == DiceState::Bouncing {
            triggered_sounds.push(DiceSound::Bounce);
            self.camera_shake = 0.3;
        }
        
        if old_state2 != self.dice2.state && self.dice2.state == DiceState::Bouncing {
            triggered_sounds.push(DiceSound::Bounce);
            self.camera_shake = 0.3;
        }
        
        if self.dice1.state == DiceState::Settled && self.dice2.state == DiceState::Settled {
            if !self.sound_triggers.iter().any(|t| matches!(t.sound_type, DiceSound::Settle)) {
                triggered_sounds.push(DiceSound::Settle);
                self.sound_triggers.push(SoundTrigger {
                    time: self.dice1.animation_time,
                    sound_type: DiceSound::Settle,
                    played: true,
                });
            }
        }
        
        // Update camera shake
        self.camera_shake *= 0.9;
        if self.camera_shake < 0.01 {
            self.camera_shake = 0.0;
        }
        
        triggered_sounds
    }
    
    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.dice1.is_settled() && self.dice2.is_settled()
    }
    
    /// Get total value
    pub fn get_total(&self) -> u8 {
        self.dice1.value + self.dice2.value
    }
}

// Temporary rand import for dice simulation
use rand;

/// Haptic feedback controller
pub struct HapticController {
    pub enabled: bool,
    intensity: f32,
}

impl HapticController {
    pub fn new() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
        }
    }
    
    /// Trigger haptic feedback for dice events
    pub fn trigger_dice_haptic(&self, sound: &DiceSound) {
        if !self.enabled {
            return;
        }
        
        let pattern = match sound {
            DiceSound::Throw => HapticPattern::Strong,
            DiceSound::Bounce => HapticPattern::Medium,
            DiceSound::Roll => HapticPattern::Light,
            DiceSound::Settle => HapticPattern::Double,
        };
        
        self.play_pattern(pattern);
    }
    
    /// Play haptic pattern
    fn play_pattern(&self, pattern: HapticPattern) {
        // In real implementation, call platform-specific haptic APIs
        match pattern {
            HapticPattern::Light => {
                // iOS: UIImpactFeedbackGenerator.light
                // Android: VibrationEffect.createOneShot(10, 50)
            }
            HapticPattern::Medium => {
                // iOS: UIImpactFeedbackGenerator.medium
                // Android: VibrationEffect.createOneShot(20, 150)
            }
            HapticPattern::Strong => {
                // iOS: UIImpactFeedbackGenerator.heavy
                // Android: VibrationEffect.createOneShot(50, 255)
            }
            HapticPattern::Double => {
                // Custom pattern for settling
                // iOS: Custom CHHapticPattern
                // Android: VibrationEffect.createWaveform
            }
        }
    }
}

#[derive(Debug)]
enum HapticPattern {
    Light,
    Medium,
    Strong,
    Double,
}