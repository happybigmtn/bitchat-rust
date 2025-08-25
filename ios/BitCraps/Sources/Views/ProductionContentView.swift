import SwiftUI
import CoreBluetooth
import Combine

@available(iOS 15.0, *)
struct ProductionContentView: View {
    @StateObject private var gameManager = GameManager()
    @StateObject private var bluetoothManager = BluetoothManager()
    @State private var showingSettings = false
    @State private var showingGameLobby = false
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 20) {
                    // Header with Dynamic Island support
                    HeaderView(gameManager: gameManager)
                    
                    // System Status Card
                    SystemStatusCard(
                        bluetoothManager: bluetoothManager,
                        gameManager: gameManager
                    )
                    
                    // Permission Controls
                    if !bluetoothManager.hasRequiredPermissions {
                        PermissionCard(bluetoothManager: bluetoothManager)
                    }
                    
                    // Game Interface
                    if gameManager.isInGame {
                        GamePlayView(gameManager: gameManager)
                    } else {
                        GameLobbyCard(gameManager: gameManager)
                    }
                    
                    // Performance Metrics (Debug builds only)
                    #if DEBUG
                    PerformanceMetricsCard(gameManager: gameManager)
                    #endif
                }
                .padding()
            }
            .navigationBarHidden(true)
            .background(
                LinearGradient(
                    gradient: Gradient(colors: [
                        Color("BackgroundPrimary"),
                        Color("BackgroundSecondary")
                    ]),
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
            )
        }
        .sheet(isPresented: $showingSettings) {
            SettingsView(bluetoothManager: bluetoothManager)
        }
        .sheet(isPresented: $showingGameLobby) {
            GameLobbyView(gameManager: gameManager)
        }
    }
}

@available(iOS 15.0, *)
struct HeaderView: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("BitCraps")
                    .font(.largeTitle)
                    .fontWeight(.black)
                    .foregroundColor(.primary)
                
                Text("Mesh Gaming Network")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            VStack(alignment: .trailing, spacing: 2) {
                if gameManager.isInGame {
                    Text("Game Active")
                        .font(.caption)
                        .foregroundColor(.green)
                        .fontWeight(.semibold)
                    
                    Text("\(gameManager.playerCount) players")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                } else {
                    Text("Ready to Play")
                        .font(.caption)
                        .foregroundColor(.blue)
                        .fontWeight(.semibold)
                }
            }
        }
        .padding(.horizontal)
    }
}

@available(iOS 15.0, *)
struct SystemStatusCard: View {
    @ObservedObject var bluetoothManager: BluetoothManager
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("System Status")
                .font(.headline)
                .fontWeight(.semibold)
            
            LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 2), spacing: 12) {
                StatusIndicator(
                    title: "Bluetooth",
                    status: bluetoothManager.isBluetoothEnabled ? .good : .error,
                    icon: "bluetooth"
                )
                
                StatusIndicator(
                    title: "Permissions",
                    status: bluetoothManager.hasRequiredPermissions ? .good : .warning,
                    icon: "lock.shield"
                )
                
                StatusIndicator(
                    title: "Discovery",
                    status: bluetoothManager.isScanning ? .good : .idle,
                    icon: "antenna.radiowaves.left.and.right"
                )
                
                StatusIndicator(
                    title: "Peers",
                    status: bluetoothManager.discoveredPeers.count > 0 ? .good : .idle,
                    icon: "person.2",
                    detail: "\(bluetoothManager.discoveredPeers.count)"
                )
            }
            
            if gameManager.isInGame {
                Divider()
                
                HStack {
                    Label("Game Session", systemImage: "gamecontroller")
                        .font(.subheadline)
                        .fontWeight(.medium)
                    
                    Spacer()
                    
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("Balance: \(gameManager.balance) chips")
                            .font(.caption)
                            .fontWeight(.semibold)
                        
                        if gameManager.currentBet > 0 {
                            Text("Bet: \(gameManager.currentBet)")
                                .font(.caption2)
                                .foregroundColor(.orange)
                        }
                    }
                }
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color("CardBackground"))
                .shadow(color: .black.opacity(0.1), radius: 8, x: 0, y: 4)
        )
    }
}

@available(iOS 15.0, *)
struct StatusIndicator: View {
    let title: String
    let status: StatusType
    let icon: String
    let detail: String?
    
    init(title: String, status: StatusType, icon: String, detail: String? = nil) {
        self.title = title
        self.status = status
        self.icon = icon
        self.detail = detail
    }
    
    enum StatusType {
        case good, warning, error, idle
        
        var color: Color {
            switch self {
            case .good: return .green
            case .warning: return .orange
            case .error: return .red
            case .idle: return .gray
            }
        }
    }
    
    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .foregroundColor(status.color)
                .font(.system(size: 16, weight: .semibold))
            
            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.caption)
                    .fontWeight(.medium)
                
                if let detail = detail {
                    Text(detail)
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
            
            Spacer()
        }
        .padding(.vertical, 4)
    }
}

@available(iOS 15.0, *)
struct PermissionCard: View {
    @ObservedObject var bluetoothManager: BluetoothManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.orange)
                
                Text("Setup Required")
                    .font(.headline)
                    .fontWeight(.semibold)
            }
            
            Text("BitCraps needs Bluetooth permissions to discover and connect with nearby players.")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            Button(action: {
                bluetoothManager.requestPermissions()
            }) {
                HStack {
                    Image(systemName: "lock.shield")
                    Text("Grant Permissions")
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding()
                .background(Color.blue)
                .foregroundColor(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color.orange.opacity(0.1))
                .overlay(
                    RoundedRectangle(cornerRadius: 16)
                        .stroke(Color.orange.opacity(0.3), lineWidth: 1)
                )
        )
    }
}

@available(iOS 15.0, *)
struct GameLobbyCard: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Game Lobby")
                .font(.headline)
                .fontWeight(.semibold)
            
            Text("Start playing with nearby devices or create a new game session.")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            HStack(spacing: 12) {
                Button(action: {
                    gameManager.createGame()
                }) {
                    VStack {
                        Image(systemName: "plus.circle.fill")
                            .font(.title2)
                        Text("Create Game")
                            .font(.caption)
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.green)
                    .foregroundColor(.white)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
                
                Button(action: {
                    gameManager.joinGame()
                }) {
                    VStack {
                        Image(systemName: "person.2.fill")
                            .font(.title2)
                        Text("Join Game")
                            .font(.caption)
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.blue)
                    .foregroundColor(.white)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
            }
            
            if gameManager.availableGames.count > 0 {
                Divider()
                
                Text("Available Games")
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                ForEach(gameManager.availableGames, id: \.id) { game in
                    GameRow(game: game) {
                        gameManager.joinSpecificGame(game)
                    }
                }
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color("CardBackground"))
                .shadow(color: .black.opacity(0.1), radius: 8, x: 0, y: 4)
        )
    }
}

@available(iOS 15.0, *)
struct GameRow: View {
    let game: AvailableGame
    let onJoin: () -> Void
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(game.hostName)
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                Text("\(game.playerCount) players • \(game.signal)dBm")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            Button("Join", action: onJoin)
                .buttonStyle(.borderProminent)
                .controlSize(.small)
        }
        .padding(.vertical, 4)
    }
}

@available(iOS 15.0, *)
struct GamePlayView: View {
    @ObservedObject var gameManager: GameManager
    @State private var showingDiceAnimation = false
    
    var body: some View {
        VStack(spacing: 20) {
            // Game Status
            GameStatusHeader(gameManager: gameManager)
            
            // Dice Display
            DiceDisplayView(
                dice1: gameManager.dice1,
                dice2: gameManager.dice2,
                isRolling: gameManager.isRolling
            )
            
            // Betting Interface
            BettingInterfaceView(gameManager: gameManager)
            
            // Game Controls
            GameControlsView(gameManager: gameManager)
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color("GameTableBackground"))
                .shadow(color: .black.opacity(0.2), radius: 12, x: 0, y: 6)
        )
    }
}

@available(iOS 15.0, *)
struct GameStatusHeader: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("Craps Game")
                    .font(.headline)
                    .fontWeight(.bold)
                
                Text(gameManager.gamePhaseDescription)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            VStack(alignment: .trailing, spacing: 2) {
                Text("\(gameManager.balance) chips")
                    .font(.title3)
                    .fontWeight(.bold)
                    .foregroundColor(.primary)
                
                if let point = gameManager.point {
                    Text("Point: \(point)")
                        .font(.caption)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                        .background(Color.orange)
                        .foregroundColor(.white)
                        .clipShape(Capsule())
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct DiceDisplayView: View {
    let dice1: Int
    let dice2: Int
    let isRolling: Bool
    
    @State private var rotationAngle: Double = 0
    @State private var scale: Double = 1.0
    
    var body: some View {
        HStack(spacing: 30) {
            DieView(value: dice1, isRolling: isRolling)
            DieView(value: dice2, isRolling: isRolling)
        }
        .scaleEffect(scale)
        .rotationEffect(.degrees(rotationAngle))
        .onAppear {
            if isRolling {
                withAnimation(.easeInOut(duration: 0.5).repeatForever()) {
                    rotationAngle = 360
                }
                withAnimation(.easeInOut(duration: 0.3).repeatForever(autoreverses: true)) {
                    scale = 1.1
                }
            }
        }
        .onChange(of: isRolling) { rolling in
            if rolling {
                withAnimation(.easeInOut(duration: 0.5).repeatForever()) {
                    rotationAngle = 360
                }
                withAnimation(.easeInOut(duration: 0.3).repeatForever(autoreverses: true)) {
                    scale = 1.1
                }
            } else {
                withAnimation(.easeOut(duration: 0.5)) {
                    rotationAngle = 0
                    scale = 1.0
                }
            }
        }
        
        if dice1 > 0 && dice2 > 0 && !isRolling {
            Text("Total: \(dice1 + dice2)")
                .font(.title2)
                .fontWeight(.bold)
                .padding(.top)
        }
    }
}

@available(iOS 15.0, *)
struct DieView: View {
    let value: Int
    let isRolling: Bool
    
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 12)
                .fill(
                    LinearGradient(
                        gradient: Gradient(colors: [Color.white, Color.gray.opacity(0.8)]),
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: 60, height: 60)
                .shadow(color: .black.opacity(0.3), radius: 4, x: 2, y: 2)
            
            if isRolling {
                Text("?")
                    .font(.title)
                    .fontWeight(.bold)
                    .foregroundColor(.black)
            } else if value > 0 {
                DieDotsView(value: value)
            }
        }
    }
}

@available(iOS 15.0, *)
struct DieDotsView: View {
    let value: Int
    
    var body: some View {
        let positions = getDotPositions(for: value)
        
        ZStack {
            ForEach(0..<positions.count, id: \.self) { index in
                Circle()
                    .fill(Color.black)
                    .frame(width: 8, height: 8)
                    .position(positions[index])
            }
        }
        .frame(width: 60, height: 60)
    }
    
    private func getDotPositions(for value: Int) -> [CGPoint] {
        let margin: CGFloat = 15
        let center: CGFloat = 30
        let far: CGFloat = 45
        
        switch value {
        case 1:
            return [CGPoint(x: center, y: center)]
        case 2:
            return [
                CGPoint(x: margin, y: margin),
                CGPoint(x: far, y: far)
            ]
        case 3:
            return [
                CGPoint(x: margin, y: margin),
                CGPoint(x: center, y: center),
                CGPoint(x: far, y: far)
            ]
        case 4:
            return [
                CGPoint(x: margin, y: margin),
                CGPoint(x: far, y: margin),
                CGPoint(x: margin, y: far),
                CGPoint(x: far, y: far)
            ]
        case 5:
            return [
                CGPoint(x: margin, y: margin),
                CGPoint(x: far, y: margin),
                CGPoint(x: center, y: center),
                CGPoint(x: margin, y: far),
                CGPoint(x: far, y: far)
            ]
        case 6:
            return [
                CGPoint(x: margin, y: margin),
                CGPoint(x: far, y: margin),
                CGPoint(x: margin, y: center),
                CGPoint(x: far, y: center),
                CGPoint(x: margin, y: far),
                CGPoint(x: far, y: far)
            ]
        default:
            return []
        }
    }
}

@available(iOS 15.0, *)
struct BettingInterfaceView: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Place Your Bet")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Spacer()
                
                if gameManager.currentBet > 0 {
                    Text("Current: \(gameManager.currentBet)")
                        .font(.caption)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 2)
                        .background(Color.blue.opacity(0.2))
                        .clipShape(Capsule())
                }
            }
            
            LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 4), spacing: 8) {
                ForEach([10, 25, 50, 100], id: \.self) { amount in
                    Button(action: {
                        gameManager.placeBet(amount)
                    }) {
                        VStack {
                            Text("$\(amount)")
                                .font(.subheadline)
                                .fontWeight(.semibold)
                            
                            Text("chips")
                                .font(.caption2)
                        }
                        .frame(height: 50)
                        .frame(maxWidth: .infinity)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(gameManager.balance >= amount ? Color.green : Color.gray.opacity(0.3))
                        )
                        .foregroundColor(gameManager.balance >= amount ? .white : .gray)
                    }
                    .disabled(gameManager.balance < amount)
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct GameControlsView: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(spacing: 12) {
            Button(action: {
                gameManager.rollDice()
            }) {
                HStack {
                    Image(systemName: gameManager.isRolling ? "arrow.clockwise" : "dice")
                        .font(.title3)
                    
                    Text(gameManager.isRolling ? "Rolling..." : "Roll Dice")
                        .font(.headline)
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding()
                .background(
                    RoundedRectangle(cornerRadius: 12)
                        .fill(gameManager.canRoll && !gameManager.isRolling ? Color.red : Color.gray.opacity(0.3))
                )
                .foregroundColor(.white)
            }
            .disabled(!gameManager.canRoll || gameManager.isRolling)
            
            HStack(spacing: 12) {
                Button("Pass Line") {
                    gameManager.placeBet(gameManager.currentBet)
                }
                .buttonStyle(.bordered)
                .disabled(gameManager.currentBet == 0)
                
                Button("Don't Pass") {
                    gameManager.placeDontPassBet(gameManager.currentBet)
                }
                .buttonStyle(.bordered)
                .disabled(gameManager.currentBet == 0)
                
                Button("Leave Game") {
                    gameManager.leaveGame()
                }
                .buttonStyle(.bordered)
                .foregroundColor(.red)
            }
            .font(.subheadline)
        }
    }
}

@available(iOS 15.0, *)
struct PerformanceMetricsCard: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Performance Metrics")
                .font(.subheadline)
                .fontWeight(.semibold)
            
            HStack {
                MetricView(title: "Frame Rate", value: "\(gameManager.frameRate)fps")
                MetricView(title: "Memory", value: "\(gameManager.memoryUsage)MB")
                MetricView(title: "Battery", value: "\(gameManager.batteryDrain)%/h")
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.gray.opacity(0.1))
        )
    }
}

@available(iOS 15.0, *)
struct MetricView: View {
    let title: String
    let value: String
    
    var body: some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.caption)
                .fontWeight(.semibold)
            Text(title)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
    }
}

@available(iOS 15.0, *)
struct ProductionContentView_Previews: PreviewProvider {
    static var previews: some View {
        ProductionContentView()
    }
}