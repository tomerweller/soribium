// L2 key custody. Spike-grade: the secret lives in localStorage (documented
// XSS caveat). A key is a Grumpkin scalar; the account id is pk_x.
import { frToHex32, hexToFr, randScalar } from '../crypto/fields';
import { pkFromSk } from '../crypto/grumpkin';

const STORAGE_KEY = 'soribium.v1.sk';

export interface Wallet {
  sk: bigint;
  pkX: string; // 0x hex
  pkY: string; // 0x hex
}

function fromSk(sk: bigint): Wallet {
  const pk = pkFromSk(sk);
  return { sk, pkX: frToHex32(pk.x), pkY: frToHex32(pk.y) };
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

export function generate(): Wallet {
  const sk = randScalar();
  localStorage.setItem(STORAGE_KEY, frToHex32(sk));
  return fromSk(sk);
}

export function importSk(hex: string): Wallet {
  const sk = hexToFr(hex);
  if (sk === 0n) throw new Error('secret key must be nonzero');
  const w = fromSk(sk);
  localStorage.setItem(STORAGE_KEY, frToHex32(sk));
  return w;
}

/** The raw secret hex, for export (guard behind a confirm in the UI). */
export function exportSk(): string | null {
  return localStorage.getItem(STORAGE_KEY);
}

export function clear(): void {
  localStorage.removeItem(STORAGE_KEY);
}
