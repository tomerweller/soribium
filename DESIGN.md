# Soribium — Design Spec

Soribium is a payments ZK-rollup operating as a **validium** on Stellar
testnet: batch transaction data lives off-chain (served by the sequencer's DA
endpoint) and is bound on-chain by a proven commitment.

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
| `DOMAIN_DA` | 7 | DA-blob commitment fold (validium) |

## State tree

- Fixed depth **8** (256 accounts), parameterized in circuits and harness.
- Leaf = `Poseidon2([DOMAIN_LEAF, pk_x, balance, nonce])`.
- Node = `Poseidon2([left, right])`.
- Empty: `zero[0] = 0`, `zero[i+1] = Poseidon2([zero[i], zero[i]])`; empty leaf = 0.
- Account key: Grumpkin public-key x-coordinate (`pk_x`). Active spends require
  **even-y** public keys (`pk_y` LSB clear): keygen flips `sk → -sk` when the
  raw point has odd y (harness `Keypair::from_sk`, wallet `canonicalizeSk`).
  This binds y-parity for spend authorization without enlarging the leaf.
- Balance range `[0, 2^64)` enforced in-circuit; `i128` on-chain. Overflow of
  an L2 balance (an unprovable FIFO queue head) is prevented by a **deployment
  invariant** rather than per-key tracking: the custody token's total supply
  must be ≤ `u64::MAX` base units (native XLM: ~1.05e18 stroops, ~17× under).
  The circuit conserves value, so every balance ≤ escrow ≤ supply.

## L2 transaction

Signing message: `msg = Poseidon2([DOMAIN_TX, from_pk_x, to_field, amount, nonce, is_withdraw])`
where `to_field` = recipient `pk_x` (transfer) or `address_to_field(dest)` (withdrawal).

Schnorr over Grumpkin (hand-rolled; std::schnorr no longer exists):
- keys: `pk = sk·G` (G = Grumpkin generator via `fixed_base_scalar_mul`), even-y
- sign: nonce `k`, `R = k·G`, `e = Poseidon2([DOMAIN_SIG, R.x, pk_x, msg])`,
  `s = k + e·sk (mod Fq_grumpkin)`
- verify (circuit): `s·G == R + e·pk`, with defenses:
  - `s_lo`, `s_hi` range-checked to 128 bits; `s ≠ 0`
  - `pk` and `R` on-curve (`y^2 = x^3 - 17`) and non-infinity
  - `e` lifted via `EmbeddedCurveScalar::from_field` (safe: BN254 Fr < Grumpkin
    scalar modulus)
- **Padding keypair** (`sk=7`, published `PAD_PK_*`): used only for inactive
  batch slots. Active deposits/transfers **blacklist** `PAD_PK_X` (secret is
  public — crediting it would make funds drainable by anyone).

## Batch circuit public interface

```
main(old_root: pub Field, new_root: pub Field,
     deposit_hash: pub Field, withdraw_hash: pub Field,
     da_commitment: pub Field,
     deposits: [Deposit; D], txs: [Tx; N])
```

Exactly 5 public inputs (160-byte PI blob):

- `old_root` — contract storage.
- `new_root` — envelope, becomes storage after verification.
- `deposit_hash` — fold over the batch's FIFO deposit-queue prefix:
  `acc' = Poseidon2([DOMAIN_DEP, acc, pk_x, amount])`, `acc₀ = 0`; the envelope
  pins `deposit_count` and the contract recomputes over exactly that prefix
  (queue-race prevention).
- `withdraw_hash` — same fold shape with `DOMAIN_WD` over
  `(address_to_field(dest), amount)` pairs from the envelope.
- `da_commitment` — fold over each **active** tx's signing message, in order:
  `acc' = Poseidon2([DOMAIN_DA, acc, tx_msg])` (3-input), `acc₀ = 0`. The
  message already binds `(from_pk_x, to_field, amount, nonce, is_withdraw)` —
  everything needed to reconstruct state from the published blob. Taken from
  the envelope (prover-supplied) and bound by the proof; verifiers fetch the
  blob from the sequencer's `GET /da/:batch_num` and re-fold. Signatures ship
  in the blob as audit data but are NOT commitment-bound (authorization is
  already established by the proof itself).

`address_to_field` = Poseidon2 over the 56-byte strkey split into two 28-byte
limbs with `DOMAIN_ADDR` (ported from OZ confidential storage.rs).

Padding: `is_active = 0` entries freeze both the running root and the fold
accumulators. Active entries require `amount > 0`.

## Envelope (submit_batch argument)

`{ new_root: BytesN<32>, deposit_count: u32, withdrawals: Vec<Withdrawal>, da_commitment: BytesN<32>, proof: Bytes }`

- The tx blob itself never touches the chain (validium): it is stored in the
  sequencer's SQLite and served at `GET /da/:batch_num`, bound by
  `da_commitment` (see above).
- Withdrawals executed inline, capped ≤ 8/batch.

## Batching cadence

The sequencer batches **eagerly**: on each tick (`TICK_SECS`), if more than
one payment is pending it builds+proves+submits immediately; deposit-queue-
full and the `BATCH_MAX_WAIT_SECS` timer remain as fallbacks so lone payments
and deposit-only activity still settle. **Production requirement:** the
pipeline must sustain Stellar's ~5s ledger cadence — prover hardware is
provisioned such that bb prove(CIRCUIT_PKG) ≤ ~3.5s (see docs/PROVING.md
§3.5), with `TICK_SECS=2` / `BATCH_MAX_WAIT_SECS=5` in deployment config so
every ledger can carry a batch.

## Validium trust model

Validity is trustless (every root advance is proven). Data availability is
trusted to the sequencer operator: if the operator withholds a blob, users
cannot recompute Merkle paths for newer roots and the system freezes (funds
cannot be stolen). Production hardening path: DAC signatures over
`da_commitment` verified in `submit_batch`.

## Known spike caveats (production deltas)

Tracked for REPORT.md: forced exits / censorship resistance; DA committee
over `da_commitment`; circuit-level `pk_x` uniqueness (honest builder +
harness enforce find-first; a malicious prover could still open a second slot
with the same `pk_x` without a sparse/nullifier tree); VK rotation/upgrade
path; sequencer decentralization; SAC clawback/auth-flag vetting for the
custody asset (including that it cannot mint past `u64::MAX` base units —
the balance-overflow safety argument depends on it); cross-instance proof
replay (no `addr_f` binding — old_root match makes replay a non-issue within
an instance).
