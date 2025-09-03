//! Interactive API Playground
//!
//! Web-based interactive environment for testing and exploring the BitCraps API
//! with real-time request/response inspection and code generation.

use crate::sdk_v2::{
    error::{SDKError, SDKResult},
    rest::generate_openapi_spec,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Filter, Reply};

/// Interactive API playground server
#[derive(Debug)]
pub struct APIPlayground {
    port: u16,
    config: PlaygroundConfig,
    state: Arc<RwLock<PlaygroundState>>,
}

/// Playground configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaygroundConfig {
    pub title: String,
    pub description: String,
    pub theme: PlaygroundTheme,
    pub features: PlaygroundFeatures,
    pub api_base_url: String,
    pub websocket_url: String,
}

/// Playground theme settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaygroundTheme {
    Light,
    Dark,
    Auto,
}

/// Available playground features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaygroundFeatures {
    pub code_generation: bool,
    pub request_history: bool,
    pub api_documentation: bool,
    pub websocket_testing: bool,
    pub auth_testing: bool,
    pub mock_responses: bool,
}

/// Playground runtime state
#[derive(Debug, Default)]
struct PlaygroundState {
    request_history: Vec<RequestEntry>,
    active_connections: HashMap<String, ConnectionInfo>,
    mock_responses: HashMap<String, MockResponse>,
}

/// Request history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestEntry {
    id: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    method: String,
    url: String,
    headers: HashMap<String, String>,
    body: Option<String>,
    response_status: u16,
    response_body: String,
    response_time_ms: u64,
}

/// WebSocket connection info
#[derive(Debug, Clone)]
struct ConnectionInfo {
    id: String,
    connected_at: chrono::DateTime<chrono::Utc>,
    last_activity: chrono::DateTime<chrono::Utc>,
    message_count: u64,
}

/// Mock response configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: serde_json::Value,
    delay_ms: Option<u64>,
}

impl Default for PlaygroundConfig {
    fn default() -> Self {
        Self {
            title: "BitCraps API Playground".to_string(),
            description: "Interactive testing environment for the BitCraps API".to_string(),
            theme: PlaygroundTheme::Dark,
            features: PlaygroundFeatures {
                code_generation: true,
                request_history: true,
                api_documentation: true,
                websocket_testing: true,
                auth_testing: true,
                mock_responses: true,
            },
            api_base_url: "https://api.bitcraps.com/v2".to_string(),
            websocket_url: "wss://ws.bitcraps.com/v2".to_string(),
        }
    }
}

impl APIPlayground {
    /// Create a new API playground
    pub fn new(port: u16) -> Self {
        Self {
            port,
            config: PlaygroundConfig::default(),
            state: Arc::new(RwLock::new(PlaygroundState::default())),
        }
    }
    
    /// Create playground with custom configuration
    pub fn with_config(port: u16, config: PlaygroundConfig) -> Self {
        Self {
            port,
            config,
            state: Arc::new(RwLock::new(PlaygroundState::default())),
        }
    }
    
    /// Start the playground server
    pub async fn start(&self) -> SDKResult<()> {
        let state = self.state.clone();
        let config = self.config.clone();
        
        // API routes
        let api_routes = self.build_api_routes(state.clone(), config.clone());
        
        // Static file routes
        let static_routes = self.build_static_routes();
        
        // WebSocket routes
        let ws_routes = self.build_websocket_routes(state.clone());
        
        let routes = api_routes
            .or(static_routes)
            .or(ws_routes)
            .with(warp::cors().allow_any_origin());
        
        println!("🎲 BitCraps API Playground starting on http://localhost:{}", self.port);
        println!("📚 Documentation: http://localhost:{}/docs", self.port);
        println!("🧪 Testing: http://localhost:{}/playground", self.port);
        
        warp::serve(routes)
            .run(([127, 0, 0, 1], self.port))
            .await;
        
        Ok(())
    }
    
    /// Build API routes
    fn build_api_routes(
        &self,
        state: Arc<RwLock<PlaygroundState>>,
        config: PlaygroundConfig,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        let config_route = warp::path("api")
            .and(warp::path("config"))
            .and(warp::path::end())
            .and(warp::get())
            .map(move || warp::reply::json(&config));
        
        let openapi_route = warp::path("api")
            .and(warp::path("openapi"))
            .and(warp::path::end())
            .and(warp::get())
            .map(|| {
                let spec = generate_openapi_spec();
                warp::reply::json(&spec)
            });
        
        let history_route = warp::path("api")
            .and(warp::path("history"))
            .and(warp::path::end())
            .and(warp::get())
            .and(with_state(state.clone()))
            .and_then(Self::get_request_history);
        
        let proxy_route = warp::path("api")
            .and(warp::path("proxy"))
            .and(warp::method())
            .and(warp::path::tail())
            .and(warp::header::headers_cloned())
            .and(warp::body::bytes())
            .and(with_state(state.clone()))
            .and_then(Self::proxy_request);
        
        let mock_route = warp::path("api")
            .and(warp::path("mock"))
            .and(warp::path::param::<String>())
            .and(warp::post())
            .and(warp::body::json())
            .and(with_state(state.clone()))
            .and_then(Self::set_mock_response);
        
        config_route
            .or(openapi_route)
            .or(history_route)
            .or(proxy_route)
            .or(mock_route)
    }
    
    /// Build static file routes
    fn build_static_routes(&self) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        let playground_html = Self::generate_playground_html();
        let docs_html = Self::generate_docs_html();
        
        let root = warp::path::end()
            .map(|| warp::reply::html(playground_html.clone()));
        
        let playground = warp::path("playground")
            .and(warp::path::end())
            .map(move || warp::reply::html(playground_html.clone()));
        
        let docs = warp::path("docs")
            .and(warp::path::end())
            .map(move || warp::reply::html(docs_html.clone()));
        
        root.or(playground).or(docs)
    }
    
    /// Build WebSocket routes
    fn build_websocket_routes(
        &self,
        state: Arc<RwLock<PlaygroundState>>,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        warp::path("ws")
            .and(warp::ws())
            .and(with_state(state))
            .map(|ws: warp::ws::Ws, state| {
                ws.on_upgrade(move |socket| Self::handle_websocket(socket, state))
            })
    }
    
    /// Handle WebSocket connections
    async fn handle_websocket(
        ws: warp::ws::WebSocket,
        state: Arc<RwLock<PlaygroundState>>,
    ) {
        use futures_util::{SinkExt, StreamExt};
        
        let (mut ws_tx, mut ws_rx) = ws.split();
        let connection_id = uuid::Uuid::new_v4().to_string();
        
        // Register connection
        {
            let mut state_guard = state.write().await;
            state_guard.active_connections.insert(connection_id.clone(), ConnectionInfo {
                id: connection_id.clone(),
                connected_at: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
                message_count: 0,
            });
        }
        
        // Send welcome message
        let welcome_msg = serde_json::json!({
            "type": "welcome",
            "connection_id": connection_id,
            "timestamp": chrono::Utc::now()
        });
        
        if ws_tx.send(warp::ws::Message::text(welcome_msg.to_string())).await.is_err() {
            return;
        }
        
        // Handle incoming messages
        while let Some(result) = ws_rx.next().await {
            match result {
                Ok(msg) => {
                    if msg.is_text() {
                        if let Ok(text) = msg.to_str() {
                            Self::handle_websocket_message(text, &connection_id, &state, &mut ws_tx).await;
                        }
                    } else if msg.is_close() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        
        // Remove connection
        {
            let mut state_guard = state.write().await;
            state_guard.active_connections.remove(&connection_id);
        }
    }
    
    /// Handle WebSocket message
    async fn handle_websocket_message(
        message: &str,
        connection_id: &str,
        state: &Arc<RwLock<PlaygroundState>>,
        ws_tx: &mut futures_util::stream::SplitSink<warp::ws::WebSocket, warp::ws::Message>,
    ) {
        use futures_util::SinkExt;
        
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(message) {
            let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
            
            let response = match msg_type {
                "ping" => serde_json::json!({
                    "type": "pong",
                    "timestamp": chrono::Utc::now()
                }),
                "get_connections" => {
                    let state_guard = state.read().await;
                    serde_json::json!({
                        "type": "connections",
                        "connections": state_guard.active_connections.len()
                    })
                }
                "echo" => serde_json::json!({
                    "type": "echo",
                    "data": msg.get("data"),
                    "connection_id": connection_id
                }),
                _ => serde_json::json!({
                    "type": "error",
                    "message": format!("Unknown message type: {}", msg_type)
                })
            };
            
            // Update connection activity
            {
                let mut state_guard = state.write().await;
                if let Some(conn) = state_guard.active_connections.get_mut(connection_id) {
                    conn.last_activity = chrono::Utc::now();
                    conn.message_count += 1;
                }
            }
            
            let _ = ws_tx.send(warp::ws::Message::text(response.to_string())).await;
        }
    }
    
    /// Get request history
    async fn get_request_history(
        state: Arc<RwLock<PlaygroundState>>,
    ) -> Result<impl Reply, warp::Rejection> {
        let state_guard = state.read().await;
        Ok(warp::reply::json(&state_guard.request_history))
    }
    
    /// Proxy API requests
    async fn proxy_request(
        method: warp::http::Method,
        path: warp::path::Tail,
        headers: warp::http::HeaderMap,
        body: bytes::Bytes,
        state: Arc<RwLock<PlaygroundState>>,
    ) -> Result<impl Reply, warp::Rejection> {
        let start_time = std::time::Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        
        // Extract headers
        let mut header_map = HashMap::new();
        for (key, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                header_map.insert(key.to_string(), value_str.to_string());
            }
        }
        
        // Build target URL
        let target_url = format!("https://api.bitcraps.com/v2/{}", path.as_str());
        
        // Make the request (mock implementation)
        let response_status = 200;
        let response_body = serde_json::json!({
            "message": "Mock response from playground",
            "method": method.to_string(),
            "path": path.as_str(),
            "timestamp": chrono::Utc::now()
        }).to_string();
        
        let response_time = start_time.elapsed().as_millis() as u64;
        
        // Record in history
        {
            let mut state_guard = state.write().await;
            state_guard.request_history.push(RequestEntry {
                id: request_id,
                timestamp: chrono::Utc::now(),
                method: method.to_string(),
                url: target_url,
                headers: header_map,
                body: if body.is_empty() { None } else { Some(String::from_utf8_lossy(&body).to_string()) },
                response_status,
                response_body: response_body.clone(),
                response_time_ms: response_time,
            });
        }
        
        Ok(warp::reply::with_status(
            warp::reply::json(&serde_json::from_str::<serde_json::Value>(&response_body).unwrap()),
            warp::http::StatusCode::from_u16(response_status).unwrap(),
        ))
    }
    
    /// Set mock response
    async fn set_mock_response(
        endpoint: String,
        mock_response: MockResponse,
        state: Arc<RwLock<PlaygroundState>>,
    ) -> Result<impl Reply, warp::Rejection> {
        {
            let mut state_guard = state.write().await;
            state_guard.mock_responses.insert(endpoint, mock_response);
        }
        
        Ok(warp::reply::json(&serde_json::json!({"success": true})))
    }
    
    /// Generate playground HTML
    fn generate_playground_html() -> String {
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>BitCraps API Playground</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 0;
            background-color: #1a1a1a;
            color: #e0e0e0;
        }
        
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            padding: 20px;
            text-align: center;
        }
        
        .container {
            display: flex;
            height: calc(100vh - 80px);
        }
        
        .sidebar {
            width: 300px;
            background-color: #2a2a2a;
            padding: 20px;
            overflow-y: auto;
        }
        
        .main-content {
            flex: 1;
            padding: 20px;
            overflow-y: auto;
        }
        
        .request-form {
            background-color: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
            margin-bottom: 20px;
        }
        
        .response-panel {
            background-color: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
        }
        
        input, select, textarea {
            width: 100%;
            padding: 10px;
            margin: 5px 0;
            border: 1px solid #555;
            border-radius: 4px;
            background-color: #3a3a3a;
            color: #e0e0e0;
        }
        
        button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 12px 24px;
            border-radius: 4px;
            cursor: pointer;
            font-size: 16px;
        }
        
        button:hover {
            opacity: 0.9;
        }
        
        .endpoint-list {
            list-style: none;
            padding: 0;
        }
        
        .endpoint-item {
            padding: 10px;
            margin: 5px 0;
            background-color: #3a3a3a;
            border-radius: 4px;
            cursor: pointer;
        }
        
        .endpoint-item:hover {
            background-color: #4a4a4a;
        }
        
        .method-tag {
            display: inline-block;
            padding: 2px 8px;
            border-radius: 3px;
            font-size: 12px;
            font-weight: bold;
        }
        
        .method-get { background-color: #28a745; }
        .method-post { background-color: #007bff; }
        .method-put { background-color: #ffc107; color: #000; }
        .method-delete { background-color: #dc3545; }
        
        pre {
            background-color: #1a1a1a;
            padding: 15px;
            border-radius: 4px;
            overflow-x: auto;
        }
        
        .tab-container {
            border-bottom: 1px solid #555;
            margin-bottom: 20px;
        }
        
        .tab {
            display: inline-block;
            padding: 10px 20px;
            background-color: #3a3a3a;
            border: none;
            cursor: pointer;
            border-radius: 4px 4px 0 0;
            margin-right: 5px;
        }
        
        .tab.active {
            background-color: #667eea;
        }
    </style>
</head>
<body>
    <div class="header">
        <h1>🎲 BitCraps API Playground</h1>
        <p>Interactive testing environment for the BitCraps API</p>
    </div>
    
    <div class="container">
        <div class="sidebar">
            <h3>API Endpoints</h3>
            <ul class="endpoint-list" id="endpoints">
                <li class="endpoint-item" onclick="loadEndpoint('GET', '/games')">
                    <span class="method-tag method-get">GET</span> /games
                </li>
                <li class="endpoint-item" onclick="loadEndpoint('POST', '/games')">
                    <span class="method-tag method-post">POST</span> /games
                </li>
                <li class="endpoint-item" onclick="loadEndpoint('GET', '/games/{id}')">
                    <span class="method-tag method-get">GET</span> /games/{id}
                </li>
                <li class="endpoint-item" onclick="loadEndpoint('POST', '/games/{id}/join')">
                    <span class="method-tag method-post">POST</span> /games/{id}/join
                </li>
                <li class="endpoint-item" onclick="loadEndpoint('GET', '/network/peers')">
                    <span class="method-tag method-get">GET</span> /network/peers
                </li>
            </ul>
            
            <h3>WebSocket</h3>
            <button onclick="connectWebSocket()">Connect</button>
            <div id="ws-status">Disconnected</div>
        </div>
        
        <div class="main-content">
            <div class="tab-container">
                <button class="tab active" onclick="showTab('request')">Request</button>
                <button class="tab" onclick="showTab('response')">Response</button>
                <button class="tab" onclick="showTab('history')">History</button>
            </div>
            
            <div id="request-tab" class="tab-content">
                <div class="request-form">
                    <select id="method">
                        <option value="GET">GET</option>
                        <option value="POST">POST</option>
                        <option value="PUT">PUT</option>
                        <option value="DELETE">DELETE</option>
                    </select>
                    
                    <input type="text" id="endpoint" placeholder="/games" />
                    
                    <h4>Headers</h4>
                    <textarea id="headers" rows="3" placeholder="Authorization: Bearer your-token"></textarea>
                    
                    <h4>Body</h4>
                    <textarea id="body" rows="6" placeholder="JSON request body"></textarea>
                    
                    <button onclick="sendRequest()">Send Request</button>
                </div>
            </div>
            
            <div id="response-tab" class="tab-content" style="display: none;">
                <div class="response-panel">
                    <h4>Response</h4>
                    <div id="response-status"></div>
                    <pre id="response-body"></pre>
                </div>
            </div>
            
            <div id="history-tab" class="tab-content" style="display: none;">
                <div class="response-panel">
                    <h4>Request History</h4>
                    <div id="history-list"></div>
                </div>
            </div>
        </div>
    </div>

    <script>
        let ws = null;
        
        function loadEndpoint(method, path) {
            document.getElementById('method').value = method;
            document.getElementById('endpoint').value = path;
            
            // Set default body for POST requests
            if (method === 'POST' && path === '/games') {
                document.getElementById('body').value = JSON.stringify({
                    name: "My Craps Game",
                    gameType: "Craps",
                    maxPlayers: 8,
                    minBet: 10,
                    maxBet: 1000
                }, null, 2);
            }
        }
        
        async function sendRequest() {
            const method = document.getElementById('method').value;
            const endpoint = document.getElementById('endpoint').value;
            const headers = document.getElementById('headers').value;
            const body = document.getElementById('body').value;
            
            try {
                const response = await fetch('/api/proxy' + endpoint, {
                    method: method,
                    headers: {
                        'Content-Type': 'application/json',
                        ...parseHeaders(headers)
                    },
                    body: method !== 'GET' ? body : undefined
                });
                
                const responseText = await response.text();
                
                document.getElementById('response-status').textContent = 
                    `Status: ${response.status} ${response.statusText}`;
                document.getElementById('response-body').textContent = 
                    JSON.stringify(JSON.parse(responseText), null, 2);
                
                showTab('response');
                loadHistory();
                
            } catch (error) {
                document.getElementById('response-status').textContent = 'Error: ' + error.message;
                document.getElementById('response-body').textContent = '';
                showTab('response');
            }
        }
        
        function parseHeaders(headerText) {
            const headers = {};
            headerText.split('\n').forEach(line => {
                const [key, value] = line.split(':').map(s => s.trim());
                if (key && value) headers[key] = value;
            });
            return headers;
        }
        
        function showTab(tabName) {
            document.querySelectorAll('.tab-content').forEach(tab => {
                tab.style.display = 'none';
            });
            document.querySelectorAll('.tab').forEach(tab => {
                tab.classList.remove('active');
            });
            
            document.getElementById(tabName + '-tab').style.display = 'block';
            event.target.classList.add('active');
        }
        
        async function loadHistory() {
            try {
                const response = await fetch('/api/history');
                const history = await response.json();
                
                const historyList = document.getElementById('history-list');
                historyList.innerHTML = '';
                
                history.slice(-10).reverse().forEach(entry => {
                    const div = document.createElement('div');
                    div.innerHTML = `
                        <strong>${entry.method} ${entry.url}</strong><br>
                        Status: ${entry.response_status}<br>
                        Time: ${entry.response_time_ms}ms<br>
                        <small>${new Date(entry.timestamp).toLocaleString()}</small>
                        <hr>
                    `;
                    historyList.appendChild(div);
                });
            } catch (error) {
                console.error('Failed to load history:', error);
            }
        }
        
        function connectWebSocket() {
            if (ws) {
                ws.close();
                return;
            }
            
            ws = new WebSocket('ws://localhost:3000/ws');
            
            ws.onopen = () => {
                document.getElementById('ws-status').textContent = 'Connected';
            };
            
            ws.onmessage = (event) => {
                console.log('WebSocket message:', JSON.parse(event.data));
            };
            
            ws.onclose = () => {
                document.getElementById('ws-status').textContent = 'Disconnected';
                ws = null;
            };
        }
        
        // Load history on page load
        window.onload = () => {
            loadHistory();
        };
    </script>
</body>
</html>"#.to_string()
    }
    
    /// Generate documentation HTML
    fn generate_docs_html() -> String {
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>BitCraps API Documentation</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #1a1a1a;
            color: #e0e0e0;
            line-height: 1.6;
        }
        
        .container {
            max-width: 1200px;
            margin: 0 auto;
        }
        
        h1, h2, h3 {
            color: #667eea;
        }
        
        .endpoint {
            background-color: #2a2a2a;
            padding: 20px;
            border-radius: 8px;
            margin: 20px 0;
        }
        
        .method {
            display: inline-block;
            padding: 4px 12px;
            border-radius: 4px;
            font-weight: bold;
            margin-right: 10px;
        }
        
        .method-get { background-color: #28a745; }
        .method-post { background-color: #007bff; }
        
        code {
            background-color: #3a3a3a;
            padding: 2px 6px;
            border-radius: 3px;
        }
        
        pre {
            background-color: #1a1a1a;
            padding: 15px;
            border-radius: 4px;
            overflow-x: auto;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🎲 BitCraps API Documentation</h1>
        
        <h2>Overview</h2>
        <p>The BitCraps API provides comprehensive access to the decentralized gaming platform, 
        allowing developers to create games, manage players, and interact with the consensus system.</p>
        
        <h2>Authentication</h2>
        <p>All API requests require authentication using a Bearer token:</p>
        <pre>Authorization: Bearer your-api-key</pre>
        
        <h2>Base URL</h2>
        <p>Production: <code>https://api.bitcraps.com/v2</code></p>
        <p>Staging: <code>https://staging-api.bitcraps.com/v2</code></p>
        
        <h2>Endpoints</h2>
        
        <div class="endpoint">
            <h3><span class="method method-get">GET</span>/games</h3>
            <p>Retrieve a list of available games with optional filtering.</p>
            <h4>Query Parameters</h4>
            <ul>
                <li><code>status</code> - Filter by game status (Waiting, InProgress, Finished)</li>
                <li><code>limit</code> - Maximum number of results (default: 50)</li>
            </ul>
            <h4>Response</h4>
            <pre>
[
  {
    "id": "game_123",
    "name": "High Stakes Craps",
    "gameType": "Craps",
    "status": "Waiting",
    "currentPlayers": 3,
    "maxPlayers": 8,
    "minBet": 100,
    "maxBet": 10000,
    "createdAt": "2024-01-01T12:00:00Z"
  }
]
            </pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method method-post">POST</span>/games</h3>
            <p>Create a new game instance.</p>
            <h4>Request Body</h4>
            <pre>
{
  "name": "My Craps Game",
  "gameType": "Craps",
  "maxPlayers": 8,
  "minBet": 10,
  "maxBet": 1000,
  "isPrivate": false
}
            </pre>
            <h4>Response</h4>
            <pre>
{
  "id": "game_456",
  "name": "My Craps Game",
  "gameType": "Craps",
  "status": "Waiting",
  "currentPlayers": 1,
  "maxPlayers": 8,
  "minBet": 10,
  "maxBet": 1000,
  "createdAt": "2024-01-01T12:30:00Z"
}
            </pre>
        </div>
        
        <div class="endpoint">
            <h3><span class="method method-post">POST</span>/games/{gameId}/join</h3>
            <p>Join an existing game.</p>
            <h4>Response</h4>
            <pre>
{
  "sessionId": "session_789",
  "gameId": "game_456",
  "playerId": "player_123",
  "joinedAt": "2024-01-01T12:35:00Z"
}
            </pre>
        </div>
        
        <h2>WebSocket API</h2>
        <p>Real-time updates are available through WebSocket connections:</p>
        <p>URL: <code>wss://ws.bitcraps.com/v2</code></p>
        
        <h3>Message Types</h3>
        <ul>
            <li><code>GameUpdate</code> - Game state changes</li>
            <li><code>PlayerAction</code> - Player actions and bets</li>
            <li><code>ChatMessage</code> - In-game chat messages</li>
            <li><code>ConsensusProposal</code> - Consensus voting proposals</li>
        </ul>
        
        <h2>Error Handling</h2>
        <p>The API uses standard HTTP status codes and returns detailed error information:</p>
        <pre>
{
  "success": false,
  "error": "Game not found",
  "errorCode": "GAME_NOT_FOUND",
  "requestId": "req_123",
  "timestamp": "2024-01-01T12:00:00Z"
}
        </pre>
        
        <h2>Rate Limiting</h2>
        <p>API requests are limited to 1000 requests per minute per API key. 
        Rate limit information is included in response headers:</p>
        <ul>
            <li><code>X-RateLimit-Limit</code> - Requests allowed per window</li>
            <li><code>X-RateLimit-Remaining</code> - Requests remaining</li>
            <li><code>X-RateLimit-Reset</code> - Window reset time</li>
        </ul>
        
        <h2>SDK Libraries</h2>
        <p>Official SDK libraries are available for multiple programming languages:</p>
        <ul>
            <li><strong>Rust:</strong> <code>bitcraps-client = "2.0"</code></li>
            <li><strong>Python:</strong> <code>pip install bitcraps-client</code></li>
            <li><strong>TypeScript/JavaScript:</strong> <code>npm install bitcraps-client</code></li>
            <li><strong>Go:</strong> <code>go get github.com/bitcraps/go-client</code></li>
        </ul>
        
        <p><a href="/playground">← Back to API Playground</a></p>
    </div>
</body>
</html>"#.to_string()
    }
}

/// Helper function for warp filter
fn with_state(
    state: Arc<RwLock<PlaygroundState>>,
) -> impl Filter<Extract = (Arc<RwLock<PlaygroundState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_playground_config_default() {
        let config = PlaygroundConfig::default();
        assert_eq!(config.title, "BitCraps API Playground");
        assert!(config.features.code_generation);
        assert!(config.features.request_history);
    }
    
    #[test]
    fn test_playground_creation() {
        let playground = APIPlayground::new(3000);
        assert_eq!(playground.port, 3000);
    }
    
    #[test]
    fn test_playground_html_generation() {
        let html = APIPlayground::generate_playground_html();
        assert!(html.contains("BitCraps API Playground"));
        assert!(html.contains("interactive"));
    }
}