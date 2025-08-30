import SwiftUI
import BitCrapsSDK

/**
 * Home View - Main dashboard for game discovery and quick actions
 */
struct HomeView: View {
    @EnvironmentObject private var gameManager: GameManager
    @State private var showCreateGame = false
    @State private var showJoinGame = false
    @State private var refreshing = false
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 20) {
                    // Balance Card
                    BalanceCard()
                        .padding(.horizontal)
                    
                    // Quick Actions
                    HStack(spacing: 15) {
                        QuickActionButton(
                            title: "Create Game",
                            icon: "plus.circle.fill",
                            color: .green
                        ) {
                            showCreateGame = true
                        }
                        
                        QuickActionButton(
                            title: "Join Game",
                            icon: "arrow.right.circle.fill",
                            color: .blue
                        ) {
                            showJoinGame = true
                        }
                    }
                    .padding(.horizontal)
                    
                    // Active Games Section
                    if !gameManager.activeGames.isEmpty {
                        ActiveGamesSection()
                            .padding(.horizontal)
                    }
                    
                    // Discovered Games
                    DiscoveredGamesSection(refreshing: $refreshing)
                        .padding(.horizontal)
                    
                    // Network Status
                    NetworkStatusCard()
                        .padding(.horizontal)
                }
                .padding(.vertical)
            }
            .navigationTitle("BitCraps")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: { refreshGames() }) {
                        Image(systemName: "arrow.clockwise")
                            .rotationEffect(.degrees(refreshing ? 360 : 0))
                            .animation(refreshing ? .linear(duration: 1).repeatForever(autoreverses: false) : .default, value: refreshing)
                    }
                }
            }
            .sheet(isPresented: $showCreateGame) {
                CreateGameView()
            }
            .sheet(isPresented: $showJoinGame) {
                JoinGameView()
            }
        }
    }
    
    private func refreshGames() {
        refreshing = true
        Task {
            await gameManager.discoverGames()
            await MainActor.run {
                refreshing = false
            }
        }
    }
}

// MARK: - Balance Card
struct BalanceCard: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Balance")
                .font(.caption)
                .foregroundColor(.secondary)
            
            HStack {
                Text("\(gameManager.balance)")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text("CRAP")
                    .font(.title3)
                    .foregroundColor(.secondary)
                
                Spacer()
                
                Image(systemName: "bitcoinsign.circle.fill")
                    .font(.title)
                    .foregroundColor(.orange)
            }
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(15)
    }
}

// MARK: - Quick Action Button
struct QuickActionButton: View {
    let title: String
    let icon: String
    let color: Color
    let action: () -> Void
    
    var body: some View {
        Button(action: action) {
            VStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.largeTitle)
                    .foregroundColor(color)
                
                Text(title)
                    .font(.caption)
                    .foregroundColor(.primary)
            }
            .frame(maxWidth: .infinity)
            .padding()
            .background(Color(.secondarySystemBackground))
            .cornerRadius(15)
        }
    }
}

// MARK: - Active Games Section
struct ActiveGamesSection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 15) {
            Text("Active Games")
                .font(.headline)
            
            ForEach(gameManager.activeGames) { game in
                ActiveGameRow(game: game)
            }
        }
    }
}

// MARK: - Game Row
struct ActiveGameRow: View {
    let game: GameSession
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("Game #\(game.id.prefix(8))")
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Text("\(game.participants.count) players • \(game.currentPhase)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            Button("Resume") {
                gameManager.resumeGame(game.id)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
        }
        .padding()
        .background(Color(.tertiarySystemBackground))
        .cornerRadius(10)
    }
}

// MARK: - Discovered Games Section
struct DiscoveredGamesSection: View {
    @EnvironmentObject private var gameManager: GameManager
    @Binding var refreshing: Bool
    
    var body: some View {
        VStack(alignment: .leading, spacing: 15) {
            HStack {
                Text("Nearby Games")
                    .font(.headline)
                
                Spacer()
                
                Text("\(gameManager.discoveredGames.count) found")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            if gameManager.discoveredGames.isEmpty {
                EmptyGamesView()
            } else {
                ForEach(gameManager.discoveredGames) { game in
                    DiscoveredGameRow(game: game)
                }
            }
        }
    }
}

// MARK: - Discovered Game Row
struct DiscoveredGameRow: View {
    let game: DiscoveredGame
    @EnvironmentObject private var gameManager: GameManager
    @State private var joining = false
    
    var body: some View {
        HStack {
            Circle()
                .fill(game.isJoinable ? Color.green : Color.red)
                .frame(width: 8, height: 8)
            
            VStack(alignment: .leading, spacing: 4) {
                Text(game.hostName)
                    .font(.subheadline)
                    .fontWeight(.semibold)
                
                Text("\(game.participants)/\(game.maxParticipants) players • Ante: \(game.ante)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            if game.isJoinable {
                Button(action: { joinGame() }) {
                    if joining {
                        ProgressView()
                            .scaleEffect(0.8)
                    } else {
                        Text("Join")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(joining)
            }
        }
        .padding()
        .background(Color(.tertiarySystemBackground))
        .cornerRadius(10)
    }
    
    private func joinGame() {
        joining = true
        Task {
            await gameManager.joinGame(game.id)
            await MainActor.run {
                joining = false
            }
        }
    }
}

// MARK: - Empty State
struct EmptyGamesView: View {
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "dice")
                .font(.largeTitle)
                .foregroundColor(.secondary)
            
            Text("No games found")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            Text("Create a new game or wait for others")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 30)
        .background(Color(.tertiarySystemBackground))
        .cornerRadius(10)
    }
}

// MARK: - Network Status Card
struct NetworkStatusCard: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var statusColor: Color {
        switch gameManager.networkStatus {
        case .connected: return .green
        case .connecting: return .orange
        case .disconnected: return .red
        }
    }
    
    var statusText: String {
        switch gameManager.networkStatus {
        case .connected: return "Connected"
        case .connecting: return "Connecting..."
        case .disconnected: return "Disconnected"
        }
    }
    
    var body: some View {
        HStack {
            Circle()
                .fill(statusColor)
                .frame(width: 10, height: 10)
            
            Text("Network: \(statusText)")
                .font(.caption)
            
            Spacer()
            
            Text("\(gameManager.connectedPeers) peers")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(10)
    }
}

// MARK: - Preview
struct HomeView_Previews: PreviewProvider {
    static var previews: some View {
        HomeView()
            .environmentObject(GameManager())
    }
}