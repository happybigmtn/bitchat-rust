# BitCraps Build Matrix Documentation

## M0 Baseline CI Gates

This document describes the comprehensive build matrix and CI configuration for BitCraps, implementing M0 baseline hardening and M8 performance milestones.

## Build Matrix Overview

### Core Rust Checks
| Platform | Rust Version | Features | M0 Gates | M8 Performance |
|----------|-------------|-----------|----------|----------------|
| Ubuntu Latest | stable, beta, nightly | all combinations | ✓ | ✓ |
| macOS Latest | stable, beta | all combinations | ✓ | ✓ |
| Windows Latest | stable, beta | all combinations | ✓ | ✓ |

### Mobile Platforms
| Platform | NDK/SDK | Architecture | Build Type |
|----------|---------|-------------|------------|
| Android | NDK 25.2.9519653 | arm64-v8a, armeabi-v7a, x86_64, x86 | AAR + APK |
| iOS | Xcode Latest | aarch64, x86_64, aarch64-sim | Framework |

## M0 CI Gates Implementation

### 1. Code Formatting (M0)
```bash
cargo fmt --all -- --check
```
- **Gate**: Must pass without any formatting violations
- **Timeout**: 30 seconds
- **Failure Action**: Immediate CI failure

### 2. Clippy with Performance Rules (M0+M8)
```bash
cargo clippy --workspace --all-targets --features="" --locked -- -D warnings \
  -W clippy::manual_memcpy \
  -W clippy::needless_collect \
  -W clippy::redundant_clone \
  -W clippy::inefficient_to_string \
  -W clippy::large_stack_arrays \
  -W clippy::vec_box \
  -W clippy::mutex_atomic \
  -W clippy::mem_forget
```
- **Gate**: Zero warnings allowed
- **Performance Lints**: Enforced for M8 compliance
- **Feature Testing**: Separate checks for `uniffi`, `android`, `tls`

### 3. Fast Test Suite (M0)
```bash
timeout 480s cargo test --workspace --lib --bins --locked \
  --tests --exclude-integration-tests \
  -- --test-threads=$(nproc) --nocapture
```
- **Budget**: 8 minutes (480 seconds) maximum
- **Parallel**: Uses all CPU cores
- **Gate**: Must complete within time budget
- **Failure**: Any test failure or timeout violation

### 4. Documentation Build (M0)
```bash
cargo doc --workspace --no-deps --locked --document-private-items \
  --features=""
```
- **Gate**: Must build without errors
- **Warnings**: Logged but not failing
- **Private Items**: Included for internal documentation

## M8 Performance Validation

### 1. Loop Budget Adherence
```bash
cargo test --workspace --locked loop_budget -- --nocapture \
  --test-threads=1
```
- **Validation**: Ensures no unbounded loops
- **Resource Control**: Monitors iteration budgets
- **Gate**: Budget controls must be functional

### 2. Lock Ordering Validation
```bash
RUST_BACKTRACE=1 cargo test --workspace --locked lock_ordering \
  -- --nocapture --test-threads=1
```
- **Deadlock Prevention**: Validates lock acquisition order
- **Debug Mode**: Enables assertion panics for ordering violations
- **Gate**: No lock ordering violations allowed

### 3. Memory Pool Health
```bash
/usr/bin/time -v cargo test --workspace --locked \
  --features="" -- memory pool
```
- **Resource Monitoring**: Tracks peak memory usage
- **Limit**: 2GB (2,097,152 KB) maximum
- **Gate**: Memory usage within operational limits

### 4. Benchmark Compilation
```bash
cargo bench --no-run --features benchmarks --locked
```
- **Compilation**: All benchmarks must compile
- **Performance Smoke Test**: 30-second quick run
- **Gate**: Benchmark infrastructure functional

## Feature Matrix

### Core Features
| Feature | Description | CI Testing | Mobile Support |
|---------|-------------|------------|---------------|
| `default` | Base functionality | ✓ Full | ✓ |
| `benchmarks` | Performance testing | ✓ Compilation | ✗ |
| `physical_device_tests` | Hardware testing | ✗ Manual | ✓ |

### Platform Features  
| Feature | Description | Platforms | CI Testing |
|---------|-------------|-----------|------------|
| `uniffi` | Cross-platform bindings | iOS, Android | ✓ Optional |
| `android` | Android JNI support | Android | ✓ Optional |
| `tls` | TLS transport support | All | ✓ Optional |

### Feature Combinations Tested
1. **Minimal**: No features (core functionality)
2. **Mobile iOS**: `uniffi` 
3. **Mobile Android**: `android` + `uniffi`
4. **Secure**: `tls`
5. **Performance**: `benchmarks`
6. **Full**: All compatible features

## Resource Budgets

### Time Budgets
- **Fast Tests**: 8 minutes maximum
- **Full CI Pipeline**: 45 minutes maximum  
- **Performance Validation**: 15 minutes maximum
- **Mobile Builds**: 30 minutes per platform

### Memory Budgets  
- **Test Execution**: 2GB maximum
- **Build Process**: 4GB maximum
- **Mobile Builds**: 6GB maximum (Android NDK)

### CPU Budgets
- **Test Parallelism**: All available cores
- **Build Jobs**: min(8, cores) for stability
- **Benchmark Runs**: Single-threaded for consistency

## Performance Targets (M8)

### Latency Targets
- **p95 Network Latency**: < 100ms steady-state
- **Consensus Finalization**: < 500ms
- **Memory Pool Operations**: < 10ms

### Throughput Targets  
- **Consensus Operations**: > 100 ops/sec
- **Network Messages**: > 1000 msg/sec
- **Database Transactions**: > 500 tx/sec

### Resource Efficiency
- **CPU Utilization**: < 70% steady-state
- **Memory Growth**: < 50MB/hour
- **Network Bandwidth**: < 80% link utilization

## Soak Test Requirements (M8)

### 8-Hour Continuous Testing
- **Duration**: 8 hours minimum
- **Sample Rate**: Every 30 seconds
- **Memory Leak Detection**: < 50MB/hour growth
- **Downtime Tolerance**: < 5 minutes total

### Monitoring Metrics
- System resource usage
- Performance degradation detection  
- Error rate monitoring
- Adaptive interval effectiveness

## Failure Thresholds

### Immediate Failures (Hard Gates)
- Compilation errors
- Test failures  
- Time budget exceeded
- Memory limit exceeded
- Security lint violations

### Warning Conditions (Soft Gates)
- Documentation warnings
- Performance degradation
- High resource usage
- Extended build times

## Emergency Overrides

### Manual Intervention Points
- Performance target adjustment
- Time budget extension
- Feature gate bypass (with approval)
- Emergency rollback procedures

### Override Commands
```bash
# Emergency interval override
cargo test -- --test-timeout=600  # 10 minute override

# Memory limit bypass (requires justification)
ulimit -v 4194304  # 4GB limit

# Skip performance gates (requires approval)  
BITCRAPS_SKIP_PERF_GATES=true cargo test
```

## Monitoring and Alerting

### CI Health Metrics
- Build success rate (target: >95%)
- Average build time (target: <30min)
- Resource usage trends
- Feature gate effectiveness

### Performance Regression Detection
- Benchmark trend analysis
- Memory usage growth tracking
- Latency degradation alerts
- Throughput drop notifications

## Implementation Status

| Component | Status | M0 Ready | M8 Ready |
|-----------|--------|----------|----------|
| Format Gate | ✅ Complete | ✅ | ✅ |
| Clippy + Performance | ✅ Complete | ✅ | ✅ |
| Fast Test Suite | ✅ Complete | ✅ | ✅ |
| Documentation Build | ✅ Complete | ✅ | ✅ |
| Loop Budget Validation | ✅ Complete | ✅ | ✅ |
| Lock Order Validation | ✅ Complete | ✅ | ✅ |
| Memory Pool Health | ✅ Complete | ✅ | ✅ |
| Benchmark Compilation | ✅ Complete | ✅ | ✅ |
| Adaptive Intervals | ✅ Complete | ✅ | ✅ |
| Soak Test Framework | ✅ Complete | ✅ | ✅ |

## Usage Examples

### Running M0 Gates Locally
```bash
# Format check
cargo fmt --all -- --check

# Performance clippy
cargo clippy --all-targets --locked -- -D warnings

# Fast tests with budget
timeout 480s cargo test --lib --bins
```

### M8 Performance Validation
```bash
# Loop budget test
cargo test loop_budget

# Lock ordering test  
cargo test lock_ordering

# Memory health check
cargo test memory pool

# Benchmark compilation
cargo bench --no-run --features benchmarks
```

### Soak Test Execution
```bash
# 8-hour production soak test
cargo run --bin soak-test -- --duration 28800 --config production

# Quick soak test (1 hour)
cargo run --bin soak-test -- --duration 3600 --config development  
```

This build matrix ensures BitCraps meets both M0 baseline hardening requirements and M8 performance milestones through comprehensive automated testing and validation.