# CDN Monitoring and Analytics Configuration

# CloudWatch Dashboard for CDN monitoring
resource "aws_cloudwatch_dashboard" "cdn_monitoring" {
  dashboard_name = "bitcraps-cdn-${var.environment}"

  dashboard_body = templatefile("${path.module}/../monitoring/cloudwatch-dashboard.json", {
    DISTRIBUTION_ID           = aws_cloudfront_distribution.main.id
    WASM_DISTRIBUTION_ID     = aws_cloudfront_distribution.wasm_distribution.id
    API_DISTRIBUTION_ID      = aws_cloudfront_distribution.api_distribution.id
    GAME_DISTRIBUTION_ID     = aws_cloudfront_distribution.game_distribution.id
    WAF_WEB_ACL_NAME        = var.enable_waf ? aws_wafv2_web_acl.cdn_waf[0].name : ""
    API_AUTH_LAMBDA_NAME    = var.enable_api_lambda ? aws_lambda_function.api_auth_lambda[0].function_name : ""
    IMAGE_OPTIMIZATION_LAMBDA_NAME = var.enable_image_optimization ? aws_lambda_function.image_optimization_lambda[0].function_name : ""
    CDN_ORIGIN_BUCKET_NAME  = aws_s3_bucket.cdn_origin.bucket
  })

  depends_on = [
    aws_cloudfront_distribution.main,
    aws_cloudfront_distribution.wasm_distribution,
    aws_cloudfront_distribution.api_distribution,
    aws_cloudfront_distribution.game_distribution
  ]
}

# CloudWatch alarms for CDN monitoring
resource "aws_cloudwatch_metric_alarm" "high_error_rate" {
  alarm_name          = "bitcraps-cdn-high-error-rate-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "4xxErrorRate"
  namespace           = "AWS/CloudFront"
  period              = "300"
  statistic           = "Average"
  threshold           = var.monitoring_config.alarm_thresholds.error_rate_threshold
  alarm_description   = "This metric monitors CDN 4xx error rate"
  alarm_actions       = [aws_sns_topic.cdn_alerts.arn]

  dimensions = {
    DistributionId = aws_cloudfront_distribution.main.id
    Region         = "Global"
  }

  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "high_origin_latency" {
  alarm_name          = "bitcraps-cdn-high-origin-latency-${var.environment}"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "3"
  metric_name         = "OriginLatency"
  namespace           = "AWS/CloudFront"
  period              = "300"
  statistic           = "Average"
  threshold           = var.monitoring_config.alarm_thresholds.origin_latency_threshold
  alarm_description   = "This metric monitors CDN origin latency"
  alarm_actions       = [aws_sns_topic.cdn_alerts.arn]

  dimensions = {
    DistributionId = aws_cloudfront_distribution.main.id
    Region         = "Global"
  }

  tags = local.common_tags
}

resource "aws_cloudwatch_metric_alarm" "low_cache_hit_rate" {
  alarm_name          = "bitcraps-cdn-low-cache-hit-rate-${var.environment}"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = "3"
  metric_name         = "CacheHitRate"
  namespace           = "AWS/CloudFront"
  period              = "900"  # 15 minutes
  statistic           = "Average"
  threshold           = var.monitoring_config.alarm_thresholds.cache_hit_rate_threshold
  alarm_description   = "This metric monitors CDN cache hit rate"
  alarm_actions       = [aws_sns_topic.cdn_alerts.arn]

  dimensions = {
    DistributionId = aws_cloudfront_distribution.main.id
    Region         = "Global"
  }

  tags = local.common_tags
}

# SNS topic for CDN alerts
resource "aws_sns_topic" "cdn_alerts" {
  name = "bitcraps-cdn-alerts-${var.environment}"

  tags = local.common_tags
}

resource "aws_sns_topic_policy" "cdn_alerts" {
  arn = aws_sns_topic.cdn_alerts.arn

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Service = "cloudwatch.amazonaws.com"
        }
        Action   = "SNS:Publish"
        Resource = aws_sns_topic.cdn_alerts.arn
        Condition = {
          StringEquals = {
            "aws:SourceAccount" = data.aws_caller_identity.current.account_id
          }
        }
      }
    ]
  })
}

# CloudWatch Log Group for CDN access logs
resource "aws_cloudwatch_log_group" "cdn_access_logs" {
  count = var.log_bucket_name != "" ? 1 : 0

  name              = "/aws/cloudfront/${aws_cloudfront_distribution.main.id}"
  retention_in_days = 30

  tags = local.common_tags
}

# CloudWatch Log Stream for real-time monitoring
resource "aws_cloudwatch_log_stream" "cdn_realtime" {
  count          = var.log_bucket_name != "" ? 1 : 0
  name           = "realtime-monitoring"
  log_group_name = aws_cloudwatch_log_group.cdn_access_logs[0].name
}

# Kinesis Data Firehose for real-time analytics
resource "aws_kinesis_firehose_delivery_stream" "cdn_analytics" {
  count       = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  name        = "bitcraps-cdn-analytics-${var.environment}"
  destination = "s3"

  s3_configuration {
    role_arn           = aws_iam_role.firehose_role[0].arn
    bucket_arn         = aws_s3_bucket.analytics_bucket[0].arn
    prefix             = "year=!{timestamp:yyyy}/month=!{timestamp:MM}/day=!{timestamp:dd}/hour=!{timestamp:HH}/"
    error_output_prefix = "errors/"
    buffer_size        = 5
    buffer_interval    = 300
    compression_format = "GZIP"

    cloudwatch_logging_options {
      enabled         = true
      log_group_name  = aws_cloudwatch_log_group.firehose_log_group[0].name
      log_stream_name = aws_cloudwatch_log_stream.firehose_log_stream[0].name
    }
  }

  tags = local.common_tags
}

# S3 bucket for analytics data
resource "aws_s3_bucket" "analytics_bucket" {
  count  = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  bucket = "bitcraps-cdn-analytics-${var.environment}-${random_id.analytics_bucket_suffix[0].hex}"

  tags = local.common_tags
}

resource "random_id" "analytics_bucket_suffix" {
  count       = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  byte_length = 4
}

resource "aws_s3_bucket_versioning" "analytics_bucket" {
  count  = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  bucket = aws_s3_bucket.analytics_bucket[0].id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "analytics_bucket" {
  count  = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  bucket = aws_s3_bucket.analytics_bucket[0].id

  rule {
    id     = "analytics_lifecycle"
    status = "Enabled"

    expiration {
      days = 90
    }

    noncurrent_version_expiration {
      noncurrent_days = 30
    }
  }
}

# IAM role for Kinesis Firehose
resource "aws_iam_role" "firehose_role" {
  count = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  name  = "bitcraps-firehose-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "firehose.amazonaws.com"
        }
      }
    ]
  })

  tags = local.common_tags
}

resource "aws_iam_policy" "firehose_policy" {
  count = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  name  = "bitcraps-firehose-policy-${var.environment}"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:AbortMultipartUpload",
          "s3:GetBucketLocation",
          "s3:GetObject",
          "s3:ListBucket",
          "s3:ListBucketMultipartUploads",
          "s3:PutObject"
        ]
        Resource = [
          aws_s3_bucket.analytics_bucket[0].arn,
          "${aws_s3_bucket.analytics_bucket[0].arn}/*"
        ]
      },
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents"
        ]
        Resource = "arn:aws:logs:*:*:*"
      }
    ]
  })
}

resource "aws_iam_role_policy_attachment" "firehose_policy" {
  count      = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  role       = aws_iam_role.firehose_role[0].name
  policy_arn = aws_iam_policy.firehose_policy[0].arn
}

# CloudWatch Log Group for Firehose
resource "aws_cloudwatch_log_group" "firehose_log_group" {
  count             = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  name              = "/aws/kinesisfirehose/bitcraps-cdn-analytics-${var.environment}"
  retention_in_days = 14

  tags = local.common_tags
}

resource "aws_cloudwatch_log_stream" "firehose_log_stream" {
  count          = var.monitoring_config.enable_real_time_metrics ? 1 : 0
  name           = "S3Delivery"
  log_group_name = aws_cloudwatch_log_group.firehose_log_group[0].name
}

# AWS X-Ray tracing for Lambda@Edge functions
resource "aws_xray_sampling_rule" "lambda_edge_sampling" {
  count = var.enable_api_lambda ? 1 : 0

  rule_name      = "bitcraps-lambda-edge-${var.environment}"
  priority       = 9000
  version        = 1
  reservoir_size = 1
  fixed_rate     = 0.1
  url_path       = "*"
  host           = "*"
  http_method    = "*"
  service_name   = "*"
  service_type   = "*"
  resource_arn   = "*"

  tags = local.common_tags
}

# Performance monitoring Lambda function
resource "aws_lambda_function" "performance_monitor" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  filename         = data.archive_file.performance_monitor_zip[0].output_path
  function_name    = "bitcraps-performance-monitor-${var.environment}"
  role            = aws_iam_role.performance_monitor_role[0].arn
  handler         = "index.handler"
  source_code_hash = data.archive_file.performance_monitor_zip[0].output_base64sha256
  runtime         = "python3.9"
  timeout         = 60
  memory_size     = 256

  environment {
    variables = {
      DISTRIBUTION_ID = aws_cloudfront_distribution.main.id
      SNS_TOPIC_ARN  = aws_sns_topic.cdn_alerts.arn
    }
  }

  tags = local.common_tags
}

# Performance monitor Lambda source
resource "local_file" "performance_monitor_source" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  filename = "${path.module}/lambda-sources/performance-monitor/index.py"
  content = <<-EOT
import json
import boto3
import os
from datetime import datetime, timedelta

cloudwatch = boto3.client('cloudwatch')
sns = boto3.client('sns')

def lambda_handler(event, context):
    distribution_id = os.environ['DISTRIBUTION_ID']
    sns_topic_arn = os.environ['SNS_TOPIC_ARN']
    
    # Get CloudFront metrics for the last 5 minutes
    end_time = datetime.utcnow()
    start_time = end_time - timedelta(minutes=5)
    
    try:
        # Check error rate
        error_rate_response = cloudwatch.get_metric_statistics(
            Namespace='AWS/CloudFront',
            MetricName='4xxErrorRate',
            Dimensions=[
                {'Name': 'DistributionId', 'Value': distribution_id},
                {'Name': 'Region', 'Value': 'Global'}
            ],
            StartTime=start_time,
            EndTime=end_time,
            Period=300,
            Statistics=['Average']
        )
        
        # Check cache hit rate
        cache_hit_response = cloudwatch.get_metric_statistics(
            Namespace='AWS/CloudFront',
            MetricName='CacheHitRate',
            Dimensions=[
                {'Name': 'DistributionId', 'Value': distribution_id},
                {'Name': 'Region', 'Value': 'Global'}
            ],
            StartTime=start_time,
            EndTime=end_time,
            Period=300,
            Statistics=['Average']
        )
        
        # Check origin latency
        latency_response = cloudwatch.get_metric_statistics(
            Namespace='AWS/CloudFront',
            MetricName='OriginLatency',
            Dimensions=[
                {'Name': 'DistributionId', 'Value': distribution_id},
                {'Name': 'Region', 'Value': 'Global'}
            ],
            StartTime=start_time,
            EndTime=end_time,
            Period=300,
            Statistics=['Average']
        )
        
        # Analyze metrics and send alerts if necessary
        alerts = []
        
        if error_rate_response['Datapoints']:
            latest_error_rate = error_rate_response['Datapoints'][-1]['Average']
            if latest_error_rate > 5.0:  # 5% threshold
                alerts.append(f"High error rate detected: {latest_error_rate:.2f}%")
        
        if cache_hit_response['Datapoints']:
            latest_cache_hit = cache_hit_response['Datapoints'][-1]['Average']
            if latest_cache_hit < 85.0:  # 85% threshold
                alerts.append(f"Low cache hit rate: {latest_cache_hit:.2f}%")
        
        if latency_response['Datapoints']:
            latest_latency = latency_response['Datapoints'][-1]['Average']
            if latest_latency > 3000:  # 3 second threshold
                alerts.append(f"High origin latency: {latest_latency:.0f}ms")
        
        # Send alerts if any issues detected
        if alerts:
            message = f"CDN Performance Alert for {distribution_id}:\n" + "\n".join(alerts)
            
            sns.publish(
                TopicArn=sns_topic_arn,
                Message=message,
                Subject=f"BitCraps CDN Performance Alert - {distribution_id}"
            )
        
        return {
            'statusCode': 200,
            'body': json.dumps({
                'message': 'Performance monitoring completed',
                'alerts': len(alerts)
            })
        }
        
    except Exception as e:
        print(f"Error in performance monitoring: {str(e)}")
        return {
            'statusCode': 500,
            'body': json.dumps({
                'error': str(e)
            })
        }
EOT
}

# Performance monitor Lambda IAM role
resource "aws_iam_role" "performance_monitor_role" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0
  name  = "bitcraps-performance-monitor-role-${var.environment}"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "lambda.amazonaws.com"
        }
      }
    ]
  })

  tags = local.common_tags
}

resource "aws_iam_role_policy" "performance_monitor_policy" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0
  name  = "bitcraps-performance-monitor-policy-${var.environment}"
  role  = aws_iam_role.performance_monitor_role[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents"
        ]
        Resource = "arn:aws:logs:*:*:*"
      },
      {
        Effect = "Allow"
        Action = [
          "cloudwatch:GetMetricStatistics",
          "cloudwatch:ListMetrics"
        ]
        Resource = "*"
      },
      {
        Effect = "Allow"
        Action = [
          "sns:Publish"
        ]
        Resource = aws_sns_topic.cdn_alerts.arn
      }
    ]
  })
}

# Zip the performance monitor Lambda function
data "archive_file" "performance_monitor_zip" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  type        = "zip"
  source_dir  = "${path.module}/lambda-sources/performance-monitor"
  output_path = "${path.module}/lambda-sources/performance-monitor.zip"

  depends_on = [local_file.performance_monitor_source]
}

# EventBridge rule for performance monitoring
resource "aws_cloudwatch_event_rule" "performance_monitor_schedule" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  name                = "bitcraps-performance-monitor-${var.environment}"
  description         = "Schedule for CDN performance monitoring"
  schedule_expression = "rate(5 minutes)"

  tags = local.common_tags
}

resource "aws_cloudwatch_event_target" "performance_monitor_target" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  rule      = aws_cloudwatch_event_rule.performance_monitor_schedule[0].name
  target_id = "PerformanceMonitorTarget"
  arn       = aws_lambda_function.performance_monitor[0].arn
}

resource "aws_lambda_permission" "allow_eventbridge" {
  count = var.monitoring_config.enable_enhanced_metrics ? 1 : 0

  statement_id  = "AllowExecutionFromEventBridge"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.performance_monitor[0].function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.performance_monitor_schedule[0].arn
}

# Outputs
output "cloudwatch_dashboard_url" {
  description = "URL to the CloudWatch dashboard"
  value       = "https://${var.aws_primary_region}.console.aws.amazon.com/cloudwatch/home?region=${var.aws_primary_region}#dashboards:name=${aws_cloudwatch_dashboard.cdn_monitoring.dashboard_name}"
}

output "sns_topic_arn" {
  description = "SNS topic ARN for CDN alerts"
  value       = aws_sns_topic.cdn_alerts.arn
}

output "analytics_bucket_name" {
  description = "S3 bucket name for analytics data"
  value       = var.monitoring_config.enable_real_time_metrics ? aws_s3_bucket.analytics_bucket[0].bucket : null
}

output "firehose_delivery_stream_name" {
  description = "Kinesis Data Firehose delivery stream name"
  value       = var.monitoring_config.enable_real_time_metrics ? aws_kinesis_firehose_delivery_stream.cdn_analytics[0].name : null
}