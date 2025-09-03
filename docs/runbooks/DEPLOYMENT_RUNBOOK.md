# BitCraps Deployment Runbook

## Pre-Deployment Checklist

### Code Quality Gates
- [ ] Zero compilation errors: `cargo build --release`
- [ ] Zero compilation warnings: `cargo check`
- [ ] Security audit passed: `cargo audit`
- [ ] All unit tests passing: `cargo test --lib`
- [ ] Integration tests validated: `cargo test --test smoke_test`

### Environment Requirements
- **Rust Version**: 1.75+ (stable)
- **System Requirements**:
  - CPU: 4+ cores recommended
  - RAM: 8GB minimum, 16GB recommended
  - Disk: 10GB for binaries and data
  - Network: 1Gbps for optimal performance

## Deployment Steps

### 1. Build Release Binary
```bash
# Clean build
cargo clean

# Build optimized release
cargo build --release --features "tls nat-traversal"

# Verify binary
./target/release/bitcraps --version
```

### 2. Configuration Setup
```bash
# Create data directory
mkdir -p /var/lib/bitcraps

# Copy configuration template
cp config/production.toml /etc/bitcraps/config.toml

# Set environment variables
export RUST_LOG=info
export BITCRAPS_DATA_DIR=/var/lib/bitcraps
```

### 3. Database Initialization
```bash
# Run migrations
./target/release/bitcraps migrate

# Verify database
./target/release/bitcraps db-check
```

### 4. Service Installation

#### SystemD (Linux)
```bash
# Copy service file
sudo cp deploy/bitcraps.service /etc/systemd/system/

# Enable service
sudo systemctl enable bitcraps

# Start service
sudo systemctl start bitcraps

# Check status
sudo systemctl status bitcraps
```

### 5. Monitoring Setup
```bash
# Prometheus metrics available at
http://localhost:9090/metrics

# Health check endpoint
http://localhost:8080/health

# Dashboard
http://localhost:8080/dashboard
```

## Post-Deployment Verification

### Health Checks
```bash
# API health
curl http://localhost:8080/health

# Metrics endpoint
curl http://localhost:9090/metrics

# Peer discovery
./target/release/bitcraps ping
```

### Performance Verification
- Connection pool utilization: < 80%
- Memory usage: < 4GB
- CPU usage: < 70%
- Network latency: < 100ms

## Rollback Procedure

### Immediate Rollback
```bash
# Stop service
sudo systemctl stop bitcraps

# Restore previous binary
cp /var/lib/bitcraps/backup/bitcraps.prev /usr/local/bin/bitcraps

# Restore database
./bitcraps db-restore /var/lib/bitcraps/backup/latest.db

# Start service
sudo systemctl start bitcraps
```

## Troubleshooting

### Common Issues

#### Service Won't Start
1. Check logs: `journalctl -u bitcraps -f`
2. Verify permissions: `ls -la /var/lib/bitcraps`
3. Check port availability: `netstat -tulpn | grep 8080`

#### High Memory Usage
1. Check connection pool: Reduce `max_connections`
2. Clear cache: `./bitcraps cache-clear`
3. Review memory pools configuration

#### Network Issues
1. Verify firewall rules: ports 8080, 9090
2. Check NAT traversal: `./bitcraps net-test`
3. Review transport configuration

## Monitoring Alerts

### Critical Alerts
- Service down > 1 minute
- Memory usage > 90%
- Disk usage > 85%
- Error rate > 1%

### Warning Alerts
- Connection pool > 70%
- Network latency > 200ms
- Cache hit rate < 60%

## Security Considerations

### Production Hardening
- Enable TLS for all connections
- Use hardware-backed key storage
- Implement rate limiting
- Enable audit logging
- Regular security updates: `cargo audit`

### Access Control
- Limit admin API access
- Use strong authentication
- Implement IP whitelisting
- Regular key rotation

## Performance Tuning

### Connection Pools
```toml
[connection_pool]
min_connections = 10
max_connections = 100
idle_timeout_seconds = 300
```

### Cache Configuration
```toml
[cache]
l1_size_mb = 512
l2_size_mb = 2048
ttl_seconds = 3600
```

### SIMD Optimization
```toml
[crypto]
enable_simd = true
batch_size = 64
```

## Contact Information

### Escalation Path
1. On-call Engineer: Check PagerDuty
2. Team Lead: Slack #bitcraps-alerts
3. Security Team: security@bitcraps.io

### Documentation
- Architecture: docs/ARCHITECTURE.md
- API Reference: docs/API.md
- Security: docs/SECURITY.md

---
*Last Updated: January 2025*
*Version: 1.0.0*