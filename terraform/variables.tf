# Terraform Variables for BitCraps Infrastructure

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  validation {
    condition     = contains(["dev", "staging", "prod"], var.environment)
    error_message = "Environment must be dev, staging, or prod."
  }
}

variable "aws_region" {
  description = "AWS region for deployment"
  type        = string
  default     = "us-west-2"
}

variable "cost_center" {
  description = "Cost center for billing"
  type        = string
  default     = "gaming-platform"
}

variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "List of availability zones"
  type        = list(string)
  default     = ["us-west-2a", "us-west-2b", "us-west-2c"]
}

# EKS Configuration
variable "eks_node_instance_types" {
  description = "EC2 instance types for EKS nodes"
  type        = map(list(string))
  default = {
    dev     = ["t3.medium"]
    staging = ["t3.large"]
    prod    = ["m5.xlarge", "m5.2xlarge"]
  }
}

variable "eks_node_group_min_size" {
  description = "Minimum size of EKS node group"
  type        = map(number)
  default = {
    dev     = 2
    staging = 3
    prod    = 5
  }
}

variable "eks_node_group_max_size" {
  description = "Maximum size of EKS node group"
  type        = map(number)
  default = {
    dev     = 4
    staging = 6
    prod    = 20
  }
}

variable "eks_node_group_desired_size" {
  description = "Desired size of EKS node group"
  type        = map(number)
  default = {
    dev     = 2
    staging = 3
    prod    = 8
  }
}

# RDS Configuration
variable "rds_instance_class" {
  description = "RDS instance class"
  type        = map(string)
  default = {
    dev     = "db.t3.micro"
    staging = "db.t3.small"
    prod    = "db.r6g.xlarge"
  }
}

variable "rds_allocated_storage" {
  description = "RDS allocated storage in GB"
  type        = map(number)
  default = {
    dev     = 20
    staging = 100
    prod    = 500
  }
}

variable "rds_multi_az" {
  description = "Enable Multi-AZ for RDS"
  type        = map(bool)
  default = {
    dev     = false
    staging = false
    prod    = true
  }
}

# Redis Configuration
variable "redis_node_type" {
  description = "ElastiCache Redis node type"
  type        = map(string)
  default = {
    dev     = "cache.t3.micro"
    staging = "cache.t3.small"
    prod    = "cache.r6g.xlarge"
  }
}

variable "redis_num_cache_nodes" {
  description = "Number of Redis cache nodes"
  type        = map(number)
  default = {
    dev     = 1
    staging = 2
    prod    = 3
  }
}

# S3 Configuration
variable "s3_versioning" {
  description = "Enable S3 versioning"
  type        = bool
  default     = true
}

variable "s3_lifecycle_days" {
  description = "S3 lifecycle transition to IA storage (days)"
  type        = number
  default     = 30
}

# Monitoring Configuration
variable "enable_monitoring" {
  description = "Enable monitoring stack (Prometheus, Grafana, Jaeger)"
  type        = bool
  default     = true
}

variable "monitoring_retention_days" {
  description = "Monitoring data retention in days"
  type        = map(number)
  default = {
    dev     = 7
    staging = 30
    prod    = 90
  }
}

# Security Configuration
variable "enable_waf" {
  description = "Enable AWS WAF"
  type        = bool
  default     = true
}

variable "enable_guardduty" {
  description = "Enable AWS GuardDuty"
  type        = bool
  default     = true
}

variable "enable_secrets_rotation" {
  description = "Enable automatic secrets rotation"
  type        = bool
  default     = true
}

# Backup Configuration
variable "backup_retention_days" {
  description = "Backup retention in days"
  type        = map(number)
  default = {
    dev     = 7
    staging = 30
    prod    = 90
  }
}

variable "backup_schedule" {
  description = "Backup schedule in cron format"
  type        = string
  default     = "0 3 * * *" # Daily at 3 AM
}

# Tags
variable "additional_tags" {
  description = "Additional tags to apply to resources"
  type        = map(string)
  default     = {}
}