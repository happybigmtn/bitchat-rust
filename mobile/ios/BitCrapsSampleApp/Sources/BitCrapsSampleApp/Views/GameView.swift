import SwiftUI
import BitCrapsSDK

/**
 * Game View - Main gameplay interface for active craps game
 */
struct GameView: View {
    @EnvironmentObject private var gameManager: GameManager
    @State private var selectedBetType: BetType = .pass
    @State private var betAmount: String = "10"
    @State private var showBetSheet = false
    @State private var animateDice = false
    @State private var showResult = false
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 20) {
                    // Game Status Card
                    GameStatusCard()
                        .padding(.horizontal)
                    
                    // Dice Display
                    DiceDisplay(animating: $animateDice)
                        .padding(.horizontal)
                    
                    // Betting Controls
                    if gameManager.canPlaceBet {
                        BettingControls(
                            selectedBetType: $selectedBetType,
                            betAmount: $betAmount,
                            showBetSheet: $showBetSheet
                        )
                        .padding(.horizontal)
                    }
                    
                    // Active Bets
                    if !gameManager.activeBets.isEmpty {
                        ActiveBetsSection()
                            .padding(.horizontal)
                    }
                    
                    // Game History
                    GameHistorySection()
                        .padding(.horizontal)
                    
                    // Participants
                    ParticipantsSection()
                        .padding(.horizontal)
                }
                .padding(.vertical)
            }
            .navigationTitle("Game #\(gameManager.currentGameSession?.id.prefix(8) ?? "")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button(action: { leaveGame() }) {
                            Label("Leave Game", systemImage: "arrow.right.square")
                        }
                        
                        Button(action: { shareGame() }) {
                            Label("Share Game", systemImage: "square.and.arrow.up")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            .sheet(isPresented: $showBetSheet) {
                PlaceBetSheet(
                    betType: $selectedBetType,
                    amount: $betAmount
                )
            }
            .alert("Round Result", isPresented: $showResult) {
                Button("OK") { }
            } message: {
                Text(gameManager.lastRoundResult ?? "")
            }
            .onReceive(gameManager.gameEventPublisher) { event in
                handleGameEvent(event)
            }
        }
    }
    
    private func handleGameEvent(_ event: GameEvent) {
        switch event {
        case .diceRolled:
            animateDice = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                animateDice = false
            }
        case .roundComplete:
            showResult = true
        default:
            break
        }
    }
    
    private func leaveGame() {
        gameManager.leaveCurrentGame()
    }
    
    private func shareGame() {
        // Share game invite
    }
}

// MARK: - Game Status Card
struct GameStatusCard: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var phaseColor: Color {
        switch gameManager.currentPhase {
        case .comeOut: return .blue
        case .point: return .orange
        case .ended: return .gray
        default: return .secondary
        }
    }
    
    var body: some View {
        VStack(spacing: 15) {
            // Phase Indicator
            HStack {
                Text(gameManager.currentPhase.displayName)
                    .font(.headline)
                    .foregroundColor(phaseColor)
                
                Spacer()
                
                if let point = gameManager.currentPoint {
                    Label("Point: \(point)", systemImage: "target")
                        .font(.caption)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(Color.orange.opacity(0.2))
                        .cornerRadius(8)
                }
            }
            
            // Turn Indicator
            if let shooter = gameManager.currentShooter {
                HStack {
                    Image(systemName: "person.fill")
                    Text("\(shooter.name) is shooting")
                        .font(.subheadline)
                    
                    if shooter.isMe {
                        Text("(You)")
                            .font(.caption)
                            .foregroundColor(.green)
                    }
                }
                .padding()
                .frame(maxWidth: .infinity)
                .background(Color(.tertiarySystemBackground))
                .cornerRadius(10)
            }
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(15)
    }
}

// MARK: - Dice Display
struct DiceDisplay: View {
    @EnvironmentObject private var gameManager: GameManager
    @Binding var animating: Bool
    
    var body: some View {
        HStack(spacing: 20) {
            DiceView(
                value: gameManager.lastRoll?.die1 ?? 1,
                animating: animating
            )
            
            DiceView(
                value: gameManager.lastRoll?.die2 ?? 1,
                animating: animating
            )
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(15)
        .overlay(
            Group {
                if let total = gameManager.lastRoll?.total {
                    Text("\(total)")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundColor(.white)
                        .padding()
                        .background(Circle().fill(Color.black.opacity(0.7)))
                        .offset(y: 60)
                }
            }
        )
    }
}

// MARK: - Individual Dice
struct DiceView: View {
    let value: Int
    let animating: Bool
    @State private var rotation = 0.0
    
    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.white)
                .frame(width: 80, height: 80)
                .shadow(radius: 5)
            
            DiceDots(value: value)
        }
        .rotation3DEffect(
            .degrees(rotation),
            axis: (x: 1, y: 1, z: 0)
        )
        .onChange(of: animating) { isAnimating in
            if isAnimating {
                withAnimation(.easeInOut(duration: 1.5)) {
                    rotation += 720
                }
            }
        }
    }
}

// MARK: - Dice Dots
struct DiceDots: View {
    let value: Int
    
    var body: some View {
        GeometryReader { geometry in
            let size = geometry.size.width / 4
            let positions = dotPositions(for: value)
            
            ForEach(positions, id: \.self) { position in
                Circle()
                    .fill(Color.black)
                    .frame(width: size, height: size)
                    .position(
                        x: geometry.size.width * position.x,
                        y: geometry.size.height * position.y
                    )
            }
        }
        .frame(width: 60, height: 60)
    }
    
    private func dotPositions(for value: Int) -> [CGPoint] {
        switch value {
        case 1: return [CGPoint(x: 0.5, y: 0.5)]
        case 2: return [CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.75, y: 0.75)]
        case 3: return [CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.5, y: 0.5), CGPoint(x: 0.75, y: 0.75)]
        case 4: return [CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.75, y: 0.25),
                       CGPoint(x: 0.25, y: 0.75), CGPoint(x: 0.75, y: 0.75)]
        case 5: return [CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.75, y: 0.25),
                       CGPoint(x: 0.5, y: 0.5),
                       CGPoint(x: 0.25, y: 0.75), CGPoint(x: 0.75, y: 0.75)]
        case 6: return [CGPoint(x: 0.25, y: 0.25), CGPoint(x: 0.75, y: 0.25),
                       CGPoint(x: 0.25, y: 0.5), CGPoint(x: 0.75, y: 0.5),
                       CGPoint(x: 0.25, y: 0.75), CGPoint(x: 0.75, y: 0.75)]
        default: return []
        }
    }
}

// MARK: - Betting Controls
struct BettingControls: View {
    @Binding var selectedBetType: BetType
    @Binding var betAmount: String
    @Binding var showBetSheet: Bool
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(spacing: 15) {
            Text("Place Your Bet")
                .font(.headline)
            
            // Bet Type Selector
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(BetType.allCases, id: \.self) { type in
                        BetTypeChip(
                            type: type,
                            isSelected: selectedBetType == type
                        ) {
                            selectedBetType = type
                        }
                    }
                }
            }
            
            // Amount Input
            HStack {
                Text("Amount:")
                    .font(.subheadline)
                
                TextField("Bet amount", text: $betAmount)
                    .keyboardType(.numberPad)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 100)
                
                Text("CRAP")
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                Spacer()
                
                Button("Place Bet") {
                    placeBet()
                }
                .buttonStyle(.borderedProminent)
                .disabled(betAmount.isEmpty)
            }
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(15)
    }
    
    private func placeBet() {
        guard let amount = Int(betAmount) else { return }
        gameManager.placeBet(type: selectedBetType, amount: amount)
        betAmount = "10"
    }
}

// MARK: - Bet Type Chip
struct BetTypeChip: View {
    let type: BetType
    let isSelected: Bool
    let action: () -> Void
    
    var body: some View {
        Button(action: action) {
            Text(type.displayName)
                .font(.caption)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(isSelected ? Color.blue : Color(.tertiarySystemBackground))
                .foregroundColor(isSelected ? .white : .primary)
                .cornerRadius(15)
        }
    }
}

// MARK: - Active Bets Section
struct ActiveBetsSection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Your Bets")
                .font(.headline)
            
            ForEach(gameManager.activeBets) { bet in
                HStack {
                    Image(systemName: bet.type.icon)
                        .foregroundColor(bet.type.color)
                    
                    Text(bet.type.displayName)
                        .font(.subheadline)
                    
                    Spacer()
                    
                    Text("\(bet.amount) CRAP")
                        .font(.caption)
                        .fontWeight(.semibold)
                }
                .padding()
                .background(Color(.tertiarySystemBackground))
                .cornerRadius(8)
            }
        }
    }
}

// MARK: - Game History
struct GameHistorySection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Roll History")
                .font(.headline)
            
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(gameManager.rollHistory.suffix(10), id: \.self) { roll in
                        VStack {
                            Text("\(roll.die1)+\(roll.die2)")
                                .font(.caption2)
                            
                            Text("\(roll.total)")
                                .font(.headline)
                                .fontWeight(.bold)
                        }
                        .frame(width: 50, height: 50)
                        .background(rollColor(for: roll.total))
                        .cornerRadius(8)
                    }
                }
            }
        }
    }
    
    private func rollColor(for total: Int) -> Color {
        switch total {
        case 7, 11: return .green.opacity(0.3)
        case 2, 3, 12: return .red.opacity(0.3)
        default: return Color(.tertiarySystemBackground)
        }
    }
}

// MARK: - Participants Section
struct ParticipantsSection: View {
    @EnvironmentObject private var gameManager: GameManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Players (\(gameManager.participants.count))")
                .font(.headline)
            
            ForEach(gameManager.participants) { player in
                HStack {
                    Circle()
                        .fill(player.isActive ? Color.green : Color.gray)
                        .frame(width: 8, height: 8)
                    
                    Text(player.name)
                        .font(.subheadline)
                    
                    if player.isShooter {
                        Image(systemName: "dice.fill")
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                    
                    Spacer()
                    
                    Text("\(player.balance) CRAP")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding(.vertical, 5)
            }
        }
        .padding()
        .background(Color(.secondarySystemBackground))
        .cornerRadius(15)
    }
}

// MARK: - Preview
struct GameView_Previews: PreviewProvider {
    static var previews: some View {
        GameView()
            .environmentObject(GameManager())
    }
}