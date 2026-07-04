//! Strict wire encoding for field elements: "0x" + exactly 64 lowercase hex
//! chars, big-endian, canonical (< BN254 r). Everything crossing the HTTP
//! boundary parses through here — the Soroban host silently reduces
//! non-canonical encodings mod r, so accepting one would make two byte
//! strings denote the same value.

use harness::poseidon::Fr;

/// BN254 scalar field modulus r, big-endian (mirrors contracts publics.rs).
pub const BN254_R: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
    0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00,
    0x00, 0x01,
];

pub fn is_canonical(word: &Fr) -> bool {
    *word < BN254_R
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexError {
    BadFormat,
    NonCanonical,
}

/// Parse a strict 0x-prefixed 64-hex-char canonical field element.
pub fn parse_fr(s: &str) -> Result<Fr, HexError> {
    let body = s.strip_prefix("0x").ok_or(HexError::BadFormat)?;
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HexError::BadFormat);
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(body.to_ascii_lowercase(), &mut out).map_err(|_| HexError::BadFormat)?;
    if !is_canonical(&out) {
        return Err(HexError::NonCanonical);
    }
    Ok(out)
}

pub fn fr_hex(fr: &Fr) -> String {
    format!("0x{}", hex::encode(fr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictness() {
        let ok = "0x0000000000000000000000000000000000000000000000000000000000000001";
        assert!(parse_fr(ok).is_ok());
        // Missing prefix, wrong length, non-hex.
        assert_eq!(parse_fr(&ok[2..]), Err(HexError::BadFormat));
        assert_eq!(parse_fr("0x01"), Err(HexError::BadFormat));
        assert_eq!(parse_fr(&format!("0x{}", "zz".repeat(32))), Err(HexError::BadFormat));
        // r itself is non-canonical; r-1 is fine.
        let r_hex = format!("0x{}", hex::encode(BN254_R));
        assert_eq!(parse_fr(&r_hex), Err(HexError::NonCanonical));
        let mut r_minus_1 = BN254_R;
        r_minus_1[31] = 0x00;
        assert!(parse_fr(&format!("0x{}", hex::encode(r_minus_1))).is_ok());
        // Roundtrip.
        let v = parse_fr(ok).unwrap();
        assert_eq!(parse_fr(&fr_hex(&v)).unwrap(), v);
    }
}
