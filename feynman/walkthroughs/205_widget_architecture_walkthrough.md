# Chapter 94: Widget Architecture

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


*The architect Christopher Alexander once said, "Each pattern describes a problem which occurs over and over again in our environment, and then describes the core of the solution to that problem." Widget architecture is the embodiment of this principle in user interface design—creating reusable, composable building blocks that solve common interface problems while maintaining flexibility for unique requirements.*

## The Evolution of Component-Based UI

The journey toward modern widget architecture began in the 1970s with the Xerox Alto's windowing system. Each window was essentially a widget—a self-contained unit with its own state and behavior. This concept evolved through Smalltalk's Model-View-Controller pattern, where widgets became the views that users could interact with.

The real revolution came with the introduction of component libraries like Motif and later Qt, which provided pre-built widgets that developers could compose into complex interfaces. Today's widget architectures—whether React components, Flutter widgets, or Rust's egui immediate mode GUI—all trace their lineage to these early innovations.

## Understanding Widget Architecture

At its core, a widget is a self-contained unit of user interface functionality. Think of widgets as LEGO blocks for UI—each piece has a specific shape and function, but they can be combined in countless ways to build complex structures. The power lies not in the individual pieces, but in how they compose together.

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::any::{Any, TypeId};

/// Core trait that all widgets must implement
pub trait Widget: Send + Sync {
    /// Unique identifier for this widget instance
    fn id(&self) -> WidgetId;
    
    /// Render the widget to a render target
    fn render(&self, context: &mut RenderContext) -> Result<(), WidgetError>;
    
    /// Handle input events
    fn handle_event(&mut self, event: &Event) -> EventResult;
    
    /// Update widget state
    fn update(&mut self, dt: f64) -> UpdateResult;
    
    /// Get the widget's layout requirements
    fn layout_requirements(&self) -> LayoutRequirements;
    
    /// Layout the widget within given constraints
    fn layout(&mut self, constraints: LayoutConstraints) -> Size;
    
    /// Get child widgets for tree traversal
    fn children(&self) -> Vec<&dyn Widget>;
    
    /// Get mutable child widgets
    fn children_mut(&mut self) -> Vec<&mut dyn Widget>;
}

/// Widget identifier for efficient lookups
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct WidgetId(u64);

impl WidgetId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        WidgetId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Result of event handling
pub enum EventResult {
    Consumed,
    Ignored,
    Bubble(Event),
}

/// Result of widget update
pub enum UpdateResult {
    None,
    Redraw,
    Relayout,
    StateChanged(Box<dyn Any>),
}
```

## The Widget Tree

Modern widget architectures organize widgets into a tree structure, where each widget can contain child widgets. This hierarchical organization provides natural scoping for state, events, and rendering.

```rust
/// Widget tree structure for managing widget hierarchy
pub struct WidgetTree {
    root: Box<dyn Widget>,
    widget_map: HashMap<WidgetId, *mut dyn Widget>,
    dirty_widgets: Vec<WidgetId>,
    focus_chain: Vec<WidgetId>,
    event_handlers: HashMap<EventType, Vec<WidgetId>>,
}

impl WidgetTree {
    pub fn new(root: Box<dyn Widget>) -> Self {
        let mut tree = Self {
            root,
            widget_map: HashMap::new(),
            dirty_widgets: Vec::new(),
            focus_chain: Vec::new(),
            event_handlers: HashMap::new(),
        };
        tree.rebuild_widget_map();
        tree
    }
    
    /// Rebuild the widget map for fast lookups
    fn rebuild_widget_map(&mut self) {
        self.widget_map.clear();
        self.add_to_map(&mut *self.root);
    }
    
    fn add_to_map(&mut self, widget: &mut dyn Widget) {
        let id = widget.id();
        self.widget_map.insert(id, widget as *mut dyn Widget);
        
        // Recursively add children
        for child in widget.children_mut() {
            self.add_to_map(child);
        }
    }
    
    /// Find a widget by ID
    pub fn find_widget(&self, id: WidgetId) -> Option<&dyn Widget> {
        self.widget_map.get(&id).map(|ptr| unsafe { &**ptr })
    }
    
    /// Find a mutable widget by ID
    pub fn find_widget_mut(&mut self, id: WidgetId) -> Option<&mut dyn Widget> {
        self.widget_map.get(&id).map(|ptr| unsafe { &mut **ptr })
    }
    
    /// Process an event through the widget tree
    pub fn process_event(&mut self, event: Event) -> Result<(), WidgetError> {
        // First try focused widget
        if let Some(&focused_id) = self.focus_chain.last() {
            if let Some(widget) = self.find_widget_mut(focused_id) {
                match widget.handle_event(&event) {
                    EventResult::Consumed => return Ok(()),
                    EventResult::Bubble(new_event) => {
                        return self.bubble_event(focused_id, new_event);
                    }
                    EventResult::Ignored => {}
                }
            }
        }
        
        // Then try global handlers
        if let Some(handlers) = self.event_handlers.get(&event.event_type()) {
            for &handler_id in handlers {
                if let Some(widget) = self.find_widget_mut(handler_id) {
                    if let EventResult::Consumed = widget.handle_event(&event) {
                        return Ok(());
                    }
                }
            }
        }
        
        // Finally, route to root
        self.root.handle_event(&event);
        Ok(())
    }
    
    fn bubble_event(&mut self, start_id: WidgetId, event: Event) -> Result<(), WidgetError> {
        // Find parent and bubble up
        // Implementation details for parent finding and bubbling
        Ok(())
    }
    
    /// Update all widgets
    pub fn update(&mut self, dt: f64) {
        self.update_widget(&mut *self.root, dt);
        self.process_dirty_widgets();
    }
    
    fn update_widget(&mut self, widget: &mut dyn Widget, dt: f64) {
        match widget.update(dt) {
            UpdateResult::Redraw => {
                self.dirty_widgets.push(widget.id());
            }
            UpdateResult::Relayout => {
                // Trigger relayout for this widget and its subtree
                self.dirty_widgets.push(widget.id());
            }
            UpdateResult::StateChanged(state) => {
                // Handle state changes
            }
            UpdateResult::None => {}
        }
        
        // Update children
        for child in widget.children_mut() {
            self.update_widget(child, dt);
        }
    }
    
    fn process_dirty_widgets(&mut self) {
        // Process widgets that need redrawing
        for id in self.dirty_widgets.drain(..) {
            // Mark for redraw in next render pass
        }
    }
}
```

## Layout Systems

A crucial aspect of widget architecture is the layout system—how widgets negotiate their size and position within their parent containers.

```rust
/// Layout requirements that widgets communicate to their parents
#[derive(Debug, Clone)]
pub struct LayoutRequirements {
    pub min_size: Size,
    pub preferred_size: Size,
    pub max_size: Size,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub aspect_ratio: Option<f32>,
}

/// Constraints passed down from parent to child
#[derive(Debug, Clone)]
pub struct LayoutConstraints {
    pub min_size: Size,
    pub max_size: Size,
    pub available_size: Size,
}

/// Size in 2D space
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

/// Position in 2D space
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Rectangle combining position and size
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub position: Position,
    pub size: Size,
}

/// Flexbox-style layout engine
pub struct FlexLayout {
    direction: FlexDirection,
    justify_content: JustifyContent,
    align_items: AlignItems,
    gap: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum FlexDirection {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy)]
pub enum JustifyContent {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy)]
pub enum AlignItems {
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

impl FlexLayout {
    pub fn calculate_layout(
        &self,
        container: Rect,
        children: &mut [&mut dyn Widget],
    ) -> Vec<Rect> {
        let mut positions = Vec::with_capacity(children.len());
        
        // Calculate total flex and fixed space
        let mut total_flex = 0.0;
        let mut fixed_size = 0.0;
        let main_axis_size = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => container.size.width,
            FlexDirection::Column | FlexDirection::ColumnReverse => container.size.height,
        };
        
        for child in children.iter() {
            let requirements = child.layout_requirements();
            total_flex += requirements.flex_grow;
            fixed_size += match self.direction {
                FlexDirection::Row | FlexDirection::RowReverse => requirements.min_size.width,
                FlexDirection::Column | FlexDirection::ColumnReverse => requirements.min_size.height,
            };
        }
        
        // Calculate spacing
        let total_gap = self.gap * (children.len() - 1) as f32;
        let available_space = main_axis_size - fixed_size - total_gap;
        let flex_unit = if total_flex > 0.0 {
            available_space / total_flex
        } else {
            0.0
        };
        
        // Position children
        let mut current_position = match self.justify_content {
            JustifyContent::Start => 0.0,
            JustifyContent::End => available_space,
            JustifyContent::Center => available_space / 2.0,
            _ => 0.0,
        };
        
        for child in children.iter_mut() {
            let requirements = child.layout_requirements();
            
            // Calculate child size
            let child_size = if requirements.flex_grow > 0.0 {
                requirements.min_size.width + (flex_unit * requirements.flex_grow)
            } else {
                requirements.preferred_size.width
            };
            
            // Create constraints for child
            let constraints = LayoutConstraints {
                min_size: requirements.min_size,
                max_size: requirements.max_size,
                available_size: Size {
                    width: child_size,
                    height: container.size.height,
                },
            };
            
            // Layout child
            let final_size = child.layout(constraints);
            
            // Calculate position
            let position = Position {
                x: container.position.x + current_position,
                y: container.position.y,
            };
            
            positions.push(Rect {
                position,
                size: final_size,
            });
            
            current_position += final_size.width + self.gap;
        }
        
        positions
    }
}
```

## State Management in Widgets

Widgets need sophisticated state management to handle user interactions, data updates, and reactive programming patterns.

```rust
use std::rc::Rc;
use std::cell::RefCell;

/// State container for widgets
pub trait WidgetState: Any + Send + Sync {
    /// Clone the state for immutable operations
    fn clone_state(&self) -> Box<dyn WidgetState>;
    
    /// Compare with another state
    fn equals(&self, other: &dyn WidgetState) -> bool;
}

/// Reactive state management for widgets
pub struct ReactiveState<T: Clone + PartialEq + 'static> {
    value: Arc<RwLock<T>>,
    listeners: Arc<RwLock<Vec<StateListener<T>>>>,
}

type StateListener<T> = Box<dyn Fn(&T) + Send + Sync>;

impl<T: Clone + PartialEq + 'static> ReactiveState<T> {
    pub fn new(initial: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(initial)),
            listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub fn get(&self) -> T {
        self.value.read().unwrap().clone()
    }
    
    pub fn set(&self, new_value: T) {
        let changed = {
            let mut value = self.value.write().unwrap();
            if *value != new_value {
                *value = new_value.clone();
                true
            } else {
                false
            }
        };
        
        if changed {
            self.notify_listeners(&new_value);
        }
    }
    
    pub fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut T),
    {
        let new_value = {
            let mut value = self.value.write().unwrap();
            let old = value.clone();
            updater(&mut *value);
            if *value != old {
                Some(value.clone())
            } else {
                None
            }
        };
        
        if let Some(new_value) = new_value {
            self.notify_listeners(&new_value);
        }
    }
    
    pub fn subscribe<F>(&self, listener: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.listeners.write().unwrap().push(Box::new(listener));
    }
    
    fn notify_listeners(&self, value: &T) {
        for listener in self.listeners.read().unwrap().iter() {
            listener(value);
        }
    }
}

/// Widget with reactive state
pub struct StatefulWidget<S: Clone + PartialEq + 'static> {
    id: WidgetId,
    state: ReactiveState<S>,
    render_fn: Arc<dyn Fn(&S) -> Box<dyn Widget> + Send + Sync>,
    cached_render: RefCell<Option<Box<dyn Widget>>>,
}

impl<S: Clone + PartialEq + 'static> StatefulWidget<S> {
    pub fn new<F>(initial_state: S, render_fn: F) -> Self
    where
        F: Fn(&S) -> Box<dyn Widget> + Send + Sync + 'static,
    {
        let widget = Self {
            id: WidgetId::new(),
            state: ReactiveState::new(initial_state),
            render_fn: Arc::new(render_fn),
            cached_render: RefCell::new(None),
        };
        
        // Subscribe to state changes
        let id = widget.id;
        widget.state.subscribe(move |_| {
            // Mark widget as dirty for re-render
            // This would communicate with the widget tree
        });
        
        widget
    }
    
    fn ensure_rendered(&self) {
        if self.cached_render.borrow().is_none() {
            let state = self.state.get();
            let rendered = (self.render_fn)(&state);
            *self.cached_render.borrow_mut() = Some(rendered);
        }
    }
}
```

## Custom Widget Creation

Creating custom widgets is essential for building domain-specific UI components.

```rust
/// Example: Creating a custom button widget
pub struct Button {
    id: WidgetId,
    label: String,
    position: Rect,
    state: ButtonState,
    on_click: Option<Box<dyn Fn() + Send + Sync>>,
    style: ButtonStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

pub struct ButtonStyle {
    pub background_color: Color,
    pub text_color: Color,
    pub border_radius: f32,
    pub padding: f32,
    pub font_size: f32,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: WidgetId::new(),
            label: label.into(),
            position: Rect {
                position: Position { x: 0.0, y: 0.0 },
                size: Size { width: 100.0, height: 40.0 },
            },
            state: ButtonState::Normal,
            on_click: None,
            style: ButtonStyle::default(),
        }
    }
    
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_click = Some(Box::new(handler));
        self
    }
    
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
}

impl Widget for Button {
    fn id(&self) -> WidgetId {
        self.id
    }
    
    fn render(&self, context: &mut RenderContext) -> Result<(), WidgetError> {
        // Render background
        let bg_color = match self.state {
            ButtonState::Normal => self.style.background_color,
            ButtonState::Hovered => self.style.background_color.lighten(0.1),
            ButtonState::Pressed => self.style.background_color.darken(0.1),
            ButtonState::Disabled => self.style.background_color.grayscale(),
        };
        
        context.draw_rounded_rect(
            self.position,
            self.style.border_radius,
            bg_color,
        )?;
        
        // Render text
        context.draw_text(
            &self.label,
            self.position.center(),
            self.style.text_color,
            self.style.font_size,
        )?;
        
        Ok(())
    }
    
    fn handle_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseMove { position } => {
                if self.position.contains(*position) {
                    if self.state == ButtonState::Normal {
                        self.state = ButtonState::Hovered;
                        return EventResult::Consumed;
                    }
                } else if self.state == ButtonState::Hovered {
                    self.state = ButtonState::Normal;
                    return EventResult::Consumed;
                }
            }
            Event::MouseDown { button, position } => {
                if *button == MouseButton::Left && self.position.contains(*position) {
                    self.state = ButtonState::Pressed;
                    return EventResult::Consumed;
                }
            }
            Event::MouseUp { button, position } => {
                if *button == MouseButton::Left && self.state == ButtonState::Pressed {
                    if self.position.contains(*position) {
                        if let Some(handler) = &self.on_click {
                            handler();
                        }
                    }
                    self.state = if self.position.contains(*position) {
                        ButtonState::Hovered
                    } else {
                        ButtonState::Normal
                    };
                    return EventResult::Consumed;
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }
    
    fn update(&mut self, _dt: f64) -> UpdateResult {
        UpdateResult::None
    }
    
    fn layout_requirements(&self) -> LayoutRequirements {
        // Calculate required size based on text
        let text_size = measure_text(&self.label, self.style.font_size);
        let padding = self.style.padding * 2.0;
        
        LayoutRequirements {
            min_size: Size {
                width: text_size.width + padding,
                height: text_size.height + padding,
            },
            preferred_size: Size {
                width: text_size.width + padding * 2.0,
                height: 40.0,
            },
            max_size: Size {
                width: f32::INFINITY,
                height: 60.0,
            },
            flex_grow: 0.0,
            flex_shrink: 1.0,
            aspect_ratio: None,
        }
    }
    
    fn layout(&mut self, constraints: LayoutConstraints) -> Size {
        let width = constraints.available_size.width
            .min(constraints.max_size.width)
            .max(constraints.min_size.width);
        let height = constraints.available_size.height
            .min(constraints.max_size.height)
            .max(constraints.min_size.height);
        
        self.position.size = Size { width, height };
        self.position.size
    }
    
    fn children(&self) -> Vec<&dyn Widget> {
        vec![]
    }
    
    fn children_mut(&mut self) -> Vec<&mut dyn Widget> {
        vec![]
    }
}

// Helper function for text measurement
fn measure_text(text: &str, font_size: f32) -> Size {
    // Simplified text measurement
    Size {
        width: text.len() as f32 * font_size * 0.6,
        height: font_size * 1.2,
    }
}
```

## Composition Patterns

Widget composition is where the true power of widget architecture emerges. Complex UIs are built by combining simple widgets.

```rust
/// Container widget for composing other widgets
pub struct Container {
    id: WidgetId,
    children: Vec<Box<dyn Widget>>,
    layout: Box<dyn Layout>,
    padding: f32,
    background: Option<Color>,
}

/// Trait for layout strategies
pub trait Layout: Send + Sync {
    fn calculate_layout(&self, container: Rect, children: &mut [&mut dyn Widget]) -> Vec<Rect>;
}

impl Container {
    pub fn new() -> Self {
        Self {
            id: WidgetId::new(),
            children: Vec::new(),
            layout: Box::new(FlexLayout {
                direction: FlexDirection::Column,
                justify_content: JustifyContent::Start,
                align_items: AlignItems::Stretch,
                gap: 0.0,
            }),
            padding: 0.0,
            background: None,
        }
    }
    
    pub fn add_child(mut self, widget: Box<dyn Widget>) -> Self {
        self.children.push(widget);
        self
    }
    
    pub fn layout(mut self, layout: Box<dyn Layout>) -> Self {
        self.layout = layout;
        self
    }
    
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }
    
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
}

/// Builder pattern for complex widget composition
pub struct WidgetBuilder {
    widget_stack: Vec<Box<dyn Widget>>,
}

impl WidgetBuilder {
    pub fn new() -> Self {
        Self {
            widget_stack: Vec::new(),
        }
    }
    
    pub fn container<F>(mut self, builder: F) -> Self
    where
        F: FnOnce(Container) -> Container,
    {
        let container = builder(Container::new());
        self.widget_stack.push(Box::new(container));
        self
    }
    
    pub fn button(mut self, label: &str) -> Self {
        self.widget_stack.push(Box::new(Button::new(label)));
        self
    }
    
    pub fn text(mut self, content: &str) -> Self {
        self.widget_stack.push(Box::new(Text::new(content)));
        self
    }
    
    pub fn build(self) -> Box<dyn Widget> {
        let mut root = Container::new();
        for widget in self.widget_stack {
            root = root.add_child(widget);
        }
        Box::new(root)
    }
}

/// Declarative widget construction using a macro
#[macro_export]
macro_rules! widget {
    (Container {
        $($key:ident: $value:expr),* $(,)?
        children: [$($child:tt),* $(,)?]
    }) => {
        {
            let mut container = Container::new();
            $(container = container.$key($value);)*
            $(container = container.add_child(widget!($child));)*
            Box::new(container)
        }
    };
    
    (Button {
        $($key:ident: $value:expr),* $(,)?
    }) => {
        {
            let mut button = Button::new("");
            $(button = button.$key($value);)*
            Box::new(button)
        }
    };
    
    (Text($content:expr)) => {
        Box::new(Text::new($content))
    };
}
```

## BitCraps Widget System

For BitCraps, we need specialized widgets for the distributed gaming interface.

```rust
/// BitCraps-specific widget for displaying game state
pub struct GameBoardWidget {
    id: WidgetId,
    game_state: Arc<RwLock<GameState>>,
    dice_animation: Option<DiceAnimation>,
    bet_areas: Vec<BetArea>,
    player_info: PlayerInfo,
}

pub struct BetArea {
    area_type: BetType,
    bounds: Rect,
    current_bets: Vec<Bet>,
    hover_state: bool,
    locked: bool,
}

pub struct DiceAnimation {
    start_time: std::time::Instant,
    duration: std::time::Duration,
    initial_values: (u8, u8),
    final_values: (u8, u8),
    trajectory: Vec<Position>,
}

impl GameBoardWidget {
    pub fn new(game_state: Arc<RwLock<GameState>>) -> Self {
        Self {
            id: WidgetId::new(),
            game_state,
            dice_animation: None,
            bet_areas: Self::create_bet_areas(),
            player_info: PlayerInfo::default(),
        }
    }
    
    fn create_bet_areas() -> Vec<BetArea> {
        vec![
            BetArea {
                area_type: BetType::PassLine,
                bounds: Rect {
                    position: Position { x: 100.0, y: 300.0 },
                    size: Size { width: 200.0, height: 50.0 },
                },
                current_bets: Vec::new(),
                hover_state: false,
                locked: false,
            },
            BetArea {
                area_type: BetType::DontPass,
                bounds: Rect {
                    position: Position { x: 100.0, y: 360.0 },
                    size: Size { width: 200.0, height: 50.0 },
                },
                current_bets: Vec::new(),
                hover_state: false,
                locked: false,
            },
            // Add more bet areas...
        ]
    }
    
    pub fn animate_dice_roll(&mut self, result: (u8, u8)) {
        self.dice_animation = Some(DiceAnimation {
            start_time: std::time::Instant::now(),
            duration: std::time::Duration::from_secs(2),
            initial_values: (
                rand::random::<u8>() % 6 + 1,
                rand::random::<u8>() % 6 + 1,
            ),
            final_values: result,
            trajectory: self.calculate_dice_trajectory(),
        });
    }
    
    fn calculate_dice_trajectory() -> Vec<Position> {
        // Physics simulation for dice movement
        let mut positions = Vec::new();
        let mut pos = Position { x: 400.0, y: 100.0 };
        let mut velocity = Position { x: -5.0, y: 10.0 };
        let gravity = 0.5;
        let bounce_damping = 0.7;
        
        for _ in 0..60 {
            velocity.y += gravity;
            pos.x += velocity.x;
            pos.y += velocity.y;
            
            // Bounce off table
            if pos.y > 250.0 {
                pos.y = 250.0;
                velocity.y *= -bounce_damping;
                velocity.x *= 0.9; // Friction
            }
            
            positions.push(pos);
        }
        
        positions
    }
}

impl Widget for GameBoardWidget {
    fn render(&self, context: &mut RenderContext) -> Result<(), WidgetError> {
        // Draw craps table background
        context.draw_image("craps_table.png", Position { x: 0.0, y: 0.0 })?;
        
        // Draw bet areas
        for area in &self.bet_areas {
            let color = if area.hover_state {
                Color::rgba(255, 255, 0, 128)
            } else if area.locked {
                Color::rgba(128, 128, 128, 64)
            } else {
                Color::rgba(0, 255, 0, 64)
            };
            
            context.draw_rect(area.bounds, color)?;
            
            // Draw bets in this area
            for bet in &area.current_bets {
                self.render_chip_stack(context, bet)?;
            }
        }
        
        // Animate dice if rolling
        if let Some(animation) = &self.dice_animation {
            let elapsed = animation.start_time.elapsed();
            if elapsed < animation.duration {
                let progress = elapsed.as_secs_f32() / animation.duration.as_secs_f32();
                let frame = (progress * animation.trajectory.len() as f32) as usize;
                if frame < animation.trajectory.len() {
                    let pos = animation.trajectory[frame];
                    
                    // Draw spinning dice
                    let rotation = progress * 720.0; // Two full rotations
                    context.draw_dice(animation.initial_values.0, pos, rotation)?;
                    context.draw_dice(animation.initial_values.1, 
                                    Position { x: pos.x + 50.0, y: pos.y }, 
                                    rotation + 45.0)?;
                }
            } else {
                // Show final result
                context.draw_dice(animation.final_values.0, 
                                Position { x: 350.0, y: 250.0 }, 0.0)?;
                context.draw_dice(animation.final_values.1, 
                                Position { x: 410.0, y: 250.0 }, 0.0)?;
            }
        }
        
        Ok(())
    }
    
    fn handle_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseMove { position } => {
                // Update hover states for bet areas
                for area in &mut self.bet_areas {
                    area.hover_state = area.bounds.contains(*position);
                }
                EventResult::Consumed
            }
            Event::MouseDown { button: MouseButton::Left, position } => {
                // Check if clicking on a bet area
                for area in &self.bet_areas {
                    if area.bounds.contains(*position) && !area.locked {
                        // Place bet event
                        return EventResult::Bubble(Event::Custom(
                            CustomEvent::PlaceBet {
                                bet_type: area.area_type.clone(),
                                amount: 10, // Default bet amount
                            }
                        ));
                    }
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
    
    // Additional trait methods...
}
```

## Performance Optimization

Widget architectures must be highly optimized to maintain smooth 60fps rendering.

```rust
/// Virtual DOM for efficient widget updates
pub struct VirtualDom {
    current_tree: Option<VNode>,
    render_queue: VecDeque<RenderCommand>,
}

#[derive(Clone)]
pub enum VNode {
    Element {
        tag: String,
        props: HashMap<String, String>,
        children: Vec<VNode>,
        key: Option<String>,
    },
    Text(String),
    Widget(WidgetId),
}

pub enum RenderCommand {
    Create(VNode),
    Update(VNode, VNode),
    Remove(VNode),
    Move(VNode, usize),
}

impl VirtualDom {
    pub fn new() -> Self {
        Self {
            current_tree: None,
            render_queue: VecDeque::new(),
        }
    }
    
    pub fn diff(&mut self, old: &VNode, new: &VNode) {
        match (old, new) {
            (VNode::Text(old_text), VNode::Text(new_text)) => {
                if old_text != new_text {
                    self.render_queue.push_back(
                        RenderCommand::Update(old.clone(), new.clone())
                    );
                }
            }
            (
                VNode::Element { tag: old_tag, props: old_props, children: old_children, key: old_key },
                VNode::Element { tag: new_tag, props: new_props, children: new_children, key: new_key },
            ) => {
                if old_tag != new_tag || old_key != new_key {
                    self.render_queue.push_back(
                        RenderCommand::Update(old.clone(), new.clone())
                    );
                } else {
                    // Diff props
                    if old_props != new_props {
                        self.render_queue.push_back(
                            RenderCommand::Update(old.clone(), new.clone())
                        );
                    }
                    
                    // Diff children with key-based reconciliation
                    self.diff_children(old_children, new_children);
                }
            }
            _ => {
                self.render_queue.push_back(
                    RenderCommand::Update(old.clone(), new.clone())
                );
            }
        }
    }
    
    fn diff_children(&mut self, old_children: &[VNode], new_children: &[VNode]) {
        // Implement key-based child reconciliation
        // This is a simplified version
        let max_len = old_children.len().max(new_children.len());
        
        for i in 0..max_len {
            match (old_children.get(i), new_children.get(i)) {
                (Some(old), Some(new)) => self.diff(old, new),
                (Some(old), None) => {
                    self.render_queue.push_back(RenderCommand::Remove(old.clone()));
                }
                (None, Some(new)) => {
                    self.render_queue.push_back(RenderCommand::Create(new.clone()));
                }
                (None, None) => unreachable!(),
            }
        }
    }
    
    pub fn apply_changes(&mut self) -> Result<(), WidgetError> {
        while let Some(command) = self.render_queue.pop_front() {
            match command {
                RenderCommand::Create(node) => {
                    // Create new DOM element
                }
                RenderCommand::Update(old, new) => {
                    // Update existing element
                }
                RenderCommand::Remove(node) => {
                    // Remove element from DOM
                }
                RenderCommand::Move(node, index) => {
                    // Move element to new position
                }
            }
        }
        Ok(())
    }
}

/// Render batching for performance
pub struct RenderBatcher {
    pending_updates: Vec<WidgetId>,
    frame_budget: Duration,
    last_frame_time: Instant,
}

impl RenderBatcher {
    pub fn new(target_fps: u32) -> Self {
        Self {
            pending_updates: Vec::new(),
            frame_budget: Duration::from_micros(1_000_000 / target_fps as u64),
            last_frame_time: Instant::now(),
        }
    }
    
    pub fn request_update(&mut self, widget_id: WidgetId) {
        if !self.pending_updates.contains(&widget_id) {
            self.pending_updates.push(widget_id);
        }
    }
    
    pub fn process_frame(&mut self, tree: &mut WidgetTree, renderer: &mut Renderer) 
        -> Result<(), WidgetError> 
    {
        let frame_start = Instant::now();
        let mut widgets_processed = 0;
        
        while let Some(widget_id) = self.pending_updates.pop() {
            // Check if we still have time in this frame
            if frame_start.elapsed() > self.frame_budget * 80 / 100 {
                // Reserve 20% of frame time for actual rendering
                break;
            }
            
            if let Some(widget) = tree.find_widget(widget_id) {
                widget.render(&mut renderer.context())?;
                widgets_processed += 1;
            }
        }
        
        // Commit all renders
        renderer.present()?;
        
        self.last_frame_time = frame_start;
        Ok(())
    }
}
```

## Testing Widget Architecture

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_widget_tree_construction() {
        let root = Container::new()
            .add_child(Box::new(Button::new("Test")))
            .add_child(Box::new(Text::new("Hello")));
        
        let tree = WidgetTree::new(Box::new(root));
        assert!(tree.find_widget(tree.root.id()).is_some());
    }
    
    #[test]
    fn test_event_propagation() {
        let mut button = Button::new("Click me");
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();
        
        button = button.on_click(move || {
            clicked_clone.store(true, Ordering::Relaxed);
        });
        
        // Simulate click
        button.handle_event(&Event::MouseDown {
            button: MouseButton::Left,
            position: Position { x: 50.0, y: 20.0 },
        });
        button.handle_event(&Event::MouseUp {
            button: MouseButton::Left,
            position: Position { x: 50.0, y: 20.0 },
        });
        
        assert!(clicked.load(Ordering::Relaxed));
    }
    
    #[test]
    fn test_flex_layout() {
        let layout = FlexLayout {
            direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            gap: 10.0,
        };
        
        let container = Rect {
            position: Position { x: 0.0, y: 0.0 },
            size: Size { width: 300.0, height: 100.0 },
        };
        
        let mut children: Vec<&mut dyn Widget> = vec![
            &mut Button::new("Button 1"),
            &mut Button::new("Button 2"),
            &mut Button::new("Button 3"),
        ];
        
        let positions = layout.calculate_layout(container, &mut children);
        
        assert_eq!(positions.len(), 3);
        // Verify spacing between elements
        assert!(positions[1].position.x > positions[0].position.x + positions[0].size.width);
    }
    
    #[test]
    fn test_reactive_state() {
        let state = ReactiveState::new(0);
        let updated = Arc::new(AtomicBool::new(false));
        let updated_clone = updated.clone();
        
        state.subscribe(move |value| {
            if *value == 42 {
                updated_clone.store(true, Ordering::Relaxed);
            }
        });
        
        state.set(42);
        assert!(updated.load(Ordering::Relaxed));
    }
    
    #[test]
    fn test_virtual_dom_diff() {
        let mut vdom = VirtualDom::new();
        
        let old = VNode::Element {
            tag: "div".to_string(),
            props: HashMap::new(),
            children: vec![VNode::Text("Hello".to_string())],
            key: None,
        };
        
        let new = VNode::Element {
            tag: "div".to_string(),
            props: HashMap::new(),
            children: vec![VNode::Text("World".to_string())],
            key: None,
        };
        
        vdom.diff(&old, &new);
        assert!(!vdom.render_queue.is_empty());
    }
}
```

## Common Pitfalls and Solutions

1. **Memory Leaks in Event Handlers**: Always use weak references in closures
2. **Inefficient Re-rendering**: Implement proper diffing and memoization
3. **Layout Thrashing**: Batch layout calculations
4. **State Synchronization**: Use message passing, not shared mutable state
5. **Focus Management**: Implement proper focus chains and keyboard navigation
6. **Accessibility**: Don't forget ARIA attributes and screen reader support

## Practical Exercises

1. **Build a Custom Widget**: Create a slider widget with custom styling
2. **Implement Virtual Scrolling**: Build a list that can handle 100,000 items
3. **Create a Theme System**: Design a theming system with dark/light modes
4. **Build a Form Framework**: Create form widgets with validation
5. **Implement Drag and Drop**: Add drag and drop support to your widget tree

## Conclusion

Widget architecture represents one of the most successful abstractions in software engineering. By breaking complex UIs into composable, reusable components, we can build sophisticated interfaces that remain maintainable and performant. The principles we've explored—composition, state management, layout systems, and reactive programming—form the foundation of modern UI development.

In the context of BitCraps and distributed systems, widget architecture becomes even more critical. It provides the structure needed to handle real-time updates from multiple peers, maintain consistency across different states, and provide responsive user interfaces even under network uncertainty.

Remember that great widget architecture isn't just about the widgets themselves—it's about how they compose, how they communicate, and how they evolve with your application's needs.

## Additional Resources

- "Design Patterns: Elements of Reusable Object-Oriented Software" by Gang of Four
- React's Reconciliation Documentation
- Flutter's Widget Architecture Documentation
- The Elm Architecture Pattern
- IMGUI (Immediate Mode GUI) Concepts
- Accessibility Guidelines for Widget Development

---

*Next Chapter: [95: Repository Pattern Implementation](./95_repository_pattern_implementation.md)*
