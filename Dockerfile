# Multi-stage Dockerfile for BitCraps production deployment

# Stage 1: Builder
FROM rust:1.75-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libbluetooth-dev \
    libdbus-1-dev \
    protobuf-compiler \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY benches ./benches
COPY build.rs ./

# Build release binary with optimizations
RUN cargo build --release --features "uniffi" \
    && strip target/release/bitcraps

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libbluetooth3 \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 -s /bin/bash bitcraps

# Copy binary from builder
COPY --from=builder /app/target/release/bitcraps /usr/local/bin/bitcraps

# Copy configuration files
COPY config /etc/bitcraps

# Create data directories
RUN mkdir -p /var/lib/bitcraps /var/log/bitcraps \
    && chown -R bitcraps:bitcraps /var/lib/bitcraps /var/log/bitcraps /etc/bitcraps

# Switch to non-root user
USER bitcraps

# Expose ports
EXPOSE 8080 8333 8334 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/bitcraps", "health"]

# Set environment variables
ENV RUST_LOG=info \
    BITCRAPS_DATA_DIR=/var/lib/bitcraps \
    BITCRAPS_CONFIG=/etc/bitcraps/production.toml

# Entry point
ENTRYPOINT ["/usr/local/bin/bitcraps"]

# Default command
CMD ["--config", "/etc/bitcraps/production.toml", "start"]