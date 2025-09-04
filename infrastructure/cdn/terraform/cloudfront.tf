# AWS CloudFront Distribution Configuration for BitCraps CDN

# CloudFront cache policies for different content types
resource "aws_cloudfront_cache_policy" "wasm_optimized" {
  name        = "bitcraps-wasm-cache-policy-${var.environment}"
  comment     = "Optimized cache policy for WASM files"
  default_ttl = var.cache_ttl_settings.wasm_files
  max_ttl     = var.cache_ttl_settings.wasm_files * 4
  min_ttl     = 1

  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_gzip   = true
    enable_accept_encoding_brotli = true

    query_strings_config {
      query_string_behavior = "none"
    }

    headers_config {
      header_behavior = "whitelist"
      headers {
        items = ["Origin", "Access-Control-Request-Method", "Access-Control-Request-Headers"]
      }
    }

    cookies_config {
      cookie_behavior = "none"
    }
  }
}

resource "aws_cloudfront_cache_policy" "static_assets" {
  name        = "bitcraps-static-cache-policy-${var.environment}"
  comment     = "Cache policy for static assets (CSS, JS, images)"
  default_ttl = var.cache_ttl_settings.static_assets
  max_ttl     = var.cache_ttl_settings.static_assets * 2
  min_ttl     = 1

  parameters_in_cache_key_and_forwarded_to_origin {
    enable_accept_encoding_gzip   = true
    enable_accept_encoding_brotli = true

    query_strings_config {
      query_string_behavior = "whitelist"
      query_strings {
        items = ["v", "version", "t"]  # Version parameters
      }
    }

    headers_config {
      header_behavior = "whitelist"
      headers {
        items = ["Origin"]
      }
    }

    cookies_config {
      cookie_behavior = "none"
    }
  }
}

resource "aws_cloudfront_cache_policy" "api_cache" {
  name        = "bitcraps-api-cache-policy-${var.environment}"
  comment     = "Cache policy for API responses"
  default_ttl = var.cache_ttl_settings.api_responses
  max_ttl     = var.cache_ttl_settings.api_responses * 2
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
          "Origin",
          "Access-Control-Request-Method", 
          "Access-Control-Request-Headers",
          "Accept",
          "Accept-Language"
        ]
      }
    }

    cookies_config {
      cookie_behavior = "whitelist"
      cookies {
        items = ["session_id", "auth_token"]
      }
    }
  }
}

# Origin request policies
resource "aws_cloudfront_origin_request_policy" "cors_enabled" {
  name    = "bitcraps-cors-policy-${var.environment}"
  comment = "Origin request policy with CORS headers"

  query_strings_config {
    query_string_behavior = "all"
  }

  headers_config {
    header_behavior = "whitelist"
    headers {
      items = [
        "Origin",
        "Access-Control-Request-Method",
        "Access-Control-Request-Headers",
        "Accept",
        "Accept-Language",
        "Accept-Encoding"
      ]
    }
  }

  cookies_config {
    cookie_behavior = "none"
  }
}

resource "aws_cloudfront_origin_request_policy" "api_forwarding" {
  name    = "bitcraps-api-forwarding-${var.environment}"
  comment = "Forward all necessary headers for API requests"

  query_strings_config {
    query_string_behavior = "all"
  }

  headers_config {
    header_behavior = "allViewerAndWhitelistCloudFront"
    headers {
      items = [
        "CloudFront-Viewer-Address",
        "CloudFront-Viewer-Country"
      ]
    }
  }

  cookies_config {
    cookie_behavior = "all"
  }
}

resource "aws_cloudfront_origin_request_policy" "static_forwarding" {
  name    = "bitcraps-static-forwarding-${var.environment}"
  comment = "Minimal forwarding for static assets"

  query_strings_config {
    query_string_behavior = "whitelist"
    query_strings {
      items = ["v", "version"]
    }
  }

  headers_config {
    header_behavior = "whitelist"
    headers {
      items = ["Origin"]
    }
  }

  cookies_config {
    cookie_behavior = "none"
  }
}

# Response headers policies
resource "aws_cloudfront_response_headers_policy" "security_headers" {
  name    = "bitcraps-security-headers-${var.environment}"
  comment = "Security headers for static content"

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
    
    xss_protection {
      mode_block = true
      protection = true
      override   = false
    }
    
    referrer_policy {
      referrer_policy = "strict-origin-when-cross-origin"
      override        = false
    }
  }

  cors_config {
    access_control_allow_credentials = false
    access_control_allow_headers {
      items = ["*"]
    }
    access_control_allow_methods {
      items = ["GET", "HEAD", "OPTIONS", "PUT", "PATCH", "POST", "DELETE"]
    }
    access_control_allow_origins {
      items = ["*"]
    }
    origin_override = false
  }

  custom_headers_config {
    items {
      header   = "Content-Security-Policy"
      value    = var.security_headers.content_security_policy
      override = false
    }
  }
}

resource "aws_cloudfront_response_headers_policy" "api_headers" {
  name    = "bitcraps-api-headers-${var.environment}"
  comment = "Headers for API responses"

  cors_config {
    access_control_allow_credentials = true
    access_control_allow_headers {
      items = ["Authorization", "Content-Type", "X-Requested-With", "Accept"]
    }
    access_control_allow_methods {
      items = ["GET", "HEAD", "OPTIONS", "PUT", "PATCH", "POST", "DELETE"]
    }
    access_control_allow_origins {
      items = ["https://${var.domain_name}", "https://*.${var.domain_name}"]
    }
    access_control_max_age_sec = 86400
    origin_override           = false
  }

  security_headers_config {
    strict_transport_security {
      access_control_max_age_sec = 31536000
      include_subdomains         = true
      override                   = false
    }
  }
}

# CloudFront distribution
resource "aws_cloudfront_distribution" "main" {
  comment             = "BitCraps CDN Distribution - ${var.environment}"
  default_root_object = "index.html"
  enabled             = true
  is_ipv6_enabled     = true
  price_class         = var.price_class
  web_acl_id          = var.enable_waf ? aws_wafv2_web_acl.cdn_waf[0].arn : null

  # Primary origin (S3)
  origin {
    domain_name              = aws_s3_bucket.cdn_origin.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.cdn_oac.id
    origin_id                = "S3-${aws_s3_bucket.cdn_origin.bucket}"

    custom_header {
      name  = "X-Origin-Source"
      value = "s3-primary"
    }
  }

  # API origin
  dynamic "origin" {
    for_each = var.cdn_subdomains.api != "" ? [1] : []
    content {
      domain_name = "${var.cdn_subdomains.api}.${var.domain_name}"
      origin_id   = "API-Origin"
      
      custom_origin_config {
        http_port              = 80
        https_port             = 443
        origin_protocol_policy = "https-only"
        origin_ssl_protocols   = ["TLSv1.2"]
      }

      custom_header {
        name  = "X-Origin-Source" 
        value = "api-backend"
      }
    }
  }

  # Default cache behavior (static content)
  default_cache_behavior {
    target_origin_id       = "S3-${aws_s3_bucket.cdn_origin.bucket}"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods         = ["GET", "HEAD", "OPTIONS"]
    compress               = var.compression_config.enable_gzip

    cache_policy_id            = aws_cloudfront_cache_policy.static_assets.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.static_forwarding.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.security_headers.id

    # Edge Lambda functions
    dynamic "lambda_function_association" {
      for_each = var.edge_lambda_functions
      content {
        event_type   = lambda_function_association.value.event_type
        lambda_arn   = lambda_function_association.value.lambda_arn
        include_body = lambda_function_association.value.include_body
      }
    }
  }

  # WASM files cache behavior
  ordered_cache_behavior {
    path_pattern           = "*.wasm"
    target_origin_id       = "S3-${aws_s3_bucket.cdn_origin.bucket}"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD", "OPTIONS"]
    cached_methods         = ["GET", "HEAD", "OPTIONS"]
    compress               = var.compression_config.enable_gzip

    cache_policy_id            = aws_cloudfront_cache_policy.wasm_optimized.id
    origin_request_policy_id   = aws_cloudfront_origin_request_policy.cors_enabled.id
    response_headers_policy_id = aws_cloudfront_response_headers_policy.security_headers.id
  }

  # API cache behavior
  dynamic "ordered_cache_behavior" {
    for_each = var.cdn_subdomains.api != "" ? [1] : []
    content {
      path_pattern           = "/api/*"
      target_origin_id       = "API-Origin"
      viewer_protocol_policy = "redirect-to-https"
      allowed_methods        = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
      cached_methods         = ["GET", "HEAD", "OPTIONS"]
      compress               = var.compression_config.enable_gzip

      cache_policy_id            = aws_cloudfront_cache_policy.api_cache.id
      origin_request_policy_id   = aws_cloudfront_origin_request_policy.api_forwarding.id
      response_headers_policy_id = aws_cloudfront_response_headers_policy.api_headers.id
    }
  }

  # Aliases (custom domain names)
  aliases = [
    var.domain_name,
    "${var.cdn_subdomains.assets}.${var.domain_name}",
    "${var.cdn_subdomains.wasm}.${var.domain_name}",
    "${var.cdn_subdomains.game}.${var.domain_name}"
  ]

  # SSL certificate
  viewer_certificate {
    acm_certificate_arn      = var.ssl_certificate_arn != "" ? var.ssl_certificate_arn : null
    cloudfront_default_certificate = var.ssl_certificate_arn == "" ? true : false
    ssl_support_method       = var.ssl_certificate_arn != "" ? "sni-only" : null
    minimum_protocol_version = "TLSv1.2_2021"
  }

  # Geographic restrictions
  restrictions {
    geo_restriction {
      restriction_type = var.geo_restrictions.restriction_type
      locations        = var.geo_restrictions.locations
    }
  }

  # Custom error responses
  dynamic "custom_error_response" {
    for_each = var.custom_error_responses
    content {
      error_code            = custom_error_response.value.error_code
      response_code         = custom_error_response.value.response_code
      response_page_path    = custom_error_response.value.response_page_path
      error_caching_min_ttl = custom_error_response.value.error_caching_min_ttl
    }
  }

  # Logging configuration
  dynamic "logging_config" {
    for_each = var.log_bucket_name != "" ? [1] : []
    content {
      include_cookies = false
      bucket          = "${var.log_bucket_name}.s3.amazonaws.com"
      prefix          = "cloudfront-logs/${var.environment}/"
    }
  }

  tags = merge(local.common_tags, var.tags)
}

# S3 bucket policy for CloudFront OAC
resource "aws_s3_bucket_policy" "cdn_origin" {
  bucket = aws_s3_bucket.cdn_origin.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "AllowCloudFrontServicePrincipal"
        Effect    = "Allow"
        Principal = {
          Service = "cloudfront.amazonaws.com"
        }
        Action   = "s3:GetObject"
        Resource = "${aws_s3_bucket.cdn_origin.arn}/*"
        Condition = {
          StringEquals = {
            "AWS:SourceArn" = aws_cloudfront_distribution.main.arn
          }
        }
      }
    ]
  })
}

# Outputs
output "cloudfront_distribution_id" {
  description = "CloudFront distribution ID"
  value       = aws_cloudfront_distribution.main.id
}

output "cloudfront_distribution_arn" {
  description = "CloudFront distribution ARN"
  value       = aws_cloudfront_distribution.main.arn
}

output "cloudfront_domain_name" {
  description = "CloudFront distribution domain name"
  value       = aws_cloudfront_distribution.main.domain_name
}

output "cloudfront_hosted_zone_id" {
  description = "CloudFront distribution hosted zone ID"
  value       = aws_cloudfront_distribution.main.hosted_zone_id
}