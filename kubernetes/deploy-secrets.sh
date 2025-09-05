#!/bin/bash
# Secure secrets deployment script for BitCraps
# This script helps deploy secrets safely without exposing them in version control

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check environment
ENVIRONMENT="${1:-}"
if [[ -z "$ENVIRONMENT" ]]; then
    echo -e "${RED}Error: Environment not specified${NC}"
    echo "Usage: $0 [production|staging]"
    exit 1
fi

# Validate environment
if [[ "$ENVIRONMENT" != "production" && "$ENVIRONMENT" != "staging" ]]; then
    echo -e "${RED}Error: Invalid environment. Must be 'production' or 'staging'${NC}"
    exit 1
fi

# Function to check if environment variable is set
check_env_var() {
    local var_name=$1
    if [[ -z "${!var_name:-}" ]]; then
        echo -e "${RED}Error: Required environment variable $var_name is not set${NC}"
        return 1
    fi
    return 0
}

echo -e "${YELLOW}Deploying secrets for $ENVIRONMENT environment...${NC}"

# Check required environment variables based on environment
if [[ "$ENVIRONMENT" == "production" ]]; then
    REQUIRED_VARS=(
        "POSTGRES_PASSWORD"
        "POSTGRES_USER"
        "TREASURY_ADDRESS"
        "MASTER_SIGNING_KEY"
        "ENCRYPTION_KEY"
    )
    NAMESPACE="bitcraps-production"
else
    REQUIRED_VARS=(
        "STAGING_POSTGRES_PASSWORD"
        "STAGING_POSTGRES_USER"
        "STAGING_TREASURY_ADDRESS"
        "STAGING_MASTER_SIGNING_KEY"
        "STAGING_ENCRYPTION_KEY"
    )
    NAMESPACE="bitcraps-staging"
fi

# Check all required variables
MISSING_VARS=0
for var in "${REQUIRED_VARS[@]}"; do
    if ! check_env_var "$var"; then
        MISSING_VARS=1
    fi
done

if [[ $MISSING_VARS -eq 1 ]]; then
    echo -e "${RED}Please set all required environment variables before running this script${NC}"
    echo -e "${YELLOW}You can source them from a secure secrets file:${NC}"
    echo "  source /path/to/secure/secrets.env"
    exit 1
fi

# Create namespace if it doesn't exist
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -

# Deploy database credentials
if [[ "$ENVIRONMENT" == "production" ]]; then
    kubectl create secret generic bitcraps-database-credentials \
        --namespace="$NAMESPACE" \
        --from-literal="POSTGRES_PASSWORD=$POSTGRES_PASSWORD" \
        --from-literal="POSTGRES_USER=$POSTGRES_USER" \
        --from-literal="DATABASE_URL=postgresql://$POSTGRES_USER:$POSTGRES_PASSWORD@postgres-service:5432/bitcraps" \
        --dry-run=client -o yaml | kubectl apply -f -
else
    kubectl create secret generic bitcraps-database-credentials \
        --namespace="$NAMESPACE" \
        --from-literal="POSTGRES_PASSWORD=$STAGING_POSTGRES_PASSWORD" \
        --from-literal="POSTGRES_USER=$STAGING_POSTGRES_USER" \
        --from-literal="DATABASE_URL=postgresql://$STAGING_POSTGRES_USER:$STAGING_POSTGRES_PASSWORD@postgres-service:5432/bitcraps_staging" \
        --dry-run=client -o yaml | kubectl apply -f -
fi

echo -e "${GREEN}✓ Database credentials deployed${NC}"

# Deploy crypto keys
if [[ "$ENVIRONMENT" == "production" ]]; then
    kubectl create secret generic bitcraps-crypto-keys \
        --namespace="$NAMESPACE" \
        --from-literal="TREASURY_ADDRESS=$TREASURY_ADDRESS" \
        --from-literal="MASTER_SIGNING_KEY=$MASTER_SIGNING_KEY" \
        --from-literal="ENCRYPTION_KEY=$ENCRYPTION_KEY" \
        --dry-run=client -o yaml | kubectl apply -f -
else
    kubectl create secret generic bitcraps-crypto-keys \
        --namespace="$NAMESPACE" \
        --from-literal="TREASURY_ADDRESS=$STAGING_TREASURY_ADDRESS" \
        --from-literal="MASTER_SIGNING_KEY=$STAGING_MASTER_SIGNING_KEY" \
        --from-literal="ENCRYPTION_KEY=$STAGING_ENCRYPTION_KEY" \
        --dry-run=client -o yaml | kubectl apply -f -
fi

echo -e "${GREEN}✓ Crypto keys deployed${NC}"

# Verify secrets were created
echo -e "${YELLOW}Verifying secrets...${NC}"
kubectl get secrets -n "$NAMESPACE" | grep bitcraps

echo -e "${GREEN}✓ Secrets successfully deployed to $ENVIRONMENT environment${NC}"
echo -e "${YELLOW}Note: Remember to rotate these secrets regularly${NC}"