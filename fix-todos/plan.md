# TODO Fix Plan for BitCraps

## Summary
- **Total TODOs Found**: 25
- **Production Source TODOs**: 8
- **Test TODOs**: 1
- **Example TODOs**: 16 (documentation/demo - not fixing)

## Resolution Order (Priority-based)

### HIGH PRIORITY - Critical for Production

#### 1. Fix Type Mismatch in main.rs ❌
**Files**: `/src/main.rs:154, 178`
**Issue**: Type mismatch between `app_state::BitCrapsApp` and `bitcraps::BitCrapsApp`
**Resolution**: Unify the app types or create proper type conversion
**Risk**: Medium - May affect multiple modules
**Status**: PENDING

#### 2. Fix Monitoring Integration ❌
**File**: `/src/monitoring/integration.rs:14`
**Issue**: Using placeholder structs instead of actual app_state types
**Resolution**: Replace local structs with proper imports from app module
**Risk**: Low - Isolated to monitoring module
**Status**: PENDING

#### 3. Fix Integration Test Compilation ❌
**File**: `/tests/integration_test.rs:5`
**Issue**: Test compilation errors blocking CI/CD
**Resolution**: Update imports and fix async test handling
**Risk**: Low - Test-only changes
**Status**: PENDING

### MEDIUM PRIORITY - Security & Performance

#### 4. Add Packet Validation ❌
**File**: `/src/protocol/packet_utils.rs:224`
**Issue**: Missing validation before parsing packet data
**Resolution**: Add signature verification and parameter validation
**Risk**: Low - Additional safety checks
**Status**: PENDING

#### 5. Add Rate Limiting for Discovery ❌
**File**: `/src/protocol/packet_utils.rs:239`
**Issue**: Discovery requests not rate limited
**Resolution**: Implement per-peer rate limiting with cache
**Risk**: Low - DOS prevention enhancement
**Status**: PENDING

#### 6. Harden DoS Protection ❌
**File**: `/src/security/dos_protection.rs:39`
**Issue**: Default thresholds may not be production-ready
**Resolution**: Implement adaptive thresholds based on network conditions
**Risk**: Medium - May affect legitimate traffic
**Status**: PENDING

#### 7. Dynamic Quota Adjustment ❌
**File**: `/src/security/resource_quotas.rs:36`
**Issue**: Static quotas don't adapt to peer reputation
**Resolution**: Implement reputation scoring and dynamic adjustment
**Risk**: Medium - Complex implementation
**Status**: PENDING

### LOW PRIORITY - Optimizations

#### 8. Memory Pool Metrics ❌
**File**: `/src/memory_pool.rs:29`
**Issue**: Missing efficiency tracking metrics
**Resolution**: Add allocation latency and lifetime tracking
**Risk**: Low - Monitoring enhancement
**Status**: PENDING

## Implementation Strategy

1. Create git checkpoint before changes
2. Fix HIGH priority items first (type system issues)
3. Then MEDIUM priority (security enhancements)
4. Finally LOW priority (optimizations)
5. Run tests after each fix
6. Commit changes incrementally

## Notes
- Example TODOs (16 items) are documentation/demo related - not fixing
- Focus on production source code TODOs only
- Each fix will be tested independently