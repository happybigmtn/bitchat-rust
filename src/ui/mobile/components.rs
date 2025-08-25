//! Reusable UI components for mobile platforms

use super::*;
use std::sync::Arc;

/// Base component trait for all UI elements
pub trait Component {
    fn render(&self, theme: &Theme) -> ComponentView;
    fn handle_event(&mut self, event: Event) -> EventResult;
}

/// Button component
pub struct Button {
    pub text: String,
    pub style: ButtonStyle,
    pub enabled: bool,
    pub on_click: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Button {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: ButtonStyle::Primary,
            enabled: true,
            on_click: None,
        }
    }

    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(handler));
        self
    }
}

impl Component for Button {
    fn render(&self, theme: &Theme) -> ComponentView {
        let background_color = match self.style {
            ButtonStyle::Primary => &theme.primary_color,
            ButtonStyle::Secondary => &theme.secondary_color,
            ButtonStyle::Danger => &theme.error_color,
            ButtonStyle::Text => &Color { r: 0, g: 0, b: 0, a: 0 },
        };

        ComponentView::Button(ButtonView {
            text: self.text.clone(),
            background_color: background_color.clone(),
            text_color: theme.text_color.clone(),
            enabled: self.enabled,
            font_size: theme.font_sizes.button,
            padding: theme.spacing.md,
            border_radius: theme.border_radius.md,
        })
    }

    fn handle_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Click if self.enabled => {
                if let Some(handler) = &self.on_click {
                    handler();
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

/// Text input component
pub struct TextInput {
    pub value: String,
    pub placeholder: Option<String>,
    pub secure: bool,
    pub enabled: bool,
    pub on_change: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: None,
            secure: false,
            enabled: true,
            on_change: None,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl Component for TextInput {
    fn render(&self, theme: &Theme) -> ComponentView {
        ComponentView::TextInput(TextInputView {
            value: if self.secure {
                "*".repeat(self.value.len())
            } else {
                self.value.clone()
            },
            placeholder: self.placeholder.clone(),
            background_color: theme.surface_color.clone(),
            text_color: theme.text_color.clone(),
            enabled: self.enabled,
            font_size: theme.font_sizes.body,
            padding: theme.spacing.sm,
            border_radius: theme.border_radius.sm,
        })
    }

    fn handle_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::TextChange(text) if self.enabled => {
                self.value = text.clone();
                if let Some(handler) = &self.on_change {
                    handler(text);
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

/// Card component for grouped content
pub struct Card {
    pub title: Option<String>,
    pub children: Vec<Box<dyn Component>>,
    pub padding: f32,
}

impl Card {
    pub fn new() -> Self {
        Self {
            title: None,
            children: Vec::new(),
            padding: 16.0,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn add_child<C: Component + 'static>(mut self, child: C) -> Self {
        self.children.push(Box::new(child));
        self
    }
}

impl Component for Card {
    fn render(&self, theme: &Theme) -> ComponentView {
        let mut children_views = Vec::new();
        
        if let Some(title) = &self.title {
            children_views.push(ComponentView::Text(TextView {
                text: title.clone(),
                color: theme.text_color.clone(),
                font_size: theme.font_sizes.heading3,
                font_weight: FontWeight::Bold,
            }));
        }

        for child in &self.children {
            children_views.push(child.render(theme));
        }

        ComponentView::Container(ContainerView {
            children: children_views,
            background_color: theme.surface_color.clone(),
            padding: self.padding,
            border_radius: theme.border_radius.md,
            spacing: theme.spacing.sm,
        })
    }

    fn handle_event(&mut self, event: Event) -> EventResult {
        for child in &mut self.children {
            if let EventResult::Handled = child.handle_event(event.clone()) {
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }
}

/// List component for displaying items
pub struct List<T: Clone> {
    pub items: Vec<T>,
    pub render_item: Arc<dyn Fn(&T, &Theme) -> ComponentView + Send + Sync>,
    pub on_select: Option<Arc<dyn Fn(usize, &T) + Send + Sync>>,
}

impl<T: Clone> List<T> {
    pub fn new<F>(items: Vec<T>, render_item: F) -> Self
    where
        F: Fn(&T, &Theme) -> ComponentView + Send + Sync + 'static,
    {
        Self {
            items,
            render_item: Arc::new(render_item),
            on_select: None,
        }
    }

    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(usize, &T) + Send + Sync + 'static,
    {
        self.on_select = Some(Arc::new(handler));
        self
    }
}

impl<T: Clone + 'static> Component for List<T> {
    fn render(&self, theme: &Theme) -> ComponentView {
        let item_views: Vec<ComponentView> = self.items
            .iter()
            .map(|item| (self.render_item)(item, theme))
            .collect();

        ComponentView::ScrollView(ScrollViewView {
            children: item_views,
            direction: ScrollDirection::Vertical,
            spacing: theme.spacing.xs,
        })
    }

    fn handle_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::ItemSelect(index) => {
                if let Some(item) = self.items.get(index) {
                    if let Some(handler) = &self.on_select {
                        handler(index, item);
                    }
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }
            _ => EventResult::Ignored,
        }
    }
}

/// Toggle switch component
pub struct Toggle {
    pub value: bool,
    pub label: Option<String>,
    pub enabled: bool,
    pub on_change: Option<Arc<dyn Fn(bool) + Send + Sync>>,
}

impl Toggle {
    pub fn new(value: bool) -> Self {
        Self {
            value,
            label: None,
            enabled: true,
            on_change: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl Component for Toggle {
    fn render(&self, theme: &Theme) -> ComponentView {
        let toggle_view = ComponentView::Toggle(ToggleView {
            value: self.value,
            enabled: self.enabled,
            on_color: theme.primary_color.clone(),
            off_color: theme.surface_color.clone(),
        });

        if let Some(label) = &self.label {
            ComponentView::Row(RowView {
                children: vec![
                    ComponentView::Text(TextView {
                        text: label.clone(),
                        color: theme.text_color.clone(),
                        font_size: theme.font_sizes.body,
                        font_weight: FontWeight::Regular,
                    }),
                    toggle_view,
                ],
                spacing: theme.spacing.sm,
                alignment: Alignment::Center,
            })
        } else {
            toggle_view
        }
    }

    fn handle_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Toggle if self.enabled => {
                self.value = !self.value;
                if let Some(handler) = &self.on_change {
                    handler(self.value);
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

/// Progress indicator component
pub struct ProgressIndicator {
    pub progress: f32,
    pub style: ProgressStyle,
    pub show_percentage: bool,
}

impl ProgressIndicator {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            style: ProgressStyle::Linear,
            show_percentage: true,
        }
    }

    pub fn circular(mut self) -> Self {
        self.style = ProgressStyle::Circular;
        self
    }
}

impl Component for ProgressIndicator {
    fn render(&self, theme: &Theme) -> ComponentView {
        ComponentView::Progress(ProgressView {
            progress: self.progress,
            style: self.style.clone(),
            color: theme.primary_color.clone(),
            background_color: theme.surface_color.clone(),
            show_percentage: self.show_percentage,
        })
    }

    fn handle_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }
}

/// Image component
pub struct Image {
    pub source: ImageSource,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub content_mode: ContentMode,
}

impl Image {
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            width: None,
            height: None,
            content_mode: ContentMode::AspectFit,
        }
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
}

impl Component for Image {
    fn render(&self, _theme: &Theme) -> ComponentView {
        ComponentView::Image(ImageView {
            source: self.source.clone(),
            width: self.width,
            height: self.height,
            content_mode: self.content_mode.clone(),
        })
    }

    fn handle_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }
}

/// Badge component for notifications
pub struct Badge {
    pub count: u32,
    pub max_display: u32,
}

impl Badge {
    pub fn new(count: u32) -> Self {
        Self {
            count,
            max_display: 99,
        }
    }
}

impl Component for Badge {
    fn render(&self, theme: &Theme) -> ComponentView {
        let text = if self.count > self.max_display {
            format!("{}+", self.max_display)
        } else {
            self.count.to_string()
        };

        ComponentView::Badge(BadgeView {
            text,
            background_color: theme.error_color.clone(),
            text_color: Color { r: 255, g: 255, b: 255, a: 255 },
            font_size: theme.font_sizes.caption,
        })
    }

    fn handle_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }
}

// View structures for rendering
#[derive(Debug, Clone)]
pub enum ComponentView {
    Button(ButtonView),
    TextInput(TextInputView),
    Text(TextView),
    Container(ContainerView),
    Row(RowView),
    Column(ColumnView),
    ScrollView(ScrollViewView),
    Toggle(ToggleView),
    Progress(ProgressView),
    Image(ImageView),
    Badge(BadgeView),
    Spacer(SpacerView),
}

#[derive(Debug, Clone)]
pub struct ButtonView {
    pub text: String,
    pub background_color: Color,
    pub text_color: Color,
    pub enabled: bool,
    pub font_size: f32,
    pub padding: f32,
    pub border_radius: f32,
}

#[derive(Debug, Clone)]
pub struct TextInputView {
    pub value: String,
    pub placeholder: Option<String>,
    pub background_color: Color,
    pub text_color: Color,
    pub enabled: bool,
    pub font_size: f32,
    pub padding: f32,
    pub border_radius: f32,
}

#[derive(Debug, Clone)]
pub struct TextView {
    pub text: String,
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
}

#[derive(Debug, Clone)]
pub struct ContainerView {
    pub children: Vec<ComponentView>,
    pub background_color: Color,
    pub padding: f32,
    pub border_radius: f32,
    pub spacing: f32,
}

#[derive(Debug, Clone)]
pub struct RowView {
    pub children: Vec<ComponentView>,
    pub spacing: f32,
    pub alignment: Alignment,
}

#[derive(Debug, Clone)]
pub struct ColumnView {
    pub children: Vec<ComponentView>,
    pub spacing: f32,
    pub alignment: Alignment,
}

#[derive(Debug, Clone)]
pub struct ScrollViewView {
    pub children: Vec<ComponentView>,
    pub direction: ScrollDirection,
    pub spacing: f32,
}

#[derive(Debug, Clone)]
pub struct ToggleView {
    pub value: bool,
    pub enabled: bool,
    pub on_color: Color,
    pub off_color: Color,
}

#[derive(Debug, Clone)]
pub struct ProgressView {
    pub progress: f32,
    pub style: ProgressStyle,
    pub color: Color,
    pub background_color: Color,
    pub show_percentage: bool,
}

#[derive(Debug, Clone)]
pub struct ImageView {
    pub source: ImageSource,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub content_mode: ContentMode,
}

#[derive(Debug, Clone)]
pub struct BadgeView {
    pub text: String,
    pub background_color: Color,
    pub text_color: Color,
    pub font_size: f32,
}

#[derive(Debug, Clone)]
pub struct SpacerView {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

// Supporting types
#[derive(Debug, Clone)]
pub enum FontWeight {
    Light,
    Regular,
    Medium,
    Bold,
}

#[derive(Debug, Clone)]
pub enum Alignment {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

#[derive(Debug, Clone)]
pub enum ScrollDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum ProgressStyle {
    Linear,
    Circular,
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Asset(String),
    Url(String),
    Base64(String),
}

#[derive(Debug, Clone)]
pub enum ContentMode {
    AspectFit,
    AspectFill,
    Fill,
    Center,
}

// Event handling
#[derive(Debug, Clone)]
pub enum Event {
    Click,
    TextChange(String),
    Toggle,
    ItemSelect(usize),
    Scroll(f32, f32),
    LongPress,
    Swipe(SwipeDirection),
}

#[derive(Debug, Clone)]
pub enum SwipeDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub enum EventResult {
    Handled,
    Ignored,
}

// Layout helpers
pub struct Stack;

impl Stack {
    pub fn horizontal(children: Vec<Box<dyn Component>>, spacing: f32) -> RowView {
        let theme = Theme::default();
        RowView {
            children: children.iter().map(|c| c.render(&theme)).collect(),
            spacing,
            alignment: Alignment::Start,
        }
    }

    pub fn vertical(children: Vec<Box<dyn Component>>, spacing: f32) -> ColumnView {
        let theme = Theme::default();
        ColumnView {
            children: children.iter().map(|c| c.render(&theme)).collect(),
            spacing,
            alignment: Alignment::Start,
        }
    }
}

// Spacer for layout
pub struct Spacer {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl Spacer {
    pub fn horizontal(width: f32) -> Self {
        Self {
            width: Some(width),
            height: None,
        }
    }

    pub fn vertical(height: f32) -> Self {
        Self {
            width: None,
            height: Some(height),
        }
    }

    pub fn flexible() -> Self {
        Self {
            width: None,
            height: None,
        }
    }
}

impl Component for Spacer {
    fn render(&self, _theme: &Theme) -> ComponentView {
        ComponentView::Spacer(SpacerView {
            width: self.width,
            height: self.height,
        })
    }

    fn handle_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }
}