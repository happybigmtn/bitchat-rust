#!/bin/bash
# BitCraps Production Deployment Script
# Comprehensive deployment automation with safety checks

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
ENVIRONMENT="production"
NAMESPACE="bitcraps-${ENVIRONMENT}"
REGION="us-west-2"
CLUSTER_NAME="bitcraps-${ENVIRONMENT}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Error handler
error_exit() {
    log_error "$1"
    exit 1
}

# Cleanup function
cleanup() {
    log_info "Cleaning up temporary files..."
    rm -f /tmp/bitcraps-*.yaml
    rm -f /tmp/helm-values-*.yaml
}

# Trap cleanup on exit
trap cleanup EXIT

# Validate prerequisites
validate_prerequisites() {
    log_info "Validating prerequisites..."
    
    # Check required tools
    local required_tools=("kubectl" "helm" "docker" "aws" "terraform")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            error_exit "Required tool '$tool' not found in PATH"
        fi
    done
    
    # Check AWS credentials
    if ! aws sts get-caller-identity &> /dev/null; then
        error_exit "AWS credentials not configured or invalid"
    fi
    
    # Check kubectl context
    local current_context
    current_context=$(kubectl config current-context 2>/dev/null || echo "")
    if [[ "$current_context" != *"$CLUSTER_NAME"* ]]; then
        log_warn "Current kubectl context: $current_context"
        log_warn "Expected context containing: $CLUSTER_NAME"
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            error_exit "Deployment cancelled"
        fi
    fi
    
    log_success "Prerequisites validated"
}

# Deploy infrastructure with Terraform
deploy_infrastructure() {
    log_info "Deploying infrastructure with Terraform..."
    
    cd "$PROJECT_ROOT/terraform"
    
    # Initialize Terraform
    terraform init -backend-config="key=${ENVIRONMENT}/terraform.tfstate"
    
    # Plan deployment
    log_info "Creating Terraform plan..."
    terraform plan -var-file="environments/${ENVIRONMENT}.tfvars" -out="/tmp/terraform-${ENVIRONMENT}.plan"
    
    # Show plan summary
    log_info "Terraform plan summary:"
    terraform show -no-color "/tmp/terraform-${ENVIRONMENT}.plan" | head -20
    
    # Confirm deployment
    read -p "Apply Terraform changes? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_warn "Infrastructure deployment skipped"
        return 0
    fi
    
    # Apply changes
    log_info "Applying Terraform changes..."
    terraform apply "/tmp/terraform-${ENVIRONMENT}.plan"
    
    # Update kubeconfig
    log_info "Updating kubeconfig..."
    aws eks update-kubeconfig --region "$REGION" --name "$CLUSTER_NAME"
    
    log_success "Infrastructure deployed successfully"
}

# Build and push Docker images
build_and_push_images() {
    log_info "Building and pushing Docker images..."
    
    cd "$PROJECT_ROOT"
    
    # Get the current git commit
    local git_sha
    git_sha=$(git rev-parse --short HEAD)
    local image_tag="${git_sha}-$(date +%Y%m%d-%H%M%S)"
    
    # Build production image
    log_info "Building production image..."
    docker build -f Dockerfile.production -t "ghcr.io/bitcraps/bitchat-rust:${image_tag}" .
    docker tag "ghcr.io/bitcraps/bitchat-rust:${image_tag}" "ghcr.io/bitcraps/bitchat-rust:latest"
    
    # Build gateway image
    log_info "Building gateway image..."
    docker build -f Dockerfile.gateway -t "ghcr.io/bitcraps/bitchat-rust-gateway:${image_tag}" .
    docker tag "ghcr.io/bitcraps/bitchat-rust-gateway:${image_tag}" "ghcr.io/bitcraps/bitchat-rust-gateway:latest"
    
    # Push images
    log_info "Pushing images to registry..."
    docker push "ghcr.io/bitcraps/bitchat-rust:${image_tag}"
    docker push "ghcr.io/bitcraps/bitchat-rust:latest"
    docker push "ghcr.io/bitcraps/bitchat-rust-gateway:${image_tag}"
    docker push "ghcr.io/bitcraps/bitchat-rust-gateway:latest"
    
    # Store image tag for Helm deployment
    echo "IMAGE_TAG=${image_tag}" > /tmp/build-info.env
    
    log_success "Images built and pushed successfully"
}

# Deploy Kubernetes manifests
deploy_kubernetes() {
    log_info "Deploying Kubernetes manifests..."
    
    cd "$PROJECT_ROOT"
    
    # Create namespace if it doesn't exist
    kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
    
    # Apply CRDs first
    if ls kubernetes/crds/*.yaml 1> /dev/null 2>&1; then
        log_info "Applying Custom Resource Definitions..."
        kubectl apply -f kubernetes/crds/
    fi
    
    # Apply secrets (should be managed externally in production)
    log_warn "Applying secret templates - ensure these are properly configured!"
    kubectl apply -f kubernetes/secrets.yaml
    
    # Apply other manifests
    kubectl apply -f kubernetes/namespaces.yaml
    kubectl apply -f kubernetes/configmaps.yaml
    kubectl apply -f kubernetes/storage.yaml
    kubectl apply -f kubernetes/serviceaccounts.yaml
    kubectl apply -f kubernetes/networkpolicies.yaml
    
    log_success "Kubernetes manifests deployed"
}

# Deploy with Helm
deploy_helm() {
    log_info "Deploying BitCraps with Helm..."
    
    cd "$PROJECT_ROOT"
    
    # Source build info if available
    if [[ -f /tmp/build-info.env ]]; then
        source /tmp/build-info.env
    fi
    
    # Add Helm repositories
    log_info "Adding Helm repositories..."
    helm repo add bitnami https://charts.bitnami.com/bitnami
    helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo add grafana https://grafana.github.io/helm-charts
    helm repo update
    
    # Create values override file
    local values_file="/tmp/helm-values-${ENVIRONMENT}.yaml"
    cat > "$values_file" << EOF
# Production deployment overrides
image:
  tag: "${IMAGE_TAG:-latest}"

environment: ${ENVIRONMENT}

# Load production values
extraEnv:
  - name: DEPLOYMENT_TIME
    value: "$(date -Iseconds)"
  - name: GIT_SHA
    value: "$(git rev-parse HEAD)"
EOF
    
    # Deploy main application
    log_info "Deploying BitCraps application..."
    helm upgrade --install bitcraps ./helm/bitcraps \
        --namespace "$NAMESPACE" \
        --values helm/bitcraps/values-${ENVIRONMENT}.yaml \
        --values "$values_file" \
        --wait \
        --timeout 10m
    
    # Deploy monitoring stack
    log_info "Deploying monitoring stack..."
    kubectl apply -f monitoring/prometheus-rules.yaml
    
    log_success "Helm deployment completed"
}

# Run health checks
run_health_checks() {
    log_info "Running health checks..."
    
    # Wait for pods to be ready
    log_info "Waiting for pods to be ready..."
    kubectl wait --for=condition=ready pod -l app=bitcraps -n "$NAMESPACE" --timeout=300s
    
    # Check service endpoints
    log_info "Checking service endpoints..."
    local services=("bitcraps-core-service" "bitcraps-gateway-service")
    for service in "${services[@]}"; do
        local endpoint
        endpoint=$(kubectl get service "$service" -n "$NAMESPACE" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')
        if [[ -n "$endpoint" ]]; then
            log_info "Service $service endpoint: $endpoint"
            # Test health endpoint
            if curl -f "http://$endpoint:8080/health" &> /dev/null; then
                log_success "Health check passed for $service"
            else
                log_warn "Health check failed for $service"
            fi
        else
            log_warn "No endpoint found for $service"
        fi
    done
    
    # Check metrics endpoint
    log_info "Checking metrics endpoints..."
    local metrics_endpoint
    metrics_endpoint=$(kubectl get service bitcraps-metrics -n "$NAMESPACE" -o jsonpath='{.spec.clusterIP}')
    if [[ -n "$metrics_endpoint" ]]; then
        kubectl run curl-test --image=curlimages/curl --rm -i --tty --restart=Never \
            -- curl -s "http://$metrics_endpoint:9091/metrics" | head -5
    fi
    
    log_success "Health checks completed"
}

# Generate deployment report
generate_report() {
    log_info "Generating deployment report..."
    
    local report_file="/tmp/bitcraps-deployment-report-$(date +%Y%m%d-%H%M%S).txt"
    
    cat > "$report_file" << EOF
BitCraps Production Deployment Report
====================================

Deployment Time: $(date -Iseconds)
Environment: ${ENVIRONMENT}
Namespace: ${NAMESPACE}
Cluster: ${CLUSTER_NAME}
Git Commit: $(git rev-parse HEAD)
Image Tag: ${IMAGE_TAG:-latest}

Deployed Resources:
$(kubectl get all -n "$NAMESPACE")

Service Endpoints:
$(kubectl get services -n "$NAMESPACE" -o wide)

Pod Status:
$(kubectl get pods -n "$NAMESPACE" -o wide)

Ingress Configuration:
$(kubectl get ingress -n "$NAMESPACE" -o wide)

Resource Usage:
$(kubectl top pods -n "$NAMESPACE" 2>/dev/null || echo "Metrics not available")

Helm Releases:
$(helm list -n "$NAMESPACE")

EOF
    
    log_info "Deployment report generated: $report_file"
    
    # Display summary
    echo
    log_success "=== DEPLOYMENT SUMMARY ==="
    log_success "Environment: $ENVIRONMENT"
    log_success "Namespace: $NAMESPACE"
    log_success "Git Commit: $(git rev-parse --short HEAD)"
    log_success "Deployment completed at: $(date)"
    echo
}

# Post-deployment tasks
post_deployment() {
    log_info "Running post-deployment tasks..."
    
    # Tag the current commit
    local deployment_tag="deploy-${ENVIRONMENT}-$(date +%Y%m%d-%H%M%S)"
    git tag "$deployment_tag" || log_warn "Failed to create git tag"
    
    # Notify monitoring systems
    if [[ -n "${SLACK_WEBHOOK_URL:-}" ]]; then
        curl -X POST -H 'Content-type: application/json' \
            --data "{\"text\":\"BitCraps ${ENVIRONMENT} deployment completed successfully at $(date)\"}" \
            "$SLACK_WEBHOOK_URL" || log_warn "Failed to send Slack notification"
    fi
    
    # Create deployment annotation in monitoring
    kubectl annotate deployment bitcraps-core -n "$NAMESPACE" \
        "deployment.bitcraps.io/timestamp=$(date -Iseconds)" \
        "deployment.bitcraps.io/git-sha=$(git rev-parse HEAD)" \
        --overwrite || log_warn "Failed to annotate deployment"
    
    log_success "Post-deployment tasks completed"
}

# Rollback function
rollback_deployment() {
    log_warn "Rolling back deployment..."
    
    # Rollback Helm release
    helm rollback bitcraps -n "$NAMESPACE" || log_error "Helm rollback failed"
    
    # Wait for rollback to complete
    kubectl rollout status deployment/bitcraps-core -n "$NAMESPACE" --timeout=300s
    
    log_success "Rollback completed"
}

# Main deployment function
main() {
    log_info "Starting BitCraps production deployment..."
    log_info "Environment: $ENVIRONMENT"
    log_info "Namespace: $NAMESPACE"
    log_info "Cluster: $CLUSTER_NAME"
    echo
    
    # Parse command line options
    local skip_infra=false
    local skip_build=false
    local dry_run=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --skip-infrastructure)
                skip_infra=true
                shift
                ;;
            --skip-build)
                skip_build=true
                shift
                ;;
            --dry-run)
                dry_run=true
                shift
                ;;
            --rollback)
                rollback_deployment
                exit 0
                ;;
            -h|--help)
                echo "Usage: $0 [OPTIONS]"
                echo "Options:"
                echo "  --skip-infrastructure  Skip Terraform infrastructure deployment"
                echo "  --skip-build           Skip Docker image build and push"
                echo "  --dry-run              Perform a dry run without making changes"
                echo "  --rollback             Rollback to previous deployment"
                echo "  -h, --help             Show this help message"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    if [[ "$dry_run" == "true" ]]; then
        log_info "DRY RUN MODE - No changes will be made"
        export KUBECTL_FLAGS="--dry-run=client"
        export HELM_FLAGS="--dry-run"
    fi
    
    # Execute deployment steps
    validate_prerequisites
    
    if [[ "$skip_infra" != "true" ]]; then
        deploy_infrastructure
    else
        log_info "Skipping infrastructure deployment"
    fi
    
    if [[ "$skip_build" != "true" ]]; then
        build_and_push_images
    else
        log_info "Skipping image build"
    fi
    
    deploy_kubernetes
    deploy_helm
    
    if [[ "$dry_run" != "true" ]]; then
        run_health_checks
        generate_report
        post_deployment
    fi
    
    log_success "BitCraps production deployment completed successfully!"
}

# Run main function with all arguments
main "$@"
