# BitCraps Production Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying BitCraps to production environments using Kubernetes, Helm, and Terraform. The deployment is designed for high availability, security, and scalability.

## Architecture

### Infrastructure Components

- **EKS Cluster**: Managed Kubernetes cluster on AWS
- **RDS PostgreSQL**: Primary database with read replicas
- **ElastiCache Redis**: Caching and session storage
- **Application Load Balancer**: Traffic distribution
- **CloudFront**: CDN and DDoS protection
- **S3**: Backup and static asset storage
- **KMS**: Encryption key management
- **Secrets Manager**: Secure credential storage

### Application Components

- **Core Nodes**: Main consensus and gaming logic (3-20 replicas)
- **Gateway Nodes**: P2P network entry points (2-10 replicas)
- **TURN Servers**: NAT traversal for P2P connections (2 replicas)
- **Monitoring Stack**: Prometheus, Grafana, Alertmanager

## Prerequisites

### Required Tools

```bash
# Install required CLI tools
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
curl -LO https://releases.hashicorp.com/terraform/1.6.0/terraform_1.6.0_linux_amd64.zip
wget https://github.com/aws/aws-cli/releases/latest/download/awscli-exe-linux-x86_64.zip
```

### AWS Configuration

```bash
# Configure AWS credentials
aws configure set aws_access_key_id YOUR_ACCESS_KEY
aws configure set aws_secret_access_key YOUR_SECRET_KEY
aws configure set default.region us-west-2

# Verify access
aws sts get-caller-identity
```

### Docker Registry Access

```bash
# Login to GitHub Container Registry
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin
```

## Deployment Steps

### 1. Infrastructure Deployment

#### Initialize Terraform Backend

```bash
cd terraform

# Create S3 bucket for state (if not exists)
aws s3 mb s3://bitcraps-terraform-state --region us-west-2

# Create DynamoDB table for locking
aws dynamodb create-table \
    --table-name bitcraps-terraform-locks \
    --attribute-definitions AttributeName=LockID,AttributeType=S \
    --key-schema AttributeName=LockID,KeyType=HASH \
    --billing-mode PAY_PER_REQUEST

# Initialize Terraform
terraform init
```

#### Deploy Infrastructure

```bash
# Plan deployment
terraform plan -var-file="environments/production.tfvars" -out=production.plan

# Review plan
terraform show production.plan

# Apply changes
terraform apply production.plan

# Update kubeconfig
aws eks update-kubeconfig --region us-west-2 --name bitcraps-production
```

### 2. Application Deployment

#### Using the Automated Script

```bash
# Full deployment (recommended)
./scripts/deploy-production.sh

# Skip infrastructure deployment
./scripts/deploy-production.sh --skip-infrastructure

# Dry run
./scripts/deploy-production.sh --dry-run
```

#### Manual Deployment Steps

```bash
# 1. Build and push images
docker build -f Dockerfile.production -t ghcr.io/bitcraps/bitchat-rust:latest .
docker build -f Dockerfile.gateway -t ghcr.io/bitcraps/bitchat-rust-gateway:latest .
docker push ghcr.io/bitcraps/bitchat-rust:latest
docker push ghcr.io/bitcraps/bitchat-rust-gateway:latest

# 2. Deploy Kubernetes manifests
kubectl apply -f kubernetes/namespaces.yaml
kubectl apply -f kubernetes/secrets.yaml
kubectl apply -f kubernetes/configmaps.yaml
kubectl apply -f kubernetes/storage.yaml
kubectl apply -f kubernetes/serviceaccounts.yaml
kubectl apply -f kubernetes/networkpolicies.yaml

# 3. Deploy with Helm
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update

helm upgrade --install bitcraps ./helm/bitcraps \
    --namespace bitcraps-production \
    --values helm/bitcraps/values-production.yaml \
    --wait \
    --timeout 10m
```

### 3. Monitoring Deployment

```bash
# Deploy Prometheus rules
kubectl apply -f monitoring/prometheus-rules.yaml

# Deploy Grafana dashboards
kubectl apply -f monitoring/dashboards/

# Check monitoring stack
kubectl get pods -n bitcraps-monitoring
```

### 4. Security Configuration

```bash
# Apply security policies
kubectl apply -f security/security-policies.yaml

# Verify network policies
kubectl get networkpolicies -n bitcraps-production

# Check pod security
kubectl get pods -n bitcraps-production -o jsonpath='{.items[*].spec.securityContext}'
```

## Configuration

### Environment Variables

Key environment variables that must be configured:

```bash
# Database
POSTGRES_PASSWORD=<secure-password>
DATABASE_URL=<connection-string>

# Cryptography
TREASURY_ADDRESS=<treasury-wallet-address>
MASTER_SIGNING_KEY=<base64-encoded-key>
ENCRYPTION_KEY=<base64-encoded-key>

# External Services
REDIS_PASSWORD=<redis-auth-token>
SLACK_WEBHOOK_URL=<slack-webhook>

# AWS
AWS_REGION=us-west-2
S3_BACKUP_BUCKET=bitcraps-production-backups
```

### Secrets Management

Production secrets are managed through AWS Secrets Manager:

```bash
# Create database secret
aws secretsmanager create-secret \
    --name "bitcraps/production/database" \
    --description "Database credentials" \
    --secret-string '{"username":"bitcraps","password":"SECURE_PASSWORD"}'

# Create Redis secret
aws secretsmanager create-secret \
    --name "bitcraps/production/redis" \
    --description "Redis authentication" \
    --secret-string '{"password":"SECURE_REDIS_PASSWORD"}'
```

### Scaling Configuration

#### Horizontal Pod Autoscaler

```yaml
# Automatically scales based on CPU/Memory usage
minReplicas: 3
maxReplicas: 20
targetCPUUtilization: 70%
targetMemoryUtilization: 80%
```

#### Cluster Autoscaler

```bash
# Node groups will scale automatically based on pod requirements
min_size = 3
max_size = 100
desired_size = 5
```

## Health Checks

### Application Health

```bash
# Check pod status
kubectl get pods -n bitcraps-production

# Check service endpoints
kubectl get services -n bitcraps-production

# Test health endpoints
curl -f https://api.bitcraps.io/health
curl -f https://gateway.bitcraps.io/health
```

### Database Health

```bash
# Check database connections
kubectl exec -it deployment/bitcraps-core -n bitcraps-production -- \
    bitcraps db health

# Check replica lag
aws rds describe-db-instances --db-instance-identifier bitcraps-production-replica
```

### Monitoring Metrics

```bash
# Check Prometheus targets
kubectl port-forward -n bitcraps-monitoring svc/prometheus 9090:9090
# Visit http://localhost:9090/targets

# Check Grafana dashboards
kubectl port-forward -n bitcraps-monitoring svc/grafana 3000:3000
# Visit http://localhost:3000
```

## Backup and Recovery

### Database Backups

```bash
# Manual backup
aws rds create-db-snapshot \
    --db-snapshot-identifier bitcraps-manual-$(date +%Y%m%d-%H%M%S) \
    --db-instance-identifier bitcraps-production

# Restore from backup
aws rds restore-db-instance-from-db-snapshot \
    --db-instance-identifier bitcraps-restored \
    --db-snapshot-identifier bitcraps-manual-20240101-120000
```

### Application State Backup

```bash
# Backup persistent volumes
kubectl exec -it deployment/bitcraps-core -n bitcraps-production -- \
    bitcraps backup create --destination s3://bitcraps-production-backups/

# List available backups
aws s3 ls s3://bitcraps-production-backups/
```

## Rollback Procedures

### Application Rollback

```bash
# Rollback to previous Helm release
helm rollback bitcraps -n bitcraps-production

# Or use the deployment script
./scripts/deploy-production.sh --rollback

# Verify rollback
kubectl rollout status deployment/bitcraps-core -n bitcraps-production
```

### Database Rollback

```bash
# Point-in-time recovery (last resort)
aws rds restore-db-instance-to-point-in-time \
    --source-db-instance-identifier bitcraps-production \
    --target-db-instance-identifier bitcraps-recovery \
    --restore-time 2024-01-01T12:00:00.000Z
```

## Troubleshooting

### Common Issues

#### Pods Not Starting

```bash
# Check pod events
kubectl describe pod <pod-name> -n bitcraps-production

# Check logs
kubectl logs <pod-name> -n bitcraps-production --previous

# Check resource constraints
kubectl top pods -n bitcraps-production
```

#### Database Connection Issues

```bash
# Test database connectivity
kubectl run postgres-test --image=postgres:15 --rm -it --restart=Never -- \
    psql -h bitcraps-postgresql -U bitcraps -d bitcraps

# Check security groups
aws ec2 describe-security-groups --filters Name=group-name,Values=bitcraps-production-rds-sg
```

#### Performance Issues

```bash
# Check resource usage
kubectl top nodes
kubectl top pods -n bitcraps-production

# Check HPA status
kubectl get hpa -n bitcraps-production

# Check cluster autoscaler
kubectl logs -n kube-system deployment/cluster-autoscaler
```

### Logs and Debugging

```bash
# Application logs
kubectl logs -f deployment/bitcraps-core -n bitcraps-production

# System logs
journalctl -u kubelet -f

# AWS CloudWatch logs
aws logs tail /aws/eks/bitcraps-production/cluster --follow
```

## Security Considerations

### Network Security

- All pod-to-pod communication encrypted with Istio mTLS
- Network policies restrict traffic between components
- WAF protects against web application attacks
- DDoS protection via CloudFront

### Data Security

- Database encryption at rest with KMS
- Secrets managed via AWS Secrets Manager
- Container images scanned for vulnerabilities
- Regular security updates applied

### Access Control

- RBAC configured for least privilege access
- Service accounts with minimal permissions
- Pod security policies enforce security standards
- Audit logging enabled for all API calls

## Maintenance

### Regular Tasks

```bash
# Update container images
docker pull ghcr.io/bitcraps/bitchat-rust:latest
helm upgrade bitcraps ./helm/bitcraps --reuse-values

# Update Kubernetes cluster
aws eks update-cluster-version --name bitcraps-production --version 1.28

# Rotate secrets
aws secretsmanager update-secret --secret-id bitcraps/production/database \
    --secret-string '{"username":"bitcraps","password":"NEW_PASSWORD"}'
```

### Monitoring and Alerting

- Prometheus scrapes metrics every 15 seconds
- Grafana dashboards provide real-time visibility
- PagerDuty integration for critical alerts
- Slack notifications for warnings

## Performance Tuning

### Resource Optimization

```yaml
# Optimize resource requests/limits
resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 4Gi
```

### Database Optimization

```sql
-- Optimize PostgreSQL settings
ALTER SYSTEM SET shared_buffers = '1GB';
ALTER SYSTEM SET effective_cache_size = '3GB';
ALTER SYSTEM SET maintenance_work_mem = '256MB';
SELECT pg_reload_conf();
```

## Cost Optimization

### Instance Types

- Use Graviton2 instances for better price/performance
- Mix on-demand and spot instances for cost savings
- Right-size instances based on actual usage

### Storage

- Use GP3 volumes with optimized IOPS/throughput
- Implement lifecycle policies for S3 backups
- Regular cleanup of unused resources

## Support and Contact

For production support:

- **Emergency**: +1-555-BITCRAPS
- **Email**: ops@bitcraps.io
- **Slack**: #ops-production
- **Documentation**: https://docs.bitcraps.io

## License

This deployment configuration is part of the BitCraps project and is licensed under MIT OR Apache-2.0.
