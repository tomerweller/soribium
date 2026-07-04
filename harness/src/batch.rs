//! Builds a complete batch witness against the account tree, mirroring
//! circuits/lib/src/{tx,batch}.nr exactly: same application order (deposits
//! then txs, sender then recipient), same fold hashes, same padding
//! convention (identity proof at slot 0 + the PAD signature).

use crate::keys::{pad_signature, sign, Keypair, Signature};
use crate::poseidon::{fr_from_u64, Fr, Hasher, FR_ZERO};
use crate::tree::{Account, Tree, DEPTH};

pub const DOMAIN_TX: u64 = 2;
pub const DOMAIN_DEP: u64 = 4;
pub const DOMAIN_WD: u64 = 5;

pub struct DepositRequest {
    pub pk_x: Fr,
    pub amount: u64,
}

pub struct TxRequest<'a> {
    pub from: &'a Keypair,
    /// Recipient pk_x (transfer) or address_to_field(dest) (withdrawal).
    pub to_field: Fr,
    pub amount: u64,
    pub is_withdraw: bool,
}

#[derive(Debug, Clone)]
pub struct DepositEntry {
    pub pk_x: Fr,
    pub amount: u64,
    pub index: u32,
    pub old_pk_x: Fr,
    pub old_balance: u64,
    pub old_nonce: u64,
    pub siblings: [Fr; DEPTH],
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct TxEntry {
    pub from_pk_x: Fr,
    pub from_pk_y: Fr,
    pub from_index: u32,
    pub from_balance: u64,
    pub from_nonce: u64,
    pub from_siblings: [Fr; DEPTH],
    pub to_field: Fr,
    pub to_index: u32,
    /// Transfer: recipient balance. Otherwise: RAW leaf value at to_index.
    pub to_balance_or_leaf: Fr,
    pub to_nonce: u64,
    pub to_siblings: [Fr; DEPTH],
    pub amount: u64,
    pub is_withdraw: bool,
    pub is_active: bool,
    pub sig: Signature,
}

pub struct BatchWitness {
    pub old_root: Fr,
    pub new_root: Fr,
    pub deposit_hash: Fr,
    pub withdraw_hash: Fr,
    pub deposits: Vec<DepositEntry>,
    pub txs: Vec<TxEntry>,
}

fn fold(hasher: &Hasher, domain: u64, acc: Fr, a: Fr, b: Fr) -> Fr {
    hasher.hash(&[fr_from_u64(domain), acc, a, b])
}

pub fn tx_message(hasher: &Hasher, from_pk_x: Fr, to_field: Fr, amount: u64, nonce: u64, is_withdraw: bool) -> Fr {
    hasher.hash(&[
        fr_from_u64(DOMAIN_TX),
        from_pk_x,
        to_field,
        fr_from_u64(amount),
        fr_from_u64(nonce),
        fr_from_u64(is_withdraw as u64),
    ])
}

/// Raw leaf value currently at `index` (0 for empty slots).
fn raw_leaf(hasher: &Hasher, tree: &Tree, index: u32) -> Fr {
    Tree::leaf_value(hasher, tree.get(index))
}

pub fn build_batch(
    hasher: &Hasher,
    tree: &mut Tree,
    d_slots: usize,
    n_slots: usize,
    deposits: &[DepositRequest],
    txs: &[TxRequest],
    rng: &mut impl rand::RngCore,
) -> BatchWitness {
    assert!(deposits.len() <= d_slots && txs.len() <= n_slots);
    let old_root = tree.root(hasher);
    let mut deposit_hash = FR_ZERO;
    let mut withdraw_hash = FR_ZERO;

    let mut dep_entries = Vec::new();
    for req in deposits {
        assert!(req.pk_x != FR_ZERO);
        let index = tree
            .find(&req.pk_x)
            .or_else(|| tree.free_index())
            .expect("tree full");
        let old = tree.get(index).cloned();
        let (siblings, _) = tree.path(hasher, index);
        let new = match &old {
            None => Account { pk_x: req.pk_x, balance: req.amount, nonce: 0 },
            Some(a) => {
                assert_eq!(a.pk_x, req.pk_x, "slot pk mismatch");
                Account { pk_x: req.pk_x, balance: a.balance + req.amount, nonce: a.nonce }
            }
        };
        tree.set(index, new);
        deposit_hash = fold(hasher, DOMAIN_DEP, deposit_hash, req.pk_x, fr_from_u64(req.amount));
        dep_entries.push(DepositEntry {
            pk_x: req.pk_x,
            amount: req.amount,
            index,
            old_pk_x: old.as_ref().map(|a| a.pk_x).unwrap_or(FR_ZERO),
            old_balance: old.as_ref().map(|a| a.balance).unwrap_or(0),
            old_nonce: old.as_ref().map(|a| a.nonce).unwrap_or(0),
            siblings,
            is_active: true,
        });
    }
    // Deposit padding: identity update of slot 0.
    while dep_entries.len() < d_slots {
        let old = tree.get(0).cloned();
        let (siblings, _) = tree.path(hasher, 0);
        dep_entries.push(DepositEntry {
            pk_x: FR_ZERO,
            amount: 0,
            index: 0,
            old_pk_x: old.as_ref().map(|a| a.pk_x).unwrap_or(FR_ZERO),
            old_balance: old.as_ref().map(|a| a.balance).unwrap_or(0),
            old_nonce: old.as_ref().map(|a| a.nonce).unwrap_or(0),
            siblings,
            is_active: false,
        });
    }

    let mut tx_entries = Vec::new();
    for req in txs {
        let from_pk_x = req.from.pk_x();
        let from_index = tree.find(&from_pk_x).expect("sender not in tree");
        let sender = tree.get(from_index).cloned().unwrap();
        let (from_siblings, _) = tree.path(hasher, from_index);
        assert!(sender.balance >= req.amount, "insufficient balance");

        let msg = tx_message(hasher, from_pk_x, req.to_field, req.amount, sender.nonce, req.is_withdraw);
        let sig = sign(hasher, req.from, msg, rng);

        // Debit sender.
        tree.set(
            from_index,
            Account {
                pk_x: sender.pk_x,
                balance: sender.balance - req.amount,
                nonce: sender.nonce + 1,
            },
        );

        let (to_index, to_balance_or_leaf, to_nonce, to_siblings) = if req.is_withdraw {
            // Identity update of slot 0 against the post-debit tree.
            let (siblings, _) = tree.path(hasher, 0);
            (0u32, raw_leaf(hasher, tree, 0), 0u64, siblings)
        } else {
            let to_index = tree.find(&req.to_field).expect("recipient not in tree");
            let recipient = tree.get(to_index).cloned().unwrap();
            let (siblings, _) = tree.path(hasher, to_index);
            tree.set(
                to_index,
                Account {
                    pk_x: recipient.pk_x,
                    balance: recipient.balance + req.amount,
                    nonce: recipient.nonce,
                },
            );
            (to_index, fr_from_u64(recipient.balance), recipient.nonce, siblings)
        };

        if req.is_withdraw {
            withdraw_hash = fold(hasher, DOMAIN_WD, withdraw_hash, req.to_field, fr_from_u64(req.amount));
        }

        tx_entries.push(TxEntry {
            from_pk_x,
            from_pk_y: req.from.pk_y(),
            from_index,
            from_balance: sender.balance,
            from_nonce: sender.nonce,
            from_siblings,
            to_field: req.to_field,
            to_index,
            to_balance_or_leaf,
            to_nonce,
            to_siblings,
            amount: req.amount,
            is_withdraw: req.is_withdraw,
            is_active: true,
        sig,
        });
    }
    // Tx padding: identity updates of slot 0 with the PAD signature.
    while tx_entries.len() < n_slots {
        let slot0 = tree.get(0).cloned();
        let (siblings, _) = tree.path(hasher, 0);
        tx_entries.push(TxEntry {
            from_pk_x: slot0.as_ref().map(|a| a.pk_x).unwrap_or(FR_ZERO),
            from_pk_y: FR_ZERO,
            from_index: 0,
            from_balance: slot0.as_ref().map(|a| a.balance).unwrap_or(0),
            from_nonce: slot0.as_ref().map(|a| a.nonce).unwrap_or(0),
            from_siblings: siblings,
            to_field: FR_ZERO,
            to_index: 0,
            to_balance_or_leaf: raw_leaf(hasher, tree, 0),
            to_nonce: 0,
            to_siblings: siblings,
            amount: 0,
            is_withdraw: false,
            is_active: false,
            sig: pad_signature(hasher),
        });
    }

    BatchWitness {
        old_root,
        new_root: tree.root(hasher),
        deposit_hash,
        withdraw_hash,
        deposits: dep_entries,
        txs: tx_entries,
    }
}
