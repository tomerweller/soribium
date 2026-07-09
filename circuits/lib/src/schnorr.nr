// Schnorr over Grumpkin with a Poseidon2 challenge (DESIGN.md). Hand-rolled:
// std::schnorr no longer exists and noir-lang/schnorr's Blake2s challenge
// would dominate the batch circuit's gate count.
//
// verify: s*G == R + e*pk, with e = Poseidon2([DOMAIN_SIG, R.x, pk_x, msg], 4).
// `s` lives in Grumpkin's scalar field (= BN254 base field, LARGER than the
// circuit Field), so it crosses as 128-bit limbs {s_lo, s_hi}. `e` is a
// circuit Field; from_field is reduction-free since Fr < Grumpkin scalar order.
//
// Security constraints (beyond the verify equation):
// - s_lo, s_hi are range-checked to 128 bits (Barretenberg MSM requires this;
//   unconstrained limbs are a soundness hazard).
// - s != 0 (rejects the zero scalar / point-at-infinity edge).
// - pk and R are on-curve (Grumpkin: y^2 = x^3 - 17) and not the point at
//   infinity. Grumpkin has cofactor 1, so on-curve => prime-order subgroup.
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

// Grumpkin curve parameter b: y^2 = x^3 + ax + b with a = 0, b = -17.
global CURVE_B: Field = 17;

pub struct Signature {
    pub r_x: Field,
    pub r_y: Field,
    pub s_lo: Field,
    pub s_hi: Field,
}

fn assert_u128(v: Field) {
    let _: [u1; 128] = v.to_le_bits();
}

/// Grumpkin on-curve + non-infinity. Identity is the point at infinity (not an
/// affine (x,y)); (0,0) is off-curve and is rejected by the equation check.
pub fn assert_on_curve(x: Field, y: Field) {
    assert(y * y == x * x * x - CURVE_B);
}

/// True iff the integer representative of `y` is even (LSB clear). Used to
/// bind y-parity for active spend keys so pk_x has a single canonical pubkey.
pub fn is_even_y(y: Field) -> bool {
    let bits: [u1; 254] = y.to_le_bits();
    bits[0] == 0
}

pub fn verify_sig(pk_x: Field, pk_y: Field, msg: Field, sig: Signature) {
    // --- limb / point well-formedness (must run before MSM) ---
    assert_u128(sig.s_lo);
    assert_u128(sig.s_hi);
    // s != 0: zero scalar maps G to infinity and is never a valid Schnorr s.
    if sig.s_lo == 0 {
        assert(sig.s_hi != 0);
    }
    assert_on_curve(pk_x, pk_y);
    assert_on_curve(sig.r_x, sig.r_y);

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
