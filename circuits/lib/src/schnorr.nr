// Schnorr over Grumpkin with a Poseidon2 challenge (DESIGN.md). Hand-rolled:
// std::schnorr no longer exists and noir-lang/schnorr's Blake2s challenge
// would dominate the batch circuit's gate count.
//
// verify: s*G == R + e*pk, with e = Poseidon2([DOMAIN_SIG, R.x, pk_x, msg], 4).
// `s` lives in Grumpkin's scalar field (= BN254 base field, LARGER than the
// circuit Field), so it crosses as 128-bit limbs {s_lo, s_hi}. `e` is a
// circuit Field; from_field is reduction-free since Fr < Grumpkin scalar order.
use dep::poseidon::poseidon2::Poseidon2;
use std::embedded_curve_ops::{multi_scalar_mul, EmbeddedCurvePoint, EmbeddedCurveScalar};

pub global DOMAIN_SIG: Field = 3;

// Grumpkin's canonical generator, pinned from ark-grumpkin (see
// `cargo run -p harness -- sig-vectors`; must match Affine::generator()).
pub global GEN: EmbeddedCurvePoint = EmbeddedCurvePoint {
    x: 1,
    y: 0x0000000000000002cf135e7506a45d632d270d45f1181294833fc48d823f272c,
    is_infinite: false,
};

pub struct Signature {
    pub r_x: Field,
    pub r_y: Field,
    pub s_lo: Field,
    pub s_hi: Field,
}

pub fn verify_sig(pk_x: Field, pk_y: Field, msg: Field, sig: Signature) {
    let pk = EmbeddedCurvePoint { x: pk_x, y: pk_y, is_infinite: false };
    let r = EmbeddedCurvePoint { x: sig.r_x, y: sig.r_y, is_infinite: false };

    let e = Poseidon2::hash([DOMAIN_SIG, sig.r_x, pk_x, msg], 4);
    let s = EmbeddedCurveScalar { lo: sig.s_lo, hi: sig.s_hi };

    let s_g = multi_scalar_mul([GEN], [s]);
    let e_pk = multi_scalar_mul([pk], [EmbeddedCurveScalar::from_field(e)]);
    let rhs = r + e_pk;

    assert(s_g.x == rhs.x);
    assert(s_g.y == rhs.y);
    assert(!s_g.is_infinite);
}
