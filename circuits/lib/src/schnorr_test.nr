// M3 checkpoint + security-remediation negatives: the circuit verifies a
// signature produced by the Rust harness (`cargo run -p harness -- sig-vectors`;
// sk=7, k=13, msg=42), and rejects tampered / malformed inputs.
use crate::schnorr::{assert_on_curve, is_even_y, verify_sig, Signature, GEN};
use std::embedded_curve_ops::{multi_scalar_mul, EmbeddedCurveScalar};

global PK_X: Field = 0x0e602b9dd6a3e8d039a17f069add3f9c2a187a8f629a1de60a33a8067b9b2842;
global PK_Y: Field = 0x14cc8e83df1b5cbb163bd2c94005cb0707fe570def5a165242b1c1419cb014cb;
global MSG: Field = 42;
global R_X: Field = 0x17e6386beac25fd11573fe37404e61d754a91ee7722dbd0c1c7fff157ed56573;
global R_Y: Field = 0x2be1ab9ccce96a8deeb82e85a35cc26804269db7ea57bc0e0afa3b835dfcff73;
global S_LO: Field = 0x000000000000000000000000000000008312af54a4f4b07d782dc23d06ef43f2;
global S_HI: Field = 0x000000000000000000000000000000002ed87ed8cf30832f5f7b93ee4d5afd52;

fn harness_sig() -> Signature {
    Signature { r_x: R_X, r_y: R_Y, s_lo: S_LO, s_hi: S_HI }
}

#[test]
fn generator_matches_ark_pk() {
    // pk = 7 * G, cross-checking the pinned GEN constant against ark-grumpkin.
    let pk = multi_scalar_mul([GEN], [EmbeddedCurveScalar::from_field(7)]);
    assert(pk.x == PK_X);
    assert(pk.y == PK_Y);
}

#[test]
fn generator_on_curve() {
    assert_on_curve(GEN.x, GEN.y);
}

#[test]
fn harness_points_on_curve() {
    assert_on_curve(PK_X, PK_Y);
    assert_on_curve(R_X, R_Y);
}

#[test]
fn harness_signature_verifies() {
    verify_sig(PK_X, PK_Y, MSG, harness_sig());
}

#[test(should_fail)]
fn wrong_message_fails() {
    verify_sig(PK_X, PK_Y, 43, harness_sig());
}

#[test(should_fail)]
fn tampered_s_fails() {
    let mut sig = harness_sig();
    sig.s_lo += 1;
    verify_sig(PK_X, PK_Y, MSG, sig);
}

#[test(should_fail)]
fn wrong_signer_fails() {
    // pk = 8 * G is a valid point but not the signer.
    let pk = multi_scalar_mul([GEN], [EmbeddedCurveScalar::from_field(8)]);
    verify_sig(pk.x, pk.y, MSG, harness_sig());
}

#[test(should_fail)]
fn zero_s_fails() {
    let sig = Signature { r_x: R_X, r_y: R_Y, s_lo: 0, s_hi: 0 };
    verify_sig(PK_X, PK_Y, MSG, sig);
}

#[test(should_fail)]
fn s_lo_not_u128_fails() {
    // 2^128 does not fit in a 128-bit limb range check.
    let two_128: Field = 0x0100000000000000000000000000000000;
    let sig = Signature { r_x: R_X, r_y: R_Y, s_lo: two_128, s_hi: S_HI };
    verify_sig(PK_X, PK_Y, MSG, sig);
}

#[test(should_fail)]
fn s_hi_not_u128_fails() {
    let two_128: Field = 0x0100000000000000000000000000000000;
    let sig = Signature { r_x: R_X, r_y: R_Y, s_lo: S_LO, s_hi: two_128 };
    verify_sig(PK_X, PK_Y, MSG, sig);
}

#[test(should_fail)]
fn off_curve_pk_fails() {
    // (1, 1) is not on Grumpkin (y^2 = x^3 - 17).
    verify_sig(1, 1, MSG, harness_sig());
}

#[test(should_fail)]
fn off_curve_r_fails() {
    let mut sig = harness_sig();
    sig.r_y = 1;
    verify_sig(PK_X, PK_Y, MSG, sig);
}

#[test]
fn even_y_detects_parity() {
    // 0 is even; 1 is odd.
    assert(is_even_y(0));
    assert(!is_even_y(1));
}
