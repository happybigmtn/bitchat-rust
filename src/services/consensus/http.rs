//! HTTP server for ConsensusService (Axum)

use super::service::ConsensusService;
use super::types::*;
use axum::{routing::{get, post}, Router, extract::{State, Query}, Json};
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState { service: Arc<RwLock<ConsensusService>> }

#[derive(Deserialize)]
struct StatusQuery {
    proposal_id: Option<String>,
}

#[derive(Deserialize)]
struct QCQuery {
    proposal_id: Option<String>,
    sequence: Option<u64>,
}

pub async fn start_http(service: Arc<RwLock<ConsensusService>>, addr: SocketAddr) -> crate::error::Result<()> {
    let state = AppState { service };
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/consensus/status", get(get_status))
        .route("/api/v1/consensus/propose", post(post_propose))
        .route("/api/v1/consensus/vote", post(post_vote))
        .route("/api/v1/consensus/qc", get(get_qc))
        .route("/api/v1/consensus/randomness/start", post(rnd_start))
        .route("/api/v1/consensus/randomness/commit", post(rnd_commit))
        .route("/api/v1/consensus/randomness/reveal", post(rnd_reveal))
        .route("/api/v1/consensus/randomness/status", get(rnd_status))
        .route("/api/v1/consensus/randomness/evidence", get(rnd_evidence))
        .route("/api/v1/consensus/randomness/penalties", get(rnd_penalties))
        .route("/api/v1/consensus/subscribe", get(ws_subscribe))
        .route("/api/v1/consensus/admin/add-validator", post(post_add_validator))
        .route("/api/v1/consensus/admin/remove-validator", post(post_remove_validator))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| crate::error::Error::NetworkError(e.to_string()))?;
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("Consensus HTTP server failed: {}", e);
        }
    });
    Ok(())
}

async fn health() -> &'static str { "ok" }

async fn get_status(State(state): State<AppState>, Query(q): Query<StatusQuery>) -> Json<StatusResponse> {
    let req = StatusRequest {
        proposal_id: q.proposal_id.as_ref().and_then(|h| hex::decode(h).ok()).and_then(|v| {
            if v.len()==32 { let mut id=[0u8;32]; id.copy_from_slice(&v); Some(id) } else { None }
        }),
    };
    let resp = state.service.read().await.get_status(req).await.unwrap_or(StatusResponse{
        network_height: 0,
        current_round: 0,
        active_validators: 0,
        leader: None,
        active_proposals: vec![],
        metrics: ConsensusMetricsResponse{
            total_proposals:0, committed_proposals:0, rejected_proposals:0, timeout_proposals:0,
            byzantine_faults_detected:0, average_rounds_to_commit:0.0, average_time_to_commit_ms:0,
        },
    });
    Json(resp)
}

async fn post_propose(State(state): State<AppState>, Json(req): Json<ProposeRequest>) -> Json<ProposeResponse> {
    let resp = state.service.read().await.propose(req).await.unwrap_or(ProposeResponse{ proposal_id:[0u8;32], status:"error".into() });
    Json(resp)
}

async fn post_vote(State(state): State<AppState>, Json(req): Json<VoteRequest>) -> Json<VoteResponse> {
    let resp = state.service.read().await.vote(req).await.unwrap_or(VoteResponse{ accepted:false, current_round:0 });
    Json(resp)
}

async fn get_qc(State(state): State<AppState>, Query(q): Query<QCQuery>) -> Json<Option<Vec<u8>>> {
    if let Some(pid_hex) = q.proposal_id.as_ref() {
        if let Ok(bytes) = hex::decode(pid_hex) {
            if bytes.len()==32 {
                let mut id=[0u8;32];
                id.copy_from_slice(&bytes);
                let qc = state.service.read().await.get_quorum_certificate_by_proposal(id).await.unwrap_or(None);
                return Json(qc);
            }
        }
    }
    if let Some(seq) = q.sequence {
        let qc = state.service.read().await.get_quorum_certificate(seq).await.unwrap_or(None);
        return Json(qc);
    }
    Json(None)
}

#[derive(Deserialize)]
struct RndStartBody { round_id: u64 }

async fn rnd_start(State(state): State<AppState>, Json(body): Json<RndStartBody>) -> Json<serde_json::Value> {
    let ok = state.service.read().await.randomness_start_round(body.round_id).await.unwrap_or(false);
    Json(serde_json::json!({"ok": ok}))
}

#[derive(Deserialize)]
struct RndPeerBody { round_id: u64, peer_id_hex: String }

async fn rnd_commit(State(state): State<AppState>, Json(body): Json<RndPeerBody>) -> Json<serde_json::Value> {
    let mut peer = [0u8;32]; if let Ok(b)=hex::decode(&body.peer_id_hex){ if b.len()==32 { peer.copy_from_slice(&b); } }
    let ok = state.service.read().await.randomness_commit(body.round_id, peer).await.unwrap_or(false);
    Json(serde_json::json!({"ok": ok}))
}

async fn rnd_reveal(State(state): State<AppState>, Json(body): Json<RndPeerBody>) -> Json<serde_json::Value> {
    let mut peer = [0u8;32]; if let Ok(b)=hex::decode(&body.peer_id_hex){ if b.len()==32 { peer.copy_from_slice(&b); } }
    let ok = state.service.read().await.randomness_reveal(body.round_id, peer).await.unwrap_or(false);
    Json(serde_json::json!({"ok": ok}))
}

#[derive(Deserialize)]
struct RndQuery { round_id: u64 }

async fn rnd_status(State(state): State<AppState>, Query(q): Query<RndQuery>) -> Json<serde_json::Value> {
    let v = state.service.read().await.randomness_status(q.round_id).await.unwrap_or(serde_json::json!({"round_id": q.round_id, "status":"error"}));
    Json(v)
}

async fn rnd_evidence(State(state): State<AppState>, Query(q): Query<RndQuery>) -> Json<Option<Vec<[u8;32]>>> {
    let e = state.service.read().await.randomness_evidence(q.round_id).await.unwrap_or(None);
    Json(e)
}

async fn rnd_penalties(State(state): State<AppState>) -> Json<serde_json::Value> {
    let items_raw = state.service.read().await.randomness_penalties_snapshot().await;
    let mut items: Vec<_> = items_raw.into_iter().map(|(p,c)| (hex::encode(p), c)).collect();
    items.sort_by(|a,b| b.1.cmp(&a.1));
    Json(serde_json::json!({"penalties": items}))
}

#[derive(Deserialize)]
struct AdminValidatorReq { peer_id_hex: String, stake: Option<u64> }

async fn post_add_validator(State(state): State<AppState>, Json(body): Json<AdminValidatorReq>) -> Json<UpdateValidatorResponse> {
    let mut peer = [0u8;32];
    if let Ok(bytes) = hex::decode(&body.peer_id_hex) { if bytes.len()==32 { peer.copy_from_slice(&bytes); } }
    let resp = state.service.read().await.update_validator(UpdateValidatorRequest{ peer_id: peer, action: super::ValidatorUpdateAction::Add, stake: body.stake }).await
        .unwrap_or(UpdateValidatorResponse{ success:false, active_validators:0 });
    Json(resp)
}

async fn post_remove_validator(State(state): State<AppState>, Json(body): Json<AdminValidatorReq>) -> Json<UpdateValidatorResponse> {
    let mut peer = [0u8;32];
    if let Ok(bytes) = hex::decode(&body.peer_id_hex) { if bytes.len()==32 { peer.copy_from_slice(&bytes); } }
    let resp = state.service.read().await.update_validator(UpdateValidatorRequest{ peer_id: peer, action: super::ValidatorUpdateAction::Remove, stake: None }).await
        .unwrap_or(UpdateValidatorResponse{ success:false, active_validators:0 });
    Json(resp)
}

async fn ws_subscribe(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    use futures_util::StreamExt;

    // Subscribe to proposals and results
    let mut proposals_rx = state.service.read().await.subscribe_proposals();
    let mut results_rx = state.service.read().await.subscribe_results();

    // Send a hello message
    let _ = socket
        .send(Message::Text("{\"type\":\"hello\",\"service\":\"consensus\"}".into()))
        .await;

    loop {
        tokio::select! {
            Ok(prop) = proposals_rx.recv() => {
                if let Ok(txt) = serde_json::to_string(&serde_json::json!({
                    "type": "proposal",
                    "proposal": prop,
                })) {
                    if socket.send(Message::Text(txt)).await.is_err() { break; }
                }
            }
            Ok(res) = results_rx.recv() => {
                if let Ok(txt) = serde_json::to_string(&serde_json::json!({
                    "type": "result",
                    "result": res,
                })) {
                    if socket.send(Message::Text(txt)).await.is_err() { break; }
                }
            }
            // Read and ignore incoming pings/pongs/texts to keep connection alive
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
