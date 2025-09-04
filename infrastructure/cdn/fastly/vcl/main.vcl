# Fastly VCL Configuration for BitCraps CDN
# Optimized for gaming content delivery and WASM performance

vcl 4.0;

import std;
import h2;
import uuid;

# Backend configurations
backend origin_s3 {
  .host = "bitcraps-assets-prod.s3.amazonaws.com";
  .port = "443";
  .use_ssl = true;
  .ssl_check_cert = true;
  .ssl_sni_hostname = "bitcraps-assets-prod.s3.amazonaws.com";
  .first_byte_timeout = 15s;
  .connect_timeout = 10s;
  .between_bytes_timeout = 10s;
  .max_connections = 200;
  .probe = {
    .url = "/health.html";
    .timeout = 5s;
    .interval = 30s;
    .window = 3;
    .threshold = 2;
  }
}

backend api_backend {
  .host = "api.bitcraps.io";
  .port = "443";
  .use_ssl = true;
  .ssl_check_cert = true;
  .ssl_sni_hostname = "api.bitcraps.io";
  .first_byte_timeout = 30s;
  .connect_timeout = 5s;
  .between_bytes_timeout = 10s;
  .max_connections = 100;
  .probe = {
    .url = "/health";
    .timeout = 5s;
    .interval = 15s;
    .window = 3;
    .threshold = 2;
  }
}

backend game_backend {
  .host = "game.bitcraps.io";
  .port = "443";
  .use_ssl = true;
  .ssl_check_cert = true;
  .ssl_sni_hostname = "game.bitcraps.io";
  .first_byte_timeout = 10s;
  .connect_timeout = 3s;
  .between_bytes_timeout = 5s;
  .max_connections = 150;
  .probe = {
    .url = "/status";
    .timeout = 3s;
    .interval = 10s;
    .window = 3;
    .threshold = 2;
  }
}

# Rate limiting table
table rate_limit_table {
  "default": "100",
  "premium": "500",
  "api": "1000"
}

# Geographic blocking table
table geo_block_table {
  "CN": "blocked",
  "RU": "blocked",
  "KP": "blocked"
}

# Cache TTL configuration
table cache_ttl_table {
  "wasm": "604800",      # 7 days
  "static": "86400",     # 24 hours
  "api": "300",          # 5 minutes
  "game": "60",          # 1 minute
  "html": "3600"         # 1 hour
}

# Content type mapping
table content_type_table {
  "wasm": "application/wasm",
  "js": "application/javascript",
  "css": "text/css",
  "json": "application/json",
  "html": "text/html",
  "ico": "image/x-icon"
}

sub vcl_recv {
  # Set client identification
  set req.http.X-Client-IP = client.ip;
  set req.http.X-Forwarded-For = client.ip;
  
  # Geographic restrictions
  if (table.lookup(geo_block_table, client.geo.country_code) == "blocked") {
    error 403 "Access denied from your location";
  }
  
  # Security: Block suspicious user agents
  if (req.http.User-Agent ~ "(?i)(bot|crawl|spider|scrape)" && 
      req.http.User-Agent !~ "(?i)bitcraps") {
    error 403 "Blocked user agent";
  }
  
  # Security: Validate request size
  if (req.http.Content-Length ~ "^\d+$" && 
      std.integer(req.http.Content-Length, 0) > 10485760) {
    error 413 "Request entity too large";
  }
  
  # Route selection based on URL path
  if (req.url ~ "^/api/") {
    set req.backend = api_backend;
    set req.http.X-Backend = "api";
    
    # API rate limiting
    if (table.lookup(rate_limit_table, "api", "1000") && 
        ratelimit.check_rate(req.http.X-Client-IP, 100, 60s, 200, 3600s)) {
      error 429 "Too Many Requests";
    }
  }
  else if (req.url ~ "^/game/" || req.url ~ "^/ws/") {
    set req.backend = game_backend;
    set req.http.X-Backend = "game";
    
    # Game rate limiting (more permissive)
    if (ratelimit.check_rate(req.http.X-Client-IP, 200, 60s, 500, 3600s)) {
      error 429 "Too Many Requests";
    }
  }
  else {
    set req.backend = origin_s3;
    set req.http.X-Backend = "static";
    
    # Static content rate limiting
    if (ratelimit.check_rate(req.http.X-Client-IP, 100, 60s, 200, 3600s)) {
      error 429 "Too Many Requests";
    }
  }
  
  # Handle WASM files specially
  if (req.url ~ "\.wasm(\?|$)") {
    set req.http.X-Content-Type = "wasm";
    
    # Add COOP/COEP headers for WASM
    set req.http.X-WASM-Request = "true";
    
    # Remove query parameters that don't affect content
    set req.url = regsub(req.url, "\?.*$", "");
  }
  
  # Normalize requests for better caching
  if (req.url ~ "\.(css|js|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)(\?.*)?$") {
    # Remove tracking parameters
    set req.url = regsuball(req.url, "[?&](utm_[^&]*|fbclid|gclid|ref|source)", "");
    set req.url = regsub(req.url, "[\?&]$", "");
    
    # Set content type hint
    if (req.url ~ "\.css") { set req.http.X-Content-Type = "css"; }
    else if (req.url ~ "\.js") { set req.http.X-Content-Type = "js"; }
    else if (req.url ~ "\.(png|jpg|jpeg|gif|svg|ico)") { set req.http.X-Content-Type = "image"; }
  }
  
  # Handle OPTIONS preflight requests
  if (req.method == "OPTIONS") {
    return(pass);
  }
  
  # Only cache GET and HEAD requests
  if (req.method != "GET" && req.method != "HEAD") {
    return(pass);
  }
  
  # Don't cache authenticated requests
  if (req.http.Authorization || req.http.Cookie ~ "session|auth|token") {
    return(pass);
  }
  
  # Lookup in cache
  return(lookup);
}

sub vcl_hash {
  # Include relevant headers in cache key
  if (req.http.X-Backend == "api") {
    hash_data(req.http.Accept);
    hash_data(req.http.Accept-Encoding);
  }
  
  # Include device type for responsive content
  if (req.http.User-Agent ~ "(?i)mobile|android|iphone|ipad") {
    hash_data("mobile");
  } else {
    hash_data("desktop");
  }
}

sub vcl_backend_request {
  # Add useful headers to backend request
  set bereq.http.X-Forwarded-For = req.http.X-Client-IP;
  set bereq.http.X-Real-IP = req.http.X-Client-IP;
  set bereq.http.X-Request-ID = uuid.uuid_v4();
  
  # Add geographic info
  set bereq.http.X-Country = client.geo.country_code;
  set bereq.http.X-Region = client.geo.region;
  
  # Remove Fastly-specific headers
  unset bereq.http.X-Varnish;
  unset bereq.http.X-Forwarded-Host;
  
  # For S3, add proper host header
  if (req.http.X-Backend == "static") {
    set bereq.http.Host = "bitcraps-assets-prod.s3.amazonaws.com";
  }
}

sub vcl_backend_response {
  # Set cache TTL based on content type
  if (beresp.http.Content-Type ~ "application/wasm") {
    set beresp.ttl = 7d;
    set beresp.grace = 1h;
    set beresp.http.Cache-Control = "public, max-age=604800, immutable";
  }
  else if (beresp.http.Content-Type ~ "(text/css|application/javascript|image/)") {
    set beresp.ttl = 1d;
    set beresp.grace = 1h;
    set beresp.http.Cache-Control = "public, max-age=86400";
  }
  else if (bereq.url ~ "^/api/" && beresp.status == 200) {
    if (beresp.http.Cache-Control ~ "public") {
      set beresp.ttl = 5m;
      set beresp.grace = 1m;
    } else {
      set beresp.ttl = 0s;  # Don't cache private API responses
    }
  }
  else if (bereq.url ~ "^/game/" && beresp.status == 200) {
    set beresp.ttl = 1m;
    set beresp.grace = 30s;
  }
  else if (beresp.http.Content-Type ~ "text/html") {
    set beresp.ttl = 1h;
    set beresp.grace = 30s;
    set beresp.http.Cache-Control = "public, max-age=3600";
  }
  
  # Enable ESI for dynamic content
  if (beresp.http.Content-Type ~ "text/html") {
    set beresp.do_esi = true;
  }
  
  # Compression settings
  if (beresp.http.Content-Type ~ "(text/|application/json|application/javascript|application/wasm|image/svg)") {
    set beresp.http.Vary = "Accept-Encoding";
  }
  
  # Add cache tags for purging
  if (bereq.url ~ "\.wasm$") {
    set beresp.http.Surrogate-Key = "wasm";
  } else if (bereq.url ~ "\.(css|js)$") {
    set beresp.http.Surrogate-Key = "assets";
  } else if (bereq.url ~ "^/api/") {
    set beresp.http.Surrogate-Key = "api";
  } else if (bereq.url ~ "^/game/") {
    set beresp.http.Surrogate-Key = "game";
  }
  
  # Security headers
  set beresp.http.X-Content-Type-Options = "nosniff";
  set beresp.http.X-Frame-Options = "DENY";
  set beresp.http.X-XSS-Protection = "1; mode=block";
  
  # Remove backend information
  unset beresp.http.Server;
  unset beresp.http.X-Powered-By;
  unset beresp.http.Via;
  
  return(deliver);
}

sub vcl_deliver {
  # Add security headers
  set resp.http.Strict-Transport-Security = "max-age=31536000; includeSubDomains";
  set resp.http.Referrer-Policy = "strict-origin-when-cross-origin";
  
  # CORS headers for API and WASM
  if (req.url ~ "^/api/" || req.url ~ "\.wasm$") {
    set resp.http.Access-Control-Allow-Origin = "https://bitcraps.io";
    set resp.http.Access-Control-Allow-Methods = "GET, POST, PUT, DELETE, OPTIONS";
    set resp.http.Access-Control-Allow-Headers = "Content-Type, Authorization";
    set resp.http.Access-Control-Max-Age = "86400";
  }
  
  # WASM-specific headers
  if (req.http.X-WASM-Request) {
    set resp.http.Cross-Origin-Embedder-Policy = "require-corp";
    set resp.http.Cross-Origin-Opener-Policy = "same-origin";
    set resp.http.Content-Type = "application/wasm";
  }
  
  # CSP header for HTML content
  if (resp.http.Content-Type ~ "text/html") {
    set resp.http.Content-Security-Policy = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' wss: https:; font-src 'self' data:; object-src 'none'; media-src 'self'; frame-src 'none';";
  }
  
  # Performance headers
  set resp.http.X-Served-By = server.hostname;
  set resp.http.X-Cache = obj.hits > 0 ? "HIT" : "MISS";
  set resp.http.X-Cache-Hits = obj.hits;
  
  # Analytics headers
  set resp.http.X-Request-ID = bereq.http.X-Request-ID;
  set resp.http.X-Edge-Location = server.datacenter;
  
  # Remove internal headers
  unset resp.http.X-Backend;
  unset resp.http.X-Content-Type;
  unset resp.http.X-WASM-Request;
  unset resp.http.Surrogate-Key;
  
  # Add timing information
  set resp.http.Server-Timing = "edge;dur=" + time.elapsed.msec;
  
  return(deliver);
}

sub vcl_backend_error {
  # Custom error pages
  if (beresp.status == 503 && bereq.restarts < 1) {
    return(restart);
  }
  
  synthetic(std.fileread("/etc/fastly/error-" + beresp.status + ".html"));
  set beresp.http.Content-Type = "text/html";
  set beresp.status = 503;
  return(deliver);
}

sub vcl_error {
  # Rate limit error response
  if (obj.status == 429) {
    synthetic({"
<!DOCTYPE html>
<html>
<head>
    <title>Rate Limited</title>
    <style>body { font-family: Arial, sans-serif; text-align: center; margin-top: 100px; }</style>
</head>
<body>
    <h1>Too Many Requests</h1>
    <p>Please wait a moment before trying again.</p>
    <p>If you believe this is an error, please contact support.</p>
</body>
</html>
    "});
    set obj.http.Content-Type = "text/html";
    set obj.http.Retry-After = "60";
    return(deliver);
  }
  
  # Geographic block error
  if (obj.status == 403) {
    synthetic({"
<!DOCTYPE html>
<html>
<head>
    <title>Access Denied</title>
    <style>body { font-family: Arial, sans-serif; text-align: center; margin-top: 100px; }</style>
</head>
<body>
    <h1>Access Denied</h1>
    <p>Access from your location is not permitted.</p>
</body>
</html>
    "});
    set obj.http.Content-Type = "text/html";
    return(deliver);
  }
}