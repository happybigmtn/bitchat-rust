#!/bin/bash
# BitCraps Mobile SDK Build Script
# Builds Android AAR and iOS Framework from Rust codebase

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

# Check if required tools are installed
check_dependencies() {
    log "Checking build dependencies..."
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    # Check UniFFI
    if ! cargo install --list | grep -q uniffi_bindgen; then
        warn "UniFFI bindgen not found. Installing..."
        cargo install uniffi_bindgen
    fi
    
    success "Dependencies check completed"
}

# Build Rust library for mobile targets
build_rust_lib() {
    log "Building Rust library for mobile targets..."
    
    cd "$ROOT_DIR"
    
    # Add mobile targets if not already added
    rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
    rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
    
    # Build for iOS targets
    log "Building for iOS targets..."
    cargo build --release --target aarch64-apple-ios --features mobile,uniffi
    cargo build --release --target x86_64-apple-ios --features mobile,uniffi
    cargo build --release --target aarch64-apple-ios-sim --features mobile,uniffi
    
    # Build for Android targets
    log "Building for Android targets..."
    cargo build --release --target aarch64-linux-android --features mobile,uniffi
    cargo build --release --target armv7-linux-androideabi --features mobile,uniffi
    cargo build --release --target i686-linux-android --features mobile,uniffi
    cargo build --release --target x86_64-linux-android --features mobile,uniffi
    
    success "Rust library build completed"
}

# Generate UniFFI bindings
generate_bindings() {
    log "Generating UniFFI bindings..."
    
    cd "$ROOT_DIR"
    
    # Create output directories
    mkdir -p bindings/android/src/main/java
    mkdir -p bindings/ios/Sources/BitCrapsSDK
    
    # Generate Kotlin bindings for Android
    log "Generating Kotlin bindings..."
    uniffi-bindgen generate src/bitcraps.udl --language kotlin --out-dir bindings/android/src/main/java
    
    # Generate Swift bindings for iOS
    log "Generating Swift bindings..."
    uniffi-bindgen generate src/bitcraps.udl --language swift --out-dir bindings/ios/Sources/BitCrapsSDK
    
    success "UniFFI bindings generated"
}

# Create iOS Framework
build_ios_framework() {
    log "Building iOS Framework..."
    
    cd "$ROOT_DIR"
    
    # Create framework directory structure
    FRAMEWORK_DIR="build/ios/BitCrapsSDK.xcframework"
    mkdir -p "$FRAMEWORK_DIR"
    
    # Create universal library for iOS device
    log "Creating iOS device library..."
    lipo -create \
        target/aarch64-apple-ios/release/libbitcraps.a \
        -output build/ios/libbitcraps-ios.a
    
    # Create universal library for iOS simulator
    log "Creating iOS simulator library..."
    lipo -create \
        target/x86_64-apple-ios/release/libbitcraps.a \
        target/aarch64-apple-ios-sim/release/libbitcraps.a \
        -output build/ios/libbitcraps-sim.a
    
    # Create XCFramework structure
    log "Creating XCFramework..."
    
    # iOS Device framework
    mkdir -p "$FRAMEWORK_DIR/ios-arm64/BitCrapsSDK.framework"
    cp build/ios/libbitcraps-ios.a "$FRAMEWORK_DIR/ios-arm64/BitCrapsSDK.framework/BitCrapsSDK"
    cp bindings/ios/Sources/BitCrapsSDK/*.swift "$FRAMEWORK_DIR/ios-arm64/BitCrapsSDK.framework/"
    
    # iOS Simulator framework
    mkdir -p "$FRAMEWORK_DIR/ios-arm64_x86_64-simulator/BitCrapsSDK.framework"
    cp build/ios/libbitcraps-sim.a "$FRAMEWORK_DIR/ios-arm64_x86_64-simulator/BitCrapsSDK.framework/BitCrapsSDK"
    cp bindings/ios/Sources/BitCrapsSDK/*.swift "$FRAMEWORK_DIR/ios-arm64_x86_64-simulator/BitCrapsSDK.framework/"
    
    # Create Info.plist files
    create_ios_info_plist "$FRAMEWORK_DIR/ios-arm64/BitCrapsSDK.framework/Info.plist"
    create_ios_info_plist "$FRAMEWORK_DIR/ios-arm64_x86_64-simulator/BitCrapsSDK.framework/Info.plist"
    
    success "iOS Framework created at $FRAMEWORK_DIR"
}

# Create Android AAR
build_android_aar() {
    log "Building Android AAR..."
    
    cd "$ROOT_DIR"
    
    # Create AAR structure
    AAR_DIR="build/android/bitcraps-sdk"
    mkdir -p "$AAR_DIR"
    
    # Copy Kotlin bindings
    mkdir -p "$AAR_DIR/src/main/java"
    cp -r bindings/android/src/main/java/* "$AAR_DIR/src/main/java/"
    
    # Copy native libraries
    mkdir -p "$AAR_DIR/src/main/jniLibs"
    mkdir -p "$AAR_DIR/src/main/jniLibs/arm64-v8a"
    mkdir -p "$AAR_DIR/src/main/jniLibs/armeabi-v7a"
    mkdir -p "$AAR_DIR/src/main/jniLibs/x86"
    mkdir -p "$AAR_DIR/src/main/jniLibs/x86_64"
    
    # Copy native libraries to appropriate directories
    cp target/aarch64-linux-android/release/libbitcraps.so "$AAR_DIR/src/main/jniLibs/arm64-v8a/"
    cp target/armv7-linux-androideabi/release/libbitcraps.so "$AAR_DIR/src/main/jniLibs/armeabi-v7a/"
    cp target/i686-linux-android/release/libbitcraps.so "$AAR_DIR/src/main/jniLibs/x86/"
    cp target/x86_64-linux-android/release/libbitcraps.so "$AAR_DIR/src/main/jniLibs/x86_64/"
    
    # Create Android manifest
    create_android_manifest "$AAR_DIR/src/main/AndroidManifest.xml"
    
    # Build AAR using Gradle
    if [ -d "mobile/android/sdk" ]; then
        log "Building AAR with Gradle..."
        cd mobile/android/sdk
        ./gradlew assembleRelease
        cd "$ROOT_DIR"
        
        # Copy built AAR
        cp mobile/android/sdk/build/outputs/aar/sdk-release.aar build/android/bitcraps-sdk.aar
        success "Android AAR created at build/android/bitcraps-sdk.aar"
    else
        warn "Gradle build not available. Creating manual AAR structure at $AAR_DIR"
    fi
}

# Create iOS Info.plist
create_ios_info_plist() {
    local plist_path="$1"
    cat > "$plist_path" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>BitCrapsSDK</string>
    <key>CFBundleIdentifier</key>
    <string>com.bitcraps.sdk</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>BitCrapsSDK</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>MinimumOSVersion</key>
    <string>13.0</string>
</dict>
</plist>
EOF
}

# Create Android Manifest
create_android_manifest() {
    local manifest_path="$1"
    cat > "$manifest_path" << EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.bitcraps.sdk">

    <!-- Bluetooth permissions -->
    <uses-permission android:name="android.permission.BLUETOOTH" />
    <uses-permission android:name="android.permission.BLUETOOTH_ADMIN" />
    <uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />
    
    <!-- Android 12+ Bluetooth permissions -->
    <uses-permission android:name="android.permission.BLUETOOTH_SCAN" />
    <uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" />
    <uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
    
    <!-- Internet for optional features -->
    <uses-permission android:name="android.permission.INTERNET" />
    
    <!-- Wake lock for background processing -->
    <uses-permission android:name="android.permission.WAKE_LOCK" />
    
    <!-- Foreground service for game sessions -->
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />

</manifest>
EOF
}

# Create distribution packages
create_distributions() {
    log "Creating distribution packages..."
    
    cd "$ROOT_DIR"
    mkdir -p dist
    
    # Create iOS distribution
    if [ -d "build/ios" ]; then
        log "Packaging iOS distribution..."
        cd build/ios
        tar -czf "../../dist/bitcraps-ios-sdk-1.0.0.tar.gz" BitCrapsSDK.xcframework
        cd "$ROOT_DIR"
        success "iOS SDK packaged: dist/bitcraps-ios-sdk-1.0.0.tar.gz"
    fi
    
    # Create Android distribution
    if [ -f "build/android/bitcraps-sdk.aar" ]; then
        log "Packaging Android distribution..."
        cp build/android/bitcraps-sdk.aar dist/bitcraps-android-sdk-1.0.0.aar
        success "Android SDK packaged: dist/bitcraps-android-sdk-1.0.0.aar"
    fi
    
    # Create documentation package
    log "Creating documentation package..."
    mkdir -p dist/docs
    cp README.md dist/docs/ 2>/dev/null || true
    cp -r docs/ dist/docs/ 2>/dev/null || true
    cp -r examples/ dist/docs/ 2>/dev/null || true
    
    success "Distribution packages created in dist/"
}

# Main build function
main() {
    log "🚀 BitCraps Mobile SDK Build Started"
    
    # Parse command line arguments
    BUILD_ANDROID=true
    BUILD_IOS=true
    SKIP_CHECKS=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --android-only)
                BUILD_IOS=false
                shift
                ;;
            --ios-only)
                BUILD_ANDROID=false
                shift
                ;;
            --skip-checks)
                SKIP_CHECKS=true
                shift
                ;;
            *)
                echo "Unknown option: $1"
                echo "Usage: $0 [--android-only] [--ios-only] [--skip-checks]"
                exit 1
                ;;
        esac
    done
    
    # Check dependencies
    if [ "$SKIP_CHECKS" = false ]; then
        check_dependencies
    fi
    
    # Create build directories
    mkdir -p build/android build/ios bindings
    
    # Build steps
    build_rust_lib
    generate_bindings
    
    if [ "$BUILD_IOS" = true ]; then
        build_ios_framework
    fi
    
    if [ "$BUILD_ANDROID" = true ]; then
        build_android_aar
    fi
    
    create_distributions
    
    success "🎉 BitCraps Mobile SDK Build Completed Successfully!"
    log "📦 Distribution files:"
    ls -la dist/ 2>/dev/null || log "No distribution files created"
    
    log "📱 Mobile SDK Usage:"
    log "  - Android: Add bitcraps-android-sdk-1.0.0.aar to your Android project"
    log "  - iOS: Add BitCrapsSDK.xcframework to your Xcode project"
    log "  - See examples/ for integration guides"
}

# Run the main function with all arguments
main "$@"