# BitCraps Kubernetes Deployment Guide

This guide provides comprehensive instructions for deploying the BitCraps gaming platform on Kubernetes using the provided manifests and Helm charts.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Architecture Overview](#architecture-overview)
- [Deployment Options](#deployment-options)
- [Configuration](#configuration)
- [Monitoring & Observability](#monitoring--observability)
- [Security](#security)
- [Scaling & Performance](#scaling--performance)
- [Troubleshooting](#troubleshooting)
- [Production Checklist](#production-checklist)

## Prerequisites

### Required Tools
- Kubernetes cluster (v1.25+)
- kubectl (v1.25+)
- Helm (v3.12+)
- Docker (for building custom images)

### Required Resources
- **Minimum**: 8 CPU cores, 16GB RAM, 200GB storage
- **Recommended**: 16 CPU cores, 32GB RAM, 500GB SSD storage
- **Production**: 32 CPU cores, 64GB RAM, 1TB SSD storage

### Cluster Requirements
- CSI-compatible storage provider
- LoadBalancer support (AWS ALB, GKE, etc.)
- NGINX Ingress Controller
- cert-manager for TLS certificates
- Prometheus Operator for monitoring

## Quick Start

### 1. Clone Repository
```bash
git clone https://github.com/bitcraps/bitcraps-rust.git
cd bitcraps-rust
```

### 2. Create Namespace
```bash
kubectl apply -f k8s/namespace.yaml
```

### 3. Deploy with Raw Manifests
```bash
# Deploy in order
kubectl apply -f k8s/configmaps/
kubectl apply -f k8s/secrets/
kubectl apply -f k8s/storage/
kubectl apply -f k8s/deployments/
kubectl apply -f k8s/services/
kubectl apply -f k8s/ingress/
kubectl apply -f k8s/monitoring/
```

### 4. Deploy with Helm (Recommended)
```bash
# Add required repositories
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx
helm repo add jetstack https://charts.jetstack.io
helm repo update

# Install dependencies
helm install cert-manager jetstack/cert-manager \
  --namespace cert-manager \
  --create-namespace \
  --version v1.12.2 \
  --set installCRDs=true

helm install ingress-nginx ingress-nginx/ingress-nginx \
  --namespace ingress-nginx \
  --create-namespace

# Install BitCraps
helm install bitcraps helm/bitcraps/ \
  --namespace bitcraps \
  --create-namespace \
  --values helm/bitcraps/values.yaml
```

### 5. Verify Deployment
```bash
kubectl get pods -n bitcraps
kubectl get services -n bitcraps
kubectl get ingress -n bitcraps
```

## Architecture Overview

### Services Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   API Gateway   │    │   Game Engine   │    │   Consensus     │
│  (Load Balancer)│────│  (Game Logic)   │────│  (Byzantine FT) │
│     Port: 8080  │    │    Port: 8000   │    │    Port: 8001   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   PostgreSQL    │
                    │   Database      │
                    │   Port: 5432    │
                    └─────────────────┘
```

### Component Details

#### Game Engine
- **Replicas**: 3 (auto-scaling 3-20)
- **Resources**: 500m CPU, 1Gi RAM (limits: 2 CPU, 4Gi RAM)
- **Storage**: 50Gi persistent volume
- **Purpose**: Core game logic, player management, game state

#### Consensus Service  
- **Replicas**: 3 (Byzantine fault tolerance)
- **Resources**: 1 CPU, 2Gi RAM (limits: 4 CPU, 8Gi RAM)
- **Storage**: 100Gi consensus data + 200Gi Raft logs
- **Purpose**: Distributed consensus, transaction ordering

#### API Gateway
- **Replicas**: 2 (auto-scaling 2-15)  
- **Resources**: 200m CPU, 512Mi RAM (limits: 1 CPU, 2Gi RAM)
- **Purpose**: HTTP/WebSocket API, load balancing, rate limiting

#### Database (PostgreSQL)
- **Replicas**: 1 (StatefulSet)
- **Resources**: 500m CPU, 1Gi RAM (limits: 2 CPU, 4Gi RAM)
- **Storage**: 100Gi persistent volume
- **Purpose**: Game data, player accounts, transaction history

#### Monitoring Stack
- **Prometheus**: Metrics collection and storage
- **Grafana**: Dashboards and visualization
- **AlertManager**: Alert routing and management

## Deployment Options

### Option 1: Raw Kubernetes Manifests
Best for: Custom deployments, advanced users, CI/CD integration

```bash
# Deploy specific components
kubectl apply -f k8s/deployments/game-engine.yaml
kubectl apply -f k8s/deployments/consensus.yaml
kubectl apply -f k8s/deployments/api-gateway.yaml
```

### Option 2: Helm Chart
Best for: Production deployments, easy customization, upgrades

```bash
# Install with custom values
helm install bitcraps helm/bitcraps/ \
  --values custom-values.yaml \
  --set app.environment=production \
  --set gameEngine.replicaCount=5
```

### Option 3: ArgoCD GitOps
Best for: Automated deployments, multi-environment management

```bash
# Apply ArgoCD applications
kubectl apply -f k8s/argocd/application.yaml
```

## Configuration

### Environment Variables
Configure services through values.yaml or environment variables:

```yaml
app:
  environment: production
  logLevel: info
  network:
    port: 8000
    maxConcurrentConnections: 5000
  gaming:
    maxGames: 1000
    sessionTimeout: 3600s
  security:
    enableTls: true
    encryptionEnabled: true
```

### Secrets Management
Update secrets before production deployment:

```bash
# Generate secure passwords
export DB_PASSWORD=$(openssl rand -base64 32)
export JWT_SECRET=$(openssl rand -base64 32)
export ENCRYPTION_KEY=$(openssl rand -base64 32)

# Update secrets
kubectl create secret generic bitcraps-app-secrets \
  --from-literal=db-password="$DB_PASSWORD" \
  --from-literal=jwt-secret="$JWT_SECRET" \
  --from-literal=encryption-key="$ENCRYPTION_KEY" \
  -n bitcraps
```

### Storage Configuration
Configure storage classes for different performance tiers:

```yaml
storage:
  classes:
    fastSsd:
      name: fast-ssd
      provisioner: kubernetes.io/aws-ebs
      parameters:
        type: gp3
        iops: "3000"
```

## Monitoring & Observability

### Access Dashboards
- **Grafana**: https://grafana.bitcraps.io
- **Prometheus**: https://prometheus.bitcraps.internal
- **API Health**: https://api.bitcraps.io/health

### Key Metrics
- Active games per pod
- Consensus latency (p99)
- Database connection utilization
- HTTP request rate and errors
- WebSocket connection count

### Alerts
Configured alerts for:
- Service unavailability
- High error rates
- Resource exhaustion
- Consensus issues
- Database problems

### Logs
Access logs through kubectl:
```bash
# View game engine logs
kubectl logs -f deployment/bitcraps-game-engine -n bitcraps

# View consensus logs  
kubectl logs -f deployment/bitcraps-consensus -n bitcraps

# View all logs with labels
kubectl logs -f -l app.kubernetes.io/name=bitcraps -n bitcraps
```

## Security

### Network Security
- Network policies restrict inter-pod communication
- Ingress controls external access
- TLS encryption for all external traffic
- Service mesh (optional) for internal encryption

### Pod Security
- Non-root containers
- Read-only root filesystems
- Dropped capabilities
- Security contexts enforced
- Pod Security Standards: Restricted

### Secrets Management
- Kubernetes secrets for sensitive data
- Support for external secret managers (Vault, AWS Secrets Manager)
- Automatic secret rotation (planned)

### RBAC
- Least privilege service accounts
- Fine-grained permissions per service
- Namespace isolation

## Scaling & Performance

### Horizontal Pod Autoscaling
Game Engine and API Gateway auto-scale based on:
- CPU utilization (70%)
- Memory utilization (80%)
- Custom metrics (active games, request rate)

### Vertical Pod Autoscaling
Consensus and Database services use VPA for resource optimization:
- Automatic resource recommendation
- Right-sizing for optimal performance

### Performance Tuning
- Fast SSD storage for critical components
- Optimized database configuration
- Connection pooling and caching
- Resource limits prevent resource starvation

### Load Testing
Run load tests to validate performance:
```bash
# Deploy load testing tools
kubectl apply -f tests/load-testing/

# Run performance tests
kubectl run load-test --image=loadtest:latest \
  --env="TARGET_URL=http://bitcraps-api-gateway:8080" \
  --env="CONCURRENT_USERS=100"
```

## Troubleshooting

### Common Issues

#### Pods Not Starting
```bash
# Check pod status
kubectl get pods -n bitcraps
kubectl describe pod <pod-name> -n bitcraps

# Check logs
kubectl logs <pod-name> -n bitcraps

# Check resource constraints
kubectl top pods -n bitcraps
```

#### Service Connectivity Issues  
```bash
# Test service connectivity
kubectl run debug --image=nicolaka/netshoot -it --rm
nslookup bitcraps-game-engine.bitcraps.svc.cluster.local
telnet bitcraps-game-engine.bitcraps.svc.cluster.local 8000
```

#### Storage Issues
```bash
# Check PVC status
kubectl get pvc -n bitcraps
kubectl describe pvc <pvc-name> -n bitcraps

# Check storage class
kubectl get storageclass
```

#### Database Connection Issues
```bash
# Test database connectivity
kubectl run postgres-client --image=postgres:15 -it --rm \
  --env="PGPASSWORD=$DB_PASSWORD" \
  -- psql -h bitcraps-database -U bitcraps -d bitcraps
```

### Debug Commands
```bash
# Get cluster information
kubectl cluster-info
kubectl get nodes
kubectl get events --sort-by='.lastTimestamp'

# Check resource usage
kubectl top nodes
kubectl top pods -n bitcraps

# Validate manifests
kubectl apply --dry-run=client -f k8s/deployments/
helm template bitcraps helm/bitcraps/ | kubectl apply --dry-run=client -f -
```

## Production Checklist

### Pre-deployment
- [ ] Update all default passwords and secrets
- [ ] Configure proper resource requests and limits
- [ ] Set up monitoring and alerting
- [ ] Configure backup strategy
- [ ] Review security configurations
- [ ] Set up TLS certificates
- [ ] Configure ingress and load balancing
- [ ] Plan for disaster recovery

### Deployment
- [ ] Deploy to staging environment first
- [ ] Run smoke tests
- [ ] Verify all services are healthy
- [ ] Check monitoring dashboards
- [ ] Test backup and restore procedures
- [ ] Validate autoscaling behavior
- [ ] Perform load testing

### Post-deployment
- [ ] Monitor metrics and logs
- [ ] Set up regular health checks
- [ ] Schedule maintenance windows
- [ ] Document operational procedures
- [ ] Train operations team
- [ ] Establish incident response procedures

### Ongoing Maintenance
- [ ] Regular security updates
- [ ] Performance optimization
- [ ] Capacity planning
- [ ] Backup verification
- [ ] Disaster recovery testing
- [ ] Documentation updates

## Support

For issues and questions:
- GitHub Issues: https://github.com/bitcraps/bitcraps-rust/issues
- Documentation: https://docs.bitcraps.io
- Community: https://discord.gg/bitcraps

## License

This deployment guide is part of the BitCraps project and is licensed under MIT OR Apache-2.0.