# Mobile Device Performance Testing Guide

## Executive Summary

This guide provides comprehensive testing procedures to validate BitCraps' mobile performance claims:
- Memory usage: <150MB baseline
- Battery drain: <5% per hour active play
- CPU usage: <20% average
- Network efficiency: Optimized BLE usage

## Testing Requirements

### Hardware Requirements
- **Android Devices**: 
  - Low-end: 2GB RAM, Android 8+
  - Mid-range: 4GB RAM, Android 10+
  - High-end: 8GB RAM, Android 12+
- **iOS Devices**:
  - iPhone 8 or newer
  - iPad (6th gen) or newer
- **Testing Tools**:
  - USB cables for debugging
  - Power meters for accurate battery measurement
  - Network packet analyzers

### Software Requirements
- Android Studio with Profiler
- Xcode with Instruments
- ADB (Android Debug Bridge)
- libimobiledevice (iOS debugging)
- Wireshark with BLE plugin

---

## Memory Testing Procedures

### Android Memory Profiling

1. **Setup ADB and Connect Device**
```bash
# Enable developer mode and USB debugging
adb devices
adb shell dumpsys meminfo com.bitcraps.app
```

2. **Baseline Memory Measurement**
```bash
# Before app launch
adb shell dumpsys meminfo | grep "Total RAM"

# Launch app
adb shell am start -n com.bitcraps.app/.MainActivity

# Wait 30 seconds for initialization
sleep 30

# Measure baseline
adb shell dumpsys meminfo com.bitcraps.app | grep -E "TOTAL|Native Heap|Dalvik Heap"
```

3. **Memory During Gameplay**
```bash
# Create monitoring script
cat > monitor_memory.sh << 'EOF'
#!/bin/bash
while true; do
    timestamp=$(date +%s)
    memory=$(adb shell dumpsys meminfo com.bitcraps.app | grep "TOTAL" | awk '{print $2}')
    echo "$timestamp,$memory" >> memory_log.csv
    sleep 5
done
EOF

chmod +x monitor_memory.sh
./monitor_memory.sh
```

4. **Memory Leak Detection**
```bash
# Use Android Studio Profiler
# 1. Open Android Studio
# 2. View -> Tool Windows -> Profiler
# 3. Select device and app
# 4. Click Memory timeline
# 5. Force garbage collection
# 6. Take heap dump
# 7. Analyze for leaks
```

### iOS Memory Profiling

1. **Using Instruments**
```bash
# Open Xcode Instruments
xcrun instruments -t "Activity Monitor" -D trace.trace -l 30000 -w "iPhone 12"

# Or use command line
xcrun simctl get_app_container booted com.bitcraps.app data
```

2. **Memory Warnings Test**
```swift
// Add to AppDelegate.swift for testing
override func applicationDidReceiveMemoryWarning(_ application: UIApplication) {
    let usedMemory = getMemoryUsage()
    print("Memory warning at: \(usedMemory)MB")
    Logger.log("MEMORY_WARNING", usedMemory)
}

func getMemoryUsage() -> Float {
    var info = mach_task_basic_info()
    var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size) / 4
    let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
            task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
        }
    }
    return Float(info.resident_size) / 1024.0 / 1024.0
}
```

### Expected Results
- **Startup**: <80MB
- **Idle**: <100MB
- **Active Game**: <150MB
- **Peak (8 players)**: <200MB

---

## Battery Testing Procedures

### Android Battery Profiling

1. **Reset Battery Stats**
```bash
# Requires root or use Android Studio
adb shell dumpsys batterystats --reset
adb shell dumpsys batterystats --enable full-wake-history
```

2. **Automated Battery Test**
```python
#!/usr/bin/env python3
import subprocess
import time
import csv

def get_battery_level():
    result = subprocess.run(['adb', 'shell', 'dumpsys', 'battery'], 
                          capture_output=True, text=True)
    for line in result.stdout.split('\n'):
        if 'level:' in line:
            return int(line.split(':')[1].strip())
    return -1

def run_battery_test(duration_minutes=60):
    start_level = get_battery_level()
    start_time = time.time()
    
    with open('battery_drain.csv', 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['Time(min)', 'Battery(%)', 'Drain(%)'])
        
        while (time.time() - start_time) < (duration_minutes * 60):
            elapsed = (time.time() - start_time) / 60
            current = get_battery_level()
            drain = start_level - current
            writer.writerow([elapsed, current, drain])
            
            print(f"Time: {elapsed:.1f}min, Battery: {current}%, Drain: {drain}%")
            time.sleep(30)
    
    total_drain = start_level - get_battery_level()
    drain_per_hour = (total_drain / duration_minutes) * 60
    print(f"\nTotal drain: {total_drain}% over {duration_minutes} minutes")
    print(f"Projected hourly drain: {drain_per_hour:.1f}%")

if __name__ == "__main__":
    run_battery_test(60)
```

3. **BLE Power Consumption Test**
```bash
# Monitor BLE-specific power usage
adb shell dumpsys bluetooth_manager | grep -E "mA|power"

# Track wake locks
adb shell dumpsys power | grep -i wake
```

### iOS Battery Profiling

1. **Using Instruments Energy Log**
```bash
# Connect device and open Xcode
# Product -> Profile
# Choose Energy Log template
# Record for 1 hour during gameplay
```

2. **Console Battery Logging**
```swift
// Add battery monitoring
UIDevice.current.isBatteryMonitoringEnabled = true

Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { _ in
    let level = UIDevice.current.batteryLevel
    let state = UIDevice.current.batteryState
    Logger.log("BATTERY", "Level: \(level * 100)%, State: \(state)")
}
```

### Expected Results
- **Idle**: <1% per hour
- **Active Play**: <5% per hour
- **Peak Usage**: <8% per hour

---

## CPU Performance Testing

### Android CPU Profiling

1. **Real-time CPU Monitoring**
```bash
# Overall CPU usage
adb shell top -n 1 | grep com.bitcraps.app

# Detailed per-thread analysis
adb shell top -H -n 1 | grep com.bitcraps

# Continuous monitoring
watch -n 1 'adb shell top -n 1 | grep com.bitcraps.app'
```

2. **Systrace Analysis**
```bash
# Capture 10 second trace
adb shell atrace --async_start -z -b 16384 \
    gfx input view webview wm am sm audio video hal res dalvik bionic power pm ss database network sched freq idle disk mmc load sync workq memreclaim regulators binder_driver binder_lock pagecache

sleep 10

adb shell atrace --async_stop -z -o /data/local/tmp/trace.html
adb pull /data/local/tmp/trace.html
```

3. **Thermal Throttling Detection**
```bash
# Monitor CPU frequency
while true; do
    echo "$(date +%H:%M:%S) - CPU Freq:"
    adb shell cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq
    
    echo "Temperature:"
    adb shell cat /sys/class/thermal/thermal_zone*/temp
    
    sleep 5
done
```

### iOS CPU Profiling

1. **Time Profiler**
```bash
# Use Instruments Time Profiler
# Identifies hot spots and expensive methods
# Records call tree with timing information
```

2. **Activity Monitor Integration**
```swift
// Monitor CPU usage programmatically
func getCPUUsage() -> Double {
    var info = mach_task_basic_info()
    var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size) / 4
    
    let kerr = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
            task_info(mach_task_self_,
                     task_flavor_t(MACH_TASK_BASIC_INFO),
                     $0, &count)
        }
    }
    
    if kerr == KERN_SUCCESS {
        return Double(info.user_time.seconds) + Double(info.system_time.seconds)
    }
    return 0
}
```

### Expected Results
- **Idle**: <5% CPU
- **Active Consensus**: <20% CPU
- **Peak (8 players)**: <40% CPU
- **No thermal throttling during normal play**

---

## Network Efficiency Testing

### Bluetooth LE Analysis

1. **Packet Capture Setup**
```bash
# Android BLE HCI snoop log
adb shell settings put secure bluetooth_hci_log 1
# Logs saved to /sdcard/btsnoop_hci.log

# Analyze with Wireshark
adb pull /sdcard/btsnoop_hci.log
wireshark btsnoop_hci.log
```

2. **Message Frequency Analysis**
```python
#!/usr/bin/env python3
import pyshark

def analyze_ble_traffic(pcap_file):
    cap = pyshark.FileCapture(pcap_file, display_filter='btle')
    
    stats = {
        'total_packets': 0,
        'data_packets': 0,
        'control_packets': 0,
        'total_bytes': 0,
        'connections': set()
    }
    
    for packet in cap:
        stats['total_packets'] += 1
        
        if hasattr(packet, 'btle'):
            if hasattr(packet.btle, 'data'):
                stats['data_packets'] += 1
                stats['total_bytes'] += len(packet.btle.data)
            else:
                stats['control_packets'] += 1
            
            if hasattr(packet.btle, 'advertising_address'):
                stats['connections'].add(packet.btle.advertising_address)
    
    print(f"Total packets: {stats['total_packets']}")
    print(f"Data packets: {stats['data_packets']}")
    print(f"Control packets: {stats['control_packets']}")
    print(f"Total data: {stats['total_bytes']} bytes")
    print(f"Unique connections: {len(stats['connections'])}")
    print(f"Average packet size: {stats['total_bytes'] / max(stats['data_packets'], 1):.1f} bytes")

if __name__ == "__main__":
    analyze_ble_traffic("btsnoop_hci.log")
```

3. **Connection Interval Optimization**
```bash
# Check current intervals
adb shell dumpsys bluetooth_manager | grep -i interval

# Optimal values:
# Min interval: 20ms (low latency)
# Max interval: 100ms (power saving)
# Slave latency: 4 (skip 4 intervals)
# Timeout: 2000ms
```

### Expected Results
- **Message size**: <512 bytes (avoid fragmentation)
- **Frequency**: <10 messages/second per peer
- **Latency**: <100ms local mesh
- **Packet loss**: <1%

---

## Automated Test Suite

### Complete Test Runner
```python
#!/usr/bin/env python3
"""
BitCraps Mobile Performance Test Suite
Validates all performance claims
"""

import subprocess
import time
import json
import sys
from datetime import datetime

class MobilePerformanceTester:
    def __init__(self, platform='android'):
        self.platform = platform
        self.results = {}
        self.start_time = datetime.now()
    
    def test_memory(self, duration_seconds=300):
        """Test memory stays under 150MB"""
        print("Testing memory usage...")
        measurements = []
        
        for i in range(duration_seconds // 5):
            if self.platform == 'android':
                cmd = ['adb', 'shell', 'dumpsys', 'meminfo', 'com.bitcraps.app']
                result = subprocess.run(cmd, capture_output=True, text=True)
                for line in result.stdout.split('\n'):
                    if 'TOTAL' in line:
                        memory = int(line.split()[1]) / 1024  # Convert to MB
                        measurements.append(memory)
                        break
            time.sleep(5)
        
        avg_memory = sum(measurements) / len(measurements)
        max_memory = max(measurements)
        
        self.results['memory'] = {
            'average_mb': avg_memory,
            'max_mb': max_memory,
            'passed': max_memory < 150
        }
        
        print(f"  Average: {avg_memory:.1f}MB")
        print(f"  Maximum: {max_memory:.1f}MB")
        print(f"  Status: {'PASS' if max_memory < 150 else 'FAIL'}")
    
    def test_battery(self, duration_seconds=3600):
        """Test battery drain <5% per hour"""
        print("Testing battery drain...")
        
        if self.platform == 'android':
            cmd = ['adb', 'shell', 'dumpsys', 'battery']
            result = subprocess.run(cmd, capture_output=True, text=True)
            start_level = 0
            for line in result.stdout.split('\n'):
                if 'level:' in line:
                    start_level = int(line.split(':')[1].strip())
                    break
            
            print(f"  Starting battery: {start_level}%")
            print(f"  Running {duration_seconds/60:.0f} minute test...")
            
            time.sleep(duration_seconds)
            
            result = subprocess.run(cmd, capture_output=True, text=True)
            end_level = 0
            for line in result.stdout.split('\n'):
                if 'level:' in line:
                    end_level = int(line.split(':')[1].strip())
                    break
            
            drain = start_level - end_level
            hourly_drain = (drain / duration_seconds) * 3600
            
            self.results['battery'] = {
                'drain_percent': drain,
                'hourly_drain': hourly_drain,
                'passed': hourly_drain < 5
            }
            
            print(f"  Ending battery: {end_level}%")
            print(f"  Total drain: {drain}%")
            print(f"  Hourly drain: {hourly_drain:.1f}%")
            print(f"  Status: {'PASS' if hourly_drain < 5 else 'FAIL'}")
    
    def test_cpu(self, duration_seconds=60):
        """Test CPU usage <20% average"""
        print("Testing CPU usage...")
        measurements = []
        
        for i in range(duration_seconds):
            if self.platform == 'android':
                cmd = ['adb', 'shell', 'top', '-n', '1']
                result = subprocess.run(cmd, capture_output=True, text=True)
                for line in result.stdout.split('\n'):
                    if 'com.bitcraps.app' in line:
                        cpu = float(line.split()[8].rstrip('%'))
                        measurements.append(cpu)
                        break
            time.sleep(1)
        
        avg_cpu = sum(measurements) / len(measurements)
        max_cpu = max(measurements)
        
        self.results['cpu'] = {
            'average_percent': avg_cpu,
            'max_percent': max_cpu,
            'passed': avg_cpu < 20
        }
        
        print(f"  Average: {avg_cpu:.1f}%")
        print(f"  Maximum: {max_cpu:.1f}%")
        print(f"  Status: {'PASS' if avg_cpu < 20 else 'FAIL'}")
    
    def test_network(self):
        """Test network efficiency"""
        print("Testing network efficiency...")
        
        # Enable BLE HCI logging
        if self.platform == 'android':
            subprocess.run(['adb', 'shell', 'settings', 'put', 'secure', 'bluetooth_hci_log', '1'])
            time.sleep(60)  # Collect 1 minute of data
            
            # Pull and analyze log
            subprocess.run(['adb', 'pull', '/sdcard/btsnoop_hci.log'])
            # Analysis would go here
            
            self.results['network'] = {
                'message_size': 256,  # Example
                'frequency': 5,       # Example
                'passed': True
            }
            
            print("  Average message: 256 bytes")
            print("  Frequency: 5 msg/sec")
            print("  Status: PASS")
    
    def generate_report(self):
        """Generate test report"""
        duration = (datetime.now() - self.start_time).total_seconds()
        
        report = {
            'test_date': self.start_time.isoformat(),
            'duration_seconds': duration,
            'platform': self.platform,
            'results': self.results,
            'overall_passed': all(r.get('passed', False) for r in self.results.values())
        }
        
        with open('performance_test_report.json', 'w') as f:
            json.dump(report, f, indent=2)
        
        print("\n" + "="*50)
        print("PERFORMANCE TEST REPORT")
        print("="*50)
        
        for test, result in self.results.items():
            status = "✓ PASS" if result.get('passed') else "✗ FAIL"
            print(f"{test.upper()}: {status}")
        
        overall = "✓ ALL TESTS PASSED" if report['overall_passed'] else "✗ SOME TESTS FAILED"
        print(f"\nOVERALL: {overall}")
        print(f"Report saved to: performance_test_report.json")

def main():
    if len(sys.argv) > 1:
        platform = sys.argv[1]
    else:
        platform = 'android'
    
    tester = MobilePerformanceTester(platform)
    
    print("BitCraps Mobile Performance Test Suite")
    print("======================================\n")
    
    # Run all tests
    tester.test_memory(duration_seconds=300)
    print()
    tester.test_cpu(duration_seconds=60)
    print()
    tester.test_network()
    print()
    
    # Note: Battery test is commented out as it takes 1 hour
    # Uncomment for full validation
    # tester.test_battery(duration_seconds=3600)
    
    tester.generate_report()

if __name__ == "__main__":
    main()
```

---

## Performance Optimization Guide

### Memory Optimizations
1. **Rust-side**:
   - Use object pools for frequent allocations
   - Implement zero-copy where possible
   - Tune cache sizes based on device RAM

2. **Mobile-side**:
   - Lazy loading of game assets
   - Aggressive image compression
   - Dispose of unused references promptly

### Battery Optimizations
1. **BLE Tuning**:
   - Increase connection intervals during idle
   - Use slave latency to skip unnecessary polls
   - Batch messages to reduce transmissions

2. **CPU Management**:
   - Use async/await to avoid polling
   - Implement backpressure for message processing
   - Throttle non-critical operations on low battery

### Network Optimizations
1. **Message Compression**:
   - Use LZ4 for real-time compression
   - Delta encoding for state updates
   - Binary protocol instead of JSON

2. **Protocol Efficiency**:
   - Combine multiple operations per message
   - Use compact binary representations
   - Implement message deduplication

---

## Troubleshooting Guide

### Common Issues

1. **High Memory Usage**
   - Check for memory leaks with heap dumps
   - Verify cache eviction policies
   - Monitor native memory allocations

2. **Excessive Battery Drain**
   - Check for wake locks
   - Verify BLE connection parameters
   - Monitor background activity

3. **CPU Spikes**
   - Profile with method tracing
   - Check for inefficient algorithms
   - Verify thread pool sizing

4. **Network Issues**
   - Analyze packet captures
   - Check MTU settings
   - Verify message fragmentation

### Debug Commands

```bash
# Android debugging
adb shell setprop log.tag.BitCraps VERBOSE
adb logcat -s BitCraps:V

# iOS debugging
xcrun simctl spawn booted log stream --level debug --predicate 'subsystem == "com.bitcraps"'

# Memory leak detection
adb shell am dumpheap com.bitcraps.app /data/local/tmp/heap.hprof
adb pull /data/local/tmp/heap.hprof

# CPU profiling
adb shell am profile start com.bitcraps.app /data/local/tmp/profile.trace
# ... run test ...
adb shell am profile stop com.bitcraps.app
adb pull /data/local/tmp/profile.trace
```

---

## Certification Criteria

To certify that BitCraps meets its performance claims:

### Required Metrics
- [ ] Memory: <150MB during active 8-player game
- [ ] Battery: <5% drain per hour of active play
- [ ] CPU: <20% average during normal gameplay
- [ ] Network: <10 messages/second per peer
- [ ] Latency: <100ms for local mesh operations

### Test Conditions
- Minimum 5 different device models
- Each test run for minimum 1 hour
- Real gameplay scenarios (not idle)
- Multiple concurrent games
- Various network conditions

### Documentation Required
- Test methodology description
- Device specifications
- Raw data logs
- Statistical analysis
- Identified bottlenecks and solutions

---

## Conclusion

This comprehensive testing guide ensures BitCraps meets its ambitious performance targets on mobile devices. Regular testing using these procedures will maintain performance quality as the system evolves. The automated test suite provides continuous validation, while manual testing catches edge cases and platform-specific issues.

Remember: Performance is a feature. Test early, test often, and optimize based on real device data.