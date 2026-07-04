#![cfg(test)]

// Fixture-driven verification tests land in M1 (tests/verify_fixture.rs uses
// checked-in artifacts). Unit tests for publics/storage accumulate here.

use crate::publics::is_canonical_field;

#[test]
fn canonical_field_boundaries() {
    let zero = [0u8; 32];
    assert!(is_canonical_field(&zero));

    // r itself is non-canonical.
    let r = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81,
        0x58, 0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93,
        0xf0, 0x00, 0x00, 0x01,
    ];
    assert!(!is_canonical_field(&r));

    // r - 1 is canonical.
    let mut r_minus_1 = r;
    r_minus_1[31] = 0x00;
    assert!(is_canonical_field(&r_minus_1));

    let max = [0xffu8; 32];
    assert!(!is_canonical_field(&max));
}
