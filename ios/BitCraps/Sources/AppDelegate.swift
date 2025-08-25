import UIKit
import os.log

@main
class AppDelegate: UIResponder, UIApplicationDelegate {

    private let logger = OSLog(subsystem: "com.bitcraps.ios", category: "AppDelegate")

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        
        os_log("BitCraps iOS app launched", log: logger, type: .info)
        
        // Configure app for BLE background operation
        configureBackgroundModes()
        
        // Initialize core components
        initializeBitCrapsCore()
        
        return true
    }
    
    private func configureBackgroundModes() {
        // Ensure proper background mode configuration
        guard let backgroundModes = Bundle.main.object(forInfoDictionaryKey: "UIBackgroundModes") as? [String] else {
            os_log("WARNING: No background modes configured", log: logger, type: .error)
            return
        }
        
        if backgroundModes.contains("bluetooth-central") {
            os_log("Bluetooth central background mode enabled", log: logger, type: .info)
        } else {
            os_log("WARNING: Bluetooth central background mode not configured", log: logger, type: .error)
        }
    }
    
    private func initializeBitCrapsCore() {
        // Initialize Rust core library
        // This would interface with the Rust FFI layer
        os_log("Initializing BitCraps core library", log: logger, type: .info)
    }
    
    // MARK: - UISceneSession Lifecycle (iOS 13+)
    
    func application(_ application: UIApplication, configurationForConnecting connectingSceneSession: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        return UISceneConfiguration(name: "Default Configuration", sessionRole: connectingSceneSession.role)
    }
    
    func application(_ application: UIApplication, didDiscardSceneSessions sceneSessions: Set<UISceneSession>) {
        // Handle scene cleanup
    }
    
    // MARK: - Background Task Management
    
    func applicationDidEnterBackground(_ application: UIApplication) {
        os_log("App entered background - BLE limitations now active", log: logger, type: .info)
        
        // Request background task to maintain BLE operations
        var backgroundTaskID = UIApplication.shared.beginBackgroundTask {
            // Clean up when background time expires
            UIApplication.shared.endBackgroundTask(UIBackgroundTaskIdentifier.invalid)
        }
        
        // Schedule task cleanup
        DispatchQueue.main.asyncAfter(deadline: .now() + 25) { // iOS allows ~30 seconds
            if backgroundTaskID != .invalid {
                UIApplication.shared.endBackgroundTask(backgroundTaskID)
                backgroundTaskID = .invalid
            }
        }
    }
    
    func applicationWillEnterForeground(_ application: UIApplication) {
        os_log("App entering foreground - full BLE capabilities restored", log: logger, type: .info)
    }
    
    func applicationWillTerminate(_ application: UIApplication) {
        os_log("App terminating - cleaning up BLE operations", log: logger, type: .info)
        
        // Clean up Rust resources
        // bitcraps_stop_node() would be called here
    }
}