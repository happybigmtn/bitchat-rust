# AWS CloudFront Additional Distributions Configuration
# Specialized distributions for different content types and regions

# Global WASM distribution with optimized caching
resource "aws_cloudfront_distribution" "wasm_distribution" {
  comment         = "BitCraps WASM Global Distribution - ${var.environment}"
  enabled         = true
  is_ipv6_enabled = true
  price_class     = "PriceClass_All"  # Global distribution for WASM
  
  # WASM-optimized origin
  origin {
    domain_name              = aws_s3_bucket.cdn_origin.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.cdn_oac.id
    origin_id                = "S3-WASM-Origin"
    origin_path              = "/wasm"
    
    custom_header {
      name  = "X-Content-Type"
      value = "application/wasm"
    }
  }
  
  # WASM-specific cache behavior
  default_cache_behavior {
    target_origin_id       = "S3-WASM-Origin"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD", "OPTIONS"]
    cached_methods         = ["GET", "HEAD", "OPTIONS"]
    compress               = false  # WASM is already compressed
    
    cache_policy_id            = aws_cloudfront_cache_policy.wasm_optimized.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.cors_enabled.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.wasm_headers.id
  }
  
  aliases = ["wasm.${var.domain_name}"]
  
  viewer_certificate {
    acm_certificate_arn      = var.ssl_certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
  
  restrictions {
    geo_restriction {
      restriction_type = var.geo_restrictions.restriction_type
      locations        = var.geo_restrictions.locations
    }
  }
  
  tags = merge(local.common_tags, {
    Purpose = "WASM Content Delivery"
  })
}

# API-specific distribution with edge lambda
resource "aws_cloudfront_distribution" "api_distribution" {
  comment         = "BitCraps API Distribution - ${var.environment}"
  enabled         = true
  is_ipv6_enabled = true
  price_class     = var.price_class
  web_acl_id      = var.enable_waf ? aws_wafv2_web_acl.cdn_waf[0].arn : null
  
  # API origin
  origin {
    domain_name = "api.${var.domain_name}"
    origin_id   = "API-Backend"
    
    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
    
    custom_header {
      name  = "X-Origin-Verify"
      value = "cloudfront-api"
    }
  }
  
  # API cache behavior with authentication support
  default_cache_behavior {
    target_origin_id       = "API-Backend"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD", "OPTIONS"]
    compress               = true
    
    cache_policy_id            = aws_cloudfront_cache_policy.api_cache.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.api_forwarding.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.api_headers.id
    
    # Lambda@Edge for API authentication and routing
    dynamic "lambda_function_association" {
      for_each = var.enable_api_lambda ? [1] : []
      content {
        event_type   = "viewer-request"
        lambda_arn   = aws_lambda_function.api_auth_lambda[0].qualified_arn
        include_body = false
      }
    }
  }
  
  aliases = ["api.${var.domain_name}"]
  
  viewer_certificate {
    acm_certificate_arn      = var.ssl_certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
  
  restrictions {
    geo_restriction {
      restriction_type = var.geo_restrictions.restriction_type
      locations        = var.geo_restrictions.locations
    }
  }
  
  # Enhanced logging for API distribution
  logging_config {
    include_cookies = true
    bucket          = "${var.log_bucket_name}.s3.amazonaws.com"
    prefix          = "api-logs/${var.environment}/"
  }
  
  tags = merge(local.common_tags, {
    Purpose = "API Content Delivery"
  })
}

# Game-specific distribution optimized for real-time updates
resource "aws_cloudfront_distribution" "game_distribution" {
  comment         = "BitCraps Game Distribution - ${var.environment}"
  enabled         = true
  is_ipv6_enabled = true
  price_class     = "PriceClass_200"  # Focus on major regions for gaming
  
  # Game backend origin
  origin {
    domain_name = "game.${var.domain_name}"
    origin_id   = "Game-Backend"
    
    custom_origin_config {
      http_port                = 80
      https_port               = 443
      origin_protocol_policy   = "https-only"
      origin_ssl_protocols     = ["TLSv1.2"]
      origin_keepalive_timeout = 60
      origin_read_timeout      = 60
    }
    
    custom_header {
      name  = "X-Game-CDN"
      value = "cloudfront"
    }
  }
  
  # Game state cache behavior - minimal caching for real-time data
  default_cache_behavior {
    target_origin_id       = "Game-Backend"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD"]
    compress               = true
    
    cache_policy_id            = aws_cloudfront_cache_policy.game_cache.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.game_forwarding.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.game_headers.id
  }
  
  # WebSocket behavior - pass through without caching
  ordered_cache_behavior {
    path_pattern           = "/ws/*"
    target_origin_id       = "Game-Backend"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"]
    cached_methods         = ["GET", "HEAD"]
    compress               = false
    
    cache_policy_id            = aws_cloudfront_cache_policy.no_cache.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.websocket_forwarding.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.websocket_headers.id
  }
  
  aliases = ["game.${var.domain_name}"]
  
  viewer_certificate {
    acm_certificate_arn      = var.ssl_certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
  
  restrictions {
    geo_restriction {
      restriction_type = var.geo_restrictions.restriction_type
      locations        = var.geo_restrictions.locations
    }
  }
  
  tags = merge(local.common_tags, {
    Purpose = "Game Content Delivery"
  })
}

# Regional distribution for static assets
resource "aws_cloudfront_distribution" "regional_assets" {
  count = var.enable_regional_distributions ? 1 : 0
  
  comment         = "BitCraps Regional Assets - ${var.environment}"
  enabled         = true
  is_ipv6_enabled = true
  price_class     = "PriceClass_100"  # Regional optimization
  
  # Regional origin with origin shield
  origin {
    domain_name              = aws_s3_bucket.cdn_origin.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.cdn_oac.id
    origin_id                = "S3-Regional-Assets"
    origin_path              = "/assets"
    
    origin_shield {
      enabled              = var.origin_shield_enabled
      origin_shield_region = var.aws_primary_region
    }
  }
  
  # Optimized for static assets
  default_cache_behavior {
    target_origin_id       = "S3-Regional-Assets"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    compress               = true
    
    cache_policy_id            = aws_cloudfront_cache_policy.static_assets.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.static_forwarding.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.security_headers.id
  }
  
  aliases = ["assets.${var.domain_name}"]
  
  viewer_certificate {
    acm_certificate_arn      = var.ssl_certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
  
  restrictions {
    geo_restriction {
      restriction_type = var.geo_restrictions.restriction_type
      locations        = var.geo_restrictions.locations
    }
  }
  
  tags = merge(local.common_tags, {
    Purpose = "Regional Asset Delivery"
  })
}

# Additional cache policies for specialized distributions
resource "aws_cloudfront_cache_policy" "game_cache" {
  name        = "bitcraps-game-cache-${var.environment}"
  comment     = "Game-optimized cache policy with minimal TTL"
  default_ttl = var.cache_ttl_settings.game_data
  max_ttl     = var.cache_ttl_settings.game_data * 2
  min_ttl     = 0
  
  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_gzip   = true
    enable_accept_encoding_brotli = true
    
    query_strings_config {
      query_string_behavior = "all"
    }
    
    headers_config {
      header_behavior = "whitelist"
      headers {
        items = [
          "Authorization",
          "X-Game-Session",
          "X-Player-ID",
          "Accept"
        ]
      }
    }
    
    cookies_config {
      cookie_behavior = "whitelist"
      cookies {
        items = ["game_session", "player_auth"]
      }
    }
  }
}

resource "aws_cloudfront_cache_policy" "no_cache" {
  name        = "bitcraps-no-cache-${var.environment}"
  comment     = "No caching policy for real-time content"
  default_ttl = 0
  max_ttl     = 0
  min_ttl     = 0
  
  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_gzip   = false
    enable_accept_encoding_brotli = false
    
    query_strings_config {
      query_string_behavior = "all"
    }
    
    headers_config {
      header_behavior = "allViewer"
    }
    
    cookies_config {
      cookie_behavior = "all"
    }
  }
}

# WASM-specific response headers policy
resource "aws_cloudfront_response_headers_policy" "wasm_headers" {
  name    = "bitcraps-wasm-headers-${var.environment}"
  comment = "Headers optimized for WASM content"
  
  cors_config {
    access_control_allow_credentials = false
    access_control_allow_headers {
      items = ["*"]
    }
    access_control_allow_methods {
      items = ["GET", "HEAD", "OPTIONS"]
    }
    access_control_allow_origins {
      items = ["*"]
    }
    origin_override = false
  }
  
  security_headers_config {
    strict_transport_security {
      access_control_max_age_sec = 31536000
      include_subdomains         = true
      override                   = false
    }
    
    content_type_options {
      override = false
    }
    
    frame_options {
      frame_option = "DENY"
      override     = false
    }
  }
  
  custom_headers_config {
    items {
      header   = "Cross-Origin-Embedder-Policy"
      value    = "require-corp"
      override = false
    }
    items {
      header   = "Cross-Origin-Opener-Policy"
      value    = "same-origin"
      override = false
    }
    items {
      header   = "Content-Type"
      value    = "application/wasm"
      override = true
    }
  }
}

# Game-specific headers policy
resource "aws_cloudfront_response_headers_policy" "game_headers" {
  name    = "bitcraps-game-headers-${var.environment}"
  comment = "Headers for game content delivery"
  
  cors_config {
    access_control_allow_credentials = true
    access_control_allow_headers {
      items = ["Authorization", "Content-Type", "X-Game-Session", "X-Player-ID"]
    }
    access_control_allow_methods {
      items = ["GET", "HEAD", "OPTIONS", "PUT", "PATCH", "POST", "DELETE"]
    }
    access_control_allow_origins {
      items = ["https://${var.domain_name}"]
    }
    access_control_max_age_sec = 300  # Short cache for game headers
    origin_override           = false
  }
  
  security_headers_config {
    strict_transport_security {
      access_control_max_age_sec = 31536000
      include_subdomains         = true
      override                   = false
    }
  }
  
  custom_headers_config {
    items {
      header   = "X-Content-Type-Options"
      value    = "nosniff"
      override = false
    }
  }
}

# WebSocket headers policy
resource "aws_cloudfront_response_headers_policy" "websocket_headers" {
  name    = "bitcraps-websocket-headers-${var.environment}"
  comment = "Headers for WebSocket upgrade requests"
  
  custom_headers_config {
    items {
      header   = "Upgrade"
      value    = "websocket"
      override = false
    }
    items {
      header   = "Connection"
      value    = "Upgrade"
      override = false
    }
  }
}

# Additional origin request policies
resource "aws_cloudfront_origin_request_policy" "game_forwarding" {
  name    = "bitcraps-game-forwarding-${var.environment}"
  comment = "Forward game-specific headers and data"
  
  query_strings_config {
    query_string_behavior = "all"
  }
  
  headers_config {
    header_behavior = "allViewerAndWhitelistCloudFront"
    headers {
      items = [
        "CloudFront-Viewer-Address",
        "CloudFront-Viewer-Country",
        "X-Game-Session",
        "X-Player-ID"
      ]
    }
  }
  
  cookies_config {
    cookie_behavior = "all"
  }
}

resource "aws_cloudfront_origin_request_policy" "websocket_forwarding" {
  name    = "bitcraps-websocket-forwarding-${var.environment}"
  comment = "Forward all headers for WebSocket connections"
  
  query_strings_config {
    query_string_behavior = "all"
  }
  
  headers_config {
    header_behavior = "allViewer"
  }
  
  cookies_config {
    cookie_behavior = "all"
  }
}

# Outputs for specialized distributions
output "wasm_distribution_id" {
  description = "WASM CloudFront distribution ID"
  value       = aws_cloudfront_distribution.wasm_distribution.id
}

output "api_distribution_id" {
  description = "API CloudFront distribution ID"
  value       = aws_cloudfront_distribution.api_distribution.id
}

output "game_distribution_id" {
  description = "Game CloudFront distribution ID"
  value       = aws_cloudfront_distribution.game_distribution.id
}