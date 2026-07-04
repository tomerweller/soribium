// Single source of truth for field encodings. Grumpkin and BN254 form a
// 2-cycle: the circuit Field (BN254 scalar field) is Grumpkin's COORDINATE
// field, while Grumpkin's SCALAR field is the (larger) BN254 base field.
export type Fr = bigint;

/** BN254 scalar field modulus r = Grumpkin base field (circuit Field). */
export const P_BN254_FR =
  0x30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001n;

/** Grumpkin scalar field order = BN254 base field modulus q. */
export const N_GRUMPKIN =
  0x30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47n;

/** Canonical 32-byte big-endian hex: '0x' + exactly 64 lowercase chars. */
export function frToHex32(v: Fr): string {
  if (v < 0n || v >= P_BN254_FR) {
    throw new Error(`frToHex32: value out of field range: ${v}`);
  }
  return '0x' + v.toString(16).padStart(64, '0');
}

/** Strict parse: requires '0x' + exactly 64 hex chars. */
export function hexToFr(s: string): Fr {
  if (!/^0x[0-9a-fA-F]{64}$/.test(s)) {
    throw new Error(`hexToFr: bad encoding: ${s}`);
  }
  return BigInt(s);
}

/**
 * Uniform-enough scalar in Grumpkin's scalar field: 40 random bytes reduced
 * mod n gives bias < 2^-64. Used for secret keys and signature nonces.
 */
export function randScalar(): bigint {
  const bytes = new Uint8Array(40);
  crypto.getRandomValues(bytes);
  let v = 0n;
  for (const b of bytes) {
    v = (v << 8n) | BigInt(b);
  }
  const scalar = v % N_GRUMPKIN;
  return scalar === 0n ? randScalar() : scalar;
}
