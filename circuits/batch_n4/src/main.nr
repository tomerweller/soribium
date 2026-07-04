// Batch size N=4 transactions, D=2 deposits. Public inputs (exactly 4,
// order matters - the contract assembles the same blob): old_root, new_root,
// deposit_hash, withdraw_hash.
use rollup_lib::batch::batch;
use rollup_lib::tx::{DepositWitness, TxWitness};

fn main(
    old_root: pub Field,
    new_root: pub Field,
    deposit_hash: pub Field,
    withdraw_hash: pub Field,
    deposits: [DepositWitness; 2],
    txs: [TxWitness; 4],
) {
    batch(old_root, new_root, deposit_hash, withdraw_hash, deposits, txs);
}
