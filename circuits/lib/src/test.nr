// M2 checkpoint: byte-for-byte agreement with the Rust harness
// (`cargo run -p harness -- vectors` regenerates these constants; the same
// values are pinned in harness/src/tree.rs tests via the shared code path).
use crate::account::{leaf_value, Account};
use crate::merkle::{compute_root, hash2, update_root, DEPTH};
use dep::poseidon::poseidon2::Poseidon2;

global HASH2_1_2: Field = 0x038682aa1cb5ae4e0a3f13da432a95c77c5c111f6f030faf9cad641ce1ed7383;
global HASH4_1_2_3_4: Field = 0x130bf204a32cac1f0ace56c78b731aa3809f06df2731ebcf6b3464a15788b1b9;
global EMPTY_ROOT_D8: Field = 0x067243231eddf4222f3911defbba7705aff06ed45960b27f6f91319196ef97e1;
global LEAF_1234_100_0: Field = 0x08e85e74234a7e239d871de3fbd4ffba29f93e4c5eab3e47a50abe2dad310ecc;
global ROOT_LEAF_AT_5: Field = 0x185afc939981ef9614d18623d87027a99db0117e16009e87db82b7bfab7865cb;

fn zeros() -> [Field; DEPTH + 1] {
    let mut z = [0; DEPTH + 1];
    for i in 0..DEPTH {
        z[i + 1] = hash2(z[i], z[i]);
    }
    z
}

#[test]
fn hash2_matches_harness() {
    assert(hash2(1, 2) == HASH2_1_2);
}

#[test]
fn hash4_matches_harness() {
    assert(Poseidon2::hash([1, 2, 3, 4], 4) == HASH4_1_2_3_4);
}

#[test]
fn empty_root_matches_harness() {
    let z = zeros();
    assert(z[DEPTH] == EMPTY_ROOT_D8);
}

#[test]
fn leaf_hash_matches_harness() {
    let leaf = leaf_value(Account { pk_x: 1234, balance: 100, nonce: 0 });
    assert(leaf == LEAF_1234_100_0);
}

#[test]
fn empty_account_is_zero_leaf() {
    assert(leaf_value(Account { pk_x: 0, balance: 0, nonce: 0 }) == 0);
}

#[test]
fn single_leaf_root_matches_harness() {
    // Insert leaf(1234, 100, 0) at index 5 of the empty depth-8 tree:
    // path bits of 5 LSB-first = [1,0,1,0,0,0,0,0]; siblings are the zero
    // ladder of an otherwise-empty tree.
    let z = zeros();
    let mut siblings = [0; DEPTH];
    for i in 0..DEPTH {
        siblings[i] = z[i];
    }
    let bits = [1, 0, 1, 0, 0, 0, 0, 0];
    let leaf = leaf_value(Account { pk_x: 1234, balance: 100, nonce: 0 });
    assert(compute_root(leaf, siblings, bits) == ROOT_LEAF_AT_5);
}

#[test]
fn update_root_roundtrip() {
    let z = zeros();
    let mut siblings = [0; DEPTH];
    for i in 0..DEPTH {
        siblings[i] = z[i];
    }
    let bits = [1, 0, 1, 0, 0, 0, 0, 0];
    let leaf = leaf_value(Account { pk_x: 1234, balance: 100, nonce: 0 });
    // empty -> leaf at index 5 must land on the harness root.
    let new_root = update_root(0, leaf, siblings, bits, EMPTY_ROOT_D8);
    assert(new_root == ROOT_LEAF_AT_5);
}

#[test(should_fail)]
fn update_root_rejects_wrong_old_root() {
    let z = zeros();
    let mut siblings = [0; DEPTH];
    for i in 0..DEPTH {
        siblings[i] = z[i];
    }
    let bits = [1, 0, 1, 0, 0, 0, 0, 0];
    let _ = update_root(0, 1, siblings, bits, ROOT_LEAF_AT_5);
}
