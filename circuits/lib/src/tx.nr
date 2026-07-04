// Batch entry application. Padding strategy: EVERY constraint always runs on
// valid witness data - inactive entries carry a real identity Merkle proof
// (new leaf == old leaf, so the root can't move) and the fixed PAD signature
// below (a public constant; proving it leaks nothing and changes no state).
// This avoids predicated blackbox calls (MSM on garbage points) entirely.
// `is_active` then only gates the deposit/withdraw fold accumulators, which
// is what binds entries to the on-chain recomputed hashes.
use crate::account::{leaf_value, Account, DOMAIN_LEAF};
use crate::merkle::{update_root, DEPTH};
use crate::schnorr::{verify_sig, Signature};
use dep::poseidon::poseidon2::Poseidon2;

pub global DOMAIN_TX: Field = 2;
pub global DOMAIN_DEP: Field = 4;
pub global DOMAIN_WD: Field = 5;

// Fixed padding signature: sk=7, k=13, msg=42 over Grumpkin (harness
// keys::pad_signature() regenerates; schnorr_test pins the same values).
pub global PAD_PK_X: Field = 0x0e602b9dd6a3e8d039a17f069add3f9c2a187a8f629a1de60a33a8067b9b2842;
pub global PAD_PK_Y: Field = 0x14cc8e83df1b5cbb163bd2c94005cb0707fe570def5a165242b1c1419cb014cb;
pub global PAD_MSG: Field = 42;
pub global PAD_R_X: Field = 0x17e6386beac25fd11573fe37404e61d754a91ee7722dbd0c1c7fff157ed56573;
pub global PAD_R_Y: Field = 0x2be1ab9ccce96a8deeb82e85a35cc26804269db7ea57bc0e0afa3b835dfcff73;
pub global PAD_S_LO: Field = 0x000000000000000000000000000000008312af54a4f4b07d782dc23d06ef43f2;
pub global PAD_S_HI: Field = 0x000000000000000000000000000000002ed87ed8cf30832f5f7b93ee4d5afd52;

fn assert_u64(v: Field) {
    let _: [u1; 64] = v.to_le_bits();
}

fn select(cond: Field, if_true: Field, if_false: Field) -> Field {
    assert(cond * (1 - cond) == 0);
    cond * if_true + (1 - cond) * if_false
}

pub struct DepositWitness {
    pub pk_x: Field,
    pub amount: Field,
    pub index: Field,
    // Pre-state of the target slot (pk_x == 0 means empty).
    pub old_pk_x: Field,
    pub old_balance: Field,
    pub old_nonce: Field,
    pub siblings: [Field; DEPTH],
    pub is_active: Field,
}

/// Apply one deposit; returns (new_root, new_deposit_fold).
pub fn apply_deposit(root: Field, dep_acc: Field, d: DepositWitness) -> (Field, Field) {
    assert(d.is_active * (1 - d.is_active) == 0);
    assert_u64(d.amount);

    let old_leaf = leaf_value(Account { pk_x: d.old_pk_x, balance: d.old_balance, nonce: d.old_nonce });

    // Empty slot: initialize. Occupied: pk must match, balance accrues.
    let is_empty = if d.old_pk_x == 0 { 1 } else { 0 };
    if d.is_active == 1 {
        assert(d.pk_x != 0);
        if is_empty == 0 {
            assert(d.old_pk_x == d.pk_x);
        }
    }
    let new_balance = select(is_empty, d.amount, d.old_balance + d.amount);
    assert_u64(new_balance);
    let new_nonce = select(is_empty, 0, d.old_nonce);
    let new_leaf_active = leaf_value(Account { pk_x: d.pk_x, balance: new_balance, nonce: new_nonce });

    // Inactive entries are identity updates: new leaf forced to old leaf.
    let new_leaf = select(d.is_active, new_leaf_active, old_leaf);
    let new_root = update_root(old_leaf, new_leaf, d.index, d.siblings, root);

    let folded = Poseidon2::hash([DOMAIN_DEP, dep_acc, d.pk_x, d.amount], 4);
    let new_acc = select(d.is_active, folded, dep_acc);

    (new_root, new_acc)
}

pub struct TxWitness {
    pub from_pk_x: Field,
    pub from_pk_y: Field,
    pub from_index: Field,
    pub from_balance: Field,
    pub from_nonce: Field,
    pub from_siblings: [Field; DEPTH],
    // Transfer: recipient pk_x. Withdrawal: address_to_field(destination).
    pub to_field: Field,
    // Recipient pre-state (transfers only; ignored for withdrawals).
    pub to_index: Field,
    pub to_balance: Field,
    pub to_nonce: Field,
    pub to_siblings: [Field; DEPTH],
    pub amount: Field,
    pub is_withdraw: Field,
    pub is_active: Field,
    pub sig: Signature,
}

/// Apply one signed L2 transaction; returns (new_root, new_withdraw_fold).
pub fn apply_tx(root: Field, wd_acc: Field, t: TxWitness) -> (Field, Field) {
    assert(t.is_active * (1 - t.is_active) == 0);
    assert(t.is_withdraw * (1 - t.is_withdraw) == 0);
    assert_u64(t.amount);

    // --- signature over the tx message (constant PAD check when inactive) ---
    let msg = Poseidon2::hash(
        [DOMAIN_TX, t.from_pk_x, t.to_field, t.amount, t.from_nonce, t.is_withdraw],
        6,
    );
    let pk_x = select(t.is_active, t.from_pk_x, PAD_PK_X);
    let pk_y = select(t.is_active, t.from_pk_y, PAD_PK_Y);
    let sig_msg = select(t.is_active, msg, PAD_MSG);
    let sig = Signature {
        r_x: select(t.is_active, t.sig.r_x, PAD_R_X),
        r_y: select(t.is_active, t.sig.r_y, PAD_R_Y),
        s_lo: select(t.is_active, t.sig.s_lo, PAD_S_LO),
        s_hi: select(t.is_active, t.sig.s_hi, PAD_S_HI),
    };
    verify_sig(pk_x, pk_y, sig_msg, sig);

    // --- debit sender (identity when inactive) ---
    if t.is_active == 1 {
        assert(t.from_pk_x != 0);
    }
    let old_sender = leaf_value(Account { pk_x: t.from_pk_x, balance: t.from_balance, nonce: t.from_nonce });
    let debited = t.from_balance - t.amount;
    // Underflow wraps to a huge field element and fails the 64-bit check.
    let checked_debited = select(t.is_active, debited, 0);
    assert_u64(checked_debited);
    let new_sender = leaf_value(Account {
        pk_x: t.from_pk_x,
        balance: debited,
        nonce: t.from_nonce + 1,
    });
    let sender_leaf = select(t.is_active, new_sender, old_sender);
    let root1 = update_root(old_sender, sender_leaf, t.from_index, t.from_siblings, root);

    // --- credit recipient (transfers) or fold withdrawal ---
    // Witness convention: for an active transfer, (to_index, to_balance,
    // to_nonce, to_siblings) describe the recipient ACCOUNT under root1; the
    // recipient must exist with pk_x == to_field (an empty slot can't produce
    // a valid inclusion proof for a non-zero leaf, so existence is
    // structural). Otherwise (withdrawal or padding) to_balance carries the
    // RAW leaf value at to_index and the update is an identity.
    let is_transfer = t.is_active * (1 - t.is_withdraw);
    if is_transfer == 1 {
        assert(t.to_field != 0);
    }
    let credited = t.to_balance + t.amount;
    assert_u64(select(is_transfer, credited, 0));
    let recipient_old = leaf_value(Account { pk_x: t.to_field, balance: t.to_balance, nonce: t.to_nonce });
    let recipient_new = leaf_value(Account { pk_x: t.to_field, balance: credited, nonce: t.to_nonce });
    let old_leaf1 = select(is_transfer, recipient_old, t.to_balance);
    let new_leaf1 = select(is_transfer, recipient_new, t.to_balance);
    let root2 = update_root(old_leaf1, new_leaf1, t.to_index, t.to_siblings, root1);

    let is_withdrawal = t.is_active * t.is_withdraw;
    let folded = Poseidon2::hash([DOMAIN_WD, wd_acc, t.to_field, t.amount], 4);
    let new_wd_acc = select(is_withdrawal, folded, wd_acc);

    (root2, new_wd_acc)
}
