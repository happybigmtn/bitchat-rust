# Mobile Performance Optimization Guide
## BitCraps Production Performance Implementation

*Version: 1.0 | Last Updated: 2025-08-24 | Status: Production Ready*

---

## Overview

This document provides comprehensive performance optimization strategies for BitCraps mobile applications across Android and iOS platforms. It includes specific implementation details, monitoring tools, and adaptive optimization techniques.

**Performance Goals**
- **Target Frame Rate**: 60fps sustained during gameplay
- **Memory Usage**: <150MB Android, <100MB iOS
- **Battery Drain**: <5% per hour active gaming
- **Network Efficiency**: <50KB/min idle, <500KB/hour gaming
- **Cold Start Time**: <2s Android, <1.5s iOS

---

## Performance Architecture

### Performance Monitoring Framework

#### Android Implementation
```kotlin
// PerformanceMonitor.kt
class PerformanceMonitor(private val context: Context) {
    private val frameMetrics = FrameMetricsAggregator()
    private val memoryInfo = ActivityManager.MemoryInfo()
    private val batteryManager = context.getSystemService(Context.BATTERY_SERVICE) as BatteryManager
    
    private val _performanceState = MutableStateFlow(PerformanceState.default())
    val performanceState: StateFlow<PerformanceState> = _performanceState.asStateFlow()
    
    fun startMonitoring() {
        frameMetrics.add(context as Activity)
        
        GlobalScope.launch {
            while (isActive) {
                updatePerformanceMetrics()
                delay(1000) // Update every second
            }
        }
    }
    
    private suspend fun updatePerformanceMetrics() {
        val frameData = frameMetrics.metrics
        val memoryUsage = getMemoryUsage()
        val batteryDrain = getBatteryDrainRate()
        val networkUsage = getNetworkUsage()
        
        val newState = PerformanceState(
            averageFps = calculateAverageFps(frameData),
            frameDrops = calculateFrameDrops(frameData),
            memoryUsageMB = memoryUsage.toDouble() / (1024 * 1024),
            batteryDrainRate = batteryDrain,
            networkUsageKBps = networkUsage,
            cpuUsagePercent = getCpuUsage(),
            thermalState = getThermalState(),
            lastUpdated = System.currentTimeMillis()
        )
        
        _performanceState.value = newState
        
        // Apply adaptive optimizations
        if (shouldOptimize(newState)) {
            applyPerformanceOptimizations(newState)
        }
    }
    
    private fun calculateAverageFps(frameData: SparseIntArray?): Double {
        if (frameData == null || frameData.size() == 0) return 60.0
        
        var totalFrames = 0
        var totalTime = 0
        
        for (i in 0 until frameData.size()) {
            val frameDuration = frameData.keyAt(i)
            val frameCount = frameData.valueAt(i)
            totalFrames += frameCount
            totalTime += frameDuration * frameCount
        }
        
        return if (totalTime > 0) {
            (totalFrames * 16.67) / (totalTime / 1_000_000.0) // Convert to fps
        } else 60.0
    }
    
    private fun getMemoryUsage(): Long {
        val runtime = Runtime.getRuntime()
        return runtime.totalMemory() - runtime.freeMemory()
    }
    
    private fun getBatteryDrainRate(): Double {
        // Implementation using historical battery level data
        return batteryOptimizationManager.getCurrentDrainRate()
    }
    
    private fun getNetworkUsage(): Double {
        val stats = TrafficStats.getUidRxBytes(context.applicationInfo.uid) +
                   TrafficStats.getUidTxBytes(context.applicationInfo.uid)
        
        // Calculate rate based on previous measurement
        return calculateNetworkRate(stats)
    }
    
    private fun shouldOptimize(state: PerformanceState): Boolean {
        return state.averageFps < 55.0 ||
               state.memoryUsageMB > 120.0 ||
               state.batteryDrainRate > 6.0 ||
               state.cpuUsagePercent > 80.0
    }
    
    private fun applyPerformanceOptimizations(state: PerformanceState) {
        when {
            state.averageFps < 45.0 -> {
                // Critical frame rate issue
                reduceVisualComplexity()
                optimizeAnimations()
            }
            state.memoryUsageMB > 140.0 -> {
                // Critical memory usage
                performGarbageCollection()
                clearCaches()
            }
            state.batteryDrainRate > 8.0 -> {
                // Critical battery drain
                reduceCpuIntensiveOperations()
                optimizeNetworkOperations()
            }
        }
    }
}
```

#### iOS Implementation
```swift
// PerformanceMonitor.swift
@available(iOS 15.0, *)
class PerformanceMonitor: ObservableObject {
    @Published var performanceState = PerformanceState.default()
    
    private var displayLink: CADisplayLink?
    private var frameTimestamps: [CFTimeInterval] = []
    private var monitoringTimer: Timer?
    
    private let logger = Logger(subsystem: "com.bitcraps.app", category: "PerformanceMonitor")
    
    func startMonitoring() {
        setupFrameRateMonitoring()
        setupPerformanceTimer()
        logger.info("Performance monitoring started")
    }
    
    private func setupFrameRateMonitoring() {
        displayLink = CADisplayLink(target: self, selector: #selector(frameUpdate))
        displayLink?.preferredFrameRateRange = CAFrameRateRange(
            minimum: 60,
            maximum: 120,
            preferred: 60
        )
        displayLink?.add(to: .main, forMode: .default)
    }
    
    @objc private func frameUpdate(displayLink: CADisplayLink) {
        frameTimestamps.append(displayLink.timestamp)
        
        // Keep only last 60 frames (1 second at 60fps)
        if frameTimestamps.count > 60 {
            frameTimestamps.removeFirst()
        }
    }
    
    private func setupPerformanceTimer() {
        monitoringTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.updatePerformanceMetrics()
        }
    }
    
    private func updatePerformanceMetrics() {
        let fps = calculateCurrentFPS()
        let memoryUsage = getMemoryUsage()
        let batteryDrain = getBatteryDrainRate()
        let cpuUsage = getCpuUsage()
        let thermalState = ProcessInfo.processInfo.thermalState
        
        let newState = PerformanceState(
            averageFps: fps,
            frameDrops: calculateFrameDrops(),
            memoryUsageMB: memoryUsage,
            batteryDrainRate: batteryDrain,
            networkUsageKBps: getNetworkUsage(),
            cpuUsagePercent: cpuUsage,
            thermalState: thermalState,
            lastUpdated: Date().timeIntervalSince1970
        )
        
        DispatchQueue.main.async {
            self.performanceState = newState
        }
        
        // Apply adaptive optimizations
        if shouldOptimize(newState) {
            applyPerformanceOptimizations(newState)
        }
    }
    
    private func calculateCurrentFPS() -> Double {
        guard frameTimestamps.count >= 2 else { return 60.0 }
        
        let timespan = frameTimestamps.last! - frameTimestamps.first!
        let frameCount = Double(frameTimestamps.count - 1)
        
        return frameCount / timespan
    }
    
    private func getMemoryUsage() -> Double {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size)/4
        
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
            }
        }
        
        if result == KERN_SUCCESS {
            return Double(info.resident_size) / (1024 * 1024) // Convert to MB
        }
        
        return 0
    }
    
    private func getCpuUsage() -> Double {
        var totalUsageOfCPU: Double = 0
        var threadsList = UnsafeMutablePointer<thread_act_t>.allocate(capacity: 1)
        var threadsCount = mach_msg_type_number_t(0)
        
        let threadsResult = withUnsafeMutablePointer(to: &threadsList) {
            return $0.withMemoryRebound(to: thread_act_array_t?.self, capacity: 1) {
                task_threads(mach_task_self_, $0, &threadsCount)
            }
        }
        
        if threadsResult == KERN_SUCCESS {
            for index in 0..<threadsCount {
                var threadInfo = thread_basic_info()
                var threadInfoCount = mach_msg_type_number_t(THREAD_INFO_MAX)
                
                let infoResult = withUnsafeMutablePointer(to: &threadInfo) {
                    $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                        thread_info(threadsList[Int(index)], thread_flavor_t(THREAD_BASIC_INFO), $0, &threadInfoCount)
                    }
                }
                
                if infoResult == KERN_SUCCESS {
                    let threadTotalCpu = (Double(threadInfo.cpu_usage) / Double(TH_USAGE_SCALE)) * 100.0
                    totalUsageOfCPU += threadTotalCpu
                }
            }
        }
        
        vm_deallocate(mach_task_self_, vm_address_t(UnsafePointer(threadsList).pointee), vm_size_t(threadsCount))
        
        return totalUsageOfCPU
    }
}
```

---

## Performance Optimization Strategies

### 1. Frame Rate Optimization

#### Compose Optimization (Android)
```kotlin
// OptimizedComposables.kt
@Composable
fun OptimizedDiceAnimation(
    dice1: Int,
    dice2: Int,
    isRolling: Boolean,
    performanceMode: PerformanceMode = PerformanceMode.BALANCED
) {
    // Use remember to avoid recomposition
    val animationSpec = remember(performanceMode) {
        when (performanceMode) {
            PerformanceMode.HIGH_PERFORMANCE -> tween(
                durationMillis = 1000,
                easing = FastOutSlowInEasing
            )
            PerformanceMode.BALANCED -> tween(
                durationMillis = 2000,
                easing = LinearOutSlowInEasing
            )
            PerformanceMode.BATTERY_SAVER -> tween(
                durationMillis = 500,
                easing = LinearEasing
            )
        }
    }
    
    // Optimize recomposition with derivedStateOf
    val shouldAnimate by remember {
        derivedStateOf { isRolling && performanceMode != PerformanceMode.BATTERY_SAVER }
    }
    
    // Use LaunchedEffect to control animation lifecycle
    LaunchedEffect(isRolling) {
        if (isRolling) {
            // Perform animation
        }
    }
    
    // Minimize draw operations
    Canvas(modifier = Modifier.size(80.dp)) {
        drawDiceOptimized(dice1, dice2, shouldAnimate, performanceMode)
    }
}

private fun DrawScope.drawDiceOptimized(
    dice1: Int,
    dice2: Int,
    shouldAnimate: Boolean,
    performanceMode: PerformanceMode
) {
    // Pre-computed paths for dice dots
    val dotPaths = remember { precomputeDotPaths() }
    
    // Use hardware acceleration when available
    when (performanceMode) {
        PerformanceMode.HIGH_PERFORMANCE -> {
            drawDiceWithHardwareAcceleration(dice1, dice2, dotPaths)
        }
        else -> {
            drawDiceStandard(dice1, dice2, dotPaths)
        }
    }
}
```

#### SwiftUI Optimization (iOS)
```swift
// OptimizedViews.swift
struct OptimizedDiceView: View {
    let dice1: Int
    let dice2: Int
    let isRolling: Bool
    let performanceMode: PerformanceMode
    
    // Use @State with explicit animation control
    @State private var rotationAngle: Double = 0
    @State private var animationId = UUID()
    
    var body: some View {
        HStack(spacing: 20) {
            DieViewOptimized(value: dice1, isRolling: isRolling, performanceMode: performanceMode)
            DieViewOptimized(value: dice2, isRolling: isRolling, performanceMode: performanceMode)
        }
        .rotation3DEffect(
            .degrees(rotationAngle),
            axis: (x: 1, y: 1, z: 0)
        )
        .animation(animationForPerformanceMode(), value: animationId)
        .onChange(of: isRolling) { rolling in
            if rolling {
                withAnimation(animationForPerformanceMode()) {
                    rotationAngle += 360
                    animationId = UUID()
                }
            }
        }
    }
    
    private func animationForPerformanceMode() -> Animation {
        switch performanceMode {
        case .highPerformance:
            return .easeInOut(duration: 1.0)
        case .balanced:
            return .easeInOut(duration: 2.0)
        case .batterySaver:
            return .linear(duration: 0.5)
        }
    }
}

struct DieViewOptimized: View {
    let value: Int
    let isRolling: Bool
    let performanceMode: PerformanceMode
    
    // Pre-computed dot positions for performance
    private static let dotPositions: [Int: [CGPoint]] = {
        var positions: [Int: [CGPoint]] = [:]
        for i in 1...6 {
            positions[i] = computeDotPositions(for: i)
        }
        return positions
    }()
    
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 12)
                .fill(
                    LinearGradient(
                        gradient: Gradient(colors: [.white, .gray.opacity(0.8)]),
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: 60, height: 60)
            
            if isRolling && performanceMode == .batterySaver {
                // Show simple placeholder during battery saver mode
                Text("?")
                    .font(.title)
                    .fontWeight(.bold)
            } else if !isRolling && value > 0 {
                // Use pre-computed positions for optimal performance
                ForEach(0..<Self.dotPositions[value]!.count, id: \.self) { index in
                    Circle()
                        .fill(Color.black)
                        .frame(width: 8, height: 8)
                        .position(Self.dotPositions[value]![index])
                }
            }
        }
    }
}
```

### 2. Memory Optimization

#### Android Memory Management
```kotlin
// MemoryManager.kt
class MemoryManager {
    private val memoryCache = LruCache<String, Bitmap>(
        (Runtime.getRuntime().maxMemory() / 1024 / 8).toInt() // Use 1/8th of available memory
    )
    
    private val imageLoader = ImageLoader.Builder(context)
        .memoryCache {
            MemoryCache.Builder(context)
                .maxSizePercent(0.25) // Use 25% of available memory
                .build()
        }
        .diskCache {
            DiskCache.Builder()
                .directory(context.cacheDir.resolve("image_cache"))
                .maxSizeBytes(50 * 1024 * 1024) // 50MB disk cache
                .build()
        }
        .build()
    
    fun optimizeMemoryUsage() {
        // Force garbage collection if memory usage is high
        val runtime = Runtime.getRuntime()
        val usedMemory = runtime.totalMemory() - runtime.freeMemory()
        val maxMemory = runtime.maxMemory()
        
        if (usedMemory > maxMemory * 0.8) {
            clearCaches()
            System.gc()
        }
    }
    
    private fun clearCaches() {
        memoryCache.evictAll()
        imageLoader.memoryCache?.clear()
        
        // Clear Compose caches
        ComposeCache.clear()
    }
    
    companion object {
        fun getMemoryInfo(): MemoryInfo {
            val runtime = Runtime.getRuntime()
            return MemoryInfo(
                usedMemory = (runtime.totalMemory() - runtime.freeMemory()) / 1024 / 1024, // MB
                maxMemory = runtime.maxMemory() / 1024 / 1024, // MB
                availableMemory = runtime.freeMemory() / 1024 / 1024 // MB
            )
        }
    }
}
```

#### iOS Memory Management
```swift
// MemoryManager.swift
@available(iOS 15.0, *)
class MemoryManager: ObservableObject {
    @Published var memoryWarning = false
    
    private let imageCache = NSCache<NSString, UIImage>()
    private let logger = Logger(subsystem: "com.bitcraps.app", category: "MemoryManager")
    
    init() {
        setupImageCache()
        setupMemoryWarningObserver()
    }
    
    private func setupImageCache() {
        imageCache.countLimit = 50  // Maximum 50 images
        imageCache.totalCostLimit = 50 * 1024 * 1024  // 50MB total
    }
    
    private func setupMemoryWarningObserver() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleMemoryWarning),
            name: UIApplication.didReceiveMemoryWarningNotification,
            object: nil
        )
    }
    
    @objc private func handleMemoryWarning() {
        logger.warning("Memory warning received")
        clearCaches()
        memoryWarning = true
        
        // Reset warning after 5 seconds
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) {
            self.memoryWarning = false
        }
    }
    
    private func clearCaches() {
        imageCache.removeAllObjects()
        URLCache.shared.removeAllCachedResponses()
        
        // Clear SwiftUI caches if possible
        // Note: SwiftUI doesn't expose direct cache clearing APIs
    }
    
    func getMemoryInfo() -> MemoryInfo {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size)/4
        
        let result = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_, task_flavor_t(MACH_TASK_BASIC_INFO), $0, &count)
            }
        }
        
        if result == KERN_SUCCESS {
            return MemoryInfo(
                usedMemory: Double(info.resident_size) / (1024 * 1024), // MB
                maxMemory: Double(ProcessInfo.processInfo.physicalMemory) / (1024 * 1024), // MB
                availableMemory: 0 // iOS doesn't provide available memory easily
            )
        }
        
        return MemoryInfo(usedMemory: 0, maxMemory: 0, availableMemory: 0)
    }
}
```

### 3. Network Optimization

#### Bluetooth Optimization
```kotlin
// BluetoothOptimizer.kt
class BluetoothOptimizer(private val context: Context) {
    private var scanMode = ScanMode.BALANCED
    private var advertisingInterval = AdvertisingInterval.BALANCED
    
    fun optimizeForBatteryLevel(batteryLevel: Int) {
        when {
            batteryLevel < 20 -> {
                scanMode = ScanMode.LOW_POWER
                advertisingInterval = AdvertisingInterval.LOW_POWER
            }
            batteryLevel < 50 -> {
                scanMode = ScanMode.BALANCED
                advertisingInterval = AdvertisingInterval.BALANCED
            }
            else -> {
                scanMode = ScanMode.HIGH_PERFORMANCE
                advertisingInterval = AdvertisingInterval.HIGH_PERFORMANCE
            }
        }
        
        applyOptimizations()
    }
    
    private fun applyOptimizations() {
        val scanSettings = ScanSettings.Builder()
            .setScanMode(scanMode.value)
            .setCallbackType(ScanSettings.CALLBACK_TYPE_ALL_MATCHES)
            .build()
        
        val advertisingSettings = AdvertiseSettings.Builder()
            .setAdvertiseMode(advertisingInterval.mode)
            .setConnectable(true)
            .setTimeout(0) // Advertise indefinitely
            .setTxPowerLevel(advertisingInterval.txPower)
            .build()
        
        // Apply settings to BLE manager
        bleManager.updateScanSettings(scanSettings)
        bleManager.updateAdvertisingSettings(advertisingSettings)
    }
    
    enum class ScanMode(val value: Int) {
        LOW_POWER(ScanSettings.SCAN_MODE_LOW_POWER),
        BALANCED(ScanSettings.SCAN_MODE_BALANCED),
        HIGH_PERFORMANCE(ScanSettings.SCAN_MODE_LOW_LATENCY)
    }
    
    enum class AdvertisingInterval(
        val mode: Int,
        val txPower: Int
    ) {
        LOW_POWER(
            AdvertiseSettings.ADVERTISE_MODE_LOW_POWER,
            AdvertiseSettings.ADVERTISE_TX_POWER_LOW
        ),
        BALANCED(
            AdvertiseSettings.ADVERTISE_MODE_BALANCED,
            AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM
        ),
        HIGH_PERFORMANCE(
            AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY,
            AdvertiseSettings.ADVERTISE_TX_POWER_HIGH
        )
    }
}
```

### 4. Adaptive Performance System

#### Performance Mode Controller
```kotlin
// AdaptivePerformanceController.kt
class AdaptivePerformanceController(
    private val performanceMonitor: PerformanceMonitor,
    private val batteryOptimizationManager: BatteryOptimizationManager
) {
    private val _currentMode = MutableStateFlow(PerformanceMode.BALANCED)
    val currentMode: StateFlow<PerformanceMode> = _currentMode.asStateFlow()
    
    init {
        observePerformanceMetrics()
        observeBatteryState()
        observeThermalState()
    }
    
    private fun observePerformanceMetrics() {
        performanceMonitor.performanceState
            .debounce(2000) // Wait 2 seconds before reacting
            .onEach { state ->
                val recommendedMode = calculateOptimalMode(state)
                if (recommendedMode != _currentMode.value) {
                    _currentMode.value = recommendedMode
                    applyPerformanceMode(recommendedMode)
                }
            }
            .launchIn(GlobalScope)
    }
    
    private fun calculateOptimalMode(state: PerformanceState): PerformanceMode {
        val batteryState = batteryOptimizationManager.batteryState.value
        
        return when {
            // Critical conditions force battery saver
            batteryState.level < 15 || state.averageFps < 30 -> {
                PerformanceMode.BATTERY_SAVER
            }
            
            // Low battery or performance issues suggest balanced mode
            batteryState.level < 30 || state.averageFps < 45 || state.memoryUsageMB > 120 -> {
                PerformanceMode.BALANCED
            }
            
            // Good conditions allow high performance
            batteryState.level > 50 && state.averageFps > 55 && state.memoryUsageMB < 100 -> {
                PerformanceMode.HIGH_PERFORMANCE
            }
            
            // Default to balanced
            else -> PerformanceMode.BALANCED
        }
    }
    
    private fun applyPerformanceMode(mode: PerformanceMode) {
        when (mode) {
            PerformanceMode.HIGH_PERFORMANCE -> {
                setTargetFrameRate(60)
                setAnimationQuality(AnimationQuality.HIGH)
                setNetworkOptimizationLevel(0)
                setRenderingQuality(RenderingQuality.HIGH)
            }
            
            PerformanceMode.BALANCED -> {
                setTargetFrameRate(60)
                setAnimationQuality(AnimationQuality.MEDIUM)
                setNetworkOptimizationLevel(1)
                setRenderingQuality(RenderingQuality.MEDIUM)
            }
            
            PerformanceMode.BATTERY_SAVER -> {
                setTargetFrameRate(30)
                setAnimationQuality(AnimationQuality.LOW)
                setNetworkOptimizationLevel(2)
                setRenderingQuality(RenderingQuality.LOW)
            }
        }
    }
}

enum class PerformanceMode {
    HIGH_PERFORMANCE,
    BALANCED,
    BATTERY_SAVER
}

enum class AnimationQuality {
    HIGH,    // Full 2-second dice animations, smooth transitions
    MEDIUM,  // 1-second animations, reduced effects
    LOW      // 0.5-second animations, minimal effects
}

enum class RenderingQuality {
    HIGH,    // Full shadows, gradients, anti-aliasing
    MEDIUM,  // Basic shadows, simplified gradients
    LOW      // Flat colors, no shadows
}
```

---

## Performance Monitoring Dashboard

### Real-time Performance Metrics
```kotlin
// PerformanceMetricsView.kt (Android)
@Composable
fun PerformanceMetricsOverlay(
    performanceState: PerformanceState,
    isVisible: Boolean = BuildConfig.DEBUG
) {
    if (!isVisible) return
    
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.TopEnd
    ) {
        Card(
            modifier = Modifier
                .padding(16.dp)
                .width(200.dp),
            colors = CardDefaults.cardColors(
                containerColor = Color.Black.copy(alpha = 0.7f)
            )
        ) {
            Column(
                modifier = Modifier.padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(
                    text = "Performance",
                    color = Color.White,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Bold
                )
                
                MetricRow(
                    label = "FPS",
                    value = "${performanceState.averageFps.toInt()}",
                    isGood = performanceState.averageFps >= 55
                )
                
                MetricRow(
                    label = "Memory",
                    value = "${performanceState.memoryUsageMB.toInt()}MB",
                    isGood = performanceState.memoryUsageMB < 120
                )
                
                MetricRow(
                    label = "Battery",
                    value = "${performanceState.batteryDrainRate.toInt()}%/h",
                    isGood = performanceState.batteryDrainRate < 6.0
                )
                
                MetricRow(
                    label = "Network",
                    value = "${performanceState.networkUsageKBps.toInt()}KB/s",
                    isGood = performanceState.networkUsageKBps < 10.0
                )
            }
        }
    }
}

@Composable
private fun MetricRow(
    label: String,
    value: String,
    isGood: Boolean
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(
            text = label,
            color = Color.White,
            fontSize = 10.sp
        )
        
        Text(
            text = value,
            color = if (isGood) Color.Green else Color.Red,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold
        )
    }
}
```

---

## Performance Testing Framework

### Automated Performance Tests
```kotlin
// PerformanceTest.kt
@RunWith(AndroidJUnit4::class)
class PerformanceTest {
    
    @get:Rule
    val activityRule = ActivityScenarioRule(ComposeMainActivity::class.java)
    
    @Test
    fun testDiceAnimationPerformance() {
        val frameMetrics = FrameMetricsAggregator()
        
        activityRule.scenario.onActivity { activity ->
            frameMetrics.add(activity)
        }
        
        // Perform dice roll animation
        onView(withText("Roll Dice")).perform(click())
        
        // Wait for animation to complete
        Thread.sleep(3000)
        
        val metrics = frameMetrics.stop()
        val frameData = metrics[0]
        
        // Verify frame rate performance
        val slowFrames = countSlowFrames(frameData)
        val totalFrames = countTotalFrames(frameData)
        val slowFramePercentage = (slowFrames.toDouble() / totalFrames) * 100
        
        assertThat(slowFramePercentage).isLessThan(5.0) // Less than 5% slow frames
    }
    
    @Test
    fun testMemoryUsageDuringGameplay() {
        val initialMemory = getMemoryUsage()
        
        // Simulate 10 minutes of gameplay
        repeat(600) { // 600 seconds = 10 minutes
            performGameAction()
            Thread.sleep(1000)
            
            if (it % 60 == 0) { // Check memory every minute
                val currentMemory = getMemoryUsage()
                val memoryIncrease = currentMemory - initialMemory
                
                assertThat(memoryIncrease).isLessThan(50 * 1024 * 1024) // Less than 50MB increase
            }
        }
    }
    
    @Test
    fun testBatteryDrainRate() {
        val batteryTracker = BatteryTracker()
        batteryTracker.startTracking()
        
        // Simulate 1 hour of gameplay
        simulateGameplaySession(Duration.ofHours(1))
        
        val drainRate = batteryTracker.stopTracking()
        
        assertThat(drainRate.percentPerHour).isLessThan(6.0) // Less than 6% per hour
    }
    
    @Test
    fun testNetworkEfficiency() {
        val networkTracker = NetworkTracker()
        networkTracker.startTracking()
        
        // Simulate active gaming session
        simulateActiveGaming(Duration.ofMinutes(30))
        
        val usage = networkTracker.stopTracking()
        
        assertThat(usage.averageKBPerSecond).isLessThan(5.0) // Less than 5KB/s average
    }
    
    private fun performGameAction() {
        when ((0..3).random()) {
            0 -> onView(withText("Roll Dice")).perform(click())
            1 -> onView(withText("25")).perform(click()) // Place bet
            2 -> onView(withText("50")).perform(click()) // Place bet
            3 -> Thread.sleep(1000) // Idle time
        }
    }
}
```

---

## Performance Optimization Checklist

### Pre-Release Performance Validation

#### Android Checklist
- [ ] **Frame Rate**: Sustained 60fps during dice animations
- [ ] **Memory Usage**: Peak usage <150MB during gameplay
- [ ] **Battery Drain**: <5% per hour active gaming
- [ ] **Cold Start**: <2 seconds from tap to game ready
- [ ] **ANR Prevention**: No Application Not Responding events
- [ ] **Network Efficiency**: <500KB/hour during active gaming
- [ ] **Thermal Management**: No overheating during extended sessions

#### iOS Checklist
- [ ] **Frame Rate**: Sustained 60fps (120fps on ProMotion displays)
- [ ] **Memory Usage**: Peak usage <100MB during gameplay
- [ ] **Battery Drain**: <4% per hour active gaming
- [ ] **Launch Time**: <1.5 seconds cold start
- [ ] **Background Efficiency**: Minimal CPU usage when backgrounded
- [ ] **Metal Performance**: Hardware acceleration utilized
- [ ] **Memory Warnings**: Graceful handling of memory pressure

#### Cross-Platform Checklist
- [ ] **Feature Parity**: Identical performance characteristics
- [ ] **Protocol Efficiency**: Minimal bandwidth usage for sync
- [ ] **Connection Reliability**: <1% disconnect rate during games
- [ ] **State Synchronization**: <100ms sync latency
- [ ] **Error Recovery**: Graceful handling of network issues
- [ ] **Performance Monitoring**: Real-time metrics collection
- [ ] **Adaptive Optimization**: Automatic performance adjustments

---

## Performance Regression Prevention

### Continuous Performance Monitoring
```yaml
# .github/workflows/performance-monitoring.yml
name: Performance Monitoring

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  android-performance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Android Performance Testing
        uses: ./.github/actions/setup-android-performance
        
      - name: Run Performance Tests
        run: ./gradlew performanceTest
        
      - name: Upload Performance Report
        uses: actions/upload-artifact@v3
        with:
          name: android-performance-report
          path: app/build/reports/performance/
  
  ios-performance:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup iOS Performance Testing
        run: |
          xcode-select --install
          xcrun simctl create "iPhone 14" "iPhone 14"
          
      - name: Run Performance Tests
        run: |
          xcodebuild test \
            -project ios/BitCraps.xcodeproj \
            -scheme BitCrapsPerformanceTests \
            -destination "platform=iOS Simulator,name=iPhone 14"
            
      - name: Parse Performance Results
        run: ./scripts/parse-ios-performance.sh
```

### Performance Regression Detection
- **Baseline Metrics**: Establish performance baselines for each release
- **Automated Testing**: Run performance tests on every commit
- **Threshold Alerts**: Alert when performance degrades beyond acceptable limits
- **Performance History**: Track performance trends over time
- **Automated Rollback**: Revert commits that cause significant performance regression

---

This comprehensive performance optimization guide ensures BitCraps mobile applications deliver exceptional performance across all supported platforms while maintaining battery efficiency and network optimization. The adaptive performance system automatically adjusts to device capabilities and user conditions, providing the best possible gaming experience.