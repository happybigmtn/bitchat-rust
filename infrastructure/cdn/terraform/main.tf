# BitCraps CDN Infrastructure - Multi-Provider Setup
# Supports Cloudflare, AWS CloudFront, and Fastly for global distribution

terraform {
  required_version = ">= 1.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
    fastly = {
      source  = "fastly/fastly"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket = "bitcraps-terraform-state"
    key    = "cdn/terraform.tfstate"
    region = "us-west-2"
  }
}

# Configure providers
provider "aws" {
  region = var.aws_primary_region
  
  default_tags {
    tags = {
      Project     = "bitcraps"
      Environment = var.environment
      Component   = "cdn"
      ManagedBy   = "terraform"
    }
  }
}

provider "cloudflare" {
  api_token = var.cloudflare_api_token
}

provider "fastly" {
  api_key = var.fastly_api_key
}

# Data sources
data "aws_caller_identity" "current" {}
data "aws_region" "current" {}

# Local values for common configurations
locals {
  common_tags = {
    Project     = "bitcraps"
    Environment = var.environment
    Component   = "cdn"
    ManagedBy   = "terraform"
  }

  # Cache behaviors for different content types
  cache_behaviors = {
    wasm = {
      path_pattern           = "*.wasm"
      cache_policy_id        = aws_cloudfront_cache_policy.wasm_optimized.id
      origin_request_policy_id = aws_cloudfront_origin_request_policy.cors_enabled.id
      response_headers_policy_id = aws_cloudfront_response_headers_policy.security_headers.id
    }
    api = {
      path_pattern           = "/api/*"
      cache_policy_id        = aws_cloudfront_cache_policy.api_cache.id
      origin_request_policy_id = aws_cloudfront_origin_request_policy.api_forwarding.id
      response_headers_policy_id = aws_cloudfront_response_headers_policy.api_headers.id
    }
    static = {
      path_pattern           = "/static/*"
      cache_policy_id        = aws_cloudfront_cache_policy.static_assets.id
      origin_request_policy_id = aws_cloudfront_origin_request_policy.static_forwarding.id
      response_headers_policy_id = aws_cloudfront_response_headers_policy.security_headers.id
    }
  }

  # Edge locations for optimal performance
  edge_locations = [
    "us-east-1",    # North America East
    "us-west-2",    # North America West  
    "eu-west-1",    # Europe
    "ap-southeast-1", # Asia Pacific
    "ap-northeast-1", # Japan
  ]
}

# S3 bucket for origin content
resource "aws_s3_bucket" "cdn_origin" {
  bucket = "bitcraps-cdn-origin-${var.environment}"
  tags   = local.common_tags
}

resource "aws_s3_bucket_public_access_block" "cdn_origin" {
  bucket = aws_s3_bucket.cdn_origin.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_versioning" "cdn_origin" {
  bucket = aws_s3_bucket.cdn_origin.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "cdn_origin" {
  bucket = aws_s3_bucket.cdn_origin.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
    bucket_key_enabled = true
  }
}

# Origin Access Control for CloudFront
resource "aws_cloudfront_origin_access_control" "cdn_oac" {
  name                              = "bitcraps-cdn-oac-${var.environment}"
  description                       = "OAC for BitCraps CDN"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# Output values for other modules
output "cdn_origin_bucket_name" {
  description = "Name of the S3 bucket used as CDN origin"
  value       = aws_s3_bucket.cdn_origin.bucket
}

output "cdn_origin_bucket_domain" {
  description = "Regional domain name of the S3 bucket"
  value       = aws_s3_bucket.cdn_origin.bucket_regional_domain_name
}

output "cloudfront_oac_id" {
  description = "CloudFront Origin Access Control ID"
  value       = aws_cloudfront_origin_access_control.cdn_oac.id
}

output "edge_locations" {
  description = "List of edge locations for CDN deployment"
  value       = local.edge_locations
}