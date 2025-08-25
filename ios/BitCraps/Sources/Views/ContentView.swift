import SwiftUI
import CoreBluetooth

@available(iOS 13.0, *)
struct ContentView: View {
    @StateObject private var bluetoothManager = BluetoothManager()
    @State private var showingBackgroundReport = false
    @State private var useProductionUI = true
    
    var body: some View {
        if useProductionUI && ProcessInfo.processInfo.environment["XCODE_RUNNING_FOR_PREVIEWS"] != "1" {
            if #available(iOS 15.0, *) {
                ProductionContentView()
            } else {
                LegacyContentView(bluetoothManager: bluetoothManager, showingBackgroundReport: $showingBackgroundReport)
            }
        } else {
            LegacyContentView(bluetoothManager: bluetoothManager, showingBackgroundReport: $showingBackgroundReport)
        }
    }
}

@available(iOS 13.0, *)
struct LegacyContentView: View {
    @ObservedObject var bluetoothManager: BluetoothManager
    @Binding var showingBackgroundReport: Bool
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                // Header
                Text("BitCraps iOS")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text("Bluetooth LE Validation")
                    .font(.headline)
                    .foregroundColor(.secondary)
                
                // Status Section
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Circle()
                            .fill(bluetoothManager.isScanning ? Color.green : Color.red)
                            .frame(width: 12, height: 12)
                        Text("Scanning: \(bluetoothManager.isScanning ? "Active" : "Inactive")")
                    }
                    
                    HStack {
                        Circle()
                            .fill(bluetoothManager.isAdvertising ? Color.green : Color.red)
                            .frame(width: 12, height: 12)
                        Text("Advertising: \(bluetoothManager.isAdvertising ? "Active" : "Inactive")")
                    }
                    
                    HStack {
                        Circle()
                            .fill(Color.blue)
                            .frame(width: 12, height: 12)
                        Text("Discovered: \(bluetoothManager.discoveredPeers.count) peers")
                    }
                    
                    HStack {
                        Circle()
                            .fill(Color.orange)
                            .frame(width: 12, height: 12)
                        Text("Connected: \(bluetoothManager.connectedPeers.count) peers")
                    }
                }
                .padding()
                .background(Color.gray.opacity(0.1))
                .cornerRadius(10)
                
                // Control Buttons
                VStack(spacing: 12) {
                    HStack(spacing: 12) {
                        Button(action: {
                            bluetoothManager.startDiscovery()
                        }) {
                            Text("Start Scan")
                                .frame(maxWidth: .infinity)
                                .padding()
                                .background(Color.blue)
                                .foregroundColor(.white)
                                .cornerRadius(10)
                        }
                        .disabled(bluetoothManager.isScanning)
                        
                        Button(action: {
                            bluetoothManager.stopDiscovery()
                        }) {
                            Text("Stop Scan")
                                .frame(maxWidth: .infinity)
                                .padding()
                                .background(Color.red)
                                .foregroundColor(.white)
                                .cornerRadius(10)
                        }
                        .disabled(!bluetoothManager.isScanning)
                    }
                    
                    HStack(spacing: 12) {
                        Button(action: {
                            bluetoothManager.startAdvertising()
                        }) {
                            Text("Start Advertise")
                                .frame(maxWidth: .infinity)
                                .padding()
                                .background(Color.green)
                                .foregroundColor(.white)
                                .cornerRadius(10)
                        }
                        .disabled(bluetoothManager.isAdvertising)
                        
                        Button(action: {
                            bluetoothManager.stopAdvertising()
                        }) {
                            Text("Stop Advertise")
                                .frame(maxWidth: .infinity)
                                .padding()
                                .background(Color.orange)
                                .foregroundColor(.white)
                                .cornerRadius(10)
                        }
                        .disabled(!bluetoothManager.isAdvertising)
                    }
                }
                
                // Background Limitations Report Button
                if bluetoothManager.backgroundLimitations != nil {
                    Button(action: {
                        showingBackgroundReport = true
                    }) {
                        Text("View Background BLE Report")
                            .frame(maxWidth: .infinity)
                            .padding()
                            .background(Color.purple)
                            .foregroundColor(.white)
                            .cornerRadius(10)
                    }
                }
                
                // Discovered Peers List
                if !bluetoothManager.discoveredPeers.isEmpty {
                    VStack(alignment: .leading) {
                        Text("Discovered Peers")
                            .font(.headline)
                            .padding(.horizontal)
                        
                        List(bluetoothManager.discoveredPeers) { peer in
                            PeerRow(peer: peer) {
                                bluetoothManager.connect(to: peer)
                            }
                        }
                        .frame(maxHeight: 200)
                    }
                }
                
                Spacer()
            }
            .padding()
            .navigationBarHidden(true)
        }
        .sheet(isPresented: $showingBackgroundReport) {
            if let report = bluetoothManager.backgroundLimitations {
                BackgroundLimitationsView(report: report)
            }
        }
    }
}

@available(iOS 13.0, *)
struct PeerRow: View {
    let peer: DiscoveredPeer
    let onConnect: () -> Void
    
    var body: some View {
        HStack {
            VStack(alignment: .leading) {
                Text(peer.id.uuidString.prefix(8))
                    .font(.headline)
                Text("RSSI: \(peer.rssi) dBm")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text("Last seen: \(formatTime(peer.lastSeen))")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            Button("Connect", action: onConnect)
                .buttonStyle(BorderedButtonStyle())
        }
    }
    
    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.timeStyle = .medium
        return formatter.string(from: date)
    }
}

@available(iOS 13.0, *)
struct BackgroundLimitationsView: View {
    let report: BackgroundLimitationReport
    @Environment(\.presentationMode) var presentationMode
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    
                    // Go/No-Go Decision
                    VStack(alignment: .leading, spacing: 8) {
                        Text("KILL-SWITCH VALIDATION")
                            .font(.title2)
                            .fontWeight(.bold)
                            .foregroundColor(.primary)
                        
                        HStack {
                            Circle()
                                .fill(report.isViableForAlwaysOnGaming ? Color.green : Color.red)
                                .frame(width: 16, height: 16)
                            
                            Text(report.isViableForAlwaysOnGaming ? "GO: iOS BLE viable for BitCraps" : "NO-GO: iOS BLE limitations too severe")
                                .font(.headline)
                                .foregroundColor(report.isViableForAlwaysOnGaming ? .green : .red)
                        }
                    }
                    .padding()
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(10)
                    
                    // Limitations Summary
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Background BLE Limitations")
                            .font(.title3)
                            .fontWeight(.semibold)
                        
                        LimitationRow(title: "Service UUID filtering required", 
                                      value: report.serviceUUIDFilteringRequired, 
                                      severity: .warning)
                        
                        LimitationRow(title: "Local name unavailable in background", 
                                      value: report.localNameUnavailableInBackground, 
                                      severity: .critical)
                        
                        LimitationRow(title: "Scan result coalescing active", 
                                      value: report.scanResultCoalescingActive, 
                                      severity: .info)
                    }
                    .padding()
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(10)
                    
                    // Performance Metrics
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Discovery Performance")
                            .font(.title3)
                            .fontWeight(.semibold)
                        
                        HStack {
                            Text("Background rate:")
                            Spacer()
                            Text("\(String(format: "%.2f", report.backgroundDiscoveryRate * 60)) peers/min")
                                .fontWeight(.semibold)
                        }
                        
                        HStack {
                            Text("Foreground rate:")
                            Spacer()
                            Text("\(String(format: "%.2f", report.foregroundDiscoveryRate * 60)) peers/min")
                                .fontWeight(.semibold)
                        }
                    }
                    .padding()
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(10)
                    
                    // Recommendations
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Implementation Recommendations")
                            .font(.title3)
                            .fontWeight(.semibold)
                        
                        ForEach(Array(report.recommendedImplementation.enumerated()), id: \.offset) { index, recommendation in
                            HStack(alignment: .top) {
                                Text("\(index + 1).")
                                    .fontWeight(.semibold)
                                    .foregroundColor(.secondary)
                                
                                Text(recommendation)
                                    .foregroundColor(recommendation.contains("KILL-SWITCH") ? .red : 
                                                   recommendation.contains("CRITICAL") ? .orange : .primary)
                                    .fontWeight(recommendation.contains("KILL-SWITCH") || recommendation.contains("CRITICAL") ? .semibold : .regular)
                            }
                        }
                    }
                    .padding()
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(10)
                    
                    // Test Results Summary
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Test Results")
                            .font(.title3)
                            .fontWeight(.semibold)
                        
                        HStack {
                            Text("Background discoveries:")
                            Spacer()
                            Text("\(report.testResults.backgroundDiscoveryCount)")
                        }
                        
                        HStack {
                            Text("Foreground discoveries:")
                            Spacer()
                            Text("\(report.testResults.foregroundDiscoveryCount)")
                        }
                        
                        HStack {
                            Text("Background transitions:")
                            Spacer()
                            Text("\(report.testResults.backgroundTransitions)")
                        }
                    }
                    .padding()
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(10)
                }
                .padding()
            }
            .navigationTitle("BLE Background Report")
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarItems(trailing: Button("Done") {
                presentationMode.wrappedValue.dismiss()
            })
        }
    }
}

@available(iOS 13.0, *)
struct LimitationRow: View {
    let title: String
    let value: Bool
    let severity: LimitationSeverity
    
    enum LimitationSeverity {
        case info, warning, critical
        
        var color: Color {
            switch self {
            case .info: return .blue
            case .warning: return .orange
            case .critical: return .red
            }
        }
        
        var icon: String {
            switch self {
            case .info: return "info.circle"
            case .warning: return "exclamationmark.triangle"
            case .critical: return "xmark.circle"
            }
        }
    }
    
    var body: some View {
        HStack {
            Image(systemName: severity.icon)
                .foregroundColor(severity.color)
            
            Text(title)
            
            Spacer()
            
            Text(value ? "YES" : "NO")
                .fontWeight(.semibold)
                .foregroundColor(value ? severity.color : .green)
        }
    }
}

@available(iOS 13.0, *)
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}