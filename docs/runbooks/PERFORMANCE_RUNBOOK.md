# Performance Runbook
## BitCraps Production Performance Operations

*Version: 1.0 | Created: 2025-08-29*

---

## Executive Summary

This performance runbook provides comprehensive procedures for monitoring, diagnosing, and optimizing performance in the BitCraps decentralized casino platform. It covers network performance, consensus optimization, mobile performance tuning, and scalability management.

---

## 1. Performance Baseline and Targets

### 1.1 Core Performance Targets

#### Network Performance
- **Peer Discovery**: <10 seconds to find available peers
- **Connection Establishment**: <5 seconds for BLE connections
- **Message Propagation**: <500ms across local mesh network
- **Consensus Round Completion**: <2 seconds for 8-player games
- **Network Partition Recovery**: <30 seconds maximum

#### Application Performance  
- **Mobile App Launch**: <3 seconds cold start, <1 second warm start
- **Game Initiation**: <5 seconds from player matching to game start
- **Cryptographic Operations**: <100ms for signature verification
- **State Synchronization**: <200ms between mesh peers
- **Anti-Cheat Detection**: <50ms per validation check

#### Resource Utilization
- **Memory Usage**: <150MB baseline, <300MB peak during gameplay
- **CPU Usage**: <20% average, <50% peak during consensus
- **Battery Drain**: <5% per hour active gameplay
- **Network Bandwidth**: <1MB/hour per active game session
- **Storage Growth**: <10MB per month per active user

### 1.2 Performance SLA Definitions

#### Availability Targets
- **Network Uptime**: 99.5% (excluding planned maintenance)
- **Consensus Availability**: 99.9% (critical path)
- **Mobile App Responsiveness**: 99% of operations <5 seconds
- **BLE Mesh Stability**: 95% connection success rate

#### Performance Degradation Thresholds
**Warning Level (Yellow):**
- Response times 50% above baseline
- Resource usage 75% of maximum capacity
- Error rates >1% but <5%
- Connection success rate 85-95%

**Critical Level (Red):**
- Response times 100% above baseline  
- Resource usage >90% of maximum capacity
- Error rates >5%
- Connection success rate <85%

---

## 2. Performance Monitoring Infrastructure

### 2.1 Real-Time Performance Monitoring

#### Mobile Application Monitoring
**Key Performance Indicators (KPIs):**
```yaml
mobile_performance:
  app_launch_time:
    cold_start: <3000ms
    warm_start: <1000ms
    measurement_interval: continuous
    
  memory_usage:
    baseline: <150MB
    peak_limit: <300MB
    monitoring_frequency: 5s
    
  battery_consumption:
    target: <5% per hour
    measurement_window: 1h rolling
    
  cpu_utilization:
    average_target: <20%
    peak_limit: <50%
    sampling_rate: 1s
```

#### Network Performance Monitoring
**Mesh Network Metrics:**
```yaml
network_performance:
  peer_discovery:
    discovery_time: <10s
    success_rate: >95%
    retry_attempts: <3
    
  message_propagation:
    local_mesh_latency: <500ms
    hop_delay: <50ms per hop
    delivery_success_rate: >99%
    
  consensus_performance:
    round_completion: <2s
    validator_response: <500ms
    agreement_rate: >90%
```

#### System Resource Monitoring
**Real-Time Resource Tracking:**
- **CPU Usage**: Per-core utilization and temperature
- **Memory Usage**: Heap, stack, and system memory consumption
- **Disk I/O**: Read/write operations and queue depth
- **Network I/O**: Bytes transferred and connection counts
- **Battery Level**: Charge level and consumption rate (mobile)

### 2.2 Performance Data Collection

#### Metrics Collection Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Mobile Client  │    │  Gateway Node   │    │  Monitoring     │
│  (Local Metrics)│───▶│  (Aggregation)  │───▶│  Infrastructure │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ Local Storage   │    │  Time Series    │    │   Prometheus    │
│ (SQLite)        │    │  Database       │    │   + Grafana     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

#### Data Collection Procedures
**Local Data Collection (Mobile):**
1. Performance metrics sampled every 1-5 seconds
2. Local SQLite database for offline storage
3. Batch uploads when network available
4. Local retention: 7 days rolling window
5. Privacy-preserving anonymization before upload

**Centralized Aggregation:**
1. Gateway nodes collect from multiple mobile clients
2. Data aggregation and anonymization
3. Time-series database storage (InfluxDB/Prometheus)
4. Real-time alerting on threshold breaches
5. Historical trend analysis and reporting

### 2.3 Performance Alerting System

#### Alert Configuration
**Critical Performance Alerts:**
```yaml
# Consensus Performance Degradation
consensus_latency_alert:
  condition: consensus_round_time > 5s for 3 consecutive rounds
  action: immediate_investigation
  escalation: performance_team_page
  auto_mitigation: reduce_consensus_complexity

# Memory Leak Detection
memory_growth_alert:
  condition: memory_usage increasing >10MB per hour for 4 hours
  action: memory_profiling_activation
  escalation: development_team_alert
  auto_mitigation: garbage_collection_force

# Network Performance
network_partition_alert:
  condition: peer_connectivity < 50% for >60s
  action: network_topology_analysis
  escalation: network_team_alert
  auto_mitigation: peer_discovery_boost
```

**Performance Alert Response Matrix:**
| Alert Type | Response Time | Team | Auto-Mitigation | Escalation |
|------------|---------------|------|------------------|------------|
| Critical Path Latency | 15 minutes | Performance + Development | Yes | CTO if >1 hour |
| Resource Exhaustion | 30 minutes | Operations + Performance | Yes | VP Engineering if >2 hours |
| Network Degradation | 45 minutes | Network + Performance | Partial | Infrastructure Lead |
| Security Performance | 10 minutes | Security + Performance | No | CISO immediate |

---

## 3. Performance Troubleshooting Procedures

### 3.1 Network Performance Issues

#### Peer Discovery Problems
**Symptoms:**
- Slow game initiation (>10 seconds)
- Players unable to find games
- Network partition events

**Diagnostic Steps:**
1. **Check Local Network Status**
   ```bash
   # Verify BLE adapter status
   bluetoothctl show
   
   # Check peer discovery logs
   tail -f /var/log/bitcraps/peer_discovery.log
   
   # Monitor active connections
   netstat -an | grep :8080
   ```

2. **Analyze Network Topology**
   ```bash
   # Check mesh network health
   bitcraps-cli network status
   
   # Validate peer reputation scores
   bitcraps-cli network peers --verbose
   
   # Test direct peer connectivity
   bitcraps-cli network ping <peer_id>
   ```

3. **Performance Optimization Actions**
   - Increase peer discovery timeout
   - Adjust BLE advertisement intervals
   - Reset network topology cache
   - Force peer reputation recalculation

#### Consensus Performance Degradation
**Symptoms:**
- Game rounds taking >5 seconds
- Consensus failures increasing
- Byzantine tolerance threshold approached

**Diagnostic Procedure:**
1. **Consensus Metrics Analysis**
   ```bash
   # Check consensus round timing
   bitcraps-cli consensus metrics
   
   # Analyze validator response times
   bitcraps-cli consensus validators
   
   # Review recent consensus failures
   tail -f /var/log/bitcraps/consensus.log | grep ERROR
   ```

2. **Byzantine Behavior Detection**
   ```bash
   # Check for suspicious peer behavior
   bitcraps-cli anti-cheat status
   
   # Review peer reputation trends
   bitcraps-cli network reputation --history
   
   # Analyze voting patterns
   bitcraps-cli consensus votes --analyze
   ```

3. **Optimization Actions**
   - Reduce consensus timeout parameters
   - Temporarily exclude slow validators
   - Increase PoW difficulty for new peers
   - Enable fast-track consensus for simple operations

### 3.2 Mobile Performance Issues

#### Memory Performance Problems
**Symptoms:**
- App crashes with out-of-memory errors
- Degraded performance after extended use
- Memory usage >300MB

**Diagnostic Steps:**
1. **Memory Usage Analysis**
   ```bash
   # Mobile memory profiling
   adb shell dumpsys meminfo com.bitcraps.app
   
   # Rust memory usage
   bitcraps-cli debug memory-stats
   
   # Heap analysis
   jcmd <pid> GC.run_finalization
   ```

2. **Memory Leak Detection**
   - Enable memory tracking in development mode
   - Analyze object allocation patterns
   - Review garbage collection frequency
   - Check for circular references in async tasks

3. **Memory Optimization Actions**
   - Force garbage collection cycle
   - Clear consensus state cache
   - Reduce mesh network peer cache size
   - Restart memory-intensive components

#### CPU Performance Issues
**Symptoms:**
- High CPU usage (>50% sustained)
- Device overheating
- Battery drain >10% per hour

**Diagnostic Procedure:**
1. **CPU Profiling**
   ```bash
   # CPU usage by component
   bitcraps-cli debug cpu-profile
   
   # Thread analysis
   ps -T -p <bitcraps_pid>
   
   # System-wide CPU monitoring
   top -p <bitcraps_pid>
   ```

2. **Optimization Actions**
   - Reduce consensus validator participation
   - Lower BLE scan frequency
   - Implement adaptive processing based on battery level
   - Enable CPU throttling for non-critical operations

### 3.3 Consensus Performance Optimization

#### Lock-Free Consensus Tuning
**Configuration Parameters:**
```toml
[consensus.performance]
# Lock-free consensus engine settings
atomic_operations_batch_size = 100
lock_free_retry_limit = 1000
cas_backoff_microseconds = 10
memory_ordering = "acquire_release"

# Consensus timing optimization
round_timeout_ms = 2000
validator_timeout_ms = 500
agreement_threshold = 0.67
byzantine_tolerance_factor = 0.33
```

**Optimization Procedures:**
1. **Atomic Operations Tuning**
   - Monitor CAS (Compare-and-Swap) failure rates
   - Adjust backoff timing for reduced contention
   - Optimize memory ordering for consistency vs performance
   - Batch atomic operations where possible

2. **Consensus Algorithm Optimization**
   - Dynamic timeout adjustment based on network conditions
   - Validator selection optimization for minimal latency
   - State compression for faster synchronization
   - Merkle tree optimization for state verification

---

## 4. Database and Storage Optimization

### 4.1 SQLite Performance Tuning

#### Database Configuration
```sql
-- Performance optimization settings
PRAGMA cache_size = -64000; -- 64MB cache
PRAGMA journal_mode = WAL;   -- Write-Ahead Logging
PRAGMA synchronous = NORMAL; -- Balanced durability/performance
PRAGMA temp_store = MEMORY;  -- In-memory temporary storage
PRAGMA mmap_size = 268435456; -- 256MB memory-mapped I/O
```

#### Query Optimization
**Indexing Strategy:**
```sql
-- Game state indices for fast lookups
CREATE INDEX idx_game_state_timestamp ON game_states(timestamp);
CREATE INDEX idx_game_state_player ON game_states(player_id);
CREATE UNIQUE INDEX idx_consensus_round ON consensus_log(round_id, validator_id);

-- Composite indices for complex queries
CREATE INDEX idx_game_player_time ON games(player_id, start_time);
CREATE INDEX idx_consensus_state ON consensus_log(state_hash, timestamp);
```

**Query Performance Monitoring:**
```bash
# Enable query logging
PRAGMA optimize;
PRAGMA analysis_limit = 1000;

# Monitor slow queries
.timer on
.explain query plan SELECT * FROM games WHERE player_id = ?;
```

### 4.2 Storage Performance Optimization

#### Data Lifecycle Management
**Retention Policies:**
- Game history: 90 days active, 1 year archived
- Consensus logs: 30 days active, 6 months archived  
- Performance metrics: 7 days detailed, 30 days summarized
- Error logs: 14 days active, 90 days archived

**Archival Procedures:**
1. Automated daily archival of old data
2. Compression of archived data (gzip level 6)
3. Periodic cleanup of temporary files
4. Database VACUUM operations during maintenance windows

---

## 5. Network Optimization Strategies

### 5.1 BLE Mesh Optimization

#### Connection Management
**Optimal BLE Parameters:**
```toml
[ble.optimization]
# Connection intervals (in 1.25ms units)
connection_interval_min = 24  # 30ms
connection_interval_max = 40  # 50ms
connection_latency = 0        # No slave latency
supervision_timeout = 200     # 2 seconds

# Advertisement parameters
advertisement_interval_min = 100  # 62.5ms
advertisement_interval_max = 200  # 125ms
advertisement_timeout = 30        # 30 seconds
```

#### Mesh Network Topology Optimization
**Dynamic Topology Management:**
1. **Peer Selection Optimization**
   - Prefer peers with high reputation scores
   - Balance connection load across available peers
   - Maintain geographic diversity for resilience
   - Implement connection pooling for frequent peers

2. **Message Routing Optimization**
   - Implement intelligent flooding with TTL optimization
   - Cache routing tables for frequently contacted peers
   - Use reputation scores to prioritize routing paths
   - Implement message deduplication with bloom filters

### 5.2 Protocol Efficiency Improvements

#### Message Compression
**Compression Strategy:**
```rust
// Adaptive compression based on message size and type
match message.size() {
    0..=100 => CompressionType::None,           // Small messages
    101..=1000 => CompressionType::LZ4,         // Medium messages
    1001.. => CompressionType::Zstd(level: 3), // Large messages
}
```

#### Batch Processing
**Consensus Message Batching:**
- Combine multiple game actions into single consensus rounds
- Batch signature verifications for efficiency
- Group state updates to minimize synchronization overhead
- Pipeline consensus operations for higher throughput

---

## 6. Mobile Platform Optimization

### 6.1 Android Performance Optimization

#### Battery Optimization
**Power Management Strategies:**
```kotlin
// Adaptive scanning based on battery level
fun adjustScanFrequency(batteryLevel: Int) {
    val scanInterval = when (batteryLevel) {
        0..20 -> 30000    // 30 seconds - preserve battery
        21..50 -> 10000   // 10 seconds - balanced
        51..100 -> 5000   // 5 seconds - performance
        else -> 10000
    }
    bluetoothLeScanner.adjustScanInterval(scanInterval)
}
```

#### Memory Optimization
**JNI Memory Management:**
```rust
// Efficient memory management in JNI layer
#[no_mangle]
pub extern "C" fn Java_com_bitcraps_BitCrapsNative_processGameAction(
    env: JNIEnv,
    _: JClass,
    action: JByteArray,
) -> jbyteArray {
    // Use stack allocation for small data
    let action_bytes = env.convert_byte_array(action).unwrap();
    
    // Process with minimal heap allocation
    let result = process_game_action_efficient(&action_bytes);
    
    // Return result as JNI byte array
    env.byte_array_from_slice(&result).unwrap().into_inner()
}
```

### 6.2 iOS Performance Optimization

#### Background Processing Optimization
**Core Bluetooth Background Mode:**
```swift
// Optimize background BLE operations
func optimizeBackgroundPerformance() {
    // Reduce advertisement frequency in background
    if UIApplication.shared.applicationState == .background {
        centralManager.updateAdvertisementInterval(to: .extended)
        peripheralManager.reduceTransmitPower()
    }
}
```

#### Memory Pressure Handling
```swift
// Respond to memory pressure warnings
override func didReceiveMemoryWarning() {
    super.didReceiveMemoryWarning()
    
    // Clear non-essential caches
    BitCrapsManager.shared.clearCaches()
    
    // Reduce background activity
    BitCrapsManager.shared.enterLowMemoryMode()
}
```

---

## 7. Load Testing and Capacity Planning

### 7.1 Performance Load Testing

#### Load Testing Framework
**Test Scenarios:**
1. **Peer Discovery Load Test**
   - Simulate 100+ peers joining network simultaneously
   - Measure discovery time and success rate
   - Validate network topology convergence

2. **Consensus Stress Test**
   - Multiple concurrent games with maximum players
   - High-frequency betting actions
   - Byzantine peer behavior simulation

3. **Memory Stress Test**
   - Extended gameplay sessions (4+ hours)
   - Memory leak detection
   - Garbage collection impact analysis

#### Automated Load Testing
```bash
#!/bin/bash
# Automated performance regression testing

# Network load test
bitcraps-load-test --scenario peer_discovery --peers 100 --duration 300s

# Consensus performance test  
bitcraps-load-test --scenario consensus_stress --games 10 --players 8

# Memory endurance test
bitcraps-load-test --scenario memory_stress --duration 14400s --monitor memory
```

### 7.2 Capacity Planning

#### Growth Projection Models
**User Growth Modeling:**
```python
# Exponential growth model for network capacity planning
def calculate_network_capacity(
    current_users: int,
    growth_rate: float,
    time_horizon_months: int
) -> dict:
    
    projected_users = current_users * (1 + growth_rate) ** time_horizon_months
    
    # Network scaling requirements
    return {
        'projected_users': projected_users,
        'required_bootstrap_nodes': max(10, projected_users // 1000),
        'expected_concurrent_games': projected_users * 0.1,  # 10% concurrent play
        'bandwidth_requirements_mbps': projected_users * 0.01,
        'storage_growth_gb_per_month': projected_users * 0.1
    }
```

#### Resource Scaling Thresholds
**Auto-Scaling Triggers:**
```yaml
scaling_policies:
  bootstrap_nodes:
    scale_up_threshold: cpu_usage > 70% OR connection_count > 500
    scale_down_threshold: cpu_usage < 30% AND connection_count < 100
    
  gateway_nodes:
    scale_up_threshold: active_games > 50 OR network_latency > 1s
    scale_down_threshold: active_games < 10 AND network_latency < 200ms
    
  monitoring_infrastructure:
    scale_up_threshold: metrics_ingestion_rate > 10000/min
    scale_down_threshold: metrics_ingestion_rate < 1000/min
```

---

## 8. Performance Optimization Maintenance

### 8.1 Regular Performance Reviews

#### Weekly Performance Review Checklist
- [ ] Review key performance metrics trends
- [ ] Analyze performance alert frequency and causes
- [ ] Identify performance regression candidates from recent releases
- [ ] Update performance baselines if needed
- [ ] Review and prioritize performance optimization backlog

#### Monthly Performance Optimization Planning
- [ ] Comprehensive performance profiling across all components
- [ ] Capacity planning review and infrastructure scaling decisions
- [ ] Performance testing strategy updates
- [ ] Third-party dependency performance impact assessment
- [ ] Performance optimization ROI analysis

### 8.2 Performance Optimization Implementation

#### Continuous Performance Improvement Process
1. **Performance Issue Identification**
   - Automated detection through monitoring alerts
   - User feedback analysis and correlation
   - Proactive profiling and bottleneck analysis
   - Competitive benchmarking and comparison

2. **Root Cause Analysis**
   - Detailed performance profiling with multiple tools
   - Code review for performance anti-patterns
   - Infrastructure analysis for bottlenecks
   - User behavior analysis for usage patterns

3. **Optimization Implementation**
   - Performance improvement development and testing
   - A/B testing for optimization validation
   - Gradual rollout with performance monitoring
   - Rollback procedures if performance degrades

4. **Validation and Documentation**
   - Performance improvement measurement and validation
   - Baseline updates and target adjustments
   - Documentation updates for optimization techniques
   - Knowledge sharing with development team

---

## 9. Emergency Performance Procedures

### 9.1 Performance Emergency Response

#### Critical Performance Degradation Response
**Immediate Actions (0-15 minutes):**
1. Activate performance emergency response team
2. Implement immediate performance circuit breakers
3. Scale up infrastructure resources if auto-scaling available
4. Enable performance diagnostic logging
5. Notify users of potential service degradation

**Short-term Actions (15 minutes - 2 hours):**
1. Deep dive performance profiling and analysis
2. Implement temporary performance workarounds
3. Coordinate with development team for emergency fixes
4. Plan and execute emergency performance patches
5. Monitor user impact and satisfaction metrics

#### Performance Emergency Rollback Procedures
**Rollback Triggers:**
- Performance degradation >100% from baseline
- User complaints increasing >500% from normal
- Critical path operations failing >10% of the time
- System resources consistently >90% utilization

**Rollback Process:**
1. Immediate rollback to last known good configuration
2. Verify performance metrics return to acceptable levels
3. Communicate rollback status to stakeholders
4. Plan alternative optimization approach
5. Document lessons learned for future improvements

---

This Performance Runbook provides comprehensive procedures for maintaining optimal performance in the BitCraps production environment. Regular review and practice of these procedures ensures reliable, scalable, and performant service delivery to users.

**Document Control:**
- Review Cycle: Monthly for procedures, Quarterly for targets
- Owner: Performance Engineering Team
- Approval: Engineering Leadership and Operations Team
- Distribution: Engineering Team, Operations Team, DevOps Team

---

*Classification: Technical Operations - Internal Distribution Only*