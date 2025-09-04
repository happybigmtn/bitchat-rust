//! Theme system for mobile UI

use super::*;
use std::collections::HashMap;
use chrono::Timelike;

/// Theme manager for handling multiple themes
pub struct ThemeManager {
    themes: HashMap<String, Theme>,
    current_theme: String,
    system_theme: Option<String>,
}

impl ThemeManager {
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        // Add default themes
        themes.insert("light".to_string(), Theme::light());
        themes.insert("dark".to_string(), Theme::dark());
        themes.insert("casino".to_string(), Theme::casino());

        Self {
            themes,
            current_theme: "dark".to_string(),
            system_theme: None,
        }
    }

    /// Get the current theme
    pub fn current(&self) -> &Theme {
        self.themes
            .get(&self.current_theme)
            .unwrap_or(&Theme::default())
    }

    /// Set the current theme
    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            true
        } else {
            false
        }
    }

    /// Add a custom theme
    pub fn add_theme(&mut self, name: String, theme: Theme) {
        self.themes.insert(name, theme);
    }

    /// Get a theme by name
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Apply system theme preference
    pub fn apply_system_theme(&mut self, is_dark: bool) {
        let theme_name = if is_dark { "dark" } else { "light" };
        self.system_theme = Some(theme_name.to_string());
        self.current_theme = theme_name.to_string();
    }

    /// List available themes
    pub fn available_themes(&self) -> Vec<String> {
        self.themes.keys().cloned().collect()
    }
}

impl Theme {
    /// Light theme preset
    pub fn light() -> Self {
        Self {
            primary_color: Color { r: 33, g: 150, b: 243, a: 255 }, // Blue
            secondary_color: Color { r: 76, g: 175, b: 80, a: 255 }, // Green
            background_color: Color { r: 255, g: 255, b: 255, a: 255 }, // White
            surface_color: Color { r: 245, g: 245, b: 245, a: 255 }, // Light gray
            error_color: Color { r: 244, g: 67, b: 54, a: 255 }, // Red
            text_color: Color { r: 33, g: 33, b: 33, a: 255 }, // Dark gray
            font_family: "System".to_string(),
            font_sizes: FontSizes::default(),
            spacing: Spacing::default(),
            border_radius: BorderRadius::default(),
        }
    }

    /// Dark theme preset
    pub fn dark() -> Self {
        Self::default() // Already defined as dark in mod.rs
    }

    /// Casino theme preset
    pub fn casino() -> Self {
        Self {
            primary_color: Color { r: 212, g: 175, b: 55, a: 255 }, // Gold
            secondary_color: Color { r: 139, g: 0, b: 0, a: 255 }, // Dark red
            background_color: Color { r: 0, g: 100, b: 0, a: 255 }, // Dark green (felt)
            surface_color: Color { r: 0, g: 120, b: 0, a: 255 }, // Lighter green
            error_color: Color { r: 220, g: 20, b: 60, a: 255 }, // Crimson
            text_color: Color { r: 255, g: 255, b: 255, a: 255 }, // White
            font_family: "Casino".to_string(),
            font_sizes: FontSizes {
                heading1: 36.0,
                heading2: 28.0,
                heading3: 22.0,
                body: 16.0,
                caption: 12.0,
                button: 16.0,
            },
            spacing: Spacing::default(),
            border_radius: BorderRadius {
                sm: 2.0,
                md: 4.0,
                lg: 8.0,
                full: 999.0,
            },
        }
    }

    /// Create a custom theme builder
    pub fn builder() -> ThemeBuilder {
        ThemeBuilder::new()
    }

    /// Get color for specific UI element
    pub fn color_for_element(&self, element: UIElement) -> &Color {
        match element {
            UIElement::Button => &self.primary_color,
            UIElement::ButtonSecondary => &self.secondary_color,
            UIElement::Background => &self.background_color,
            UIElement::Surface => &self.surface_color,
            UIElement::Error => &self.error_color,
            UIElement::Text => &self.text_color,
            UIElement::TextSecondary => &self.text_color, // Could be dimmed
            UIElement::Border => &self.surface_color,
            UIElement::Success => &self.secondary_color,
            UIElement::Warning => &Color { r: 255, g: 152, b: 0, a: 255 },
        }
    }

    /// Apply color adjustments
    pub fn with_adjustments(&self, adjustments: ColorAdjustments) -> Self {
        let mut theme = self.clone();

        if let Some(brightness) = adjustments.brightness {
            theme.primary_color = theme.primary_color.adjust_brightness(brightness);
            theme.secondary_color = theme.secondary_color.adjust_brightness(brightness);
            theme.background_color = theme.background_color.adjust_brightness(brightness);
            theme.surface_color = theme.surface_color.adjust_brightness(brightness);
            theme.text_color = theme.text_color.adjust_brightness(brightness);
        }

        if let Some(contrast) = adjustments.contrast {
            theme.primary_color = theme.primary_color.adjust_contrast(contrast);
            theme.secondary_color = theme.secondary_color.adjust_contrast(contrast);
            theme.error_color = theme.error_color.adjust_contrast(contrast);
        }

        theme
    }
}

impl Color {
    /// Adjust brightness
    pub fn adjust_brightness(&self, factor: f32) -> Self {
        let factor = factor.clamp(0.0, 2.0);
        Self {
            r: (self.r as f32 * factor).min(255.0) as u8,
            g: (self.g as f32 * factor).min(255.0) as u8,
            b: (self.b as f32 * factor).min(255.0) as u8,
            a: self.a,
        }
    }

    /// Adjust contrast
    pub fn adjust_contrast(&self, factor: f32) -> Self {
        let factor = factor.clamp(0.0, 2.0);
        let gray = 128.0;

        Self {
            r: ((self.r as f32 - gray) * factor + gray).clamp(0.0, 255.0) as u8,
            g: ((self.g as f32 - gray) * factor + gray).clamp(0.0, 255.0) as u8,
            b: ((self.b as f32 - gray) * factor + gray).clamp(0.0, 255.0) as u8,
            a: self.a,
        }
    }

    /// Mix with another color
    pub fn mix(&self, other: &Color, ratio: f32) -> Self {
        let ratio = ratio.clamp(0.0, 1.0);
        let inv_ratio = 1.0 - ratio;

        Self {
            r: (self.r as f32 * inv_ratio + other.r as f32 * ratio) as u8,
            g: (self.g as f32 * inv_ratio + other.g as f32 * ratio) as u8,
            b: (self.b as f32 * inv_ratio + other.b as f32 * ratio) as u8,
            a: (self.a as f32 * inv_ratio + other.a as f32 * ratio) as u8,
        }
    }

    /// Get luminance value
    pub fn luminance(&self) -> f32 {
        // Using relative luminance formula
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;

        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Check if color is dark
    pub fn is_dark(&self) -> bool {
        self.luminance() < 0.5
    }

    /// Get contrasting text color
    pub fn contrasting_text_color(&self) -> Self {
        if self.is_dark() {
            Color { r: 255, g: 255, b: 255, a: 255 } // White
        } else {
            Color { r: 0, g: 0, b: 0, a: 255 } // Black
        }
    }

    /// Apply alpha
    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: alpha,
        }
    }
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            heading1: 32.0,
            heading2: 24.0,
            heading3: 20.0,
            body: 16.0,
            caption: 12.0,
            button: 14.0,
        }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 16.0,
            lg: 24.0,
            xl: 32.0,
        }
    }
}

impl Default for BorderRadius {
    fn default() -> Self {
        Self {
            sm: 4.0,
            md: 8.0,
            lg: 16.0,
            full: 999.0,
        }
    }
}

/// Theme builder for creating custom themes
pub struct ThemeBuilder {
    theme: Theme,
}

impl ThemeBuilder {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
        }
    }

    pub fn primary_color(mut self, color: Color) -> Self {
        self.theme.primary_color = color;
        self
    }

    pub fn secondary_color(mut self, color: Color) -> Self {
        self.theme.secondary_color = color;
        self
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.theme.background_color = color;
        self
    }

    pub fn surface_color(mut self, color: Color) -> Self {
        self.theme.surface_color = color;
        self
    }

    pub fn error_color(mut self, color: Color) -> Self {
        self.theme.error_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.theme.text_color = color;
        self
    }

    pub fn font_family(mut self, font: String) -> Self {
        self.theme.font_family = font;
        self
    }

    pub fn font_sizes(mut self, sizes: FontSizes) -> Self {
        self.theme.font_sizes = sizes;
        self
    }

    pub fn spacing(mut self, spacing: Spacing) -> Self {
        self.theme.spacing = spacing;
        self
    }

    pub fn border_radius(mut self, radius: BorderRadius) -> Self {
        self.theme.border_radius = radius;
        self
    }

    pub fn build(self) -> Theme {
        self.theme
    }
}

/// UI element types for theming
#[derive(Debug, Clone)]
pub enum UIElement {
    Button,
    ButtonSecondary,
    Background,
    Surface,
    Error,
    Text,
    TextSecondary,
    Border,
    Success,
    Warning,
}

/// Color adjustments
#[derive(Debug, Clone)]
pub struct ColorAdjustments {
    pub brightness: Option<f32>,
    pub contrast: Option<f32>,
    pub saturation: Option<f32>,
}

impl ColorAdjustments {
    pub fn new() -> Self {
        Self {
            brightness: None,
            contrast: None,
            saturation: None,
        }
    }

    pub fn brightness(mut self, value: f32) -> Self {
        self.brightness = Some(value);
        self
    }

    pub fn contrast(mut self, value: f32) -> Self {
        self.contrast = Some(value);
        self
    }

    pub fn saturation(mut self, value: f32) -> Self {
        self.saturation = Some(value);
        self
    }
}

/// Theme variant for automatic theme switching
#[derive(Debug, Clone)]
pub enum ThemeVariant {
    Light,
    Dark,
    Auto,
    Custom(String),
}

/// Semantic colors for better theme consistency
pub struct SemanticColors {
    pub primary: Color,
    pub primary_variant: Color,
    pub secondary: Color,
    pub secondary_variant: Color,
    pub background: Color,
    pub surface: Color,
    pub error: Color,
    pub on_primary: Color,
    pub on_secondary: Color,
    pub on_background: Color,
    pub on_surface: Color,
    pub on_error: Color,
}

impl SemanticColors {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            primary: theme.primary_color.clone(),
            primary_variant: theme.primary_color.adjust_brightness(0.8),
            secondary: theme.secondary_color.clone(),
            secondary_variant: theme.secondary_color.adjust_brightness(0.8),
            background: theme.background_color.clone(),
            surface: theme.surface_color.clone(),
            error: theme.error_color.clone(),
            on_primary: theme.primary_color.contrasting_text_color(),
            on_secondary: theme.secondary_color.contrasting_text_color(),
            on_background: theme.background_color.contrasting_text_color(),
            on_surface: theme.surface_color.contrasting_text_color(),
            on_error: theme.error_color.contrasting_text_color(),
        }
    }
}

/// Dynamic theme that changes based on time or conditions
pub struct DynamicTheme {
    day_theme: Theme,
    night_theme: Theme,
    transition_hour_morning: u8,
    transition_hour_evening: u8,
}

impl DynamicTheme {
    pub fn new(day_theme: Theme, night_theme: Theme) -> Self {
        Self {
            day_theme,
            night_theme,
            transition_hour_morning: 6,
            transition_hour_evening: 18,
        }
    }

    pub fn current_theme(&self) -> Theme {
        let now = chrono::Local::now();
        let hour = now.hour() as u8;

        if hour >= self.transition_hour_morning && hour < self.transition_hour_evening {
            self.day_theme.clone()
        } else {
            self.night_theme.clone()
        }
    }

    pub fn set_transition_times(&mut self, morning: u8, evening: u8) {
        self.transition_hour_morning = morning.min(23);
        self.transition_hour_evening = evening.min(23);
    }
}

// Clone implementation for Theme (since it contains non-Clone fields)
impl Clone for Theme {
    fn clone(&self) -> Self {
        Self {
            primary_color: self.primary_color.clone(),
            secondary_color: self.secondary_color.clone(),
            background_color: self.background_color.clone(),
            surface_color: self.surface_color.clone(),
            error_color: self.error_color.clone(),
            text_color: self.text_color.clone(),
            font_family: self.font_family.clone(),
            font_sizes: FontSizes {
                heading1: self.font_sizes.heading1,
                heading2: self.font_sizes.heading2,
                heading3: self.font_sizes.heading3,
                body: self.font_sizes.body,
                caption: self.font_sizes.caption,
                button: self.font_sizes.button,
            },
            spacing: Spacing {
                xs: self.spacing.xs,
                sm: self.spacing.sm,
                md: self.spacing.md,
                lg: self.spacing.lg,
                xl: self.spacing.xl,
            },
            border_radius: BorderRadius {
                sm: self.border_radius.sm,
                md: self.border_radius.md,
                lg: self.border_radius.lg,
                full: self.border_radius.full,
            },
        }
    }
}