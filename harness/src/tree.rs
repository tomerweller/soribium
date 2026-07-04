//! The sequencer's account tree. Must stay byte-identical to the circuit
//! (circuits/lib/src/{merkle,account}.nr) and the contract's view of roots —
//! shared test vectors are pinned in both test suites.

use crate::poseidon::{fr_from_u64, Fr, Hasher, FR_ZERO};
use serde::{Deserialize, Serialize};

pub const DEPTH: usize = 8;
pub const N_LEAVES: usize = 1 << DEPTH;
pub const DOMAIN_LEAF: u64 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub pk_x: Fr,
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Tree {
    /// Sparse leaf storage: index -> account. Empty slots hash to 0.
    pub leaves: std::collections::BTreeMap<u32, Account>,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn leaf_value(hasher: &Hasher, account: Option<&Account>) -> Fr {
        match account {
            None => FR_ZERO,
            Some(a) => hasher.hash(&[
                fr_from_u64(DOMAIN_LEAF),
                a.pk_x,
                fr_from_u64(a.balance),
                fr_from_u64(a.nonce),
            ]),
        }
    }

    fn level0(&self, hasher: &Hasher) -> Vec<Fr> {
        (0..N_LEAVES as u32)
            .map(|i| Self::leaf_value(hasher, self.leaves.get(&i)))
            .collect()
    }

    /// Full recompute; 255 hashes at depth 8, fine for the spike.
    pub fn root(&self, hasher: &Hasher) -> Fr {
        let mut level = self.level0(hasher);
        while level.len() > 1 {
            level = level
                .chunks(2)
                .map(|pair| hasher.hash2(pair[0], pair[1]))
                .collect();
        }
        level[0]
    }

    /// Merkle path for `index`: (siblings bottom-up, index bits LSB-first).
    /// bit i == 0 means the running node is the left child at level i.
    pub fn path(&self, hasher: &Hasher, index: u32) -> ([Fr; DEPTH], [u8; DEPTH]) {
        assert!((index as usize) < N_LEAVES);
        let mut siblings = [FR_ZERO; DEPTH];
        let mut bits = [0u8; DEPTH];
        let mut level = self.level0(hasher);
        let mut idx = index as usize;
        for (i, sibling) in siblings.iter_mut().enumerate() {
            bits[i] = (idx & 1) as u8;
            *sibling = level[idx ^ 1];
            level = level
                .chunks(2)
                .map(|pair| hasher.hash2(pair[0], pair[1]))
                .collect();
            idx >>= 1;
        }
        (siblings, bits)
    }

    pub fn get(&self, index: u32) -> Option<&Account> {
        self.leaves.get(&index)
    }

    pub fn set(&mut self, index: u32, account: Account) {
        assert!((index as usize) < N_LEAVES);
        self.leaves.insert(index, account);
    }

    /// Find the index holding pk_x, if any.
    pub fn find(&self, pk_x: &Fr) -> Option<u32> {
        self.leaves
            .iter()
            .find(|(_, a)| &a.pk_x == pk_x)
            .map(|(i, _)| *i)
    }

    /// Lowest empty slot.
    pub fn free_index(&self) -> Option<u32> {
        (0..N_LEAVES as u32).find(|i| !self.leaves.contains_key(i))
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap())
    }

    pub fn load(path: &std::path::Path) -> std::io::Result<Self> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poseidon::to_hex;

    #[test]
    fn path_verifies_against_root() {
        let hasher = Hasher::new();
        let mut tree = Tree::new();
        tree.set(
            5,
            Account {
                pk_x: fr_from_u64(1234),
                balance: 100,
                nonce: 0,
            },
        );
        let root = tree.root(&hasher);
        let (siblings, bits) = tree.path(&hasher, 5);

        // Recompute the root from the leaf + path, mirroring merkle.nr.
        let mut cur = Tree::leaf_value(&hasher, tree.get(5));
        for i in 0..DEPTH {
            cur = if bits[i] == 0 {
                hasher.hash2(cur, siblings[i])
            } else {
                hasher.hash2(siblings[i], cur)
            };
        }
        assert_eq!(to_hex(&cur), to_hex(&root));
    }
}
