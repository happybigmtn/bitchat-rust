//! Bet Aggregation module
//! Groups identical bets to reduce on-chain consensus load and produces a merkle root for contributors.

use crate::protocol::{PeerId, Hash256};
use crate::protocol::craps::{BetType, CrapTokens};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Contributor {
    pub player: PeerId,
    pub amount: CrapTokens,
}

#[derive(Debug, Clone)]
pub struct AggregatedBet {
    pub bet_type: BetType,
    pub total_amount: CrapTokens,
    pub contributors: Vec<Contributor>,
    pub merkle_root: Hash256,
}

/// Compute a simple merkle root over (player||amount) leaves
fn compute_merkle_root(contributors: &[Contributor]) -> Hash256 {
    if contributors.is_empty() {
        return [0u8; 32];
    }
    let mut leaves: Vec<[u8; 32]> = contributors
        .iter()
        .map(|c| {
            let mut h = Sha256::new();
            h.update(&c.player);
            h.update(c.amount.0.to_le_bytes());
            h.finalize().into()
        })
        .collect();

    while leaves.len() > 1 {
        let mut next = Vec::with_capacity((leaves.len() + 1) / 2);
        for chunk in leaves.chunks(2) {
            let combined = if chunk.len() == 2 {
                let mut h = Sha256::new();
                h.update(&chunk[0]);
                h.update(&chunk[1]);
                h.finalize().into()
            } else {
                chunk[0]
            };
            next.push(combined);
        }
        leaves = next;
    }
    leaves[0]
}

/// Aggregate a list of (player, bet_type, amount) into groups of identical bet_type
pub fn aggregate_bets(bets: Vec<(PeerId, BetType, CrapTokens)>) -> Vec<AggregatedBet> {
    use std::collections::HashMap;
    let mut groups: HashMap<BetType, Vec<Contributor>> = HashMap::new();
    for (player, bet_type, amount) in bets {
        groups.entry(bet_type).or_default().push(Contributor { player, amount });
    }
    let mut results = Vec::with_capacity(groups.len());
    for (bet_type, contributors) in groups.into_iter() {
        let total = contributors.iter().fold(CrapTokens(0), |acc, c| CrapTokens(acc.0 + c.amount.0));
        let merkle = compute_merkle_root(&contributors);
        results.push(AggregatedBet { bet_type, total_amount: total, contributors, merkle_root: merkle });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_aggregate_simple() {
        let p1 = [1u8; 32];
        let p2 = [2u8; 32];
        let bets = vec![
            (p1, BetType::Pass, CrapTokens(5)),
            (p2, BetType::Pass, CrapTokens(10)),
        ];
        let agg = aggregate_bets(bets);
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].bet_type, BetType::Pass);
        assert_eq!(agg[0].total_amount.0, 15);
        assert_ne!(agg[0].merkle_root, [0u8; 32]);
    }
}

