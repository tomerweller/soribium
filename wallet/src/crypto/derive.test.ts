import { describe, expect, it } from 'vitest';
import { frToHex32 } from './fields';
import { deriveSkFromSignature } from './derive';
import { N_GRUMPKIN } from './fields';
import { pkFromSk } from './grumpkin';

// A fixed 64-byte "signature" stand-in (Ed25519 sig length).
const SIG = Uint8Array.from({ length: 64 }, (_, i) => (i * 7 + 3) & 0xff);

describe('Freighter-signature key derivation', () => {
  it('matches the pinned v1 golden vector (DO NOT casually update)', async () => {
    // sk = canonicalize(SHA-256("soribium/spend-key/v1" || sig) mod N_GRUMPKIN).
    // If this test fails, the derivation formula changed: every existing user's
    // L2 key — and therefore their funds — silently changes with it. Changing
    // the formula requires a v2 domain + migration, never an update to this pin.
    const sk = await deriveSkFromSignature(SIG);
    expect(frToHex32(sk)).toBe('0x099eaf828e3d5232c2fc2ed5f2117b052c926d80a3ba4716933adf0c7c27e720');
    expect(frToHex32(pkFromSk(sk).x)).toBe('0x2b0b924f522b0b6e583ea76d68d3b5a4c7347f39821ebea1c1249ea87e547f2f');
  });

  it('is deterministic for the same signature', async () => {
    const a = await deriveSkFromSignature(SIG);
    const b = await deriveSkFromSignature(SIG);
    expect(a).toBe(b);
  });

  it('produces a valid Grumpkin scalar and public key', async () => {
    const sk = await deriveSkFromSignature(SIG);
    expect(sk).toBeGreaterThan(0n);
    expect(sk).toBeLessThan(N_GRUMPKIN);
    // pk derivation must succeed and be even-y canonical.
    const pk = pkFromSk(sk);
    expect(pk.x).toBeGreaterThan(0n);
    expect(pk.y & 1n).toBe(0n);
  });

  it('differs when the signature differs', async () => {
    const other = Uint8Array.from(SIG);
    other[0] ^= 0x01;
    expect(await deriveSkFromSignature(SIG)).not.toBe(await deriveSkFromSignature(other));
  });
});
