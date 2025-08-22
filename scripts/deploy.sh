#!/bin/bash

set -euo pipefail

REGISTRY="${REGISTRY:-localhost:5000}"
TAG="${TAG:-latest}"
IMAGE_NAME="bitchat"

echo "Building BitChat ${TAG}..."

# Build and test
cargo test --release
cargo build --release

# Security scan
cargo audit
cargo clippy -- -D warnings

# Build Docker image
docker build -t "${REGISTRY}/${IMAGE_NAME}:${TAG}" .

# Push to registry
docker push "${REGISTRY}/${IMAGE_NAME}:${TAG}"

echo "Deployment completed: ${REGISTRY}/${IMAGE_NAME}:${TAG}"