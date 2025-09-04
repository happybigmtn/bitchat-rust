//! HTTP server for GameEngineService (Axum)

use super::service::GameEngineService;
use super::types::*;
use axum::{routing::{get, post}, Router, extract::{State, Path}, Json};
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
        .route("/api/v1/games/:id/snapshot", get(get_snapshot))
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
    Json(svc.get_game_state(GetGameStateRequest{ game_id: gid }).await.unwrap_or(GetGameStateResponse { session_info: GameSessionInfo::new(gid, vec![], crate::protocol::craps::GamePhase::Off), valid_actions: Default::default() }))
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
    Json(svc.get_game_state(GetGameStateRequest{ game_id: gid }).await.unwrap_or(GetGameStateResponse { session_info: GameSessionInfo::new(gid, vec![], crate::protocol::craps::GamePhase::Off), valid_actions: Default::default() }))
}

fn parse_game_id_hex(s: &str) -> crate::protocol::GameId {
    if let Ok(bytes) = hex::decode(s) { if bytes.len()==16 { let mut id=[0u8;16]; id.copy_from_slice(&bytes); return id; } }
    [0u8;16]
}

