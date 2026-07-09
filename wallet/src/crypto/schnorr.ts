// Schnorr over Grumpkin with a Poseidon2 challenge — mirrors
// harness/src/keys.rs byte-for-byte (the vector gate enforces it).
//
//   R = k*G;  e = Poseidon2([DOMAIN_SIG, R.x, pk.x, msg]);
//   s = (k + e*sk) mod n;  wire form: (r_x, r_y, s_lo, s_hi) 128-bit limbs.
import { Fr, N_GRUMPKIN, randScalar } from './fields';
import { Grumpkin, mulBase, pkFromSk, pointFromAffine } from './grumpkin';
import { DOMAIN_DA, DOMAIN_SIG, DOMAIN_TX, p2 } from './poseidon2';

export interface Signature {
  r_x: Fr;
  r_y: Fr;
  s_lo: bigint;
  s_hi: bigint;
}

const LIMB_MASK = (1n << 128n) - 1n;

/** The signed transaction message (arity 6, DESIGN.md). */
export function txMessage(
  fromPkX: Fr,
  toField: Fr,
  amount: bigint,
  nonce: bigint,
  isWithdraw: boolean,
): Fr {
  return p2([DOMAIN_TX, fromPkX, toField, amount, nonce, isWithdraw ? 1n : 0n]);
}

/** DA-commitment fold step (DOMAIN_DA, arity 3). */
export function daFold(acc: Fr, msg: Fr): Fr {
  return p2([DOMAIN_DA, acc, msg]);
}

export function sign(sk: bigint, msg: Fr, k: bigint = randScalar()): Signature {
  const pk = pkFromSk(sk);
  const R = mulBase(k);
  const e = p2([DOMAIN_SIG, R.x, pk.x, msg]);
  // e < P_BN254_FR < N_GRUMPKIN: the lift into the scalar field is
  // reduction-free (same argument as keys.rs).
  const s = (k + e * sk) % N_GRUMPKIN;
  return { r_x: R.x, r_y: R.y, s_lo: s & LIMB_MASK, s_hi: s >> 128n };
}

/**
 * Verify with the same equation the circuit enforces: s*G == R + e*pk.
 * Returns false (never throws) on malformed limbs or off-curve points —
 * mirrors keys.rs from_limbs/pk_from_coords rejection behavior.
 */
export function verify(pkX: Fr, pkY: Fr, msg: Fr, sig: Signature): boolean {
  if (sig.s_lo < 0n || sig.s_lo > LIMB_MASK || sig.s_hi < 0n || sig.s_hi > LIMB_MASK) {
    return false;
  }
  const s = (sig.s_lo + (sig.s_hi << 128n)) % N_GRUMPKIN;
  if (s === 0n) {
    return false;
  }
  // Even-y is enforced for active spends in the batch circuit (apply_tx), not
  // in bare verify_sig — padding uses a published odd-y key by design.
  try {
    const pk = pointFromAffine(pkX, pkY);
    const R = pointFromAffine(sig.r_x, sig.r_y);
    const e = p2([DOMAIN_SIG, sig.r_x, pkX, msg]);
    if (e === 0n) {
      return false;
    }
    const lhs = Grumpkin.BASE.multiply(s);
    const rhs = R.add(pk.multiply(e));
    return lhs.equals(rhs);
  } catch {
    return false;
  }
}
