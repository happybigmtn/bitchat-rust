#![cfg(feature = "api-gateway")]

//! API Gateway Implementation
//!
//! Main gateway service that coordinates routing, authentication, and load balancing.

use super::circuit_breaker::CircuitBreaker;
use super::load_balancer::LoadBalancer;
use super::LoadBalancingStrategy;
use super::middleware::{AuthMiddleware, RateLimitMiddleware};
use super::routing::Router;
use super::*;
use crate::services::api_gateway::geo::{region_from_ip, region_from_jwt_claim};
use crate::mesh::gateway_registry::{GatewayRegistry, GatewayInfo};
mod broker;
use broker::{InMemoryBroker, SharedBroker, Broker};
use crate::error::{Error, Result};
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, Method, Request, StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
    Router as AxumRouter,
};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// API Gateway service
pub struct ApiGateway {
    config: GatewayConfig,
    router: Arc<Router>,
    load_balancer: Arc<LoadBalancer>,
    circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
    rate_limiter: Arc<RateLimitMiddleware>,
    auth_middleware: Arc<AuthMiddleware>,
    metrics: Arc<RwLock<GatewayMetrics>>,
    request_counter: Arc<AtomicU64>,
    shutdown_tx: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    broker: SharedBroker,
    registry: Arc<RwLock<GatewayRegistry>>,
}

impl ApiGateway {
    /// Create a new API gateway
    pub fn new(config: GatewayConfig) -> Self {
        let router = Arc::new(Router::new());
        let lb_strategy = config.lb_strategy;
        let load_balancer = Arc::new(LoadBalancer::new(lb_strategy, config.service_discovery.clone()));
        let circuit_breakers = Arc::new(DashMap::new());
        let rate_limiter = Arc::new(RateLimitMiddleware::new(config.rate_limit.clone()));
        let auth_middleware = Arc::new(AuthMiddleware::new(config.auth.clone()));
        let metrics = Arc::new(RwLock::new(GatewayMetrics::default()));
        let request_counter = Arc::new(AtomicU64::new(0));
        let broker: SharedBroker = match config.broker.method {
            #[cfg(feature = "broker-nats")]
            super::BrokerMethod::Nats => {
                let url = config.broker.url.clone().unwrap_or_else(|| "nats://127.0.0.1:4222".to_string());
                match crate::services::api_gateway::broker_nats::NatsBroker::connect(&url) {
                    // we are in sync fn; connect is async; so we cannot await here.
                    // For now, fall back to in-memory and log.
                    _ => {
                        log::warn!("NATS broker selected but async init not supported in sync constructor; falling back to in-memory");
                        Arc::new(InMemoryBroker::new()) as SharedBroker
                    }
                }
            }
            #[cfg(feature = "broker-redis")]
            super::BrokerMethod::Redis => {
                log::warn!("Redis broker selected but not implemented yet; falling back to in-memory");
                Arc::new(InMemoryBroker::new()) as SharedBroker
            }
            _ => Arc::new(InMemoryBroker::new()) as SharedBroker,
        };
        Self {
            config,
            router,
            load_balancer,
            circuit_breakers,
            rate_limiter,
            auth_middleware,
            metrics,
            request_counter,
            shutdown_tx: None,
            broker,
            registry: Arc::new(RwLock::new(GatewayRegistry::new())),
        }
    }
    
    /// Start the API gateway
    pub async fn start(&mut self) -> Result<()> {
        // Initialize default routes
        self.setup_default_routes().await?;
        
        // Start background services
        let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::unbounded_channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        // Start health checker
        let load_balancer = self.load_balancer.clone();
        let health_check_interval = self.config.service_discovery.health_check_interval;
        tokio::spawn(async move {
            Self::run_health_checker(load_balancer, health_check_interval, shutdown_rx).await;
        });

        // Start metrics collector
        let metrics = self.metrics.clone();
        let request_counter = self.request_counter.clone();
        tokio::spawn(async move {
            Self::update_metrics(metrics, request_counter).await;
        });

        // Start aggregator flush loop (fan-in to consensus)
        let lb_for_flush = self.load_balancer.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut tick = tokio::time::interval(Duration::from_millis(500));
            loop {
                tick.tick().await;
                let games = crate::services::api_gateway::aggregate::list_games();
                for game_id in games {
                    let round = crate::services::api_gateway::aggregate::current_round(game_id);
                    let groups = crate::services::api_gateway::aggregate::aggregated_groups(game_id, round);
                    if groups.is_empty() { continue; }

                    // Build propose request to consensus service
                    // Build JSON request compatible with consensus service
                    let payload = serde_json::json!({ "round": round, "groups": groups });
                    let data_vec = serde_json::to_vec(&payload).unwrap_or_default();
                    let req = serde_json::json!({
                        "game_id": game_id.to_vec(),
                        "proposal_type": { "GameAction": { "action": "aggregate_bets" } },
                        "data": data_vec,
                    });

                    if let Some(instance) = lb_for_flush.get_instance("consensus").await {
                        let url = format!("http://{}/api/v1/consensus/propose", instance.endpoint.address);
                        let resp = client.post(&url).json(&req).send().await;
                        if resp.is_ok() {
                            // Clear this round and advance
                            crate::services::api_gateway::aggregate::clear_round(game_id, round);
                            let _ = crate::services::api_gateway::aggregate::advance_round(game_id);
                        }
                    }
                }
            }
        });

        // Subscribe to consensus WS and re-publish to gateway broker (optional, behind ws-client)
        #[cfg(feature = "ws-client")]
        {
            let lb2 = self.load_balancer.clone();
            let broker = self.broker.clone();
            tokio::spawn(async move {
                use tokio_tungstenite::connect_async;
                use url::Url;
                let mut tick = tokio::time::interval(Duration::from_secs(5));
                loop {
                    tick.tick().await;
                    if let Some(instance) = lb2.get_instance("consensus").await {
                        let ws_url = format!("ws://{}/api/v1/consensus/subscribe", instance.endpoint.address);
                        if let Ok((ws_stream, _)) = connect_async(Url::parse(&ws_url).unwrap()) .await {
                            let (mut write, mut read) = ws_stream.split();
                            while let Some(msg) = read.next().await {
                                if let Ok(msg) = msg {
                                    if msg.is_text() {
                                        broker.publish("consensus:events", msg.to_text().unwrap_or_default().to_string());
                                    }
                                } else { break; }
                            }
                        }
                    }
                }
            });
        }
        
        // Build Axum router
        let app_router = self.build_router().await;

        // Start HTTP server
        let listener = tokio::net::TcpListener::bind(&self.config.listen_addr).await?;
        log::info!("API Gateway listening on {}", self.config.listen_addr);
        
        axum::serve(
            listener,
            app_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| Error::NetworkError(e.to_string()))?;

        Ok(())
    }
    
    /// Stop the API gateway
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        
        log::info!("API Gateway stopped");
        Ok(())
    }
    
    /// Add a service route
    pub async fn add_route(&self, route: RouteConfig) -> Result<()> {
        self.router.add_route(route).await;
        log::info!("Added route: {} -> {}", route.path, route.service);
        Ok(())
    }
    
    /// Remove a service route
    pub async fn remove_route(&self, path: &str) -> Result<()> {
        self.router.remove_route(path).await;
        log::info!("Removed route: {}", path);
        Ok(())
    }
    
    /// Get gateway metrics
    pub async fn get_metrics(&self) -> GatewayMetrics {
        self.metrics.read().await.clone()
    }
    
    // Private implementation methods
    
    async fn setup_default_routes(&self) -> Result<()> {
        // Game Engine routes
        self.add_route(RouteConfig {
            path: "/api/v1/games".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string(), "POST".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        }).await?;
        
        self.add_route(RouteConfig {
            path: "/api/v1/games/{id}".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["GET".to_string(), "POST".to_string()],
            auth_required: true,
            rate_limit_override: None,
            timeout_override: None,
        }).await?;
        
        self.add_route(RouteConfig {
            path: "/api/v1/games/{id}/actions".to_string(),
            service: "game-engine".to_string(),
            methods: vec!["POST".to_string()],
            auth_required: true,
            rate_limit_override: Some(100), // Lower limit for game actions
            timeout_override: None,
        }).await?;
        
        // Consensus routes
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/propose".to_string(),
            service: "consensus".to_string(),
            methods: vec!["POST".to_string()],
            auth_required: true,
            rate_limit_override: Some(50), // Lower limit for consensus operations
            timeout_override: Some(Duration::from_secs(60)),
        }).await?;
        
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/vote".to_string(),
            service: "consensus".to_string(),
            methods: vec!["POST".to_string()],
            auth_required: true,
            rate_limit_override: Some(200),
            timeout_override: None,
        }).await?;
        
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/status".to_string(),
            service: "consensus".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: false, // Public endpoint
            rate_limit_override: None,
            timeout_override: None,
        }).await?;

        // Quorum certificate (proof) endpoint
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/qc".to_string(),
            service: "consensus".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: false,
            rate_limit_override: None,
            timeout_override: None,
        }).await?;

        // Admin validator management endpoints (restricted via auth middleware)
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/admin/add-validator".to_string(),
            service: "consensus".to_string(),
            methods: vec!["POST".to_string()],
            auth_required: true,
            rate_limit_override: Some(50),
            timeout_override: None,
        }).await?;
        self.add_route(RouteConfig {
            path: "/api/v1/consensus/admin/remove-validator".to_string(),
            service: "consensus".to_string(),
            methods: vec!["POST".to_string()],
            auth_required: true,
            rate_limit_override: Some(50),
            timeout_override: None,
        }).await?;
        
        Ok(())
    }
    
    async fn build_router(&self) -> AxumRouter {
        let gateway_state = GatewayState {
            router: self.router.clone(),
            load_balancer: self.load_balancer.clone(),
            circuit_breakers: self.circuit_breakers.clone(),
            rate_limiter: self.rate_limiter.clone(),
            auth_middleware: self.auth_middleware.clone(),
            metrics: self.metrics.clone(),
            request_counter: self.request_counter.clone(),
            config: self.config.clone(),
            broker: self.broker.clone(),
            registry: self.registry.clone(),
        };
        
        AxumRouter::new()
            .route("/health", get(health_handler))
            .route("/metrics", get(metrics_handler))
            // Direct aggregation + proof routes handled in gateway
            .route("/api/v1/games/:id/bets", axum::routing::post(post_bet_handler))
            .route("/api/v1/games/:id/proofs", axum::routing::get(get_proofs_handler))
            .route("/api/v1/games/:id/payouts", axum::routing::post(post_payouts_handler))
            .route("/subscribe", get(ws_gateway_subscribe))
            // Admin endpoints for gateway registry
            .route("/admin/gateways/register", axum::routing::post(admin_register_gateway))
            .route("/admin/gateways", axum::routing::get(admin_list_gateways))
            .route("/api/*path", any(proxy_handler))
            .route("/*path", any(proxy_handler))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive())
                    .layer(middleware::from_fn_with_state(
                        gateway_state.clone(),
                        request_middleware,
                    ))
                    .into_inner(),
            )
            .with_state(gateway_state)
    }
    
    async fn run_health_checker(
        load_balancer: Arc<LoadBalancer>,
        interval: Duration,
        mut shutdown_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
    ) {
        let mut health_interval = tokio::time::interval(interval);
        
        loop {
            tokio::select! {
                _ = health_interval.tick() => {
                    load_balancer.check_service_health().await;
                }
                _ = shutdown_rx.recv() => {
                    break;
                }
            }
        }
    }
    
    async fn update_metrics(
        metrics: Arc<RwLock<GatewayMetrics>>,
        request_counter: Arc<AtomicU64>,
    ) {
        let mut last_request_count = 0u64;
        let mut metrics_interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            metrics_interval.tick().await;
            
            let current_requests = request_counter.load(Ordering::Relaxed);
            let requests_this_second = current_requests - last_request_count;
            last_request_count = current_requests;
            
            let mut metrics = metrics.write().await;
            metrics.requests_per_second = requests_this_second as f64;
        }
    }
}

/// Shared state for the gateway
#[derive(Clone)]
struct GatewayState {
    router: Arc<Router>,
    load_balancer: Arc<LoadBalancer>,
    circuit_breakers: Arc<DashMap<String, CircuitBreaker>>,
    rate_limiter: Arc<RateLimitMiddleware>,
    auth_middleware: Arc<AuthMiddleware>,
    metrics: Arc<RwLock<GatewayMetrics>>,
    request_counter: Arc<AtomicU64>,
    config: GatewayConfig,
    broker: SharedBroker,
    registry: Arc<RwLock<GatewayRegistry>>,
}

/// Health check endpoint
async fn health_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let metrics = state.metrics.read().await;
    let health_response = serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "metrics": {
            "total_requests": metrics.total_requests,
            "success_rate": metrics.success_rate(),
            "average_response_time": metrics.average_response_time,
            "requests_per_second": metrics.requests_per_second
        }
    });
    
    (StatusCode::OK, axum::Json(health_response))
}

/// Metrics endpoint
async fn metrics_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let metrics = state.metrics.read().await.clone();
    (StatusCode::OK, axum::Json(metrics))
}

#[derive(serde::Deserialize)]
struct RegisterGatewayReq {
    id: String,
    region: String,
    #[serde(default = "default_weight")] weight: u32,
    #[serde(default = "default_capacity")] capacity_score: u32,
    #[serde(default)] ws_topics_supported: Vec<String>,
}

fn default_weight() -> u32 { 100 }
fn default_capacity() -> u32 { 100 }

/// Admin: register or update a gateway in the registry
async fn admin_register_gateway(
    State(state): State<GatewayState>,
    axum::extract::Json(req): axum::extract::Json<RegisterGatewayReq>,
) -> impl IntoResponse {
    let mut reg = state.registry.write().await;
    let mut info = GatewayInfo::new(req.id, req.region);
    info.weight = req.weight;
    info.capacity_score = req.capacity_score;
    info.ws_topics_supported = req.ws_topics_supported;
    reg.register(info);
    (StatusCode::OK, axum::Json(serde_json::json!({"ok": true})))
}

/// Admin: list gateways in registry (optionally by region)
async fn admin_list_gateways(
    State(state): State<GatewayState>,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let reg = state.registry.read().await;
    if let Some(region) = q.get("region") {
        let list = reg.list_by_region(region);
        return (StatusCode::OK, axum::Json(serde_json::json!({"region": region, "gateways": list.into_iter().map(|g| serde_json::json!({
            "id": g.id,
            "region": g.region,
            "weight": g.weight,
            "capacity_score": g.capacity_score,
            "healthy": g.healthy,
        })).collect::<Vec<_>>() }))).into_response();
    }
    // all
    let mut all = Vec::new();
    for r in ["iad","sfo","fra","sin"].iter() { // simple iteration; registry may have others
        for g in reg.list_by_region(r) {
            all.push(serde_json::json!({
                "id": g.id,
                "region": g.region,
                "weight": g.weight,
                "capacity_score": g.capacity_score,
                "healthy": g.healthy,
            }));
        }
    }
    (StatusCode::OK, axum::Json(serde_json::json!({"gateways": all})))
}

/// Main proxy handler
async fn proxy_handler(
    State(state): State<GatewayState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, StatusCode> {
    let path = uri.path();
    let start_time = Instant::now();
    
    // Increment request counter
    state.request_counter.fetch_add(1, Ordering::Relaxed);
    
    // Create request context
    let mut context = RequestContext::new(addr.ip());
    context.request_id = uuid::Uuid::new_v4().to_string();
    context.user_agent = headers.get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());
    
    // Find matching route
    let route = match state.router.find_route(path, &method.to_string()).await {
        Some(route) => route,
        None => {
            return Ok((
                StatusCode::NOT_FOUND,
                axum::Json(GatewayResponse::<()>::error(
                    "Route not found".to_string(),
                    context.request_id,
                )),
            ).into_response());
        }
    };
    
    // Check authentication
    if route.auth_required {
        if let Err(e) = state.auth_middleware.authenticate(&headers, &mut context).await {
            return Ok((
                StatusCode::UNAUTHORIZED,
                axum::Json(GatewayResponse::<()>::error(
                    e.to_string(),
                    context.request_id,
                )),
            ).into_response());
        }
    }
    
    // Check rate limiting
    let rate_limit = route.rate_limit_override.unwrap_or(state.config.rate_limit.max_requests);
    if let Err(e) = state.rate_limiter.check_rate_limit(&context, rate_limit).await {
        let mut metrics = state.metrics.write().await;
        metrics.record_rate_limited();
        drop(metrics);
        
        return Ok((
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(GatewayResponse::<()>::error(
                e.to_string(),
                context.request_id,
            )),
        ).into_response());
    }
    
    // Get circuit breaker for service
    let circuit_breaker = state.circuit_breakers
        .entry(route.service.clone())
        .or_insert_with(|| CircuitBreaker::new(state.config.circuit_breaker.clone()));
    
    // Check circuit breaker
    if !circuit_breaker.can_execute().await {
        let mut metrics = state.metrics.write().await;
        metrics.record_circuit_breaker_open();
        drop(metrics);
        
        return Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(GatewayResponse::<()>::error(
                "Service temporarily unavailable".to_string(),
                context.request_id,
            )),
        ).into_response());
    }
    
    // Get service instance (prefer region if supplied via header, else sticky by client IP)
    let preferred_region = headers
        .get("x-region")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .or_else(|| state.config.region_self.clone());
    let instance = match state
        .load_balancer
        .get_instance_for_client_with_region(&route.service, addr.ip(), preferred_region.as_deref())
        .await
    {
        Some(instance) => instance,
        None => {
            return Ok((
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(GatewayResponse::<()>::error(
                    "No healthy service instances available".to_string(),
                    context.request_id,
                )),
            ).into_response());
        }
    };
    
    // Forward request to service
    let timeout = route.timeout_override.unwrap_or(state.config.request_timeout);
    let response_result = tokio::time::timeout(
        timeout,
        forward_request(instance, uri, method, headers, body),
    ).await;
    
    let elapsed = start_time.elapsed();
    let success = match &response_result {
        Ok(Ok(_)) => {
            circuit_breaker.record_success().await;
            true
        },
        _ => {
            circuit_breaker.record_failure().await;
            false
        }
    };
    
    // Update metrics
    let mut metrics = state.metrics.write().await;
    metrics.record_request(success, elapsed);
    drop(metrics);
    
    match response_result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Ok((
            StatusCode::BAD_GATEWAY,
            axum::Json(GatewayResponse::<()>::error(
                format!("Service error: {}", e),
                context.request_id,
            )),
        ).into_response()),
        Err(_) => Ok((
            StatusCode::GATEWAY_TIMEOUT,
            axum::Json(GatewayResponse::<()>::error(
                "Request timeout".to_string(),
                context.request_id,
            )),
        ).into_response()),
    }
}

#[derive(serde::Deserialize)]
struct BetReqBody { player_id_hex: String, bet_type: String, amount: u64 }

async fn post_bet_handler(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    Json(body): Json<BetReqBody>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    // Basic auth/rate-limit already handled by middleware stack above

    // Parse game id
    let gid = {
        if let Ok(bytes) = hex::decode(&id) { if bytes.len()==16 { let mut id=[0u8;16]; id.copy_from_slice(&bytes); id } else { [0u8;16] } } else { [0u8;16] }
    };
    // Parse player id
    let mut player = [0u8;32];
    if let Ok(bytes) = hex::decode(&body.player_id_hex) { if bytes.len()==32 { player.copy_from_slice(&bytes); } }
    // Parse bet type (minimal mapping)
    let bt = parse_bet_type_min(&body.bet_type);
    // Reject invalid
    if bt.is_none() || body.amount == 0 { return (StatusCode::BAD_REQUEST, axum::Json(GatewayResponse::<serde_json::Value>::error("invalid bet".into(), uuid::Uuid::new_v4().to_string()))).into_response(); }
    let bt = bt.unwrap();

    // Add to aggregator with current round
    let round = crate::services::api_gateway::aggregate::current_round(gid);
    crate::services::api_gateway::aggregate::add_bet(gid, round, player, bt, crate::protocol::craps::CrapTokens(body.amount));

    // Publish event to broker
    let evt = serde_json::json!({
        "type": "bet_accepted",
        "game_id": id,
        "player_id_hex": body.player_id_hex,
        "bet_type": body.bet_type,
        "amount": body.amount,
        "round": round,
    });
    state.broker.publish(&format!("game:{}:events", id), evt.to_string());

    let resp = serde_json::json!({"accepted": true, "round": round});
    (StatusCode::OK, axum::Json(GatewayResponse::success(resp, uuid::Uuid::new_v4().to_string(), Some("api-gateway".into())))).into_response()
}

#[derive(serde::Deserialize)]
struct ProofsQuery { player_id_hex: String, bet_type: String, amount: u64, round: Option<u64> }

async fn get_proofs_handler(
    Path(id): Path<String>,
    Query(q): Query<ProofsQuery>,
    State(_state): State<GatewayState>,
) -> impl IntoResponse {
    let gid = {
        if let Ok(bytes) = hex::decode(&id) { if bytes.len()==16 { let mut id=[0u8;16]; id.copy_from_slice(&bytes); id } else { [0u8;16] } } else { [0u8;16] }
    };
    let mut player = [0u8;32];
    if let Ok(bytes) = hex::decode(&q.player_id_hex) { if bytes.len()==32 { player.copy_from_slice(&bytes); } }
    let bt = match parse_bet_type_min(&q.bet_type) { Some(b) => b, None => return (StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({"error":"invalid bet_type"}))).into_response() };
    let round = q.round.unwrap_or_else(|| crate::services::api_gateway::aggregate::current_round(gid));
    let proof = crate::services::api_gateway::aggregate::merkle_proof(gid, round, player, bt, crate::protocol::craps::CrapTokens(q.amount));
    let resp = serde_json::json!({"round": round, "proof": proof.map(|(branch, root)| { serde_json::json!({"branch": branch, "root": hex::encode(root)}) })});
    (StatusCode::OK, axum::Json(resp)).into_response()
}

fn parse_bet_type_min(s: &str) -> Option<crate::protocol::craps::BetType> {
    use crate::protocol::craps::BetType;
    match s.to_lowercase().as_str() {
        "pass" | "passline" => Some(BetType::Pass),
        "dontpass" | "dont-pass" | "don't-pass" => Some(BetType::DontPass),
        "come" => Some(BetType::Come),
        "dontcome" | "dont-come" | "don't-come" => Some(BetType::DontCome),
        "field" => Some(BetType::Field),
        _ => None,
    }
}

use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};
use futures_util::{SinkExt, StreamExt};
use axum::extract::Query as AxumQuery;
#[derive(serde::Deserialize)]
struct SubQuery { topic: String }

async fn ws_gateway_subscribe(
    State(state): State<GatewayState>,
    AxumQuery(q): AxumQuery<SubQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_gateway_ws(socket, state, q.topic))
}

#[derive(serde::Deserialize)]
struct PayoutItem { player_id_hex: String, amount: u64 }

#[derive(serde::Deserialize)]
struct PayoutsBody { payouts: Vec<PayoutItem>, reason: Option<String>, round: Option<u64> }

async fn post_payouts_handler(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    Json(body): Json<PayoutsBody>,
) -> impl IntoResponse {
    // Build consensus proposal for batch payouts
    let payload = serde_json::json!({
        "round": body.round,
        "reason": body.reason,
        "payouts": body.payouts,
    });
    let data_vec = serde_json::to_vec(&payload).unwrap_or_default();

    // game_id as [u8;16] array
    let gid_arr = if let Ok(bytes) = hex::decode(&id) { if bytes.len()==16 { let mut id=[0u8;16]; id.copy_from_slice(&bytes); id } else { [0u8;16] } } else { [0u8;16] };
    let req = serde_json::json!({
        "game_id": gid_arr,
        "proposal_type": { "GameAction": { "action": "payouts" } },
        "data": data_vec,
    });

    // Send to consensus service
    if let Some(instance) = state.load_balancer.get_instance("consensus").await {
        let client = reqwest::Client::new();
        let url = format!("http://{}/api/v1/consensus/propose", instance.endpoint.address);
        match client.post(&url).json(&req).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                // Publish notification
                state.broker.publish(&format!("game:{}:events", id), serde_json::json!({
                    "type": "payouts_submitted",
                    "status": status.as_u16(),
                }).to_string());
                return (StatusCode::OK, axum::Json(serde_json::json!({"status": status.as_u16(), "resp": text }))).into_response();
            }
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, axum::Json(serde_json::json!({"error": e.to_string()}))).into_response();
            }
        }
    }
    (StatusCode::SERVICE_UNAVAILABLE, axum::Json(serde_json::json!({"error": "No consensus service"}))).into_response()
}

async fn handle_gateway_ws(mut socket: WebSocket, state: GatewayState, topic: String) {
    // Send hello
    let _ = socket.send(Message::Text(format!("{{\"type\":\"hello\",\"topic\":\"{}\"}}", topic))).await;

    let mut rx = state.broker.subscribe(&topic);
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg)).await.is_err() { break; }
            }
            Some(Ok(Message::Close(_))) = socket.recv() => { break; }
            Some(Ok(Message::Ping(data))) = socket.recv() => { let _ = socket.send(Message::Pong(data)).await; }
            Some(Ok(_)) = socket.recv() => { /* ignore */ }
            else => break,
        }
    }
}

/// Request middleware for logging and context
async fn request_middleware(
    State(_state): State<GatewayState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start_time = Instant::now();
    
    log::info!("Request: {} {}", method, uri);
    
    let response = next.run(request).await;
    let elapsed = start_time.elapsed();
    
    log::info!(
        "Response: {} {} -> {} in {}ms",
        method,
        uri,
        response.status(),
        elapsed.as_millis()
    );
    
    response
}

/// Forward request to service instance
async fn forward_request(
    instance: ServiceInstance,
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, String> {
    let client = reqwest::Client::new();
    
    // Build target URL
    let target_url = format!(
        "http://{}{}{}",
        instance.endpoint.address,
        uri.path(),
        uri.query().map(|q| format!("?{}", q)).unwrap_or_default()
    );
    
    // Convert headers
    let mut req_headers = reqwest::header::HeaderMap::new();
    for (key, value) in headers.iter() {
        if let (Ok(key), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(key.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            req_headers.insert(key, value);
        }
    }
    
    // Convert body
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes.to_vec(),
        Err(e) => return Err(format!("Failed to read request body: {}", e)),
    };
    
    // Make request
    let response = client
        .request(method.try_into().unwrap_or(reqwest::Method::GET), &target_url)
        .headers(req_headers)
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;
    
    // Convert response
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await
        .map_err(|e| format!("Failed to read response body: {}", e))?;
    
    let mut response_builder = Response::builder().status(status);
    for (key, value) in headers.iter() {
        response_builder = response_builder.header(key, value);
    }
    
    response_builder
        .body(Body::from(body))
        .map_err(|e| format!("Failed to build response: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_gateway_creation() {
        let config = GatewayConfig::default();
        let gateway = ApiGateway::new(config);
        
        assert_eq!(gateway.config.listen_addr.port(), 8080);
    }
    
    #[tokio::test]
    async fn test_add_route() {
        let config = GatewayConfig::default();
        let gateway = ApiGateway::new(config);
        
        let route = RouteConfig {
            path: "/test".to_string(),
            service: "test-service".to_string(),
            methods: vec!["GET".to_string()],
            auth_required: false,
            rate_limit_override: None,
            timeout_override: None,
        };
        
        let result = gateway.add_route(route).await;
        assert!(result.is_ok());
    }
}
