// Depth-8 Poseidon2 account tree, mirroring circuits/lib/src/{merkle,account}.nr
// and harness/src/tree.rs. Path bits are the leaf index LSB-first; bit = 0
// means the running node is the LEFT child at that level.
import { Fr } from './fields';
import { DOMAIN_LEAF, p2 } from './poseidon2';

export const DEPTH = 8;

export function leafValue(pkX: Fr, balance: bigint, nonce: bigint): Fr {
  return pkX === 0n ? 0n : p2([DOMAIN_LEAF, pkX, balance, nonce]);
}

export function computeRoot(leaf: Fr, index: number, siblings: Fr[]): Fr {
  if (siblings.length !== DEPTH) {
    throw new Error(`computeRoot: expected ${DEPTH} siblings`);
  }
  if (index < 0 || index >= 1 << DEPTH) {
    throw new Error('computeRoot: index out of range');
  }
  let cur = leaf;
  for (let i = 0; i < DEPTH; i++) {
    const bit = (index >> i) & 1;
    cur = bit === 0 ? p2([cur, siblings[i]]) : p2([siblings[i], cur]);
  }
  return cur;
}

export function verifyPath(leaf: Fr, index: number, siblings: Fr[], root: Fr): boolean {
  try {
    return computeRoot(leaf, index, siblings) === root;
  } catch {
    return false;
  }
}
