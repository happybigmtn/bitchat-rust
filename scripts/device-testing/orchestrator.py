#!/usr/bin/env python3
"""
BitCraps Device Test Orchestrator

Manages parallel testing across multiple physical Android and iOS devices,
including BLE connectivity, performance monitoring, and battery tracking.
"""

import subprocess
import time
import json
import os
import sys
import threading
import queue
from datetime import datetime
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, asdict
from concurrent.futures import ThreadPoolExecutor, as_completed
import argparse
import logging

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

@dataclass
class Device:
    """Represents a test device"""
    id: str
    platform: str  # 'android' or 'ios'
    name: str
    os_version: str
    ble_version: str
    status: str = 'idle'
    battery_level: int = -1
    temperature: float = 0.0

@dataclass
class TestResult:
    """Represents a test result"""
    device_id: str
    test_name: str
    success: bool
    duration: float
    timestamp: str
    output: str
    metrics: Dict[str, any]

class DeviceManager:
    """Manages connected devices"""
    
    def __init__(self):
        self.devices: Dict[str, Device] = {}
        self.discover_devices()
    
    def discover_devices(self):
        """Discover all connected devices"""
        self._discover_android_devices()
        self._discover_ios_devices()
        logger.info(f"Discovered {len(self.devices)} devices")
    
    def _discover_android_devices(self):
        """Discover Android devices via ADB"""
        try:
            result = subprocess.run(
                ['adb', 'devices', '-l'],
                capture_output=True,
                text=True,
                timeout=5
            )
            
            for line in result.stdout.split('\n'):
                if 'device product:' in line:
                    parts = line.split()
                    device_id = parts[0]
                    
                    # Get device properties
                    props = self._get_android_properties(device_id)
                    
                    device = Device(
                        id=device_id,
                        platform='android',
                        name=props.get('model', 'Unknown'),
                        os_version=props.get('version', 'Unknown'),
                        ble_version=props.get('ble_version', 'Unknown')
                    )
                    
                    self.devices[device_id] = device
                    logger.info(f"Found Android device: {device.name} ({device_id})")
                    
        except subprocess.TimeoutExpired:
            logger.error("ADB command timed out")
        except Exception as e:
            logger.error(f"Error discovering Android devices: {e}")
    
    def _get_android_properties(self, device_id: str) -> Dict:
        """Get Android device properties"""
        props = {}
        
        try:
            # Get model
            result = subprocess.run(
                ['adb', '-s', device_id, 'shell', 'getprop', 'ro.product.model'],
                capture_output=True, text=True, timeout=5
            )
            props['model'] = result.stdout.strip()
            
            # Get Android version
            result = subprocess.run(
                ['adb', '-s', device_id, 'shell', 'getprop', 'ro.build.version.release'],
                capture_output=True, text=True, timeout=5
            )
            props['version'] = f"Android {result.stdout.strip()}"
            
            # Get Bluetooth version (simplified)
            props['ble_version'] = "5.0+"  # Would need more complex detection
            
        except Exception as e:
            logger.error(f"Error getting Android properties: {e}")
        
        return props
    
    def _discover_ios_devices(self):
        """Discover iOS devices"""
        try:
            result = subprocess.run(
                ['idevice_id', '-l'],
                capture_output=True,
                text=True,
                timeout=5
            )
            
            for device_id in result.stdout.strip().split('\n'):
                if device_id:
                    # Get device info
                    info = self._get_ios_info(device_id)
                    
                    device = Device(
                        id=device_id,
                        platform='ios',
                        name=info.get('DeviceName', 'Unknown'),
                        os_version=f"iOS {info.get('ProductVersion', 'Unknown')}",
                        ble_version="5.0+"  # iOS devices generally have BLE 5.0+
                    )
                    
                    self.devices[device_id] = device
                    logger.info(f"Found iOS device: {device.name} ({device_id})")
                    
        except FileNotFoundError:
            logger.warning("idevice_id not found - iOS testing unavailable")
        except Exception as e:
            logger.error(f"Error discovering iOS devices: {e}")
    
    def _get_ios_info(self, device_id: str) -> Dict:
        """Get iOS device information"""
        info = {}
        
        try:
            result = subprocess.run(
                ['ideviceinfo', '-u', device_id],
                capture_output=True, text=True, timeout=5
            )
            
            for line in result.stdout.split('\n'):
                if ':' in line:
                    key, value = line.split(':', 1)
                    info[key.strip()] = value.strip()
                    
        except Exception as e:
            logger.error(f"Error getting iOS info: {e}")
        
        return info
    
    def update_device_status(self, device_id: str):
        """Update device battery and status"""
        if device_id not in self.devices:
            return
        
        device = self.devices[device_id]
        
        if device.platform == 'android':
            self._update_android_status(device)
        elif device.platform == 'ios':
            self._update_ios_status(device)
    
    def _update_android_status(self, device: Device):
        """Update Android device status"""
        try:
            # Get battery info
            result = subprocess.run(
                ['adb', '-s', device.id, 'shell', 'dumpsys', 'battery'],
                capture_output=True, text=True, timeout=5
            )
            
            for line in result.stdout.split('\n'):
                if 'level:' in line:
                    device.battery_level = int(line.split(':')[1].strip())
                elif 'temperature:' in line:
                    temp = int(line.split(':')[1].strip())
                    device.temperature = temp / 10.0  # Convert to Celsius
                    
        except Exception as e:
            logger.error(f"Error updating Android status: {e}")
    
    def _update_ios_status(self, device: Device):
        """Update iOS device status"""
        try:
            result = subprocess.run(
                ['ideviceinfo', '-u', device.id, '-q', 'com.apple.mobile.battery'],
                capture_output=True, text=True, timeout=5
            )
            
            for line in result.stdout.split('\n'):
                if 'BatteryCurrentCapacity' in line:
                    device.battery_level = int(line.split(':')[1].strip())
                    
        except Exception as e:
            logger.error(f"Error updating iOS status: {e}")

class TestRunner:
    """Runs tests on devices"""
    
    def __init__(self, device_manager: DeviceManager):
        self.device_manager = device_manager
        self.results: List[TestResult] = []
        self.test_suite = self._load_test_suite()
    
    def _load_test_suite(self) -> List[Dict]:
        """Load test suite configuration"""
        return [
            {
                'name': 'BLEDiscoveryTest',
                'timeout': 30,
                'description': 'Test BLE device discovery'
            },
            {
                'name': 'ConnectionStabilityTest',
                'timeout': 60,
                'description': 'Test connection stability over time'
            },
            {
                'name': 'MeshFormationTest',
                'timeout': 120,
                'description': 'Test mesh network formation'
            },
            {
                'name': 'ConsensusTest',
                'timeout': 90,
                'description': 'Test consensus protocol'
            },
            {
                'name': 'BatteryDrainTest',
                'timeout': 300,
                'description': 'Test battery consumption'
            }
        ]
    
    def run_test_on_device(self, device: Device, test: Dict) -> TestResult:
        """Run a single test on a device"""
        logger.info(f"Running {test['name']} on {device.name}")
        
        start_time = time.time()
        timestamp = datetime.now().isoformat()
        
        # Update device status before test
        self.device_manager.update_device_status(device.id)
        initial_battery = device.battery_level
        
        if device.platform == 'android':
            success, output, metrics = self._run_android_test(device, test)
        else:
            success, output, metrics = self._run_ios_test(device, test)
        
        # Update device status after test
        self.device_manager.update_device_status(device.id)
        battery_drain = initial_battery - device.battery_level
        
        duration = time.time() - start_time
        
        # Add battery metrics
        metrics['battery_drain'] = battery_drain
        metrics['final_battery'] = device.battery_level
        metrics['temperature'] = device.temperature
        
        result = TestResult(
            device_id=device.id,
            test_name=test['name'],
            success=success,
            duration=duration,
            timestamp=timestamp,
            output=output,
            metrics=metrics
        )
        
        self.results.append(result)
        
        # Log result
        status = "✅ PASS" if success else "❌ FAIL"
        logger.info(f"{status} {device.name}: {test['name']} ({duration:.2f}s)")
        
        return result
    
    def _run_android_test(self, device: Device, test: Dict) -> Tuple[bool, str, Dict]:
        """Run test on Android device"""
        metrics = {}
        
        try:
            # Clear logcat
            subprocess.run(
                ['adb', '-s', device.id, 'logcat', '-c'],
                timeout=5
            )
            
            # Run instrumented test
            result = subprocess.run(
                ['adb', '-s', device.id, 'shell', 'am', 'instrument',
                 '-w', '-r', '-e', 'debug', 'false',
                 '-e', 'class', f'com.bitcraps.test.{test["name"]}',
                 'com.bitcraps.app.test/androidx.test.runner.AndroidJUnitRunner'],
                capture_output=True,
                text=True,
                timeout=test['timeout']
            )
            
            # Parse test output
            success = 'FAILURES!!!' not in result.stdout
            
            # Get performance metrics from logcat
            logcat = subprocess.run(
                ['adb', '-s', device.id, 'logcat', '-d', 'BitCraps:I', '*:S'],
                capture_output=True, text=True, timeout=5
            ).stdout
            
            # Parse metrics from logcat
            metrics = self._parse_android_metrics(logcat)
            
            return success, result.stdout, metrics
            
        except subprocess.TimeoutExpired:
            return False, "Test timed out", metrics
        except Exception as e:
            return False, str(e), metrics
    
    def _run_ios_test(self, device: Device, test: Dict) -> Tuple[bool, str, Dict]:
        """Run test on iOS device"""
        metrics = {}
        
        try:
            result = subprocess.run(
                ['xcodebuild', 'test',
                 '-project', 'ios/BitCraps.xcodeproj',
                 '-scheme', 'BitCraps',
                 '-destination', f'id={device.id}',
                 '-only-testing', f'BitCrapsTests/{test["name"]}'],
                capture_output=True,
                text=True,
                timeout=test['timeout']
            )
            
            success = 'TEST SUCCEEDED' in result.stdout
            
            # Parse metrics from output
            metrics = self._parse_ios_metrics(result.stdout)
            
            return success, result.stdout, metrics
            
        except subprocess.TimeoutExpired:
            return False, "Test timed out", metrics
        except Exception as e:
            return False, str(e), metrics
    
    def _parse_android_metrics(self, logcat: str) -> Dict:
        """Parse metrics from Android logcat"""
        metrics = {}
        
        for line in logcat.split('\n'):
            # Parse performance metrics
            if 'METRIC:' in line:
                try:
                    metric_str = line.split('METRIC:')[1].strip()
                    metric_parts = metric_str.split('=')
                    if len(metric_parts) == 2:
                        key = metric_parts[0].strip()
                        value = metric_parts[1].strip()
                        
                        # Try to convert to number
                        try:
                            if '.' in value:
                                metrics[key] = float(value)
                            else:
                                metrics[key] = int(value)
                        except ValueError:
                            metrics[key] = value
                            
                except Exception:
                    pass
        
        return metrics
    
    def _parse_ios_metrics(self, output: str) -> Dict:
        """Parse metrics from iOS test output"""
        metrics = {}
        
        # Parse test metrics from xcodebuild output
        for line in output.split('\n'):
            if 'measured' in line.lower():
                # Parse performance test results
                try:
                    if 'average:' in line:
                        parts = line.split('average:')
                        if len(parts) > 1:
                            value = parts[1].split()[0]
                            metrics['average'] = float(value)
                except Exception:
                    pass
        
        return metrics
    
    def run_parallel_tests(self, max_workers: int = 4):
        """Run tests in parallel across devices"""
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = []
            
            for test in self.test_suite:
                for device in self.device_manager.devices.values():
                    future = executor.submit(self.run_test_on_device, device, test)
                    futures.append((future, device, test))
            
            for future, device, test in futures:
                try:
                    result = future.result(timeout=test['timeout'] + 30)
                except Exception as e:
                    logger.error(f"Test {test['name']} failed on {device.name}: {e}")

class ReportGenerator:
    """Generates test reports"""
    
    def __init__(self, results: List[TestResult], devices: Dict[str, Device]):
        self.results = results
        self.devices = devices
    
    def generate_markdown_report(self) -> str:
        """Generate Markdown report"""
        report = ["# BitCraps Device Test Report\n"]
        report.append(f"**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
        report.append(f"**Devices Tested**: {len(self.devices)}\n")
        report.append(f"**Total Tests Run**: {len(self.results)}\n\n")
        
        # Device summary
        report.append("## Device Summary\n\n")
        report.append("| Device | Platform | OS Version | Battery Start | Battery End | Status |\n")
        report.append("|--------|----------|------------|---------------|-------------|--------|\n")
        
        for device in self.devices.values():
            # Calculate pass rate for this device
            device_results = [r for r in self.results if r.device_id == device.id]
            if device_results:
                pass_rate = sum(1 for r in device_results if r.success) / len(device_results) * 100
                status = f"{pass_rate:.0f}% pass"
            else:
                status = "No tests"
            
            report.append(f"| {device.name} | {device.platform} | {device.os_version} | "
                         f"{device.battery_level}% | {device.battery_level}% | {status} |\n")
        
        # Test summary
        report.append("\n## Test Summary\n\n")
        
        test_names = list(set(r.test_name for r in self.results))
        for test_name in test_names:
            test_results = [r for r in self.results if r.test_name == test_name]
            passed = sum(1 for r in test_results if r.success)
            total = len(test_results)
            
            report.append(f"### {test_name}\n")
            report.append(f"- **Pass Rate**: {passed}/{total} ({passed/total*100:.1f}%)\n")
            report.append(f"- **Average Duration**: {sum(r.duration for r in test_results)/total:.2f}s\n")
            
            # Metrics summary
            if test_results[0].metrics:
                report.append("- **Metrics**:\n")
                for key in test_results[0].metrics.keys():
                    values = [r.metrics.get(key, 0) for r in test_results if key in r.metrics]
                    if values and isinstance(values[0], (int, float)):
                        avg = sum(values) / len(values)
                        report.append(f"  - {key}: {avg:.2f} (avg)\n")
            
            report.append("\n")
        
        # Detailed results
        report.append("## Detailed Results\n\n")
        
        for device in self.devices.values():
            device_results = [r for r in self.results if r.device_id == device.id]
            if not device_results:
                continue
                
            report.append(f"### {device.name} ({device.platform})\n\n")
            
            for result in device_results:
                status = "✅" if result.success else "❌"
                report.append(f"- {status} **{result.test_name}** ({result.duration:.2f}s)\n")
                
                if not result.success and result.output:
                    # Include first few lines of error output
                    error_lines = result.output.split('\n')[:3]
                    for line in error_lines:
                        if line.strip():
                            report.append(f"  - {line.strip()}\n")
            
            report.append("\n")
        
        return ''.join(report)
    
    def generate_json_report(self) -> str:
        """Generate JSON report"""
        report_data = {
            'metadata': {
                'generated': datetime.now().isoformat(),
                'devices_tested': len(self.devices),
                'total_tests': len(self.results)
            },
            'devices': [asdict(d) for d in self.devices.values()],
            'results': [asdict(r) for r in self.results]
        }
        
        return json.dumps(report_data, indent=2)
    
    def save_reports(self, base_path: str = "test-results"):
        """Save reports to files"""
        os.makedirs(base_path, exist_ok=True)
        
        timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
        
        # Save markdown report
        md_path = os.path.join(base_path, f"report_{timestamp}.md")
        with open(md_path, 'w') as f:
            f.write(self.generate_markdown_report())
        logger.info(f"Markdown report saved to {md_path}")
        
        # Save JSON report
        json_path = os.path.join(base_path, f"report_{timestamp}.json")
        with open(json_path, 'w') as f:
            f.write(self.generate_json_report())
        logger.info(f"JSON report saved to {json_path}")

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description='BitCraps Device Test Orchestrator')
    parser.add_argument('--parallel', type=int, default=4, help='Number of parallel workers')
    parser.add_argument('--suite', type=str, help='Test suite JSON file')
    parser.add_argument('--output', type=str, default='test-results', help='Output directory')
    parser.add_argument('--verbose', action='store_true', help='Verbose logging')
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Initialize components
    logger.info("🚀 Starting BitCraps Device Test Orchestrator")
    
    device_manager = DeviceManager()
    
    if not device_manager.devices:
        logger.error("No devices found! Please connect devices and try again.")
        sys.exit(1)
    
    logger.info(f"Found {len(device_manager.devices)} devices")
    
    # Run tests
    test_runner = TestRunner(device_manager)
    
    logger.info("Running test suite...")
    test_runner.run_parallel_tests(max_workers=args.parallel)
    
    # Generate reports
    logger.info("Generating reports...")
    report_generator = ReportGenerator(test_runner.results, device_manager.devices)
    report_generator.save_reports(args.output)
    
    # Summary
    total_tests = len(test_runner.results)
    passed_tests = sum(1 for r in test_runner.results if r.success)
    
    logger.info(f"\n{'='*50}")
    logger.info(f"Test execution complete!")
    logger.info(f"Total tests: {total_tests}")
    logger.info(f"Passed: {passed_tests}")
    logger.info(f"Failed: {total_tests - passed_tests}")
    logger.info(f"Pass rate: {passed_tests/total_tests*100:.1f}%")
    logger.info(f"{'='*50}")

if __name__ == "__main__":
    main()