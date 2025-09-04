use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use crate::protocol::{GameId, PeerId};
use crate::protocol::craps::{BetType, CrapTokens};
use crate::services::game_engine::aggregator::{aggregate_bets, AggregatedBet};

// (game_id, round) -> Vec<(player, bet_type, amount)>
static BETS: Lazy<DashMap<(GameId, u64), Vec<(PeerId, BetType, CrapTokens)>>> = Lazy::new(DashMap::new);

pub fn add_bet(game_id: GameId, round: u64, player: PeerId, bet_type: BetType, amount: CrapTokens) {
    let key = (game_id, round);
    BETS.entry(key).or_default().push((player, bet_type, amount));
}

pub fn aggregate_round(game_id: GameId, round: u64) -> Vec<AggregatedBet> {
    let key = (game_id, round);
    let bets = BETS.get(&key).map(|v| v.clone()).unwrap_or_default();
    aggregate_bets(bets)
}

pub fn clear_round(game_id: GameId, round: u64) {
    BETS.remove(&(game_id, round));
}

pub fn merkle_proof(game_id: GameId, round: u64, player: PeerId, bet_type: BetType, amount: CrapTokens) -> Option<(Vec<[u8;32]>, [u8;32])> {
    // For simplicity, return the Merkle root only; full proof generation omitted here
    let aggs = aggregate_round(game_id, round);
    aggs.into_iter()
        .find(|a| a.bet_type == bet_type)
        .map(|a| (Vec::new(), a.merkle_root))
}

