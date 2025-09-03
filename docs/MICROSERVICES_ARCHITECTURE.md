# BitCraps Microservices Architecture

This document describes the microservices architecture implemented for the BitCraps decentralized casino system.

## Overview

The BitCraps system has been architected using microservices patterns to provide:
- **Scalability**: Services can be scaled independently based on demand
- **Reliability**: Service failures are isolated and don't affect the entire system
- **Maintainability**: Services can be developed, deployed, and maintained independently
- **Technology Diversity**: Each service can use the most appropriate technology stack

## Service Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Client Apps   │────▶│   API Gateway   │────▶│ Service Discovery│
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                 │
                    ┌────────────┼────────────┐
                    │            │            │
              ┌─────▼─────┐ ┌───▼───┐ ┌──────▼──────┐
              │Game Engine│ │Consensus│ │   Database  │
              │  Service  │ │ Service │ │   Service   │
              └───────────┘ └─────────┘ └─────────────┘
```

## Core Services

### 1. API Gateway (`src/services/api_gateway/`)
**Port**: 8080
**Purpose**: Central entry point for all client requests

**Features**:
- Request routing and load balancing
- Authentication and authorization
- Rate limiting and throttling
- Circuit breaker pattern
- Request/response transformation
- Metrics collection

**Endpoints**:
- `GET /health` - Gateway health status
- `GET /metrics` - Performance metrics
- `POST /api/v1/games` - Create new game (→ Game Engine)
- `GET /api/v1/games` - List games (→ Game Engine)
- `POST /api/v1/consensus/propose` - Submit proposal (→ Consensus)
- `POST /api/v1/consensus/vote` - Vote on proposal (→ Consensus)

### 2. Game Engine Service (`src/services/game_engine/`)
**Port**: 8081
**Purpose**: Manages game logic, state, and player interactions

**Features**:
- Game session management
- Player action processing
- Game state validation
- Real-time game updates
- Multiple game type support (Craps, Blackjack, Poker)
- Performance optimizations with memory pooling

**API Endpoints**:
- `POST /games` - Create new game session
- `GET /games/{id}` - Get game state
- `POST /games/{id}/actions` - Process player action
- `GET /games` - List active games
- `GET /health` - Service health check

### 3. Consensus Service (`src/services/consensus/`)
**Port**: 8082
**Purpose**: Distributed consensus for game fairness and network agreement

**Features**:
- Byzantine Fault Tolerance (BFT)
- Multiple consensus algorithms (PBFT, Tendermint, HotStuff)
- Validator management
- Byzantine behavior detection
- Network partition recovery
- Cryptographic proof validation

**API Endpoints**:
- `POST /propose` - Submit consensus proposal
- `POST /vote` - Vote on active proposal
- `GET /status` - Current consensus state
- `POST /validators` - Update validator set
- `GET /health` - Service health check

## Service Communication

### Service Discovery
The system supports multiple service discovery mechanisms:
- **Static Configuration**: Hardcoded service endpoints
- **Consul**: Dynamic service registration and discovery
- **Kubernetes**: Native Kubernetes service discovery

### Load Balancing
Multiple load balancing strategies are available:
- **Round Robin**: Distributes requests evenly
- **Weighted Round Robin**: Based on service capacity
- **Least Connections**: Routes to least busy instance
- **IP Hash**: Consistent routing for same client
- **Random**: Random selection

### Circuit Breaker Pattern
Prevents cascading failures by:
- Monitoring service health and response times
- Opening circuit when failure threshold is exceeded
- Allowing gradual recovery with half-open state
- Failing fast during outages

## Configuration

### Environment Variables
```bash
# API Gateway
GATEWAY_LISTEN_ADDR=0.0.0.0:8080
GATEWAY_REQUEST_TIMEOUT=30s
RATE_LIMIT_MAX_REQUESTS=1000
RATE_LIMIT_WINDOW=60s

# Service Discovery
SERVICE_DISCOVERY_METHOD=static  # static, consul, kubernetes
CONSUL_ADDRESS=127.0.0.1:8500
HEALTH_CHECK_INTERVAL=30s

# Game Engine
GAME_ENGINE_ADDR=127.0.0.1:8081
MAX_CONCURRENT_GAMES=100
MAX_PLAYERS_PER_GAME=8

# Consensus
CONSENSUS_ADDR=127.0.0.1:8082
BYZANTINE_THRESHOLD=1
ROUND_TIMEOUT=15s
CONSENSUS_ALGORITHM=pbft  # pbft, tendermint, hotstuff
```

### Service Configuration Files
Services can be configured via TOML files:

```toml
[gateway]
listen_addr = "0.0.0.0:8080"
request_timeout = "30s"

[gateway.rate_limit]
enabled = true
max_requests = 1000
window = "60s"
by_ip = true
by_api_key = true

[gateway.auth]
enabled = true
jwt_secret = "your-secret-key"
token_expiration = "24h"

[service_discovery]
method = "static"
health_check_interval = "30s"

[service_discovery.static_services]
game-engine = [
    { address = "127.0.0.1:8081", weight = 100, health_check_path = "/health" }
]
consensus = [
    { address = "127.0.0.1:8082", weight = 100, health_check_path = "/health" }
]
```

## Running the Microservices

### Development Mode
```bash
# Run the demo with all services
cargo run --example microservices_demo

# Run individual services
cargo run --bin game-engine-service
cargo run --bin consensus-service
cargo run --bin api-gateway
```

### Production Deployment
```bash
# Build optimized binaries
cargo build --release

# Deploy with Docker
docker-compose up -d

# Or with Kubernetes
kubectl apply -f k8s/
```

### Docker Compose Example
```yaml
version: '3.8'
services:
  consul:
    image: consul:1.15
    ports:
      - "8500:8500"
    command: agent -server -ui -node=server-1 -bootstrap-expect=1 -client=0.0.0.0

  game-engine:
    build: .
    command: game-engine-service
    environment:
      - SERVICE_DISCOVERY_METHOD=consul
      - CONSUL_ADDRESS=consul:8500
    depends_on:
      - consul

  consensus:
    build: .
    command: consensus-service
    environment:
      - SERVICE_DISCOVERY_METHOD=consul
      - CONSUL_ADDRESS=consul:8500
    depends_on:
      - consul

  api-gateway:
    build: .
    command: api-gateway
    ports:
      - "8080:8080"
    environment:
      - SERVICE_DISCOVERY_METHOD=consul
      - CONSUL_ADDRESS=consul:8500
    depends_on:
      - game-engine
      - consensus
```

## Monitoring and Observability

### Health Checks
Each service provides health check endpoints:
- **Liveness**: Service is running
- **Readiness**: Service can handle requests
- **Dependencies**: External dependencies status

### Metrics Collection
Services expose Prometheus-compatible metrics:
- Request rates and response times
- Error rates and success rates
- Resource utilization
- Business metrics (games created, consensus rounds)

### Distributed Tracing
Requests are traced across service boundaries:
- Correlation IDs for request tracking
- Service dependency mapping
- Performance bottleneck identification
- Error propagation analysis

### Logging
Structured logging with correlation:
- JSON format for machine parsing
- Log levels (DEBUG, INFO, WARN, ERROR)
- Request correlation IDs
- Service context information

## Security Considerations

### Authentication & Authorization
- JWT-based authentication
- API key management
- Role-based access control
- Rate limiting per user/API key

### Network Security
- TLS encryption for service communication
- Service mesh integration
- Network policies and firewalls
- Secret management

### Input Validation
- Request payload validation
- SQL injection prevention
- XSS protection
- CSRF protection

## Testing Strategy

### Unit Tests
Each service has comprehensive unit tests:
```bash
cargo test --lib services::game_engine
cargo test --lib services::consensus
cargo test --lib services::api_gateway
```

### Integration Tests
Cross-service integration testing:
```bash
cargo test --test integration_microservices
```

### Load Testing
Performance testing with realistic workloads:
```bash
# Install k6
brew install k6

# Run load tests
k6 run tests/load/gateway_load_test.js
```

### Chaos Engineering
Fault injection and resilience testing:
```bash
cargo test --test chaos_microservices
```

## Migration from Monolith

The microservices architecture maintains backward compatibility:
1. **Gradual Migration**: Services can be migrated incrementally
2. **Adapter Pattern**: Legacy code can call microservices via adapters  
3. **Shared Libraries**: Common functionality remains in shared crates
4. **Database Migration**: Data can be gradually partitioned by service

## Performance Characteristics

### Benchmarks
- API Gateway: >10,000 requests/second
- Game Engine: >1,000 concurrent games
- Consensus: Sub-second finality for most operations

### Optimization Strategies
- Connection pooling and keep-alive
- Request batching and pipelining
- Caching at multiple layers
- Asynchronous processing
- Memory pool optimization

## Troubleshooting

### Common Issues
1. **Service Discovery Failures**
   - Check network connectivity
   - Verify service registration
   - Review health check configurations

2. **Circuit Breaker Tripping**
   - Monitor service health metrics
   - Adjust failure thresholds
   - Check resource availability

3. **Performance Issues**
   - Review service metrics
   - Check database connections
   - Monitor resource utilization
   - Analyze distributed traces

### Debugging Tools
- Service mesh dashboard
- Distributed tracing UI
- Metrics visualization (Grafana)
- Log aggregation (ELK stack)

## Future Roadmap

### Phase 1 (Current)
- ✅ Basic microservices architecture
- ✅ API Gateway with routing
- ✅ Service discovery
- ✅ Health checks and circuit breakers

### Phase 2 (Next)
- [ ] gRPC service communication
- [ ] Service mesh (Istio/Linkerd)
- [ ] Advanced observability
- [ ] Auto-scaling

### Phase 3 (Future)
- [ ] Event-driven architecture
- [ ] CQRS and Event Sourcing
- [ ] Multi-region deployment
- [ ] Serverless functions

## Conclusion

The BitCraps microservices architecture provides a robust, scalable foundation for the decentralized casino system. The modular design enables independent development and deployment while maintaining strong consistency guarantees through the consensus service.

For questions or contributions, see [CONTRIBUTING.md](CONTRIBUTING.md).