//! Mobile wallet screen for managing CRAP tokens
//!
//! Provides wallet functionality including balance display, transaction history,
//! and token management with mobile-optimized UI

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

use crate::protocol::CrapTokens;
use crate::token::{TokenTransaction, TransactionType};
use super::screen_base::{Screen, ScreenElement, Theme, TouchEvent, RenderContext, ScreenTransition};

/// Wallet screen for token management
pub struct WalletScreen {
    balance: Arc<RwLock<CrapTokens>>,
    staked_balance: Arc<RwLock<CrapTokens>>,
    pending_rewards: Arc<RwLock<CrapTokens>>,
    transactions: Arc<RwLock<Vec<TransactionDisplay>>>,
    selected_tab: Arc<RwLock<WalletTab>>,
    scroll_offset: Arc<RwLock<f32>>,
    qr_code_data: Arc<RwLock<Option<String>>>,
    receive_address: Arc<RwLock<String>>,
    send_amount: Arc<RwLock<String>>,
    send_address: Arc<RwLock<String>>,
}

/// Wallet tabs for different functions
#[derive(Debug, Clone, PartialEq)]
enum WalletTab {
    Overview,
    Send,
    Receive,
    History,
    Staking,
}

/// Transaction display information
#[derive(Debug, Clone)]
struct TransactionDisplay {
    tx_type: TransactionType,
    amount: CrapTokens,
    timestamp: SystemTime,
    description: String,
    status: TransactionStatus,
    peer_id: Option<String>,
}

#[derive(Debug, Clone)]
enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

impl WalletScreen {
    /// Create a new wallet screen
    pub fn new(wallet_address: String) -> Self {
        Self {
            balance: Arc::new(RwLock::new(CrapTokens(1000))),
            staked_balance: Arc::new(RwLock::new(CrapTokens(0))),
            pending_rewards: Arc::new(RwLock::new(CrapTokens(0))),
            transactions: Arc::new(RwLock::new(Self::generate_sample_transactions())),
            selected_tab: Arc::new(RwLock::new(WalletTab::Overview)),
            scroll_offset: Arc::new(RwLock::new(0.0)),
            qr_code_data: Arc::new(RwLock::new(None)),
            receive_address: Arc::new(RwLock::new(wallet_address)),
            send_amount: Arc::new(RwLock::new(String::new())),
            send_address: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Generate sample transaction history
    fn generate_sample_transactions() -> Vec<TransactionDisplay> {
        vec![
            TransactionDisplay {
                tx_type: TransactionType::GameWin,
                amount: CrapTokens(250),
                timestamp: SystemTime::now() - Duration::from_secs(3600),
                description: "Won craps game".to_string(),
                status: TransactionStatus::Confirmed,
                peer_id: Some("peer123...".to_string()),
            },
            TransactionDisplay {
                tx_type: TransactionType::GameLoss,
                amount: CrapTokens(100),
                timestamp: SystemTime::now() - Duration::from_secs(7200),
                description: "Lost bet on field".to_string(),
                status: TransactionStatus::Confirmed,
                peer_id: Some("peer456...".to_string()),
            },
            TransactionDisplay {
                tx_type: TransactionType::RelayReward,
                amount: CrapTokens(5),
                timestamp: SystemTime::now() - Duration::from_secs(10800),
                description: "Relay reward".to_string(),
                status: TransactionStatus::Confirmed,
                peer_id: None,
            },
            TransactionDisplay {
                tx_type: TransactionType::Transfer,
                amount: CrapTokens(500),
                timestamp: SystemTime::now() - Duration::from_secs(86400),
                description: "Received from friend".to_string(),
                status: TransactionStatus::Confirmed,
                peer_id: Some("peer789...".to_string()),
            },
        ]
    }

    /// Send tokens to another wallet
    pub async fn send_tokens(&self, to_address: String, amount: u64) -> Result<(), String> {
        let balance = self.balance.read().await;

        if amount > balance.0 {
            return Err("Insufficient balance".to_string());
        }

        // Deduct from balance
        *self.balance.write().await = CrapTokens(balance.0 - amount);

        // Add to transaction history
        let mut txs = self.transactions.write().await;
        txs.insert(0, TransactionDisplay {
            tx_type: TransactionType::Transfer,
            amount: CrapTokens(amount),
            timestamp: SystemTime::now(),
            description: format!("Sent to {}", &to_address[..8]),
            status: TransactionStatus::Pending,
            peer_id: Some(to_address),
        });

        // In real implementation, broadcast transaction

        Ok(())
    }

    /// Stake tokens for rewards
    pub async fn stake_tokens(&self, amount: u64) -> Result<(), String> {
        let balance = self.balance.read().await;

        if amount > balance.0 {
            return Err("Insufficient balance".to_string());
        }

        // Move from balance to staked
        *self.balance.write().await = CrapTokens(balance.0 - amount);
        let mut staked = self.staked_balance.write().await;
        *staked = CrapTokens(staked.0 + amount);

        // Add to transaction history
        let mut txs = self.transactions.write().await;
        txs.insert(0, TransactionDisplay {
            tx_type: TransactionType::Stake,
            amount: CrapTokens(amount),
            timestamp: SystemTime::now(),
            description: "Staked for rewards".to_string(),
            status: TransactionStatus::Confirmed,
            peer_id: None,
        });

        Ok(())
    }

    /// Unstake tokens
    pub async fn unstake_tokens(&self, amount: u64) -> Result<(), String> {
        let staked = self.staked_balance.read().await;

        if amount > staked.0 {
            return Err("Insufficient staked balance".to_string());
        }

        // Move from staked to balance
        *self.staked_balance.write().await = CrapTokens(staked.0 - amount);
        let mut balance = self.balance.write().await;
        *balance = CrapTokens(balance.0 + amount);

        // Add to transaction history
        let mut txs = self.transactions.write().await;
        txs.insert(0, TransactionDisplay {
            tx_type: TransactionType::Unstake,
            amount: CrapTokens(amount),
            timestamp: SystemTime::now(),
            description: "Unstaked tokens".to_string(),
            status: TransactionStatus::Confirmed,
            peer_id: None,
        });

        Ok(())
    }

    /// Claim pending rewards
    pub async fn claim_rewards(&self) -> Result<(), String> {
        let rewards = self.pending_rewards.read().await;

        if rewards.0 == 0 {
            return Err("No rewards to claim".to_string());
        }

        // Add rewards to balance
        let mut balance = self.balance.write().await;
        *balance = CrapTokens(balance.0 + rewards.0);

        // Clear pending rewards
        *self.pending_rewards.write().await = CrapTokens(0);

        // Add to transaction history
        let mut txs = self.transactions.write().await;
        txs.insert(0, TransactionDisplay {
            tx_type: TransactionType::StakingReward,
            amount: *rewards,
            timestamp: SystemTime::now(),
            description: "Claimed staking rewards".to_string(),
            status: TransactionStatus::Confirmed,
            peer_id: None,
        });

        Ok(())
    }

    /// Generate QR code for receiving
    pub async fn generate_receive_qr(&self) {
        let address = self.receive_address.read().await;
        // In real implementation, generate actual QR code
        *self.qr_code_data.write().await = Some(format!("bitcraps:{}", address));
    }

    /// Calculate total portfolio value
    pub async fn get_total_value(&self) -> CrapTokens {
        let balance = self.balance.read().await;
        let staked = self.staked_balance.read().await;
        let rewards = self.pending_rewards.read().await;

        CrapTokens(balance.0 + staked.0 + rewards.0)
    }
}

impl Screen for WalletScreen {
    fn render(&self, ctx: &mut RenderContext) {
        // Render header with balance
        self.render_header(ctx);

        // Render tab bar
        self.render_tabs(ctx);

        // Render current tab content
        let tab = self.selected_tab.try_read().ok()
            .and_then(|t| Some(t.clone()))
            .unwrap_or(WalletTab::Overview);

        match tab {
            WalletTab::Overview => self.render_overview(ctx),
            WalletTab::Send => self.render_send(ctx),
            WalletTab::Receive => self.render_receive(ctx),
            WalletTab::History => self.render_history(ctx),
            WalletTab::Staking => self.render_staking(ctx),
        }
    }

    fn handle_touch(&mut self, event: TouchEvent) -> Option<ScreenTransition> {
        match event {
            TouchEvent::Tap { x, y } => {
                // Handle tab selection
                if y >= 0.15 && y <= 0.25 {
                    let tab_index = (x * 5.0) as usize;
                    let new_tab = match tab_index {
                        0 => WalletTab::Overview,
                        1 => WalletTab::Send,
                        2 => WalletTab::Receive,
                        3 => WalletTab::History,
                        4 => WalletTab::Staking,
                        _ => return None,
                    };

                    if let Ok(mut tab) = self.selected_tab.try_write() {
                        *tab = new_tab;
                    }
                }

                // Handle back button
                if x >= 0.0 && x <= 0.1 && y >= 0.0 && y <= 0.1 {
                    return Some(ScreenTransition::Pop);
                }
            }
            TouchEvent::Swipe { start_y, end_y, .. } => {
                let delta = end_y - start_y;
                if delta > 0.1 {
                    // Scroll down
                    if let Ok(mut scroll) = self.scroll_offset.try_write() {
                        *scroll = (*scroll + 0.1).min(1.0);
                    }
                } else if delta < -0.1 {
                    // Scroll up
                    if let Ok(mut scroll) = self.scroll_offset.try_write() {
                        *scroll = (*scroll - 0.1).max(0.0);
                    }
                }
            }
            _ => {}
        }

        None
    }

    fn update(&mut self, _delta_time: Duration) {
        // Update pending rewards simulation
        if let Ok(mut rewards) = self.pending_rewards.try_write() {
            if let Ok(staked) = self.staked_balance.try_read() {
                // 5% APY calculated per second for demo
                let reward_rate = 0.05 / (365.0 * 24.0 * 3600.0);
                let new_reward = (staked.0 as f64 * reward_rate) as u64;
                *rewards = CrapTokens(rewards.0 + new_reward);
            }
        }
    }
}

// Rendering helper methods
impl WalletScreen {
    fn render_header(&self, ctx: &mut RenderContext) {
        // Background
        ctx.fill_rect(0.0, 0.0, 1.0, 0.15, (40, 40, 40, 255));

        // Back button
        ctx.draw_text("←", 0.02, 0.075, 24.0, (255, 255, 255, 255));

        // Title
        ctx.draw_text("Wallet", 0.5, 0.05, 20.0, (255, 255, 255, 255));

        // Balance
        if let Ok(balance) = self.balance.try_read() {
            ctx.draw_text(
                &format!("{} CRAP", balance.0),
                0.5, 0.1, 28.0, (100, 255, 100, 255)
            );
        }
    }

    fn render_tabs(&self, ctx: &mut RenderContext) {
        let tabs = ["Overview", "Send", "Receive", "History", "Staking"];
        let selected = self.selected_tab.try_read().ok()
            .and_then(|t| Some(t.clone()))
            .unwrap_or(WalletTab::Overview);

        for (i, tab_name) in tabs.iter().enumerate() {
            let x = i as f32 * 0.2;
            let is_selected = match (i, &selected) {
                (0, WalletTab::Overview) => true,
                (1, WalletTab::Send) => true,
                (2, WalletTab::Receive) => true,
                (3, WalletTab::History) => true,
                (4, WalletTab::Staking) => true,
                _ => false,
            };

            let color = if is_selected {
                (100, 150, 255, 255)
            } else {
                (150, 150, 150, 255)
            };

            ctx.fill_rect(x, 0.15, 0.2, 0.1, color);
            ctx.draw_text(tab_name, x + 0.1, 0.2, 12.0, (255, 255, 255, 255));
        }
    }

    fn render_overview(&self, ctx: &mut RenderContext) {
        // Portfolio breakdown
        ctx.draw_text("Portfolio", 0.1, 0.35, 18.0, (255, 255, 255, 255));

        if let Ok(balance) = self.balance.try_read() {
            ctx.draw_text(
                &format!("Available: {} CRAP", balance.0),
                0.1, 0.4, 14.0, (200, 200, 200, 255)
            );
        }

        if let Ok(staked) = self.staked_balance.try_read() {
            ctx.draw_text(
                &format!("Staked: {} CRAP", staked.0),
                0.1, 0.45, 14.0, (200, 200, 200, 255)
            );
        }

        if let Ok(rewards) = self.pending_rewards.try_read() {
            ctx.draw_text(
                &format!("Rewards: {} CRAP", rewards.0),
                0.1, 0.5, 14.0, (100, 255, 100, 255)
            );
        }

        // Quick actions
        ctx.draw_text("Quick Actions", 0.1, 0.6, 18.0, (255, 255, 255, 255));

        // Send button
        ctx.fill_rect(0.1, 0.65, 0.35, 0.1, (50, 100, 200, 255));
        ctx.draw_text("Send", 0.275, 0.7, 16.0, (255, 255, 255, 255));

        // Receive button
        ctx.fill_rect(0.55, 0.65, 0.35, 0.1, (50, 200, 100, 255));
        ctx.draw_text("Receive", 0.725, 0.7, 16.0, (255, 255, 255, 255));
    }

    fn render_send(&self, ctx: &mut RenderContext) {
        ctx.draw_text("Send CRAP Tokens", 0.1, 0.35, 18.0, (255, 255, 255, 255));

        // Amount input
        ctx.draw_text("Amount:", 0.1, 0.45, 14.0, (200, 200, 200, 255));
        ctx.fill_rect(0.1, 0.48, 0.8, 0.08, (60, 60, 60, 255));
        if let Ok(amount) = self.send_amount.try_read() {
            ctx.draw_text(&amount, 0.15, 0.52, 16.0, (255, 255, 255, 255));
        }

        // Address input
        ctx.draw_text("To Address:", 0.1, 0.6, 14.0, (200, 200, 200, 255));
        ctx.fill_rect(0.1, 0.63, 0.8, 0.08, (60, 60, 60, 255));
        if let Ok(address) = self.send_address.try_read() {
            ctx.draw_text(&address, 0.15, 0.67, 16.0, (255, 255, 255, 255));
        }

        // Send button
        ctx.fill_rect(0.25, 0.8, 0.5, 0.1, (50, 200, 50, 255));
        ctx.draw_text("Send Tokens", 0.5, 0.85, 18.0, (255, 255, 255, 255));
    }

    fn render_receive(&self, ctx: &mut RenderContext) {
        ctx.draw_text("Receive CRAP Tokens", 0.1, 0.35, 18.0, (255, 255, 255, 255));

        // QR code area
        ctx.fill_rect(0.25, 0.4, 0.5, 0.3, (255, 255, 255, 255));
        ctx.draw_text("QR CODE", 0.5, 0.55, 16.0, (0, 0, 0, 255));

        // Address display
        if let Ok(address) = self.receive_address.try_read() {
            ctx.draw_text("Your Address:", 0.1, 0.75, 14.0, (200, 200, 200, 255));
            ctx.draw_text(&address, 0.5, 0.8, 12.0, (150, 150, 150, 255));
        }

        // Copy button
        ctx.fill_rect(0.35, 0.85, 0.3, 0.08, (100, 100, 100, 255));
        ctx.draw_text("Copy", 0.5, 0.89, 14.0, (255, 255, 255, 255));
    }

    fn render_history(&self, ctx: &mut RenderContext) {
        ctx.draw_text("Transaction History", 0.1, 0.35, 18.0, (255, 255, 255, 255));

        if let Ok(txs) = self.transactions.try_read() {
            let offset = self.scroll_offset.try_read().ok()
                .and_then(|o| Some(*o))
                .unwrap_or(0.0);

            for (i, tx) in txs.iter().enumerate() {
                let y = 0.4 + (i as f32 * 0.12) - offset;

                if y < 0.3 || y > 0.9 {
                    continue;
                }

                // Transaction box
                ctx.fill_rect(0.05, y, 0.9, 0.1, (50, 50, 50, 255));

                // Type icon and amount
                let (icon, color) = match tx.tx_type {
                    TransactionType::GameWin => ("↑", (100, 255, 100, 255)),
                    TransactionType::GameLoss => ("↓", (255, 100, 100, 255)),
                    TransactionType::Transfer => ("→", (100, 150, 255, 255)),
                    _ => ("•", (200, 200, 200, 255)),
                };

                ctx.draw_text(icon, 0.1, y + 0.05, 20.0, color);
                ctx.draw_text(&format!("{} CRAP", tx.amount.0), 0.2, y + 0.04, 14.0, color);
                ctx.draw_text(&tx.description, 0.2, y + 0.07, 12.0, (180, 180, 180, 255));
            }
        }
    }

    fn render_staking(&self, ctx: &mut RenderContext) {
        ctx.draw_text("Staking", 0.1, 0.35, 18.0, (255, 255, 255, 255));

        // Current staking info
        if let Ok(staked) = self.staked_balance.try_read() {
            ctx.draw_text(
                &format!("Staked: {} CRAP", staked.0),
                0.1, 0.45, 16.0, (200, 200, 200, 255)
            );
        }

        if let Ok(rewards) = self.pending_rewards.try_read() {
            ctx.draw_text(
                &format!("Pending Rewards: {} CRAP", rewards.0),
                0.1, 0.5, 16.0, (100, 255, 100, 255)
            );
        }

        // APY display
        ctx.draw_text("Current APY: 5%", 0.1, 0.55, 14.0, (150, 150, 150, 255));

        // Stake button
        ctx.fill_rect(0.1, 0.65, 0.35, 0.1, (50, 100, 200, 255));
        ctx.draw_text("Stake", 0.275, 0.7, 16.0, (255, 255, 255, 255));

        // Unstake button
        ctx.fill_rect(0.55, 0.65, 0.35, 0.1, (200, 100, 50, 255));
        ctx.draw_text("Unstake", 0.725, 0.7, 16.0, (255, 255, 255, 255));

        // Claim rewards button
        ctx.fill_rect(0.25, 0.8, 0.5, 0.1, (50, 200, 50, 255));
        ctx.draw_text("Claim Rewards", 0.5, 0.85, 16.0, (255, 255, 255, 255));
    }
}