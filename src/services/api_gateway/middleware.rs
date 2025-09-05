//! API Gateway Middleware
//!
//! Authentication, rate limiting, and other middleware components.

use super::*;
use crate::error::{Error, Result};
use crate::protocol::PeerId;
use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use axum::http::HeaderMap;

/// Rate limiting middleware
pub struct RateLimitMiddleware {
    config: RateLimitConfig,
    ip_buckets: Arc<DashMap<IpAddr, TokenBucket>>,
    api_key_buckets: Arc<DashMap<String, TokenBucket>>,
}

impl RateLimitMiddleware {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            ip_buckets: Arc::new(DashMap::new()),
            api_key_buckets: Arc::new(DashMap::new()),
        }
    }
    
    pub async fn check_rate_limit(
        &self,
        context: &RequestContext,
        override_limit: u32,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        let limit = override_limit.max(1); // Ensure we always have at least 1
        
        // Check IP-based rate limiting
        if self.config.by_ip {
            let mut bucket = self.ip_buckets
                .entry(context.client_ip)
                .or_insert_with(|| TokenBucket::new(limit, self.config.window));
            
            if !bucket.try_consume() {
                return Err(Error::RateLimitExceeded("API rate limit exceeded".to_string()));
            }
        }
        
        // Check API key-based rate limiting
        if self.config.by_api_key {
            if let Some(api_key) = &context.api_key {
                let mut bucket = self.api_key_buckets
                    .entry(api_key.clone())
                    .or_insert_with(|| TokenBucket::new(limit, self.config.window));
                
                if !bucket.try_consume() {
                    return Err(Error::RateLimitExceeded("API rate limit exceeded".to_string()));
                }
            }
        }
        
        Ok(())
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    capacity: u32,
    tokens: AtomicU64,
    last_refill: parking_lot::Mutex<Instant>,
    refill_interval: Duration,
}

impl TokenBucket {
    fn new(capacity: u32, window: Duration) -> Self {
        Self {
            capacity,
            tokens: AtomicU64::new(capacity as u64),
            last_refill: parking_lot::Mutex::new(Instant::now()),
            refill_interval: window,
        }
    }
    
    fn try_consume(&self) -> bool {
        self.refill_tokens();
        
        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current == 0 {
                return false;
            }
            
            if self.tokens.compare_exchange_weak(
                current,
                current - 1,
                Ordering::Release,
                Ordering::Relaxed,
            ).is_ok() {
                return true;
            }
        }
    }
    
    fn refill_tokens(&self) {
        let now = Instant::now();
        let mut last_refill = self.last_refill.lock();
        
        if now.duration_since(*last_refill) >= self.refill_interval {
            self.tokens.store(self.capacity as u64, Ordering::Release);
            *last_refill = now;
        }
    }
}

/// Authentication middleware
pub struct AuthMiddleware {
    config: AuthConfig,
}

impl AuthMiddleware {
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }
    
    pub async fn authenticate(
        &self,
        headers: &HeaderMap,
        context: &mut RequestContext,
    ) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        // Try API key authentication first
        if let Some(api_key) = self.extract_api_key(headers) {
            if let Some(key_info) = self.config.api_keys.get(&api_key) {
                // Check if key is expired
                if let Some(expires_at) = key_info.expires_at {
                    if SystemTime::now() > expires_at {
                        return Err(Error::AuthenticationFailed("API key expired".to_string()));
                    }
                }
                
                context.api_key = Some(api_key);
                context.peer_id = Some(key_info.peer_id);
                return Ok(());
            }
        }
        
        // Try JWT authentication
        if let Some(token) = self.extract_bearer_token(headers) {
            match self.validate_jwt(&token) {
                Ok(claims) => {
                    context.peer_id = claims.peer_id;
                    return Ok(());
                },
                Err(e) => {
                    return Err(Error::AuthenticationFailed(format!("Invalid JWT: {}", e)));
                }
            }
        }
        
        Err(Error::AuthenticationFailed("No valid authentication provided".to_string()))
    }
    
    fn extract_api_key(&self, headers: &HeaderMap) -> Option<String> {
        headers.get("x-api-key")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }
    
    fn extract_bearer_token(&self, headers: &HeaderMap) -> Option<String> {
        headers.get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string())
    }
    
    fn validate_jwt(&self, _token: &str) -> Result<JwtClaims> {
        // NOTE: JWT validation is intentionally disabled for security
        //       All authentication attempts will fail until proper JWT
        //       validation library integration is implemented
        Err(Error::AuthenticationFailed("JWT validation not implemented".to_string()))
    }
}

/// JWT claims structure
#[derive(Debug, Clone)]
struct JwtClaims {
    peer_id: Option<PeerId>,
    expires_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_token_bucket() {
        let bucket = TokenBucket::new(5, Duration::from_secs(1));
        
        // Should be able to consume 5 tokens
        for _ in 0..5 {
            assert!(bucket.try_consume());
        }
        
        // Should be rate limited after consuming all tokens
        assert!(!bucket.try_consume());
    }
    
    #[tokio::test]
    async fn test_rate_limit_middleware() {
        let config = RateLimitConfig {
            max_requests: 3,
            window: Duration::from_secs(60),
            enabled: true,
            by_ip: true,
            by_api_key: false,
        };
        
        let middleware = RateLimitMiddleware::new(config);
        let context = RequestContext::new("192.168.1.100".parse().unwrap());
        
        // Should allow first 3 requests
        for _ in 0..3 {
            assert!(middleware.check_rate_limit(&context, 3).await.is_ok());
        }
        
        // Should block 4th request
        assert!(middleware.check_rate_limit(&context, 3).await.is_err());
    }
    
    #[tokio::test]
    async fn test_auth_middleware_api_key() {
        let mut api_keys = std::collections::HashMap::new();
        api_keys.insert("test-key".to_string(), ApiKeyInfo {
            peer_id: [0u8; 32],
            permissions: vec!["read".to_string()],
            rate_limit_override: None,
            expires_at: None,
        });
        
        let config = AuthConfig {
            enabled: true,
            jwt_secret: "test-secret-for-unit-tests".to_string(), // OK for tests
            token_expiration: Duration::from_secs(3600),
            api_keys,
        };
        
        let middleware = AuthMiddleware::new(config);
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "test-key".parse().unwrap());
        
        let mut context = RequestContext::new("192.168.1.100".parse().unwrap());
        
        let result = middleware.authenticate(&headers, &mut context).await;
        assert!(result.is_ok());
        assert!(context.api_key.is_some());
    }
}