# Chapter 90: BLE Peripheral Mode - Broadcasting Your Digital Presence

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending


## Introduction: The Lighthouse in Your Pocket

In 1859, the Eddystone Lighthouse was lit for the first time, broadcasting its presence to ships in the English Channel. It didn't chase after ships or demand attention - it simply announced "I am here" to anyone listening. Bluetooth Low Energy (BLE) peripherals work the same way. Your phone, your smartwatch, your BitCraps game node - they're all lighthouses in the electromagnetic spectrum, quietly broadcasting their presence to the digital world.

BLE Peripheral mode is the unsung hero of the Internet of Things. It's how your fitness tracker talks to your phone, how your smart lock recognizes you're approaching, and how BitCraps nodes discover each other in a crowded casino. Unlike classic Bluetooth's power-hungry constant connections, BLE peripherals can run for years on a coin cell battery by being smart about when and how they communicate.

This chapter explores the art of being a BLE peripheral - how to advertise efficiently, design GATT services that make sense, handle multiple connections gracefully, and navigate the quirks of different platforms. By the end, you'll understand how to make your device discoverable, connectable, and useful in the BLE ecosystem.

## The BLE Architecture: A Protocol Within a Protocol

BLE isn't just Bluetooth with less power - it's a completely different architecture:

### The Stack Layers
```
┌─────────────────────────────┐
│     Application Layer       │ ← Your BitCraps code
├─────────────────────────────┤
│         GATT/GAP           │ ← Service definitions
├─────────────────────────────┤
│          ATT/SM            │ ← Attribute protocol
├─────────────────────────────┤
│          L2CAP             │ ← Logical link control
├─────────────────────────────┤
│      Link Layer            │ ← Connection management
├─────────────────────────────┤
│    Physical Layer          │ ← Radio (2.4 GHz)
└─────────────────────────────┘
```

## Advertisement: Being Discoverable

Advertising is how peripherals announce their presence without maintaining connections:

```rust
use btleplug::api::{Peripheral as _, Manager as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use uuid::Uuid;
use std::time::Duration;

/// BLE Advertisement packet structure
#[derive(Debug, Clone)]
pub struct AdvertisementData {
    /// Local name (up to 29 bytes in advertisement)
    pub local_name: Option<String>,
    
    /// Service UUIDs to advertise
    pub service_uuids: Vec<Uuid>,
    
    /// Manufacturer specific data
    pub manufacturer_data: Option<ManufacturerData>,
    
    /// TX power level for range estimation
    pub tx_power_level: Option<i8>,
    
    /// Service data
    pub service_data: HashMap<Uuid, Vec<u8>>,
    
    /// Flags (BR/EDR, LE capabilities)
    pub flags: AdvertisementFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct AdvertisementFlags {
    pub le_limited_discoverable: bool,
    pub le_general_discoverable: bool,
    pub br_edr_not_supported: bool,
    pub le_br_edr_controller: bool,
    pub le_br_edr_host: bool,
}

impl AdvertisementData {
    pub fn new_ibeacon(uuid: Uuid, major: u16, minor: u16, power: i8) -> Self {
        // Apple iBeacon format
        let mut data = Vec::with_capacity(23);
        data.extend_from_slice(&[0x02, 0x15]); // iBeacon prefix
        data.extend_from_slice(uuid.as_bytes());
        data.extend_from_slice(&major.to_be_bytes());
        data.extend_from_slice(&minor.to_be_bytes());
        data.push(power as u8);
        
        Self {
            local_name: None,
            service_uuids: vec![],
            manufacturer_data: Some(ManufacturerData {
                company_id: 0x004C, // Apple Inc.
                data,
            }),
            tx_power_level: Some(power),
            service_data: HashMap::new(),
            flags: AdvertisementFlags {
                le_general_discoverable: true,
                br_edr_not_supported: true,
                ..Default::default()
            },
        }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut packet = Vec::new();
        
        // Flags
        packet.push(0x02); // Length
        packet.push(0x01); // Type: Flags
        packet.push(self.flags.to_byte());
        
        // TX Power
        if let Some(power) = self.tx_power_level {
            packet.push(0x02); // Length
            packet.push(0x0A); // Type: TX Power Level
            packet.push(power as u8);
        }
        
        // Local name (shortened if necessary)
        if let Some(ref name) = self.local_name {
            let name_bytes = name.as_bytes();
            let available_space = 31 - packet.len() - 2; // 31 byte limit
            
            if name_bytes.len() <= available_space {
                packet.push((name_bytes.len() + 1) as u8);
                packet.push(0x09); // Complete local name
                packet.extend_from_slice(name_bytes);
            } else {
                packet.push((available_space + 1) as u8);
                packet.push(0x08); // Shortened local name
                packet.extend_from_slice(&name_bytes[..available_space]);
            }
        }
        
        packet
    }
}
```

## GATT Services: The Peripheral's API

GATT (Generic Attribute Profile) defines how peripherals expose their capabilities:

```rust
/// BitCraps Game Service
pub struct GameService {
    service_uuid: Uuid,
    characteristics: Vec<Characteristic>,
}

impl GameService {
    pub fn new() -> Self {
        // BitCraps custom service UUID
        let service_uuid = Uuid::parse_str("12345678-1234-5678-1234-56789abcdef0").unwrap();
        
        Self {
            service_uuid,
            characteristics: vec![
                Self::game_state_characteristic(),
                Self::player_info_characteristic(),
                Self::bet_command_characteristic(),
                Self::dice_result_characteristic(),
                Self::chat_message_characteristic(),
            ],
        }
    }
    
    fn game_state_characteristic() -> Characteristic {
        Characteristic {
            uuid: Uuid::parse_str("12345678-1234-5678-1234-56789abcdef1").unwrap(),
            properties: CharacteristicProperties {
                read: true,
                write: false,
                notify: true,
                indicate: false,
                broadcast: false,
            },
            value: Arc::new(RwLock::new(Vec::new())),
            descriptors: vec![
                Descriptor::user_description("Current game state"),
                Descriptor::client_characteristic_configuration(),
            ],
            permissions: Permissions {
                read: SecurityLevel::None,
                write: SecurityLevel::None,
            },
        }
    }
    
    fn bet_command_characteristic() -> Characteristic {
        Characteristic {
            uuid: Uuid::parse_str("12345678-1234-5678-1234-56789abcdef3").unwrap(),
            properties: CharacteristicProperties {
                read: false,
                write: true,
                notify: false,
                indicate: false,
                broadcast: false,
            },
            value: Arc::new(RwLock::new(Vec::new())),
            descriptors: vec![
                Descriptor::user_description("Place bet command"),
            ],
            permissions: Permissions {
                read: SecurityLevel::None,
                write: SecurityLevel::Authenticated, // Requires pairing
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Characteristic {
    pub uuid: Uuid,
    pub properties: CharacteristicProperties,
    pub value: Arc<RwLock<Vec<u8>>>,
    pub descriptors: Vec<Descriptor>,
    pub permissions: Permissions,
}

#[derive(Debug, Clone, Copy)]
pub struct CharacteristicProperties {
    pub read: bool,
    pub write: bool,
    pub notify: bool,
    pub indicate: bool,
    pub broadcast: bool,
}

#[derive(Debug, Clone)]
pub enum SecurityLevel {
    None,
    Authenticated,
    AuthenticatedEncrypted,
    AuthenticatedSC, // Secure Connections
}
```

## Peripheral Implementation: Being a Good BLE Citizen

Here's how to implement a robust BLE peripheral:

```rust
use tokio::sync::broadcast;
use std::sync::Arc;

pub struct BlePeripheral {
    adapter: Adapter,
    services: Vec<Box<dyn GattService>>,
    connections: Arc<RwLock<HashMap<ConnectionId, Connection>>>,
    advertisement: AdvertisementData,
    config: PeripheralConfig,
    event_tx: broadcast::Sender<PeripheralEvent>,
}

#[derive(Debug, Clone)]
pub struct PeripheralConfig {
    pub device_name: String,
    pub appearance: u16, // GAP appearance value
    pub preferred_mtu: u16,
    pub connection_interval: ConnectionInterval,
    pub advertising_interval: Duration,
    pub max_connections: usize,
    pub security_requirements: SecurityRequirements,
}

impl BlePeripheral {
    pub async fn new(config: PeripheralConfig) -> Result<Self, BleError> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or(BleError::NoAdapter)?;
        
        // Initialize adapter
        adapter.power_on().await?;
        
        let (event_tx, _) = broadcast::channel(100);
        
        Ok(Self {
            adapter,
            services: Vec::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            advertisement: AdvertisementData::default(),
            config,
            event_tx,
        })
    }
    
    pub async fn add_service(&mut self, service: Box<dyn GattService>) {
        self.services.push(service);
    }
    
    pub async fn start_advertising(&mut self) -> Result<(), BleError> {
        // Build advertisement data
        self.advertisement.local_name = Some(self.config.device_name.clone());
        
        for service in &self.services {
            self.advertisement.service_uuids.push(service.uuid());
        }
        
        // Platform-specific advertising
        #[cfg(target_os = "linux")]
        self.start_advertising_bluez().await?;
        
        #[cfg(target_os = "macos")]
        self.start_advertising_corebluetooth().await?;
        
        #[cfg(target_os = "windows")]
        self.start_advertising_winrt().await?;
        
        // Start connection handler
        self.spawn_connection_handler();
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    async fn start_advertising_bluez(&mut self) -> Result<(), BleError> {
        use bluez_async::{BluetoothSession, AdvertisingManager};
        
        let session = BluetoothSession::new().await?;
        let adapter_path = format!("/org/bluez/hci0");
        
        let ad_manager = AdvertisingManager::new(&session, &adapter_path).await?;
        
        // Register advertisement
        let mut adv = Advertisement::new();
        adv.set_type(AdvertisementType::Peripheral);
        adv.set_local_name(&self.config.device_name);
        adv.set_appearance(self.config.appearance);
        
        for service in &self.services {
            adv.add_service_uuid(service.uuid());
        }
        
        // Set advertising interval (Linux bluez specific)
        adv.set_min_interval(self.config.advertising_interval.as_millis() as u16 / 625); // 0.625ms units
        adv.set_max_interval((self.config.advertising_interval.as_millis() as u16 / 625) + 10);
        
        ad_manager.register_advertisement(adv).await?;
        
        Ok(())
    }
    
    fn spawn_connection_handler(&self) {
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        let max_connections = self.config.max_connections;
        
        tokio::spawn(async move {
            loop {
                // Accept incoming connections
                match self.accept_connection().await {
                    Ok(connection) => {
                        let mut conns = connections.write().await;
                        
                        if conns.len() >= max_connections {
                            // Reject if at max capacity
                            connection.reject(DisconnectReason::ResourcesUnavailable);
                            continue;
                        }
                        
                        let conn_id = connection.id();
                        conns.insert(conn_id, connection.clone());
                        
                        // Notify about new connection
                        let _ = event_tx.send(PeripheralEvent::Connected {
                            connection_id: conn_id,
                            peer_address: connection.peer_address(),
                        });
                        
                        // Handle this connection
                        Self::handle_connection(connection, event_tx.clone()).await;
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });
    }
    
    async fn handle_connection(
        mut connection: Connection,
        event_tx: broadcast::Sender<PeripheralEvent>,
    ) {
        // Negotiate MTU
        let mtu = connection.negotiate_mtu().await.unwrap_or(23);
        
        loop {
            match connection.receive_request().await {
                Ok(request) => {
                    match request {
                        GattRequest::ReadCharacteristic { handle, offset } => {
                            let response = self.handle_read(handle, offset).await;
                            connection.send_response(response).await;
                        }
                        
                        GattRequest::WriteCharacteristic { handle, value } => {
                            let response = self.handle_write(handle, value).await;
                            connection.send_response(response).await;
                        }
                        
                        GattRequest::EnableNotifications { handle } => {
                            self.enable_notifications(connection.id(), handle).await;
                            connection.send_response(GattResponse::Success).await;
                        }
                        
                        _ => {
                            connection.send_response(
                                GattResponse::Error(AttError::RequestNotSupported)
                            ).await;
                        }
                    }
                }
                Err(e) => {
                    // Connection lost
                    let _ = event_tx.send(PeripheralEvent::Disconnected {
                        connection_id: connection.id(),
                        reason: e,
                    });
                    break;
                }
            }
        }
    }
}
```

## Notification and Indication: Push Updates to Centrals

Notifications let peripherals push data to connected centrals:

```rust
pub struct NotificationManager {
    subscriptions: Arc<RwLock<HashMap<CharacteristicHandle, Vec<ConnectionId>>>>,
    connections: Arc<RwLock<HashMap<ConnectionId, Connection>>>,
}

impl NotificationManager {
    pub async fn notify_value_change(
        &self,
        characteristic: CharacteristicHandle,
        value: &[u8],
    ) -> Result<(), BleError> {
        let subscriptions = self.subscriptions.read().await;
        
        if let Some(subscribers) = subscriptions.get(&characteristic) {
            let connections = self.connections.read().await;
            
            for conn_id in subscribers {
                if let Some(connection) = connections.get(conn_id) {
                    // Send notification (no acknowledgment required)
                    connection.send_notification(characteristic, value).await?;
                }
            }
        }
        
        Ok(())
    }
    
    pub async fn indicate_value_change(
        &self,
        characteristic: CharacteristicHandle,
        value: &[u8],
    ) -> Result<Vec<ConnectionId>, BleError> {
        let subscriptions = self.subscriptions.read().await;
        let mut confirmed = Vec::new();
        
        if let Some(subscribers) = subscriptions.get(&characteristic) {
            let connections = self.connections.read().await;
            
            for conn_id in subscribers {
                if let Some(connection) = connections.get(conn_id) {
                    // Send indication (requires acknowledgment)
                    match connection.send_indication(characteristic, value).await {
                        Ok(()) => confirmed.push(*conn_id),
                        Err(e) => {
                            warn!("Indication failed for {:?}: {}", conn_id, e);
                        }
                    }
                }
            }
        }
        
        Ok(confirmed)
    }
}

// Real-time game updates
impl GameService {
    pub async fn broadcast_game_update(&self, update: GameUpdate) {
        let encoded = self.encode_game_update(&update);
        
        // Notify all subscribed players
        self.notification_manager
            .notify_value_change(self.game_state_handle, &encoded)
            .await
            .ok();
    }
    
    fn encode_game_update(&self, update: &GameUpdate) -> Vec<u8> {
        // Efficient binary encoding for BLE (max 512 bytes with extended MTU)
        let mut buffer = Vec::new();
        
        buffer.push(update.event_type as u8);
        buffer.extend_from_slice(&update.timestamp.to_le_bytes());
        
        match &update.data {
            UpdateData::DiceRoll { player, result } => {
                buffer.extend_from_slice(&player.to_le_bytes());
                buffer.push(result.die1);
                buffer.push(result.die2);
            }
            UpdateData::BetPlaced { player, amount } => {
                buffer.extend_from_slice(&player.to_le_bytes());
                buffer.extend_from_slice(&amount.to_le_bytes());
            }
            UpdateData::GameWon { winner, payout } => {
                buffer.extend_from_slice(&winner.to_le_bytes());
                buffer.extend_from_slice(&payout.to_le_bytes());
            }
        }
        
        buffer
    }
}
```

## Security: Pairing and Bonding

BLE security protects against eavesdropping and impersonation:

```rust
pub struct SecurityManager {
    pairing_delegate: Box<dyn PairingDelegate>,
    bonded_devices: Arc<RwLock<HashMap<Address, BondInfo>>>,
    security_db: Arc<dyn SecurityDatabase>,
}

#[derive(Debug, Clone)]
pub struct BondInfo {
    pub address: Address,
    pub identity_key: [u8; 16],
    pub csrk: Option<[u8; 16]>, // Connection Signature Resolving Key
    pub ltk: Option<LongTermKey>,
    pub trust_level: TrustLevel,
}

pub trait PairingDelegate: Send + Sync {
    /// Called when pairing is initiated
    async fn on_pairing_request(&self, peer: Address) -> bool;
    
    /// Display passkey for user confirmation
    async fn display_passkey(&self, passkey: u32) -> Result<(), SecurityError>;
    
    /// Request passkey input from user
    async fn request_passkey(&self) -> Result<u32, SecurityError>;
    
    /// Numeric comparison (for Secure Connections)
    async fn confirm_numeric(&self, number: u32) -> Result<bool, SecurityError>;
}

impl SecurityManager {
    pub async fn handle_pairing(
        &self,
        connection: &mut Connection,
        initiator: bool,
    ) -> Result<BondInfo, SecurityError> {
        let peer = connection.peer_address();
        
        // Check if already bonded
        if let Some(bond) = self.get_bond(&peer).await {
            return Ok(bond);
        }
        
        // Request user confirmation
        if !self.pairing_delegate.on_pairing_request(peer).await {
            return Err(SecurityError::PairingRejected);
        }
        
        // Determine pairing method based on IO capabilities
        let pairing_method = self.determine_pairing_method(connection).await?;
        
        match pairing_method {
            PairingMethod::JustWorks => {
                // No user interaction needed (least secure)
                self.perform_just_works_pairing(connection).await
            }
            
            PairingMethod::PasskeyEntry => {
                // One device displays, other enters
                if initiator {
                    let passkey = rand::thread_rng().gen_range(0..1000000);
                    self.pairing_delegate.display_passkey(passkey).await?;
                    self.perform_passkey_pairing(connection, passkey).await
                } else {
                    let passkey = self.pairing_delegate.request_passkey().await?;
                    self.perform_passkey_pairing(connection, passkey).await
                }
            }
            
            PairingMethod::NumericComparison => {
                // Both devices display same number (Secure Connections)
                let number = self.generate_numeric_value(connection).await?;
                
                let local_confirm = self.pairing_delegate.confirm_numeric(number).await?;
                let remote_confirm = connection.exchange_numeric_confirmation(local_confirm).await?;
                
                if local_confirm && remote_confirm {
                    self.complete_secure_pairing(connection).await
                } else {
                    Err(SecurityError::NumericComparisonFailed)
                }
            }
            
            PairingMethod::OutOfBand => {
                // Use external channel (NFC, QR code, etc.)
                self.perform_oob_pairing(connection).await
            }
        }
    }
    
    async fn complete_secure_pairing(
        &self,
        connection: &mut Connection,
    ) -> Result<BondInfo, SecurityError> {
        // Generate and exchange keys
        let local_keys = self.generate_key_set();
        let remote_keys = connection.exchange_keys(local_keys).await?;
        
        // Create bond info
        let bond = BondInfo {
            address: connection.peer_address(),
            identity_key: remote_keys.irk,
            csrk: remote_keys.csrk,
            ltk: remote_keys.ltk,
            trust_level: TrustLevel::Authenticated,
        };
        
        // Store bond
        self.store_bond(&bond).await?;
        
        // Enable encryption
        connection.enable_encryption(&bond.ltk.unwrap()).await?;
        
        Ok(bond)
    }
}
```

## Power Management: Battery Life Optimization

BLE peripherals must be power-efficient:

```rust
pub struct PowerManager {
    current_mode: Arc<RwLock<PowerMode>>,
    battery_level: Arc<AtomicU8>,
    connection_params: Arc<RwLock<ConnectionParameters>>,
}

#[derive(Debug, Clone, Copy)]
pub enum PowerMode {
    HighPerformance, // Minimum latency, maximum power
    Balanced,        // Good performance, moderate power
    PowerSaving,     // Acceptable performance, minimum power
    UltraLowPower,   // Emergency mode, bare minimum functionality
}

#[derive(Debug, Clone)]
pub struct ConnectionParameters {
    pub interval_min: Duration, // 7.5ms to 4s
    pub interval_max: Duration,
    pub latency: u16,           // Number of events peripheral can skip
    pub timeout: Duration,      // Supervision timeout
}

impl PowerManager {
    pub async fn optimize_for_mode(&self, mode: PowerMode) {
        let params = match mode {
            PowerMode::HighPerformance => ConnectionParameters {
                interval_min: Duration::from_millis(7), // 7.5ms
                interval_max: Duration::from_millis(15),
                latency: 0, // Never skip events
                timeout: Duration::from_secs(1),
            },
            
            PowerMode::Balanced => ConnectionParameters {
                interval_min: Duration::from_millis(30),
                interval_max: Duration::from_millis(50),
                latency: 4, // Can skip 4 events
                timeout: Duration::from_secs(2),
            },
            
            PowerMode::PowerSaving => ConnectionParameters {
                interval_min: Duration::from_millis(100),
                interval_max: Duration::from_millis(200),
                latency: 10,
                timeout: Duration::from_secs(5),
            },
            
            PowerMode::UltraLowPower => ConnectionParameters {
                interval_min: Duration::from_millis(1000),
                interval_max: Duration::from_millis(2000),
                latency: 20,
                timeout: Duration::from_secs(10),
            },
        };
        
        *self.connection_params.write().await = params.clone();
        
        // Request connection parameter update for all connections
        self.update_all_connections(params).await;
        
        // Adjust advertising interval
        self.adjust_advertising_interval(mode).await;
    }
    
    pub async fn monitor_battery(&self) {
        loop {
            let battery_level = self.read_battery_level().await;
            self.battery_level.store(battery_level, Ordering::Relaxed);
            
            // Auto-adjust power mode based on battery
            if battery_level < 10 {
                self.optimize_for_mode(PowerMode::UltraLowPower).await;
            } else if battery_level < 30 {
                self.optimize_for_mode(PowerMode::PowerSaving).await;
            } else if battery_level < 60 {
                self.optimize_for_mode(PowerMode::Balanced).await;
            }
            
            // Update battery service characteristic
            self.update_battery_characteristic(battery_level).await;
            
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
```

## Multi-Connection Handling

Modern peripherals must handle multiple simultaneous connections:

```rust
pub struct MultiConnectionManager {
    connections: Arc<RwLock<HashMap<ConnectionId, ManagedConnection>>>,
    max_connections: usize,
    scheduler: Arc<ConnectionScheduler>,
}

struct ManagedConnection {
    connection: Connection,
    priority: ConnectionPriority,
    last_activity: Instant,
    data_queued: VecDeque<QueuedData>,
    stats: ConnectionStats,
}

impl MultiConnectionManager {
    pub async fn schedule_transmissions(&self) {
        loop {
            let connections = self.connections.read().await;
            
            // Group connections by priority
            let mut priority_groups: BTreeMap<ConnectionPriority, Vec<ConnectionId>> = 
                BTreeMap::new();
            
            for (id, conn) in connections.iter() {
                priority_groups.entry(conn.priority)
                    .or_default()
                    .push(*id);
            }
            
            drop(connections); // Release lock
            
            // Process in priority order
            for (_priority, conn_ids) in priority_groups.iter().rev() {
                for conn_id in conn_ids {
                    self.process_connection_queue(*conn_id).await;
                }
            }
            
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    
    pub async fn fair_bandwidth_allocation(&self) {
        let total_bandwidth = self.calculate_available_bandwidth();
        let connections = self.connections.read().await;
        
        // Calculate weights based on priority and activity
        let mut weights = Vec::new();
        let mut total_weight = 0.0;
        
        for conn in connections.values() {
            let weight = match conn.priority {
                ConnectionPriority::High => 3.0,
                ConnectionPriority::Normal => 2.0,
                ConnectionPriority::Low => 1.0,
            } * conn.activity_factor();
            
            weights.push(weight);
            total_weight += weight;
        }
        
        // Allocate bandwidth proportionally
        for (conn, weight) in connections.values().zip(weights.iter()) {
            let allocated_bandwidth = (total_bandwidth as f64 * weight / total_weight) as u32;
            conn.set_bandwidth_limit(allocated_bandwidth).await;
        }
    }
}
```

## Platform-Specific Quirks

Each platform has its own BLE peculiarities:

```rust
#[cfg(target_os = "ios")]
mod ios_specific {
    /// iOS Background Advertising Limitations:
    /// - Local name not included in background
    /// - Can only advertise service UUIDs
    /// - Advertising interval increased to save power
    pub fn configure_background_advertising() -> AdvertisementData {
        AdvertisementData {
            local_name: None, // Not supported in background
            service_uuids: vec![/* Your service UUIDs */],
            manufacturer_data: None, // Not supported
            tx_power_level: None,
            service_data: HashMap::new(),
            flags: Default::default(),
        }
    }
}

#[cfg(target_os = "android")]
mod android_specific {
    /// Android BLE Peripheral Limitations:
    /// - Requires BLUETOOTH_ADVERTISE permission (Android 12+)
    /// - Some devices don't support peripheral mode
    /// - Advertisement data limited to 31 bytes (no scan response)
    pub fn check_peripheral_support() -> bool {
        // Runtime check for peripheral mode support
        BluetoothAdapter::getDefaultAdapter()
            .isMultipleAdvertisementSupported()
    }
}

#[cfg(target_os = "windows")]
mod windows_specific {
    /// Windows BLE Limitations:
    /// - Requires Windows 10 version 1703+
    /// - Limited GATT server functionality
    /// - No background advertising without foreground app
    pub async fn request_radio_access() -> Result<(), WindowsError> {
        let access = Radio::RequestAccessAsync()?.await?;
        
        match access {
            RadioAccessStatus::Allowed => Ok(()),
            RadioAccessStatus::DeniedByUser => Err(WindowsError::UserDenied),
            RadioAccessStatus::DeniedBySystem => Err(WindowsError::SystemDenied),
            _ => Err(WindowsError::Unknown),
        }
    }
}
```

## Practical Exercises

### Exercise 1: Implement Custom GATT Service
Create a temperature sensor service:

```rust
pub struct TemperatureService {
    // Your implementation
}

impl TemperatureService {
    pub fn new() -> Self {
        // Your task: Create service with:
        // - Temperature measurement characteristic (notify)
        // - Temperature type characteristic (read)
        // - Measurement interval characteristic (read/write)
        todo!("Implement temperature service")
    }
}
```

### Exercise 2: Build Advertisement Scanner
Create a scanner that finds specific peripherals:

```rust
pub struct BleScanner {
    // Your implementation
}

impl BleScanner {
    pub async fn scan_for_service(&self, service_uuid: Uuid) -> Vec<Peripheral> {
        // Your task: Scan for peripherals advertising specific service
        // Filter by RSSI for proximity
        // Handle platform differences
        todo!("Implement targeted scanning")
    }
}
```

### Exercise 3: Connection State Machine
Implement robust connection management:

```rust
pub struct ConnectionStateMachine {
    // Your implementation
}

impl ConnectionStateMachine {
    pub async fn manage_connection(&mut self) {
        // Your task: Implement state machine with:
        // - Disconnected, Connecting, Connected, Disconnecting states
        // - Automatic reconnection
        // - Exponential backoff
        // - Connection parameter negotiation
        todo!("Implement connection state machine")
    }
}
```

## Common Pitfalls and Solutions

### 1. The 20-Byte Limit
Default ATT MTU is 23 bytes (20 for data):

```rust
// Bad: Trying to send large data in one packet
characteristic.set_value(&large_data); // Truncated!

// Good: Negotiate larger MTU or fragment
let mtu = connection.negotiate_mtu(512).await?;
if data.len() > mtu - 3 {
    // Fragment data
    for chunk in data.chunks(mtu - 3) {
        characteristic.send_chunk(chunk).await?;
    }
}
```

### 2. Advertisement Data Overflow
Advertisement limited to 31 bytes:

```rust
// Bad: Trying to fit everything
adv.local_name = "My Really Long Device Name That Won't Fit";
adv.add_service_uuid(uuid1);
adv.add_service_uuid(uuid2);
// ... Overflow!

// Good: Use scan response for additional data
adv.local_name = Some("BitCraps");
adv.scan_response.complete_name = "BitCraps Game Node #42";
```

### 3. iOS Background Limitations
iOS heavily restricts background BLE:

```rust
#[cfg(target_os = "ios")]
fn configure_for_ios_background() {
    // Must act as central to work in background
    // Or use iBeacon format
    // Or have active audio/location/voip session
}
```

## Conclusion: The Invisible Network

BLE Peripheral mode represents a fundamental shift in how devices communicate. Instead of maintaining constant connections, peripherals whisper their presence to the world, waiting for interested parties to connect. It's networking designed for a battery-powered, mobile-first world.

In BitCraps, BLE enables the dream of truly peer-to-peer gaming. No WiFi needed, no cellular required - just devices discovering and connecting to each other directly. A game can form spontaneously wherever people gather, limited only by radio range and imagination.

Key principles to remember:

1. **Advertise efficiently** - Every byte and millisecond costs battery
2. **Design services thoughtfully** - GATT structure is your API
3. **Handle connections gracefully** - Multiple centrals, varying capabilities
4. **Respect platform limits** - Each OS has its own restrictions
5. **Optimize for power** - Battery life determines usability

The next time your phone discovers a BitCraps game nearby, remember the elegant dance of advertisements and connections happening invisibly in the 2.4 GHz spectrum around you.

## Additional Resources

- **Bluetooth Core Specification 5.3** - The definitive reference
- **Getting Started with Bluetooth Low Energy** by Townsend, Cufí, Akiba, and Davidson
- **Bluetooth Developer Portal** - Official resources and tools
- **Nordic Semiconductor nRF Connect** - Excellent debugging tools

Remember: In BLE, less is more. The best peripheral is one users never think about - it just works.
