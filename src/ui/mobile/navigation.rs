//! Navigation system for mobile UI

use super::*;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Navigation controller for managing screen transitions
pub struct NavigationController {
    stack: Vec<Route>,
    modal_stack: Vec<Route>,
    tab_controller: Option<TabController>,
    transition_style: TransitionStyle,
    navigation_callbacks: HashMap<String, NavigationCallback>,
}

impl NavigationController {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            modal_stack: Vec::new(),
            tab_controller: None,
            transition_style: TransitionStyle::default(),
            navigation_callbacks: HashMap::new(),
        }
    }

    /// Push a new screen onto the navigation stack
    pub fn push(&mut self, route: Route) -> NavigationResult {
        // Check if we can navigate
        if let Some(callback) = self.navigation_callbacks.get("will_push") {
            if !callback.can_navigate(&route) {
                return NavigationResult::Cancelled;
            }
        }

        self.stack.push(route.clone());
        
        // Notify callbacks
        if let Some(callback) = self.navigation_callbacks.get("did_push") {
            callback.did_navigate(&route);
        }

        NavigationResult::Success
    }

    /// Pop the current screen from the stack
    pub fn pop(&mut self) -> NavigationResult {
        if self.stack.len() <= 1 {
            return NavigationResult::CannotPopRoot;
        }

        if let Some(route) = self.stack.pop() {
            // Notify callbacks
            if let Some(callback) = self.navigation_callbacks.get("did_pop") {
                callback.did_navigate(&route);
            }
            NavigationResult::Success
        } else {
            NavigationResult::Failed
        }
    }

    /// Pop to the root screen
    pub fn pop_to_root(&mut self) -> NavigationResult {
        if self.stack.len() <= 1 {
            return NavigationResult::AlreadyAtRoot;
        }

        let root = self.stack[0].clone();
        self.stack.clear();
        self.stack.push(root);

        NavigationResult::Success
    }

    /// Replace the current screen
    pub fn replace(&mut self, route: Route) -> NavigationResult {
        if self.stack.is_empty() {
            self.stack.push(route);
        } else {
            let index = self.stack.len() - 1;
            self.stack[index] = route;
        }
        NavigationResult::Success
    }

    /// Present a modal screen
    pub fn present_modal(&mut self, route: Route) -> NavigationResult {
        self.modal_stack.push(route.clone());
        
        if let Some(callback) = self.navigation_callbacks.get("did_present_modal") {
            callback.did_navigate(&route);
        }

        NavigationResult::Success
    }

    /// Dismiss the current modal
    pub fn dismiss_modal(&mut self) -> NavigationResult {
        if let Some(route) = self.modal_stack.pop() {
            if let Some(callback) = self.navigation_callbacks.get("did_dismiss_modal") {
                callback.did_navigate(&route);
            }
            NavigationResult::Success
        } else {
            NavigationResult::NoModalToDissmiss
        }
    }

    /// Get the current route
    pub fn current_route(&self) -> Option<&Route> {
        self.modal_stack.last().or_else(|| self.stack.last())
    }

    /// Check if a specific screen is in the stack
    pub fn contains_screen(&self, screen: &Screen) -> bool {
        self.stack.iter().any(|route| &route.screen == screen)
    }

    /// Set up tab navigation
    pub fn setup_tabs(&mut self, tabs: Vec<Tab>) {
        self.tab_controller = Some(TabController::new(tabs));
    }

    /// Switch to a different tab
    pub fn switch_tab(&mut self, index: usize) -> NavigationResult {
        if let Some(controller) = &mut self.tab_controller {
            controller.select_tab(index)
        } else {
            NavigationResult::NoTabController
        }
    }

    /// Register a navigation callback
    pub fn register_callback(&mut self, name: String, callback: NavigationCallback) {
        self.navigation_callbacks.insert(name, callback);
    }

    /// Get navigation history
    pub fn get_history(&self) -> Vec<Route> {
        self.stack.clone()
    }

    /// Clear navigation history
    pub fn clear_history(&mut self) {
        self.stack.clear();
        self.modal_stack.clear();
    }
}

/// Route representation
#[derive(Debug, Clone)]
pub struct Route {
    pub screen: Screen,
    pub params: RouteParams,
    pub transition: Option<TransitionStyle>,
}

impl Route {
    pub fn new(screen: Screen) -> Self {
        Self {
            screen,
            params: RouteParams::default(),
            transition: None,
        }
    }

    pub fn with_params(mut self, params: RouteParams) -> Self {
        self.params = params;
        self
    }

    pub fn with_transition(mut self, transition: TransitionStyle) -> Self {
        self.transition = Some(transition);
        self
    }
}

/// Route parameters
#[derive(Debug, Clone, Default)]
pub struct RouteParams {
    pub data: HashMap<String, String>,
}

impl RouteParams {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
}

/// Tab controller for bottom navigation
pub struct TabController {
    tabs: Vec<Tab>,
    selected_index: usize,
    badge_counts: HashMap<usize, u32>,
}

impl TabController {
    pub fn new(tabs: Vec<Tab>) -> Self {
        Self {
            tabs,
            selected_index: 0,
            badge_counts: HashMap::new(),
        }
    }

    pub fn select_tab(&mut self, index: usize) -> NavigationResult {
        if index >= self.tabs.len() {
            return NavigationResult::InvalidTabIndex;
        }

        self.selected_index = index;
        NavigationResult::Success
    }

    pub fn current_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.selected_index)
    }

    pub fn set_badge(&mut self, index: usize, count: u32) {
        self.badge_counts.insert(index, count);
    }

    pub fn get_badge(&self, index: usize) -> Option<u32> {
        self.badge_counts.get(&index).copied()
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
}

/// Tab definition
#[derive(Debug, Clone)]
pub struct Tab {
    pub title: String,
    pub icon: String,
    pub screen: Screen,
    pub selected_icon: Option<String>,
}

impl Tab {
    pub fn new(title: impl Into<String>, icon: impl Into<String>, screen: Screen) -> Self {
        Self {
            title: title.into(),
            icon: icon.into(),
            screen,
            selected_icon: None,
        }
    }

    pub fn with_selected_icon(mut self, icon: impl Into<String>) -> Self {
        self.selected_icon = Some(icon.into());
        self
    }
}

/// Transition styles for navigation
#[derive(Debug, Clone)]
pub enum TransitionStyle {
    Push,
    Modal,
    Fade,
    None,
    Custom(CustomTransition),
}

impl Default for TransitionStyle {
    fn default() -> Self {
        TransitionStyle::Push
    }
}

/// Custom transition definition
#[derive(Debug, Clone)]
pub struct CustomTransition {
    pub duration_ms: u64,
    pub animation_type: AnimationType,
}

#[derive(Debug, Clone)]
pub enum AnimationType {
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Scale,
    Rotate,
}

/// Navigation result
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationResult {
    Success,
    Failed,
    Cancelled,
    CannotPopRoot,
    AlreadyAtRoot,
    NoModalToDissmiss,
    NoTabController,
    InvalidTabIndex,
}

/// Navigation callback
pub struct NavigationCallback {
    can_navigate: Box<dyn Fn(&Route) -> bool + Send + Sync>,
    did_navigate: Box<dyn Fn(&Route) + Send + Sync>,
}

impl NavigationCallback {
    pub fn new<F1, F2>(can_navigate: F1, did_navigate: F2) -> Self
    where
        F1: Fn(&Route) -> bool + Send + Sync + 'static,
        F2: Fn(&Route) + Send + Sync + 'static,
    {
        Self {
            can_navigate: Box::new(can_navigate),
            did_navigate: Box::new(did_navigate),
        }
    }

    pub fn can_navigate(&self, route: &Route) -> bool {
        (self.can_navigate)(route)
    }

    pub fn did_navigate(&self, route: &Route) {
        (self.did_navigate)(route)
    }
}

/// Deep link handler
pub struct DeepLinkHandler {
    patterns: HashMap<String, DeepLinkPattern>,
}

impl DeepLinkHandler {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
        }
    }

    pub fn register_pattern(&mut self, pattern: String, handler: DeepLinkPattern) {
        self.patterns.insert(pattern, handler);
    }

    pub fn handle_url(&self, url: &str) -> Option<Route> {
        for (pattern, handler) in &self.patterns {
            if let Some(route) = handler.match_url(url, pattern) {
                return Some(route);
            }
        }
        None
    }
}

/// Deep link pattern
pub struct DeepLinkPattern {
    handler: Box<dyn Fn(&str, &str) -> Option<Route> + Send + Sync>,
}

impl DeepLinkPattern {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(&str, &str) -> Option<Route> + Send + Sync + 'static,
    {
        Self {
            handler: Box::new(handler),
        }
    }

    pub fn match_url(&self, url: &str, pattern: &str) -> Option<Route> {
        (self.handler)(url, pattern)
    }
}

/// Navigation event system
pub struct NavigationEventSystem {
    sender: mpsc::Sender<NavigationEvent>,
    receiver: Option<mpsc::Receiver<NavigationEvent>>,
}

impl NavigationEventSystem {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(500); // Low priority: UI navigation events
        Self {
            sender,
            receiver: Some(receiver),
        }
    }

    pub fn emit(&self, event: NavigationEvent) {
        // Use try_send for bounded channels to handle backpressure
        match self.sender.try_send(event) {
            Ok(_) => {},
            Err(mpsc::error::TrySendError::Full(_)) => {
                log::warn!("UI navigation channel full, dropping event (backpressure)");
                // Could add metrics here: UI_EVENT_DROPS.inc();
                // Drop the event instead of blocking UI
            },
            Err(mpsc::error::TrySendError::Closed(_)) => {
                log::error!("Navigation event channel closed");
            }
        }
    }

    pub async fn next_event(&mut self) -> Option<NavigationEvent> {
        if let Some(receiver) = &mut self.receiver {
            receiver.recv().await
        } else {
            None
        }
    }
}

/// Navigation events
#[derive(Debug, Clone)]
pub enum NavigationEvent {
    ScreenPushed(Screen),
    ScreenPopped(Screen),
    ModalPresented(Screen),
    ModalDismissed(Screen),
    TabChanged(usize),
    DeepLinkReceived(String),
}

/// Navigation coordinator for complex flows
pub struct NavigationCoordinator {
    flows: HashMap<String, NavigationFlow>,
    active_flow: Option<String>,
}

impl NavigationCoordinator {
    pub fn new() -> Self {
        Self {
            flows: HashMap::new(),
            active_flow: None,
        }
    }

    pub fn register_flow(&mut self, name: String, flow: NavigationFlow) {
        self.flows.insert(name, flow);
    }

    pub fn start_flow(&mut self, name: &str) -> Option<Route> {
        if let Some(flow) = self.flows.get(name) {
            self.active_flow = Some(name.to_string());
            flow.first_screen()
        } else {
            None
        }
    }

    pub fn next_in_flow(&self) -> Option<Route> {
        if let Some(flow_name) = &self.active_flow {
            if let Some(flow) = self.flows.get(flow_name) {
                return flow.next_screen();
            }
        }
        None
    }

    pub fn complete_flow(&mut self) {
        self.active_flow = None;
    }
}

/// Navigation flow definition
pub struct NavigationFlow {
    screens: Vec<Screen>,
    current_index: std::sync::Arc<std::sync::Mutex<usize>>,
    completion_handler: Option<Box<dyn Fn() + Send + Sync>>,
}

impl NavigationFlow {
    pub fn new(screens: Vec<Screen>) -> Self {
        Self {
            screens,
            current_index: std::sync::Arc::new(std::sync::Mutex::new(0)),
            completion_handler: None,
        }
    }

    pub fn with_completion<F>(mut self, handler: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.completion_handler = Some(Box::new(handler));
        self
    }

    pub fn first_screen(&self) -> Option<Route> {
        self.screens.first().map(|screen| Route::new(screen.clone()))
    }

    pub fn next_screen(&self) -> Option<Route> {
        let mut index = self.current_index.lock().unwrap();
        *index += 1;
        
        if *index < self.screens.len() {
            Some(Route::new(self.screens[*index].clone()))
        } else {
            if let Some(handler) = &self.completion_handler {
                handler();
            }
            None
        }
    }

    pub fn previous_screen(&self) -> Option<Route> {
        let mut index = self.current_index.lock().unwrap();
        if *index > 0 {
            *index -= 1;
            Some(Route::new(self.screens[*index].clone()))
        } else {
            None
        }
    }

    pub fn reset(&self) {
        let mut index = self.current_index.lock().unwrap();
        *index = 0;
    }
}