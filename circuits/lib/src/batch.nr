// The batch state-transition relation: apply D deposits then N signed L2
// transactions to old_root, arriving at new_root, with the deposit and
// withdrawal lists bound to fold hashes the contract recomputes on-chain
// from its own queue / the envelope, and the full tx list bound to
// da_commitment (validium data availability - DESIGN.md "Batch circuit
// public interface").
use crate::tx::{apply_deposit, apply_tx, DepositWitness, TxWitness};

pub fn batch<let D: u32, let N: u32>(
    old_root: Field,
    new_root: Field,
    deposit_hash: Field,
    withdraw_hash: Field,
    da_commitment: Field,
    deposits: [DepositWitness; D],
    txs: [TxWitness; N],
) {
    let mut root = old_root;
    let mut dep_acc = 0;
    for i in 0..D {
        let (r, a) = apply_deposit(root, dep_acc, deposits[i]);
        root = r;
        dep_acc = a;
    }
    let mut wd_acc = 0;
    let mut da_acc = 0;
    for i in 0..N {
        let (r, w, d) = apply_tx(root, wd_acc, da_acc, txs[i]);
        root = r;
        wd_acc = w;
        da_acc = d;
    }
    assert(root == new_root);
    assert(dep_acc == deposit_hash);
    assert(wd_acc == withdraw_hash);
    assert(da_acc == da_commitment);
}
