FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests

# Build optimized binary
RUN cargo build --release --bin bitchat

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/bitchat /usr/local/bin/

# Create non-root user
RUN useradd -r -s /bin/false bitchat
USER bitchat

EXPOSE 8080
VOLUME ["/data"]

CMD ["bitchat", "chat"]