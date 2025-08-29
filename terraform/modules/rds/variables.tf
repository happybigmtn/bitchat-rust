# RDS Module Variables

variable "environment" {
  description = "Environment name"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID where RDS will be deployed"
  type        = string
}

variable "database_subnet_ids" {
  description = "List of subnet IDs for RDS subnet group"
  type        = list(string)
}

variable "eks_worker_security_group_id" {
  description = "Security group ID of EKS worker nodes"
  type        = string
}

variable "db_instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.r6g.large"
}

variable "allocated_storage" {
  description = "Initial allocated storage in GB"
  type        = number
  default     = 100
}

variable "max_allocated_storage" {
  description = "Maximum allocated storage for autoscaling in GB"
  type        = number
  default     = 1000
}

variable "iops" {
  description = "IOPS for GP3 storage"
  type        = number
  default     = 3000
}

variable "storage_throughput" {
  description = "Storage throughput for GP3 in MiB/s"
  type        = number
  default     = 125
}

variable "db_username" {
  description = "Database master username"
  type        = string
  default     = "bitcraps"
}

variable "db_password" {
  description = "Database master password"
  type        = string
  sensitive   = true
}

variable "backup_retention_period" {
  description = "Backup retention period in days"
  type        = number
  default     = 30
}

variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = true
}

variable "multi_az" {
  description = "Enable Multi-AZ deployment"
  type        = bool
  default     = true
}

variable "skip_final_snapshot" {
  description = "Skip final snapshot on deletion"
  type        = bool
  default     = false
}

variable "create_read_replica" {
  description = "Create a read replica"
  type        = bool
  default     = true
}

variable "replica_instance_class" {
  description = "Instance class for read replica"
  type        = string
  default     = "db.r6g.large"
}

variable "max_connections" {
  description = "Maximum number of database connections"
  type        = string
  default     = "1000"
}

variable "connection_alarm_threshold" {
  description = "CloudWatch alarm threshold for database connections"
  type        = number
  default     = 800
}

variable "sns_topic_arn" {
  description = "SNS topic ARN for alarms"
  type        = string
}
