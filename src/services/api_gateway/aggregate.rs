use dashmap::{DashMap, DashSet};
use once_cell::sync::Lazy;
use crate::protocol::{GameId, PeerId};
use crate::protocol::craps::{BetType, CrapTokens};
use crate::services::game_engine::aggregator::{aggregate_bets, AggregatedBet, inclusion_proof, Contributor};

// (game_id, round) -> Vec<(player, bet_type, amount)>
static BETS: Lazy<DashMap<(GameId, u64), Vec<(PeerId, BetType, CrapTokens)>>> = Lazy::new(DashMap::new);
static GAMES: Lazy<DashSet<GameId>> = Lazy::new(DashSet::new);
static ROUNDS: Lazy<DashMap<GameId, u64>> = Lazy::new(DashMap::new);

pub fn add_bet(game_id: GameId, round: u64, player: PeerId, bet_type: BetType, amount: CrapTokens) {
    let key = (game_id, round);
    BETS.entry(key).or_default().push((player, bet_type, amount));
    GAMES.insert(game_id);
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
    let aggs = aggregate_round(game_id, round);
    let agg = aggs.into_iter().find(|a| a.bet_type == bet_type)?;
    inclusion_proof(&agg.contributors, &player, amount)
}

pub fn list_games() -> Vec<GameId> {
    GAMES.iter().map(|e| *e.key()).collect()
}

pub fn current_round(game_id: GameId) -> u64 {
    if let Some(r) = ROUNDS.get(&game_id) { *r.value() } else { 0 }
}

pub fn advance_round(game_id: GameId) -> u64 {
    let mut next = 1;
    if let Some(mut r) = ROUNDS.get_mut(&game_id) {
        next = *r + 1;
        *r = next;
        return next;
    }
    ROUNDS.insert(game_id, next);
    next
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AggregatedGroup {
    pub bet_type: BetType,
    pub total_amount: u64,
    pub merkle_root: [u8;32],
}

pub fn aggregated_groups(game_id: GameId, round: u64) -> Vec<AggregatedGroup> {
    aggregate_round(game_id, round)
        .into_iter()
        .map(|a| AggregatedGroup { bet_type: a.bet_type, total_amount: a.total_amount.0, merkle_root: a.merkle_root })
        .collect()
}
