// Batch size N=4 transactions, D=2 deposits - kept for fast contract-test
// fixtures. Public inputs (exactly 5, order matters - the contract assembles
// the same blob): old_root, new_root, deposit_hash, withdraw_hash,
// da_commitment.
use rollup_lib::batch::batch;
use rollup_lib::tx::{DepositWitness, TxWitness};

fn main(
    old_root: pub Field,
    new_root: pub Field,
    deposit_hash: pub Field,
    withdraw_hash: pub Field,
    da_commitment: pub Field,
    deposits: [DepositWitness; 2],
    txs: [TxWitness; 4],
) {
    batch(old_root, new_root, deposit_hash, withdraw_hash, da_commitment, deposits, txs);
}
