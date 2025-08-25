#!/bin/bash

# Generate UniFFI bindings for mobile platforms

set -e

echo "Generating UniFFI bindings..."

# Build the library with uniffi feature
echo "Building library with UniFFI feature..."
cargo build --lib --features uniffi

# Generate Kotlin bindings for Android
echo "Generating Kotlin bindings..."
cargo run --bin uniffi-bindgen generate \
    --library target/debug/libbitcraps.so \
    --language kotlin \
    --out-dir android/app/src/main/java \
    src/bitcraps_simple.udl || echo "Kotlin generation needs uniffi-bindgen installed"

# Generate Swift bindings for iOS
echo "Generating Swift bindings..."
cargo run --bin uniffi-bindgen generate \
    --library target/debug/libbitcraps.dylib \
    --language swift \
    --out-dir ios/BitCraps \
    src/bitcraps_simple.udl || echo "Swift generation needs uniffi-bindgen installed"

echo "Bindings generation complete!"