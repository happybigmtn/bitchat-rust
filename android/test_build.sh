#!/bin/bash

# Android Build Test Script for BitCraps
# This script validates the Android build setup

set -e

echo "🚀 BitCraps Android Build Validation"
echo "====================================="

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Must run from bitchat-rust root directory"
    exit 1
fi

echo "📋 Checking build prerequisites..."

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust."
    exit 1
fi
echo "✅ Rust/Cargo found"

# Check for Android NDK (if available)
if [ -z "$ANDROID_NDK_ROOT" ] && [ -z "$NDK_HOME" ]; then
    echo "⚠️  Warning: Android NDK environment variables not set"
    echo "    Set ANDROID_NDK_ROOT or NDK_HOME for cross-compilation"
else
    echo "✅ Android NDK environment detected"
fi

# Check for Java/Gradle (for Android)
if command -v java &> /dev/null; then
    echo "✅ Java found: $(java -version 2>&1 | head -n 1)"
else
    echo "⚠️  Warning: Java not found - required for Android development"
fi

echo ""
echo "📦 Validating Rust dependencies..."

# Check if JNI dependencies are properly configured
if cargo tree --target x86_64-unknown-linux-gnu | grep -q jni; then
    echo "✅ JNI dependency configured"
else
    echo "❌ JNI dependency not found in cargo tree"
fi

echo ""
echo "🔧 Testing Rust build..."

# Test regular build first
if cargo check --lib; then
    echo "✅ Rust library builds successfully"
else
    echo "❌ Rust library build failed"
    exit 1
fi

echo ""
echo "📁 Validating Android project structure..."

# Check Android manifest
if [ -f "android/app/src/main/AndroidManifest.xml" ]; then
    echo "✅ AndroidManifest.xml present"
    
    # Check for critical permissions
    if grep -q "BLUETOOTH_ADVERTISE" android/app/src/main/AndroidManifest.xml; then
        echo "✅ Android 14+ BLE permissions configured"
    else
        echo "❌ Missing Android 14+ BLE permissions"
    fi
    
    if grep -q "FOREGROUND_SERVICE_CONNECTED_DEVICE" android/app/src/main/AndroidManifest.xml; then
        echo "✅ Foreground service permissions configured"
    else
        echo "❌ Missing foreground service permissions"
    fi
else
    echo "❌ AndroidManifest.xml not found"
    exit 1
fi

# Check Gradle files
if [ -f "android/app/build.gradle" ] && [ -f "android/build.gradle" ]; then
    echo "✅ Gradle build files present"
else
    echo "❌ Missing Gradle build files"
    exit 1
fi

# Check service implementation
if [ -f "android/app/src/main/java/com/bitcraps/app/service/BitCrapsService.kt" ]; then
    echo "✅ BitCrapsService implementation present"
else
    echo "❌ BitCrapsService implementation missing"
    exit 1
fi

# Check BLE components
if [ -f "android/app/src/main/java/com/bitcraps/app/ble/BleManager.kt" ] && [ -f "android/app/src/main/java/com/bitcraps/app/ble/BleAdvertiser.kt" ]; then
    echo "✅ BLE components present"
else
    echo "❌ BLE components missing"
    exit 1
fi

echo ""
echo "🎯 Build validation summary:"
echo "=========================="

# Count completed components
total_checks=8
passed_checks=0

[ -f "android/app/src/main/AndroidManifest.xml" ] && ((passed_checks++))
[ -f "android/app/build.gradle" ] && ((passed_checks++))
[ -f "android/app/src/main/java/com/bitcraps/app/service/BitCrapsService.kt" ] && ((passed_checks++))
[ -f "android/app/src/main/java/com/bitcraps/app/ble/BleManager.kt" ] && ((passed_checks++))
[ -f "android/app/src/main/java/com/bitcraps/app/ble/BleAdvertiser.kt" ] && ((passed_checks++))
[ -f "android/app/src/main/java/com/bitcraps/app/MainActivity.kt" ] && ((passed_checks++))
[ -f "android/app/src/main/res/layout/activity_main.xml" ] && ((passed_checks++))
command -v cargo &> /dev/null && ((passed_checks++))

echo "✅ Passed: $passed_checks/$total_checks checks"

if [ $passed_checks -eq $total_checks ]; then
    echo ""
    echo "🎉 SUCCESS: Android build setup is complete!"
    echo "   Ready for Android development and testing."
    echo ""
    echo "Next steps:"
    echo "1. Install Android Studio and SDK"
    echo "2. Configure Android NDK for Rust cross-compilation"
    echo "3. Run 'cd android && ./gradlew build' to build APK"
    echo "4. Test on physical Android 14+ device"
    exit 0
else
    echo ""
    echo "❌ FAILED: Android build setup incomplete"
    echo "   Please address the missing components above."
    exit 1
fi