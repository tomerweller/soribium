// Demo state for the explainer: a depth-4 (16-slot) account tree computed
// with the REAL Poseidon2 — every hash shown on the page is a genuine value,
// just over a smaller tree than production's depth-8 (for legibility).
// Depth is generic here (crypto/merkle.ts pins production's DEPTH=8).
import { Fr } from '../crypto/fields';
import { pkFromSk } from '../crypto/grumpkin';
import { leafValue } from '../crypto/merkle';
import { p2 } from '../crypto/poseidon2';

export const DEMO_DEPTH = 4;
export const DEMO_SLOTS = 1 << DEMO_DEPTH;

export interface DemoAccount {
  name: string;
  sk: bigint;
  pkX: Fr;
  pkY: Fr;
  balance: bigint; // stroops
  nonce: bigint;
  index: number;
  onRollup: boolean; // false = never deposited (RECIPIENT_UNKNOWN teaching)
}

export function makeAccounts(): DemoAccount[] {
  const mk = (name: string, sk: bigint, balance: bigint, index: number, onRollup = true): DemoAccount => {
    const pk = pkFromSk(sk);
    return { name, sk, pkX: pk.x, pkY: pk.y, balance, nonce: 0n, index, onRollup };
  };
  return [
    mk('alice', 101n, 30_000_000n, 0), // 3 XLM
    mk('bob', 202n, 20_000_000n, 1), // 2 XLM
    mk('carol', 303n, 0n, 2, false), // never deposited
  ];
}

/** Leaves array (Fr leaf values) for the demo tree. */
export function demoLeaves(accounts: DemoAccount[]): Fr[] {
  const leaves: Fr[] = Array.from({ length: DEMO_SLOTS }, () => 0n);
  for (const a of accounts) {
    if (a.onRollup) leaves[a.index] = leafValue(a.pkX, a.balance, a.nonce);
  }
  return leaves;
}

/** All internal levels bottom-up: levels[0] = leaves, levels[DEPTH] = [root]. */
export function demoLevels(leaves: Fr[]): Fr[][] {
  const levels: Fr[][] = [leaves];
  let cur = leaves;
  while (cur.length > 1) {
    const next: Fr[] = [];
    for (let i = 0; i < cur.length; i += 2) next.push(p2([cur[i], cur[i + 1]]));
    levels.push(next);
    cur = next;
  }
  return levels;
}

export function demoRoot(accounts: DemoAccount[]): Fr {
  const levels = demoLevels(demoLeaves(accounts));
  return levels[levels.length - 1][0];
}

/** Sibling hashes along `index`'s path (bottom-up), for path illumination. */
export function demoPath(levels: Fr[][], index: number): { siblings: Fr[]; nodes: Fr[] } {
  const siblings: Fr[] = [];
  const nodes: Fr[] = [levels[0][index]];
  let idx = index;
  for (let d = 0; d < DEMO_DEPTH; d++) {
    siblings.push(levels[d][idx ^ 1]);
    idx >>= 1;
    nodes.push(levels[d + 1][idx]);
  }
  return { siblings, nodes };
}

/** Recompute a root from a (possibly tampered) leaf + honest siblings —
 * the check the sandbox's "fake balance" attack fails. */
export function rootFromPath(leaf: Fr, index: number, siblings: Fr[]): Fr {
  let cur = leaf;
  let idx = index;
  for (let d = 0; d < DEMO_DEPTH; d++) {
    cur = idx % 2 === 0 ? p2([cur, siblings[d]]) : p2([siblings[d], cur]);
    idx >>= 1;
  }
  return cur;
}
