# BitCraps Mobile Integration Guide

A comprehensive guide for integrating BitCraps peer-to-peer gaming functionality into Android and iOS applications.

## Table of Contents

- [Overview](#overview)
- [Getting Started](#getting-started)
- [Android Integration](#android-integration)
- [iOS Integration](#ios-integration)
- [Platform-Specific Considerations](#platform-specific-considerations)
- [Bluetooth & Permissions](#bluetooth--permissions)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Sample Code](#sample-code)

## Overview

The BitCraps Mobile SDKs provide a unified interface for building peer-to-peer gaming applications across Android and iOS platforms. The SDKs handle:

- **Bluetooth Low Energy** peer discovery and communication
- **Byzantine fault tolerance** for secure gaming consensus
- **Battery optimization** for background operation
- **Cross-platform compatibility** with consistent APIs
- **Security** with biometric authentication and encrypted communication

### Architecture

```
┌─────────────────────────────────────────────────┐
│                Your App                         │
├─────────────────────────────────────────────────┤
│               BitCraps SDK                      │
├─────────────────┬───────────────────────────────┤
│   Core Manager  │    Bluetooth Manager          │
├─────────────────┼───────────────────────────────┤
│ Security Mgr    │    Battery Manager            │
├─────────────────┼───────────────────────────────┤
│                Rust Core Library               │
└─────────────────────────────────────────────────┘
```

## Getting Started

### System Requirements

**Android:**
- Android API Level 21+ (Android 5.0)
- Bluetooth Low Energy support
- 2GB+ RAM recommended
- ARMv7/ARM64 architecture

**iOS:**
- iOS 14.0+ or macOS 11.0+
- Bluetooth Low Energy support
- 2GB+ RAM recommended
- ARM64 architecture

### Installation

#### Android (Gradle)

Add to your `build.gradle.kts` (app level):

```kotlin
dependencies {
    implementation("com.bitcraps:android-sdk:1.0.0")
    
    // Required dependencies
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.2")
    implementation("com.jakewharton.timber:timber:5.0.1")
}
```

#### iOS (Swift Package Manager)

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/bitcraps/bitcraps-ios-sdk.git", from: "1.0.0")
]
```

Or add via Xcode: **File > Add Package Dependencies** and enter the repository URL.

#### iOS (CocoaPods)

Add to your `Podfile`:

```ruby
pod 'BitCrapsSDK', '~> 1.0.0'
```

## Android Integration

### Basic Setup

#### 1. Initialize the SDK

```kotlin
class MainActivity : ComponentActivity() {
    private val gameViewModel: GameViewModel by viewModels()
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize SDK
        lifecycleScope.launch {
            try {
                val sdk = BitCrapsSDK.initialize(
                    context = applicationContext,
                    config = SDKConfig.production()
                )
                gameViewModel.onSDKInitialized(sdk)
            } catch (e: BitCrapsException) {
                // Handle initialization error
                gameViewModel.onSDKInitializationFailed(e)
            }
        }
    }
}
```

#### 2. Request Permissions

Add to your `AndroidManifest.xml`:

```xml
<!-- Bluetooth permissions -->
<uses-permission android:name="android.permission.BLUETOOTH" />
<uses-permission android:name="android.permission.BLUETOOTH_ADMIN" />

<!-- Android 12+ (API 31+) -->
<uses-permission android:name="android.permission.BLUETOOTH_ADVERTISE" />
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
<uses-permission android:name="android.permission.BLUETOOTH_SCAN" />

<!-- Location permission (required for BLE scanning) -->
<uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />

<!-- Background location (optional, for background scanning) -->
<uses-permission android:name="android.permission.ACCESS_BACKGROUND_LOCATION" />

<!-- Biometric authentication -->
<uses-permission android:name="android.permission.USE_BIOMETRIC" />
<uses-permission android:name="android.permission.USE_FINGERPRINT" />
```

Runtime permission handling:

```kotlin
class GameViewModel(private val context: Context) : ViewModel() {
    
    suspend fun requestPermissions(): Boolean {
        val permissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            arrayOf(
                Manifest.permission.BLUETOOTH_ADVERTISE,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        } else {
            arrayOf(
                Manifest.permission.BLUETOOTH,
                Manifest.permission.BLUETOOTH_ADMIN,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        }
        
        return permissions.all { permission ->
            ContextCompat.checkSelfPermission(context, permission) == 
                PackageManager.PERMISSION_GRANTED
        }
    }
}
```

#### 3. Core Operations

```kotlin
class GameManager {
    private lateinit var sdk: BitCrapsSDK
    
    suspend fun startDiscovery() {
        try {
            sdk.startDiscovery(
                config = DiscoveryConfig.default()
            )
        } catch (e: BitCrapsException) {
            handleError(e)
        }
    }
    
    suspend fun createGame(gameType: GameType = GameType.CRAPS) {
        try {
            val gameSession = sdk.createGame(
                gameType = gameType,
                config = GameConfig.default()
            )
            // Handle game session
        } catch (e: BitCrapsException) {
            handleError(e)
        }
    }
    
    fun observeEvents() {
        sdk.events.collect { event ->
            when (event) {
                is GameEvent.PeerDiscovered -> handlePeerDiscovered(event.peer)
                is GameEvent.GameCreated -> handleGameCreated(event.gameId)
                is GameEvent.DiceRolled -> handleDiceRolled(event)
                // ... handle other events
            }
        }
    }
}
```

### Advanced Features

#### Battery Optimization Detection

```kotlin
class BatteryOptimizationManager(private val context: Context) {
    
    fun checkBatteryOptimization(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val powerManager = context.getSystemService(Context.POWER_SERVICE) as PowerManager
            !powerManager.isIgnoringBatteryOptimizations(context.packageName)
        } else {
            false
        }
    }
    
    fun requestBatteryOptimizationExemption(activity: Activity) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                data = Uri.parse("package:${activity.packageName}")
            }
            activity.startActivity(intent)
        }
    }
}
```

#### Background Service Integration

```kotlin
class BitCrapsService : Service() {
    private val binder = LocalBinder()
    private lateinit var sdk: BitCrapsSDK
    
    inner class LocalBinder : Binder() {
        fun getService(): BitCrapsService = this@BitCrapsService
    }
    
    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(NOTIFICATION_ID, createNotification())
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // Handle discovery and game operations in background
        return START_STICKY
    }
    
    override fun onBind(intent: Intent): IBinder = binder
    
    private fun createNotification(): Notification {
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("BitCraps")
            .setContentText("Discovering peers...")
            .setSmallIcon(R.drawable.ic_notification)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .build()
    }
}
```

## iOS Integration

### Basic Setup

#### 1. Initialize the SDK

```swift
@MainActor
class GameManager: ObservableObject {
    @Published var isSDKInitialized = false
    @Published var lastError: BitCrapsError?
    
    private var sdk: BitCrapsSDK?
    private var cancellables = Set<AnyCancellable>()
    
    func initializeSDK() async {
        do {
            sdk = try await BitCrapsSDK.initialize(
                config: SDKConfig.production
            )
            isSDKInitialized = true
            setupEventHandling()
        } catch {
            lastError = error as? BitCrapsError
            isSDKInitialized = false
        }
    }
    
    private func setupEventHandling() {
        sdk?.events
            .receive(on: DispatchQueue.main)
            .sink { [weak self] event in
                self?.handleEvent(event)
            }
            .store(in: &cancellables)
    }
}
```

#### 2. Configure Info.plist

Add required permissions to your `Info.plist`:

```xml
<dict>
    <!-- Bluetooth usage description -->
    <key>NSBluetoothAlwaysUsageDescription</key>
    <string>BitCraps uses Bluetooth to discover and connect with other players for peer-to-peer gaming.</string>
    
    <key>NSBluetoothPeripheralUsageDescription</key>
    <string>BitCraps uses Bluetooth to advertise your device to other players.</string>
    
    <!-- Face ID / Touch ID -->
    <key>NSFaceIDUsageDescription</key>
    <string>BitCraps uses Face ID for secure authentication during gaming sessions.</string>
    
    <!-- Background modes (optional) -->
    <key>UIBackgroundModes</key>
    <array>
        <string>bluetooth-central</string>
        <string>bluetooth-peripheral</string>
    </array>
    
    <!-- Required device capabilities -->
    <key>UIRequiredDeviceCapabilities</key>
    <array>
        <string>bluetooth-le</string>
    </array>
</dict>
```

#### 3. Core Operations

```swift
@MainActor
class GameService: ObservableObject {
    @Published var discoveredPeers: [PeerInfo] = []
    @Published var gameState: GameState?
    @Published var connectionStatus: ConnectionStatus = .disconnected
    
    private var sdk: BitCrapsSDK?
    
    func startDiscovery() async throws {
        guard let sdk = sdk else { 
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        try await sdk.startDiscovery(config: .default)
    }
    
    func createGame(gameType: GameType = .craps) async throws -> GameSession {
        guard let sdk = sdk else {
            throw BitCrapsError.initializationFailed(reason: "SDK not initialized")
        }
        
        return try await sdk.createGame(gameType: gameType, config: .default)
    }
    
    func handleEvent(_ event: GameEvent) {
        switch event {
        case .peerDiscovered(let peer):
            discoveredPeers.append(peer)
            
        case .gameStateChanged(_, let newState):
            gameState = newState
            
        case .errorOccurred(let error):
            // Handle error
            print("Error: \(error.displayMessage)")
            
        default:
            break
        }
    }
}
```

### SwiftUI Integration

#### Reactive UI Updates

```swift
struct GameView: View {
    @StateObject private var gameService = GameService()
    @State private var showingError = false
    
    var body: some View {
        NavigationView {
            VStack {
                // Connection Status
                ConnectionStatusView(status: gameService.connectionStatus)
                
                // Peer List
                PeerListView(peers: gameService.discoveredPeers)
                
                // Game Controls
                GameControlsView(
                    onStartDiscovery: {
                        Task {
                            try await gameService.startDiscovery()
                        }
                    },
                    onCreateGame: {
                        Task {
                            _ = try await gameService.createGame()
                        }
                    }
                )
            }
            .navigationTitle("BitCraps")
        }
        .onAppear {
            Task {
                await gameService.initializeSDK()
            }
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") { }
        } message: {
            Text(gameService.lastError?.displayMessage ?? "Unknown error")
        }
    }
}
```

#### Background Handling

```swift
@main
struct BitCrapsApp: App {
    @StateObject private var gameManager = GameManager()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(gameManager)
                .onReceive(NotificationCenter.default.publisher(
                    for: UIApplication.willEnterForegroundNotification
                )) { _ in
                    gameManager.handleAppWillEnterForeground()
                }
                .onReceive(NotificationCenter.default.publisher(
                    for: UIApplication.didEnterBackgroundNotification
                )) { _ in
                    gameManager.handleAppDidEnterBackground()
                }
        }
    }
}

extension GameManager {
    func handleAppWillEnterForeground() {
        sdk?.applicationWillEnterForeground()
    }
    
    func handleAppDidEnterBackground() {
        sdk?.applicationDidEnterBackground()
    }
}
```

## Platform-Specific Considerations

### Android Considerations

#### Battery Optimization

Android's Doze mode and App Standby can significantly impact BLE functionality:

```kotlin
// Check if battery optimization is enabled
fun isBatteryOptimizationEnabled(context: Context): Boolean {
    val powerManager = context.getSystemService(Context.POWER_SERVICE) as PowerManager
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        !powerManager.isIgnoringBatteryOptimizations(context.packageName)
    } else {
        false
    }
}

// Request exemption from battery optimization
fun requestBatteryOptimizationExemption(activity: Activity) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
        intent.data = Uri.parse("package:${activity.packageName}")
        activity.startActivity(intent)
    }
}
```

#### Foreground Services

For continuous discovery, use a foreground service:

```xml
<!-- AndroidManifest.xml -->
<service
    android:name=".BitCrapsService"
    android:enabled="true"
    android:exported="false"
    android:foregroundServiceType="connectedDevice" />
```

```kotlin
class BitCrapsService : Service() {
    companion object {
        const val CHANNEL_ID = "BitCrapsService"
        const val NOTIFICATION_ID = 1
    }
    
    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("BitCraps")
            .setContentText("Discovering peers...")
            .setSmallIcon(R.drawable.ic_notification)
            .setForegroundServiceBehavior(
                NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE
            )
            .build()
            
        startForeground(NOTIFICATION_ID, notification)
    }
}
```

### iOS Considerations

#### Background Limitations

iOS severely restricts background BLE operations:

- **Background Scanning**: Limited to 30 seconds before suspension
- **Background Advertising**: Local name not included, limited service UUIDs
- **App State Preservation**: Enable state preservation for BLE operations

```swift
// Enable state preservation
class BluetoothManager: NSObject, CBCentralManagerDelegate, CBPeripheralManagerDelegate {
    private let centralManager = CBCentralManager(
        delegate: self,
        queue: nil,
        options: [CBCentralManagerOptionRestoreIdentifierKey: "BitCrapsRestoreIdentifier"]
    )
    
    // Implement state restoration
    func centralManager(
        _ central: CBCentralManager,
        willRestoreState dict: [String: Any]
    ) {
        if let peripherals = dict[CBCentralManagerRestoredStatePeripheralsKey] as? [CBPeripheral] {
            // Restore discovered peripherals
            for peripheral in peripherals {
                // Re-establish connections
            }
        }
    }
}
```

#### Battery Optimization

Implement power-aware scanning:

```swift
func configurePowerOptimizedScanning() {
    let scanOptions: [String: Any] = [
        CBCentralManagerScanOptionAllowDuplicatesKey: false, // Reduce battery drain
        CBCentralManagerScanOptionSolicitedServiceUUIDsKey: serviceUUIDs
    ]
    
    // Use scan intervals based on battery level
    let batteryLevel = UIDevice.current.batteryLevel
    let scanInterval: TimeInterval = batteryLevel < 0.2 ? 10.0 : 2.0
    
    centralManager.scanForPeripherals(withServices: serviceUUIDs, options: scanOptions)
}
```

## Bluetooth & Permissions

### Permission Flow

#### Android Permission Flow

```kotlin
class PermissionManager(private val activity: Activity) {
    
    fun checkAndRequestPermissions(): Boolean {
        val requiredPermissions = getRequiredPermissions()
        val missingPermissions = requiredPermissions.filter { permission ->
            ContextCompat.checkSelfPermission(activity, permission) != 
                PackageManager.PERMISSION_GRANTED
        }
        
        if (missingPermissions.isNotEmpty()) {
            ActivityCompat.requestPermissions(
                activity,
                missingPermissions.toTypedArray(),
                REQUEST_CODE_PERMISSIONS
            )
            return false
        }
        
        return true
    }
    
    private fun getRequiredPermissions(): List<String> {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            listOf(
                Manifest.permission.BLUETOOTH_ADVERTISE,
                Manifest.permission.BLUETOOTH_CONNECT,
                Manifest.permission.BLUETOOTH_SCAN,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        } else {
            listOf(
                Manifest.permission.BLUETOOTH,
                Manifest.permission.BLUETOOTH_ADMIN,
                Manifest.permission.ACCESS_FINE_LOCATION
            )
        }
    }
}
```

#### iOS Permission Flow

```swift
class PermissionManager {
    
    func checkBluetoothPermission() -> Bool {
        switch CBCentralManager.authorization {
        case .allowedAlways:
            return true
        case .denied, .restricted:
            return false
        case .notDetermined:
            // Permission will be requested when CBCentralManager is initialized
            return false
        @unknown default:
            return false
        }
    }
    
    func requestBiometricPermission() async -> Bool {
        let context = LAContext()
        var error: NSError?
        
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
            return false
        }
        
        do {
            let result = try await context.evaluatePolicy(
                .deviceOwnerAuthenticationWithBiometrics,
                localizedReason: "Authenticate to enable secure gaming"
            )
            return result
        } catch {
            return false
        }
    }
}
```

### BLE Service Configuration

#### Service UUIDs

Use consistent service UUIDs across platforms:

```kotlin
// Android
object BitCrapsConstants {
    val PRIMARY_SERVICE_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345678")
    val GAME_DATA_CHARACTERISTIC_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-567812345679")
    val PEER_INFO_CHARACTERISTIC_UUID: UUID = UUID.fromString("12345678-1234-5678-1234-56781234567A")
}
```

```swift
// iOS
struct BitCrapsConstants {
    static let primaryServiceUUID = CBUUID(string: "12345678-1234-5678-1234-567812345678")
    static let gameDataCharacteristicUUID = CBUUID(string: "12345678-1234-5678-1234-567812345679")
    static let peerInfoCharacteristicUUID = CBUUID(string: "12345678-1234-5678-1234-56781234567A")
}
```

## Best Practices

### Performance Optimization

#### 1. Efficient Discovery

```kotlin
// Android - Use appropriate scan intervals
val discoveryConfig = DiscoveryConfig(
    scanWindowMs = 300,          // Short scan window
    scanIntervalMs = 2000,       // Longer interval between scans
    powerMode = PowerMode.BALANCED,
    maxPeers = 20                // Limit peer cache size
)
```

```swift
// iOS - Power-aware scanning
func startPowerOptimizedScanning() {
    let batteryLevel = UIDevice.current.batteryLevel
    let config = DiscoveryConfig(
        scanWindowMs: batteryLevel < 0.3 ? 100 : 300,
        scanIntervalMs: batteryLevel < 0.3 ? 5000 : 2000,
        powerMode: batteryLevel < 0.3 ? .batterySaver : .balanced
    )
    
    try await sdk.startDiscovery(config: config)
}
```

#### 2. Memory Management

```kotlin
// Android - Proper lifecycle management
class GameActivity : AppCompatActivity() {
    private var sdk: BitCrapsSDK? = null
    
    override fun onDestroy() {
        super.onDestroy()
        lifecycleScope.launch {
            sdk?.shutdown()
            sdk = null
        }
    }
}
```

```swift
// iOS - Clean up resources
class GameManager: ObservableObject {
    deinit {
        Task {
            await sdk?.shutdown()
        }
    }
}
```

#### 3. Error Handling

Implement comprehensive error handling:

```kotlin
// Android
suspend fun handleBitCrapsError(error: BitCrapsException) {
    when (error) {
        is BluetoothException -> {
            when (error.bluetoothErrorCode) {
                BluetoothException.ERROR_ADAPTER_DISABLED -> {
                    // Prompt user to enable Bluetooth
                    showEnableBluetoothDialog()
                }
                BluetoothException.ERROR_PERMISSION_DENIED -> {
                    // Request permissions
                    requestBluetoothPermissions()
                }
                else -> {
                    // General Bluetooth error
                    showError("Bluetooth error: ${error.message}")
                }
            }
        }
        is NetworkException -> {
            if (error.retryable) {
                // Retry with exponential backoff
                retryWithBackoff { /* retry operation */ }
            } else {
                showError("Network error: ${error.message}")
            }
        }
        // Handle other error types...
    }
}
```

```swift
// iOS
func handleBitCrapsError(_ error: BitCrapsError) {
    switch error {
    case .bluetoothError(let reason, _, _):
        // Handle Bluetooth errors
        showBluetoothError(reason)
        
    case .networkError(let reason, let networkErrorType, _):
        if error.isRetryable {
            // Retry operation
            retryLastOperation()
        } else {
            showNetworkError(reason)
        }
        
    case .permissionError(let reason, let permissionType, _):
        // Handle permission errors
        requestPermission(permissionType)
        
    default:
        showGenericError(error.displayMessage)
    }
}
```

### Security Best Practices

#### 1. Biometric Authentication

```kotlin
// Android
class BiometricAuthenticator(private val activity: FragmentActivity) {
    
    fun authenticate(onSuccess: () -> Unit, onError: (String) -> Unit) {
        val biometricPrompt = BiometricPrompt(
            activity as ComponentActivity,
            ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    onSuccess()
                }
                
                override fun onAuthenticationFailed() {
                    onError("Authentication failed")
                }
                
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    onError(errString.toString())
                }
            }
        )
        
        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle("Authenticate for BitCraps")
            .setSubtitle("Use your biometric credential to secure your game")
            .setNegativeButtonText("Cancel")
            .build()
            
        biometricPrompt.authenticate(promptInfo)
    }
}
```

```swift
// iOS
class BiometricAuthenticator {
    
    func authenticate() async -> Bool {
        let context = LAContext()
        
        do {
            let result = try await context.evaluatePolicy(
                .deviceOwnerAuthenticationWithBiometrics,
                localizedReason: "Authenticate to secure your BitCraps session"
            )
            return result
        } catch let error as LAError {
            switch error.code {
            case .userCancel, .userFallback:
                return false
            case .biometryNotAvailable:
                // Fallback to passcode
                return await authenticateWithPasscode()
            default:
                return false
            }
        } catch {
            return false
        }
    }
    
    private func authenticateWithPasscode() async -> Bool {
        let context = LAContext()
        
        do {
            let result = try await context.evaluatePolicy(
                .deviceOwnerAuthentication,
                localizedReason: "Authenticate with your device passcode"
            )
            return result
        } catch {
            return false
        }
    }
}
```

#### 2. Data Validation

```kotlin
// Android - Input validation
class GameInputValidator {
    
    fun validateBetAmount(amount: Long, minBet: Long, maxBet: Long, balance: Long): ValidationResult {
        return when {
            amount < minBet -> ValidationResult.Error("Bet amount below minimum ($minBet)")
            amount > maxBet -> ValidationResult.Error("Bet amount above maximum ($maxBet)")
            amount > balance -> ValidationResult.Error("Insufficient balance")
            else -> ValidationResult.Success
        }
    }
    
    fun validateGameId(gameId: String): Boolean {
        return gameId.matches(Regex("[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}"))
    }
}

sealed class ValidationResult {
    object Success : ValidationResult()
    data class Error(val message: String) : ValidationResult()
}
```

### UI/UX Guidelines

#### 1. Progressive Disclosure

```kotlin
// Android - Show features progressively
@Composable
fun GameSetupScreen(gameManager: GameManager) {
    var showAdvancedOptions by remember { mutableStateOf(false) }
    
    Column {
        // Basic options always visible
        BasicGameOptions()
        
        // Advanced options behind toggle
        Row(
            modifier = Modifier.clickable { 
                showAdvancedOptions = !showAdvancedOptions 
            }
        ) {
            Text("Advanced Options")
            Icon(
                imageVector = if (showAdvancedOptions) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                contentDescription = null
            )
        }
        
        AnimatedVisibility(visible = showAdvancedOptions) {
            AdvancedGameOptions()
        }
    }
}
```

#### 2. Loading States

```swift
// iOS - Clear loading indicators
struct DiscoveryView: View {
    @StateObject private var gameService = GameService()
    
    var body: some View {
        VStack {
            if gameService.isDiscovering {
                VStack {
                    ProgressView()
                        .scaleEffect(1.2)
                    Text("Discovering peers...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Button("Stop") {
                        Task {
                            await gameService.stopDiscovery()
                        }
                    }
                }
                .padding()
            } else {
                Button("Start Discovery") {
                    Task {
                        await gameService.startDiscovery()
                    }
                }
                .buttonStyle(.borderedProminent)
            }
            
            PeerListView(peers: gameService.discoveredPeers)
        }
    }
}
```

#### 3. Error Feedback

```kotlin
// Android - User-friendly error messages
fun getErrorMessage(error: BitCrapsException): String {
    return when (error) {
        is BluetoothException -> when (error.bluetoothErrorCode) {
            BluetoothException.ERROR_ADAPTER_DISABLED -> 
                "Please enable Bluetooth to discover other players"
            BluetoothException.ERROR_PERMISSION_DENIED -> 
                "Bluetooth permission is required to play with others"
            else -> "Bluetooth connection issue. Please try again."
        }
        is NetworkException -> 
            "Connection problem. Check your network and try again."
        is GameException -> 
            "Game error occurred. Please restart the game."
        else -> "Something went wrong. Please try again."
    }
}
```

## Troubleshooting

### Common Issues

#### Android Issues

**Issue**: App crashes on startup
```
Solution: Check if all required permissions are declared in AndroidManifest.xml
and runtime permissions are properly requested.
```

**Issue**: Peers not discovered
```
Solution: 
1. Verify Bluetooth is enabled and permissions are granted
2. Check if battery optimization is affecting the app
3. Ensure location services are enabled (required for BLE scanning)
4. Test on different devices - some manufacturers have BLE restrictions
```

**Issue**: Background discovery stops working
```
Solution:
1. Implement a foreground service
2. Request battery optimization exemption
3. Handle device-specific battery optimization settings
```

#### iOS Issues

**Issue**: Scanning stops after 30 seconds in background
```
Solution: This is expected iOS behavior. Implement state preservation
and handle app state transitions properly.
```

**Issue**: Advertising doesn't include device name in background
```
Solution: iOS limitation. Use service UUIDs for identification instead
of relying on device names.
```

**Issue**: Biometric authentication fails
```
Solution: 
1. Check if biometric authentication is available on device
2. Implement fallback to passcode authentication
3. Handle various LAError cases appropriately
```

### Debugging Tools

#### Enable Debug Logging

```kotlin
// Android
BitCrapsSDK.setDebugLogging(
    enabled = true,
    logLevel = LogLevel.DEBUG
)
```

```swift
// iOS
sdk.setDebugLogging(
    enabled: true,
    logLevel: .debug
)
```

#### Performance Monitoring

```kotlin
// Android - Monitor performance metrics
lifecycleScope.launch {
    while (isActive) {
        val metrics = sdk.getPerformanceMetrics()
        Log.d("BitCraps", "CPU: ${metrics.cpuUsagePercent}%, Memory: ${metrics.memoryUsageMB}MB")
        delay(5000) // Log every 5 seconds
    }
}
```

```swift
// iOS - Monitor performance
Timer.scheduledTimer(withTimeInterval: 5.0, repeats: true) { _ in
    Task {
        let metrics = await sdk.getPerformanceMetrics()
        print("CPU: \(metrics.cpuUsagePercent)%, Memory: \(metrics.memoryUsageMB)MB")
    }
}
```

#### Network Diagnostics

```kotlin
// Android
suspend fun runDiagnostics(): DiagnosticsReport {
    return try {
        sdk.runNetworkDiagnostics()
    } catch (e: Exception) {
        // Handle diagnostics error
        DiagnosticsReport.empty()
    }
}
```

```swift
// iOS
func runDiagnostics() async -> DiagnosticsReport? {
    do {
        return try await sdk.runNetworkDiagnostics()
    } catch {
        print("Diagnostics failed: \(error)")
        return nil
    }
}
```

### Getting Help

- **GitHub Issues**: https://github.com/bitcraps/bitcraps-rust/issues
- **Documentation**: https://docs.bitcraps.com
- **Community Forum**: https://community.bitcraps.com
- **Email Support**: support@bitcraps.com

## Sample Code

Complete sample applications are available in the SDK repository:

- **Android Sample**: `/mobile/android/sample-app/`
- **iOS Sample**: `/mobile/ios/BitCrapsSampleApp/`

These samples demonstrate:
- Complete integration flow
- Permission handling
- Game creation and joining
- Real-time gameplay
- Error handling and recovery
- Performance optimization
- UI/UX best practices

### Quick Start Templates

#### Android (Kotlin + Compose)

```kotlin
@Composable
fun BitCrapsQuickStart() {
    var sdk by remember { mutableStateOf<BitCrapsSDK?>(null) }
    var isDiscovering by remember { mutableStateOf(false) }
    var discoveredPeers by remember { mutableStateOf<List<PeerInfo>>(emptyList()) }
    val context = LocalContext.current
    
    LaunchedEffect(Unit) {
        try {
            sdk = BitCrapsSDK.initialize(context)
        } catch (e: BitCrapsException) {
            // Handle initialization error
        }
    }
    
    Column(
        modifier = Modifier.padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Button(
            onClick = {
                if (!isDiscovering) {
                    lifecycleScope.launch {
                        try {
                            sdk?.startDiscovery()
                            isDiscovering = true
                        } catch (e: BitCrapsException) {
                            // Handle error
                        }
                    }
                }
            },
            enabled = sdk != null && !isDiscovering
        ) {
            Text("Start Discovery")
        }
        
        LazyColumn {
            items(discoveredPeers) { peer ->
                PeerItem(peer = peer) {
                    // Handle peer selection
                    lifecycleScope.launch {
                        try {
                            sdk?.connectToPeer(peer.id)
                        } catch (e: BitCrapsException) {
                            // Handle connection error
                        }
                    }
                }
            }
        }
    }
}
```

#### iOS (SwiftUI)

```swift
struct BitCrapsQuickStart: View {
    @StateObject private var gameManager = GameManager()
    
    var body: some View {
        NavigationView {
            VStack(spacing: 16) {
                Button("Start Discovery") {
                    Task {
                        await gameManager.startDiscovery()
                    }
                }
                .disabled(!gameManager.isSDKInitialized || gameManager.isDiscovering)
                
                List(gameManager.discoveredPeers) { peer in
                    PeerRow(peer: peer) {
                        Task {
                            await gameManager.connectToPeer(peer.id)
                        }
                    }
                }
            }
            .padding()
            .navigationTitle("BitCraps")
        }
        .task {
            await gameManager.initializeSDK()
        }
    }
}

struct PeerRow: View {
    let peer: PeerInfo
    let onConnect: () -> Void
    
    var body: some View {
        HStack {
            VStack(alignment: .leading) {
                Text(peer.displayName ?? peer.id)
                    .font(.headline)
                Text("Signal: \(peer.signalStrength) dBm")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            Button("Connect", action: onConnect)
                .buttonStyle(.bordered)
                .disabled(peer.isConnected)
        }
    }
}
```

This integration guide provides a comprehensive foundation for building BitCraps-powered mobile applications. For the most up-to-date examples and advanced features, refer to the official SDK documentation and sample applications.