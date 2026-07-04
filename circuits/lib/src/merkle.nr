// Poseidon2 Merkle tree, aligned with the harness (tree.rs) and the contract:
// node = Poseidon2([l, r], 2); empty leaf = 0; zero[i+1] = H(zero[i], zero[i]).
// The leaf index is decomposed in-circuit (LSB-first; bit i = 0 means the
// running node is the LEFT child at level i), which also range-checks
// index < 2^DEPTH.
use dep::poseidon::poseidon2::Poseidon2;

pub global DEPTH: u32 = 8;

pub fn hash2(a: Field, b: Field) -> Field {
    Poseidon2::hash([a, b], 2)
}

pub fn compute_root(leaf: Field, index: Field, siblings: [Field; DEPTH]) -> Field {
    let bits: [u1; DEPTH] = index.to_le_bits();
    let mut cur = leaf;
    for i in 0..DEPTH {
        if bits[i] == 0 {
            cur = hash2(cur, siblings[i]);
        } else {
            cur = hash2(siblings[i], cur);
        }
    }
    cur
}

/// One leaf update: proves `old_leaf` is in `old_root` at `index`, and
/// returns the root with that leaf replaced by `new_leaf` (same siblings -
/// nothing else in the tree moved).
pub fn update_root(
    old_leaf: Field,
    new_leaf: Field,
    index: Field,
    siblings: [Field; DEPTH],
    old_root: Field,
) -> Field {
    assert(compute_root(old_leaf, index, siblings) == old_root);
    compute_root(new_leaf, index, siblings)
}
