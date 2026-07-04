// Batch size N=16 transactions, D=4 deposits. Same public interface as
// batch_n4 (see DESIGN.md).
use rollup_lib::batch::batch;
use rollup_lib::tx::{DepositWitness, TxWitness};

fn main(
    old_root: pub Field,
    new_root: pub Field,
    deposit_hash: pub Field,
    withdraw_hash: pub Field,
    deposits: [DepositWitness; 4],
    txs: [TxWitness; 16],
) {
    batch(old_root, new_root, deposit_hash, withdraw_hash, deposits, txs);
}
