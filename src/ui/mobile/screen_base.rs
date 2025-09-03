//! Base screen traits and rendering context
//!
//! Provides the foundation for mobile screen implementations

use std::time::Duration;

/// Base trait for all screens
pub trait Screen {
    /// Render the screen
    fn render(&self, ctx: &mut RenderContext);

    /// Handle touch input
    fn handle_touch(&mut self, event: TouchEvent) -> Option<ScreenTransition>;

    /// Update screen state
    fn update(&mut self, delta_time: Duration);
}

/// Screen transition types
#[derive(Debug, Clone)]
pub enum ScreenTransition {
    Push(ScreenType),
    Pop,
    Replace(ScreenType),
    None,
}

/// Screen types in the app
#[derive(Debug, Clone)]
pub enum ScreenType {
    Home,
    Game(String), // game_id
    Wallet,
    Discovery,
    Settings,
    Profile,
}

/// Touch event types
#[derive(Debug, Clone)]
pub enum TouchEvent {
    Tap { x: f32, y: f32 },
    DoubleTap { x: f32, y: f32 },
    LongPress { x: f32, y: f32 },
    Swipe { start_x: f32, start_y: f32, end_x: f32, end_y: f32, velocity: f32 },
    Pinch { scale: f32, center_x: f32, center_y: f32 },
    Pan { x: f32, y: f32, delta_x: f32, delta_y: f32 },
}

/// Rendering context for drawing
pub struct RenderContext {
    width: f32,
    height: f32,
    scale_factor: f32,
    theme: Theme,
    // Platform-specific renderer would be here
}

impl RenderContext {
    /// Create new render context
    pub fn new(width: f32, height: f32, scale_factor: f32, theme: Theme) -> Self {
        Self {
            width,
            height,
            scale_factor,
            theme,
        }
    }

    /// Fill a rectangle
    pub fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (u8, u8, u8, u8)) {
        // Platform-specific implementation
    }

    /// Draw a rectangle outline
    pub fn draw_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (u8, u8, u8, u8), line_width: f32) {
        // Platform-specific implementation
    }

    /// Fill a circle
    pub fn fill_circle(&mut self, x: f32, y: f32, radius: f32, color: (u8, u8, u8, u8)) {
        // Platform-specific implementation
    }

    /// Draw a circle outline
    pub fn draw_circle(&mut self, x: f32, y: f32, radius: f32, color: (u8, u8, u8, u8), line_width: f32) {
        // Platform-specific implementation
    }

    /// Draw text
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, size: f32, color: (u8, u8, u8, u8)) {
        // Platform-specific implementation
    }

    /// Draw a line
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: (u8, u8, u8, u8), line_width: f32) {
        // Platform-specific implementation
    }

    /// Fill with gradient
    pub fn fill_gradient(&mut self, x: f32, y: f32, width: f32, height: f32,
                         start_color: (u8, u8, u8, u8), end_color: (u8, u8, u8, u8)) {
        // Platform-specific implementation
    }

    /// Draw an image
    pub fn draw_image(&mut self, image_id: &str, x: f32, y: f32, width: f32, height: f32) {
        // Platform-specific implementation
    }

    /// Push a clipping rectangle
    pub fn push_clip(&mut self, x: f32, y: f32, width: f32, height: f32) {
        // Platform-specific implementation
    }

    /// Pop the clipping rectangle
    pub fn pop_clip(&mut self) {
        // Platform-specific implementation
    }

    /// Get screen dimensions
    pub fn dimensions(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// Get theme
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

/// Theme configuration
#[derive(Clone)]
pub struct Theme {
    pub primary: (u8, u8, u8, u8),
    pub secondary: (u8, u8, u8, u8),
    pub background: (u8, u8, u8, u8),
    pub surface: (u8, u8, u8, u8),
    pub text_primary: (u8, u8, u8, u8),
    pub text_secondary: (u8, u8, u8, u8),
    pub success: (u8, u8, u8, u8),
    pub warning: (u8, u8, u8, u8),
    pub error: (u8, u8, u8, u8),
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: (102, 126, 234, 255),
            secondary: (118, 75, 162, 255),
            background: (15, 15, 15, 255),
            surface: (26, 26, 26, 255),
            text_primary: (255, 255, 255, 255),
            text_secondary: (180, 180, 180, 255),
            success: (76, 175, 80, 255),
            warning: (255, 152, 0, 255),
            error: (244, 67, 54, 255),
        }
    }
}

/// Screen element for UI components
pub struct ScreenElement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub visible: bool,
}

impl ScreenElement {
    /// Check if point is inside element
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.visible &&
        x >= self.x && x <= self.x + self.width &&
        y >= self.y && y <= self.y + self.height
    }

    /// Get center point
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}