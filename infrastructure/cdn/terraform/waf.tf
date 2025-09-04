# AWS WAF v2 Configuration for CDN Protection

# WAF Web ACL for CloudFront
resource "aws_wafv2_web_acl" "cdn_waf" {
  count = var.enable_waf ? 1 : 0
  
  name  = "bitcraps-cdn-waf-${var.environment}"
  scope = "CLOUDFRONT"

  default_action {
    allow {}
  }

  # Rate limiting rule - prevent DDoS
  rule {
    name     = "RateLimitRule"
    priority = 1

    override_action {
      none {}
    }

    statement {
      rate_based_statement {
        limit              = 2000
        aggregate_key_type = "IP"
        
        scope_down_statement {
          not_statement {
            statement {
              byte_match_statement {
                search_string         = "/health"
                field_to_match {
                  uri_path {}
                }
                text_transformation {
                  priority = 0
                  type     = "LOWERCASE"
                }
                positional_constraint = "STARTS_WITH"
              }
            }
          }
        }
      }
    }

    action {
      block {}
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "RateLimitRule"
    }
  }

  # AWS Managed Rules - Core Rule Set
  rule {
    name     = "AWS-AWSManagedRulesCommonRuleSet"
    priority = 2

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesCommonRuleSet"
        vendor_name = "AWS"
        
        # Exclude rules that might interfere with legitimate WASM/gaming traffic
        excluded_rule {
          name = "SizeRestrictions_BODY"
        }
        excluded_rule {
          name = "GenericRFI_BODY"
        }
      }
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "CommonRuleSetMetric"
    }
  }

  # AWS Managed Rules - Known Bad Inputs
  rule {
    name     = "AWS-AWSManagedRulesKnownBadInputsRuleSet"
    priority = 3

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesKnownBadInputsRuleSet"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "KnownBadInputsRuleSetMetric"
    }
  }

  # AWS Managed Rules - Anonymous IP List
  rule {
    name     = "AWS-AWSManagedRulesAnonymousIpList"
    priority = 4

    override_action {
      none {}
    }

    statement {
      managed_rule_group_statement {
        name        = "AWSManagedRulesAnonymousIpList"
        vendor_name = "AWS"
      }
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "AnonymousIpListMetric"
    }
  }

  # Custom rule - Block specific bot patterns
  rule {
    name     = "BlockBadBots"
    priority = 5

    action {
      block {}
    }

    statement {
      or_statement {
        statement {
          byte_match_statement {
            search_string = "badbot"
            field_to_match {
              single_header {
                name = "user-agent"
              }
            }
            text_transformation {
              priority = 0
              type     = "LOWERCASE"
            }
            positional_constraint = "CONTAINS"
          }
        }
        statement {
          byte_match_statement {
            search_string = "crawler"
            field_to_match {
              single_header {
                name = "user-agent"
              }
            }
            text_transformation {
              priority = 0
              type     = "LOWERCASE"
            }
            positional_constraint = "CONTAINS"
          }
        }
      }
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "BlockBadBotsMetric"
    }
  }

  # Custom rule - Allow legitimate gaming traffic
  rule {
    name     = "AllowGameTraffic"
    priority = 6

    action {
      allow {}
    }

    statement {
      and_statement {
        statement {
          byte_match_statement {
            search_string = "/game/"
            field_to_match {
              uri_path {}
            }
            text_transformation {
              priority = 0
              type     = "LOWERCASE"
            }
            positional_constraint = "STARTS_WITH"
          }
        }
        statement {
          byte_match_statement {
            search_string = "bitcraps"
            field_to_match {
              single_header {
                name = "user-agent"
              }
            }
            text_transformation {
              priority = 0
              type     = "LOWERCASE"
            }
            positional_constraint = "CONTAINS"
          }
        }
      }
    }

    visibility_config {
      sampled_requests_enabled   = true
      cloudwatch_metrics_enabled = true
      metric_name                = "AllowGameTrafficMetric"
    }
  }

  # Geographic restriction rule (if enabled)
  dynamic "rule" {
    for_each = var.geo_restrictions.restriction_type != "none" ? [1] : []
    content {
      name     = "GeoRestrictionRule"
      priority = 7

      action {
        block {}
      }

      statement {
        geo_match_statement {
          country_codes = var.geo_restrictions.restriction_type == "blacklist" ? var.geo_restrictions.locations : []
        }
      }

      visibility_config {
        sampled_requests_enabled   = true
        cloudwatch_metrics_enabled = true
        metric_name                = "GeoRestrictionMetric"
      }
    }
  }

  visibility_config {
    sampled_requests_enabled   = true
    cloudwatch_metrics_enabled = true
    metric_name                = "bitcrapsWAF"
  }

  tags = merge(local.common_tags, var.tags)
}

# WAF logging configuration
resource "aws_wafv2_web_acl_logging_configuration" "cdn_waf_logging" {
  count                   = var.enable_waf && var.log_bucket_name != "" ? 1 : 0
  resource_arn            = aws_wafv2_web_acl.cdn_waf[0].arn
  log_destination_configs = ["arn:aws:s3:::${var.log_bucket_name}/waf-logs/"]

  redacted_fields {
    single_header {
      name = "authorization"
    }
  }

  redacted_fields {
    single_header {
      name = "x-api-key"
    }
  }
}

# CloudWatch dashboard for WAF metrics
resource "aws_cloudwatch_dashboard" "waf_dashboard" {
  count          = var.enable_waf ? 1 : 0
  dashboard_name = "bitcraps-waf-${var.environment}"

  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6

        properties = {
          metrics = [
            ["AWS/WAFV2", "AllowedRequests", "WebACL", aws_wafv2_web_acl.cdn_waf[0].name, "Region", "CloudFront", "Rule", "ALL"],
            [".", "BlockedRequests", ".", ".", ".", ".", ".", "."],
          ]
          view    = "timeSeries"
          stacked = false
          region  = "us-east-1"  # WAF metrics for CloudFront are in us-east-1
          title   = "WAF Request Volume"
          period  = 300
        }
      },
      {
        type   = "metric"
        x      = 0
        y      = 6
        width  = 12
        height = 6

        properties = {
          metrics = [
            ["AWS/WAFV2", "BlockedRequests", "WebACL", aws_wafv2_web_acl.cdn_waf[0].name, "Region", "CloudFront", "Rule", "RateLimitRule"],
            [".", ".", ".", ".", ".", ".", ".", "CommonRuleSetMetric"],
            [".", ".", ".", ".", ".", ".", ".", "KnownBadInputsRuleSetMetric"],
            [".", ".", ".", ".", ".", ".", ".", "BlockBadBotsMetric"],
          ]
          view    = "timeSeries"
          stacked = false
          region  = "us-east-1"
          title   = "WAF Rule Blocks by Rule"
          period  = 300
        }
      }
    ]
  })
}

# WAF alarms
resource "aws_cloudwatch_metric_alarm" "waf_high_block_rate" {
  count               = var.enable_waf ? 1 : 0
  alarm_name          = "bitcraps-waf-high-block-rate-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "BlockedRequests"
  namespace           = "AWS/WAFV2"
  period              = "300"
  statistic           = "Sum"
  threshold           = "1000"
  alarm_description   = "This metric monitors WAF blocked requests"
  alarm_actions       = [] # Add SNS topic ARN for notifications

  dimensions = {
    WebACL = aws_wafv2_web_acl.cdn_waf[0].name
    Region = "CloudFront"
  }

  tags = local.common_tags
}

output "waf_web_acl_id" {
  description = "WAF Web ACL ID"
  value       = var.enable_waf ? aws_wafv2_web_acl.cdn_waf[0].id : null
}

output "waf_web_acl_arn" {
  description = "WAF Web ACL ARN"
  value       = var.enable_waf ? aws_wafv2_web_acl.cdn_waf[0].arn : null
}