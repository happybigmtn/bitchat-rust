//! Game Engine Service API
//!
//! Provides both gRPC and REST API endpoints for the game engine service.

use super::service::GameEngineService;
use super::types::*;
use crate::error::{Error, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

/// REST API implementation
pub struct GameEngineApi {
    service: Arc<GameEngineService>,
}

impl GameEngineApi {
    pub fn new(service: Arc<GameEngineService>) -> Self {
        Self { service }
    }
    
    /// Create REST API router
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/games", post(create_game_handler))
            .route("/games", get(list_games_handler))
            .route("/games/:game_id", get(get_game_state_handler))
            .route("/games/:game_id/actions", post(process_action_handler))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .into_inner(),
            )
            .with_state(self.service.clone())
    }
}

/// Health check endpoint
async fn health_handler(
    State(service): State<Arc<GameEngineService>>,
) -> std::result::Result<Json<HealthCheckResponse>, (StatusCode, String)> {
    match service.health_check().await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Create game endpoint
async fn create_game_handler(
    State(service): State<Arc<GameEngineService>>,
    Json(request): Json<CreateGameRequest>,
) -> std::result::Result<Json<CreateGameResponse>, (StatusCode, String)> {
    match service.create_game(request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => match e {
            Error::GameError(_) => Err((StatusCode::BAD_REQUEST, e.to_string())),
            _ => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        },
    }
}

/// List games endpoint
async fn list_games_handler(
    State(service): State<Arc<GameEngineService>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<Json<ListGamesResponse>, (StatusCode, String)> {
    let mut request = ListGamesRequest {
        player_id: None,
        active_only: params.get("active_only").map_or(false, |v| v == "true"),
        limit: params.get("limit").and_then(|v| v.parse().ok()),
    };
    
    if let Some(player_id_str) = params.get("player_id") {
        // Parse player ID from string (simplified for this example)
        // In production, you'd have proper UUID parsing
        request.player_id = Some(crate::protocol::PeerId::new());
    }
    
    match service.list_games(request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Get game state endpoint
async fn get_game_state_handler(
    Path(game_id): Path<String>,
    State(service): State<Arc<GameEngineService>>,
) -> std::result::Result<Json<GetGameStateResponse>, (StatusCode, String)> {
    // Parse game ID from string (simplified for this example)
    let game_id = crate::protocol::GameId::new(); // In production, parse from path
    
    let request = GetGameStateRequest { game_id };
    
    match service.get_game_state(request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => match e {
            Error::GameError(_) => Err((StatusCode::NOT_FOUND, e.to_string())),
            _ => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        },
    }
}

/// Process action endpoint
async fn process_action_handler(
    Path(game_id): Path<String>,
    State(service): State<Arc<GameEngineService>>,
    Json(mut request): Json<ProcessActionRequest>,
) -> std::result::Result<Json<ProcessActionResponse>, (StatusCode, String)> {
    // Parse game ID from path
    request.game_id = crate::protocol::GameId::new(); // In production, parse from path
    
    match service.process_action(request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => match e {
            Error::GameError(_) => Err((StatusCode::BAD_REQUEST, e.to_string())),
            _ => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        },
    }
}

/// gRPC service implementation
pub mod grpc {
    use super::*;
    use tonic::{Request, Response, Status};
    
    // Proto definitions would be generated here
    // For now, we'll define the service traits manually
    
    pub struct GameEngineGrpcService {
        service: Arc<GameEngineService>,
    }
    
    impl GameEngineGrpcService {
        pub fn new(service: Arc<GameEngineService>) -> Self {
            Self { service }
        }
    }
    
    // In a real implementation, these would be auto-generated from protobuf
    #[tonic::async_trait]
    pub trait GameEngineServer: Send + Sync + 'static {
        async fn create_game(
            &self,
            request: Request<CreateGameRequest>,
        ) -> std::result::Result<Response<CreateGameResponse>, Status>;
        
        async fn process_action(
            &self,
            request: Request<ProcessActionRequest>,
        ) -> std::result::Result<Response<ProcessActionResponse>, Status>;
        
        async fn get_game_state(
            &self,
            request: Request<GetGameStateRequest>,
        ) -> std::result::Result<Response<GetGameStateResponse>, Status>;
        
        async fn list_games(
            &self,
            request: Request<ListGamesRequest>,
        ) -> std::result::Result<Response<ListGamesResponse>, Status>;
        
        async fn health_check(
            &self,
            request: Request<()>,
        ) -> std::result::Result<Response<HealthCheckResponse>, Status>;
    }
    
    #[tonic::async_trait]
    impl GameEngineServer for GameEngineGrpcService {
        async fn create_game(
            &self,
            request: Request<CreateGameRequest>,
        ) -> std::result::Result<Response<CreateGameResponse>, Status> {
            match self.service.create_game(request.into_inner()).await {
                Ok(response) => Ok(Response::new(response)),
                Err(e) => Err(Status::internal(e.to_string())),
            }
        }
        
        async fn process_action(
            &self,
            request: Request<ProcessActionRequest>,
        ) -> std::result::Result<Response<ProcessActionResponse>, Status> {
            match self.service.process_action(request.into_inner()).await {
                Ok(response) => Ok(Response::new(response)),
                Err(e) => Err(Status::internal(e.to_string())),
            }
        }
        
        async fn get_game_state(
            &self,
            request: Request<GetGameStateRequest>,
        ) -> std::result::Result<Response<GetGameStateResponse>, Status> {
            match self.service.get_game_state(request.into_inner()).await {
                Ok(response) => Ok(Response::new(response)),
                Err(e) => Err(Status::internal(e.to_string())),
            }
        }
        
        async fn list_games(
            &self,
            request: Request<ListGamesRequest>,
        ) -> std::result::Result<Response<ListGamesResponse>, Status> {
            match self.service.list_games(request.into_inner()).await {
                Ok(response) => Ok(Response::new(response)),
                Err(e) => Err(Status::internal(e.to_string())),
            }
        }
        
        async fn health_check(
            &self,
            _request: Request<()>,
        ) -> std::result::Result<Response<HealthCheckResponse>, Status> {
            match self.service.health_check().await {
                Ok(response) => Ok(Response::new(response)),
                Err(e) => Err(Status::internal(e.to_string())),
            }
        }
    }
}

/// Rate limiting middleware
pub mod middleware {
    use axum::{
        extract::Request,
        http::{HeaderMap, StatusCode},
        middleware::Next,
        response::Response,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::RwLock;
    
    pub struct RateLimiter {
        requests: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
        max_requests: usize,
        window: Duration,
    }
    
    impl RateLimiter {
        pub fn new(max_requests: usize, window: Duration) -> Self {
            Self {
                requests: Arc::new(RwLock::new(HashMap::new())),
                max_requests,
                window,
            }
        }
        
        pub async fn check_rate_limit(
            &self,
            identifier: &str,
        ) -> std::result::Result<(), StatusCode> {
            let now = Instant::now();
            let mut requests = self.requests.write().await;
            
            let client_requests = requests.entry(identifier.to_string()).or_insert_with(Vec::new);
            
            // Remove old requests outside the window
            client_requests.retain(|&time| now.duration_since(time) < self.window);
            
            if client_requests.len() >= self.max_requests {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            
            client_requests.push(now);
            Ok(())
        }
    }
    
    pub async fn rate_limiting_middleware(
        headers: HeaderMap,
        request: Request,
        next: Next,
    ) -> std::result::Result<Response, StatusCode> {
        // In production, you'd get the rate limiter from app state
        let rate_limiter = RateLimiter::new(100, Duration::from_secs(60));
        
        // Extract client identifier (IP, API key, etc.)
        let client_id = headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        
        rate_limiter.check_rate_limit(&client_id).await?;
        
        Ok(next.run(request).await)
    }
}