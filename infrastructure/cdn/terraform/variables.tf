# BitCraps CDN Infrastructure Variables

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "prod"

  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be one of: dev, staging, prod."
  }
}

variable "aws_primary_region" {
  description = "Primary AWS region for CDN infrastructure"
  type        = string
  default     = "us-west-2"
}

variable "cloudflare_api_token" {
  description = "Cloudflare API token for zone management"
  type        = string
  sensitive   = true
}

variable "cloudflare_zone_id" {
  description = "Cloudflare Zone ID for the domain"
  type        = string
}

variable "fastly_api_key" {
  description = "Fastly API key for service management"
  type        = string
  sensitive   = true
}

variable "domain_name" {
  description = "Primary domain name for CDN"
  type        = string
  default     = "bitcraps.io"
}

variable "cdn_subdomains" {
  description = "CDN subdomains for different content types"
  type = object({
    assets = string
    api    = string
    game   = string
    wasm   = string
  })
  default = {
    assets = "assets"
    api    = "api"  
    game   = "game"
    wasm   = "wasm"
  }
}

variable "cache_ttl_settings" {
  description = "Cache TTL settings for different content types"
  type = object({
    static_assets = number
    wasm_files    = number
    api_responses = number
    game_data     = number
  })
  default = {
    static_assets = 86400    # 24 hours
    wasm_files    = 604800   # 7 days
    api_responses = 300      # 5 minutes
    game_data     = 60       # 1 minute
  }
}

variable "enable_waf" {
  description = "Enable AWS WAF for CDN protection"
  type        = bool
  default     = true
}

variable "enable_ddos_protection" {
  description = "Enable DDoS protection"
  type        = bool
  default     = true
}

variable "geo_restrictions" {
  description = "Geographic restrictions configuration"
  type = object({
    restriction_type = string
    locations        = list(string)
  })
  default = {
    restriction_type = "none"
    locations        = []
  }

  validation {
    condition     = contains(["none", "whitelist", "blacklist"], var.geo_restrictions.restriction_type)
    error_message = "Geo restriction type must be: none, whitelist, or blacklist."
  }
}

variable "ssl_certificate_arn" {
  description = "ARN of SSL certificate in ACM"
  type        = string
  default     = ""
}

variable "origin_shield_enabled" {
  description = "Enable origin shield for additional caching layer"
  type        = bool
  default     = true
}

variable "price_class" {
  description = "CloudFront price class"
  type        = string
  default     = "PriceClass_100"

  validation {
    condition     = contains(["PriceClass_All", "PriceClass_200", "PriceClass_100"], var.price_class)
    error_message = "Price class must be: PriceClass_All, PriceClass_200, or PriceClass_100."
  }
}

variable "custom_error_responses" {
  description = "Custom error response configurations"
  type = list(object({
    error_code         = number
    response_code      = number
    response_page_path = string
    error_caching_min_ttl = number
  }))
  default = [
    {
      error_code            = 404
      response_code         = 404
      response_page_path    = "/404.html"
      error_caching_min_ttl = 300
    },
    {
      error_code            = 500
      response_code         = 500
      response_page_path    = "/500.html"
      error_caching_min_ttl = 60
    }
  ]
}

variable "log_bucket_name" {
  description = "S3 bucket for CDN access logs"
  type        = string
  default     = ""
}

variable "monitoring_config" {
  description = "Monitoring and alerting configuration"
  type = object({
    enable_real_time_metrics = bool
    enable_enhanced_metrics  = bool
    alarm_thresholds = object({
      error_rate_threshold       = number
      origin_latency_threshold   = number
      cache_hit_rate_threshold   = number
    })
  })
  default = {
    enable_real_time_metrics = true
    enable_enhanced_metrics  = true
    alarm_thresholds = {
      error_rate_threshold     = 5.0   # 5% error rate
      origin_latency_threshold = 3000  # 3 seconds
      cache_hit_rate_threshold = 85.0  # 85% cache hit rate
    }
  }
}

variable "compression_config" {
  description = "Content compression configuration"
  type = object({
    enable_gzip    = bool
    enable_brotli  = bool
    file_types     = list(string)
  })
  default = {
    enable_gzip   = true
    enable_brotli = true
    file_types = [
      "text/html",
      "text/css",
      "text/javascript",
      "application/javascript",
      "application/json",
      "application/wasm",
      "image/svg+xml"
    ]
  }
}

variable "security_headers" {
  description = "Security headers configuration"
  type = object({
    strict_transport_security = string
    content_type_options     = string
    frame_options            = string
    xss_protection          = string
    referrer_policy         = string
    content_security_policy = string
  })
  default = {
    strict_transport_security = "max-age=31536000; includeSubDomains"
    content_type_options     = "nosniff"
    frame_options            = "DENY"
    xss_protection          = "1; mode=block"
    referrer_policy         = "strict-origin-when-cross-origin"
    content_security_policy = "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' wss: https:; font-src 'self' data:; object-src 'none'; media-src 'self'; frame-src 'none';"
  }
}

variable "edge_lambda_functions" {
  description = "Edge Lambda function configurations"
  type = map(object({
    event_type   = string
    lambda_arn   = string
    include_body = bool
  }))
  default = {}
}

variable "tags" {
  description = "Additional tags for resources"
  type        = map(string)
  default     = {}
}