//! Public-input assembly. All public inputs are 32-byte big-endian canonical
//! BN254 scalar-field encodings. The host's field conversion silently reduces
//! non-canonical values mod r, so every appended word must be checked — a
//! non-canonical encoding accepted here would let two different byte strings
//! stand for the same field element.
//!
//! Fold hashes and domain constants mirror circuits/lib/src/tx.nr and
//! harness/src/{batch,l1}.rs; DESIGN.md is the spec.

use soroban_poseidon::poseidon2_hash;
use soroban_sdk::{crypto::BnScalar, Address, Bytes, BytesN, Env, U256, Vec as SVec};

pub const DOMAIN_DEP: u32 = 4;
pub const DOMAIN_WD: u32 = 5;
pub const DOMAIN_ADDR: u32 = 6;

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

fn u256_from_word(env: &Env, word: &BytesN<32>) -> U256 {
    U256::from_be_bytes(env, &Bytes::from_array(env, &word.to_array()))
}

fn word_from_u256(env: &Env, v: &U256) -> BytesN<32> {
    let bytes = v.to_be_bytes();
    let mut arr = [0u8; 32];
    bytes.copy_into_slice(&mut arr);
    BytesN::from_array(env, &arr)
}

fn poseidon4(env: &Env, a: U256, b: U256, c: U256, d: U256) -> U256 {
    let mut inputs: SVec<U256> = SVec::new(env);
    inputs.push_back(a);
    inputs.push_back(b);
    inputs.push_back(c);
    inputs.push_back(d);
    poseidon2_hash::<4, BnScalar>(env, &inputs)
}

/// One fold step: acc' = Poseidon2([domain, acc, a, b]). `amount` must
/// already be validated < 2^64 (deposit()/submit_batch() enforce it).
pub fn fold(env: &Env, domain: u32, acc: &BytesN<32>, a: &BytesN<32>, amount: i128) -> BytesN<32> {
    let out = poseidon4(
        env,
        U256::from_u32(env, domain),
        u256_from_word(env, acc),
        u256_from_word(env, a),
        U256::from_u128(env, amount as u128),
    );
    word_from_u256(env, &out)
}

/// Compress a Soroban address into one field element:
/// Poseidon2([DOMAIN_ADDR, limb0, limb1]) over the two 28-byte halves of the
/// 56-char ASCII strkey (harness/src/l1.rs is the mirror implementation).
pub fn address_to_field(env: &Env, addr: &Address) -> BytesN<32> {
    let strkey = addr.to_string();
    let mut ascii = [0u8; 56];
    assert_eq!(strkey.len(), 56, "expected 56-char strkey");
    strkey.copy_into_slice(&mut ascii);

    let mut limb0 = [0u8; 32];
    let mut limb1 = [0u8; 32];
    limb0[4..].copy_from_slice(&ascii[..28]);
    limb1[4..].copy_from_slice(&ascii[28..]);

    let mut inputs: SVec<U256> = SVec::new(env);
    inputs.push_back(U256::from_u32(env, DOMAIN_ADDR));
    inputs.push_back(U256::from_be_bytes(env, &Bytes::from_array(env, &limb0)));
    inputs.push_back(U256::from_be_bytes(env, &Bytes::from_array(env, &limb1)));
    let out = poseidon2_hash::<4, BnScalar>(env, &inputs);
    word_from_u256(env, &out)
}

pub const FR_ZERO_WORD: [u8; 32] = [0u8; 32];
