//! Grumpkin Schnorr signing, mirroring circuits/lib/src/schnorr.nr.
//!
//! Field naming is a minefield: Grumpkin and BN254 form a 2-cycle, so
//! `ark_grumpkin::Fq` (coordinates) IS the BN254 scalar field — the circuit
//! `Field` and our 32-byte `Fr` type — while `ark_grumpkin::Fr` (scalars) is
//! the larger BN254 base field, which is why the signature scalar `s` crosses
//! into the circuit as two 128-bit limbs instead of one Field.
//!
//! Scheme (DESIGN.md): R = k·G; e = Poseidon2([DOMAIN_SIG, R.x, pk_x, msg]);
//! s = k + e·sk (mod Grumpkin scalar order). Verify: s·G == R + e·pk.

use crate::poseidon::{fr_from_u64, Fr, Hasher};
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{BigInteger, PrimeField, UniformRand};
use ark_grumpkin::{Affine, Fq as Coord, Fr as Scalar};

pub const DOMAIN_SIG: u64 = 3;

pub fn coord_to_fr(c: &Coord) -> Fr {
    let bytes = c.into_bigint().to_bytes_be();
    let mut out = [0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

pub struct Keypair {
    pub sk: Scalar,
    pub pk: Affine,
}

impl Keypair {
    pub fn generate(rng: &mut impl rand::RngCore) -> Self {
        Self::from_sk(Scalar::rand(rng))
    }

    pub fn from_sk(sk: Scalar) -> Self {
        let pk = (Affine::generator() * sk).into_affine();
        Self { sk, pk }
    }

    pub fn pk_x(&self) -> Fr {
        coord_to_fr(&self.pk.x)
    }

    pub fn pk_y(&self) -> Fr {
        coord_to_fr(&self.pk.y)
    }
}

#[derive(Clone, Debug)]
pub struct Signature {
    pub r_x: Fr,
    pub r_y: Fr,
    pub s: Scalar,
}

impl Signature {
    /// The circuit takes `s` as EmbeddedCurveScalar { lo, hi } 128-bit limbs:
    /// scalar = lo + hi * 2^128. Returned as 32-byte BE field encodings.
    pub fn s_limbs(&self) -> (Fr, Fr) {
        let bytes = self.s.into_bigint().to_bytes_be();
        let mut full = [0u8; 32];
        full[32 - bytes.len()..].copy_from_slice(&bytes);
        let mut lo = [0u8; 32];
        let mut hi = [0u8; 32];
        lo[16..].copy_from_slice(&full[16..]);
        hi[16..].copy_from_slice(&full[..16]);
        (lo, hi)
    }

    /// Reconstruct from wire format (untrusted wallet input): s = lo + hi·2^128.
    /// Rejects limbs that don't fit 128 bits — a non-canonical split would
    /// make the wire encoding ambiguous.
    pub fn from_limbs(r_x: Fr, r_y: Fr, s_lo: Fr, s_hi: Fr) -> Option<Signature> {
        if s_lo[..16] != [0u8; 16] || s_hi[..16] != [0u8; 16] {
            return None;
        }
        let mut full = [0u8; 32];
        full[..16].copy_from_slice(&s_hi[16..]);
        full[16..].copy_from_slice(&s_lo[16..]);
        // BigInt < 2^256; reduction into the scalar field is fine here: any
        // s' ≥ n is equivalent to s' mod n for verification, and the limb
        // check above already pins the byte encoding uniquely.
        let s = Scalar::from_be_bytes_mod_order(&full);
        Some(Signature { r_x, r_y, s })
    }
}

/// Reconstruct a public key from untrusted wire coordinates, checking curve
/// membership (Grumpkin has cofactor 1, so on-curve implies correct subgroup).
pub fn pk_from_coords(pk_x: &Fr, pk_y: &Fr) -> Option<Affine> {
    let p = Affine::new_unchecked(
        Coord::from_be_bytes_mod_order(pk_x),
        Coord::from_be_bytes_mod_order(pk_y),
    );
    if !p.is_on_curve() || p.infinity {
        return None;
    }
    Some(p)
}

/// The Schnorr challenge. Its output is a BN254-Fr element; lifting it into
/// the Grumpkin scalar field is reduction-free because Fr < Fq_scalar.
fn challenge(hasher: &Hasher, r_x: Fr, pk_x: Fr, msg: Fr) -> Scalar {
    let e = hasher.hash(&[fr_from_u64(DOMAIN_SIG), r_x, pk_x, msg]);
    Scalar::from_be_bytes_mod_order(&e)
}

pub fn sign(hasher: &Hasher, keypair: &Keypair, msg: Fr, rng: &mut impl rand::RngCore) -> Signature {
    sign_with_nonce(hasher, keypair, msg, Scalar::rand(rng))
}

/// Deterministic-nonce variant for pinned test vectors.
pub fn sign_with_nonce(hasher: &Hasher, keypair: &Keypair, msg: Fr, k: Scalar) -> Signature {
    let r = (Affine::generator() * k).into_affine();
    let r_x = coord_to_fr(&r.x);
    let e = challenge(hasher, r_x, keypair.pk_x(), msg);
    Signature {
        r_x,
        r_y: coord_to_fr(&r.y),
        s: k + e * keypair.sk,
    }
}

/// The fixed padding signature baked into circuits/lib/src/tx.nr (PAD_*
/// globals): sk=7, k=13, msg=42. Public by design; padded batch entries
/// re-verify it instead of predicating the MSM.
pub fn pad_signature(hasher: &Hasher) -> Signature {
    let keypair = Keypair::from_sk(Scalar::from(7u64));
    sign_with_nonce(hasher, &keypair, fr_from_u64(42), Scalar::from(13u64))
}

/// Off-chain sanity check with the same equation the circuit enforces.
pub fn verify(hasher: &Hasher, pk: &Affine, msg: Fr, sig: &Signature) -> bool {
    let pk_x = coord_to_fr(&pk.x);
    let e = challenge(hasher, sig.r_x, pk_x, msg);
    let s_g = (Affine::generator() * sig.s).into_affine();
    let r = Affine::new_unchecked(
        Coord::from_be_bytes_mod_order(&sig.r_x),
        Coord::from_be_bytes_mod_order(&sig.r_y),
    );
    if !r.is_on_curve() {
        return false;
    }
    let rhs = (r + *pk * e).into_affine();
    s_g == rhs
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::Field as _;

    #[test]
    fn sign_verify_roundtrip() {
        let hasher = Hasher::new();
        let mut rng = rand::thread_rng();
        let keypair = Keypair::generate(&mut rng);
        let msg = fr_from_u64(42);
        let sig = sign(&hasher, &keypair, msg, &mut rng);
        assert!(verify(&hasher, &keypair.pk, msg, &sig));

        // Tampered message fails.
        assert!(!verify(&hasher, &keypair.pk, fr_from_u64(43), &sig));
    }

    #[test]
    fn limb_split_reconstructs() {
        let hasher = Hasher::new();
        let keypair = Keypair::from_sk(Scalar::from(7u64));
        let sig = sign_with_nonce(&hasher, &keypair, fr_from_u64(42), Scalar::from(13u64));
        let (lo, hi) = sig.s_limbs();
        // lo + hi * 2^128 == s
        let lo_int = Scalar::from_be_bytes_mod_order(&lo);
        let hi_int = Scalar::from_be_bytes_mod_order(&hi);
        let two_128 = Scalar::from(2u64).pow([128u64]);
        assert_eq!(lo_int + hi_int * two_128, sig.s);
        // hi limb's top 16 bytes are zero by construction
        assert_eq!(hi[..16], [0u8; 16]);
    }
}
