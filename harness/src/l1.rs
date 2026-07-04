//! L1 (Soroban) interop helpers shared with the contract.

use crate::poseidon::{fr_from_u64, Fr, Hasher};

pub const DOMAIN_ADDR: u64 = 6;

/// Compress a Soroban address (56-char strkey, G... or C...) into one field
/// element: Poseidon2([DOMAIN_ADDR, limb0, limb1], 3) over the two 28-byte
/// halves of the ASCII strkey, each read as a big-endian integer (28 bytes
/// < 32, so both limbs are canonical field elements by construction).
/// The contract implements the same function over `address.to_string()`.
pub fn address_to_field(hasher: &Hasher, strkey: &str) -> Fr {
    let bytes = strkey.as_bytes();
    assert_eq!(bytes.len(), 56, "expected 56-char strkey, got {}", bytes.len());
    let mut limb0 = [0u8; 32];
    let mut limb1 = [0u8; 32];
    limb0[4..].copy_from_slice(&bytes[..28]);
    limb1[4..].copy_from_slice(&bytes[28..]);
    hasher.hash(&[fr_from_u64(DOMAIN_ADDR), limb0, limb1])
}
