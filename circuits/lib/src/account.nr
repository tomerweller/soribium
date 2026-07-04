// Account leaf encoding shared with harness tree.rs and the contract.
// leaf = 0 when the slot is empty, else Poseidon2([DOMAIN_LEAF, pk_x, balance, nonce], 4).
// An account with pk_x == 0 is treated as the empty slot (spike simplification).
use dep::poseidon::poseidon2::Poseidon2;

pub global DOMAIN_LEAF: Field = 1;

pub struct Account {
    pub pk_x: Field,
    pub balance: Field,
    pub nonce: Field,
}

pub fn leaf_value(a: Account) -> Field {
    if a.pk_x == 0 {
        0
    } else {
        Poseidon2::hash([DOMAIN_LEAF, a.pk_x, a.balance, a.nonce], 4)
    }
}
