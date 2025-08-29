# RDS PostgreSQL for BitCraps
# Production-ready database with high availability

# DB Subnet Group
resource "aws_db_subnet_group" "bitcraps" {
  name       = "bitcraps-${var.environment}-db-subnet-group"
  subnet_ids = var.database_subnet_ids

  tags = {
    Name        = "BitCraps DB Subnet Group"
    Environment = var.environment
  }
}

# Security Group for RDS
resource "aws_security_group" "rds" {
  name        = "bitcraps-${var.environment}-rds-sg"
  description = "Security group for BitCraps RDS instance"
  vpc_id      = var.vpc_id

  ingress {
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [var.eks_worker_security_group_id]
    description     = "PostgreSQL access from EKS workers"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "All outbound traffic"
  }

  tags = {
    Name        = "BitCraps RDS Security Group"
    Environment = var.environment
  }
}

# KMS Key for RDS encryption
resource "aws_kms_key" "rds" {
  description = "KMS key for BitCraps RDS encryption"
  
  tags = {
    Name        = "BitCraps RDS KMS Key"
    Environment = var.environment
  }
}

resource "aws_kms_alias" "rds" {
  name          = "alias/bitcraps-${var.environment}-rds"
  target_key_id = aws_kms_key.rds.key_id
}

# Parameter Group for PostgreSQL optimization
resource "aws_db_parameter_group" "bitcraps" {
  name   = "bitcraps-${var.environment}-postgres-params"
  family = "postgres15"

  parameter {
    name  = "shared_preload_libraries"
    value = "pg_stat_statements"
  }

  parameter {
    name  = "log_statement"
    value = "all"
  }

  parameter {
    name  = "log_min_duration_statement"
    value = "1000"
  }

  parameter {
    name  = "log_checkpoints"
    value = "on"
  }

  parameter {
    name  = "log_connections"
    value = "on"
  }

  parameter {
    name  = "log_disconnections"
    value = "on"
  }

  parameter {
    name  = "log_lock_waits"
    value = "on"
  }

  parameter {
    name  = "max_connections"
    value = var.max_connections
  }

  tags = {
    Name        = "BitCraps DB Parameter Group"
    Environment = var.environment
  }
}

# RDS Instance
resource "aws_db_instance" "bitcraps" {
  identifier = "bitcraps-${var.environment}"

  # Engine configuration
  engine         = "postgres"
  engine_version = "15.4"
  instance_class = var.db_instance_class

  # Storage configuration
  allocated_storage     = var.allocated_storage
  max_allocated_storage = var.max_allocated_storage
  storage_type          = "gp3"
  storage_encrypted     = true
  kms_key_id           = aws_kms_key.rds.arn
  iops                 = var.iops
  storage_throughput   = var.storage_throughput

  # Database configuration
  db_name  = "bitcraps"
  username = var.db_username
  password = var.db_password
  port     = 5432

  # Network configuration
  db_subnet_group_name   = aws_db_subnet_group.bitcraps.name
  vpc_security_group_ids = [aws_security_group.rds.id]
  publicly_accessible    = false

  # Backup configuration
  backup_retention_period = var.backup_retention_period
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"
  copy_tags_to_snapshot  = true
  delete_automated_backups = false
  deletion_protection    = var.deletion_protection

  # Monitoring
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_enhanced_monitoring.arn
  enabled_cloudwatch_logs_exports = ["postgresql"]
  performance_insights_enabled = true
  performance_insights_retention_period = 7

  # Parameter and option groups
  parameter_group_name = aws_db_parameter_group.bitcraps.name

  # Multi-AZ for production
  multi_az = var.multi_az

  # Prevent accidental deletion
  skip_final_snapshot = var.skip_final_snapshot
  final_snapshot_identifier = "bitcraps-${var.environment}-final-${formatdate("YYYY-MM-DD-hhmm", timestamp())}"

  tags = {
    Name        = "BitCraps Database"
    Environment = var.environment
    Backup      = "required"
  }

  lifecycle {
    ignore_changes = [password]
  }
}

# Read Replica for production
resource "aws_db_instance" "bitcraps_replica" {
  count = var.create_read_replica ? 1 : 0

  identifier = "bitcraps-${var.environment}-replica"
  
  # Replica configuration
  replicate_source_db = aws_db_instance.bitcraps.identifier
  instance_class      = var.replica_instance_class
  publicly_accessible = false
  
  # Storage (inherited from source)
  storage_encrypted = true
  
  # Monitoring
  monitoring_interval = 60
  monitoring_role_arn = aws_iam_role.rds_enhanced_monitoring.arn
  performance_insights_enabled = true

  tags = {
    Name        = "BitCraps Database Replica"
    Environment = var.environment
    Role        = "read-replica"
  }
}

# IAM Role for Enhanced Monitoring
resource "aws_iam_role" "rds_enhanced_monitoring" {
  name = "bitcraps-${var.environment}-rds-monitoring-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "monitoring.rds.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name        = "BitCraps RDS Monitoring Role"
    Environment = var.environment
  }
}

resource "aws_iam_role_policy_attachment" "rds_enhanced_monitoring" {
  role       = aws_iam_role.rds_enhanced_monitoring.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonRDSEnhancedMonitoringRole"
}

# CloudWatch Alarms
resource "aws_cloudwatch_metric_alarm" "database_cpu" {
  alarm_name          = "bitcraps-${var.environment}-database-cpu"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "CPUUtilization"
  namespace           = "AWS/RDS"
  period              = "120"
  statistic           = "Average"
  threshold           = "80"
  alarm_description   = "This metric monitors RDS CPU utilization"
  alarm_actions       = [var.sns_topic_arn]

  dimensions = {
    DBInstanceIdentifier = aws_db_instance.bitcraps.id
  }

  tags = {
    Name        = "BitCraps DB CPU Alarm"
    Environment = var.environment
  }
}

resource "aws_cloudwatch_metric_alarm" "database_connection_count" {
  alarm_name          = "bitcraps-${var.environment}-database-connections"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "2"
  metric_name         = "DatabaseConnections"
  namespace           = "AWS/RDS"
  period              = "120"
  statistic           = "Average"
  threshold           = var.connection_alarm_threshold
  alarm_description   = "This metric monitors RDS connection count"
  alarm_actions       = [var.sns_topic_arn]

  dimensions = {
    DBInstanceIdentifier = aws_db_instance.bitcraps.id
  }

  tags = {
    Name        = "BitCraps DB Connection Alarm"
    Environment = var.environment
  }
}
