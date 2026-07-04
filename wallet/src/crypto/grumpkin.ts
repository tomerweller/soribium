// Grumpkin: y^2 = x^3 - 17 over the BN254 scalar field, cofactor 1.
// Generator pinned identically to circuits/lib/src/schnorr.nr GEN and
// ark-grumpkin's Affine::generator().
import { weierstrass } from '@noble/curves/abstract/weierstrass.js';
import { N_GRUMPKIN, P_BN254_FR } from './fields';

export const Grumpkin = weierstrass({
  p: P_BN254_FR,
  n: N_GRUMPKIN,
  h: 1n,
  a: 0n,
  b: P_BN254_FR - 17n,
  Gx: 1n,
  Gy: 0x0000000000000002cf135e7506a45d632d270d45f1181294833fc48d823f272cn,
});

export type AffinePoint = { x: bigint; y: bigint };

export function mulBase(k: bigint): AffinePoint {
  const p = Grumpkin.BASE.multiply(k).toAffine();
  return { x: p.x, y: p.y };
}

export function pkFromSk(sk: bigint): AffinePoint {
  return mulBase(sk);
}

/** Validates the point is on the curve; throws otherwise. */
export function pointFromAffine(x: bigint, y: bigint) {
  const p = Grumpkin.fromAffine({ x, y });
  p.assertValidity();
  return p;
}
