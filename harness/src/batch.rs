//! Builds a complete batch witness against the account tree, mirroring
//! circuits/lib/src/{tx,batch}.nr exactly: same application order (deposits
//! then txs, sender then recipient), same fold hashes (deposit, withdraw,
//! DA), same padding convention (identity proof at slot 0 + the PAD
//! signature).
//!
//! Inputs are USER-SIGNED transactions ([`SignedTx`]) — the sequencer never
//! holds user secret keys. Every admission failure is a typed error so the
//! sequencer can reject one bad mempool entry and rebuild.

use crate::keys::{pad_signature, pk_from_coords, verify, Keypair, Signature};
use crate::poseidon::{fr_from_u64, Fr, Hasher, FR_ZERO};
use crate::tree::{Account, Tree, DEPTH};

pub const DOMAIN_TX: u64 = 2;
pub const DOMAIN_DEP: u64 = 4;
pub const DOMAIN_WD: u64 = 5;
pub const DOMAIN_DA: u64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    SenderNotFound { tx_index: usize },
    NonceMismatch { tx_index: usize, expected: u64, got: u64 },
    InsufficientBalance { tx_index: usize, balance: u64, amount: u64 },
    RecipientNotFound { tx_index: usize },
    BadSignature { tx_index: usize },
    ZeroDepositPk,
    DepositPkMismatch { deposit_index: usize },
    BalanceOverflow { deposit_index: usize },
    TreeFull,
    TooManyEntries,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BuildError {}

pub struct DepositRequest {
    pub pk_x: Fr,
    pub amount: u64,
}

/// A user-signed L2 transaction as received over the wire.
#[derive(Debug, Clone)]
pub struct SignedTx {
    pub from_pk_x: Fr,
    pub from_pk_y: Fr,
    /// Recipient pk_x (transfer) or address_to_field(dest) (withdrawal).
    pub to_field: Fr,
    pub amount: u64,
    /// The nonce this signature covers; must equal the sender's tree nonce
    /// at application time.
    pub nonce: u64,
    pub is_withdraw: bool,
    pub sig: Signature,
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
    pub da_commitment: Fr,
    pub deposits: Vec<DepositEntry>,
    pub txs: Vec<TxEntry>,
}

fn fold(hasher: &Hasher, domain: u64, acc: Fr, a: Fr, b: Fr) -> Fr {
    hasher.hash(&[fr_from_u64(domain), acc, a, b])
}

/// 3-input fold used by the DA commitment: acc' = P2([domain, acc, x]).
fn fold3(hasher: &Hasher, domain: u64, acc: Fr, x: Fr) -> Fr {
    hasher.hash(&[fr_from_u64(domain), acc, x])
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

/// Sign a transaction for tests/demos (production wallets sign client-side).
pub fn make_signed_tx(
    hasher: &Hasher,
    from: &Keypair,
    to_field: Fr,
    amount: u64,
    nonce: u64,
    is_withdraw: bool,
    rng: &mut impl rand::RngCore,
) -> SignedTx {
    let msg = tx_message(hasher, from.pk_x(), to_field, amount, nonce, is_withdraw);
    SignedTx {
        from_pk_x: from.pk_x(),
        from_pk_y: from.pk_y(),
        to_field,
        amount,
        nonce,
        is_withdraw,
        sig: crate::keys::sign(hasher, from, msg, rng),
    }
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
    txs: &[SignedTx],
) -> Result<BatchWitness, BuildError> {
    if deposits.len() > d_slots || txs.len() > n_slots {
        return Err(BuildError::TooManyEntries);
    }
    let old_root = tree.root(hasher);
    let mut deposit_hash = FR_ZERO;
    let mut withdraw_hash = FR_ZERO;
    let mut da_commitment = FR_ZERO;

    let mut dep_entries = Vec::new();
    for (i, req) in deposits.iter().enumerate() {
        if req.pk_x == FR_ZERO {
            return Err(BuildError::ZeroDepositPk);
        }
        let index = tree
            .find(&req.pk_x)
            .or_else(|| tree.free_index())
            .ok_or(BuildError::TreeFull)?;
        let old = tree.get(index).cloned();
        let (siblings, _) = tree.path(hasher, index);
        let new = match &old {
            None => Account { pk_x: req.pk_x, balance: req.amount, nonce: 0 },
            Some(a) => {
                if a.pk_x != req.pk_x {
                    return Err(BuildError::DepositPkMismatch { deposit_index: i });
                }
                let balance = a
                    .balance
                    .checked_add(req.amount)
                    .ok_or(BuildError::BalanceOverflow { deposit_index: i })?;
                Account { pk_x: req.pk_x, balance, nonce: a.nonce }
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
    for (i, req) in txs.iter().enumerate() {
        let from_index = tree
            .find(&req.from_pk_x)
            .ok_or(BuildError::SenderNotFound { tx_index: i })?;
        let sender = tree.get(from_index).cloned().unwrap();
        if sender.nonce != req.nonce {
            return Err(BuildError::NonceMismatch { tx_index: i, expected: sender.nonce, got: req.nonce });
        }
        if sender.balance < req.amount {
            return Err(BuildError::InsufficientBalance {
                tx_index: i,
                balance: sender.balance,
                amount: req.amount,
            });
        }

        // Belt-and-braces signature check (the circuit is the final arbiter,
        // but an unprovable batch must never reach the prover).
        let msg = tx_message(hasher, req.from_pk_x, req.to_field, req.amount, req.nonce, req.is_withdraw);
        let pk = pk_from_coords(&req.from_pk_x, &req.from_pk_y)
            .ok_or(BuildError::BadSignature { tx_index: i })?;
        if !verify(hasher, &pk, msg, &req.sig) {
            return Err(BuildError::BadSignature { tx_index: i });
        }

        let (from_siblings, _) = tree.path(hasher, from_index);

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
            let to_index = tree
                .find(&req.to_field)
                .ok_or(BuildError::RecipientNotFound { tx_index: i })?;
            let recipient = tree.get(to_index).cloned().unwrap();
            let (siblings, _) = tree.path(hasher, to_index);
            let credited = recipient
                .balance
                .checked_add(req.amount)
                .ok_or(BuildError::BalanceOverflow { deposit_index: i })?;
            tree.set(
                to_index,
                Account { pk_x: recipient.pk_x, balance: credited, nonce: recipient.nonce },
            );
            (to_index, fr_from_u64(recipient.balance), recipient.nonce, siblings)
        };

        if req.is_withdraw {
            withdraw_hash = fold(hasher, DOMAIN_WD, withdraw_hash, req.to_field, fr_from_u64(req.amount));
        }
        da_commitment = fold3(hasher, DOMAIN_DA, da_commitment, msg);

        tx_entries.push(TxEntry {
            from_pk_x: req.from_pk_x,
            from_pk_y: req.from_pk_y,
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
            sig: req.sig.clone(),
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

    Ok(BatchWitness {
        old_root,
        new_root: tree.root(hasher),
        deposit_hash,
        withdraw_hash,
        da_commitment,
        deposits: dep_entries,
        txs: tx_entries,
    })
}
