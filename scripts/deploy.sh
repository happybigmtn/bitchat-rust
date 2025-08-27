#!/bin/bash
# BitCraps Production Deployment Script
# Handles deployment to Kubernetes using Helm

set -e

# Configuration
NAMESPACE="${NAMESPACE:-bitcraps}"
RELEASE_NAME="${RELEASE_NAME:-bitcraps}"
CHART_PATH="./helm/bitcraps"
VALUES_FILE="${VALUES_FILE:-./helm/bitcraps/values.yaml}"
ENVIRONMENT="${ENVIRONMENT:-production}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check for kubectl
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed"
        exit 1
    fi
    
    # Check for helm
    if ! command -v helm &> /dev/null; then
        log_error "helm is not installed"
        exit 1
    fi
    
    # Check kubernetes connection
    if ! kubectl cluster-info &> /dev/null; then
        log_error "Cannot connect to Kubernetes cluster"
        exit 1
    fi
    
    log_info "Prerequisites check passed"
}

# Create namespace if it doesn't exist
create_namespace() {
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        log_info "Creating namespace: $NAMESPACE"
        kubectl create namespace "$NAMESPACE"
        
        # Label namespace
        kubectl label namespace "$NAMESPACE" \
            name="$NAMESPACE" \
            environment="$ENVIRONMENT"
    else
        log_info "Namespace $NAMESPACE already exists"
    fi
}

# Create secrets
create_secrets() {
    log_info "Creating secrets..."
    
    # Check if backup credentials secret exists
    if ! kubectl get secret backup-credentials -n "$NAMESPACE" &> /dev/null; then
        log_warn "Backup credentials secret not found. Please create it manually:"
        echo "kubectl create secret generic backup-credentials \\"
        echo "  --from-literal=aws-access-key-id=YOUR_KEY_ID \\"
        echo "  --from-literal=aws-secret-access-key=YOUR_SECRET_KEY \\"
        echo "  -n $NAMESPACE"
    fi
    
    # Check if TLS secret exists for ingress
    if ! kubectl get secret bitcraps-tls -n "$NAMESPACE" &> /dev/null; then
        log_warn "TLS secret not found. Cert-manager should create it automatically if configured."
    fi
}

# Update Helm dependencies
update_dependencies() {
    log_info "Updating Helm dependencies..."
    helm dependency update "$CHART_PATH"
}

# Deploy or upgrade release
deploy() {
    log_info "Deploying BitCraps to namespace: $NAMESPACE"
    
    # Check if release exists
    if helm list -n "$NAMESPACE" | grep -q "^$RELEASE_NAME"; then
        log_info "Upgrading existing release: $RELEASE_NAME"
        helm upgrade "$RELEASE_NAME" "$CHART_PATH" \
            --namespace "$NAMESPACE" \
            --values "$VALUES_FILE" \
            --wait \
            --timeout 10m
    else
        log_info "Installing new release: $RELEASE_NAME"
        helm install "$RELEASE_NAME" "$CHART_PATH" \
            --namespace "$NAMESPACE" \
            --values "$VALUES_FILE" \
            --create-namespace \
            --wait \
            --timeout 10m
    fi
}

# Verify deployment
verify_deployment() {
    log_info "Verifying deployment..."
    
    # Check deployment status
    kubectl rollout status deployment/"$RELEASE_NAME" -n "$NAMESPACE" --timeout=5m
    
    # Check pod status
    log_info "Pod status:"
    kubectl get pods -n "$NAMESPACE" -l "app.kubernetes.io/instance=$RELEASE_NAME"
    
    # Check service status
    log_info "Service status:"
    kubectl get svc -n "$NAMESPACE" -l "app.kubernetes.io/instance=$RELEASE_NAME"
    
    # Check ingress status
    if kubectl get ingress -n "$NAMESPACE" -l "app.kubernetes.io/instance=$RELEASE_NAME" &> /dev/null; then
        log_info "Ingress status:"
        kubectl get ingress -n "$NAMESPACE" -l "app.kubernetes.io/instance=$RELEASE_NAME"
    fi
    
    # Run health check
    log_info "Running health check..."
    POD_NAME=$(kubectl get pods -n "$NAMESPACE" -l "app.kubernetes.io/instance=$RELEASE_NAME" -o jsonpath='{.items[0].metadata.name}')
    if [ -n "$POD_NAME" ]; then
        kubectl exec "$POD_NAME" -n "$NAMESPACE" -- curl -s http://localhost:8080/health || true
    fi
}

# Rollback deployment
rollback() {
    log_warn "Rolling back deployment..."
    helm rollback "$RELEASE_NAME" -n "$NAMESPACE"
}

# Main execution
main() {
    case "${1:-deploy}" in
        deploy)
            check_prerequisites
            create_namespace
            create_secrets
            update_dependencies
            deploy
            verify_deployment
            log_info "Deployment completed successfully!"
            ;;
        rollback)
            rollback
            verify_deployment
            ;;
        verify)
            verify_deployment
            ;;
        delete)
            log_warn "Deleting release: $RELEASE_NAME"
            helm uninstall "$RELEASE_NAME" -n "$NAMESPACE"
            ;;
        dry-run)
            check_prerequisites
            update_dependencies
            log_info "Running dry-run..."
            helm install "$RELEASE_NAME" "$CHART_PATH" \
                --namespace "$NAMESPACE" \
                --values "$VALUES_FILE" \
                --dry-run \
                --debug
            ;;
        *)
            echo "Usage: $0 {deploy|rollback|verify|delete|dry-run}"
            exit 1
            ;;
    esac
}

# Handle script interruption
trap 'log_error "Deployment interrupted"; exit 1' INT TERM

# Run main function with arguments
main "$@"
