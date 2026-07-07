// L2 key custody. Spike-grade: the secret lives in localStorage (documented
// XSS caveat). A key is a Grumpkin scalar; the account id is pk_x.
import { frToHex32, hexToFr, randScalar } from '../crypto/fields';
import { pkFromSk } from '../crypto/grumpkin';

const STORAGE_KEY = 'soribium.v1.sk';
const LINK_KEY = 'soribium.v1.linkedAddress';

export interface Wallet {
  sk: bigint;
  pkX: string; // 0x hex
  pkY: string; // 0x hex
  /** Stellar address this key was derived from (if via Freighter). */
  linkedAddress?: string;
}

function fromSk(sk: bigint): Wallet {
  const pk = pkFromSk(sk);
  const linked = localStorage.getItem(LINK_KEY) ?? undefined;
  return { sk, pkX: frToHex32(pk.x), pkY: frToHex32(pk.y), linkedAddress: linked };
}

export function load(): Wallet | null {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    return fromSk(hexToFr(raw));
  } catch {
    return null;
  }
}

/** Persist a Freighter-derived key plus the Stellar address it's bound to. */
export function saveDerived(sk: bigint, linkedAddress: string): Wallet {
  localStorage.setItem(STORAGE_KEY, frToHex32(sk));
  localStorage.setItem(LINK_KEY, linkedAddress);
  return fromSk(sk);
}

export function generate(): Wallet {
  const sk = randScalar();
  localStorage.setItem(STORAGE_KEY, frToHex32(sk));
  localStorage.removeItem(LINK_KEY); // random/imported keys aren't wallet-bound
  return fromSk(sk);
}

export function importSk(hex: string): Wallet {
  let sk: bigint;
  try {
    sk = hexToFr(hex);
  } catch {
    throw new Error('Not a valid secret key — expected 0x followed by exactly 64 hex characters.');
  }
  if (sk === 0n) throw new Error('Secret key must be nonzero.');
  localStorage.setItem(STORAGE_KEY, frToHex32(sk));
  localStorage.removeItem(LINK_KEY);
  return fromSk(sk);
}

/** The raw secret hex, for export (guard behind a confirm in the UI). */
export function exportSk(): string | null {
  return localStorage.getItem(STORAGE_KEY);
}

export function clear(): void {
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(LINK_KEY);
}
