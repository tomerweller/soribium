import { describe, expect, it } from 'vitest';
import { deriveSkFromSignature } from './derive';
import { N_GRUMPKIN } from './fields';
import { pkFromSk } from './grumpkin';

// A fixed 64-byte "signature" stand-in (Ed25519 sig length).
const SIG = Uint8Array.from({ length: 64 }, (_, i) => (i * 7 + 3) & 0xff);

describe('Freighter-signature key derivation', () => {
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
