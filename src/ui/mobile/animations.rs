//! Animation system for mobile UI
//!
//! Provides smooth animations for dice rolling, transitions, and UI feedback

use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

/// Animation controller for managing UI animations
pub struct AnimationController {
    animations: Vec<Box<dyn Animation>>,
    frame_rate: u32,
    last_update: Instant,
}

/// Trait for animations
pub trait Animation: Send + Sync {
    /// Update the animation state
    fn update(&mut self, delta_time: Duration) -> bool;
    
    /// Get the current animation value
    fn value(&self) -> AnimationValue;
    
    /// Check if animation is complete
    fn is_complete(&self) -> bool;
    
    /// Reset the animation to start
    fn reset(&mut self);
}

/// Animation value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimationValue {
    Float(f32),
    Vector2(f32, f32),
    Vector3(f32, f32, f32),
    Color(u8, u8, u8, u8),
    Transform(Transform),
}

/// Transform for animated objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: (f32, f32, f32),
    pub rotation: (f32, f32, f32),
    pub scale: (f32, f32, f32),
}

/// Dice rolling animation
pub struct DiceAnimation {
    dice_values: (u8, u8),
    current_frame: u32,
    total_frames: u32,
    rotation_speed: f32,
    bounce_height: f32,
    state: DiceAnimationState,
    transform: Transform,
}

#[derive(Debug, Clone, PartialEq)]
enum DiceAnimationState {
    Rolling,
    Bouncing,
    Settling,
    Complete,
}

impl DiceAnimation {
    pub fn new(dice_values: (u8, u8)) -> Self {
        Self {
            dice_values,
            current_frame: 0,
            total_frames: 60, // 1 second at 60fps
            rotation_speed: 720.0, // 2 rotations per second
            bounce_height: 100.0,
            state: DiceAnimationState::Rolling,
            transform: Transform {
                position: (0.0, 0.0, 0.0),
                rotation: (0.0, 0.0, 0.0),
                scale: (1.0, 1.0, 1.0),
            },
        }
    }
    
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.total_frames = (duration.as_millis() as u32 * 60) / 1000;
        self
    }
}

impl Animation for DiceAnimation {
    fn update(&mut self, delta_time: Duration) -> bool {
        if self.state == DiceAnimationState::Complete {
            return false;
        }
        
        self.current_frame += 1;
        let progress = self.current_frame as f32 / self.total_frames as f32;
        
        match self.state {
            DiceAnimationState::Rolling => {
                // Rotate dice while rolling
                let rotation_delta = self.rotation_speed * delta_time.as_secs_f32();
                self.transform.rotation.0 += rotation_delta;
                self.transform.rotation.1 += rotation_delta * 0.7;
                self.transform.rotation.2 += rotation_delta * 1.3;
                
                // Add bounce effect
                let bounce = (progress * std::f32::consts::PI * 4.0).sin() * self.bounce_height;
                self.transform.position.1 = bounce.abs();
                
                if progress >= 0.7 {
                    self.state = DiceAnimationState::Bouncing;
                }
            }
            DiceAnimationState::Bouncing => {
                // Slow down rotation
                let slowdown = 1.0 - (progress - 0.7) * 3.0;
                let rotation_delta = self.rotation_speed * delta_time.as_secs_f32() * slowdown.max(0.1);
                self.transform.rotation.0 += rotation_delta;
                
                // Damped bouncing
                let bounce_dampening = 1.0 - (progress - 0.7) * 2.0;
                let bounce = (progress * std::f32::consts::PI * 8.0).sin() 
                    * self.bounce_height * bounce_dampening.max(0.0);
                self.transform.position.1 = bounce.abs();
                
                if progress >= 0.95 {
                    self.state = DiceAnimationState::Settling;
                }
            }
            DiceAnimationState::Settling => {
                // Final settling
                self.transform.position.1 *= 0.9;
                
                // Snap to final rotation showing dice values
                let target_rotation = self.get_target_rotation();
                self.transform.rotation.0 = lerp(self.transform.rotation.0, target_rotation.0, 0.2);
                self.transform.rotation.1 = lerp(self.transform.rotation.1, target_rotation.1, 0.2);
                self.transform.rotation.2 = lerp(self.transform.rotation.2, target_rotation.2, 0.2);
                
                if progress >= 1.0 {
                    self.state = DiceAnimationState::Complete;
                    self.transform.rotation = target_rotation;
                    self.transform.position.1 = 0.0;
                }
            }
            DiceAnimationState::Complete => {}
        }
        
        true
    }
    
    fn value(&self) -> AnimationValue {
        AnimationValue::Transform(self.transform.clone())
    }
    
    fn is_complete(&self) -> bool {
        self.state == DiceAnimationState::Complete
    }
    
    fn reset(&mut self) {
        self.current_frame = 0;
        self.state = DiceAnimationState::Rolling;
        self.transform = Transform {
            position: (0.0, 0.0, 0.0),
            rotation: (0.0, 0.0, 0.0),
            scale: (1.0, 1.0, 1.0),
        };
    }
}

impl DiceAnimation {
    fn get_target_rotation(&self) -> (f32, f32, f32) {
        // Calculate rotation to show the correct dice faces
        let die1_rotation = match self.dice_values.0 {
            1 => (0.0, 0.0, 0.0),
            2 => (90.0, 0.0, 0.0),
            3 => (0.0, 90.0, 0.0),
            4 => (0.0, -90.0, 0.0),
            5 => (-90.0, 0.0, 0.0),
            6 => (180.0, 0.0, 0.0),
            _ => (0.0, 0.0, 0.0),
        };
        
        let die2_rotation = match self.dice_values.1 {
            1 => (0.0, 0.0, 0.0),
            2 => (90.0, 0.0, 0.0),
            3 => (0.0, 90.0, 0.0),
            4 => (0.0, -90.0, 0.0),
            5 => (-90.0, 0.0, 0.0),
            6 => (180.0, 0.0, 0.0),
            _ => (0.0, 0.0, 0.0),
        };
        
        // Average the rotations for display
        (
            (die1_rotation.0 + die2_rotation.0) / 2.0,
            (die1_rotation.1 + die2_rotation.1) / 2.0,
            (die1_rotation.2 + die2_rotation.2) / 2.0,
        )
    }
}

/// Fade transition animation
pub struct FadeAnimation {
    start_opacity: f32,
    end_opacity: f32,
    current_opacity: f32,
    duration: Duration,
    elapsed: Duration,
}

impl FadeAnimation {
    pub fn fade_in(duration: Duration) -> Self {
        Self {
            start_opacity: 0.0,
            end_opacity: 1.0,
            current_opacity: 0.0,
            duration,
            elapsed: Duration::ZERO,
        }
    }
    
    pub fn fade_out(duration: Duration) -> Self {
        Self {
            start_opacity: 1.0,
            end_opacity: 0.0,
            current_opacity: 1.0,
            duration,
            elapsed: Duration::ZERO,
        }
    }
}

impl Animation for FadeAnimation {
    fn update(&mut self, delta_time: Duration) -> bool {
        if self.elapsed >= self.duration {
            return false;
        }
        
        self.elapsed += delta_time;
        let progress = self.elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let eased_progress = ease_in_out_cubic(progress.min(1.0));
        
        self.current_opacity = lerp(self.start_opacity, self.end_opacity, eased_progress);
        
        true
    }
    
    fn value(&self) -> AnimationValue {
        AnimationValue::Float(self.current_opacity)
    }
    
    fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }
    
    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        self.current_opacity = self.start_opacity;
    }
}

/// Slide transition animation
pub struct SlideAnimation {
    start_position: (f32, f32),
    end_position: (f32, f32),
    current_position: (f32, f32),
    duration: Duration,
    elapsed: Duration,
    easing: EasingFunction,
}

impl SlideAnimation {
    pub fn new(start: (f32, f32), end: (f32, f32), duration: Duration) -> Self {
        Self {
            start_position: start,
            end_position: end,
            current_position: start,
            duration,
            elapsed: Duration::ZERO,
            easing: EasingFunction::EaseInOutCubic,
        }
    }
    
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }
}

impl Animation for SlideAnimation {
    fn update(&mut self, delta_time: Duration) -> bool {
        if self.elapsed >= self.duration {
            return false;
        }
        
        self.elapsed += delta_time;
        let progress = self.elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let eased_progress = apply_easing(progress.min(1.0), self.easing);
        
        self.current_position.0 = lerp(self.start_position.0, self.end_position.0, eased_progress);
        self.current_position.1 = lerp(self.start_position.1, self.end_position.1, eased_progress);
        
        true
    }
    
    fn value(&self) -> AnimationValue {
        AnimationValue::Vector2(self.current_position.0, self.current_position.1)
    }
    
    fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }
    
    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        self.current_position = self.start_position;
    }
}

/// Spring animation for bouncy effects
pub struct SpringAnimation {
    target: f32,
    current: f32,
    velocity: f32,
    stiffness: f32,
    damping: f32,
    mass: f32,
}

impl SpringAnimation {
    pub fn new(target: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            target,
            current: 0.0,
            velocity: 0.0,
            stiffness,
            damping,
            mass: 1.0,
        }
    }
}

impl Animation for SpringAnimation {
    fn update(&mut self, delta_time: Duration) -> bool {
        let dt = delta_time.as_secs_f32();
        
        // Spring physics simulation
        let spring_force = -self.stiffness * (self.current - self.target);
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;
        
        self.velocity += acceleration * dt;
        self.current += self.velocity * dt;
        
        // Check if animation is effectively complete
        let is_at_rest = self.velocity.abs() < 0.01 && (self.current - self.target).abs() < 0.01;
        
        !is_at_rest
    }
    
    fn value(&self) -> AnimationValue {
        AnimationValue::Float(self.current)
    }
    
    fn is_complete(&self) -> bool {
        self.velocity.abs() < 0.01 && (self.current - self.target).abs() < 0.01
    }
    
    fn reset(&mut self) {
        self.current = 0.0;
        self.velocity = 0.0;
    }
}

/// Easing functions for smooth animations
#[derive(Debug, Clone, Copy)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInElastic,
    EaseOutElastic,
    EaseInBounce,
    EaseOutBounce,
}

/// Apply easing function to progress value
fn apply_easing(t: f32, easing: EasingFunction) -> f32 {
    match easing {
        EasingFunction::Linear => t,
        EasingFunction::EaseIn => t * t,
        EasingFunction::EaseOut => t * (2.0 - t),
        EasingFunction::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        EasingFunction::EaseInCubic => t * t * t,
        EasingFunction::EaseOutCubic => {
            let t = t - 1.0;
            t * t * t + 1.0
        }
        EasingFunction::EaseInOutCubic => ease_in_out_cubic(t),
        EasingFunction::EaseInElastic => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                let p = 0.3;
                let s = p / 4.0;
                let t = t - 1.0;
                -(2.0_f32.powf(10.0 * t) * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin())
            }
        }
        EasingFunction::EaseOutElastic => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                let p = 0.3;
                let s = p / 4.0;
                2.0_f32.powf(-10.0 * t) * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin() + 1.0
            }
        }
        EasingFunction::EaseInBounce => 1.0 - apply_easing(1.0 - t, EasingFunction::EaseOutBounce),
        EasingFunction::EaseOutBounce => {
            if t < 1.0 / 2.75 {
                7.5625 * t * t
            } else if t < 2.0 / 2.75 {
                let t = t - 1.5 / 2.75;
                7.5625 * t * t + 0.75
            } else if t < 2.5 / 2.75 {
                let t = t - 2.25 / 2.75;
                7.5625 * t * t + 0.9375
            } else {
                let t = t - 2.625 / 2.75;
                7.5625 * t * t + 0.984375
            }
        }
    }
}

/// Cubic ease in-out function
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        1.0 + t * t * t / 2.0
    }
}

/// Linear interpolation
fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t
}

impl AnimationController {
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
            frame_rate: 60,
            last_update: Instant::now(),
        }
    }
    
    /// Add an animation to the controller
    pub fn add_animation(&mut self, animation: Box<dyn Animation>) {
        self.animations.push(animation);
    }
    
    /// Update all animations
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta_time = now - self.last_update;
        self.last_update = now;
        
        // Update all animations and remove completed ones
        self.animations.retain_mut(|animation| {
            animation.update(delta_time)
        });
    }
    
    /// Get the number of active animations
    pub fn active_count(&self) -> usize {
        self.animations.len()
    }
    
    /// Clear all animations
    pub fn clear(&mut self) {
        self.animations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dice_animation() {
        let mut animation = DiceAnimation::new((3, 5));
        assert!(!animation.is_complete());
        
        // Simulate animation updates
        for _ in 0..100 {
            animation.update(Duration::from_millis(16)); // ~60fps
        }
        
        assert!(animation.is_complete());
    }
    
    #[test]
    fn test_fade_animation() {
        let mut fade = FadeAnimation::fade_in(Duration::from_secs(1));
        
        // Start at 0 opacity
        if let AnimationValue::Float(opacity) = fade.value() {
            assert_eq!(opacity, 0.0);
        }
        
        // Update for half duration
        fade.update(Duration::from_millis(500));
        
        // Should be partially faded in
        if let AnimationValue::Float(opacity) = fade.value() {
            assert!(opacity > 0.0 && opacity < 1.0);
        }
        
        // Complete the animation
        fade.update(Duration::from_millis(600));
        assert!(fade.is_complete());
    }
    
    #[test]
    fn test_spring_animation() {
        let mut spring = SpringAnimation::new(100.0, 10.0, 0.5);
        
        // Run animation
        for _ in 0..1000 {
            if !spring.update(Duration::from_millis(16)) {
                break;
            }
        }
        
        // Should settle near target
        if let AnimationValue::Float(value) = spring.value() {
            assert!((value - 100.0).abs() < 1.0);
        }
    }
}