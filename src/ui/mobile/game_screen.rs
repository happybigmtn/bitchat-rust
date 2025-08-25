//! Mobile game screen implementation
//! 
//! Provides the core game UI for mobile platforms with touch-optimized controls

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use crate::protocol::{DiceRoll, BetType, CrapTokens};
use crate::gaming::GamePhase;
use super::screen_base::{Screen, ScreenElement, Theme, TouchEvent, RenderContext, ScreenTransition};

/// Main game screen for active gameplay
pub struct GameScreen {
    game_id: String,
    phase: Arc<RwLock<GamePhase>>,
    current_bet: Arc<RwLock<Option<ActiveBet>>>,
    last_roll: Arc<RwLock<Option<DiceRoll>>>,
    balance: Arc<RwLock<CrapTokens>>,
    pot_size: Arc<RwLock<CrapTokens>>,
    players: Arc<RwLock<Vec<PlayerInfo>>>,
    message_log: Arc<RwLock<Vec<GameMessage>>>,
    animation_state: Arc<RwLock<AnimationState>>,
    selected_bet_type: Arc<RwLock<BetType>>,
    bet_amount: Arc<RwLock<u64>>,
}

/// Active bet information
#[derive(Debug, Clone)]
struct ActiveBet {
    bet_type: BetType,
    amount: CrapTokens,
    placed_at: SystemTime,
    potential_payout: CrapTokens,
}

/// Player information for display
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: String,
    pub name: String,
    pub balance: CrapTokens,
    pub current_bet: Option<CrapTokens>,
    pub is_shooter: bool,
    pub avatar_color: (u8, u8, u8),
}

/// Game message for the activity log
#[derive(Debug, Clone)]
pub struct GameMessage {
    pub timestamp: SystemTime,
    pub sender: String,
    pub content: String,
    pub message_type: MessageType,
}

#[derive(Debug, Clone)]
pub enum MessageType {
    System,
    Chat,
    Bet,
    Roll,
    Win,
    Loss,
}

/// Animation state for dice and chips
#[derive(Debug, Clone)]
struct AnimationState {
    dice_rolling: bool,
    dice_rotation: (f32, f32),
    dice_position: (f32, f32),
    chip_animations: Vec<ChipAnimation>,
    celebration_active: bool,
}

#[derive(Debug, Clone)]
struct ChipAnimation {
    start_pos: (f32, f32),
    end_pos: (f32, f32),
    progress: f32,
    amount: CrapTokens,
}

impl GameScreen {
    /// Create a new game screen
    pub fn new(game_id: String) -> Self {
        Self {
            game_id,
            phase: Arc::new(RwLock::new(GamePhase::WaitingForPlayers)),
            current_bet: Arc::new(RwLock::new(None)),
            last_roll: Arc::new(RwLock::new(None)),
            balance: Arc::new(RwLock::new(CrapTokens(1000))),
            pot_size: Arc::new(RwLock::new(CrapTokens(0))),
            players: Arc::new(RwLock::new(Vec::new())),
            message_log: Arc::new(RwLock::new(Vec::new())),
            animation_state: Arc::new(RwLock::new(AnimationState {
                dice_rolling: false,
                dice_rotation: (0.0, 0.0),
                dice_position: (0.5, 0.3),
                chip_animations: Vec::new(),
                celebration_active: false,
            })),
            selected_bet_type: Arc::new(RwLock::new(BetType::Pass)),
            bet_amount: Arc::new(RwLock::new(10)),
        }
    }
    
    /// Handle placing a bet
    pub async fn place_bet(&self, bet_type: BetType, amount: u64) -> Result<(), String> {
        let balance = *self.balance.read().await;
        
        if amount > balance.0 {
            return Err("Insufficient balance".to_string());
        }
        
        // Deduct from balance
        *self.balance.write().await = CrapTokens(balance.0 - amount);
        
        // Add to pot
        let mut pot = self.pot_size.write().await;
        *pot = CrapTokens(pot.0 + amount);
        
        // Set current bet
        *self.current_bet.write().await = Some(ActiveBet {
            bet_type,
            amount: CrapTokens(amount),
            placed_at: SystemTime::now(),
            potential_payout: Self::calculate_payout(bet_type, amount),
        });
        
        // Add to message log
        self.add_message(
            "You".to_string(),
            format!("Placed {} CRAP on {:?}", amount, bet_type),
            MessageType::Bet,
        ).await;
        
        // Trigger chip animation
        let mut anim = self.animation_state.write().await;
        anim.chip_animations.push(ChipAnimation {
            start_pos: (0.5, 0.8),
            end_pos: Self::get_bet_position(bet_type),
            progress: 0.0,
            amount: CrapTokens(amount),
        });
        
        Ok(())
    }
    
    /// Handle rolling dice
    pub async fn roll_dice(&self) -> Result<DiceRoll, String> {
        // Start dice animation
        let mut anim = self.animation_state.write().await;
        anim.dice_rolling = true;
        anim.dice_rotation = (0.0, 0.0);
        
        // Simulate dice roll (in real implementation, this would use consensus)
        let die1 = (rand::random::<u8>() % 6) + 1;
        let die2 = (rand::random::<u8>() % 6) + 1;
        
        let roll = DiceRoll {
            die1,
            die2,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        *self.last_roll.write().await = Some(roll.clone());
        
        // Add to message log
        self.add_message(
            "Shooter".to_string(),
            format!("Rolled {} + {} = {}", die1, die2, die1 + die2),
            MessageType::Roll,
        ).await;
        
        // Process the roll result
        self.process_roll_result(&roll).await;
        
        Ok(roll)
    }
    
    /// Process the result of a dice roll
    async fn process_roll_result(&self, roll: &DiceRoll) {
        let total = roll.die1 + roll.die2;
        let phase = self.phase.read().await.clone();
        
        match phase {
            GamePhase::ComeOut => {
                match total {
                    7 | 11 => {
                        // Natural - Pass line wins
                        self.resolve_bets(true).await;
                        self.add_message(
                            "System".to_string(),
                            "Natural! Pass line wins!".to_string(),
                            MessageType::Win,
                        ).await;
                    }
                    2 | 3 | 12 => {
                        // Craps - Don't pass wins
                        self.resolve_bets(false).await;
                        self.add_message(
                            "System".to_string(),
                            "Craps! Don't pass wins!".to_string(),
                            MessageType::Loss,
                        ).await;
                    }
                    point => {
                        // Point established
                        *self.phase.write().await = GamePhase::Point(point);
                        self.add_message(
                            "System".to_string(),
                            format!("Point is {}", point),
                            MessageType::System,
                        ).await;
                    }
                }
            }
            GamePhase::Point(point) => {
                if total == point {
                    // Point made - Pass line wins
                    self.resolve_bets(true).await;
                    self.add_message(
                        "System".to_string(),
                        "Point made! Pass line wins!".to_string(),
                        MessageType::Win,
                    ).await;
                    *self.phase.write().await = GamePhase::ComeOut;
                } else if total == 7 {
                    // Seven out - Don't pass wins
                    self.resolve_bets(false).await;
                    self.add_message(
                        "System".to_string(),
                        "Seven out! Don't pass wins!".to_string(),
                        MessageType::Loss,
                    ).await;
                    *self.phase.write().await = GamePhase::ComeOut;
                }
            }
            _ => {}
        }
    }
    
    /// Resolve bets based on outcome
    async fn resolve_bets(&self, pass_wins: bool) {
        if let Some(bet) = self.current_bet.read().await.as_ref() {
            let won = match bet.bet_type {
                BetType::Pass => pass_wins,
                BetType::DontPass => !pass_wins,
                _ => false,
            };
            
            if won {
                // Add winnings to balance
                let winnings = bet.potential_payout.0;
                let mut balance = self.balance.write().await;
                *balance = CrapTokens(balance.0 + winnings);
                
                // Trigger celebration
                let mut anim = self.animation_state.write().await;
                anim.celebration_active = true;
                
                self.add_message(
                    "You".to_string(),
                    format!("Won {} CRAP!", winnings),
                    MessageType::Win,
                ).await;
            } else {
                self.add_message(
                    "You".to_string(),
                    format!("Lost {} CRAP", bet.amount.0),
                    MessageType::Loss,
                ).await;
            }
            
            // Clear current bet
            *self.current_bet.write().await = None;
        }
    }
    
    /// Add a message to the game log
    async fn add_message(&self, sender: String, content: String, message_type: MessageType) {
        let mut log = self.message_log.write().await;
        log.push(GameMessage {
            timestamp: SystemTime::now(),
            sender,
            content,
            message_type,
        });
        
        // Keep only last 50 messages
        if log.len() > 50 {
            log.remove(0);
        }
    }
    
    /// Calculate potential payout for a bet
    fn calculate_payout(bet_type: BetType, amount: u64) -> CrapTokens {
        let multiplier = match bet_type {
            BetType::Pass | BetType::DontPass => 2.0,
            BetType::Field => 2.0,
            BetType::Any7 => 5.0,
            BetType::AnyCraps => 8.0,
            BetType::Hardway(_) => 10.0,
            _ => 2.0,
        };
        
        CrapTokens((amount as f64 * multiplier) as u64)
    }
    
    /// Get visual position for bet placement
    fn get_bet_position(bet_type: BetType) -> (f32, f32) {
        match bet_type {
            BetType::Pass => (0.3, 0.5),
            BetType::DontPass => (0.7, 0.5),
            BetType::Field => (0.5, 0.6),
            BetType::Any7 => (0.5, 0.4),
            BetType::AnyCraps => (0.5, 0.7),
            _ => (0.5, 0.5),
        }
    }
    
    /// Update animations
    pub async fn update_animations(&self, delta_time: Duration) {
        let mut anim = self.animation_state.write().await;
        
        // Update dice rolling animation
        if anim.dice_rolling {
            anim.dice_rotation.0 += delta_time.as_secs_f32() * 720.0;
            anim.dice_rotation.1 += delta_time.as_secs_f32() * 540.0;
            
            // Stop after 2 seconds
            if anim.dice_rotation.0 > 1440.0 {
                anim.dice_rolling = false;
            }
        }
        
        // Update chip animations
        anim.chip_animations.retain_mut(|chip| {
            chip.progress += delta_time.as_secs_f32() * 2.0;
            chip.progress < 1.0
        });
        
        // Update celebration
        if anim.celebration_active {
            // Auto-disable after 3 seconds
            // In real implementation, track time properly
            anim.celebration_active = false;
        }
    }
}

impl Screen for GameScreen {
    fn render(&self, ctx: &mut RenderContext) {
        // Render game table background
        self.render_table(ctx);
        
        // Render dice
        self.render_dice(ctx);
        
        // Render betting areas
        self.render_betting_areas(ctx);
        
        // Render chips
        self.render_chips(ctx);
        
        // Render player info
        self.render_players(ctx);
        
        // Render controls
        self.render_controls(ctx);
        
        // Render message log
        self.render_message_log(ctx);
        
        // Render animations
        self.render_animations(ctx);
    }
    
    fn handle_touch(&mut self, event: TouchEvent) -> Option<ScreenTransition> {
        match event {
            TouchEvent::Tap { x, y } => {
                // Handle betting area taps
                if self.is_betting_area(x, y) {
                    // Place bet logic
                }
                
                // Handle control button taps
                if self.is_roll_button(x, y) {
                    // Roll dice logic
                }
            }
            _ => {}
        }
        
        None
    }
    
    fn update(&mut self, _delta_time: Duration) {
        // Update is handled by async methods
    }
}

// Rendering helper methods
impl GameScreen {
    fn render_table(&self, ctx: &mut RenderContext) {
        // Draw green felt table
        ctx.fill_rect(0.0, 0.0, 1.0, 1.0, (0, 100, 0, 255));
        
        // Draw betting area outlines
        ctx.draw_rect(0.2, 0.4, 0.2, 0.2, (255, 255, 255, 255), 2.0);
        ctx.draw_text("PASS", 0.3, 0.5, 16.0, (255, 255, 255, 255));
        
        ctx.draw_rect(0.6, 0.4, 0.2, 0.2, (255, 255, 255, 255), 2.0);
        ctx.draw_text("DON'T", 0.7, 0.5, 16.0, (255, 255, 255, 255));
    }
    
    fn render_dice(&self, _ctx: &mut RenderContext) {
        // Render dice with current animation state
    }
    
    fn render_betting_areas(&self, _ctx: &mut RenderContext) {
        // Highlight active betting areas
    }
    
    fn render_chips(&self, _ctx: &mut RenderContext) {
        // Render placed chips with animations
    }
    
    fn render_players(&self, _ctx: &mut RenderContext) {
        // Show connected players
    }
    
    fn render_controls(&self, ctx: &mut RenderContext) {
        // Render bet amount selector
        ctx.fill_rect(0.1, 0.8, 0.3, 0.1, (50, 50, 50, 200));
        ctx.draw_text("BET: 10 CRAP", 0.25, 0.85, 14.0, (255, 255, 255, 255));
        
        // Render roll button
        ctx.fill_rect(0.6, 0.8, 0.3, 0.1, (200, 50, 50, 255));
        ctx.draw_text("ROLL DICE", 0.75, 0.85, 16.0, (255, 255, 255, 255));
    }
    
    fn render_message_log(&self, _ctx: &mut RenderContext) {
        // Show recent game messages
    }
    
    fn render_animations(&self, _ctx: &mut RenderContext) {
        // Render active animations
    }
    
    fn is_betting_area(&self, _x: f32, _y: f32) -> bool {
        // Check if touch is in betting area
        false
    }
    
    fn is_roll_button(&self, x: f32, y: f32) -> bool {
        x >= 0.6 && x <= 0.9 && y >= 0.8 && y <= 0.9
    }
}

// Add rand for dice simulation (temporary)
use rand;