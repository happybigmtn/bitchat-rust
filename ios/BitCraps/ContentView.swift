import SwiftUI
import CoreBluetooth

struct ContentView: View {
    @StateObject private var viewModel = BitCrapsViewModel()
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                // Status Card
                VStack(alignment: .leading, spacing: 10) {
                    Text("Status")
                        .font(.headline)
                    
                    HStack {
                        Circle()
                            .fill(viewModel.isBluetoothEnabled ? Color.green : Color.red)
                            .frame(width: 10, height: 10)
                        Text("Bluetooth: \(viewModel.isBluetoothEnabled ? "Enabled" : "Disabled")")
                            .font(.subheadline)
                    }
                    
                    HStack {
                        Circle()
                            .fill(viewModel.isDiscovering ? Color.green : Color.gray)
                            .frame(width: 10, height: 10)
                        Text("Discovery: \(viewModel.isDiscovering ? "Active" : "Inactive")")
                            .font(.subheadline)
                    }
                    
                    Text("Connected Peers: \(viewModel.connectedPeers.count)")
                        .font(.subheadline)
                }
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(.systemGray6))
                .cornerRadius(10)
                
                // Discovery Controls
                HStack(spacing: 15) {
                    Button(action: {
                        viewModel.startDiscovery()
                    }) {
                        Text("Start Discovery")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(!viewModel.isBluetoothEnabled || viewModel.isDiscovering)
                    
                    Button(action: {
                        viewModel.stopDiscovery()
                    }) {
                        Text("Stop Discovery")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .disabled(!viewModel.isDiscovering)
                }
                
                // Game Controls
                HStack(spacing: 15) {
                    Button(action: {
                        viewModel.createGame()
                    }) {
                        Text("Create Game")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(viewModel.connectedPeers.isEmpty)
                    
                    Button(action: {
                        viewModel.joinGame()
                    }) {
                        Text("Join Game")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    .disabled(viewModel.connectedPeers.isEmpty)
                }
                
                // Peers List
                if !viewModel.connectedPeers.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Connected Peers")
                            .font(.headline)
                        
                        ForEach(viewModel.connectedPeers, id: \.peerId) { peer in
                            HStack {
                                Image(systemName: "person.circle.fill")
                                    .foregroundColor(.blue)
                                VStack(alignment: .leading) {
                                    Text(peer.name)
                                        .font(.subheadline)
                                    Text(String(peer.peerId.prefix(8)) + "...")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                                Spacer()
                                Image(systemName: "wifi")
                                    .foregroundColor(connectionQualityColor(peer.connectionQuality))
                            }
                            .padding(.vertical, 5)
                        }
                    }
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(.systemGray6))
                    .cornerRadius(10)
                }
                
                Spacer()
            }
            .padding()
            .navigationTitle("BitCraps")
            .alert("Error", isPresented: $viewModel.showError) {
                Button("OK", role: .cancel) { }
            } message: {
                Text(viewModel.errorMessage)
            }
        }
    }
    
    func connectionQualityColor(_ quality: ConnectionQuality) -> Color {
        switch quality {
        case .excellent:
            return .green
        case .good:
            return .yellow
        case .fair:
            return .orange
        case .poor:
            return .red
        }
    }
}

class BitCrapsViewModel: ObservableObject {
    @Published var isBluetoothEnabled = false
    @Published var isDiscovering = false
    @Published var connectedPeers: [PeerInfo] = []
    @Published var showError = false
    @Published var errorMessage = ""
    
    private var bitcrapsNode: BitCrapsNode?
    private var centralManager: CBCentralManager?
    private var peripheralManager: CBPeripheralManager?
    
    init() {
        setupBitCraps()
        setupBluetooth()
    }
    
    private func setupBitCraps() {
        do {
            // Create configuration
            let config = BitCrapsConfig(
                bluetoothName: "BitCraps-iOS",
                enableBatteryOptimization: true,
                maxPeers: 10,
                discoveryTimeoutSeconds: 30
            )
            
            // Create node
            bitcrapsNode = try createNode(config: config)
            
            // Start event polling
            startEventPolling()
            
        } catch {
            showError(message: "Failed to initialize BitCraps: \(error)")
        }
    }
    
    private func setupBluetooth() {
        centralManager = CBCentralManager(delegate: nil, queue: nil)
        peripheralManager = CBPeripheralManager(delegate: nil, queue: nil)
        
        // Check Bluetooth state
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
            self?.checkBluetoothState()
        }
    }
    
    private func checkBluetoothState() {
        isBluetoothEnabled = centralManager?.state == .poweredOn
    }
    
    func startDiscovery() {
        Task {
            do {
                try await bitcrapsNode?.startDiscovery()
                await MainActor.run {
                    isDiscovering = true
                }
            } catch {
                await MainActor.run {
                    showError(message: "Failed to start discovery: \(error)")
                }
            }
        }
    }
    
    func stopDiscovery() {
        Task {
            do {
                try await bitcrapsNode?.stopDiscovery()
                await MainActor.run {
                    isDiscovering = false
                }
            } catch {
                await MainActor.run {
                    showError(message: "Failed to stop discovery: \(error)")
                }
            }
        }
    }
    
    func createGame() {
        Task {
            do {
                let config = GameConfig(
                    minBet: 10,
                    maxBet: 1000,
                    playerLimit: 4,
                    timeoutSeconds: 30,
                    allowSpectators: true
                )
                
                let gameHandle = try await bitcrapsNode?.createGame(config: config)
                // Navigate to game view
                
            } catch {
                await MainActor.run {
                    showError(message: "Failed to create game: \(error)")
                }
            }
        }
    }
    
    func joinGame() {
        // Show game selection sheet
    }
    
    private func startEventPolling() {
        Task {
            while true {
                do {
                    if let events = await bitcrapsNode?.drainEvents() {
                        for event in events {
                            handleEvent(event)
                        }
                    }
                    
                    // Update peers
                    if let peers = bitcrapsNode?.getConnectedPeers() {
                        await MainActor.run {
                            connectedPeers = peers
                        }
                    }
                    
                    try await Task.sleep(nanoseconds: 1_000_000_000) // 1 second
                    
                } catch {
                    // Handle error silently
                }
            }
        }
    }
    
    private func handleEvent(_ event: GameEvent) {
        Task { @MainActor in
            switch event {
            case .peerDiscovered(let peer):
                print("Peer discovered: \(peer.peerId)")
            case .peerConnected(let peerId):
                print("Peer connected: \(peerId)")
            case .gameCreated(let gameId):
                print("Game created: \(gameId)")
            default:
                break
            }
        }
    }
    
    private func showError(message: String) {
        errorMessage = message
        showError = true
    }
}

// Placeholder types until UniFFI generates them
struct PeerInfo {
    let peerId: String
    let name: String
    let reputation: UInt32
    let gamesPlayed: UInt32
    let connectionQuality: ConnectionQuality
}

enum ConnectionQuality {
    case excellent
    case good
    case fair
    case poor
}

struct BitCrapsConfig {
    let bluetoothName: String
    let enableBatteryOptimization: Bool
    let maxPeers: UInt32
    let discoveryTimeoutSeconds: UInt32
}

struct GameConfig {
    let minBet: UInt64
    let maxBet: UInt64
    let playerLimit: Int
    let timeoutSeconds: UInt32
    let allowSpectators: Bool
}

enum GameEvent {
    case peerDiscovered(PeerInfo)
    case peerConnected(String)
    case gameCreated(String)
}

// Placeholder functions until UniFFI generates them
class BitCrapsNode {
    func startDiscovery() async throws {}
    func stopDiscovery() async throws {}
    func createGame(config: GameConfig) async throws -> GameHandle? { nil }
    func drainEvents() async -> [GameEvent] { [] }
    func getConnectedPeers() -> [PeerInfo] { [] }
}

struct GameHandle {}

func createNode(config: BitCrapsConfig) throws -> BitCrapsNode {
    return BitCrapsNode()
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}