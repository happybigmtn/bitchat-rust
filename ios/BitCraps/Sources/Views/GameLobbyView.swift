import SwiftUI
import CoreBluetooth

@available(iOS 15.0, *)
struct GameLobbyView: View {
    @ObservedObject var gameManager: GameManager
    @Environment(\.dismiss) private var dismiss
    
    @State private var isSearching = false
    @State private var showingCreateGame = false
    @State private var selectedGameType = 0
    
    let gameTypes = ["Craps", "Poker", "Blackjack"]
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 20) {
                    // Header
                    GameLobbyHeader(gameManager: gameManager)
                    
                    // Quick Actions
                    QuickActionCards(
                        gameManager: gameManager,
                        showingCreateGame: $showingCreateGame,
                        isSearching: $isSearching
                    )
                    
                    // Available Games
                    AvailableGamesSection(
                        gameManager: gameManager,
                        isSearching: $isSearching
                    )
                    
                    // Network Status
                    NetworkStatusSection(gameManager: gameManager)
                }
                .padding()
            }
            .navigationTitle("Game Lobby")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Close") {
                        dismiss()
                    }
                }
                
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: {
                        showingCreateGame = true
                    }) {
                        Image(systemName: "plus")
                    }
                }
            }
        }
        .sheet(isPresented: $showingCreateGame) {
            CreateGameView(gameManager: gameManager)
        }
        .onAppear {
            gameManager.startGameDiscovery()
        }
        .onDisappear {
            gameManager.stopGameDiscovery()
        }
    }
}

@available(iOS 15.0, *)
struct GameLobbyHeader: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(spacing: 12) {
            HStack {
                Image(systemName: "gamecontroller")
                    .font(.largeTitle)
                    .foregroundColor(.blue)
                
                VStack(alignment: .leading) {
                    Text("BitCraps Lobby")
                        .font(.title2)
                        .fontWeight(.bold)
                    
                    Text("Find players or create a new game")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
            }
            
            // Player stats
            HStack(spacing: 20) {
                StatView(title: "Balance", value: "\(gameManager.balance)")
                StatView(title: "Games Played", value: "12") // TODO: Add to GameManager
                StatView(title: "Win Rate", value: "68%") // TODO: Add to GameManager
            }
            .padding()
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.blue.opacity(0.1))
            )
        }
    }
}

@available(iOS 15.0, *)
struct StatView: View {
    let title: String
    let value: String
    
    var body: some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.headline)
                .fontWeight(.bold)
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
    }
}

@available(iOS 15.0, *)
struct QuickActionCards: View {
    @ObservedObject var gameManager: GameManager
    @Binding var showingCreateGame: Bool
    @Binding var isSearching: Bool
    
    var body: some View {
        VStack(spacing: 12) {
            Text("Quick Actions")
                .font(.headline)
                .fontWeight(.semibold)
                .frame(maxWidth: .infinity, alignment: .leading)
            
            HStack(spacing: 12) {
                ActionCard(
                    title: "Create Game",
                    subtitle: "Host a new session",
                    icon: "plus.circle.fill",
                    color: .green
                ) {
                    showingCreateGame = true
                }
                
                ActionCard(
                    title: "Quick Join",
                    subtitle: "Join any available game",
                    icon: "bolt.fill",
                    color: .orange
                ) {
                    gameManager.joinGame()
                }
            }
            
            HStack(spacing: 12) {
                ActionCard(
                    title: "Search Games",
                    subtitle: "Find specific games",
                    icon: "magnifyingglass",
                    color: .blue
                ) {
                    isSearching.toggle()
                }
                
                ActionCard(
                    title: "Practice Mode",
                    subtitle: "Play offline",
                    icon: "person.fill",
                    color: .purple
                ) {
                    gameManager.simulateGameSession()
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct ActionCard: View {
    let title: String
    let subtitle: String
    let icon: String
    let color: Color
    let action: () -> Void
    
    var body: some View {
        Button(action: action) {
            VStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.title2)
                    .foregroundColor(color)
                
                VStack(spacing: 2) {
                    Text(title)
                        .font(.subheadline)
                        .fontWeight(.semibold)
                    
                    Text(subtitle)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
            }
            .frame(maxWidth: .infinity)
            .padding()
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.gray.opacity(0.1))
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

@available(iOS 15.0, *)
struct AvailableGamesSection: View {
    @ObservedObject var gameManager: GameManager
    @Binding var isSearching: Bool
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Available Games")
                    .font(.headline)
                    .fontWeight(.semibold)
                
                Spacer()
                
                Button(isSearching ? "Stop" : "Refresh") {
                    isSearching.toggle()
                    if isSearching {
                        gameManager.startGameDiscovery()
                    } else {
                        gameManager.stopGameDiscovery()
                    }
                }
                .font(.subheadline)
                .foregroundColor(.blue)
            }
            
            if gameManager.availableGames.isEmpty {
                EmptyGamesView(isSearching: isSearching)
            } else {
                LazyVStack(spacing: 8) {
                    ForEach(gameManager.availableGames) { game in
                        GameCard(game: game) {
                            gameManager.joinSpecificGame(game)
                        }
                    }
                }
            }
        }
    }
}

@available(iOS 15.0, *)
struct GameCard: View {
    let game: AvailableGame
    let onJoin: () -> Void
    
    var body: some View {
        HStack(spacing: 12) {
            // Game icon and info
            VStack(alignment: .leading, spacing: 4) {
                Text(game.hostName)
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                HStack(spacing: 8) {
                    Label("\(game.playerCount) players", systemImage: "person.2")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Label("\(abs(game.signal))dBm", systemImage: signalIcon(for: game.signal))
                        .font(.caption)
                        .foregroundColor(signalColor(for: game.signal))
                }
            }
            
            Spacer()
            
            // Join button
            Button(action: onJoin) {
                Text("Join")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                    .foregroundColor(.white)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(Color.blue)
                    .clipShape(Capsule())
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.gray.opacity(0.05))
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(Color.gray.opacity(0.2), lineWidth: 1)
                )
        )
    }
    
    private func signalIcon(for signal: Int) -> String {
        switch signal {
        case -50...0: return "wifi.circle.fill"
        case -70...(-51): return "wifi.circle"
        default: return "wifi.exclamationmark"
        }
    }
    
    private func signalColor(for signal: Int) -> Color {
        switch signal {
        case -50...0: return .green
        case -70...(-51): return .orange
        default: return .red
        }
    }
}

@available(iOS 15.0, *)
struct EmptyGamesView: View {
    let isSearching: Bool
    
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: isSearching ? "antenna.radiowaves.left.and.right" : "gamecontroller")
                .font(.title)
                .foregroundColor(.gray)
                .scaleEffect(isSearching ? 1.2 : 1.0)
                .animation(.easeInOut(duration: 1.0).repeatForever(), value: isSearching)
            
            VStack(spacing: 4) {
                Text(isSearching ? "Searching for games..." : "No games found")
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                Text(isSearching ? "Looking for nearby players" : "Try creating a new game or refreshing")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }
}

@available(iOS 15.0, *)
struct NetworkStatusSection: View {
    @ObservedObject var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Network Status")
                .font(.subheadline)
                .fontWeight(.semibold)
            
            HStack(spacing: 16) {
                NetworkStatusItem(
                    title: "Bluetooth",
                    status: "Connected",
                    color: .green
                )
                
                NetworkStatusItem(
                    title: "Mesh Network",
                    status: "Active",
                    color: .blue
                )
                
                NetworkStatusItem(
                    title: "Performance",
                    status: "\(gameManager.frameRate)fps",
                    color: gameManager.frameRate > 55 ? .green : .orange
                )
            }
        }
        .padding()
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.gray.opacity(0.05))
        )
    }
}

@available(iOS 15.0, *)
struct NetworkStatusItem: View {
    let title: String
    let status: String
    let color: Color
    
    var body: some View {
        VStack(spacing: 2) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
            
            Text(title)
                .font(.caption2)
                .foregroundColor(.secondary)
            
            Text(status)
                .font(.caption)
                .fontWeight(.medium)
        }
        .frame(maxWidth: .infinity)
    }
}

@available(iOS 15.0, *)
struct CreateGameView: View {
    @ObservedObject var gameManager: GameManager
    @Environment(\.dismiss) private var dismiss
    
    @State private var gameName = ""
    @State private var maxPlayers = 4.0
    @State private var isPrivate = false
    @State private var selectedGameType = 0
    
    let gameTypes = ["Craps", "Texas Hold'em", "Blackjack"]
    
    var body: some View {
        NavigationView {
            Form {
                Section("Game Settings") {
                    TextField("Game Name", text: $gameName)
                    
                    Picker("Game Type", selection: $selectedGameType) {
                        ForEach(0..<gameTypes.count, id: \.self) { index in
                            Text(gameTypes[index]).tag(index)
                        }
                    }
                    
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Max Players")
                            Spacer()
                            Text("\(Int(maxPlayers))")
                                .fontWeight(.medium)
                        }
                        
                        Slider(value: $maxPlayers, in: 2...8, step: 1)
                    }
                    
                    Toggle("Private Game", isOn: $isPrivate)
                }
                
                Section("Network Options") {
                    HStack {
                        Label("Bluetooth Discovery", systemImage: "bluetooth")
                        Spacer()
                        Text("Enabled")
                            .foregroundColor(.green)
                    }
                    
                    HStack {
                        Label("Mesh Routing", systemImage: "network")
                        Spacer()
                        Text("Auto")
                            .foregroundColor(.blue)
                    }
                }
                
                Section {
                    Button("Create Game") {
                        createGame()
                    }
                    .frame(maxWidth: .infinity)
                    .foregroundColor(.white)
                    .padding()
                    .background(Color.green)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                .listRowBackground(Color.clear)
            }
            .navigationTitle("Create Game")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
    }
    
    private func createGame() {
        gameManager.createGame()
        dismiss()
    }
}

@available(iOS 15.0, *)
struct GameLobbyView_Previews: PreviewProvider {
    static var previews: some View {
        GameLobbyView(gameManager: GameManager())
    }
}