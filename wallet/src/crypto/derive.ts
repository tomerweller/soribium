// Derive the L2 spending key from a wallet signature (Nethermind
// stellar-private-payments pattern). The user's Stellar account (via
// Freighter) is the only root of trust: signing a fixed, domain-separated
// message yields — deterministically, because Ed25519 is deterministic — the
// bytes we hash into a Grumpkin scalar. So the L2 key needs no separate
// backup (reconnect the wallet, re-sign, re-derive) and is provably bound to
// the signing Stellar address.
import { N_GRUMPKIN } from './fields';
import { canonicalizeSk } from './grumpkin';

/** The message the wallet signs. Human-readable (Freighter shows it) and
 *  versioned so we can rotate the derivation without ambiguity. */
export const KEY_DERIVATION_MESSAGE =
  'Soribium account key derivation (v1)\n\nSign to create or restore your rollup account. This does not move funds.';

const DOMAIN = 'soribium/spend-key/v1';

/**
 * sk = SHA-256(DOMAIN || signature) reduced into Grumpkin's scalar field.
 * Grumpkin's scalar order is BN254's base field, so this is the same
 * hash-and-reduce move the note-key derivation uses, just a different modulus.
 */
export async function deriveSkFromSignature(sig: Uint8Array): Promise<bigint> {
  const domain = new TextEncoder().encode(DOMAIN);
  const input = new Uint8Array(domain.length + sig.length);
  input.set(domain, 0);
  input.set(sig, domain.length);
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', input));
  let v = 0n;
  for (const b of digest) v = (v << 8n) | BigInt(b);
  const sk = v % N_GRUMPKIN;
  // Even-y canonical form required by the batch circuit for active spends.
  return canonicalizeSk(sk === 0n ? 1n : sk);
}
