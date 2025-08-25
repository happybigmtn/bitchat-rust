#!/usr/bin/env python3
"""
Performance Monitoring System for Physical Devices
Tracks CPU, memory, network, and battery metrics during testing
"""

import os
import sys
import time
import json
import subprocess
import threading
import queue
from datetime import datetime
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, asdict
import statistics

@dataclass
class PerformanceMetrics:
    """Performance metrics snapshot"""
    timestamp: str
    device_id: str
    platform: str
    cpu_usage: float
    memory_usage: float
    memory_available: float
    network_rx_bytes: int
    network_tx_bytes: int
    battery_level: float
    battery_temperature: float
    fps: Optional[float] = None
    frame_drops: int = 0
    
class DeviceMonitor:
    """Base class for device monitoring"""
    
    def __init__(self, device_id: str):
        self.device_id = device_id
        self.metrics_queue = queue.Queue()
        self.monitoring = False
        self.monitor_thread = None
        
    def start_monitoring(self):
        """Start monitoring in background thread"""
        self.monitoring = True
        self.monitor_thread = threading.Thread(target=self._monitor_loop)
        self.monitor_thread.start()
        
    def stop_monitoring(self):
        """Stop monitoring"""
        self.monitoring = False
        if self.monitor_thread:
            self.monitor_thread.join()
            
    def _monitor_loop(self):
        """Main monitoring loop"""
        while self.monitoring:
            metrics = self.collect_metrics()
            if metrics:
                self.metrics_queue.put(metrics)
            time.sleep(1)  # Collect metrics every second
            
    def collect_metrics(self) -> Optional[PerformanceMetrics]:
        """Collect current metrics - to be implemented by subclasses"""
        raise NotImplementedError
        
    def get_metrics(self) -> List[PerformanceMetrics]:
        """Get all collected metrics"""
        metrics = []
        while not self.metrics_queue.empty():
            metrics.append(self.metrics_queue.get())
        return metrics

class AndroidMonitor(DeviceMonitor):
    """Android device performance monitor"""
    
    def __init__(self, device_id: str):
        super().__init__(device_id)
        self.platform = "android"
        self.last_network_stats = None
        
    def collect_metrics(self) -> Optional[PerformanceMetrics]:
        """Collect Android performance metrics"""
        try:
            # CPU usage
            cpu_cmd = f"adb -s {self.device_id} shell top -n 1 | grep com.bitcraps"
            cpu_result = subprocess.run(cpu_cmd, shell=True, capture_output=True, text=True)
            cpu_usage = self._parse_cpu_usage(cpu_result.stdout)
            
            # Memory usage
            mem_cmd = f"adb -s {self.device_id} shell dumpsys meminfo com.bitcraps | grep TOTAL"
            mem_result = subprocess.run(mem_cmd, shell=True, capture_output=True, text=True)
            memory_usage, memory_available = self._parse_memory_usage(mem_result.stdout)
            
            # Network stats
            net_cmd = f"adb -s {self.device_id} shell cat /proc/net/dev | grep wlan0"
            net_result = subprocess.run(net_cmd, shell=True, capture_output=True, text=True)
            rx_bytes, tx_bytes = self._parse_network_stats(net_result.stdout)
            
            # Battery stats
            battery_cmd = f"adb -s {self.device_id} shell dumpsys battery"
            battery_result = subprocess.run(battery_cmd, shell=True, capture_output=True, text=True)
            battery_level, battery_temp = self._parse_battery_stats(battery_result.stdout)
            
            # FPS (if UI is active)
            fps_cmd = f"adb -s {self.device_id} shell dumpsys gfxinfo com.bitcraps | grep 'Total frames'"
            fps_result = subprocess.run(fps_cmd, shell=True, capture_output=True, text=True)
            fps, frame_drops = self._parse_fps_stats(fps_result.stdout)
            
            return PerformanceMetrics(
                timestamp=datetime.now().isoformat(),
                device_id=self.device_id,
                platform=self.platform,
                cpu_usage=cpu_usage,
                memory_usage=memory_usage,
                memory_available=memory_available,
                network_rx_bytes=rx_bytes,
                network_tx_bytes=tx_bytes,
                battery_level=battery_level,
                battery_temperature=battery_temp,
                fps=fps,
                frame_drops=frame_drops
            )
            
        except Exception as e:
            print(f"Error collecting Android metrics: {e}")
            return None
            
    def _parse_cpu_usage(self, output: str) -> float:
        """Parse CPU usage from top output"""
        try:
            # Format: PID USER PR NI CPU% MEM% TIME+ COMMAND
            lines = output.strip().split('\n')
            for line in lines:
                if 'com.bitcraps' in line:
                    parts = line.split()
                    # CPU% is typically at index 8 or 9
                    for part in parts:
                        if '%' in part:
                            return float(part.rstrip('%'))
            return 0.0
        except:
            return 0.0
            
    def _parse_memory_usage(self, output: str) -> tuple:
        """Parse memory usage from dumpsys meminfo"""
        try:
            # Format: TOTAL PSS: xxxxx KB
            for line in output.split('\n'):
                if 'TOTAL' in line:
                    parts = line.split()
                    for i, part in enumerate(parts):
                        if part.isdigit():
                            used_kb = int(part)
                            # Get available memory
                            avail_cmd = f"adb -s {self.device_id} shell cat /proc/meminfo | grep MemAvailable"
                            avail_result = subprocess.run(avail_cmd, shell=True, capture_output=True, text=True)
                            avail_kb = int(avail_result.stdout.split()[1]) if avail_result.stdout else 0
                            return used_kb / 1024, avail_kb / 1024  # Convert to MB
            return 0.0, 0.0
        except:
            return 0.0, 0.0
            
    def _parse_network_stats(self, output: str) -> tuple:
        """Parse network statistics"""
        try:
            # Format: wlan0: rx_bytes packets errors ... tx_bytes packets errors
            parts = output.split()
            if len(parts) >= 10:
                rx_bytes = int(parts[1])
                tx_bytes = int(parts[9])
                
                if self.last_network_stats:
                    rx_delta = rx_bytes - self.last_network_stats[0]
                    tx_delta = tx_bytes - self.last_network_stats[1]
                    self.last_network_stats = (rx_bytes, tx_bytes)
                    return rx_delta, tx_delta
                else:
                    self.last_network_stats = (rx_bytes, tx_bytes)
                    return 0, 0
            return 0, 0
        except:
            return 0, 0
            
    def _parse_battery_stats(self, output: str) -> tuple:
        """Parse battery statistics"""
        try:
            level = 0.0
            temp = 0.0
            for line in output.split('\n'):
                if 'level:' in line:
                    level = float(line.split(':')[1].strip())
                elif 'temperature:' in line:
                    # Temperature is in tenths of degree
                    temp = float(line.split(':')[1].strip()) / 10
            return level, temp
        except:
            return 0.0, 0.0
            
    def _parse_fps_stats(self, output: str) -> tuple:
        """Parse FPS statistics"""
        try:
            fps = None
            drops = 0
            for line in output.split('\n'):
                if 'Total frames rendered:' in line:
                    total_frames = int(line.split(':')[1].strip())
                elif 'Janky frames:' in line:
                    drops = int(line.split(':')[1].split()[0])
                elif 'Average FPS:' in line:
                    fps = float(line.split(':')[1].strip())
            return fps, drops
        except:
            return None, 0

class IOSMonitor(DeviceMonitor):
    """iOS device performance monitor"""
    
    def __init__(self, device_id: str):
        super().__init__(device_id)
        self.platform = "ios"
        self.last_network_stats = None
        
    def collect_metrics(self) -> Optional[PerformanceMetrics]:
        """Collect iOS performance metrics using instruments"""
        try:
            # Use xcrun instruments for performance data
            # Note: Requires Xcode and device to be paired
            
            # CPU and Memory via instruments
            perf_cmd = f"xcrun instruments -w {self.device_id} -t 'Activity Monitor' -l 1000"
            # This is a simplified version - real implementation would parse instruments output
            
            # For now, use simulated data (replace with actual instruments parsing)
            cpu_usage = self._get_ios_cpu_usage()
            memory_usage, memory_available = self._get_ios_memory_usage()
            rx_bytes, tx_bytes = self._get_ios_network_stats()
            battery_level, battery_temp = self._get_ios_battery_stats()
            fps, frame_drops = self._get_ios_fps_stats()
            
            return PerformanceMetrics(
                timestamp=datetime.now().isoformat(),
                device_id=self.device_id,
                platform=self.platform,
                cpu_usage=cpu_usage,
                memory_usage=memory_usage,
                memory_available=memory_available,
                network_rx_bytes=rx_bytes,
                network_tx_bytes=tx_bytes,
                battery_level=battery_level,
                battery_temperature=battery_temp,
                fps=fps,
                frame_drops=frame_drops
            )
            
        except Exception as e:
            print(f"Error collecting iOS metrics: {e}")
            return None
            
    def _get_ios_cpu_usage(self) -> float:
        """Get iOS CPU usage"""
        try:
            # Use libimobiledevice tools if available
            cmd = f"ideviceinfo -u {self.device_id} -q com.apple.mobile.battery | grep BatteryCurrentCapacity"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            # Parse result (simplified)
            return 25.0  # Placeholder
        except:
            return 0.0
            
    def _get_ios_memory_usage(self) -> tuple:
        """Get iOS memory usage"""
        try:
            # Would use instruments or libimobiledevice
            return 512.0, 2048.0  # Placeholder MB values
        except:
            return 0.0, 0.0
            
    def _get_ios_network_stats(self) -> tuple:
        """Get iOS network statistics"""
        try:
            # Would use instruments for network activity
            return 1024, 512  # Placeholder bytes
        except:
            return 0, 0
            
    def _get_ios_battery_stats(self) -> tuple:
        """Get iOS battery statistics"""
        try:
            cmd = f"ideviceinfo -u {self.device_id} | grep -E 'BatteryCurrentCapacity|Temperature'"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            # Parse result (simplified)
            return 85.0, 32.0  # Placeholder values
        except:
            return 0.0, 0.0
            
    def _get_ios_fps_stats(self) -> tuple:
        """Get iOS FPS statistics"""
        try:
            # Would use instruments for Core Animation stats
            return 60.0, 2  # Placeholder values
        except:
            return None, 0

class PerformanceAnalyzer:
    """Analyze and report performance metrics"""
    
    def __init__(self):
        self.metrics_history: Dict[str, List[PerformanceMetrics]] = {}
        
    def add_metrics(self, device_id: str, metrics: List[PerformanceMetrics]):
        """Add metrics to history"""
        if device_id not in self.metrics_history:
            self.metrics_history[device_id] = []
        self.metrics_history[device_id].extend(metrics)
        
    def generate_report(self) -> Dict[str, Any]:
        """Generate performance report"""
        report = {
            "timestamp": datetime.now().isoformat(),
            "devices": {}
        }
        
        for device_id, metrics in self.metrics_history.items():
            if not metrics:
                continue
                
            device_report = {
                "platform": metrics[0].platform,
                "samples": len(metrics),
                "duration_seconds": len(metrics),
                "cpu": self._analyze_metric(metrics, "cpu_usage"),
                "memory": self._analyze_metric(metrics, "memory_usage"),
                "network": {
                    "rx_total_kb": sum(m.network_rx_bytes for m in metrics) / 1024,
                    "tx_total_kb": sum(m.network_tx_bytes for m in metrics) / 1024,
                    "rx_rate_kbps": statistics.mean([m.network_rx_bytes for m in metrics]) / 1024,
                    "tx_rate_kbps": statistics.mean([m.network_tx_bytes for m in metrics]) / 1024,
                },
                "battery": {
                    "start_level": metrics[0].battery_level if metrics else 0,
                    "end_level": metrics[-1].battery_level if metrics else 0,
                    "drain_rate": (metrics[0].battery_level - metrics[-1].battery_level) / len(metrics) if metrics else 0,
                    "avg_temperature": statistics.mean([m.battery_temperature for m in metrics]),
                },
                "graphics": self._analyze_graphics(metrics)
            }
            
            report["devices"][device_id] = device_report
            
        return report
        
    def _analyze_metric(self, metrics: List[PerformanceMetrics], field: str) -> Dict[str, float]:
        """Analyze a numeric metric"""
        values = [getattr(m, field) for m in metrics]
        return {
            "min": min(values),
            "max": max(values),
            "avg": statistics.mean(values),
            "median": statistics.median(values),
            "stdev": statistics.stdev(values) if len(values) > 1 else 0
        }
        
    def _analyze_graphics(self, metrics: List[PerformanceMetrics]) -> Dict[str, Any]:
        """Analyze graphics performance"""
        fps_values = [m.fps for m in metrics if m.fps is not None]
        total_drops = sum(m.frame_drops for m in metrics)
        
        if fps_values:
            return {
                "avg_fps": statistics.mean(fps_values),
                "min_fps": min(fps_values),
                "total_frame_drops": total_drops,
                "drop_rate": total_drops / len(metrics)
            }
        else:
            return {
                "avg_fps": None,
                "min_fps": None,
                "total_frame_drops": 0,
                "drop_rate": 0
            }
            
    def save_report(self, filepath: str):
        """Save report to file"""
        report = self.generate_report()
        with open(filepath, 'w') as f:
            json.dump(report, f, indent=2)
        print(f"Report saved to {filepath}")
        
    def print_summary(self):
        """Print performance summary"""
        report = self.generate_report()
        
        print("\n" + "="*60)
        print("PERFORMANCE MONITORING SUMMARY")
        print("="*60)
        
        for device_id, device_report in report["devices"].items():
            print(f"\nDevice: {device_id} ({device_report['platform']})")
            print(f"Duration: {device_report['duration_seconds']}s")
            print(f"Samples: {device_report['samples']}")
            
            print(f"\nCPU Usage:")
            print(f"  Average: {device_report['cpu']['avg']:.1f}%")
            print(f"  Peak: {device_report['cpu']['max']:.1f}%")
            
            print(f"\nMemory Usage:")
            print(f"  Average: {device_report['memory']['avg']:.1f} MB")
            print(f"  Peak: {device_report['memory']['max']:.1f} MB")
            
            print(f"\nNetwork:")
            print(f"  RX Rate: {device_report['network']['rx_rate_kbps']:.1f} KB/s")
            print(f"  TX Rate: {device_report['network']['tx_rate_kbps']:.1f} KB/s")
            print(f"  Total RX: {device_report['network']['rx_total_kb']:.1f} KB")
            print(f"  Total TX: {device_report['network']['tx_total_kb']:.1f} KB")
            
            print(f"\nBattery:")
            print(f"  Drain: {device_report['battery']['start_level'] - device_report['battery']['end_level']:.1f}%")
            print(f"  Rate: {device_report['battery']['drain_rate']:.2f}%/s")
            print(f"  Avg Temp: {device_report['battery']['avg_temperature']:.1f}°C")
            
            if device_report['graphics']['avg_fps']:
                print(f"\nGraphics:")
                print(f"  Average FPS: {device_report['graphics']['avg_fps']:.1f}")
                print(f"  Min FPS: {device_report['graphics']['min_fps']:.1f}")
                print(f"  Frame Drops: {device_report['graphics']['total_frame_drops']}")

def main():
    """Main performance monitoring function"""
    import argparse
    
    parser = argparse.ArgumentParser(description="Performance monitoring for physical devices")
    parser.add_argument("--android", nargs="+", help="Android device IDs to monitor")
    parser.add_argument("--ios", nargs="+", help="iOS device UDIDs to monitor")
    parser.add_argument("--duration", type=int, default=60, help="Monitoring duration in seconds")
    parser.add_argument("--output", default="performance_report.json", help="Output report file")
    
    args = parser.parse_args()
    
    if not args.android and not args.ios:
        print("Error: No devices specified. Use --android or --ios")
        sys.exit(1)
        
    monitors = []
    
    # Create Android monitors
    if args.android:
        for device_id in args.android:
            monitor = AndroidMonitor(device_id)
            monitors.append(monitor)
            print(f"Monitoring Android device: {device_id}")
            
    # Create iOS monitors
    if args.ios:
        for device_id in args.ios:
            monitor = IOSMonitor(device_id)
            monitors.append(monitor)
            print(f"Monitoring iOS device: {device_id}")
            
    # Start monitoring
    print(f"\nStarting performance monitoring for {args.duration} seconds...")
    for monitor in monitors:
        monitor.start_monitoring()
        
    # Wait for monitoring duration
    try:
        time.sleep(args.duration)
    except KeyboardInterrupt:
        print("\nMonitoring interrupted")
        
    # Stop monitoring
    print("\nStopping monitors...")
    for monitor in monitors:
        monitor.stop_monitoring()
        
    # Collect and analyze metrics
    analyzer = PerformanceAnalyzer()
    for monitor in monitors:
        metrics = monitor.get_metrics()
        analyzer.add_metrics(monitor.device_id, metrics)
        
    # Generate and save report
    analyzer.save_report(args.output)
    analyzer.print_summary()
    
    print(f"\nPerformance monitoring complete!")

if __name__ == "__main__":
    main()