# Physical Device Test Lab Setup Guide

## Overview

This guide provides comprehensive instructions for setting up and maintaining a physical device test lab for BitCraps BLE mesh networking validation. Physical devices are **required** for BLE testing as simulators/emulators do not support Bluetooth Low Energy.

**Document Version**: 1.0  
**Date**: 2025-08-24  
**Estimated Setup Time**: 4-6 hours  
**Budget**: $2,000-5,000 (depending on device coverage)

---

## Required Device Matrix

### Android Devices (Minimum 5)

| Device | OS Version | Purpose | Priority | Cost |
|--------|------------|---------|----------|------|
| **Google Pixel 7/8** | Android 14 (API 34) | Latest Android validation | P0 | $600 |
| **Samsung Galaxy S22/S23** | Android 13-14 | OEM-specific testing | P0 | $800 |
| **OnePlus 9/10** | Android 12-13 | Performance testing | P1 | $500 |
| **Xiaomi Mi 11/12** | Android 11-12 | Battery optimization testing | P1 | $400 |
| **Google Pixel 3a/4a** | Android 10-11 | Older device support | P2 | $200 |

### iOS Devices (Minimum 5)

| Device | OS Version | Purpose | Priority | Cost |
|--------|------------|---------|----------|------|
| **iPhone 14/15** | iOS 17 | Latest iOS validation | P0 | $800 |
| **iPhone 12/13** | iOS 16 | Common device testing | P0 | $600 |
| **iPhone SE 3** | iOS 15-17 | Budget device testing | P1 | $400 |
| **iPad Air** | iPadOS 16-17 | Tablet testing | P2 | $600 |
| **iPhone X/11** | iOS 15 | Older device support | P2 | $300 |

### Total Budget Breakdown

- **Android devices**: $2,500
- **iOS devices**: $2,700
- **Accessories** (cables, stands, hub): $300
- **Total**: ~$5,500 (can start with $2,000 for P0 devices)

---

## Lab Setup Instructions

### 1. Physical Infrastructure

#### Device Rack Setup
```
┌─────────────────────────────────────────┐
│          Device Test Rack               │
├─────────────────────────────────────────┤
│  [P7] [S22] [OP9] [Mi11] [P3a]         │  Android Row
│   🔌   🔌    🔌    🔌     🔌           │  (USB-C)
├─────────────────────────────────────────┤
│  [i14] [i12] [SE3] [iPad] [i11]        │  iOS Row
│   🔌   🔌    🔌    🔌     🔌           │  (Lightning/USB-C)
├─────────────────────────────────────────┤
│        USB Hub (20-port)                │
│         Connected to PC                 │
└─────────────────────────────────────────┘
```

#### Required Hardware
- **20-port powered USB hub** (Anker recommended)
- **Device stands** (adjustable angle)
- **USB cables** (3ft, high-quality)
- **Power strips** with surge protection
- **Webcam** for remote monitoring (optional)
- **Labels** for device identification

### 2. Device Preparation

#### Android Setup Script
```bash
#!/bin/bash
# setup_android_device.sh

echo "Setting up Android device for testing..."

# Enable developer options (manual step required)
echo "1. Go to Settings > About Phone"
echo "2. Tap 'Build Number' 7 times"
echo "Press Enter when complete..."
read

# Enable USB debugging
adb shell settings put global development_settings_enabled 1
adb shell settings put global adb_enabled 1

# Stay awake while charging
adb shell settings put global stay_on_while_plugged_in 15

# Disable battery optimization for test app
PACKAGE="com.bitcraps.app"
adb shell dumpsys deviceidle whitelist +$PACKAGE

# Set animation scales to 0 for faster testing
adb shell settings put global window_animation_scale 0
adb shell settings put global transition_animation_scale 0
adb shell settings put global animator_duration_scale 0

# Grant all permissions
adb shell pm grant $PACKAGE android.permission.BLUETOOTH_SCAN
adb shell pm grant $PACKAGE android.permission.BLUETOOTH_CONNECT
adb shell pm grant $PACKAGE android.permission.BLUETOOTH_ADVERTISE
adb shell pm grant $PACKAGE android.permission.ACCESS_FINE_LOCATION
adb shell pm grant $PACKAGE android.permission.POST_NOTIFICATIONS

echo "✅ Android device setup complete!"
```

#### iOS Setup Script
```bash
#!/bin/bash
# setup_ios_device.sh

echo "Setting up iOS device for testing..."

# Install iOS dependencies
brew install ios-deploy
brew install libimobiledevice

# Check device connection
idevice_id -l

# Install test app
ios-deploy --bundle BitCraps.app --id <device_id>

# Enable UI testing
instruments -w <device_id>

echo "✅ iOS device setup complete!"
```

### 3. Device Configuration

#### Android Developer Settings
1. **Developer Options**
   - USB debugging: ON
   - Stay awake: ON
   - USB debugging (Security settings): ON
   - Disable permission monitoring: ON

2. **Battery Optimization**
   - Settings → Battery → Battery Optimization
   - BitCraps app → Don't optimize

3. **Bluetooth Settings**
   - Bluetooth: Always ON
   - Scanning: Always available

#### iOS Developer Settings
1. **Settings → Developer** (appears after Xcode connection)
   - Enable UI Automation: ON
   - Network Link Conditioner: Available

2. **Settings → Privacy & Security → Developer Mode**
   - Developer Mode: ON

3. **Settings → Bluetooth**
   - Bluetooth: ON
   - Allow New Connections: Always

---

## Test Orchestration

### Device Test Runner Script
```python
#!/usr/bin/env python3
# device_test_runner.py

import subprocess
import time
import json
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import List, Dict

class DeviceTestRunner:
    def __init__(self):
        self.devices = self.discover_devices()
        self.results = {}
    
    def discover_devices(self) -> Dict[str, List[str]]:
        """Discover connected devices"""
        devices = {
            'android': [],
            'ios': []
        }
        
        # Discover Android devices
        result = subprocess.run(['adb', 'devices'], capture_output=True, text=True)
        for line in result.stdout.split('\n'):
            if '\tdevice' in line:
                device_id = line.split('\t')[0]
                devices['android'].append(device_id)
        
        # Discover iOS devices
        result = subprocess.run(['idevice_id', '-l'], capture_output=True, text=True)
        for device_id in result.stdout.strip().split('\n'):
            if device_id:
                devices['ios'].append(device_id)
        
        return devices
    
    def run_android_test(self, device_id: str, test_name: str) -> Dict:
        """Run test on Android device"""
        print(f"🤖 Running {test_name} on Android {device_id}")
        
        start_time = time.time()
        
        # Install APK
        subprocess.run([
            'adb', '-s', device_id, 'install', '-r', 
            'android/app/build/outputs/apk/debug/app-debug.apk'
        ])
        
        # Clear logcat
        subprocess.run(['adb', '-s', device_id, 'logcat', '-c'])
        
        # Run instrumented test
        result = subprocess.run([
            'adb', '-s', device_id, 'shell',
            'am', 'instrument', '-w', '-r',
            '-e', 'debug', 'false',
            '-e', 'class', f'com.bitcraps.app.test.{test_name}',
            'com.bitcraps.app.test/androidx.test.runner.AndroidJUnitRunner'
        ], capture_output=True, text=True)
        
        # Collect logs
        logs = subprocess.run(
            ['adb', '-s', device_id, 'logcat', '-d'],
            capture_output=True, text=True
        ).stdout
        
        # Get battery stats
        battery = subprocess.run(
            ['adb', '-s', device_id, 'shell', 'dumpsys', 'battery'],
            capture_output=True, text=True
        ).stdout
        
        duration = time.time() - start_time
        
        return {
            'device_id': device_id,
            'platform': 'android',
            'test': test_name,
            'duration': duration,
            'success': 'FAILURES!!!' not in result.stdout,
            'output': result.stdout,
            'logs': logs[-10000:],  # Last 10KB of logs
            'battery_level': self.parse_battery_level(battery)
        }
    
    def run_ios_test(self, device_id: str, test_name: str) -> Dict:
        """Run test on iOS device"""
        print(f"🍎 Running {test_name} on iOS {device_id}")
        
        start_time = time.time()
        
        # Run XCTest
        result = subprocess.run([
            'xcodebuild', 'test',
            '-project', 'ios/BitCraps.xcodeproj',
            '-scheme', 'BitCraps',
            '-destination', f'id={device_id}',
            '-only-testing', f'BitCrapsTests/{test_name}'
        ], capture_output=True, text=True)
        
        duration = time.time() - start_time
        
        return {
            'device_id': device_id,
            'platform': 'ios',
            'test': test_name,
            'duration': duration,
            'success': 'TEST FAILED' not in result.stdout,
            'output': result.stdout
        }
    
    def parse_battery_level(self, battery_output: str) -> int:
        """Parse battery level from dumpsys output"""
        for line in battery_output.split('\n'):
            if 'level:' in line:
                return int(line.split(':')[1].strip())
        return -1
    
    def run_parallel_tests(self, test_suite: List[str]):
        """Run tests in parallel across all devices"""
        with ThreadPoolExecutor(max_workers=len(self.devices['android']) + len(self.devices['ios'])) as executor:
            futures = []
            
            # Submit Android tests
            for device_id in self.devices['android']:
                for test in test_suite:
                    future = executor.submit(self.run_android_test, device_id, test)
                    futures.append(future)
            
            # Submit iOS tests
            for device_id in self.devices['ios']:
                for test in test_suite:
                    future = executor.submit(self.run_ios_test, device_id, test)
                    futures.append(future)
            
            # Collect results
            for future in as_completed(futures):
                result = future.result()
                device_id = result['device_id']
                test_name = result['test']
                
                if device_id not in self.results:
                    self.results[device_id] = {}
                
                self.results[device_id][test_name] = result
                
                # Print progress
                status = "✅" if result['success'] else "❌"
                print(f"{status} {device_id}: {test_name} ({result['duration']:.2f}s)")
    
    def generate_report(self) -> str:
        """Generate test report"""
        report = "# Device Test Report\n\n"
        report += f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}\n"
        report += f"**Devices**: {len(self.devices['android'])} Android, {len(self.devices['ios'])} iOS\n\n"
        
        # Summary table
        report += "## Summary\n\n"
        report += "| Device | Platform | Tests | Passed | Failed | Duration |\n"
        report += "|--------|----------|-------|--------|--------|----------|\n"
        
        for device_id, tests in self.results.items():
            platform = 'Android' if device_id in self.devices['android'] else 'iOS'
            total = len(tests)
            passed = sum(1 for t in tests.values() if t['success'])
            failed = total - passed
            duration = sum(t['duration'] for t in tests.values())
            
            report += f"| {device_id[:8]} | {platform} | {total} | {passed} | {failed} | {duration:.1f}s |\n"
        
        # Detailed results
        report += "\n## Detailed Results\n\n"
        for device_id, tests in self.results.items():
            report += f"### Device: {device_id}\n\n"
            
            for test_name, result in tests.items():
                status = "✅ PASS" if result['success'] else "❌ FAIL"
                report += f"- **{test_name}**: {status} ({result['duration']:.2f}s)\n"
                
                if not result['success'] and 'output' in result:
                    report += f"  ```\n  {result['output'][:500]}\n  ```\n"
        
        return report
    
    def save_results(self, filename: str = "test_results.json"):
        """Save results to JSON file"""
        with open(filename, 'w') as f:
            json.dump(self.results, f, indent=2)

if __name__ == "__main__":
    # Test suite
    test_suite = [
        "BluetoothConnectionTest",
        "MeshNetworkFormationTest",
        "ConsensusProtocolTest",
        "BatteryUsageTest",
        "PerformanceTest"
    ]
    
    # Run tests
    runner = DeviceTestRunner()
    print(f"📱 Found {len(runner.devices['android'])} Android and {len(runner.devices['ios'])} iOS devices")
    
    runner.run_parallel_tests(test_suite)
    
    # Generate report
    report = runner.generate_report()
    with open("device_test_report.md", "w") as f:
        f.write(report)
    
    # Save JSON results
    runner.save_results()
    
    print("\n✅ Testing complete! Report saved to device_test_report.md")
```

---

## Performance Monitoring

### Battery Usage Tracking
```bash
#!/bin/bash
# monitor_battery.sh

DEVICE=$1
DURATION=${2:-3600}  # Default 1 hour
INTERVAL=60  # Check every minute

echo "Monitoring battery on $DEVICE for $DURATION seconds..."
echo "Time,Level,Temperature,Voltage" > battery_log.csv

START_TIME=$(date +%s)
END_TIME=$((START_TIME + DURATION))

while [ $(date +%s) -lt $END_TIME ]; do
    TIMESTAMP=$(date +%H:%M:%S)
    
    # Get battery stats
    BATTERY=$(adb -s $DEVICE shell dumpsys battery)
    LEVEL=$(echo "$BATTERY" | grep "level:" | cut -d: -f2 | tr -d ' ')
    TEMP=$(echo "$BATTERY" | grep "temperature:" | cut -d: -f2 | tr -d ' ')
    VOLTAGE=$(echo "$BATTERY" | grep "voltage:" | cut -d: -f2 | tr -d ' ')
    
    echo "$TIMESTAMP,$LEVEL,$TEMP,$VOLTAGE" >> battery_log.csv
    echo "[$TIMESTAMP] Battery: $LEVEL% | Temp: $((TEMP/10))°C | Voltage: ${VOLTAGE}mV"
    
    sleep $INTERVAL
done

echo "✅ Battery monitoring complete. Results in battery_log.csv"
```

### Memory Usage Tracking
```bash
#!/bin/bash
# monitor_memory.sh

DEVICE=$1
PACKAGE="com.bitcraps.app"

echo "Monitoring memory usage for $PACKAGE..."
echo "Time,PSS,PrivateDirty,SharedDirty" > memory_log.csv

while true; do
    TIMESTAMP=$(date +%H:%M:%S)
    
    # Get memory info
    MEMINFO=$(adb -s $DEVICE shell dumpsys meminfo $PACKAGE | grep "TOTAL")
    PSS=$(echo $MEMINFO | awk '{print $2}')
    PRIVATE=$(echo $MEMINFO | awk '{print $3}')
    SHARED=$(echo $MEMINFO | awk '{print $4}')
    
    echo "$TIMESTAMP,$PSS,$PRIVATE,$SHARED" >> memory_log.csv
    echo "[$TIMESTAMP] PSS: ${PSS}KB | Private: ${PRIVATE}KB | Shared: ${SHARED}KB"
    
    sleep 10
done
```

### Network Performance Monitoring
```python
#!/usr/bin/env python3
# monitor_network.py

import subprocess
import time
import statistics

def measure_ble_latency(device1, device2, iterations=100):
    """Measure BLE round-trip latency between two devices"""
    latencies = []
    
    for i in range(iterations):
        start = time.time()
        
        # Send ping via BLE (simplified - actual implementation would use test app)
        subprocess.run([
            'adb', '-s', device1, 'shell',
            'am', 'broadcast', '-a', 'com.bitcraps.PING',
            '--es', 'target', device2
        ])
        
        # Wait for pong
        result = subprocess.run([
            'adb', '-s', device1, 'shell',
            'logcat', '-d', '-s', 'BitCraps:I', '|', 'grep', 'PONG'
        ], capture_output=True, text=True)
        
        if 'PONG' in result.stdout:
            latency = (time.time() - start) * 1000  # Convert to ms
            latencies.append(latency)
            print(f"Ping {i+1}: {latency:.2f}ms")
    
    if latencies:
        print(f"\nLatency Statistics:")
        print(f"  Min: {min(latencies):.2f}ms")
        print(f"  Max: {max(latencies):.2f}ms")
        print(f"  Avg: {statistics.mean(latencies):.2f}ms")
        print(f"  P50: {statistics.median(latencies):.2f}ms")
        print(f"  P99: {statistics.quantiles(latencies, n=100)[98]:.2f}ms")
    
    return latencies

if __name__ == "__main__":
    import sys
    device1 = sys.argv[1]
    device2 = sys.argv[2]
    
    measure_ble_latency(device1, device2)
```

---

## Automated Test Scenarios

### 1. Connection Stability Test
```bash
# test_connection_stability.sh
#!/bin/bash

echo "Running 24-hour connection stability test..."

# Start app on all devices
for device in $(adb devices | grep device$ | cut -f1); do
    adb -s $device shell am start com.bitcraps.app/.MainActivity &
done

# Monitor for 24 hours
for i in {1..1440}; do  # 1440 minutes = 24 hours
    echo "Minute $i/1440"
    
    # Check connection status
    for device in $(adb devices | grep device$ | cut -f1); do
        STATUS=$(adb -s $device shell dumpsys activity | grep "mResumed=true")
        if [ -z "$STATUS" ]; then
            echo "⚠️ App crashed on $device at minute $i"
            # Restart app
            adb -s $device shell am start com.bitcraps.app/.MainActivity
        fi
    done
    
    sleep 60
done
```

### 2. Battery Drain Test
```bash
# test_battery_drain.sh
#!/bin/bash

echo "Running 4-hour battery drain test..."

# Record initial battery levels
for device in $(adb devices | grep device$ | cut -f1); do
    INITIAL=$(adb -s $device shell dumpsys battery | grep level | cut -d: -f2)
    echo "$device: Initial battery $INITIAL%"
done

# Run test for 4 hours
./monitor_battery.sh $1 14400 &

# Start heavy BLE activity
adb -s $1 shell am broadcast -a com.bitcraps.START_STRESS_TEST

sleep 14400

# Record final battery levels
for device in $(adb devices | grep device$ | cut -f1); do
    FINAL=$(adb -s $device shell dumpsys battery | grep level | cut -d: -f2)
    echo "$device: Final battery $FINAL%"
done
```

### 3. Cross-Platform Compatibility Test
```python
#!/usr/bin/env python3
# test_cross_platform.py

import itertools
import subprocess

def test_device_pair(android_device, ios_device):
    """Test Android-iOS interoperability"""
    print(f"Testing {android_device} <-> {ios_device}")
    
    # Start apps
    subprocess.run(['adb', '-s', android_device, 'shell', 'am', 'start', 'com.bitcraps.app/.MainActivity'])
    subprocess.run(['ios-deploy', '--id', ios_device, '--bundle', 'BitCraps.app', '--justlaunch'])
    
    # Wait for discovery
    time.sleep(10)
    
    # Check if devices discovered each other
    android_log = subprocess.run(
        ['adb', '-s', android_device, 'logcat', '-d', '|', 'grep', 'Peer discovered'],
        capture_output=True, text=True
    ).stdout
    
    return 'Peer discovered' in android_log

# Test all combinations
android_devices = ['device1', 'device2']
ios_devices = ['device3', 'device4']

for android, ios in itertools.product(android_devices, ios_devices):
    result = test_device_pair(android, ios)
    status = "✅" if result else "❌"
    print(f"{status} {android} <-> {ios}")
```

---

## Troubleshooting

### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| Device not recognized | Check USB debugging, reinstall drivers |
| App crashes on launch | Check permissions, clear app data |
| BLE not working | Reset Bluetooth, check airplane mode |
| High battery drain | Check background apps, disable unnecessary services |
| Connection timeouts | Increase timeout values, check interference |

### Debug Commands

```bash
# Android debugging
adb logcat -s BitCraps:V          # Verbose logs
adb shell dumpsys bluetooth_manager # Bluetooth state
adb shell dumpsys battery          # Battery stats
adb shell dumpsys meminfo          # Memory usage

# iOS debugging  
idevicesyslog -u <device_id>      # System logs
instruments -s devices             # List devices
xcrun simctl list                  # List simulators
```

---

## Maintenance

### Daily Tasks
- [ ] Check all devices are charged
- [ ] Clear old test data
- [ ] Update test apps
- [ ] Review overnight test results

### Weekly Tasks
- [ ] Clean device screens and ports
- [ ] Update device OS if available
- [ ] Backup test results
- [ ] Review and update test scenarios

### Monthly Tasks
- [ ] Factory reset underperforming devices
- [ ] Replace worn cables
- [ ] Update this documentation
- [ ] Review device matrix for new models

---

## Appendix: Device Specifications

### Bluetooth Capabilities by Device

| Device | BLE Version | Max Connections | Advertising | Roles |
|--------|-------------|-----------------|-------------|-------|
| Pixel 7 | 5.3 | 7 | Yes | Central & Peripheral |
| Samsung S22 | 5.2 | 5 | Yes | Central & Peripheral |
| iPhone 14 | 5.3 | 8 | Limited | Central & Peripheral |
| iPhone SE | 5.0 | 5 | Limited | Central & Peripheral |

### Performance Benchmarks

| Device | Discovery Time | Connection Time | Throughput |
|--------|---------------|-----------------|------------|
| Pixel 7 | <2s | <1s | 2 Mbps |
| Samsung S22 | <3s | <1.5s | 1.5 Mbps |
| iPhone 14 | <2s | <1s | 2 Mbps |
| iPhone SE | <4s | <2s | 1 Mbps |

---

*Document maintained by BitCraps Development Team*  
*Last Updated: 2025-08-24*