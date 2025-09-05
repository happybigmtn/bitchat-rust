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

/// Compute a Merkle branch (sibling hashes up the tree) for a given leaf index.
fn compute_merkle_branch(contributors: &[Contributor], mut index: usize) -> Vec<[u8;32]> {
    use sha2::{Digest, Sha256};
    if contributors.is_empty() || index >= contributors.len() { return Vec::new(); }

    // Build initial leaves
    let mut level: Vec<[u8;32]> = contributors.iter().map(|c| {
        let mut h = Sha256::new();
        h.update(&c.player);
        h.update(c.amount.0.to_le_bytes());
        h.finalize().into()
    }).collect();

    let mut branch: Vec<[u8;32]> = Vec::new();
    while level.len() > 1 {
        let is_right = index % 2 == 1;
        let sibling_index = if is_right { index - 1 } else { index + 1 };
        if sibling_index < level.len() {
            branch.push(level[sibling_index]);
        }

        // Build next level
        let mut next: Vec<[u8;32]> = Vec::with_capacity((level.len() + 1) / 2);
        for chunk in level.chunks(2) {
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
        level = next;
        index /= 2;
    }
    branch
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

/// Public helper to compute inclusion branch and root for a specific bet entry
pub fn inclusion_proof(
    contributors: &[Contributor],
    player: &PeerId,
    amount: CrapTokens,
) -> Option<(Vec<[u8;32]>, [u8;32])> {
    // Locate index of leaf
    let idx = contributors.iter().position(|c| &c.player == player && c.amount.0 == amount.0)?;
    let branch = compute_merkle_branch(contributors, idx);
    let root = compute_merkle_root(contributors);
    Some((branch, root))
}
