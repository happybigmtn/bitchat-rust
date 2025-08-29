import SwiftUI
import BitCrapsSDK
import Combine

/**
 * BitCraps Sample App
 * 
 * Demonstrates how to integrate and use the BitCraps iOS SDK
 * 
 * Features showcased:
 * - SDK initialization and configuration
 * - Peer discovery and connection management
 * - Game creation and participation
 * - Real-time event handling
 * - Error handling and recovery
 * - Performance monitoring
 */

@main
struct BitCrapsSampleApp: App {
    @StateObject private var gameManager = GameManager()
    @StateObject private var settingsManager = SettingsManager()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(gameManager)
                .environmentObject(settingsManager)
                .onAppear {
                    Task {
                        await gameManager.initializeSDK()
                    }
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                    gameManager.handleAppWillEnterForeground()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.didEnterBackgroundNotification)) { _ in
                    gameManager.handleAppDidEnterBackground()
                }
        }
    }
}

/**
 * Main content view with navigation
 */
struct ContentView: View {
    @EnvironmentObject private var gameManager: GameManager
    @EnvironmentObject private var settingsManager: SettingsManager
    @State private var selectedTab = 0
    
    var body: some View {
        TabView(selection: $selectedTab) {
            // Home / Discovery Tab
            HomeView()
                .tabItem {
                    Image(systemName: "house.fill")
                    Text("Home")
                }
                .tag(0)
            
            // Peers Tab
            PeersView()
                .tabItem {
                    Image(systemName: "person.2.fill")
                    Text("Peers")
                }
                .tag(1)
                .badge(gameManager.discoveredPeers.count)
            
            // Game Tab
            if gameManager.currentGameSession != nil {
                GameView()
                    .tabItem {
                        Image(systemName: "dice.fill")
                        Text("Game")
                    }
                    .tag(2)
            }
            
            // Statistics Tab
            StatsView()
                .tabItem {
                    Image(systemName: "chart.bar.fill")
                    Text("Stats")
                }
                .tag(3)
            
            // Settings Tab
            SettingsView()
                .tabItem {
                    Image(systemName: "gear")
                    Text("Settings")
                }
                .tag(4)
        }
        .onChange(of: gameManager.currentGameSession) { session in
            // Navigate to game tab when joining a game
            if session != nil {
                selectedTab = 2
            }
        }
        .alert("Error", isPresented: .constant(gameManager.lastError != nil)) {
            Button("OK") {
                gameManager.clearError()
            }
            Button("Retry") {
                gameManager.retryLastOperation()
            }
        } message: {
            Text(gameManager.lastError?.displayMessage ?? "Unknown error occurred")
        }
        .sheet(isPresented: $gameManager.showingInitializationError) {
            InitializationErrorView()
        }
    }
}

/**
 * Home view for discovery and game management
 */
struct HomeView: View {
    @EnvironmentObject private var gameManager: GameManager
    @State private var showingCreateGameSheet = false
    @State private var showingJoinGameSheet = false
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 20) {
                    // Status Card
                    StatusCard()
                    
                    // Discovery Section
                    DiscoverySection()
                    
                    // Quick Actions
                    QuickActionsSection(
                        onCreateGame: { showingCreateGameSheet = true },
                        onJoinGame: { showingJoinGameSheet = true }
                    )
                    
                    // Available Games
                    AvailableGamesSection()
                    
                    // Recent Events
                    RecentEventsSection()
                }
                .padding()
            }
            .navigationTitle("BitCraps")
            .navigationBarTitleDisplayMode(.large)
            .refreshable {
                await gameManager.refreshAvailableGames()
            }
        }
        .sheet(isPresented: $showingCreateGameSheet) {
            CreateGameSheet()
        }
        .sheet(isPresented: $showingJoinGameSheet) {
            JoinGameSheet()
        }
    }
}

/**
 * Status card showing current node status
 */
struct StatusCard: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: gameManager.nodeStatus?.state.isOperational == true ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                    .foregroundColor(gameManager.nodeStatus?.state.isOperational == true ? .green : .orange)
                    .font(.title2)
                
                Text("Node Status")
                    .font(.headline)
                
                Spacer()
                
                Text(gameManager.nodeStatus?.state.displayName ?? "Unknown")
                    .font(.caption)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.secondary.opacity(0.2))
                    .cornerRadius(8)
            }
            
            HStack {
                VStack(alignment: .leading) {
                    Text("Discovered Peers")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text("\(gameManager.discoveredPeers.count)")
                        .font(.title3)
                        .fontWeight(.semibold)
                }
                
                Spacer()
                
                VStack(alignment: .trailing) {
                    Text("Connected")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text("\(gameManager.connectionStatus.connectedPeerCount)")
                        .font(.title3)
                        .fontWeight(.semibold)
                }
                
                Spacer()
                
                VStack(alignment: .trailing) {
                    Text("Connection Quality")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text(gameManager.connectionStatus.connectionQuality.displayName)
                        .font(.title3)
                        .fontWeight(.semibold)
                        .foregroundColor(Color(gameManager.connectionStatus.connectionQuality.color))
                }
            }
        }
        .padding()
        .background(Color(UIColor.systemBackground))
        .cornerRadius(12)
        .shadow(radius: 2)
    }
}

/**
 * Discovery controls section
 */
struct DiscoverySection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Peer Discovery")
                    .font(.headline)
                
                Spacer()
                
                if gameManager.isDiscovering {
                    ProgressView()
                        .scaleEffect(0.8)
                }
            }
            
            HStack(spacing: 12) {
                Button(action: {
                    Task {
                        if gameManager.isDiscovering {
                            await gameManager.stopDiscovery()
                        } else {
                            await gameManager.startDiscovery()
                        }
                    }
                }) {
                    HStack {
                        Image(systemName: gameManager.isDiscovering ? "stop.circle.fill" : "play.circle.fill")
                        Text(gameManager.isDiscovering ? "Stop Discovery" : "Start Discovery")
                    }
                    .foregroundColor(.white)
                    .padding()
                    .background(gameManager.isDiscovering ? Color.red : Color.blue)
                    .cornerRadius(8)
                }
                
                Button("Refresh") {
                    Task {
                        await gameManager.refreshPeers()
                    }
                }
                .padding()
                .background(Color.secondary.opacity(0.2))
                .cornerRadius(8)
            }
            
            // Power Mode Selector
            Picker("Power Mode", selection: .constant(gameManager.nodeStatus?.currentPowerMode ?? .balanced)) {
                ForEach(PowerMode.allCases, id: \.self) { mode in
                    Text(mode.displayName).tag(mode)
                }
            }
            .pickerStyle(SegmentedPickerStyle())
            .onChange(of: gameManager.nodeStatus?.currentPowerMode) { newMode in
                if let mode = newMode {
                    gameManager.setPowerMode(mode)
                }
            }
        }
        .padding()
        .background(Color(UIColor.systemBackground))
        .cornerRadius(12)
        .shadow(radius: 2)
    }
}

/**
 * Quick actions section
 */
struct QuickActionsSection: View {
    let onCreateGame: () -> Void
    let onJoinGame: () -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Quick Actions")
                .font(.headline)
            
            HStack(spacing: 12) {
                Button(action: onCreateGame) {
                    VStack {
                        Image(systemName: "plus.circle.fill")
                            .font(.title)
                        Text("Create Game")
                            .font(.caption)
                    }
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.green)
                    .cornerRadius(12)
                }
                
                Button(action: onJoinGame) {
                    VStack {
                        Image(systemName: "person.2.fill")
                            .font(.title)
                        Text("Join Game")
                            .font(.caption)
                    }
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.blue)
                    .cornerRadius(12)
                }
            }
        }
        .padding()
        .background(Color(UIColor.systemBackground))
        .cornerRadius(12)
        .shadow(radius: 2)
    }
}

/**
 * Available games section
 */
struct AvailableGamesSection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Available Games")
                    .font(.headline)
                
                Spacer()
                
                Text("\(gameManager.availableGames.count)")
                    .font(.caption)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Color.secondary.opacity(0.2))
                    .cornerRadius(8)
            }
            
            if gameManager.availableGames.isEmpty {
                VStack {
                    Image(systemName: "gamecontroller")
                        .font(.title)
                        .foregroundColor(.secondary)
                    Text("No games available")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Text("Start discovery to find games")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding()
            } else {
                LazyVStack(spacing: 8) {
                    ForEach(gameManager.availableGames) { game in
                        AvailableGameRow(game: game) {
                            Task {
                                await gameManager.joinGame(gameId: game.id)
                            }
                        }
                    }
                }
            }
        }
        .padding()
        .background(Color(UIColor.systemBackground))
        .cornerRadius(12)
        .shadow(radius: 2)
    }
}

/**
 * Single available game row
 */
struct AvailableGameRow: View {
    let game: AvailableGame
    let onJoin: () -> Void
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Image(systemName: game.gameType.iconName)
                        .foregroundColor(.blue)
                    Text(game.gameType.displayName)
                        .font(.headline)
                    
                    if game.requiresPassword {
                        Image(systemName: "lock.fill")
                            .foregroundColor(.orange)
                            .font(.caption)
                    }
                }
                
                Text("Host: \(game.hostDisplayName ?? game.hostPeerId)")
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                HStack {
                    Text("Players: \(game.currentPlayers)/\(game.maxPlayers)")
                        .font(.caption2)
                    
                    Spacer()
                    
                    Text("Bet: \(game.minBet)-\(game.maxBet)")
                        .font(.caption2)
                }
                .foregroundColor(.secondary)
            }
            
            Spacer()
            
            VStack {
                Circle()
                    .fill(Color(game.gameState.status.color))
                    .frame(width: 10, height: 10)
                
                Button("Join") {
                    onJoin()
                }
                .font(.caption)
                .padding(.horizontal, 12)
                .padding(.vertical, 4)
                .background(game.canJoin ? Color.blue : Color.secondary)
                .foregroundColor(.white)
                .cornerRadius(8)
                .disabled(!game.canJoin)
            }
        }
        .padding()
        .background(Color.secondary.opacity(0.1))
        .cornerRadius(8)
    }
}

/**
 * Recent events section
 */
struct RecentEventsSection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Recent Events")
                .font(.headline)
            
            if gameManager.recentEvents.isEmpty {
                VStack {
                    Image(systemName: "clock")
                        .font(.title2)
                        .foregroundColor(.secondary)
                    Text("No recent events")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding()
            } else {
                LazyVStack(alignment: .leading, spacing: 6) {
                    ForEach(gameManager.recentEvents.prefix(5)) { event in
                        EventRow(event: event)
                    }
                }
            }
        }
        .padding()
        .background(Color(UIColor.systemBackground))
        .cornerRadius(12)
        .shadow(radius: 2)
    }
}

/**
 * Single event row
 */
struct EventRow: View {
    let event: GameEvent
    
    var body: some View {
        HStack {
            Circle()
                .fill(Color.blue)
                .frame(width: 6, height: 6)
            
            Text(event.displayDescription)
                .font(.caption)
                .lineLimit(2)
            
            Spacer()
            
            Text(event.timestamp.formatted(.relative(presentation: .abbreviated)))
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .padding(.vertical, 2)
    }
}

// MARK: - Additional Views

/**
 * Create game sheet
 */
struct CreateGameSheet: View {
    @EnvironmentObject private var gameManager: GameManager
    @Environment(\.dismiss) private var dismiss
    
    @State private var gameType: GameType = .craps
    @State private var gameName = ""
    @State private var minBet: Int64 = 1
    @State private var maxBet: Int64 = 100
    @State private var maxPlayers = 8
    @State private var timeoutMinutes = 5
    @State private var requireBiometric = false
    @State private var enableChat = true
    @State private var privateGame = false
    @State private var password = ""
    
    var body: some View {
        NavigationView {
            Form {
                Section("Game Details") {
                    Picker("Game Type", selection: $gameType) {
                        ForEach(GameType.allCases, id: \.self) { type in
                            Text(type.displayName).tag(type)
                        }
                    }
                    
                    TextField("Game Name (Optional)", text: $gameName)
                }
                
                Section("Betting Limits") {
                    HStack {
                        Text("Minimum Bet")
                        Spacer()
                        TextField("Min", value: $minBet, format: .number)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 80)
                    }
                    
                    HStack {
                        Text("Maximum Bet")
                        Spacer()
                        TextField("Max", value: $maxBet, format: .number)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 80)
                    }
                }
                
                Section("Game Settings") {
                    Stepper("Max Players: \(maxPlayers)", value: $maxPlayers, in: 2...16)
                    Stepper("Timeout: \(timeoutMinutes) min", value: $timeoutMinutes, in: 1...60)
                }
                
                Section("Security & Privacy") {
                    Toggle("Require Biometric Auth", isOn: $requireBiometric)
                    Toggle("Enable Chat", isOn: $enableChat)
                    Toggle("Private Game", isOn: $privateGame)
                    
                    if privateGame {
                        SecureField("Password", text: $password)
                    }
                }
            }
            .navigationTitle("Create Game")
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarBackButtonHidden()
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
                
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        Task {
                            await createGame()
                        }
                    }
                    .disabled(maxBet <= minBet)
                }
            }
        }
    }
    
    private func createGame() async {
        let config = GameConfig(
            gameName: gameName.isEmpty ? nil : gameName,
            gameType: gameType,
            minBet: minBet,
            maxBet: maxBet,
            maxPlayers: maxPlayers,
            minPlayers: 2,
            timeoutSeconds: timeoutMinutes * 60,
            houseEdgePercent: 2.5,
            requireBiometric: requireBiometric,
            enableChat: enableChat,
            privateGame: privateGame,
            password: privateGame ? password : nil
        )
        
        await gameManager.createGame(gameType: gameType, config: config)
        dismiss()
    }
}

/**
 * Join game sheet
 */
struct JoinGameSheet: View {
    @EnvironmentObject private var gameManager: GameManager
    @Environment(\.dismiss) private var dismiss
    
    @State private var gameId = ""
    @State private var password = ""
    @State private var selectedGame: AvailableGame?
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                // Manual Game ID Entry
                VStack(alignment: .leading, spacing: 8) {
                    Text("Join by Game ID")
                        .font(.headline)
                    
                    TextField("Enter Game ID", text: $gameId)
                        .textFieldStyle(.roundedBorder)
                    
                    if selectedGame?.requiresPassword == true {
                        SecureField("Password", text: $password)
                            .textFieldStyle(.roundedBorder)
                    }
                    
                    Button("Join Game") {
                        Task {
                            await gameManager.joinGame(gameId: gameId)
                            dismiss()
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(gameId.isEmpty ? Color.secondary : Color.blue)
                    .foregroundColor(.white)
                    .cornerRadius(8)
                    .disabled(gameId.isEmpty)
                }
                .padding()
                .background(Color.secondary.opacity(0.1))
                .cornerRadius(12)
                
                // Available Games List
                VStack(alignment: .leading, spacing: 8) {
                    Text("Available Games")
                        .font(.headline)
                    
                    if gameManager.availableGames.isEmpty {
                        Text("No games available. Start discovery to find games.")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: .infinity)
                            .padding()
                    } else {
                        ScrollView {
                            LazyVStack(spacing: 8) {
                                ForEach(gameManager.availableGames) { game in
                                    AvailableGameRow(game: game) {
                                        selectedGame = game
                                        gameId = game.id
                                        if game.requiresPassword {
                                            // Keep sheet open for password entry
                                        } else {
                                            Task {
                                                await gameManager.joinGame(gameId: game.id)
                                                dismiss()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                Spacer()
            }
            .padding()
            .navigationTitle("Join Game")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
        .onAppear {
            Task {
                await gameManager.refreshAvailableGames()
            }
        }
    }
}

/**
 * Initialization error view
 */
struct InitializationErrorView: View {
    @EnvironmentObject private var gameManager: GameManager
    @Environment(\.dismiss) private var dismiss
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 60))
                    .foregroundColor(.orange)
                
                Text("SDK Initialization Failed")
                    .font(.title)
                    .fontWeight(.semibold)
                
                Text(gameManager.lastError?.displayMessage ?? "Unknown error occurred during SDK initialization")
                    .font(.body)
                    .multilineTextAlignment(.center)
                    .foregroundColor(.secondary)
                
                VStack(spacing: 12) {
                    Button("Retry Initialization") {
                        Task {
                            await gameManager.initializeSDK()
                            if gameManager.isSDKInitialized {
                                dismiss()
                            }
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.blue)
                    .foregroundColor(.white)
                    .cornerRadius(8)
                    
                    Button("View Diagnostics") {
                        Task {
                            await gameManager.runDiagnostics()
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.secondary.opacity(0.2))
                    .cornerRadius(8)
                }
            }
            .padding()
            .navigationTitle("Initialization Error")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Close") {
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: - Preview Provider

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
            .environmentObject(GameManager())
            .environmentObject(SettingsManager())
    }
}