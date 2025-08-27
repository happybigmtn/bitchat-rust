# Chapter 27: Treasury Management - The Mathematics of Being The House

## A Primer on Casino Economics: Why The House Always Wins (Until It Doesn't)

In 1960, a mathematics professor named Edward O. Thorp walked into a casino in Reno, Nevada, with a revolutionary idea. He had used an IBM 704 computer to prove that blackjack could be beaten by counting cards. His book "Beat the Dealer" would forever change how casinos operate. But here's the fascinating part: despite Thorp proving the house could be beaten, casinos didn't ban blackjack. Instead, they adapted - adding more decks, changing rules, setting betting limits. The lesson? The house doesn't need to win every bet; it just needs to win slightly more often than it loses.

This is the fundamental principle of casino economics: the house edge. It's not about winning big; it's about winning consistently. A casino with a 1% house edge will, over millions of bets, retain exactly 1% of all money wagered. It's as reliable as the law of large numbers itself.

But running a casino, whether physical or digital, isn't just about mathematical edges. It's about treasury management - ensuring you have enough capital to pay out wins, even during unlikely losing streaks. This is where most amateur casinos fail. They understand the math but not the money management.

Let me tell you about the Monte Carlo Casino disaster of 1913, often called "the night that broke the bank." At a roulette table, the ball landed on black 26 times in a row. The probability of this happening is about 1 in 136 million. Gamblers, believing red was "due," bet increasingly large amounts on red, and lost fortune after fortune. The casino's winnings that night were legendary. But here's the untold part: the casino nearly ran out of money to pay potential red wins if the streak had broken earlier with maximum bets. They had the mathematical edge but almost lost due to poor treasury management.

The key insight is that casinos face two types of risk: statistical risk (losing despite the house edge) and liquidity risk (not having enough cash to pay winners). Statistical risk decreases with more bets due to the law of large numbers. Liquidity risk, however, can destroy a casino instantly if not properly managed.

Consider the concept of "ruin probability" - the chance a casino goes bankrupt despite having a mathematical edge. With infinite capital, ruin probability is zero. But real casinos have finite capital. The formula for ruin probability is elegant:

P(ruin) = ((q/p)^a - (q/p)^b) / (1 - (q/p)^b)

Where p is the probability of winning a unit, q is the probability of losing a unit, a is the starting capital, and b is the target capital. Even with a house edge (p > q), if 'a' is too small relative to maximum bet sizes, ruin becomes likely.

This is why casinos set maximum bets. It's not because they fear skilled players; it's because they must limit their exposure relative to their treasury. A casino with $1 million in capital cannot safely accept $100,000 bets, even with a 5% edge. The volatility would create unacceptable ruin risk.

Modern online casinos face additional challenges. In physical casinos, chips create natural friction - players must physically exchange money, creating psychological barriers to large bets. Online, a player can bet their entire balance with one click. This demands more sophisticated treasury management.

Furthermore, decentralized casinos can't rely on traditional banking. They can't get a loan from a bank during a bad streak. Their treasury must be entirely self-sufficient, with enough reserves to weather any statistically possible losing streak.

The concept of "locked funds" becomes crucial. When a player places a bet, the casino must immediately lock enough funds to pay the maximum possible win. These funds cannot be used for other payouts until that bet resolves. Poor lock management can create a situation where the casino has money but can't access it - a liquidity crisis despite solvency.

There's also the time value of money to consider. Funds locked for pending bets aren't earning interest or being productively used. This creates an opportunity cost. The longer bets take to resolve, the higher this cost. This is why most casino games resolve quickly - not just for player engagement, but for treasury efficiency.

The relationship between house edge and treasury size is non-linear. A casino with a 1% edge needs much more capital than one with a 2% edge to achieve the same ruin probability. This is because variance relative to edge determines required bankroll. The Kelly Criterion, developed by John Kelly at Bell Labs in 1956, provides the optimal betting fraction:

f* = (p * b - q) / b

Where f* is the fraction of capital to risk, p is probability of winning, q is probability of losing, and b is the odds received. For the casino, this determines maximum safe bet sizes relative to treasury.

But the Kelly Criterion assumes independent bets. In reality, casino bets can be correlated. If multiple players bet on the same outcome (like "pass" in craps), a single roll can trigger multiple payouts. This correlation multiplies risk. Treasury management must account for worst-case correlation scenarios.

There's also the principal-agent problem. In traditional casinos, managers might take excessive risks because they get bonuses for profits but don't bear losses. Decentralized casinos solve this by making every token holder a partial owner. Everyone's incentives align with prudent treasury management.

The concept of "float" from insurance provides another lens. Insurance companies collect premiums upfront but pay claims later. The money in between - the float - can be invested. Warren Buffett built Berkshire Hathaway largely on insurance float. Casinos have negative float - they must lock funds immediately but collect winnings later. This creates a financing burden that must be carefully managed.

Consider also the "gambler's ruin" problem from the player's perspective. A player with finite funds playing against a casino with near-infinite funds will eventually go broke, even in a fair game (50-50 odds). This asymmetry is another source of casino advantage beyond the house edge.

But what happens when the casino itself has finite funds? The situation becomes symmetric - both player and house face ruin risk. The outcome depends on the ratio of bankrolls and the house edge. This is why minimum treasury reserves are crucial. The casino must maintain overwhelming bankroll superiority to ensure the house edge manifests.

There's a beautiful connection to thermodynamics here. Just as heat flows from hot to cold, money flows from smaller bankrolls to larger ones in repeated gambling. The house edge biases this flow, but bankroll size difference drives it. This is why casino consolidation is common - larger treasuries have structural advantages beyond economies of scale.

The fractional reserve banking system offers another parallel. Banks don't keep all deposits in vaults; they lend most out, keeping only a fraction in reserve. This works because not all depositors withdraw simultaneously. Casinos operate inversely - they must keep more than 100% reserves because they lock funds for potential payouts exceeding bet amounts.

This "inverse fractional reserve" requirement makes casino treasuries capital-inefficient. A bank might operate with 10% reserves. A casino might need 200% reserves relative to active betting volume. This is why casino margins must be higher than bank margins to generate equivalent returns on capital.

The advent of cryptocurrency adds new dimensions. Traditional casinos can freeze funds, reverse transactions, and rely on legal systems for dispute resolution. Crypto casinos cannot. Every transaction is final. This demands more robust treasury management and automated, trustless payout systems.

Smart contracts enable programmable treasury management. Rules can be encoded immutably: maximum bet sizes relative to treasury, automatic reserve requirements, algorithmic house edge adjustments. The treasury becomes not just a pool of money but an autonomous financial entity.

The concept of "treasury attacks" emerges in decentralized systems. Malicious players might coordinate to place maximum bets simultaneously, trying to exhaust treasury reserves. Or they might exploit timing attacks, placing bets when the treasury is momentarily vulnerable. Defense requires sophisticated real-time treasury monitoring and dynamic limits.

There's also the oracle problem. The treasury must know game outcomes to process payouts. But who provides this information? In centralized casinos, the house is trusted. In decentralized casinos, consensus mechanisms or cryptographic proofs must determine outcomes. Treasury management must account for the possibility of disputed or delayed outcomes.

The interplay between tokenomics and treasury management is crucial. If the casino token's value drops, the treasury's real purchasing power decreases, even if token count remains constant. This creates a death spiral risk: losses reduce treasury, causing token sales, dropping price, further reducing treasury value. Proper reserve management must consider token volatility.

Modern portfolio theory applies here. The treasury shouldn't be entirely in the casino's native token. Diversification across uncorrelated assets reduces risk. But this creates complexity - multi-asset treasuries need real-time pricing, rebalancing, and cross-chain bridges.

The concept of "treasury mining" emerges in DeFi casinos. Users provide liquidity to the treasury in exchange for rewards. This creates a larger, more resilient treasury but also obligations to liquidity providers. The treasury must generate enough returns to cover both player winnings and liquidity rewards.

Finally, there's the philosophical question: who really owns the treasury? In traditional casinos, shareholders own it. In decentralized casinos, it might be token holders, players, liquidity providers, or some combination. This ambiguity can create governance challenges when deciding treasury parameters.

## The BitCraps Treasury Implementation

Now let's examine how BitCraps implements these treasury management concepts in code. The treasury module is the financial heart of the casino, managing funds with the precision of a Swiss bank and the transparency of a blockchain.

```rust
//! Treasury System - Counterparty to All Bets
//! 
//! The treasury acts as the house/bank in the decentralized casino,
//! serving as the counterparty to all player bets. This ensures that:
//! - Players always have a counterparty to bet against
//! - Payouts are guaranteed from a common pool
//! - The house edge maintains treasury solvency
//! - No player can refuse to pay out winnings
```

This header comment encapsulates the treasury's role: it's the universal counterparty. In peer-to-peer betting, finding opponents is hard. The treasury solves this by always being ready to take the other side of any bet, providing liquidity and ensuring games can always proceed.

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use crate::protocol::{CrapTokens, GameId, Hash256};
use crate::error::Error;
use crate::crypto::GameCrypto;
```

The imports reveal the architecture. `RwLock` enables concurrent reads but exclusive writes - perfect for a treasury where balance checks are frequent but updates are atomic. The `HashMap` stores per-game locks, enabling granular fund management.

```rust
/// Treasury configuration constants
pub const INITIAL_TREASURY_BALANCE: u64 = 1_000_000_000; // 1 billion CRAP tokens
pub const MIN_TREASURY_RESERVE: u64 = 100_000_000; // 100 million minimum reserve
pub const MAX_BET_RATIO: f64 = 0.01; // Max single bet is 1% of treasury
pub const HOUSE_EDGE: f64 = 0.014; // 1.4% house edge (standard craps)
pub const TREASURY_FEE: f64 = 0.001; // 0.1% transaction fee
```

These constants encode crucial risk parameters. The 1% maximum bet ratio implements Kelly Criterion-inspired bankroll management. The 1.4% house edge matches real craps, providing authenticity. The 0.1% transaction fee generates revenue beyond game outcomes.

```rust
/// Treasury state - the decentralized bank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Treasury {
    /// Current treasury balance
    pub balance: CrapTokens,
    
    /// Locked funds for active bets (potential payouts)
    pub locked_funds: CrapTokens,
    
    /// Historical profit/loss tracking
    pub total_wagered: CrapTokens,
    pub total_paid_out: CrapTokens,
    pub total_fees_collected: CrapTokens,
    
    /// Per-game locked funds
    pub game_locks: HashMap<GameId, CrapTokens>,
    
    /// Treasury state hash for consensus
    pub state_hash: Hash256,
    
    /// Treasury version for upgrade compatibility
    pub version: u32,
}
```

The Treasury structure separates available and locked funds - crucial for liquidity management. The historical tracking enables profit/loss analysis. Per-game locks prevent one game from affecting another's liquidity. The state hash enables distributed consensus on treasury state.

```rust
impl Treasury {
    /// Create a new treasury with initial balance
    pub fn new() -> Self {
        let mut treasury = Self {
            balance: CrapTokens::from(INITIAL_TREASURY_BALANCE),
            locked_funds: CrapTokens::from(0),
            total_wagered: CrapTokens::from(0),
            total_paid_out: CrapTokens::from(0),
            total_fees_collected: CrapTokens::from(0),
            game_locks: HashMap::new(),
            state_hash: [0; 32],
            version: 1,
        };
        treasury.update_state_hash();
        treasury
    }
```

Starting with 1 billion tokens provides substantial buffer against variance. The immediate hash calculation ensures even the genesis state is verifiable.

```rust
    /// Calculate maximum allowed bet based on treasury balance
    pub fn max_bet_amount(&self) -> CrapTokens {
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        let max_amount = (available.0 as f64 * MAX_BET_RATIO) as u64;
        let reserve_adjusted = available.0.saturating_sub(MIN_TREASURY_RESERVE);
        CrapTokens::from(max_amount.min(reserve_adjusted))
    }
```

This function implements sophisticated bet limiting. It considers both the percentage limit (Kelly Criterion) and absolute reserve requirement. The `saturating_sub` prevents underflow if reserves are depleted. The minimum of both constraints ensures conservative treasury protection.

```rust
    /// Lock funds for a potential payout
    pub fn lock_funds(&mut self, game_id: GameId, amount: CrapTokens, max_payout: CrapTokens) -> Result<(), Error> {
        // Check if treasury can cover the maximum payout
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        if max_payout > available {
            return Err(Error::InsufficientFunds(
                format!("Treasury cannot cover max payout of {} CRAP", max_payout.0)
            ));
        }
        
        // Check if bet exceeds maximum allowed
        let max_bet = self.max_bet_amount();
        if amount > max_bet {
            return Err(Error::InvalidBet(
                format!("Bet of {} exceeds max allowed bet of {}", amount.0, max_bet.0)
            ));
        }
```

The two-phase validation is crucial. First, check if the treasury can cover the worst-case payout. Second, verify the bet doesn't exceed risk limits. This ordering prevents race conditions where a valid bet becomes invalid due to concurrent bets.

```rust
        // Lock the maximum potential payout
        self.locked_funds = self.locked_funds.checked_add(max_payout)
            .ok_or_else(|| Error::InvalidData("Overflow in locked funds".to_string()))?;
        let current = self.game_locks.get(&game_id).copied().unwrap_or(CrapTokens::from(0));
        *self.game_locks.entry(game_id).or_insert(CrapTokens::from(0)) = 
            current.checked_add(max_payout)
                .ok_or_else(|| Error::InvalidData("Overflow in game locks".to_string()))?;
```

Locking the maximum payout, not the bet amount, is crucial. A $100 bet might have a $500 maximum payout. The treasury must reserve $500 to guarantee payment. The per-game tracking enables partial releases if games have multiple bets.

```rust
        // Track wagered amount
        self.total_wagered = self.total_wagered.checked_add(amount)
            .ok_or_else(|| Error::InvalidData("Overflow in total wagered".to_string()))?;
        
        // Collect treasury fee
        let fee = CrapTokens::from((amount.0 as f64 * TREASURY_FEE) as u64);
        self.balance = self.balance.checked_add(fee)
            .ok_or_else(|| Error::InvalidData("Overflow in balance".to_string()))?;
        self.total_fees_collected = self.total_fees_collected.checked_add(fee)
            .ok_or_else(|| Error::InvalidData("Overflow in fees collected".to_string()))?;
```

The immediate fee collection is brilliant. Even if the house loses the bet, it collected the fee. Over many bets, these fees provide steady revenue independent of game variance. The checked arithmetic prevents overflow attacks.

```rust
    /// Release locked funds and process payout
    pub fn settle_bet(&mut self, game_id: GameId, locked_amount: CrapTokens, actual_payout: CrapTokens) -> Result<(), Error> {
        // Get locked amount for this game
        let game_locked = self.game_locks.get(&game_id).copied()
            .unwrap_or(CrapTokens::from(0));
        if locked_amount > game_locked {
            return Err(Error::InvalidState(
                format!("Attempted to release {} but only {} locked for game", 
                    locked_amount.0, game_locked.0)
            ));
        }
```

Settlement validation prevents double-spending attacks. A malicious actor can't release more funds than were locked, preventing treasury drain attacks.

```rust
        // Release the locked funds
        self.locked_funds = self.locked_funds.checked_sub(locked_amount)
            .unwrap_or(CrapTokens::from(0));
        if let Some(lock) = self.game_locks.get_mut(&game_id) {
            *lock = lock.checked_sub(locked_amount)
                .unwrap_or(CrapTokens::from(0));
            if lock.0 == 0 {
                self.game_locks.remove(&game_id);
            }
        }
```

The cleanup of zero-balance game locks prevents HashMap growth over time. This is crucial for long-running systems where memory leaks could accumulate.

```rust
        // Process the actual payout
        if actual_payout.0 > 0 {
            if actual_payout > self.balance {
                return Err(Error::InsufficientFunds(
                    format!("Treasury balance {} insufficient for payout {}", 
                        self.balance.0, actual_payout.0)
                ));
            }
            self.balance = self.balance.checked_sub(actual_payout)
                .ok_or_else(|| Error::InvalidData("Underflow in balance".to_string()))?;
            self.total_paid_out = self.total_paid_out.checked_add(actual_payout)
                .ok_or_else(|| Error::InvalidData("Overflow in total paid out".to_string()))?;
        } else {
            // House wins - add the bet amount to treasury
            let house_win = locked_amount.checked_sub(actual_payout)
                .unwrap_or(locked_amount);
            self.balance = self.balance.checked_add(house_win)
                .ok_or_else(|| Error::InvalidData("Overflow in balance".to_string()))?;
        }
```

The asymmetric handling of wins and losses is important. Player wins deduct from treasury balance. House wins add to it. The locked amount was reserved for maximum payout, so the house win is the difference between that and actual payout.

```rust
    /// Calculate house edge for a specific bet type
    pub fn calculate_house_edge(&self, bet_type: &crate::protocol::BetType) -> f64 {
        use crate::protocol::BetType;
        
        // Standard craps house edges
        match bet_type {
            BetType::Pass | BetType::DontPass => 0.014,
            BetType::Come | BetType::DontCome => 0.014,
            BetType::Field => 0.027,
            BetType::Yes6 | BetType::Yes8 => 0.015,
            BetType::Yes5 | BetType::Yes9 => 0.040,
            BetType::Yes4 | BetType::Yes10 => 0.067,
            BetType::Hard4 | BetType::Hard10 => 0.111,
            BetType::Hard6 | BetType::Hard8 => 0.091,
            BetType::Next7 => 0.167,
            BetType::Next2 | BetType::Next3 | BetType::Next11 | BetType::Next12 => 0.111,
            _ => HOUSE_EDGE,
        }
    }
```

These house edges match real craps exactly, providing authenticity. Notice how proposition bets (Hard ways, Any Seven) have much higher edges - they're sucker bets that subsidize the lower-edge main bets.

```rust
    /// Get treasury health metrics
    pub fn get_health(&self) -> TreasuryHealth {
        let available = self.balance.checked_sub(self.locked_funds)
            .unwrap_or(CrapTokens::from(0));
        let utilization = if self.balance.0 > 0 {
            (self.locked_funds.0 as f64 / self.balance.0 as f64) * 100.0
        } else {
            0.0
        };
        
        let profit = self.total_wagered.0 as i64 - self.total_paid_out.0 as i64 
            + self.total_fees_collected.0 as i64;
        
        TreasuryHealth {
            balance: self.balance,
            available_balance: available,
            locked_funds: self.locked_funds,
            utilization_percent: utilization,
            total_profit: profit,
            is_solvent: available.0 >= MIN_TREASURY_RESERVE,
            max_single_bet: self.max_bet_amount(),
            active_games: self.game_locks.len(),
        }
    }
```

Health metrics provide real-time treasury monitoring. Utilization percentage shows how much treasury is committed. The solvency check ensures minimum reserves are maintained. These metrics enable automated risk management and alerts.

```rust
    /// Update the state hash for consensus verification
    fn update_state_hash(&mut self) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.balance.0.to_le_bytes());
        data.extend_from_slice(&self.locked_funds.0.to_le_bytes());
        data.extend_from_slice(&self.total_wagered.0.to_le_bytes());
        data.extend_from_slice(&self.total_paid_out.0.to_le_bytes());
        data.extend_from_slice(&self.total_fees_collected.0.to_le_bytes());
        data.extend_from_slice(&self.version.to_le_bytes());
        
        self.state_hash = GameCrypto::hash(&data);
    }
```

The deterministic hash enables distributed consensus. All nodes can verify they have the same treasury state by comparing hashes. This is crucial for preventing treasury manipulation in decentralized deployments.

```rust
    /// Verify treasury state integrity
    pub fn verify_integrity(&self) -> bool {
        // Check basic invariants
        if self.locked_funds > self.balance {
            return false;
        }
        
        // Verify game locks sum matches total locked
        let total_game_locks: u64 = self.game_locks.values()
            .map(|t| t.0)
            .sum();
        if total_game_locks != self.locked_funds.0 {
            return false;
        }
        
        // Verify state hash
        let mut temp = self.clone();
        temp.update_state_hash();
        temp.state_hash == self.state_hash
    }
```

Integrity verification ensures internal consistency. The locked funds can't exceed total balance (preventing impossible states). The sum of per-game locks must equal total locks (preventing accounting errors). The hash verification detects any tampering.

```rust
/// Treasury manager with thread-safe access
pub struct TreasuryManager {
    treasury: Arc<RwLock<Treasury>>,
}

impl TreasuryManager {
    /// Create a new treasury manager
    pub fn new() -> Self {
        Self {
            treasury: Arc::new(RwLock::new(Treasury::new())),
        }
    }
    
    /// Get a read-only reference to the treasury
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, Treasury> {
        self.treasury.read().unwrap()
    }
    
    /// Get a mutable reference to the treasury
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, Treasury> {
        self.treasury.write().unwrap()
    }
```

The TreasuryManager wraps the treasury in thread-safe primitives. `Arc` enables sharing between threads. `RwLock` allows multiple concurrent readers but exclusive writers. This pattern maximizes concurrency while ensuring consistency.

```rust
    /// Process a bet with automatic locking
    pub fn process_bet(&self, game_id: GameId, bet_amount: CrapTokens, max_payout: CrapTokens) -> Result<(), Error> {
        let mut treasury = self.write();
        treasury.lock_funds(game_id, bet_amount, max_payout)
    }
    
    /// Settle a bet with automatic unlocking
    pub fn settle_bet(&self, game_id: GameId, locked_amount: CrapTokens, actual_payout: CrapTokens) -> Result<(), Error> {
        let mut treasury = self.write();
        treasury.settle_bet(game_id, locked_amount, actual_payout)
    }
```

These convenience methods encapsulate the lock/unlock pattern. They acquire the write lock, perform the operation, and automatically release the lock when the guard goes out of scope. This prevents deadlocks from forgotten unlocks.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_treasury_creation() {
        let treasury = Treasury::new();
        assert_eq!(treasury.balance.amount(), INITIAL_TREASURY_BALANCE);
        assert_eq!(treasury.locked_funds.amount(), 0);
        assert!(treasury.verify_integrity());
    }
```

Testing treasury creation verifies initial conditions. Starting with correct balance and no locked funds is crucial for system bootstrap.

```rust
    #[test]
    fn test_lock_and_release() {
        let mut treasury = Treasury::new();
        let game_id = [1; 16];
        
        // Lock funds for a bet
        let bet_amount = CrapTokens::new_unchecked(1000);
        let max_payout = CrapTokens::new_unchecked(2000);
        assert!(treasury.lock_funds(game_id, bet_amount, max_payout).is_ok());
        assert_eq!(treasury.locked_funds.amount(), 2000);
        assert!(treasury.balance.amount() > INITIAL_TREASURY_BALANCE); // Fee added
        
        // Settle with payout
        let payout = CrapTokens::new_unchecked(1500);
        assert!(treasury.settle_bet(game_id, max_payout, payout).is_ok());
        assert_eq!(treasury.locked_funds.amount(), 0);
        assert!(treasury.balance.amount() < INITIAL_TREASURY_BALANCE);
        
        assert!(treasury.verify_integrity());
    }
```

This test verifies the complete bet lifecycle. Funds are locked for the maximum payout, fees are collected immediately, and settlement correctly adjusts balances. The integrity check ensures no invariants were violated.

```rust
    #[test]
    fn test_max_bet_limits() {
        let treasury = Treasury::new();
        let max_bet = treasury.max_bet_amount();
        
        assert!(max_bet.amount() > 0);
        assert!(max_bet.amount() < treasury.balance.amount());
        assert!(max_bet.amount() <= (treasury.balance.amount() as f64 * MAX_BET_RATIO) as u64);
    }
```

Maximum bet testing ensures risk limits are properly enforced. The bet must be positive, less than total balance, and within the percentage limit.

```rust
    #[test]
    fn test_treasury_health() {
        let mut treasury = Treasury::new();
        let health = treasury.get_health();
        
        assert!(health.is_solvent);
        assert_eq!(health.utilization_percent, 0.0);
        assert_eq!(health.active_games, 0);
        
        // Lock some funds
        let game_id = [1; 16];
        let bet_amount = CrapTokens::new_unchecked(1000);
        let max_payout = CrapTokens::new_unchecked(5000);
        treasury.lock_funds(game_id, bet_amount, max_payout).unwrap();
        
        let health = treasury.get_health();
        assert!(health.utilization_percent > 0.0);
        assert_eq!(health.active_games, 1);
    }
}
```

Health monitoring tests verify that metrics accurately reflect treasury state. As funds are locked, utilization increases and active games are tracked.

## Key Lessons from Treasury Management

The BitCraps treasury implementation embodies several crucial principles:

1. **Separation of Available and Locked Funds**: Never commit funds you might need for other payouts. The lock mechanism ensures every bet has reserved funds for its maximum payout.

2. **Conservative Risk Limits**: The 1% maximum bet limit implements Kelly Criterion-inspired bankroll management, ensuring the treasury can survive variance.

3. **Immediate Fee Collection**: Transaction fees are collected upfront, providing steady revenue independent of game outcomes.

4. **Per-Game Tracking**: Each game's locked funds are tracked separately, preventing one game from affecting another's liquidity.

5. **Integrity Through Hashing**: The state hash enables distributed verification that all nodes agree on treasury state.

6. **Thread-Safe Design**: The RwLock pattern maximizes read concurrency while ensuring write atomicity.

7. **Comprehensive Health Metrics**: Real-time monitoring enables automated risk management and early problem detection.

8. **Authentic House Edges**: Using real craps odds provides legitimacy and predictable long-term returns.

The treasury is where game theory meets financial engineering. It must be mathematically sound (house edge ensures profitability), financially robust (reserves prevent insolvency), and technically reliable (thread-safety prevents race conditions).

This implementation also demonstrates the importance of defensive programming in financial systems. Every arithmetic operation is checked for overflow. Every fund release is validated against locks. Every state change updates the hash. This paranoia is justified - financial systems are prime targets for exploitation.

The module also shows how traditional casino concepts translate to code. Physical casinos have vaults, cashiers, and audit trails. Digital casinos have balance variables, lock mechanisms, and cryptographic hashes. The concepts map directly, but the implementation is pure software.

Finally, notice how the treasury acts as a market maker. By always being willing to take bets (up to risk limits), it provides liquidity to the game ecosystem. This is similar to how designated market makers provide liquidity in financial markets. The treasury profits from the spread (house edge) while providing a valuable service (guaranteed counterparty).