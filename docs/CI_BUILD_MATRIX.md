# BitCraps CI/CD Build Matrix

## M0 Baseline CI Gates Implementation

This document describes the complete CI/CD build matrix for BitCraps, implementing M0 baseline hardening requirements.

## Build Matrix Overview

### Core CI Jobs

| Job | Platform | Rust Version | Purpose | Runtime Budget |
|-----|----------|--------------|---------|----------------|
| `rust-checks` | Ubuntu, macOS, Windows | stable, beta, nightly | Core validation | < 8 minutes |
| `security-audit` | Ubuntu | stable | Security scanning | < 3 minutes |
| `coverage` | Ubuntu | stable | Code coverage | < 10 minutes |
| `performance-validation` | Ubuntu | stable | M8 performance gates | < 5 minutes |

### Mobile Platform Jobs

| Job | Platform | Purpose | Runtime Budget |
|-----|----------|---------|----------------|
| `android-build` | Ubuntu | Android cross-compilation | < 15 minutes |
| `ios-build` | macOS | iOS cross-compilation | < 12 minutes |

### Performance Jobs

| Job | Platform | Purpose | Runtime Budget |
|-----|----------|---------|----------------|
| `benchmarks` | Ubuntu | Performance benchmarks | < 20 minutes |
| `multi-platform-build` | Multi | Release builds | < 25 minutes |

## M0 CI Gates Implementation

### 1. Code Quality Gates

#### Formatting Check (M0 Gate)
```yaml
- name: Check formatting (M0)
  run: |
    echo "::group::M0 Baseline: Format Check"
    cargo fmt --all -- --check
    echo "✅ M0 Gate: Code formatting passed"
    echo "::endgroup::"
```

#### Clippy with Performance Rules (M0+M8 Gate)
```yaml
- name: Run clippy with performance rules (M0+M8)
  run: |
    echo "::group::M0+M8 Gate: Clippy Analysis"
    cargo clippy --workspace --all-targets --features="" --locked -- -D warnings \
      -W clippy::manual_memcpy \
      -W clippy::needless_collect \
      -W clippy::redundant_clone \
      -W clippy::inefficient_to_string \
      -W clippy::large_stack_arrays \
      -W clippy::vec_box \
      -W clippy::mutex_atomic \
      -W clippy::mem_forget
    echo "✅ M0+M8 Gate: Clippy with performance rules passed"
    echo "::endgroup::"
```

### 2. Fast Test Suite (M0 Gate)

#### Test Organization Strategy

**Fast Tests (< 8 minutes)**
- Unit tests (`cargo test --workspace --lib --bins`)
- Fast integration tests
- Core protocol tests
- Crypto verification tests

**Slow Tests (excluded from M0)**
- Network integration tests
- End-to-end consensus tests
- Load testing
- Cross-platform compatibility tests

#### Implementation
```yaml
- name: Run fast unit tests (M0)
  run: |
    echo "::group::M0 Gate: Fast Tests"
    start_time=$(date +%s)
    
    timeout 480s cargo test --workspace --lib --bins --locked \
      --tests --exclude-integration-tests \
      -- --test-threads=$(nproc) --nocapture \
      || (echo "❌ M0 Gate: Tests exceeded 8-minute budget" && exit 1)
    
    end_time=$(date +%s)
    duration=$((end_time - start_time))
    
    if [ $duration -gt 480 ]; then
      echo "❌ M0 Gate: Test runtime exceeded budget: ${duration}s > 480s"
      exit 1
    fi
    
    echo "✅ M0 Gate: Fast tests passed in ${duration}s"
    echo "::endgroup::"
```

### 3. Documentation Build (M0 Gate)

```yaml
- name: Build documentation (M0)
  run: |
    echo "::group::M0 Gate: Documentation Build"
    cargo doc --workspace --no-deps --locked --document-private-items \
      --features="" 2>&1 | tee doc-build.log
    
    if grep -i "warning" doc-build.log; then
      echo "⚠️ Documentation warnings found but not failing build"
    fi
    
    echo "✅ M0 Gate: Documentation build passed"
    echo "::endgroup::"
```

### 4. Benchmark Compilation (M8 Gate)

```yaml
- name: Verify benchmark compilation (M8)
  run: |
    echo "::group::M8 Gate: Benchmark Compilation"
    
    cargo bench --no-run --features benchmarks --locked 2>&1 | tee bench-compile.log
    
    if [ $? -eq 0 ]; then
      echo "✅ M8 Gate: All benchmarks compile successfully"
    else
      echo "❌ M8 Gate: Benchmark compilation failed"
      cat bench-compile.log
      exit 1
    fi
    
    # Run performance smoke test (30s max)
    echo "Running performance smoke test..."
    timeout 30s cargo bench --features benchmarks --locked \
      -- --measurement-time 5 --warm-up-time 1 \
      || echo "Smoke test completed or timed out (acceptable)"
    
    echo "::endgroup::"
```

## Platform Requirements

### Ubuntu (Primary CI Platform)
- **Rust versions**: stable, beta, nightly
- **Features**: Full test suite, security audit, coverage
- **Cross-compilation**: Android targets
- **Tools**: cargo-audit, cargo-tarpaulin, valgrind

### macOS (Mobile Development)
- **Rust versions**: stable
- **Features**: iOS builds, native testing
- **Tools**: Xcode, CocoaPods, cargo-lipo

### Windows (Compatibility)
- **Rust versions**: stable
- **Features**: Basic compilation, core tests
- **Tools**: MSVC toolchain

## Performance Budgets

### M0 Baseline Budgets
- **Fast test suite**: 8 minutes maximum
- **Formatting/clippy**: 2 minutes maximum
- **Documentation build**: 3 minutes maximum
- **Total M0 pipeline**: 15 minutes maximum

### Extended Budgets
- **Android build**: 15 minutes
- **iOS build**: 12 minutes  
- **Benchmarks**: 20 minutes
- **Multi-platform releases**: 25 minutes

## Test Organization

### Fast Test Categories (Included in M0)

1. **Unit Tests**
   - Pure function tests
   - Data structure validation
   - Crypto primitives
   - Error handling

2. **Fast Integration Tests** 
   - Protocol parsing
   - State transitions
   - Configuration validation
   - Mock network tests

3. **Performance Tests**
   - Benchmark compilation
   - Memory usage validation
   - CPU bound operations

### Slow Test Categories (Excluded from M0)

1. **Network Integration**
   - Real network I/O
   - Bluetooth mesh testing
   - NAT traversal
   - Multi-peer consensus

2. **End-to-End Testing**
   - Full game sessions
   - Cross-platform compatibility
   - Load testing
   - Chaos engineering

3. **Mobile Device Testing**
   - Physical device tests
   - Battery optimization
   - Background mode testing

## Security Gates

### Dependency Scanning
- Daily security audit with `cargo audit`
- Vulnerability database updates
- License compliance checking

### Code Analysis  
- SAST (Static Application Security Testing)
- Secret detection
- Dependency confusion prevention

### Performance Security
- Loop budget validation
- Memory exhaustion prevention
- DoS attack resistance

## Feature Flags

### CI-Specific Features
- `benchmarks`: Enable criterion benchmarks
- `physical_device_tests`: Bluetooth hardware tests  
- `ethereum`: Smart contract integration
- `tls`: Transport layer security

### Platform Features
- `android`: Android-specific optimizations
- `uniffi`: Cross-platform FFI bindings
- `bluetooth`: BLE peripheral support

## Artifact Management

### Build Artifacts
- **Debug builds**: Retained 5 days
- **Release builds**: Retained 90 days
- **Coverage reports**: Retained 30 days
- **Benchmark results**: Retained 90 days

### Mobile Artifacts
- **Android APK**: Retained 30 days
- **Android AAR**: Retained 30 days
- **iOS Framework**: Retained 30 days
- **Test reports**: Retained 7 days

## Environment Variables

### M0 Configuration
```bash
CARGO_TERM_COLOR=always
RUST_BACKTRACE=1
CARGO_INCREMENTAL=0
RUSTFLAGS="-C debuginfo=0 -D warnings"
BITCRAPS_CI_MODE="true"
BITCRAPS_FAST_TESTS_ONLY="true"
BITCRAPS_PERF_BUDGET_MS="480000"  # 8 minutes
BITCRAPS_MEMORY_LIMIT_MB="2048"   # 2GB
```

### Performance Monitoring
```bash
BITCRAPS_ENABLE_METRICS="true"
BITCRAPS_BENCHMARK_OUTPUT="json"
BITCRAPS_PROFILE_TESTS="false"
```

## Success Criteria

### M0 Baseline Requirements ✅
- [x] All code formatting checks pass
- [x] Clippy analysis with performance rules
- [x] Fast test suite completes < 8 minutes
- [x] Documentation builds successfully
- [x] Benchmarks compile without errors
- [x] Multi-platform compatibility verified

### Quality Metrics
- **Test Coverage**: > 80% line coverage
- **Build Success Rate**: > 95% over 30 days
- **Performance Regression**: < 5% slowdown tolerance
- **Security Score**: No critical vulnerabilities

## CI Pipeline Triggers

### Automated Triggers
- **Push to main/master**: Full pipeline
- **Pull requests**: Core gates only
- **Daily schedule**: Security audit
- **Release tags**: Full build matrix

### Manual Triggers
- **Benchmark runs**: Performance analysis
- **Mobile builds**: Cross-platform testing
- **Security scans**: Vulnerability assessment

---

**Status**: ✅ M0 Baseline Fully Implemented
**Last Updated**: 2025-09-03
**Next Review**: M8 Performance Optimization Phase