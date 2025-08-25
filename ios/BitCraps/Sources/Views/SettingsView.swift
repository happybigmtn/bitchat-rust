import SwiftUI
import CoreBluetooth

@available(iOS 15.0, *)
struct SettingsView: View {
    @ObservedObject var bluetoothManager: BluetoothManager
    @Environment(\.dismiss) private var dismiss
    
    @State private var enableHapticFeedback = true
    @State private var enableSoundEffects = true
    @State private var enablePerformanceMode = false
    @State private var maxBatteryDrain = 5.0
    @State private var showingDebugInfo = false
    
    var body: some View {
        NavigationView {
            Form {
                Section("Bluetooth Settings") {
                    BluetoothSettingsSection(bluetoothManager: bluetoothManager)
                }
                
                Section("Game Settings") {
                    GameSettingsSection(
                        enableHapticFeedback: $enableHapticFeedback,
                        enableSoundEffects: $enableSoundEffects,
                        enablePerformanceMode: $enablePerformanceMode
                    )
                }
                
                Section("Battery Optimization") {
                    BatteryOptimizationSection(
                        maxBatteryDrain: $maxBatteryDrain,
                        bluetoothManager: bluetoothManager
                    )
                }
                
                Section("Debug Information") {
                    DebugInformationSection(
                        showingDebugInfo: $showingDebugInfo,
                        bluetoothManager: bluetoothManager
                    )
                }
                
                Section("About") {
                    AboutSection()
                }
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct BluetoothSettingsSection: View {
    @ObservedObject var bluetoothManager: BluetoothManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label("Bluetooth Status", systemImage: "bluetooth")
                Spacer()
                Text(bluetoothManager.isBluetoothEnabled ? "Enabled" : "Disabled")
                    .foregroundColor(bluetoothManager.isBluetoothEnabled ? .green : .red)
                    .fontWeight(.medium)
            }
            
            HStack {
                Label("Scanning", systemImage: "antenna.radiowaves.left.and.right")
                Spacer()
                if bluetoothManager.isScanning {
                    Button("Stop") {
                        bluetoothManager.stopDiscovery()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                } else {
                    Button("Start") {
                        bluetoothManager.startDiscovery()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            
            HStack {
                Label("Advertising", systemImage: "broadcast")
                Spacer()
                if bluetoothManager.isAdvertising {
                    Button("Stop") {
                        bluetoothManager.stopAdvertising()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                } else {
                    Button("Start") {
                        bluetoothManager.startAdvertising()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }
            
            if bluetoothManager.discoveredPeers.count > 0 {
                HStack {
                    Label("Discovered Peers", systemImage: "person.2")
                    Spacer()
                    Text("\(bluetoothManager.discoveredPeers.count)")
                        .fontWeight(.medium)
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct GameSettingsSection: View {
    @Binding var enableHapticFeedback: Bool
    @Binding var enableSoundEffects: Bool
    @Binding var enablePerformanceMode: Bool
    
    var body: some View {
        Toggle("Haptic Feedback", isOn: $enableHapticFeedback)
        Toggle("Sound Effects", isOn: $enableSoundEffects)
        Toggle("Performance Mode", isOn: $enablePerformanceMode)
        
        if enablePerformanceMode {
            Text("Performance mode reduces visual effects to save battery and improve frame rate.")
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }
}

@available(iOS 15.0, *)
struct BatteryOptimizationSection: View {
    @Binding var maxBatteryDrain: Double
    @ObservedObject var bluetoothManager: BluetoothManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Max Battery Drain", systemImage: "battery.25")
                Spacer()
                Text("\(Int(maxBatteryDrain))% per hour")
                    .fontWeight(.medium)
            }
            
            Slider(value: $maxBatteryDrain, in: 1...10, step: 1) {
                Text("Battery Drain Limit")
            } minimumValueLabel: {
                Text("1%")
                    .font(.caption)
            } maximumValueLabel: {
                Text("10%")
                    .font(.caption)
            }
        }
        
        Button("Optimize Battery Settings") {
            optimizeBatterySettings()
        }
        .buttonStyle(.bordered)
    }
    
    private func optimizeBatterySettings() {
        // TODO: Implement battery optimization
    }
}

@available(iOS 15.0, *)
struct DebugInformationSection: View {
    @Binding var showingDebugInfo: Bool
    @ObservedObject var bluetoothManager: BluetoothManager
    
    var body: some View {
        Toggle("Show Debug Info", isOn: $showingDebugInfo)
        
        if showingDebugInfo {
            VStack(alignment: .leading, spacing: 4) {
                DebugInfoRow(label: "Central State", value: bluetoothManager.centralManager?.state.description ?? "Unknown")
                DebugInfoRow(label: "Peripheral State", value: bluetoothManager.peripheralManager?.state.description ?? "Unknown")
                DebugInfoRow(label: "Background Limitations", value: bluetoothManager.backgroundLimitations != nil ? "Active" : "None")
                DebugInfoRow(label: "App Version", value: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "Unknown")
                DebugInfoRow(label: "Build", value: Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "Unknown")
                DebugInfoRow(label: "iOS Version", value: UIDevice.current.systemVersion)
                DebugInfoRow(label: "Device Model", value: UIDevice.current.model)
            }
            .padding()
            .background(Color.gray.opacity(0.1))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }
}

@available(iOS 15.0, *)
struct DebugInfoRow: View {
    let label: String
    let value: String
    
    var body: some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
            Spacer()
            Text(value)
                .font(.caption)
                .fontWeight(.medium)
        }
    }
}

@available(iOS 15.0, *)
struct AboutSection: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Version", systemImage: "info.circle")
                Spacer()
                Text(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0")
                    .fontWeight(.medium)
            }
            
            HStack {
                Label("Build", systemImage: "hammer")
                Spacer()
                Text(Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "1")
                    .fontWeight(.medium)
            }
            
            Button("View Privacy Policy") {
                // TODO: Open privacy policy
            }
            .buttonStyle(.bordered)
            
            Button("Report Issue") {
                // TODO: Open issue reporting
            }
            .buttonStyle(.bordered)
        }
    }
}

// Extension for CBManagerState description
extension CBManagerState {
    var description: String {
        switch self {
        case .unknown: return "Unknown"
        case .resetting: return "Resetting"
        case .unsupported: return "Unsupported"
        case .unauthorized: return "Unauthorized"
        case .poweredOff: return "Powered Off"
        case .poweredOn: return "Powered On"
        @unknown default: return "Unknown"
        }
    }
}

@available(iOS 15.0, *)
struct SettingsView_Previews: PreviewProvider {
    static var previews: some View {
        SettingsView(bluetoothManager: BluetoothManager())
    }
}