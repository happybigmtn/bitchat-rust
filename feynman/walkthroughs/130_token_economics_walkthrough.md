# Chapter 16: Token Economics and Financial Systems

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending

## Walking Through `src/token/mod.rs`

*Part of the comprehensive BitCraps curriculum - a deep dive into decentralized financial systems*

---

## Part I: Token Economics and Financial Systems for Complete Beginners

Money is one of humanity's greatest inventions. It solves the fundamental problem of trade: how do you exchange value when you don't directly need what someone else is offering? But traditional money requires trust in central authorities - banks, governments, central banks. What if we could create money that works without requiring trust in any single entity?

Welcome to token economics - the science of designing digital currencies that incentivize desired behavior in decentralized networks. This isn't just about creating "digital coins." It's about engineering economic systems that coordinate human behavior at scale without central control.

### The Evolution of Money: From Barter to Bits

**The Barter Problem**

Imagine you're a blacksmith in medieval times. You make horseshoes, but you need bread. The baker needs shoes, but you need horseshoes. The shoemaker needs bread, but the baker already has shoes. This is the "coincidence of wants" problem - trade only works when both parties have what the other wants at the same time.

**The Dawn of Money**

Societies solved this by adopting common mediums of exchange: cattle, shells, salt, precious metals. Money needed three properties:
- **Medium of exchange**: Widely accepted for trade
- **Store of value**: Maintains worth over time  
- **Unit of account**: Provides a standard measure of value

**The Trust Problem**

Physical money worked because it was hard to counterfeit. But as societies grew, carrying around gold became impractical. Banks emerged, issuing paper notes backed by gold reserves. This required trust - you had to believe the bank actually had the gold.

Governments eventually abandoned the gold standard, creating "fiat" money backed only by trust in the government. This system works... until it doesn't.

### Famous Monetary Disasters and What They Teach Us

**The Hyperinflation of Weimar Germany (1921-1923)**

After World War I, Germany printed money to pay war reparations. Prices doubled every few days. A loaf of bread cost 160 billion marks. People brought wheelbarrows of cash to buy groceries. The economic chaos helped lead to World War II.

Lesson: *Unlimited money printing destroys value.* Any sustainable monetary system needs scarcity mechanisms.

**The Zimbabwe Hyperinflation (2000-2009)**

Political instability led Zimbabwe to print money to fund government operations. At its peak, prices doubled every day. Zimbabwe issued a 100 trillion dollar note. The currency became so worthless that people used it as wallpaper.

Lesson: *Trust in institutions is fragile.* When people lose faith in a currency, it can collapse overnight.

**The 2008 Financial Crisis**

Banks created complex financial instruments backed by subprime mortgages. When housing prices fell, the entire system nearly collapsed. Governments bailed out banks with taxpayer money, but millions lost their homes and jobs.

Lesson: *Centralized systems create systemic risk.* When critical institutions fail, the entire system can collapse.

**The Greek Debt Crisis (2010-2018)**

Greece couldn't print more euros to pay its debts, but also couldn't devalue its currency to become competitive. This led to severe austerity, massive unemployment, and social unrest.

Lesson: *Monetary sovereignty matters.* Being unable to control your own currency creates unique vulnerabilities.

These disasters share common themes:
- **Central points of failure**: When key institutions fail, the system fails
- **Moral hazard**: Easy money creation leads to irresponsible behavior
- **Lack of transparency**: People can't verify the system's integrity
- **Political manipulation**: Monetary policy becomes a political tool

### Enter Cryptocurrency: Programmable Money

Bitcoin, launched in 2009, proposed a radical solution: money controlled by mathematics rather than institutions. Key innovations:

**1. Decentralization**: No single point of failure
**2. Transparency**: All transactions are publicly verifiable  
**3. Scarcity**: Maximum supply is mathematically guaranteed
**4. Programmability**: Rules are enforced by code, not institutions

But Bitcoin was just the beginning. Modern token systems can incentivize complex behaviors and coordinate entire economies.

### Token Economics: Engineering Behavior

Token economics (tokenomics) is about designing monetary systems that incentivize desired behavior. It combines:
- **Computer Science**: How to implement and secure the system
- **Economics**: How to balance supply, demand, and incentives  
- **Psychology**: How humans actually behave with money
- **Game Theory**: How to prevent exploitation and gaming

**Key Questions in Token Design:**

1. **What behavior do we want to encourage?**
2. **How do we prevent abuse and gaming?**
3. **How do we bootstrap adoption?**
4. **How do we maintain long-term sustainability?**
5. **How do we handle economic volatility?**

### Core Concepts in Token Economics

**1. Utility vs. Investment Tokens**

- **Utility tokens**: Provide access to services (like quarters for an arcade)
- **Investment tokens**: Expected to appreciate in value (like stocks)
- **Hybrid tokens**: Serve both purposes (like CRAP tokens)

The distinction matters for both economics and regulation.

**2. Token Distribution**

How tokens enter circulation affects the entire system:
- **Initial distribution**: Who gets tokens at launch?
- **Mining/staking rewards**: How are new tokens created?
- **Vesting schedules**: How do early participants get their tokens?
- **Burn mechanisms**: How are tokens removed from circulation?

**3. Network Effects and Metcalfe's Law**

A network's value grows with the square of its users. The first telephone was useless; millions of telephones create immense value. Token systems need to reach "critical mass" for adoption.

**4. The Token Velocity Problem**

If tokens are only used for transactions and immediately sold, velocity is high but price is low. Successful token systems need "holding incentives" - reasons to keep tokens rather than immediately spending them.

**5. Staking and Governance**

Many modern token systems let holders "stake" tokens to:
- Earn rewards
- Vote on governance decisions  
- Secure the network
- Signal long-term commitment

### Types of Token Economic Models

**1. Proof of Work (Bitcoin Model)**

- **Incentive**: Mine blocks to earn new tokens
- **Security**: Attack the network becomes exponentially expensive
- **Trade-off**: High energy consumption
- **Good for**: Maximizing security and decentralization

**2. Proof of Stake (Ethereum 2.0 Model)**

- **Incentive**: Stake tokens to earn rewards
- **Security**: Attacking requires owning significant tokens
- **Trade-off**: Potential centralization as large holders earn more
- **Good for**: Energy efficiency and lower barriers to participation

**3. Delegated Proof of Stake (EOS Model)**

- **Incentive**: Vote for delegates who secure the network
- **Security**: Social consensus and reputation
- **Trade-off**: More centralized, political dynamics
- **Good for**: High transaction throughput

**4. Proof of Burn (Some altcoins)**

- **Incentive**: Permanently destroy tokens to earn mining rights
- **Security**: Attacking requires destroying valuable tokens
- **Trade-off**: Reduces token supply over time
- **Good for**: Creating scarcity and demonstrating commitment

**5. Proof of Relay (BitCraps Model)**

- **Incentive**: Forward messages to earn tokens
- **Security**: Network reliability depends on honest relaying
- **Trade-off**: Potential for fake relay attacks
- **Good for**: Incentivizing network infrastructure

### The Psychology of Token Holders

Understanding human psychology is crucial for token design:

**1. Loss Aversion**
People hate losses more than they like equivalent gains. Token systems must be careful about mechanisms that can cause losses.

**2. Anchoring Bias**
People anchor on the first price they see. Early token pricing has disproportionate psychological impact.

**3. FOMO (Fear of Missing Out)**
Fear of missing opportunities can drive irrational behavior. Successful tokens often leverage limited-time opportunities.

**4. Social Proof**
People do what others are doing. Early adoption requires social signaling and community building.

**5. Endowment Effect**
People value things more once they own them. Free token airdrops can create psychological ownership.

### Common Token Economic Failure Modes

**1. The Death Spiral**
Token price falls → People lose confidence → Usage decreases → Price falls further → System collapses

**2. Whale Domination**
Large holders (whales) accumulate so many tokens that they control the system, undermining decentralization.

**3. Mining Centralization**
As mining becomes more profitable, it attracts industrial operations that centralize control.

**4. Governance Attacks**
Attackers buy tokens specifically to vote for harmful changes to the system.

**5. Flash Loan Attacks**
Attackers borrow large amounts temporarily to manipulate voting or pricing mechanisms.

### Designing Sustainable Token Economics

**1. Balanced Incentives**
Reward good behavior without over-rewarding. Punish bad behavior without being draconian.

**2. Multiple Revenue Streams**
Don't rely on a single mechanism for value creation. Diversify sources of demand.

**3. Gradual Distribution**
Avoid dumping large amounts of tokens at once. Use vesting schedules and gradual unlocks.

**4. Anti-Gaming Mechanisms**
Design systems to be resistant to manipulation and exploitation.

**5. Long-term Thinking**
Optimize for sustainability over short-term growth.

### Token Economics in Gaming

Gaming presents unique challenges for token economics:

**1. Real-time Requirements**
Games need instant feedback, but blockchains can be slow.

**2. Micropayments**
Games involve many small transactions. Transaction fees must be minimal.

**3. Fairness Concerns**
Players are extremely sensitive to perceived unfairness. The system must be provably fair.

**4. Regulatory Issues**
Gaming tokens often resemble gambling, raising legal concerns.

**5. User Experience**
Complexity must be hidden from players who just want to have fun.

### The Mathematics of Token Velocity

Token velocity is a crucial concept. The equation is:

**PQ = MV**

Where:
- P = Average price level
- Q = Real economic activity  
- M = Money supply (token supply)
- V = Velocity (how often tokens are spent)

For token holders, we want high PQ (economic activity) and low V (people hold tokens). This drives up token price.

### Mechanism Design: The Art of Rules

Token systems are "mechanism design" problems - creating rules that produce desired outcomes even when participants act selfishly.

**The Revelation Principle**
Any outcome achievable by a complex mechanism can be achieved by a simple mechanism where everyone tells the truth about their preferences.

**Incentive Compatibility**
The mechanism should make it profitable for participants to behave honestly.

**Individual Rationality**
Participants should be better off participating than not participating.

**Budget Balance**
The mechanism shouldn't require external subsidies to operate.

### Modern Token Innovations

**1. Algorithmic Stablecoins**
Tokens that automatically adjust supply to maintain stable prices (though many have failed spectacularly).

**2. Decentralized Autonomous Organizations (DAOs)**
Organizations controlled entirely by token holders through voting.

**3. Non-Fungible Tokens (NFTs)**
Unique tokens representing ownership of specific digital assets.

**4. Yield Farming**
Providing liquidity to earn token rewards, creating complex economic incentives.

**5. Bonding Curves**
Mathematical functions that determine token price based on supply, creating predictable pricing.

### The Future of Token Economics

**Programmable Money**: Smart contracts will enable increasingly sophisticated monetary policies.

**Cross-chain Integration**: Tokens will work across multiple blockchain networks.

**Real-world Integration**: Tokens will increasingly represent real-world assets and rights.

**Regulatory Clarity**: Clear regulations will enable more institutional adoption.

**User Experience**: Complexity will be abstracted away, making token systems as easy to use as credit cards.

### Token Economics and Social Coordination

The deepest insight of token economics is that money is a coordination tool. By carefully designing token incentives, we can coordinate human behavior at scale without central authority.

This represents a fundamental shift in how societies organize economic activity. Instead of relying on governments and corporations, we can create self-sustaining economic systems governed by mathematical rules.

The BitCraps token system exemplifies these principles, creating economic incentives for network participation, fair gaming, and decentralized governance.

---

Now that you understand the theoretical foundations, let's examine how BitCraps implements these concepts in practice, creating a sophisticated token economy designed specifically for decentralized gaming.

---

## Part II: BitCraps Token Economics Implementation Deep Dive

The BitCraps token system implements a sophisticated multi-layered economy designed to incentivize network participation, ensure fair gaming, and maintain long-term sustainability. The CRAP token serves multiple functions: utility token for gaming, reward mechanism for network infrastructure, and governance token for decentralized decision-making.

### Module Architecture: `src/token/mod.rs`

The token system is architected around several key concepts that work together to create a complete economic system:

**Lines 1-9: System Overview**
```rust
//! Token economics for BitCraps
//! 
//! This module implements the CRAP token system including:
//! - Token ledger and balance management
//! - Proof-of-relay mining rewards
//! - Treasury management and liquidity provision
//! - Transaction validation and consensus
//! - Staking and reward distribution
```

This documentation immediately establishes the multi-faceted nature of the token system. Unlike simple cryptocurrencies that only track balances, BitCraps tokens integrate deeply with gaming mechanics and network operations.

### Transaction Types: Encoding Economic Activities

**Lines 24-34: Comprehensive Transaction Framework**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Transfer { from: PeerId, to: PeerId, amount: u64 },
    GameBet { player: PeerId, game_id: GameId, amount: u64, bet_type: u8 },
    GamePayout { winner: PeerId, game_id: GameId, amount: u64 },
    RelayReward { relayer: PeerId, amount: u64, proof: RelayProof },
    TreasuryDeposit { from: PeerId, amount: u64 },
    TreasuryWithdraw { to: PeerId, amount: u64 },
    Mint { to: PeerId, amount: u64, reason: String },
}
```

This enumeration captures the complete economic activity of the BitCraps network:

**Transfer**: Basic peer-to-peer payments (like Bitcoin transactions)
**GameBet/GamePayout**: Gaming-specific transactions that integrate with game logic
**RelayReward**: Infrastructure incentives for network participants
**TreasuryDeposit/TreasuryWithdraw**: Liquidity management for the ecosystem
**Mint**: Token creation with auditability (includes reason string)

The diversity of transaction types reflects the complexity of coordinating a decentralized gaming network.

### Proof-of-Relay: Incentivizing Network Infrastructure

**Lines 36-46: Cryptographic Proof System**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelayProof {
    pub relayer: PeerId,
    pub packet_hash: [u8; 32],
    pub source: PeerId,
    pub destination: PeerId,
    pub timestamp: u64,
    pub hop_count: u8,
    pub signature: BitchatSignature,
}
```

This structure implements a novel consensus mechanism: **Proof-of-Relay**. Instead of wasting energy (Proof-of-Work) or requiring existing wealth (Proof-of-Stake), participants earn tokens by providing valuable network services - relaying messages between peers.

Key design elements:
- **Cryptographic proof**: The signature ensures relay claims are authentic
- **Hop count tracking**: Longer routes earn higher rewards (incentivizing network reach)
- **Timestamp verification**: Prevents replay attacks and stale claims
- **Packet hash**: Links the proof to actual network activity

### Account Model: Beyond Simple Balances

**Lines 61-89: Rich Account Structure**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub peer_id: PeerId,
    pub balance: u64,
    pub staked_amount: u64,
    pub pending_rewards: u64,
    pub transaction_count: u64,
    pub reputation: f64,
    pub last_activity: u64,
}
```

This goes far beyond Bitcoin's UTXO model or Ethereum's simple balance tracking. Each account maintains:

**Balance**: Current spendable tokens
**Staked amount**: Tokens locked for network security
**Pending rewards**: Earned but not yet distributed rewards
**Transaction count**: Used for nonce-based replay protection
**Reputation**: Social scoring for network quality
**Last activity**: Enables inactive account cleanup

The reputation system is particularly sophisticated - it creates social incentives for good behavior beyond pure economic incentives.

### Staking Mechanism: Long-term Alignment

**Lines 91-100: Staking Position Tracking**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakingPosition {
    pub staker: PeerId,
    pub amount: u64,
    pub staked_at: u64,
    pub lock_duration: Duration,
    pub reward_rate: f64,
    pub accumulated_rewards: u64,
}
```

Staking solves the "token velocity problem" we discussed in theory. By locking tokens for specific periods, the system:
- Reduces circulating supply (supporting price)
- Aligns participant incentives with long-term network success
- Provides predictable rewards for network security

The variable reward rates and lock durations create sophisticated economic incentives.

### Mining Configuration: Sustainable Token Economics

**Lines 115-135: Economic Parameter Management**
```rust
#[derive(Debug, Clone)]
pub struct MiningConfig {
    pub base_reward: u64,
    pub difficulty_adjustment_interval: Duration,
    pub target_block_time: Duration,
    pub max_supply: u64,
    pub halving_interval: u64, // Number of transactions
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            base_reward: CrapTokens::from_crap(0.1).unwrap_or_else(|_| CrapTokens::new_unchecked(100_000)).0, // 0.1 CRAP per relay
            difficulty_adjustment_interval: Duration::from_secs(3600), // 1 hour
            target_block_time: Duration::from_secs(60), // 1 minute average
            max_supply: CrapTokens::from_crap(21_000_000.0).unwrap_or_else(|_| CrapTokens::new_unchecked(21_000_000_000_000)).0, // 21M CRAP total
            halving_interval: 210_000, // Halve rewards every 210k transactions
        }
    }
}
```

This configuration implements key tokenomic principles:

**Controlled Supply**: 21M CRAP maximum (borrowing Bitcoin's scarcity model)
**Halving Mechanism**: Rewards decrease over time (creating deflationary pressure)
**Dynamic Difficulty**: Adjusts based on network activity
**Predictable Timing**: Target block times enable economic planning

The economic parameters are carefully chosen to balance incentives across different time horizons.

### Token Ledger: The Core Financial Engine

**Lines 103-113: Comprehensive State Management**
```rust
pub struct TokenLedger {
    accounts: Arc<RwLock<HashMap<PeerId, Account>>>,
    transactions: Arc<RwLock<Vec<TokenTransaction>>>,
    staking_positions: Arc<RwLock<HashMap<PeerId, StakingPosition>>>,
    pending_transactions: Arc<RwLock<HashMap<[u8; 32], TokenTransaction>>>,
    total_supply: Arc<RwLock<u64>>,
    treasury_balance: Arc<RwLock<u64>>,
    mining_config: MiningConfig,
    event_sender: mpsc::UnboundedSender<TokenEvent>,
}
```

The ledger uses `Arc<RwLock<_>>` for thread-safe access across the distributed system. Key design decisions:

**Separate transaction pools**: Confirmed vs. pending transactions enable optimistic execution
**Treasury tracking**: Enables sophisticated liquidity management
**Event broadcasting**: Other systems can react to token events in real-time
**Configurable mining**: Parameters can evolve as the network grows

### Balance Management: Safe Financial Operations

**Lines 196-227: Atomic Transfer Implementation**
```rust
pub async fn transfer(&self, from: PeerId, to: PeerId, amount: u64) -> Result<()> {
    let mut accounts = self.accounts.write().await;
    
    // Get source account
    let from_account = accounts.get_mut(&from)
        .ok_or_else(|| Error::Protocol("Source account not found".to_string()))?;
    
    if from_account.balance < amount {
        return Err(Error::Protocol("Insufficient balance".to_string()));
    }
    
    // Deduct from source
    from_account.balance -= amount;
    from_account.transaction_count += 1;
    
    // Add to destination (create if doesn't exist)
    let to_account = accounts.entry(to).or_insert_with(|| Account::new(to, 0));
    to_account.balance += amount;
    to_account.transaction_count += 1;
    
    Ok(())
}
```

This implements atomic transfers with several important properties:

**ACID Compliance**: The operation either completely succeeds or completely fails
**Balance Validation**: Prevents negative balances (no overdrafts)
**Auto-creation**: Destination accounts are created automatically
**Transaction Counting**: Enables nonce-based replay protection

The design prevents classic double-spending attacks while maintaining usability.

### Gaming Integration: Economic Game Logic

**Lines 266-314: Game Bet Processing**
```rust
pub async fn process_game_bet(
    &self,
    player: PeerId,
    amount: u64,
    game_id: GameId,
    bet_type: u8,
) -> Result<[u8; 32]> {
    let mut accounts = self.accounts.write().await;
    
    // Check player balance
    let account = accounts.get_mut(&player)
        .ok_or_else(|| Error::Protocol("Player account not found".to_string()))?;
    
    if account.balance < amount {
        return Err(Error::Protocol("Insufficient balance for bet".to_string()));
    }
    
    // Deduct bet amount from player
    account.balance -= amount;
    account.transaction_count += 1;
    
    // Create transaction record
    let transaction = TokenTransaction {
        id: self.generate_transaction_id(),
        transaction_type: TransactionType::GameBet {
            player,
            game_id,
            amount,
            bet_type,
        },
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs(),
        nonce: account.transaction_count,
        fee: 0,
        signature: None, // Game bets are pre-authorized
        confirmations: 1,
    };
    
    let tx_id = transaction.id;
    self.transactions.write().await.push(transaction);
    
    log::info!("Game bet: {} CRAP from {:?} for game {:?}", 
              CrapTokens::new_unchecked(amount).to_crap(), player, game_id);
    
    Ok(tx_id)
}
```

This demonstrates deep integration between the token system and game logic:

**Immediate Settlement**: Game bets are processed instantly (no blockchain delays)
**Comprehensive Logging**: Every bet is recorded for auditability
**Transaction Linking**: Game actions are tied to specific transactions
**Nonce Protection**: Account nonces prevent replay attacks

The comment "Game bets are pre-authorized" indicates a trust model where players have already authorized the game to deduct funds, enabling real-time gameplay.

### Treasury Management: Economic Stability

**Lines 253-260: Treasury Initialization**
```rust
// Initialize treasury if this is treasury address
if peer_id == TREASURY_ADDRESS {
    let initial_supply = CrapTokens::from_crap(1_000_000.0).unwrap_or_else(|_| CrapTokens::new_unchecked(1_000_000_000_000)).amount(); // 1M CRAP for treasury
    if let Some(treasury_account) = accounts.get_mut(&peer_id) {
        treasury_account.balance = initial_supply;
        *self.total_supply.write().await = initial_supply;
        *self.treasury_balance.write().await = initial_supply;
    }
}
```

The treasury starts with 1M CRAP tokens (about 5% of total supply). This serves several functions:
- **Liquidity provision**: Ensures tokens are available for trading
- **Economic stability**: Treasury can intervene during market volatility
- **Development funding**: Supports ongoing development and operations
- **Emergency reserves**: Provides resources for unforeseen circumstances

### Proof-of-Relay Implementation: Infrastructure Incentives

**Lines 400-486: Relay Reward Processing**
```rust
pub async fn process_relay_reward(
    &self,
    relayer: PeerId,
    messages_relayed: u64,
) -> Result<[u8; 32]> {
    let mut accounts = self.accounts.write().await;
    
    // Calculate reward amount based on messages relayed
    let base_reward_per_message = self.mining_config.base_reward / 10; // 0.01 CRAP per message
    let reward_amount = messages_relayed * base_reward_per_message;
    
    // Create or get relayer account
    let account = accounts.entry(relayer).or_insert_with(|| Account {
        peer_id: relayer,
        balance: 0,
        staked_amount: 0,
        pending_rewards: 0,
        transaction_count: 0,
        reputation: 0.5,
        last_activity: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs(),
    });
    
    // Add reward to balance
    account.balance += reward_amount;
    account.reputation = (account.reputation + 0.01).min(1.0); // Increase reputation
    account.last_activity = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    
    // Update total supply with newly minted tokens
    *self.total_supply.write().await += reward_amount;
    
    log::info!("Relay reward: {} CRAP to {:?} for {} messages relayed", 
              CrapTokens::new_unchecked(reward_amount).to_crap(), relayer, messages_relayed);
    
    // Emit event
    let _ = self.event_sender.send(TokenEvent::RewardMinted {
        recipient: relayer,
        amount: reward_amount,
        reason: format!("Relay reward for {} messages", messages_relayed),
    });
    
    Ok(tx_id)
}
```

This implements the core incentive mechanism of the BitCraps network:

**Linear Rewards**: More relaying earns proportionally more tokens
**Reputation Building**: Successful relaying improves social standing  
**Supply Inflation**: New tokens are minted as rewards (controlled inflation)
**Activity Tracking**: Updates last activity timestamp
**Event Broadcasting**: Other systems can react to reward events

The reputation system creates social incentives beyond pure economic rewards.

### Advanced Mining System: `ProofOfRelay`

**Lines 516-675: Sophisticated Mining Implementation**
```rust
impl ProofOfRelay {
    pub async fn record_relay(
        &self,
        relayer: PeerId,
        packet_hash: [u8; 32],
        source: PeerId,
        destination: PeerId,
        hop_count: u8,
    ) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs();
        
        let relay_entry = RelayEntry {
            relayer,
            packet_hash,
            relay_path: vec![source, relayer, destination],
            timestamp,
            reward_claimed: false,
        };
        
        // Store relay entry
        self.relay_log.write().await.insert(packet_hash, relay_entry);
        
        // Calculate reward based on hop distance
        let reward_multiplier = match hop_count {
            1 => 1, // Direct relay
            2..=3 => 2, // Medium distance
            4..=6 => 3, // Long distance
            _ => 4, // Very long distance (max multiplier)
        };
        
        let base_reward = self.ledger.mining_config.base_reward / 100; // 0.001 CRAP base
        let reward_amount = base_reward * reward_multiplier;
        
        // Process relay reward through ledger
        if let Ok(_tx_id) = self.ledger.process_relay_reward(relayer, reward_amount).await {
            // Mark reward as claimed
            if let Some(entry) = self.relay_log.write().await.get_mut(&packet_hash) {
                entry.reward_claimed = true;
            }
            
            // Update stats
            let mut stats = self.mining_stats.write().await;
            stats.total_rewards_distributed += reward_amount;
        }
        
        Ok(())
    }
}
```

This implements sophisticated economic incentives for network infrastructure:

**Distance-based Rewards**: Longer routes earn higher multipliers (encouraging network expansion)
**Duplicate Prevention**: Each packet hash can only be claimed once
**Statistical Tracking**: Network activity metrics enable economic analysis
**Atomic Operations**: Reward claiming is tied to successful ledger updates

The hop count multiplier creates interesting economic dynamics - participants are incentivized to relay messages across longer distances, improving network connectivity.

### Economic Event System

**Lines 138-146: Comprehensive Event Broadcasting**
```rust
#[derive(Debug, Clone)]
pub enum TokenEvent {
    TransactionSubmitted { tx_id: [u8; 32], transaction: TokenTransaction },
    TransactionConfirmed { tx_id: [u8; 32] },
    RewardMinted { recipient: PeerId, amount: u64, reason: String },
    BalanceUpdated { peer_id: PeerId, old_balance: u64, new_balance: u64 },
    StakingPositionCreated { staker: PeerId, amount: u64 },
    RewardsDistributed { total_amount: u64, recipient_count: usize },
}
```

The event system enables reactive programming throughout the BitCraps ecosystem:
- **Monitoring systems** can track economic activity in real-time
- **User interfaces** can update balances immediately
- **Analytics systems** can compute economic metrics
- **Alerting systems** can detect unusual activity patterns

### Advanced Mining Operations

**Lines 631-675: Difficulty Adjustment and Cleanup**
```rust
/// Adjust mining difficulty based on network activity
pub async fn adjust_mining_difficulty(&self) -> Result<()> {
    let stats = self.mining_stats.read().await;
    let current_activity = stats.total_relays;
    drop(stats);
    
    // Simple difficulty adjustment based on activity
    if current_activity > 1000 {
        log::info!("High network activity detected, adjusting mining difficulty");
    } else if current_activity < 100 {
        log::info!("Low network activity detected, adjusting mining difficulty");
    }
    
    Ok(())
}

/// Clean up old relay entries
pub async fn cleanup_old_entries(&self) -> Result<()> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    
    let mut relay_log = self.relay_log.write().await;
    let old_count = relay_log.len();
    
    // Remove entries older than 1 hour
    relay_log.retain(|_hash, entry| {
        current_time - entry.timestamp < 3600
    });
    
    let removed = old_count - relay_log.len();
    if removed > 0 {
        log::info!("Cleaned up {} old relay entries", removed);
    }
    
    Ok(())
}
```

These functions implement essential economic maintenance:

**Difficulty Adjustment**: Prevents inflation if network activity changes dramatically
**Entry Cleanup**: Prevents unbounded memory growth from relay logs
**Activity Monitoring**: Provides data for economic policy decisions

The 1-hour cleanup window balances memory efficiency with the ability to verify recent relay claims.

### Economic Statistics and Analysis

**Lines 677-686: Comprehensive Metrics**
```rust
#[derive(Debug, Clone)]
pub struct LedgerStats {
    pub total_accounts: usize,
    pub total_transactions: usize,
    pub total_supply: u64,
    pub treasury_balance: u64,
    pub total_staked: u64,
    pub active_stakers: usize,
}
```

These statistics enable economic analysis and policy decisions:
- **Velocity calculations**: Transactions/accounts ratio
- **Staking participation**: Active stakers/total accounts
- **Treasury health**: Treasury balance/total supply
- **Network growth**: Account creation over time

### Key Economic Design Principles

**1. Multi-layered Incentives**
The system rewards both immediate activity (relaying) and long-term commitment (staking), creating balanced incentives across different time horizons.

**2. Social and Economic Alignment**
Reputation scoring creates social incentives that complement economic rewards, making purely selfish behavior less profitable.

**3. Controlled Inflation**
New token creation is tied to valuable network services (relaying), ensuring inflation corresponds to real economic value.

**4. Automatic Balancing**
Treasury management and difficulty adjustments help maintain economic stability without manual intervention.

**5. Transparency and Auditability**
All economic activity is logged and verifiable, building trust in the system's fairness.

### Production Considerations

**1. Economic Security**
The system prevents common attacks like double-spending, replay attacks, and reward gaming through cryptographic proofs and careful state management.

**2. Scalability**
Asynchronous operations and efficient data structures enable high transaction throughput required for gaming applications.

**3. Regulatory Compliance**
The detailed transaction logging and audit trails support potential regulatory requirements while maintaining decentralization.

**4. User Experience**  
Complex economic mechanisms are abstracted behind simple APIs, making the system accessible to game developers and players.

The BitCraps token system demonstrates how sophisticated economic theory translates into practical code that can coordinate human behavior at scale while maintaining the real-time performance requirements of gaming applications.
