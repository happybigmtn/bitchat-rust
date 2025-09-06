//! HTTP server for GameEngineService (Axum)

use super::service::GameEngineService;
use super::types::*;
use axum::{routing::{get, post}, Router, extract::{State, Path}, Json};
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState { service: Arc<RwLock<GameEngineService>> }

pub async fn start_http(service: Arc<RwLock<GameEngineService>>, addr: SocketAddr) -> crate::error::Result<()> {
    let state = AppState { service };
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/games", get(list_games).post(create_game))
        .route("/api/v1/games/:id", get(get_game_state))
        .route("/api/v1/games/:id/actions", post(process_action))
        .route("/api/v1/games/:id/subscribe", get(ws_subscribe))
        .route("/api/v1/games/:id/snapshot", get(get_snapshot))
        .route("/api/v1/games/:id/randomness/:round", get(get_randomness).post(set_randomness))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| crate::error::Error::NetworkError(e.to_string()))?;
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await { log::error!("GameEngine HTTP failed: {}", e); }
    });
    Ok(())
}

async fn health() -> &'static str { "ok" }

async fn list_games(State(state): State<AppState>) -> Json<ListGamesResponse> {
    let req = ListGamesRequest { 
        player_id: None, 
        active_only: false, 
        limit: None 
    };
    let svc = state.service.read().await;
    Json(svc.list_games(req).await.unwrap_or(ListGamesResponse { games: vec![], total_count: 0 }))
}

async fn create_game(State(state): State<AppState>, Json(req): Json<CreateGameRequest>) -> Json<CreateGameResponse> {
    let svc = state.service.read().await;
    Json(svc.create_game(req).await.unwrap_or(CreateGameResponse { game_id: [0u8;16], session_info: GameSessionInfo::new([0u8;16], vec![], crate::protocol::craps::GamePhase::Off) }))
}

async fn get_game_state(State(state): State<AppState>, Path(id): Path<String>) -> Json<GetGameStateResponse> {
    let gid = parse_game_id_hex(&id);
    let svc = state.service.read().await;
    Json(svc.get_game_state(GetGameStateRequest{ game_id: gid }).await.unwrap_or(GetGameStateResponse { session_info: GameSessionInfo::new(gid, vec![], crate::protocol::craps::GamePhase::Off), valid_actions: Default::default(), sequence: None, qc: None }))
}

async fn process_action(State(state): State<AppState>, Path(id): Path<String>, Json(mut req): Json<ProcessActionRequest>) -> Json<ProcessActionResponse> {
    req.game_id = parse_game_id_hex(&id);
    let game_id = req.game_id;
    let svc = state.service.read().await;
    Json(svc.process_action(req).await.unwrap_or(ProcessActionResponse { result: GameActionResult::CashOut { player: [0u8;32], amount: 0 }, updated_session: GameSessionInfo::new(game_id, vec![], crate::protocol::craps::GamePhase::Off) }))
}

async fn get_snapshot(State(state): State<AppState>, Path(id): Path<String>) -> Json<GetGameStateResponse> {
    // Use get_game_state as a snapshot endpoint
    let gid = parse_game_id_hex(&id);
    let svc = state.service.read().await;
    let mut resp = svc.get_game_state(GetGameStateRequest{ game_id: gid })
        .await
        .unwrap_or(GetGameStateResponse { session_info: GameSessionInfo::new(gid, vec![], crate::protocol::craps::GamePhase::Off), valid_actions: Default::default(), sequence: None, qc: None });
    // If a sequence is present, try to fetch QC from consensus service
    if let Some(seq) = resp.sequence {
        let url = format!("http://127.0.0.1:8082/api/v1/consensus/qc?sequence={}", seq);
        if let Ok(client) = reqwest::Client::builder().timeout(std::time::Duration::from_millis(800)).build() {
            if let Ok(r) = client.get(url).send().await { if r.status().is_success() {
                if let Ok(qc_bytes) = r.json::<Option<Vec<u8>>>().await { resp.qc = qc_bytes; }
            }}
        }
    }
    Json(resp)
}

fn parse_game_id_hex(s: &str) -> crate::protocol::GameId {
    if let Ok(bytes) = hex::decode(s) { if bytes.len()==16 { let mut id=[0u8;16]; id.copy_from_slice(&bytes); return id; } }
    [0u8;16]
}

/// Placeholder randomness endpoint: returns not available until proof store is wired
async fn get_randomness(
    State(state): State<AppState>,
    Path((id, round)): Path<(String, String)>,
) -> (axum::http::StatusCode, String) {
    let gid = parse_game_id_hex(&id);
    let round_num: u64 = round.parse().unwrap_or(0);
    let svc = state.service.read().await;
    if let Some(proof) = svc.get_randomness_proof(gid, round_num).await {
        (axum::http::StatusCode::OK, proof)
    } else {
        let body = format!(
            "{{\"game_id\":\"{}\",\"round\":{},\"status\":\"not_available\"}}",
            id,
            round_num
        );
        (axum::http::StatusCode::NOT_FOUND, body)
    }
}

#[derive(serde::Deserialize)]
struct SetProofBody { proof_json: String }

/// Admin/testing: set randomness proof bundle for a round
async fn set_randomness(
    State(state): State<AppState>,
    Path((id, round)): Path<(String, String)>,
    axum::Json(body): axum::Json<SetProofBody>,
) -> (axum::http::StatusCode, String) {
    let gid = parse_game_id_hex(&id);
    let round_num: u64 = round.parse().unwrap_or(0);
    let svc = state.service.read().await;
    svc.set_randomness_proof(gid, round_num, body.proof_json.clone()).await;
    (axum::http::StatusCode::OK, "{\"ok\":true}".to_string())
}

async fn ws_subscribe(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    let gid = parse_game_id_hex(&id);
    ws.on_upgrade(move |socket| handle_ws(socket, state, gid))
}

async fn handle_ws(mut socket: WebSocket, state: AppState, game_id: crate::protocol::GameId) {
    use futures_util::StreamExt;
    let rx = state.service.read().await.subscribe_events();
    tokio::pin!(rx);

    let _ = socket
        .send(Message::Text("{\"type\":\"hello\",\"service\":\"game\"}".into()))
        .await;

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                // Filter by game id
                let matches = match &event {
                    super::types::GameEvent::GameCreated{ game_id: gid, .. } => gid == &game_id,
                    super::types::GameEvent::BetPlaced{ game_id: gid, .. } => gid == &game_id,
                    super::types::GameEvent::DiceRolled{ game_id: gid, .. } => gid == &game_id,
                    super::types::GameEvent::CashOut{ game_id: gid, .. } => gid == &game_id,
                    super::types::GameEvent::Snapshot{ game_id: gid, .. } => gid == &game_id,
                };
                if matches {
                    // Wrap event with timestamp for latency measurement
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let payload = serde_json::json!({
                        "ts": ts,
                        "event": event,
                    });
                    let txt = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
                    if socket.send(Message::Text(txt)).await.is_err() { break; }
                }
            }
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Ping(data) => { let _ = socket.send(Message::Pong(data)).await; }
                    Message::Close(_) => { break; }
                    _ => {}
                }
            }
            else => { break; }
        }
    }
}
