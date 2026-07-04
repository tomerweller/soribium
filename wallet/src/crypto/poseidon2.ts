// Poseidon2 over BN254 with Barretenberg's exact parameters.
// @zkpassport/poseidon2 carries the same constants as bb / noir-lang/poseidon
// / soroban-poseidon (equivalence proven transitively by this repo's pinned
// vectors — see vectors.test.ts, which gates every release).
import { poseidon2Hash } from '@zkpassport/poseidon2';

export const p2 = (inputs: bigint[]): bigint => poseidon2Hash(inputs);

// Domain separators (DESIGN.md).
export const DOMAIN_LEAF = 1n;
export const DOMAIN_TX = 2n;
export const DOMAIN_SIG = 3n;
export const DOMAIN_DEP = 4n;
export const DOMAIN_WD = 5n;
export const DOMAIN_ADDR = 6n;
export const DOMAIN_DA = 7n;
