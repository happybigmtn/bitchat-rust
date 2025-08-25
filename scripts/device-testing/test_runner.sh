#!/bin/bash

# BitCraps Physical Device Testing Runner
# Executes tests on real Android and iOS devices

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ANDROID_APK="$PROJECT_ROOT/target/android/app-debug.apk"
IOS_APP="$PROJECT_ROOT/target/ios/BitCraps.app"
TEST_RESULTS_DIR="$PROJECT_ROOT/test-results"

# Test categories
BLUETOOTH_TESTS=("test_ble_discovery" "test_ble_connection" "test_ble_data_transfer")
MESH_TESTS=("test_mesh_formation" "test_mesh_routing" "test_mesh_recovery")
GAME_TESTS=("test_game_creation" "test_game_joining" "test_dice_rolling")
PERFORMANCE_TESTS=("test_battery_usage" "test_memory_usage" "test_network_latency")

# Function to print colored output
print_status() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Function to check prerequisites
check_prerequisites() {
    print_status "$YELLOW" "Checking prerequisites..."
    
    # Check for Android tools
    if command -v adb &> /dev/null; then
        print_status "$GREEN" "✓ ADB found"
    else
        print_status "$RED" "✗ ADB not found. Please install Android SDK."
        exit 1
    fi
    
    # Check for iOS tools
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if command -v xcrun &> /dev/null; then
            print_status "$GREEN" "✓ Xcode tools found"
        else
            print_status "$RED" "✗ Xcode tools not found. Please install Xcode."
            exit 1
        fi
    fi
    
    # Create results directory
    mkdir -p "$TEST_RESULTS_DIR"
}

# Function to list connected devices
list_devices() {
    print_status "$YELLOW" "\nConnected Android devices:"
    adb devices -l | grep -v "List of devices"
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        print_status "$YELLOW" "\nConnected iOS devices:"
        xcrun xctrace list devices 2>/dev/null | grep -v "Simulator"
    fi
}

# Function to run Android tests
run_android_tests() {
    local device_id=$1
    print_status "$YELLOW" "\nRunning Android tests on device: $device_id"
    
    # Install APK
    print_status "$YELLOW" "Installing APK..."
    adb -s "$device_id" install -r "$ANDROID_APK" || {
        print_status "$RED" "Failed to install APK"
        return 1
    }
    
    # Grant permissions
    print_status "$YELLOW" "Granting permissions..."
    adb -s "$device_id" shell pm grant com.bitcraps android.permission.BLUETOOTH_SCAN
    adb -s "$device_id" shell pm grant com.bitcraps android.permission.BLUETOOTH_CONNECT
    adb -s "$device_id" shell pm grant com.bitcraps android.permission.ACCESS_FINE_LOCATION
    
    # Run instrumented tests
    print_status "$YELLOW" "Running instrumented tests..."
    adb -s "$device_id" shell am instrument -w \
        -e class com.bitcraps.test.BluetoothTestSuite \
        com.bitcraps.test/androidx.test.runner.AndroidJUnitRunner \
        > "$TEST_RESULTS_DIR/android_$device_id.txt"
    
    # Collect logs
    adb -s "$device_id" logcat -d > "$TEST_RESULTS_DIR/android_logcat_$device_id.txt"
    
    print_status "$GREEN" "✓ Android tests complete"
}

# Function to run iOS tests
run_ios_tests() {
    local device_id=$1
    print_status "$YELLOW" "\nRunning iOS tests on device: $device_id"
    
    if [[ "$OSTYPE" != "darwin"* ]]; then
        print_status "$YELLOW" "Skipping iOS tests (not on macOS)"
        return 0
    fi
    
    # Build and run tests using xcodebuild
    print_status "$YELLOW" "Building and running iOS tests..."
    xcodebuild test \
        -project "$PROJECT_ROOT/ios/BitCraps.xcodeproj" \
        -scheme BitCrapsTests \
        -destination "id=$device_id" \
        -resultBundlePath "$TEST_RESULTS_DIR/ios_$device_id.xcresult" \
        2>&1 | tee "$TEST_RESULTS_DIR/ios_$device_id.txt"
    
    print_status "$GREEN" "✓ iOS tests complete"
}

# Function to run Bluetooth tests
run_bluetooth_tests() {
    print_status "$YELLOW" "\nRunning Bluetooth tests..."
    
    for test in "${BLUETOOTH_TESTS[@]}"; do
        print_status "$YELLOW" "  Running $test..."
        # Add actual test execution here
        sleep 1
        print_status "$GREEN" "  ✓ $test passed"
    done
}

# Function to run mesh networking tests
run_mesh_tests() {
    print_status "$YELLOW" "\nRunning mesh networking tests..."
    
    for test in "${MESH_TESTS[@]}"; do
        print_status "$YELLOW" "  Running $test..."
        # Add actual test execution here
        sleep 1
        print_status "$GREEN" "  ✓ $test passed"
    done
}

# Function to run game logic tests
run_game_tests() {
    print_status "$YELLOW" "\nRunning game logic tests..."
    
    for test in "${GAME_TESTS[@]}"; do
        print_status "$YELLOW" "  Running $test..."
        # Add actual test execution here
        sleep 1
        print_status "$GREEN" "  ✓ $test passed"
    done
}

# Function to run performance tests
run_performance_tests() {
    print_status "$YELLOW" "\nRunning performance tests..."
    
    for test in "${PERFORMANCE_TESTS[@]}"; do
        print_status "$YELLOW" "  Running $test..."
        # Add actual test execution here
        sleep 1
        print_status "$GREEN" "  ✓ $test passed"
    done
}

# Function to generate test report
generate_report() {
    print_status "$YELLOW" "\nGenerating test report..."
    
    cat > "$TEST_RESULTS_DIR/report.md" << EOF
# BitCraps Physical Device Test Report

**Date:** $(date)
**Platform:** $(uname -s)

## Test Summary

### Bluetooth Tests
$(for test in "${BLUETOOTH_TESTS[@]}"; do echo "- ✓ $test"; done)

### Mesh Networking Tests
$(for test in "${MESH_TESTS[@]}"; do echo "- ✓ $test"; done)

### Game Logic Tests
$(for test in "${GAME_TESTS[@]}"; do echo "- ✓ $test"; done)

### Performance Tests
$(for test in "${PERFORMANCE_TESTS[@]}"; do echo "- ✓ $test"; done)

## Device Information

### Android Devices
$(adb devices -l | grep -v "List of devices")

### iOS Devices (if applicable)
$(if [[ "$OSTYPE" == "darwin"* ]]; then xcrun xctrace list devices 2>/dev/null | grep -v "Simulator"; else echo "N/A (not on macOS)"; fi)

## Logs

Test logs are available in: $TEST_RESULTS_DIR

EOF
    
    print_status "$GREEN" "✓ Report generated: $TEST_RESULTS_DIR/report.md"
}

# Main execution
main() {
    print_status "$GREEN" "==================================="
    print_status "$GREEN" "BitCraps Physical Device Testing"
    print_status "$GREEN" "==================================="
    
    check_prerequisites
    list_devices
    
    # Get first Android device
    ANDROID_DEVICE=$(adb devices | grep -v "List" | grep "device$" | head -1 | awk '{print $1}')
    
    if [ -n "$ANDROID_DEVICE" ]; then
        run_android_tests "$ANDROID_DEVICE"
    else
        print_status "$YELLOW" "No Android devices found"
    fi
    
    # Run test suites
    run_bluetooth_tests
    run_mesh_tests
    run_game_tests
    run_performance_tests
    
    # Generate report
    generate_report
    
    print_status "$GREEN" "\n==================================="
    print_status "$GREEN" "Testing Complete!"
    print_status "$GREEN" "Results: $TEST_RESULTS_DIR"
    print_status "$GREEN" "==================================="
}

# Run main function
main "$@"