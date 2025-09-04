/**
 * Cloudflare Worker for BitCraps CDN Edge Processing
 * Handles edge caching, security, and performance optimization
 */

import { Router } from 'itty-router';

// Environment configuration
const config = {
  CACHE_TTL: {
    STATIC: 86400,    // 24 hours
    WASM: 604800,     // 7 days  
    API: 300,         // 5 minutes
    GAME: 60          // 1 minute
  },
  SECURITY: {
    MAX_BODY_SIZE: 10 * 1024 * 1024, // 10MB
    RATE_LIMIT: {
      REQUESTS_PER_MINUTE: 100,
      BURST_LIMIT: 20
    }
  },
  COMPRESSION: {
    ENABLED_TYPES: [
      'text/html',
      'text/css', 
      'text/javascript',
      'application/javascript',
      'application/json',
      'application/wasm',
      'image/svg+xml'
    ]
  }
};

// Initialize router
const router = Router();

// Rate limiting using Durable Objects
class RateLimiter {
  constructor(state, env) {
    this.state = state;
    this.env = env;
  }

  async fetch(request) {
    const ip = request.headers.get('CF-Connecting-IP');
    const key = `rate_limit:${ip}`;
    const now = Date.now();
    const windowStart = now - (60 * 1000); // 1 minute window

    // Get current request count
    const current = await this.state.storage.get(key) || { count: 0, timestamps: [] };
    
    // Filter out old timestamps
    current.timestamps = current.timestamps.filter(ts => ts > windowStart);
    
    // Check rate limit
    if (current.timestamps.length >= config.SECURITY.RATE_LIMIT.REQUESTS_PER_MINUTE) {
      return new Response('Rate limit exceeded', { status: 429 });
    }
    
    // Update counter
    current.timestamps.push(now);
    current.count = current.timestamps.length;
    
    await this.state.storage.put(key, current);
    
    return new Response('OK');
  }
}

// Security headers middleware
function addSecurityHeaders(response) {
  const headers = new Headers(response.headers);
  
  headers.set('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
  headers.set('X-Content-Type-Options', 'nosniff');
  headers.set('X-Frame-Options', 'DENY');
  headers.set('X-XSS-Protection', '1; mode=block');
  headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');
  headers.set('Content-Security-Policy', 
    "default-src 'self'; " +
    "script-src 'self' 'unsafe-inline' 'unsafe-eval'; " +
    "style-src 'self' 'unsafe-inline'; " +
    "img-src 'self' data: https:; " +
    "connect-src 'self' wss: https:; " +
    "font-src 'self' data:; " +
    "object-src 'none'; " +
    "media-src 'self'; " +
    "frame-src 'none';"
  );
  
  // CORS headers for gaming APIs
  if (response.headers.get('content-type')?.includes('application/json')) {
    headers.set('Access-Control-Allow-Origin', 'https://bitcraps.io');
    headers.set('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
    headers.set('Access-Control-Allow-Headers', 'Content-Type, Authorization');
    headers.set('Access-Control-Max-Age', '86400');
  }
  
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers: headers
  });
}

// Cache key generation
function generateCacheKey(request, pathname) {
  const url = new URL(request.url);
  
  // For WASM files, include query parameters for versioning
  if (pathname.endsWith('.wasm')) {
    return `wasm:${pathname}:${url.search}`;
  }
  
  // For API requests, include relevant headers
  if (pathname.startsWith('/api/')) {
    const authHeader = request.headers.get('Authorization') || '';
    const userAgent = request.headers.get('User-Agent') || '';
    return `api:${pathname}:${url.search}:${btoa(authHeader + userAgent).slice(0, 16)}`;
  }
  
  // For static assets
  return `static:${pathname}:${url.search}`;
}

// Determine cache TTL based on content type
function getCacheTTL(pathname, contentType) {
  if (pathname.endsWith('.wasm')) {
    return config.CACHE_TTL.WASM;
  }
  
  if (pathname.startsWith('/api/')) {
    return config.CACHE_TTL.API;
  }
  
  if (pathname.startsWith('/game/') || pathname.includes('game-state')) {
    return config.CACHE_TTL.GAME;
  }
  
  return config.CACHE_TTL.STATIC;
}

// WASM optimization handler
router.get('*.wasm', async (request, env) => {
  const url = new URL(request.url);
  const cacheKey = generateCacheKey(request, url.pathname);
  
  // Check cache first
  const cache = caches.default;
  let response = await cache.match(cacheKey);
  
  if (!response) {
    // Fetch from origin
    response = await fetch(request);
    
    if (response.ok) {
      // Add WASM-specific headers
      const wasmResponse = new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers: {
          ...response.headers,
          'Content-Type': 'application/wasm',
          'Cross-Origin-Embedder-Policy': 'require-corp',
          'Cross-Origin-Opener-Policy': 'same-origin',
          'Cache-Control': `public, max-age=${config.CACHE_TTL.WASM}, immutable`
        }
      });
      
      // Cache the response
      const cacheResponse = wasmResponse.clone();
      event.waitUntil(cache.put(cacheKey, cacheResponse));
      
      return addSecurityHeaders(wasmResponse);
    }
  }
  
  return addSecurityHeaders(response);
});

// API route handler
router.all('/api/*', async (request, env) => {
  const url = new URL(request.url);
  
  // Rate limiting check
  const rateLimitId = env.RATE_LIMITER.idFromName('global');
  const rateLimiter = env.RATE_LIMITER.get(rateLimitId);
  const rateLimitResponse = await rateLimiter.fetch(request);
  
  if (rateLimitResponse.status === 429) {
    return new Response('Rate limit exceeded', { 
      status: 429,
      headers: { 'Retry-After': '60' }
    });
  }
  
  // Handle preflight requests
  if (request.method === 'OPTIONS') {
    return new Response(null, {
      status: 204,
      headers: {
        'Access-Control-Allow-Origin': 'https://bitcraps.io',
        'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type, Authorization',
        'Access-Control-Max-Age': '86400'
      }
    });
  }
  
  // For cacheable API requests (GET only)
  if (request.method === 'GET') {
    const cacheKey = generateCacheKey(request, url.pathname);
    const cache = caches.default;
    let response = await cache.match(cacheKey);
    
    if (!response) {
      response = await fetch(request);
      
      if (response.ok && response.headers.get('cache-control')?.includes('public')) {
        const cacheResponse = response.clone();
        event.waitUntil(cache.put(cacheKey, cacheResponse));
      }
    }
    
    return addSecurityHeaders(response);
  }
  
  // Pass through non-cacheable requests
  return addSecurityHeaders(await fetch(request));
});

// Game state handler
router.all('/game/*', async (request, env) => {
  const url = new URL(request.url);
  
  // WebSocket upgrade for real-time game communication
  if (request.headers.get('Upgrade') === 'websocket') {
    return await fetch(request);
  }
  
  // Short cache for game state
  const cacheKey = generateCacheKey(request, url.pathname);
  const cache = caches.default;
  let response = await cache.match(cacheKey);
  
  if (!response) {
    response = await fetch(request);
    
    if (response.ok && request.method === 'GET') {
      const gameResponse = new Response(response.body, {
        ...response,
        headers: {
          ...response.headers,
          'Cache-Control': `public, max-age=${config.CACHE_TTL.GAME}`
        }
      });
      
      const cacheResponse = gameResponse.clone();
      event.waitUntil(cache.put(cacheKey, cacheResponse));
      
      return addSecurityHeaders(gameResponse);
    }
  }
  
  return addSecurityHeaders(response);
});

// Static asset handler (default)
router.get('*', async (request, env) => {
  const url = new URL(request.url);
  const cacheKey = generateCacheKey(request, url.pathname);
  
  // Check cache
  const cache = caches.default;
  let response = await cache.match(cacheKey);
  
  if (!response) {
    // Fetch from origin
    response = await fetch(request);
    
    if (response.ok) {
      const contentType = response.headers.get('Content-Type') || '';
      const ttl = getCacheTTL(url.pathname, contentType);
      
      // Create cached response with optimized headers
      const cachedResponse = new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers: {
          ...response.headers,
          'Cache-Control': `public, max-age=${ttl}`,
          'ETag': response.headers.get('ETag') || `"${Date.now()}"`,
          'Last-Modified': response.headers.get('Last-Modified') || new Date().toUTCString()
        }
      });
      
      // Cache the response
      const cacheResponse = cachedResponse.clone();
      event.waitUntil(cache.put(cacheKey, cacheResponse));
      
      return addSecurityHeaders(cachedResponse);
    }
  }
  
  return addSecurityHeaders(response);
});

// Analytics and monitoring
async function logRequest(request, response, startTime) {
  const endTime = Date.now();
  const duration = endTime - startTime;
  
  const logData = {
    timestamp: new Date().toISOString(),
    url: request.url,
    method: request.method,
    status: response.status,
    duration: duration,
    userAgent: request.headers.get('User-Agent'),
    country: request.cf?.country,
    colo: request.cf?.colo,
    cached: response.headers.get('cf-cache-status') === 'HIT'
  };
  
  // Send to analytics endpoint (implement as needed)
  console.log('Request log:', JSON.stringify(logData));
}

// Main handler
export default {
  async fetch(request, env, ctx) {
    const startTime = Date.now();
    
    try {
      const response = await router.handle(request, env, ctx);
      
      // Log request for analytics
      ctx.waitUntil(logRequest(request, response, startTime));
      
      return response;
    } catch (error) {
      console.error('Worker error:', error);
      
      return new Response('Internal Server Error', { 
        status: 500,
        headers: { 'Content-Type': 'text/plain' }
      });
    }
  }
};

// Export Durable Object
export { RateLimiter };