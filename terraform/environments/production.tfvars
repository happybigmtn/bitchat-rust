# Production Environment Variables for BitCraps Infrastructure

# General Configuration
environment  = "production"
aws_region   = "us-west-2"
cost_center  = "bitcraps-prod"

# VPC Configuration
vpc_cidr = "10.0.0.0/16"
availability_zones = ["us-west-2a", "us-west-2b", "us-west-2c"]

# Public subnets for load balancers and NAT gateways
public_subnets = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]

# Private subnets for EKS worker nodes
private_subnets = ["10.0.10.0/24", "10.0.20.0/24", "10.0.30.0/24"]

# Database subnets (isolated)
database_subnets = ["10.0.100.0/24", "10.0.200.0/24", "10.0.300.0/24"]

# EKS Configuration
cluster_version = "1.28"
node_groups = {
  production = {
    instance_types = ["m5.xlarge", "m5.large"]
    scaling_config = {
      desired_size = 3
      max_size     = 20
      min_size     = 3
    }
    capacity_type = "ON_DEMAND"
    disk_size     = 100
    labels = {
      role        = "production"
      environment = "production"
    }
    taints = [
      {
        key    = "bitcraps.io/dedicated"
        value  = "production"
        effect = "NO_SCHEDULE"
      }
    ]
  }
  
  gateway = {
    instance_types = ["m5.large", "m5.medium"]
    scaling_config = {
      desired_size = 2
      max_size     = 10
      min_size     = 2
    }
    capacity_type = "SPOT"
    disk_size     = 50
    labels = {
      role        = "gateway"
      environment = "production"
    }
    taints = [
      {
        key    = "bitcraps.io/dedicated"
        value  = "gateway"
        effect = "NO_SCHEDULE"
      }
    ]
  }
}

# RDS Configuration
db_instance_class           = "db.r6g.xlarge"
allocated_storage          = 500
max_allocated_storage      = 2000
iops                       = 10000
storage_throughput         = 1000
backup_retention_period    = 30
multi_az                   = true
deletion_protection        = true
skip_final_snapshot        = false
create_read_replica        = true
replica_instance_class     = "db.r6g.large"
max_connections            = "2000"
connection_alarm_threshold = 1600

# ElastiCache Configuration
cache_node_type             = "cache.r6g.large"
cache_num_nodes            = 3
cache_parameter_group      = "default.redis7"
cache_engine_version       = "7.0"
auth_token_enabled         = true
transit_encryption_enabled = true
at_rest_encryption_enabled = true

# Load Balancer Configuration
enable_deletion_protection = true
enable_http2              = true
idle_timeout              = 60

# Security Configuration
enable_vpc_flow_logs = true
flow_log_destination = "cloud-watch-logs"

# WAF Configuration
enable_waf = true
waf_rules = [
  {
    name     = "AWSManagedRulesCommonRuleSet"
    priority = 1
    override_action = "none"
  },
  {
    name     = "AWSManagedRulesKnownBadInputsRuleSet"
    priority = 2
    override_action = "none"
  },
  {
    name     = "AWSManagedRulesSQLiRuleSet"
    priority = 3
    override_action = "none"
  }
]

# CloudWatch and Monitoring
enable_detailed_monitoring = true
log_retention_days        = 90
metric_retention_days     = 365

# Backup Configuration
backup_vault_name = "bitcraps-production-backup"
backup_rules = [
  {
    rule_name         = "daily_backups"
    schedule          = "cron(0 2 * * ? *)"
    start_window      = 60
    completion_window = 300
    lifecycle = {
      cold_storage_after = 30
      delete_after      = 365
    }
  },
  {
    rule_name         = "weekly_backups"
    schedule          = "cron(0 3 ? * SUN *)"
    start_window      = 60
    completion_window = 600
    lifecycle = {
      cold_storage_after = 90
      delete_after      = 2555  # 7 years
    }
  }
]

# DNS Configuration
domain_name = "bitcraps.io"
subdomains = {
  api     = "api.bitcraps.io"
  gateway = "gateway.bitcraps.io"
  admin   = "admin.bitcraps.io"
  metrics = "metrics.bitcraps.io"
}

# S3 Configuration
backup_bucket_name = "bitcraps-production-backups"
logs_bucket_name   = "bitcraps-production-logs"
versioning_enabled = true
lifecycle_rules = [
  {
    id     = "transition_to_ia"
    status = "Enabled"
    transition = {
      days          = 30
      storage_class = "STANDARD_IA"
    }
  },
  {
    id     = "transition_to_glacier"
    status = "Enabled"
    transition = {
      days          = 90
      storage_class = "GLACIER"
    }
  },
  {
    id     = "delete_old_backups"
    status = "Enabled"
    expiration = {
      days = 2555  # 7 years
    }
  }
]

# Secrets Manager Configuration
secrets = {
  database = {
    name        = "bitcraps/production/database"
    description = "Database credentials for BitCraps production"
  }
  redis = {
    name        = "bitcraps/production/redis"
    description = "Redis authentication token for BitCraps production"
  }
  jwt = {
    name        = "bitcraps/production/jwt"
    description = "JWT signing keys for BitCraps production"
  }
  encryption = {
    name        = "bitcraps/production/encryption"
    description = "Application encryption keys for BitCraps production"
  }
}

# Auto Scaling Configuration
auto_scaling_policies = {
  scale_up = {
    scaling_adjustment     = 2
    adjustment_type       = "ChangeInCapacity"
    cooldown             = 300
    metric_aggregation_type = "Average"
  }
  scale_down = {
    scaling_adjustment     = -1
    adjustment_type       = "ChangeInCapacity"
    cooldown             = 300
    metric_aggregation_type = "Average"
  }
}

# Certificate Configuration
certificate_domains = [
  "bitcraps.io",
  "*.bitcraps.io"
]

# CloudFront Configuration
enable_cloudfront = true
cloudfront_price_class = "PriceClass_All"
cloudfront_aliases = [
  "api.bitcraps.io",
  "gateway.bitcraps.io"
]

# Feature Flags
features = {
  enable_nat_gateway    = true
  enable_vpn_gateway    = false
  enable_transit_gateway = false
  enable_direct_connect = false
  enable_global_accelerator = true
}
