//! Reward calculation engine

#[derive(Debug, Clone)]
pub struct RewardCalculator;

#[derive(Debug, Clone)]
pub enum NetworkAction {
    RelayMessage,
    StoreMessage,
    MaintainUptime,
}
