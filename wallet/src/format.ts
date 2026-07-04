// Display/parse helpers. Internally every amount is a bigint of stroops
// (7 decimals = 1 XLM); XLM strings appear ONLY at the UI boundary.
import { P_BN254_FR, hexToFr } from './crypto/fields';

export const STROOPS_PER_XLM = 10_000_000n;

/** Format stroops as an XLM decimal string, trimming trailing zeros. */
export function stroopsToXlm(v: bigint): string {
  const neg = v < 0n;
  const abs = neg ? -v : v;
  const whole = abs / STROOPS_PER_XLM;
  const frac = abs % STROOPS_PER_XLM;
  let out = whole.toString();
  if (frac > 0n) {
    const fracStr = frac.toString().padStart(7, '0').replace(/0+$/, '');
    out += '.' + fracStr;
  }
  return neg ? '-' + out : out;
}

/**
 * Parse an XLM decimal string into stroops. Rejects blanks, signs, NaN, and
 * more than 7 fractional digits. Throws on invalid input.
 */
export function xlmToStroops(s: string): bigint {
  const t = s.trim();
  if (!/^\d+(\.\d{1,7})?$/.test(t)) {
    throw new Error('Enter a positive amount with at most 7 decimal places');
  }
  const [whole, frac = ''] = t.split('.');
  const fracPadded = frac.padEnd(7, '0');
  const v = BigInt(whole) * STROOPS_PER_XLM + BigInt(fracPadded);
  if (v <= 0n) {
    throw new Error('Amount must be greater than zero');
  }
  return v;
}

/** Abbreviate a long hex string for display: 0x1234…abcd. */
export function shortHex(hex: string, n = 6): string {
  if (hex.length <= 2 + n * 2) return hex;
  return `${hex.slice(0, 2 + n)}…${hex.slice(-n)}`;
}

/**
 * True when `s` is a canonical L2 account id: '0x' + 64 lowercase hex,
 * nonzero, and within the BN254 scalar field.
 */
export function isCanonicalPkX(s: string): boolean {
  if (!/^0x[0-9a-f]{64}$/.test(s)) return false;
  try {
    const v = hexToFr(s);
    return v !== 0n && v < P_BN254_FR;
  } catch {
    return false;
  }
}

/** Big-endian 32-byte encoding of a pk_x hex, for the deposit `bytes` ScVal. */
export function pkxToBytes32(pkXHex: string): Uint8Array {
  const hex = pkXHex.startsWith('0x') ? pkXHex.slice(2) : pkXHex;
  const padded = hex.padStart(64, '0');
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    out[i] = parseInt(padded.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}
