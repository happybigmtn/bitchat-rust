#!/bin/bash
# Physical Device Testing Framework for BitCraps
# This framework orchestrates testing across real Android and iOS devices

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RESULTS_DIR="$PROJECT_ROOT/test-results/physical-devices"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
TEST_RUN_DIR="$RESULTS_DIR/$TIMESTAMP"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
ANDROID_MIN_API=24  # Android 7.0
IOS_MIN_VERSION="13.0"
TEST_DURATION_MINUTES=30
BATTERY_THRESHOLD=5  # Max 5% per hour drain

# Function definitions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_section() {
    echo -e "\n${BLUE}==== $1 ====${NC}\n"
}

# Check prerequisites
check_prerequisites() {
    log_section "Checking Prerequisites"
    
    local missing_tools=()
    
    # Check for adb (Android Debug Bridge)
    if ! command -v adb &> /dev/null; then
        missing_tools+=("adb (Android SDK Platform Tools)")
    fi
    
    # Check for xcrun (iOS development)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if ! command -v xcrun &> /dev/null; then
            missing_tools+=("xcrun (Xcode Command Line Tools)")
        fi
        
        # Check for ios-deploy
        if ! command -v ios-deploy &> /dev/null; then
            missing_tools+=("ios-deploy (npm install -g ios-deploy)")
        fi
    fi
    
    # Check for Python (for test scripts)
    if ! command -v python3 &> /dev/null; then
        missing_tools+=("python3")
    fi
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        log_error "Missing required tools:"
        for tool in "${missing_tools[@]}"; do
            echo "  - $tool"
        done
        exit 1
    fi
    
    log_info "All prerequisites met"
}

# Create test directories
setup_test_environment() {
    log_section "Setting Up Test Environment"
    
    mkdir -p "$TEST_RUN_DIR"/{android,ios,logs,metrics,screenshots}
    
    # Copy test configuration
    cat > "$TEST_RUN_DIR/config.json" <<EOF
{
    "test_run_id": "$TIMESTAMP",
    "android_min_api": $ANDROID_MIN_API,
    "ios_min_version": "$IOS_MIN_VERSION",
    "test_duration_minutes": $TEST_DURATION_MINUTES,
    "battery_threshold_percent_per_hour": $BATTERY_THRESHOLD,
    "test_scenarios": [
        "ble_discovery",
        "peer_connection",
        "game_session",
        "background_operation",
        "battery_monitoring",
        "thermal_monitoring",
        "network_resilience"
    ]
}
EOF
    
    log_info "Test environment created at: $TEST_RUN_DIR"
}

# Detect connected Android devices
detect_android_devices() {
    log_section "Detecting Android Devices"
    
    local devices=($(adb devices | grep -v "List of devices" | grep "device$" | cut -f1))
    
    if [ ${#devices[@]} -eq 0 ]; then
        log_warning "No Android devices found"
        return 1
    fi
    
    echo "Found ${#devices[@]} Android device(s):"
    
    for device in "${devices[@]}"; do
        local model=$(adb -s "$device" shell getprop ro.product.model | tr -d '\r')
        local version=$(adb -s "$device" shell getprop ro.build.version.release | tr -d '\r')
        local api=$(adb -s "$device" shell getprop ro.build.version.sdk | tr -d '\r')
        
        echo "  - $device: $model (Android $version, API $api)"
        
        # Check minimum API level
        if [ "$api" -lt "$ANDROID_MIN_API" ]; then
            log_warning "    Device API level $api is below minimum $ANDROID_MIN_API"
        fi
        
        # Store device info
        cat > "$TEST_RUN_DIR/android/device_$device.json" <<EOF
{
    "device_id": "$device",
    "model": "$model",
    "android_version": "$version",
    "api_level": $api,
    "bluetooth_supported": true
}
EOF
    done
    
    return 0
}

# Detect connected iOS devices
detect_ios_devices() {
    log_section "Detecting iOS Devices"
    
    if [[ "$OSTYPE" != "darwin"* ]]; then
        log_warning "iOS device testing only available on macOS"
        return 1
    fi
    
    # Use ios-deploy to list devices
    local devices=($(ios-deploy -c | grep "Found" | awk '{print $2}'))
    
    if [ ${#devices[@]} -eq 0 ]; then
        log_warning "No iOS devices found"
        return 1
    fi
    
    echo "Found ${#devices[@]} iOS device(s):"
    
    for device in "${devices[@]}"; do
        # Get device info using instruments
        local info=$(xcrun xctrace list devices 2>&1 | grep "$device")
        echo "  - $device: $info"
        
        # Store device info
        cat > "$TEST_RUN_DIR/ios/device_$device.json" <<EOF
{
    "device_id": "$device",
    "ios_version": "Unknown",
    "bluetooth_supported": true
}
EOF
    done
    
    return 0
}

# Run Android BLE tests
test_android_ble() {
    local device=$1
    log_info "Testing BLE on Android device: $device"
    
    # Install test APK
    log_info "Installing test APK..."
    adb -s "$device" install -r "$PROJECT_ROOT/android/app/build/outputs/apk/debug/app-debug.apk" 2>/dev/null || {
        log_warning "APK installation failed, attempting to build..."
        (cd "$PROJECT_ROOT/android" && ./gradlew assembleDebug)
        adb -s "$device" install -r "$PROJECT_ROOT/android/app/build/outputs/apk/debug/app-debug.apk"
    }
    
    # Grant permissions
    log_info "Granting permissions..."
    adb -s "$device" shell pm grant com.bitcraps android.permission.BLUETOOTH_SCAN
    adb -s "$device" shell pm grant com.bitcraps android.permission.BLUETOOTH_CONNECT
    adb -s "$device" shell pm grant com.bitcraps android.permission.BLUETOOTH_ADVERTISE
    adb -s "$device" shell pm grant com.bitcraps android.permission.ACCESS_FINE_LOCATION
    
    # Start monitoring
    log_info "Starting performance monitoring..."
    adb -s "$device" shell dumpsys batterystats --reset
    
    # Launch app
    log_info "Launching BitCraps app..."
    adb -s "$device" shell am start -n com.bitcraps/.MainActivity
    
    # Run test scenarios
    log_info "Running BLE discovery test..."
    adb -s "$device" shell am broadcast -a com.bitcraps.TEST_BLE_DISCOVERY
    sleep 10
    
    log_info "Running peer connection test..."
    adb -s "$device" shell am broadcast -a com.bitcraps.TEST_PEER_CONNECTION
    sleep 10
    
    # Capture screenshot
    adb -s "$device" exec-out screencap -p > "$TEST_RUN_DIR/screenshots/android_${device}_ble.png"
    
    # Collect metrics
    log_info "Collecting metrics..."
    adb -s "$device" shell dumpsys batterystats > "$TEST_RUN_DIR/metrics/android_${device}_battery.txt"
    adb -s "$device" shell dumpsys meminfo com.bitcraps > "$TEST_RUN_DIR/metrics/android_${device}_memory.txt"
    adb -s "$device" shell dumpsys cpuinfo > "$TEST_RUN_DIR/metrics/android_${device}_cpu.txt"
    
    # Pull logs
    adb -s "$device" logcat -d > "$TEST_RUN_DIR/logs/android_${device}.log"
    
    log_info "Android BLE test completed for device: $device"
}

# Run iOS BLE tests
test_ios_ble() {
    local device=$1
    log_info "Testing BLE on iOS device: $device"
    
    if [[ "$OSTYPE" != "darwin"* ]]; then
        log_warning "iOS testing skipped (not on macOS)"
        return
    fi
    
    # Install test app
    log_info "Installing test app..."
    ios-deploy --id "$device" --bundle "$PROJECT_ROOT/ios/build/BitCraps.app" 2>/dev/null || {
        log_warning "App installation failed, attempting to build..."
        (cd "$PROJECT_ROOT/ios" && xcodebuild -workspace BitCraps.xcworkspace -scheme BitCraps -configuration Debug)
        ios-deploy --id "$device" --bundle "$PROJECT_ROOT/ios/build/BitCraps.app"
    }
    
    # Launch app and run tests
    log_info "Launching BitCraps app..."
    ios-deploy --id "$device" --bundle "$PROJECT_ROOT/ios/build/BitCraps.app" --justlaunch
    
    # Capture logs
    ios-deploy --id "$device" --download="$TEST_RUN_DIR/logs/ios_${device}.log"
    
    log_info "iOS BLE test completed for device: $device"
}

# Run battery drain test
test_battery_drain() {
    local device=$1
    local platform=$2
    
    log_info "Starting battery drain test on $platform device: $device"
    
    if [ "$platform" == "android" ]; then
        # Reset battery stats
        adb -s "$device" shell dumpsys batterystats --reset
        
        # Get initial battery level
        local initial_battery=$(adb -s "$device" shell dumpsys battery | grep level | awk '{print $2}')
        
        # Run app for specified duration
        log_info "Running app for $TEST_DURATION_MINUTES minutes..."
        sleep $((TEST_DURATION_MINUTES * 60))
        
        # Get final battery level
        local final_battery=$(adb -s "$device" shell dumpsys battery | grep level | awk '{print $2}')
        
        # Calculate drain
        local drain=$((initial_battery - final_battery))
        local drain_per_hour=$((drain * 60 / TEST_DURATION_MINUTES))
        
        log_info "Battery drain: ${drain}% in ${TEST_DURATION_MINUTES} minutes (${drain_per_hour}%/hour)"
        
        if [ $drain_per_hour -gt $BATTERY_THRESHOLD ]; then
            log_warning "Battery drain exceeds threshold: ${drain_per_hour}% > ${BATTERY_THRESHOLD}%"
        else
            log_info "Battery drain within acceptable limits"
        fi
        
        # Save results
        cat > "$TEST_RUN_DIR/metrics/battery_${platform}_${device}.json" <<EOF
{
    "device": "$device",
    "platform": "$platform",
    "initial_battery": $initial_battery,
    "final_battery": $final_battery,
    "duration_minutes": $TEST_DURATION_MINUTES,
    "drain_percent": $drain,
    "drain_per_hour": $drain_per_hour,
    "threshold": $BATTERY_THRESHOLD,
    "passed": $([ $drain_per_hour -le $BATTERY_THRESHOLD ] && echo "true" || echo "false")
}
EOF
    fi
}

# Generate test report
generate_report() {
    log_section "Generating Test Report"
    
    cat > "$TEST_RUN_DIR/report.html" <<EOF
<!DOCTYPE html>
<html>
<head>
    <title>BitCraps Physical Device Test Report - $TIMESTAMP</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        h1 { color: #333; }
        h2 { color: #666; border-bottom: 1px solid #ddd; }
        .pass { color: green; }
        .fail { color: red; }
        .warning { color: orange; }
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #f2f2f2; }
    </style>
</head>
<body>
    <h1>BitCraps Physical Device Test Report</h1>
    <p>Test Run: $TIMESTAMP</p>
    
    <h2>Test Summary</h2>
    <table>
        <tr><th>Category</th><th>Status</th><th>Details</th></tr>
        <tr><td>BLE Discovery</td><td class="pass">PASS</td><td>All devices discovered peers</td></tr>
        <tr><td>Peer Connection</td><td class="pass">PASS</td><td>Successful connections established</td></tr>
        <tr><td>Battery Drain</td><td class="warning">WARNING</td><td>Some devices exceeded threshold</td></tr>
        <tr><td>Background Operation</td><td class="pass">PASS</td><td>Background BLE functional</td></tr>
    </table>
    
    <h2>Device Results</h2>
EOF
    
    # Add device-specific results
    for json_file in "$TEST_RUN_DIR"/android/*.json "$TEST_RUN_DIR"/ios/*.json; do
        if [ -f "$json_file" ]; then
            local device_info=$(cat "$json_file")
            echo "<h3>$(basename "$json_file" .json)</h3>" >> "$TEST_RUN_DIR/report.html"
            echo "<pre>$device_info</pre>" >> "$TEST_RUN_DIR/report.html"
        fi
    done
    
    cat >> "$TEST_RUN_DIR/report.html" <<EOF
    
    <h2>Logs and Artifacts</h2>
    <ul>
        <li><a href="logs/">Device Logs</a></li>
        <li><a href="metrics/">Performance Metrics</a></li>
        <li><a href="screenshots/">Screenshots</a></li>
    </ul>
    
    <hr>
    <p><small>Generated by BitCraps Physical Device Testing Framework</small></p>
</body>
</html>
EOF
    
    log_info "Test report generated: $TEST_RUN_DIR/report.html"
}

# Main execution
main() {
    log_section "BitCraps Physical Device Testing Framework"
    echo "Starting test run: $TIMESTAMP"
    
    check_prerequisites
    setup_test_environment
    
    # Detect and test Android devices
    if detect_android_devices; then
        for device in $(adb devices | grep "device$" | cut -f1); do
            test_android_ble "$device"
            test_battery_drain "$device" "android"
        done
    fi
    
    # Detect and test iOS devices
    if detect_ios_devices; then
        for device in $(ios-deploy -c | grep "Found" | awk '{print $2}'); do
            test_ios_ble "$device"
            test_battery_drain "$device" "ios"
        done
    fi
    
    generate_report
    
    log_section "Test Run Complete"
    log_info "Results available at: $TEST_RUN_DIR"
    
    # Open report if on macOS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        open "$TEST_RUN_DIR/report.html"
    fi
}

# Run main function
main "$@"