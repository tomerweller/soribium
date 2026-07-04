//! Public-input assembly. All public inputs are 32-byte big-endian canonical
//! BN254 scalar-field encodings. The host's field conversion silently reduces
//! non-canonical values mod r, so every appended word must be checked — a
//! non-canonical encoding accepted here would let two different byte strings
//! verify against the same proof.

use soroban_sdk::{crypto::BnScalar, Bytes, BytesN, Env, U256};

/// BN254 scalar field modulus r, big-endian.
const BN254_R: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
    0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00,
    0x00, 0x01,
];

pub fn is_canonical_field(word: &[u8; 32]) -> bool {
    *word < BN254_R
}

/// Append one canonical field word to the public-input blob, panicking on a
/// non-canonical encoding.
pub fn append_field(env: &Env, blob: &mut Bytes, word: &BytesN<32>) {
    let arr = word.to_array();
    assert!(is_canonical_field(&arr), "non-canonical field encoding");
    blob.append(&Bytes::from_array(env, &arr));
}

/// Canonicalize arbitrary 32 bytes into the scalar field (used only where the
/// input is by-construction a hash output, never for prover-supplied words).
pub fn reduce_to_field(env: &Env, word: &BytesN<32>) -> U256 {
    let modulus = <BnScalar as soroban_poseidon::Field>::modulus(env);
    U256::from_be_bytes(env, &Bytes::from_array(env, &word.to_array())).rem_euclid(&modulus)
}
