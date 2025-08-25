import Foundation
import UIKit
import Combine
import os.log

/// Comprehensive battery optimization and monitoring for BitCraps iOS gaming
@available(iOS 15.0, *)
class BatteryOptimizationManager: ObservableObject {
    private let logger = Logger(subsystem: "com.bitcraps.app", category: "BatteryOptimizationManager")
    
    // Configuration
    private let monitoringInterval: TimeInterval = 30.0 // 30 seconds
    private let criticalBatteryThreshold: Float = 0.15 // 15%
    private let lowBatteryThreshold: Float = 0.30 // 30%
    private let highDrainRateThreshold: Double = 8.0 // %/hour
    private let optimizationCheckInterval: TimeInterval = 300.0 // 5 minutes
    
    // Published state
    @Published var batteryState: BatteryState = BatteryState.unknown()
    @Published var optimizationState: OptimizationState = OptimizationState.normal()
    @Published var powerProfile: PowerProfile = .balanced
    
    // Internal monitoring
    private var batteryHistory: [BatteryReading] = []
    private var lastOptimizationCheck: TimeInterval = 0
    private var baselineConsumption: Double = 0
    
    // Adaptive features
    private var isGamingMode = false
    private var adaptiveFrameRate = 60
    private var networkOptimizationLevel = 0
    
    // System monitoring
    private var cancellables = Set<AnyCancellable>()
    private var monitoringTimer: Timer?
    private var optimizationTimer: Timer?
    
    // Notifications
    private let notificationCenter = NotificationCenter.default
    
    init() {
        setupBatteryMonitoring()
        setupNotificationObservers()
        checkInitialOptimizationState()
        logger.info("BatteryOptimizationManager initialized")
    }
    
    deinit {
        cleanup()
    }
    
    // MARK: - Public API
    
    func setGameMode(_ enabled: Bool) {
        isGamingMode = enabled
        if enabled {
            applyGamingOptimizations()
        } else {
            restoreNormalMode()
        }
        logger.info("Gaming mode: \(enabled ? "enabled" : "disabled")")
    }
    
    func setPowerProfile(_ profile: PowerProfile) {
        powerProfile = profile
        applyPowerProfile(profile)
        logger.info("Power profile changed to: \(profile)")
    }
    
    func checkBatteryOptimizationStatus() -> BatteryOptimizationStatus {
        // iOS doesn't have direct battery optimization settings like Android
        // Check for Low Power Mode instead
        if ProcessInfo.processInfo.isLowPowerModeEnabled {
            return .systemPowerSaveMode
        }
        
        // Check if app is backgrounded frequently (indicator of aggressive power management)
        return .normal
    }
    
    func getOptimizationRecommendations() -> [OptimizationRecommendation] {
        var recommendations: [OptimizationRecommendation] = []
        
        // Low battery recommendations
        if batteryState.level < lowBatteryThreshold {
            recommendations.append(
                OptimizationRecommendation(
                    type: .reduceVisualEffects,
                    title: "Reduce Visual Effects",
                    description: "Lower frame rate and disable animations to save battery",
                    impact: .medium,
                    action: { [weak self] in self?.applyLowPowerOptimizations() }
                )
            )
        }
        
        // High drain rate recommendations
        if batteryState.drainRate > highDrainRateThreshold {
            recommendations.append(
                OptimizationRecommendation(
                    type: .optimizeNetwork,
                    title: "Optimize Network Usage",
                    description: "Reduce Bluetooth scan frequency and optimize mesh networking",
                    impact: .medium,
                    action: { [weak self] in self?.optimizeNetworkUsage() }
                )
            )
        }
        
        // Low Power Mode recommendation
        if !ProcessInfo.processInfo.isLowPowerModeEnabled && batteryState.level < lowBatteryThreshold {
            recommendations.append(
                OptimizationRecommendation(
                    type: .enableLowPowerMode,
                    title: "Enable Low Power Mode",
                    description: "System-wide battery saving features",
                    impact: .high,
                    action: { [weak self] in self?.suggestLowPowerMode() }
                )
            )
        }
        
        // Background App Refresh
        recommendations.append(
            OptimizationRecommendation(
                type: .optimizeBackgroundRefresh,
                title: "Optimize Background Refresh",
                description: "Ensure BitCraps can run in background for continuous gaming",
                impact: .high,
                action: { [weak self] in self?.checkBackgroundAppRefresh() }
            )
        )
        
        return recommendations
    }
    
    func applyAllRecommendations() {
        let recommendations = getOptimizationRecommendations()
        for recommendation in recommendations {
            recommendation.action?()
        }
    }
    
    // MARK: - Battery Monitoring
    
    private func setupBatteryMonitoring() {
        UIDevice.current.isBatteryMonitoringEnabled = true
        
        // Start monitoring timer
        monitoringTimer = Timer.scheduledTimer(withTimeInterval: monitoringInterval, repeats: true) { [weak self] _ in
            self?.updateBatteryState()
        }
        
        // Start optimization check timer
        optimizationTimer = Timer.scheduledTimer(withTimeInterval: optimizationCheckInterval, repeats: true) { [weak self] _ in
            self?.checkForOptimizations()
        }
        
        // Initial update
        updateBatteryState()
    }
    
    private func setupNotificationObservers() {
        // Battery state changes
        notificationCenter.publisher(for: UIDevice.batteryStateDidChangeNotification)
            .sink { [weak self] _ in
                self?.updateBatteryState()
            }
            .store(in: &cancellables)
        
        notificationCenter.publisher(for: UIDevice.batteryLevelDidChangeNotification)
            .sink { [weak self] _ in
                self?.updateBatteryState()
            }
            .store(in: &cancellables)
        
        // Low Power Mode changes
        notificationCenter.publisher(for: .NSProcessInfoPowerStateDidChange)
            .sink { [weak self] _ in
                self?.handlePowerStateChange()
            }
            .store(in: &cancellables)
        
        // App lifecycle
        notificationCenter.publisher(for: UIApplication.didEnterBackgroundNotification)
            .sink { [weak self] _ in
                self?.handleAppDidEnterBackground()
            }
            .store(in: &cancellables)
        
        notificationCenter.publisher(for: UIApplication.willEnterForegroundNotification)
            .sink { [weak self] _ in
                self?.handleAppWillEnterForeground()
            }
            .store(in: &cancellables)
    }
    
    private func updateBatteryState() {
        let device = UIDevice.current
        let level = device.batteryLevel // -1 if unknown, otherwise 0.0-1.0
        let isCharging = device.batteryState == .charging || device.batteryState == .full
        let drainRate = calculateDrainRate()
        
        let reading = BatteryReading(
            timestamp: Date().timeIntervalSince1970,
            level: level,
            isCharging: isCharging
        )
        
        // Add to history
        batteryHistory.append(reading)
        if batteryHistory.count > 120 { // Keep last 120 readings (1 hour at 30s intervals)
            batteryHistory.removeFirst()
        }
        
        let newState = BatteryState(
            level: level,
            isCharging: isCharging,
            drainRate: drainRate,
            status: getBatteryStatus(level: level, isCharging: isCharging, drainRate: drainRate),
            timeRemaining: calculateTimeRemaining(level: level, drainRate: drainRate, isCharging: isCharging),
            isLowPowerMode: ProcessInfo.processInfo.isLowPowerModeEnabled,
            thermalState: ProcessInfo.processInfo.thermalState,
            lastUpdated: Date().timeIntervalSince1970
        )
        
        DispatchQueue.main.async {
            self.batteryState = newState
        }
    }
    
    private func calculateDrainRate() -> Double {
        guard batteryHistory.count >= 2 else { return 0.0 }
        
        let recent = Array(batteryHistory.suffix(10)) // Last 10 readings (5 minutes)
        guard recent.count >= 2 else { return 0.0 }
        
        let firstReading = recent.first!
        let lastReading = recent.last!
        
        let timeDiff = (lastReading.timestamp - firstReading.timestamp) / 3600.0 // hours
        let levelDiff = firstReading.level - lastReading.level // positive = draining
        
        guard timeDiff > 0, levelDiff > 0, !firstReading.isCharging, !lastReading.isCharging else {
            return 0.0
        }
        
        return Double(levelDiff) / timeDiff * 100.0 // %/hour
    }
    
    private func getBatteryStatus(level: Float, isCharging: Bool, drainRate: Double) -> BatteryStatus {
        if level < 0 {
            return .unknown
        }
        
        switch true {
        case isCharging:
            return .charging
        case level < criticalBatteryThreshold:
            return .critical
        case level < lowBatteryThreshold:
            return .low
        case drainRate > highDrainRateThreshold:
            return .highDrain
        default:
            return .normal
        }
    }
    
    private func calculateTimeRemaining(level: Float, drainRate: Double, isCharging: Bool) -> Double {
        if level < 0 || isCharging || drainRate <= 0 {
            return -1.0 // Unknown/infinite
        }
        
        return Double(level * 100) / drainRate // hours
    }
    
    // MARK: - Optimizations
    
    private func checkForOptimizations() {
        let now = Date().timeIntervalSince1970
        guard now - lastOptimizationCheck >= optimizationCheckInterval else { return }
        
        lastOptimizationCheck = now
        
        // Auto-apply critical optimizations
        switch batteryState.status {
        case .critical:
            applyEmergencyOptimizations()
        case .low:
            applyLowPowerOptimizations()
        case .highDrain:
            optimizeNetworkUsage()
        default:
            break
        }
        
        // Handle thermal state
        if ProcessInfo.processInfo.thermalState == .critical {
            applyThermalOptimizations()
        }
    }
    
    private func applyGamingOptimizations() {
        let profile: PowerProfile
        
        switch batteryState.level {
        case ..<criticalBatteryThreshold:
            profile = .ultraBatterySaver
        case criticalBatteryThreshold..<lowBatteryThreshold:
            profile = .batterySaver
        default:
            profile = .gaming
        }
        
        setPowerProfile(profile)
    }
    
    private func restoreNormalMode() {
        setPowerProfile(.balanced)
    }
    
    private func applyPowerProfile(_ profile: PowerProfile) {
        switch profile {
        case .gaming:
            adaptiveFrameRate = 60
            networkOptimizationLevel = 0
        case .balanced:
            adaptiveFrameRate = 60
            networkOptimizationLevel = 1
        case .batterySaver:
            adaptiveFrameRate = 30
            networkOptimizationLevel = 2
        case .ultraBatterySaver:
            adaptiveFrameRate = 15
            networkOptimizationLevel = 3
        }
        
        updateOptimizationState()
    }
    
    private func applyEmergencyOptimizations() {
        logger.warning("Applying emergency battery optimizations")
        setPowerProfile(.ultraBatterySaver)
        optimizeNetworkUsage()
        reduceBackgroundActivity()
    }
    
    private func applyLowPowerOptimizations() {
        logger.info("Applying low power optimizations")
        if powerProfile == .gaming {
            setPowerProfile(.batterySaver)
        }
    }
    
    private func applyThermalOptimizations() {
        logger.warning("Applying thermal optimizations")
        adaptiveFrameRate = min(adaptiveFrameRate, 30)
        networkOptimizationLevel = max(networkOptimizationLevel, 2)
        updateOptimizationState()
    }
    
    private func optimizeNetworkUsage() {
        networkOptimizationLevel = min(networkOptimizationLevel + 1, 3)
        updateOptimizationState()
        logger.info("Network optimization level: \(networkOptimizationLevel)")
    }
    
    private func reduceBackgroundActivity() {
        updateOptimizationState()
        logger.info("Reduced background activity")
    }
    
    private func updateOptimizationState() {
        let newState = OptimizationState(
            targetFrameRate: adaptiveFrameRate,
            networkOptimizationLevel: networkOptimizationLevel,
            backgroundActivityReduced: networkOptimizationLevel > 1,
            visualEffectsReduced: adaptiveFrameRate < 60,
            isOptimized: adaptiveFrameRate < 60 || networkOptimizationLevel > 0,
            lastOptimization: Date().timeIntervalSince1970
        )
        
        DispatchQueue.main.async {
            self.optimizationState = newState
        }
    }
    
    // MARK: - System Integration
    
    private func suggestLowPowerMode() {
        // iOS doesn't allow programmatic enabling of Low Power Mode
        // Show user guidance instead
        logger.info("Suggesting user enable Low Power Mode")
        
        // This would typically show an alert or notification to the user
        // directing them to Control Center or Settings
    }
    
    private func checkBackgroundAppRefresh() {
        // Check if Background App Refresh is enabled for this app
        let backgroundRefreshStatus = UIApplication.shared.backgroundRefreshStatus
        
        switch backgroundRefreshStatus {
        case .available:
            logger.info("Background App Refresh is available")
        case .denied:
            logger.warning("Background App Refresh is denied")
            // Could show user guidance to enable it
        case .restricted:
            logger.warning("Background App Refresh is restricted")
        @unknown default:
            logger.warning("Background App Refresh status unknown")
        }
    }
    
    // MARK: - Event Handlers
    
    private func handlePowerStateChange() {
        updateBatteryState()
        
        if ProcessInfo.processInfo.isLowPowerModeEnabled {
            logger.info("System Low Power Mode enabled")
            applyLowPowerOptimizations()
        } else {
            logger.info("System Low Power Mode disabled")
            if isGamingMode {
                applyGamingOptimizations()
            }
        }
    }
    
    private func handleAppDidEnterBackground() {
        logger.info("App entered background")
        // Reduce monitoring frequency in background
        monitoringTimer?.invalidate()
        monitoringTimer = Timer.scheduledTimer(withTimeInterval: monitoringInterval * 2, repeats: true) { [weak self] _ in
            self?.updateBatteryState()
        }
    }
    
    private func handleAppWillEnterForeground() {
        logger.info("App will enter foreground")
        // Resume normal monitoring frequency
        monitoringTimer?.invalidate()
        monitoringTimer = Timer.scheduledTimer(withTimeInterval: monitoringInterval, repeats: true) { [weak self] _ in
            self?.updateBatteryState()
        }
        
        // Immediate update
        updateBatteryState()
    }
    
    private func checkInitialOptimizationState() {
        updateBatteryState()
        checkForOptimizations()
    }
    
    private func cleanup() {
        monitoringTimer?.invalidate()
        optimizationTimer?.invalidate()
        cancellables.removeAll()
        batteryHistory.removeAll()
        UIDevice.current.isBatteryMonitoringEnabled = false
    }
}

// MARK: - Data Models

struct BatteryState {
    let level: Float // -1 if unknown, otherwise 0.0-1.0
    let isCharging: Bool
    let drainRate: Double // %/hour
    let status: BatteryStatus
    let timeRemaining: Double // hours, -1 if unknown
    let isLowPowerMode: Bool
    let thermalState: ProcessInfo.ThermalState
    let lastUpdated: TimeInterval
    
    static func unknown() -> BatteryState {
        return BatteryState(
            level: -1,
            isCharging: false,
            drainRate: 0.0,
            status: .unknown,
            timeRemaining: -1.0,
            isLowPowerMode: false,
            thermalState: .nominal,
            lastUpdated: 0
        )
    }
}

struct OptimizationState {
    let targetFrameRate: Int
    let networkOptimizationLevel: Int
    let backgroundActivityReduced: Bool
    let visualEffectsReduced: Bool
    let isOptimized: Bool
    let lastOptimization: TimeInterval
    
    static func normal() -> OptimizationState {
        return OptimizationState(
            targetFrameRate: 60,
            networkOptimizationLevel: 0,
            backgroundActivityReduced: false,
            visualEffectsReduced: false,
            isOptimized: false,
            lastOptimization: 0
        )
    }
}

struct BatteryReading {
    let timestamp: TimeInterval
    let level: Float
    let isCharging: Bool
}

struct OptimizationRecommendation {
    let type: RecommendationType
    let title: String
    let description: String
    let impact: OptimizationImpact
    let action: (() -> Void)?
}

enum BatteryStatus {
    case unknown
    case normal
    case low
    case critical
    case highDrain
    case charging
}

enum PowerProfile {
    case gaming
    case balanced
    case batterySaver
    case ultraBatterySaver
}

enum BatteryOptimizationStatus {
    case normal
    case systemPowerSaveMode
    case backgroundRefreshDisabled
}

enum RecommendationType {
    case enableLowPowerMode
    case reduceVisualEffects
    case optimizeNetwork
    case optimizeBackgroundRefresh
}

enum OptimizationImpact {
    case low
    case medium
    case high
}