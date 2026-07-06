import { describe, expect, it } from 'vitest';
import { leafValue } from '../crypto/merkle';
import { demoLeaves, demoLevels, demoPath, demoRoot, makeAccounts, rootFromPath } from './demo';

describe('explainer demo tree (real Poseidon2)', () => {
  it('root is stable and path recomputation matches', () => {
    const accounts = makeAccounts();
    const levels = demoLevels(demoLeaves(accounts));
    const root = levels[levels.length - 1][0];
    expect(root).toBe(demoRoot(accounts));

    const alice = accounts[0];
    const { siblings } = demoPath(levels, alice.index);
    const leaf = leafValue(alice.pkX, alice.balance, alice.nonce);
    expect(rootFromPath(leaf, alice.index, siblings)).toBe(root);
  });

  it('a tampered leaf yields a different root (the fake-balance attack)', () => {
    const accounts = makeAccounts();
    const levels = demoLevels(demoLeaves(accounts));
    const root = levels[levels.length - 1][0];
    const alice = accounts[0];
    const { siblings } = demoPath(levels, alice.index);
    const fat = leafValue(alice.pkX, 10_000_000_000n, alice.nonce);
    expect(rootFromPath(fat, alice.index, siblings)).not.toBe(root);
  });

  it('balance changes move the root; carol is off-rollup', () => {
    const a1 = makeAccounts();
    const a2 = makeAccounts();
    a2[0].balance += 1n;
    expect(demoRoot(a1)).not.toBe(demoRoot(a2));
    expect(a1.find((a) => a.name === 'carol')!.onRollup).toBe(false);
  });
});
