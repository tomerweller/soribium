# Payments ZK-Rollup Spike — Design Spec

Single source of truth for hash layouts, domains, and message formats shared by
the Noir circuits, the Soroban contract, and the Rust harness. If a constant
here changes, all three must change together (CI-less spike: grep for the
domain name).

## Toolchain (pinned)

| Tool | Version |
|---|---|
| nargo | 1.0.0-beta.11 |
| bb (Barretenberg) | 0.87.0 |
| soroban-sdk / soroban-poseidon | 26.0.0 |
| ultrahonk-soroban-verifier | NethermindEth/rs-soroban-ultrahonk @ `661db07` |
| rust | 1.88.0 (wasm32v1-none) |
| Proof flavor | UltraHonk, BN254, **Keccak transcript** (`--oracle_hash keccak` on prove AND write_vk), non-ZK, non-recursive |

Verifier invariants: proof = 456 fields = 14,592 bytes; VK = 1760 packed bytes;
public inputs = 32-byte BE canonical Fr words, count = `vk.public_inputs_size − 16`.

## Hashing

All hashes are Poseidon2 over BN254 Fr (noir-lang/poseidon v0.2.0 in-circuit ≡
`soroban_poseidon::poseidon2_hash::<4, BnScalar>` on-chain ≡ harness via `Env`).

Domain separators (Fr constants):

| Domain | Value | Use |
|---|---|---|
| `DOMAIN_LEAF` | 1 | account leaf hash |
| `DOMAIN_TX` | 2 | L2 transaction signing message |
| `DOMAIN_SIG` | 3 | Schnorr challenge |
| `DOMAIN_DEP` | 4 | deposit-list fold |
| `DOMAIN_WD` | 5 | withdrawal-list fold |
| `DOMAIN_ADDR` | 6 | address_to_field |

## State tree

- Fixed depth **8** (256 accounts), parameterized in circuits and harness.
- Leaf = `Poseidon2([DOMAIN_LEAF, pk_x, balance, nonce])`.
- Node = `Poseidon2([left, right])`.
- Empty: `zero[0] = 0`, `zero[i+1] = Poseidon2([zero[i], zero[i]])`; empty leaf = 0.
- Account key: Grumpkin public-key x-coordinate (`pk_x`). **Spike caveat**: the
  y-parity is not bound, so a pk_x has two valid pubkeys; production must bind
  the sign bit.
- Balance range `[0, 2^64)` enforced in-circuit; `i128` on-chain.

## L2 transaction

Signing message: `msg = Poseidon2([DOMAIN_TX, from_pk_x, to_field, amount, nonce, is_withdraw])`
where `to_field` = recipient `pk_x` (transfer) or `address_to_field(dest)` (withdrawal).

Schnorr over Grumpkin (hand-rolled; std::schnorr no longer exists):
- keys: `pk = sk·G` (G = Grumpkin generator via `fixed_base_scalar_mul`)
- sign: nonce `k`, `R = k·G`, `e = Poseidon2([DOMAIN_SIG, R.x, pk_x, msg])`,
  `s = k + e·sk (mod Fq_grumpkin)`
- verify (circuit): `multi_scalar_mul([G, pk], [s, -e]) == R` — exact
  formulation settled in M3; `e` lifted via `EmbeddedCurveScalar::from_field`
  (safe: BN254 Fr < Grumpkin scalar modulus), `s` passed as `{s_lo, s_hi}`
  128-bit limbs.

## Batch circuit public interface

```
main(old_root: pub Field, new_root: pub Field,
     deposit_hash: pub Field, withdraw_hash: pub Field,
     deposits: [Deposit; D], txs: [Tx; N])
```

Exactly 4 public inputs (128-byte PI blob), all recomputed/held on-chain:

- `old_root` — contract storage.
- `new_root` — envelope, becomes storage after verification.
- `deposit_hash` — fold over the batch's FIFO deposit-queue prefix:
  `acc' = Poseidon2([DOMAIN_DEP, acc, pk_x, amount])`, `acc₀ = 0`; the envelope
  pins `deposit_count` and the contract recomputes over exactly that prefix
  (queue-race prevention).
- `withdraw_hash` — same fold shape with `DOMAIN_WD` over
  `(address_to_field(dest), amount)` pairs from the envelope.

`address_to_field` = Poseidon2 over the 56-byte strkey split into two 28-byte
limbs with `DOMAIN_ADDR` (ported from OZ confidential storage.rs).

Padding: `is_active = 0` entries freeze both the running root and the fold
accumulators.

## Envelope (submit_batch argument, XDR)

`{ new_root: BytesN<32>, deposit_count: u32, withdrawals: Vec<(Address, i128)>, txs_blob: Bytes, proof: Bytes }`

- `txs_blob` is carried for DA measurement but **not cryptographically bound**
  in the spike (production: hash it in-circuit, ≈1 Poseidon2 absorb/tx).
- Withdrawals executed inline, capped ≤ 8/batch.

## Known spike caveats (production deltas)

Tracked for REPORT.md: forced exits / censorship resistance; txs_blob
commitment; pk_x y-parity; VK rotation/upgrade path; sequencer
decentralization; SAC clawback/auth-flag vetting for the custody asset;
cross-instance proof replay (no `addr_f` binding — old_root match makes replay
a non-issue within an instance).
