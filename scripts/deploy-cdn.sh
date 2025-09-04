#!/bin/bash
# CDN Deployment Script for BitCraps
# Deploys infrastructure and assets to multiple CDN providers

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
INFRA_DIR="$PROJECT_ROOT/infrastructure/cdn"
TERRAFORM_DIR="$INFRA_DIR/terraform"

# Environment configuration
ENVIRONMENT="${ENVIRONMENT:-prod}"
TERRAFORM_WORKSPACE="${TERRAFORM_WORKSPACE:-$ENVIRONMENT}"
DEPLOY_REGION="${DEPLOY_REGION:-us-west-2}"

# CDN Provider flags
DEPLOY_CLOUDFLARE="${DEPLOY_CLOUDFLARE:-true}"
DEPLOY_FASTLY="${DEPLOY_FASTLY:-true}"
DEPLOY_CLOUDFRONT="${DEPLOY_CLOUDFRONT:-true}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[WARNING] $1${NC}"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}" >&2
}

success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    log "Checking deployment prerequisites..."
    
    local missing_deps=()
    
    # Check required tools
    command -v terraform >/dev/null || missing_deps+=("terraform")
    command -v aws >/dev/null || missing_deps+=("aws-cli")
    command -v jq >/dev/null || missing_deps+=("jq")
    
    # Check optional tools based on enabled providers
    if [[ "$DEPLOY_CLOUDFLARE" == "true" ]]; then
        command -v wrangler >/dev/null || missing_deps+=("wrangler")
    fi
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        error "Missing required dependencies: ${missing_deps[*]}"
        exit 1
    fi
    
    # Check AWS credentials
    if ! aws sts get-caller-identity >/dev/null 2>&1; then
        error "AWS credentials not configured or invalid"
        exit 1
    fi
    
    # Check environment variables
    local required_vars=()
    
    if [[ "$DEPLOY_CLOUDFLARE" == "true" ]]; then
        [[ -n "${CLOUDFLARE_API_TOKEN:-}" ]] || required_vars+=("CLOUDFLARE_API_TOKEN")
        [[ -n "${CLOUDFLARE_ZONE_ID:-}" ]] || required_vars+=("CLOUDFLARE_ZONE_ID")
    fi
    
    if [[ "$DEPLOY_FASTLY" == "true" ]]; then
        [[ -n "${FASTLY_API_KEY:-}" ]] || required_vars+=("FASTLY_API_KEY")
    fi
    
    if [[ ${#required_vars[@]} -gt 0 ]]; then
        error "Missing required environment variables: ${required_vars[*]}"
        exit 1
    fi
    
    success "Prerequisites check passed"
}

# Initialize Terraform
init_terraform() {
    log "Initializing Terraform..."
    
    cd "$TERRAFORM_DIR"
    
    # Initialize Terraform
    terraform init -upgrade
    
    # Select or create workspace
    if ! terraform workspace select "$TERRAFORM_WORKSPACE" 2>/dev/null; then
        log "Creating new Terraform workspace: $TERRAFORM_WORKSPACE"
        terraform workspace new "$TERRAFORM_WORKSPACE"
    fi
    
    success "Terraform initialized for workspace: $TERRAFORM_WORKSPACE"
}

# Plan infrastructure changes
plan_infrastructure() {
    log "Planning infrastructure changes..."
    
    cd "$TERRAFORM_DIR"
    
    # Create terraform.tfvars for this deployment
    cat > "terraform.tfvars" << EOF
environment = "$ENVIRONMENT"
aws_primary_region = "$DEPLOY_REGION"

# CDN Configuration
domain_name = "${DOMAIN_NAME:-bitcraps.io}"
cdn_subdomains = {
  assets = "assets"
  api    = "api"
  game   = "game"
  wasm   = "wasm"
}

# Security
enable_waf = true
enable_ddos_protection = true

# Performance
cache_ttl_settings = {
  static_assets = 86400
  wasm_files    = 604800
  api_responses = 300
  game_data     = 60
}

# Compression
compression_config = {
  enable_gzip   = true
  enable_brotli = true
  file_types = [
    "text/html",
    "text/css", 
    "text/javascript",
    "application/javascript",
    "application/json",
    "application/wasm",
    "image/svg+xml"
  ]
}

# Lambda@Edge
enable_api_lambda = true
enable_image_optimization = true
enable_ab_testing = false

# Monitoring
monitoring_config = {
  enable_real_time_metrics = true
  enable_enhanced_metrics  = true
  alarm_thresholds = {
    error_rate_threshold     = 5.0
    origin_latency_threshold = 3000
    cache_hit_rate_threshold = 85.0
  }
}
EOF

    # Add provider-specific variables
    if [[ "$DEPLOY_CLOUDFLARE" == "true" ]]; then
        cat >> "terraform.tfvars" << EOF

# Cloudflare
cloudflare_api_token = "$CLOUDFLARE_API_TOKEN"
cloudflare_zone_id = "$CLOUDFLARE_ZONE_ID"
EOF
    fi
    
    if [[ "$DEPLOY_FASTLY" == "true" ]]; then
        cat >> "terraform.tfvars" << EOF

# Fastly
fastly_api_key = "$FASTLY_API_KEY"
EOF
    fi
    
    # Plan changes
    terraform plan -out=tfplan
    
    success "Infrastructure plan created"
}

# Apply infrastructure changes
apply_infrastructure() {
    log "Applying infrastructure changes..."
    
    cd "$TERRAFORM_DIR"
    
    # Apply the plan
    terraform apply tfplan
    
    # Save outputs
    terraform output -json > "$PROJECT_ROOT/terraform-outputs.json"
    
    success "Infrastructure deployed successfully"
}

# Deploy Cloudflare Workers
deploy_cloudflare() {
    if [[ "$DEPLOY_CLOUDFLARE" != "true" ]]; then
        log "Skipping Cloudflare deployment (disabled)"
        return
    fi
    
    log "Deploying Cloudflare Workers..."
    
    cd "$INFRA_DIR/cloudflare"
    
    # Install dependencies
    if [[ -f "package.json" ]]; then
        npm install
    fi
    
    # Deploy to appropriate environment
    case "$ENVIRONMENT" in
        "prod")
            wrangler deploy --env production
            ;;
        "staging")
            wrangler deploy --env staging
            ;;
        *)
            wrangler deploy --env development
            ;;
    esac
    
    success "Cloudflare Workers deployed"
}

# Configure Fastly service
deploy_fastly() {
    if [[ "$DEPLOY_FASTLY" != "true" ]]; then
        log "Skipping Fastly deployment (disabled)"
        return
    fi
    
    log "Configuring Fastly service..."
    
    # Get Fastly service ID from Terraform outputs
    local service_id
    if [[ -f "$PROJECT_ROOT/terraform-outputs.json" ]]; then
        service_id=$(jq -r '.fastly_service_id.value // empty' "$PROJECT_ROOT/terraform-outputs.json")
    fi
    
    if [[ -z "$service_id" ]]; then
        warn "Fastly service ID not found, skipping VCL deployment"
        return
    fi
    
    log "Deploying VCL to Fastly service: $service_id"
    
    # Upload VCL configuration (this would require Fastly CLI or API calls)
    # For now, just log the configuration location
    log "VCL configuration available at: $INFRA_DIR/fastly/vcl/main.vcl"
    warn "Manual VCL deployment required - please upload via Fastly console"
    
    success "Fastly configuration prepared"
}

# Upload assets to CDN origins
upload_assets() {
    log "Uploading assets to CDN origins..."
    
    # Build assets first
    log "Building optimized assets..."
    "$SCRIPT_DIR/build-assets.sh"
    
    # Get S3 bucket name from Terraform outputs
    local s3_bucket
    if [[ -f "$PROJECT_ROOT/terraform-outputs.json" ]]; then
        s3_bucket=$(jq -r '.cdn_origin_bucket_name.value // empty' "$PROJECT_ROOT/terraform-outputs.json")
    fi
    
    if [[ -z "$s3_bucket" ]]; then
        error "CDN origin bucket name not found in Terraform outputs"
        return 1
    fi
    
    log "Uploading to S3 bucket: $s3_bucket"
    
    # Upload with appropriate cache headers
    aws s3 sync "$PROJECT_ROOT/dist/cdn/" "s3://$s3_bucket/" \
        --delete \
        --metadata-directive REPLACE \
        --cache-control "public, max-age=31536000" \
        --exclude "*.html" \
        --exclude "*.json"
    
    # Upload HTML and JSON files with shorter cache times
    aws s3 sync "$PROJECT_ROOT/dist/cdn/" "s3://$s3_bucket/" \
        --metadata-directive REPLACE \
        --cache-control "public, max-age=3600" \
        --include "*.html" \
        --include "*.json"
    
    # Set specific headers for WASM files
    aws s3 sync "$PROJECT_ROOT/dist/cdn/wasm/" "s3://$s3_bucket/wasm/" \
        --metadata-directive REPLACE \
        --content-type "application/wasm" \
        --cache-control "public, max-age=604800, immutable" \
        --metadata "cross-origin-embedder-policy=require-corp,cross-origin-opener-policy=same-origin"
    
    success "Assets uploaded to CDN origins"
}

# Invalidate CDN caches
invalidate_caches() {
    log "Invalidating CDN caches..."
    
    # CloudFront invalidation
    if [[ "$DEPLOY_CLOUDFRONT" == "true" ]]; then
        local distribution_id
        if [[ -f "$PROJECT_ROOT/terraform-outputs.json" ]]; then
            distribution_id=$(jq -r '.cloudfront_distribution_id.value // empty' "$PROJECT_ROOT/terraform-outputs.json")
        fi
        
        if [[ -n "$distribution_id" ]]; then
            log "Creating CloudFront invalidation for distribution: $distribution_id"
            aws cloudfront create-invalidation \
                --distribution-id "$distribution_id" \
                --paths "/*" \
                --query 'Invalidation.Id' \
                --output text
            success "CloudFront cache invalidated"
        fi
    fi
    
    # Cloudflare cache purge
    if [[ "$DEPLOY_CLOUDFLARE" == "true" ]]; then
        log "Purging Cloudflare cache..."
        curl -X POST "https://api.cloudflare.com/client/v4/zones/$CLOUDFLARE_ZONE_ID/purge_cache" \
             -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
             -H "Content-Type: application/json" \
             --data '{"purge_everything":true}' \
             --silent --show-error
        success "Cloudflare cache purged"
    fi
    
    # Note: Fastly cache invalidation would require service ID and API calls
    if [[ "$DEPLOY_FASTLY" == "true" ]]; then
        warn "Fastly cache invalidation requires manual action via Fastly console"
    fi
}

# Verify deployment
verify_deployment() {
    log "Verifying CDN deployment..."
    
    local domain="${DOMAIN_NAME:-bitcraps.io}"
    local endpoints=(
        "https://$domain"
        "https://assets.$domain"
        "https://api.$domain/health"
        "https://wasm.$domain"
    )
    
    for endpoint in "${endpoints[@]}"; do
        log "Testing endpoint: $endpoint"
        
        if curl --fail --silent --head --max-time 30 "$endpoint" >/dev/null; then
            success "✓ $endpoint is responding"
        else
            warn "✗ $endpoint is not responding or returned an error"
        fi
    done
    
    # Test WASM loading
    log "Testing WASM endpoint with proper headers..."
    local wasm_response
    wasm_response=$(curl --fail --silent --head --max-time 30 \
        -H "Origin: https://$domain" \
        "https://wasm.$domain/bitcraps.wasm" 2>&1) || true
    
    if echo "$wasm_response" | grep -q "content-type.*application/wasm"; then
        success "✓ WASM endpoint configured correctly"
    else
        warn "✗ WASM endpoint may not be configured correctly"
    fi
    
    success "Deployment verification completed"
}

# Generate deployment report
generate_deployment_report() {
    log "Generating deployment report..."
    
    local report_file="$PROJECT_ROOT/deployment-report.json"
    local deploy_time=$(date -Iseconds)
    local git_commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    
    cat > "$report_file" << EOF
{
  "deployTime": "$deploy_time",
  "environment": "$ENVIRONMENT",
  "region": "$DEPLOY_REGION",
  "gitCommit": "$git_commit",
  "providers": {
    "cloudfront": $DEPLOY_CLOUDFRONT,
    "cloudflare": $DEPLOY_CLOUDFLARE,
    "fastly": $DEPLOY_FASTLY
  },
  "domains": {
    "primary": "${DOMAIN_NAME:-bitcraps.io}",
    "assets": "assets.${DOMAIN_NAME:-bitcraps.io}",
    "api": "api.${DOMAIN_NAME:-bitcraps.io}",
    "wasm": "wasm.${DOMAIN_NAME:-bitcraps.io}"
  }
}
EOF
    
    # Include Terraform outputs if available
    if [[ -f "$PROJECT_ROOT/terraform-outputs.json" ]]; then
        jq -s '.[0] + {terraform: .[1]}' "$report_file" "$PROJECT_ROOT/terraform-outputs.json" > "${report_file}.tmp"
        mv "${report_file}.tmp" "$report_file"
    fi
    
    success "Deployment report generated: $report_file"
}

# Cleanup function
cleanup() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        error "Deployment failed with exit code $exit_code"
    fi
    
    # Clean up temporary files
    rm -f "$TERRAFORM_DIR/terraform.tfvars"
    rm -f "$TERRAFORM_DIR/tfplan"
}

trap cleanup EXIT

# Main execution
main() {
    log "Starting CDN deployment for environment: $ENVIRONMENT"
    
    check_prerequisites
    init_terraform
    plan_infrastructure
    apply_infrastructure
    deploy_cloudflare
    deploy_fastly
    upload_assets
    invalidate_caches
    verify_deployment
    generate_deployment_report
    
    success "CDN deployment completed successfully!"
    log "Deployment report: $PROJECT_ROOT/deployment-report.json"
}

# Show usage if requested
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    cat << 'EOF'
CDN Deployment Script for BitCraps

Usage: ./deploy-cdn.sh [options]

Environment Variables:
  ENVIRONMENT          Deployment environment (dev, staging, prod)
  DOMAIN_NAME         Primary domain name (default: bitcraps.io)
  DEPLOY_REGION       AWS region for deployment (default: us-west-2)
  DEPLOY_CLOUDFLARE   Deploy to Cloudflare (default: true)
  DEPLOY_FASTLY       Deploy to Fastly (default: true)  
  DEPLOY_CLOUDFRONT   Deploy to CloudFront (default: true)
  CLOUDFLARE_API_TOKEN Cloudflare API token (required if deploying to Cloudflare)
  CLOUDFLARE_ZONE_ID  Cloudflare Zone ID (required if deploying to Cloudflare)
  FASTLY_API_KEY      Fastly API key (required if deploying to Fastly)

Examples:
  # Deploy to production
  ENVIRONMENT=prod ./deploy-cdn.sh
  
  # Deploy to staging with custom domain
  ENVIRONMENT=staging DOMAIN_NAME=staging.bitcraps.io ./deploy-cdn.sh
  
  # Deploy only to CloudFront
  DEPLOY_CLOUDFLARE=false DEPLOY_FASTLY=false ./deploy-cdn.sh

EOF
    exit 0
fi

# Run main function
main "$@"