//! State management for mobile UI

use super::*;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use serde::{Serialize, Deserialize};

/// Global state manager for the mobile application
pub struct StateManager {
    state: Arc<RwLock<AppState>>,
    listeners: Arc<RwLock<Vec<StateListener>>>,
    persistence: Option<Box<dyn StatePersistence>>,
    event_bus: EventBus,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(AppState::default())),
            listeners: Arc::new(RwLock::new(Vec::new())),
            persistence: None,
            event_bus: EventBus::new(),
        }
    }

    /// Initialize with persistence
    pub fn with_persistence<P: StatePersistence + 'static>(mut self, persistence: P) -> Self {
        self.persistence = Some(Box::new(persistence));
        self
    }

    /// Load state from persistence
    pub async fn load(&self) -> Result<(), StateError> {
        if let Some(persistence) = &self.persistence {
            let loaded_state = persistence.load().await?;
            let mut state = self.state.write().await;
            *state = loaded_state;
        }
        Ok(())
    }

    /// Save state to persistence
    pub async fn save(&self) -> Result<(), StateError> {
        if let Some(persistence) = &self.persistence {
            let state = self.state.read().await;
            persistence.save(&state).await?;
        }
        Ok(())
    }

    /// Update state with a mutation
    pub async fn dispatch(&self, action: StateAction) -> Result<(), StateError> {
        let mut state = self.state.write().await;
        
        // Apply the action
        match action {
            StateAction::SetUser(user) => {
                state.user = Some(user);
            }
            StateAction::UpdateBalance(balance) => {
                state.wallet_balance = balance;
            }
            StateAction::StartGame(game) => {
                state.current_game = Some(game);
            }
            StateAction::EndGame => {
                state.current_game = None;
            }
            StateAction::AddPeer(peer) => {
                if !state.connected_peers.iter().any(|p| p.id == peer.id) {
                    state.connected_peers.push(peer);
                }
            }
            StateAction::RemovePeer(peer_id) => {
                state.connected_peers.retain(|p| p.id != peer_id);
            }
            StateAction::UpdateSettings(settings) => {
                state.settings = settings;
            }
            StateAction::Custom(handler) => {
                handler(&mut state);
            }
        }

        // Clone state for notification
        let new_state = state.clone();
        drop(state); // Release write lock

        // Notify listeners
        self.notify_listeners(new_state.clone()).await;

        // Save to persistence
        if self.persistence.is_some() {
            self.save().await?;
        }

        // Emit event
        self.event_bus.emit(StateEvent::StateChanged(new_state)).await;

        Ok(())
    }

    /// Get current state
    pub async fn get_state(&self) -> AppState {
        self.state.read().await.clone()
    }

    /// Subscribe to state changes
    pub async fn subscribe(&self, listener: StateListener) {
        let mut listeners = self.listeners.write().await;
        listeners.push(listener);
    }

    /// Notify all listeners of state change
    async fn notify_listeners(&self, state: AppState) {
        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener.on_state_change(&state);
        }
    }

    /// Get a specific slice of state
    pub async fn select<T, F>(&self, selector: F) -> T
    where
        F: FnOnce(&AppState) -> T,
    {
        let state = self.state.read().await;
        selector(&state)
    }

    /// Subscribe to events
    pub fn event_stream(&self) -> mpsc::UnboundedReceiver<StateEvent> {
        self.event_bus.subscribe()
    }
}

/// State actions for modifying app state
#[derive(Debug, Clone)]
pub enum StateAction {
    SetUser(UserProfile),
    UpdateBalance(u64),
    StartGame(GameState),
    EndGame,
    AddPeer(PeerInfo),
    RemovePeer(String),
    UpdateSettings(AppSettings),
    Custom(Arc<dyn Fn(&mut AppState) + Send + Sync>),
}

/// State listener for observing changes
pub struct StateListener {
    callback: Arc<dyn Fn(&AppState) + Send + Sync>,
}

impl StateListener {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(&AppState) + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn on_state_change(&self, state: &AppState) {
        (self.callback)(state);
    }
}

/// State persistence trait
#[async_trait::async_trait]
pub trait StatePersistence: Send + Sync {
    async fn save(&self, state: &AppState) -> Result<(), StateError>;
    async fn load(&self) -> Result<AppState, StateError>;
    async fn clear(&self) -> Result<(), StateError>;
}

/// File-based state persistence
pub struct FileStatePersistence {
    file_path: String,
}

impl FileStatePersistence {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }
}

#[async_trait::async_trait]
impl StatePersistence for FileStatePersistence {
    async fn save(&self, state: &AppState) -> Result<(), StateError> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| StateError::SerializationError(e.to_string()))?;
        
        tokio::fs::write(&self.file_path, json)
            .await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;
        
        Ok(())
    }

    async fn load(&self) -> Result<AppState, StateError> {
        let json = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;
        
        let state = serde_json::from_str(&json)
            .map_err(|e| StateError::DeserializationError(e.to_string()))?;
        
        Ok(state)
    }

    async fn clear(&self) -> Result<(), StateError> {
        tokio::fs::remove_file(&self.file_path)
            .await
            .map_err(|e| StateError::PersistenceError(e.to_string()))?;
        Ok(())
    }
}

/// Event bus for state events
pub struct EventBus {
    sender: mpsc::UnboundedSender<StateEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = mpsc::channel(100); // Bounded UI events
        Self { sender }
    }

    pub async fn emit(&self, event: StateEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> mpsc::Receiver<StateEvent> {
        let (tx, rx) = mpsc::channel(100); // Bounded subscription channel
        // In a real implementation, would store tx and forward events
        rx
    }
}

/// State events
#[derive(Debug, Clone)]
pub enum StateEvent {
    StateChanged(AppState),
    UserLoggedIn(UserProfile),
    UserLoggedOut,
    GameStarted(String),
    GameEnded(String),
    PeerConnected(String),
    PeerDisconnected(String),
    BalanceUpdated(u64),
    SettingsChanged(AppSettings),
}

/// State store for component-level state
pub struct ComponentState<T> {
    value: Arc<RwLock<T>>,
    listeners: Arc<RwLock<Vec<Arc<dyn Fn(&T) + Send + Sync>>>>,
}

impl<T: Clone> ComponentState<T> {
    pub fn new(initial: T) -> Self {
        Self {
            value: Arc::new(RwLock::new(initial)),
            listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn get(&self) -> T {
        self.value.read().await.clone()
    }

    pub async fn set(&self, new_value: T) {
        let mut value = self.value.write().await;
        *value = new_value.clone();
        drop(value);

        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener(&new_value);
        }
    }

    pub async fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut T),
    {
        let mut value = self.value.write().await;
        updater(&mut value);
        let new_value = value.clone();
        drop(value);

        let listeners = self.listeners.read().await;
        for listener in listeners.iter() {
            listener(&new_value);
        }
    }

    pub async fn subscribe<F>(&self, listener: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        let mut listeners = self.listeners.write().await;
        listeners.push(Arc::new(listener));
    }
}

/// Derived state that computes from other state
pub struct DerivedState<T, S> {
    source: Arc<ComponentState<S>>,
    compute: Arc<dyn Fn(&S) -> T + Send + Sync>,
    cached: Arc<RwLock<Option<T>>>,
}

impl<T: Clone, S: Clone> DerivedState<T, S> {
    pub fn new<F>(source: Arc<ComponentState<S>>, compute: F) -> Self
    where
        F: Fn(&S) -> T + Send + Sync + 'static,
    {
        Self {
            source,
            compute: Arc::new(compute),
            cached: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get(&self) -> T {
        let mut cached = self.cached.write().await;
        if cached.is_none() {
            let source_value = self.source.get().await;
            *cached = Some((self.compute)(&source_value));
        }
        cached.as_ref().unwrap().clone()
    }

    pub async fn invalidate(&self) {
        let mut cached = self.cached.write().await;
        *cached = None;
    }
}

/// State error types
#[derive(Debug)]
pub enum StateError {
    SerializationError(String),
    DeserializationError(String),
    PersistenceError(String),
    InvalidAction(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            StateError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            StateError::PersistenceError(msg) => write!(f, "Persistence error: {}", msg),
            StateError::InvalidAction(msg) => write!(f, "Invalid action: {}", msg),
        }
    }
}

impl std::error::Error for StateError {}

/// State selector for efficient state access
pub struct StateSelector<T> {
    selector: Arc<dyn Fn(&AppState) -> T + Send + Sync>,
}

impl<T: Clone + PartialEq> StateSelector<T> {
    pub fn new<F>(selector: F) -> Self
    where
        F: Fn(&AppState) -> T + Send + Sync + 'static,
    {
        Self {
            selector: Arc::new(selector),
        }
    }

    pub async fn select(&self, state_manager: &StateManager) -> T {
        state_manager.select(|state| (self.selector)(state)).await
    }
}

/// Middleware for state actions
pub trait StateMiddleware: Send + Sync {
    fn before_action(&self, action: &StateAction, state: &AppState);
    fn after_action(&self, action: &StateAction, state: &AppState);
}

/// Logging middleware
pub struct LoggingMiddleware;

impl StateMiddleware for LoggingMiddleware {
    fn before_action(&self, action: &StateAction, _state: &AppState) {
        tracing::debug!("Dispatching action: {:?}", action);
    }

    fn after_action(&self, _action: &StateAction, _state: &AppState) {
        tracing::debug!("Action completed");
    }
}

/// State history for undo/redo
pub struct StateHistory {
    past: Vec<AppState>,
    present: AppState,
    future: Vec<AppState>,
    max_history: usize,
}

impl StateHistory {
    pub fn new(initial: AppState) -> Self {
        Self {
            past: Vec::new(),
            present: initial,
            future: Vec::new(),
            max_history: 50,
        }
    }

    pub fn push(&mut self, state: AppState) {
        self.past.push(self.present.clone());
        self.present = state;
        self.future.clear();

        // Limit history size
        if self.past.len() > self.max_history {
            self.past.remove(0);
        }
    }

    pub fn undo(&mut self) -> Option<AppState> {
        if let Some(prev) = self.past.pop() {
            self.future.push(self.present.clone());
            self.present = prev.clone();
            Some(prev)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<AppState> {
        if let Some(next) = self.future.pop() {
            self.past.push(self.present.clone());
            self.present = next.clone();
            Some(next)
        } else {
            None
        }
    }

    pub fn current(&self) -> &AppState {
        &self.present
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}