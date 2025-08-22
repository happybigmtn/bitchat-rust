# BitCraps Implementation Plan - Path to 100% Completion

## Current Status: 70% Complete, 0% Functional
The architecture is solid but critical components for actual gameplay are missing.

## Priority 1: Bluetooth Transport (CRITICAL - 0% → 40% Functional)

### 1.1 BLE Service Implementation
**File**: `src/transport/bluetooth.rs`
- [ ] Implement actual BLE peripheral setup using `btleplug`
- [ ] Create BitCraps service with UUID `12345678-1234-5678-1234-567812345678`
- [ ] Implement TX characteristic for sending packets
- [ ] Implement RX characteristic for receiving packets
- [ ] Handle characteristic notifications for real-time updates

### 1.2 Device Discovery & Connection
- [ ] Implement BLE scanning for BitCraps services
- [ ] Handle peripheral advertisement with peer ID in manufacturer data
- [ ] Implement connection handshake protocol
- [ ] Manage multiple simultaneous BLE connections
- [ ] Handle connection drops and reconnection

### 1.3 Packet Transmission
- [ ] Implement packet fragmentation for BLE MTU limits (512 bytes)
- [ ] Add packet reassembly on receive
- [ ] Implement reliable delivery with ACK/retry
- [ ] Add connection state management

**Estimated Lines**: 800-1000
**Complexity**: High
**Testing Required**: Physical devices or BLE simulator

## Priority 2: Peer Discovery (CRITICAL - 40% → 60% Functional)

### 2.1 Local Discovery Protocol
**File**: `src/discovery/bluetooth_discovery.rs`
- [ ] Implement peer announcement broadcasts
- [ ] Create peer registry with TTL management
- [ ] Add periodic peer list exchange
- [ ] Implement dead peer detection

### 2.2 Simple DHT Implementation
**File**: `src/transport/kademlia.rs`
- [ ] Replace stub with basic Kademlia routing
- [ ] Implement FIND_NODE operations
- [ ] Add STORE and FIND_VALUE
- [ ] Create routing table management

**Estimated Lines**: 400-500
**Complexity**: Medium
**Testing Required**: Multi-node simulation

## Priority 3: Terminal UI (CRITICAL - 60% → 85% Functional)

### 3.1 Game Display
**File**: `src/ui/tui/mod.rs`
- [ ] Implement main game screen layout
- [ ] Create dice roll animation
- [ ] Display betting table with current bets
- [ ] Show player balances and pot
- [ ] Add game phase indicators

### 3.2 Interactive Controls
**File**: `src/ui/tui/input.rs`
- [ ] Implement bet placement interface
- [ ] Add keyboard navigation for bet types
- [ ] Create amount input with validation
- [ ] Add game action buttons (roll, pass dice)

### 3.3 Network Status Display
**File**: `src/ui/tui/widgets.rs`
- [ ] Show connected peers list
- [ ] Display network health metrics
- [ ] Show mining rewards in real-time
- [ ] Add message relay indicators

**Estimated Lines**: 600-800
**Complexity**: Medium
**Testing Required**: Terminal emulator testing

## Priority 4: Mining Integration (85% → 95% Functional)

### 4.1 Proof-of-Relay Activation
**File**: `src/token/mod.rs`
- [ ] Connect message relay events to mining rewards
- [ ] Implement reward calculation based on relay distance
- [ ] Add reward distribution mechanism
- [ ] Create mining difficulty adjustment

### 4.2 Consensus Integration
- [ ] Wire up consensus for game state agreement
- [ ] Implement fork resolution for conflicting states
- [ ] Add transaction confirmation requirements

**Estimated Lines**: 200-300
**Complexity**: Low-Medium
**Testing Required**: Multi-node consensus testing

## Priority 5: Integration & Polish (95% → 100% Functional)

### 5.1 End-to-End Testing
- [ ] Create integration tests for full game flow
- [ ] Test Bluetooth connectivity between devices
- [ ] Verify token transfers and mining rewards
- [ ] Test network partition recovery

### 5.2 Production Hardening
- [ ] Add retry logic for failed operations
- [ ] Implement graceful shutdown
- [ ] Add data persistence across restarts
- [ ] Create backup/restore functionality

### 5.3 Documentation & Examples
- [ ] Write user guide for gameplay
- [ ] Create network setup instructions
- [ ] Add troubleshooting guide
- [ ] Include example configurations

**Estimated Lines**: 300-400
**Complexity**: Low
**Testing Required**: Full system testing

## Implementation Order & Dependencies

```
1. Bluetooth Transport (Week 1)
   ├── BLE Service Setup
   ├── Connection Management
   └── Packet Handling
   
2. Peer Discovery (Week 1-2)
   ├── Local Discovery
   └── Basic DHT
   
3. Terminal UI (Week 2)
   ├── Game Display
   ├── Input Handling
   └── Network Status
   
4. Mining Integration (Week 3)
   ├── Relay Rewards
   └── Consensus
   
5. Integration & Testing (Week 3-4)
   ├── End-to-End Tests
   ├── Hardening
   └── Documentation
```

## Success Criteria

### Minimum Viable Product (MVP)
- [ ] Two devices can discover each other via Bluetooth
- [ ] Players can create and join games
- [ ] Dice rolls work with proper randomness
- [ ] Bets are placed and resolved correctly
- [ ] CRAP tokens transfer between players
- [ ] Mining rewards are earned for relaying

### Full Product
- [ ] 5+ players can play simultaneously
- [ ] Network remains stable under load
- [ ] Games recover from disconnections
- [ ] UI provides smooth gameplay experience
- [ ] Mining provides meaningful incentives
- [ ] Treasury participates automatically

## Risk Factors

1. **BLE Platform Differences**: iOS/Android/Linux have different BLE APIs
2. **Testing Complexity**: Need multiple physical devices for realistic testing
3. **Performance**: BLE bandwidth limitations may affect gameplay
4. **Battery Usage**: Continuous BLE scanning/advertising drains battery

## Estimated Timeline

- **Week 1**: Bluetooth Transport + Basic Discovery (40% functional)
- **Week 2**: Complete Discovery + Terminal UI (70% functional)
- **Week 3**: Mining Integration + Initial Testing (90% functional)
- **Week 4**: Polish + Documentation + Full Testing (100% functional)

**Total New Code**: ~2,500-3,500 lines
**Total Effort**: 4 weeks for single developer, 1-2 weeks with parallel implementation

## Next Steps

1. Start with Bluetooth transport as it's the critical blocker
2. Implement basic discovery for peer-to-peer connectivity
3. Add minimal UI for user interaction
4. Wire up mining rewards
5. Test end-to-end gameplay
6. Polish and document

This plan transforms BitCraps from a well-architected but non-functional codebase into a fully working decentralized casino game over Bluetooth mesh networks.