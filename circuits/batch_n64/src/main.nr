// Batch size N=64 transactions, D=8 deposits - kept for prover-cost scaling
// measurements. Same 5-public-input interface as batch_n16 (see DESIGN.md).
use rollup_lib::batch::batch;
use rollup_lib::tx::{DepositWitness, TxWitness};

fn main(
    old_root: pub Field,
    new_root: pub Field,
    deposit_hash: pub Field,
    withdraw_hash: pub Field,
    da_commitment: pub Field,
    deposits: [DepositWitness; 8],
    txs: [TxWitness; 64],
) {
    batch(old_root, new_root, deposit_hash, withdraw_hash, da_commitment, deposits, txs);
}
