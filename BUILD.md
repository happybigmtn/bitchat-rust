# BitCraps Build Configuration

## Production Builds

### Minimal Build (Default)
The default configuration provides only core consensus and networking functionality:

```bash
# Minimal build - only essential components
cargo build --release
```

**Features included:**
- Core consensus protocols
- Basic error handling
- Protocol definitions
- Token economics

**Size:** ~2MB optimized binary
**Dependencies:** Minimal set only

### Core Build (Recommended)
Production-ready build with essential features:

```bash
# Core build with BLE and database
cargo build --release --features core
```

**Features included:**
- Bluetooth LE mesh networking
- SQLite database storage
- Consensus protocols
- Basic UI support

**Size:** ~8MB optimized binary
**Dependencies:** btleplug, rusqlite, core networking

### Full Build (Development)
Complete feature set for development and testing:

```bash
# Full build with all features
cargo build --release --features full
```

**Features included:**
- All transport protocols (BLE, TCP, UDP)
- Multiple database backends (SQLite, PostgreSQL)
- Mobile platform bindings
- GPU acceleration
- WebAssembly support
- Complete UI suite

**Size:** ~25MB optimized binary
**Dependencies:** All optional features

## Feature Flags

### Core Features
- `consensus` - Byzantine consensus protocols
- `bluetooth` - BLE mesh networking via btleplug
- `sqlite` - SQLite database backend
- `ui` - Terminal user interface

### Transport Features
- `nat-traversal` - STUN/TURN NAT traversal
- `tls` - Transport Layer Security
- `webrtc` - WebRTC peer-to-peer

### Database Features
- `postgres` - PostgreSQL backend
- `database-migrations` - Schema migration support

### Mobile Features  
- `android` - Android JNI bindings
- `uniffi` - Cross-platform FFI generation

### Hardware Features
- `gpu` - GPU acceleration via WebGPU
- `hsm` - Hardware Security Module support
- `yubikey` - YubiKey integration

### Platform Features
- `wasm` - WebAssembly browser support
- `monitoring` - System monitoring and metrics

## CI/CD Builds

### Continuous Integration
```yaml
# Minimal CI build
- name: Build minimal
  run: cargo build --no-default-features

# Core feature testing
- name: Build core
  run: cargo build --features core

# Full feature testing  
- name: Build full
  run: cargo build --features full
```

### Production Deployment
```bash
# Production optimized build
RUSTFLAGS="-C target-cpu=native -C opt-level=3" \
cargo build --release --features core --target x86_64-unknown-linux-gnu

# Strip debug symbols
strip target/x86_64-unknown-linux-gnu/release/bitcraps
```

## Performance Optimization

### Release Profile
The `Cargo.toml` includes optimized release settings:
```toml
[profile.release]
lto = true              # Link-time optimization
codegen-units = 1       # Single codegen unit for better optimization  
strip = true            # Strip debug symbols
opt-level = 3           # Maximum optimization
```

### Platform-Specific Builds

#### Android
```bash
# Android ARM64
cargo build --release --target aarch64-linux-android --features "core android"

# Android x86_64 (emulator)
cargo build --release --target x86_64-linux-android --features "core android"
```

#### iOS
```bash
# iOS ARM64
cargo build --release --target aarch64-apple-ios --features "core uniffi"

# iOS Simulator
cargo build --release --target x86_64-apple-ios --features "core uniffi"
```

#### WebAssembly
```bash
# Browser WebAssembly
cargo build --release --target wasm32-unknown-unknown --features "wasm browser"

# WASI runtime
cargo build --release --target wasm32-wasi --features "wasm-runtime"
```

## Testing Builds

### Unit Tests
```bash
# Run tests with minimal features
cargo test --no-default-features

# Run tests with core features
cargo test --features core

# Run MVP-specific tests
cargo test --features mvp

# Run legacy comprehensive tests
cargo test --features legacy-tests
```

### Integration Tests
```bash  
# Multi-peer integration tests
cargo test multi_peer_integration --features "core legacy-tests"

# Security integration tests  
cargo test security --features "core legacy-tests"

# Performance tests
cargo test performance --features "core legacy-tests"
```

## Binary Targets

### Main Application
```bash
# BitCraps CLI
cargo build --bin bitcraps --features "core ui"
```

### Examples (Feature-Gated)
```bash
# Enable legacy examples
cargo run --example mesh_network --features legacy-examples

# SDK quickstart
cargo run --example sdk_quickstart --features legacy-examples  
```

## Size Optimization

### Minimal Dependencies
The default build includes only essential dependencies:
- `tokio` (runtime)
- `serde` (serialization)
- `thiserror` (error handling)
- `bytes` (buffer management)

### Binary Size Comparison
- Minimal: ~2MB (no optional deps)
- Core: ~8MB (BLE + database)  
- Full: ~25MB (all features)

### Further Optimization
```bash
# Ultra-minimal build
cargo build --release --no-default-features

# Size analysis
cargo bloat --release --features core

# Dependency tree
cargo tree --features core
```

## Production Checklist

### Pre-Deployment
- [ ] Run `cargo check --no-default-features`
- [ ] Run `cargo check --features core`  
- [ ] Run `cargo test --features mvp`
- [ ] Run security audit: `cargo audit`
- [ ] Check dependencies: `cargo outdated`

### Performance Verification
- [ ] Profile with `cargo bench --features core`
- [ ] Memory usage: `valgrind` or `heaptrack`
- [ ] Binary size: `ls -lh target/release/bitcraps`
- [ ] Startup time: `time ./target/release/bitcraps --help`

### Security Validation
- [ ] No hardcoded secrets: `grep -r \"password\\|secret\\|key\" src/`
- [ ] Dependency vulnerabilities: `cargo audit`
- [ ] Static analysis: `cargo clippy --features core`
- [ ] Supply chain: `cargo supply-chain`

## Troubleshooting

### Common Build Issues

#### Missing System Dependencies
```bash
# Ubuntu/Debian
sudo apt install pkg-config libdbus-1-dev libudev-dev

# macOS  
brew install pkg-config

# Windows
# Use vcpkg or pre-built binaries
```

#### Feature Flag Conflicts
```bash
# Clear conflicting features
cargo clean
cargo check --no-default-features
cargo check --features core
```

#### Cross-Compilation Issues
```bash
# Install target
rustup target add aarch64-linux-android

# Set linker
export CC_aarch64_linux_android=aarch64-linux-android21-clang
```

### Build Cache Optimization
```bash
# Use sccache for faster builds
export RUSTC_WRAPPER=sccache
cargo build --features core

# Cache statistics
sccache --show-stats
```

## Development Workflow

### Quick Iteration
```bash
# Fast development build
cargo check --features core

# Run with file watching
cargo watch -x "check --features core"
```

### Release Preparation
```bash
# Version bump
cargo set-version 0.2.0

# Update dependencies
cargo update

# Full test suite
cargo test --features "core legacy-tests"

# Release build
cargo build --release --features core
```

This configuration provides flexibility for different deployment scenarios while maintaining a clean, minimal default that compiles quickly and has minimal dependencies.