# Feasibility Spike Report: Payments ZK-Rollup on Stellar (Noir + UltraHonk)

**Date:** 2026-07-03 · **Verdict: GO on all three questions.**

A minimal but honest payments rollup — SEP-41 custody, Poseidon2 account tree,
Grumpkin-Schnorr-signed L2 transfers, one UltraHonk proof per batch verified
on-chain via Protocol 25/26 BN254 host functions — runs end-to-end on a
Protocol 26 localnet and on **testnet** (Protocol 27):

- Batch on testnet: [`c018a0f5…`](https://stellar.expert/explorer/testnet/tx/c018a0f5786d0a7cf25356d8890197f368dfe532acc131800a4eccc40b1e9571)
  (2 deposits consumed, 1 L2 transfer, 1 L2 withdrawal paid out on L1).

## 1. Does a rollup-shaped circuit verify on-chain within budget? — YES, with huge headroom

| circuit | gates (log2) | ACIR opcodes | bb prove (M-series laptop, 12 threads) | peak RSS | on-chain verify (declared insns) | % of 400M cap |
|---|---|---|---|---|---|---|
| trivial (Poseidon2 preimage) | 2^12 | ~30 | <0.1 s | — | 78,720,984 | 19.7% |
| batch_n4 (D=2, N=4) | 2^16 | 2,250 | 0.30 s | 171 MB | 86,702,028 | 21.7% |
| batch_n16 (D=4, N=16) | 2^18 | 8,018 | 0.78 s | 747 MB | 90,511,011 | 22.6% |

Verification cost is **logarithmic in circuit size**: ~+2M instructions per
doubling. Extrapolated, even a 2^24-gate circuit (~1,000+ payments/batch)
verifies at ~103M instructions — **on-chain verification never becomes the
binding constraint** under the Protocol 26 400M cap.

Full `submit_batch` (verify + on-chain Poseidon2 fold recomputation + queue +
1 withdrawal transfer) for batch_n4: **100.5M declared instructions**, 2,600
write bytes, 7 footprint entries. Caveat: the Rust test env (native
execution) reports 67.4M for the same call — wasm metering is ~1.5× that;
always measure via simulateTransaction.

## 2. Does the single-SEP-41-token custody loop work? — YES

Deposits escrow via `token.transfer(from, contract)` + FIFO queue; the batch
proof must consume an exact queue prefix (`deposit_count` pins it — no
queue races); withdrawals are authorized solely by the proof (the contract
recomputes `withdraw_hash` from the envelope's `(dest, amount)` list via
`address_to_field`, so redirecting or reamounting a payout breaks
verification — covered by negative tests); root advances atomically.
Verified in unit tests (15 tests incl. real-proof positive + 8 adversarial
negatives) and live on localnet + testnet.

## 3. Economics — viable; the value proposition is throughput/features, not fee arbitrage

Measured on testnet (native-asset SAC):

| op | unsigned tx bytes | min resource fee | actually charged |
|---|---|---|---|
| deposit | 244 B | 0.1318 XLM | ~0.13 XLM (incl. persistent-entry rent) |
| submit_batch (n4) | 15,076 B (11.4% of the 132,096 B cap) | 0.1362 XLM | **0.1195 XLM** |

Per-batch cost is ~fixed (proof = 14,592 B of the tx; verification CPU nearly
flat), so **cost per payment ≈ 0.12 XLM / N**:

| batch size | est. fee/payment |
|---|---|
| 4 | ~0.030 XLM |
| 16 | ~0.0078 XLM |
| 256 | ~0.0005 XLM |
| ~4,000 (tx-size ceiling with ~28 B/payment DA blob) | ~0.00004 XLM |

**Honest framing:** Stellar L1 payments cost ~0.00001 XLM base fee. A rollup
on Stellar does not win on per-payment fees until batches reach thousands of
payments — the rollup's actual value is (a) throughput beyond ledger TPS
limits (~4,000 payments per submit_batch tx at the byte ceiling; the
per-ledger write budget admits ~2 such txs → order 1,000+ payments/sec), and
(b) as a substrate for features L1 can't do (privacy, custom execution).

**Which constraint binds:** neither CPU (never) nor per-tx bytes (up to
~4,000 payments). The practical scale limiter is the **prover** (memory grows
~linearly: est. several GB at 2^22 gates / ~256 payments — still
laptop-feasible) and, at high cadence, the **ledger-wide** Soroban byte/CPU
budgets shared with all other traffic.

## Toolchain verdict

nargo 1.0.0-beta.11 + bb 0.87.0 + NethermindEth/rs-soroban-ultrahonk @
`661db07` worked **without version bisection**. Notes:
- `--oracle_hash keccak` required on both `bb prove` and `bb write_vk`; the
  keccak-oracle VK is already the packed 1760-byte layout (the 1764→1760
  strip in OZ's pipeline applies only to the default oracle).
- Quickstart's `--limits testnet` preset still caps CPU at the pre-P26 100M
  and rejects submit_batch (100.5M); use `--limits unlimited` locally. Real
  testnet accepted everything.
- The verifier crate is pre-release and unaudited (pin the rev).

## Production deltas (out of spike scope, tracked)

1. **DA binding**: `txs_blob` is carried but not committed in-circuit
   (~1 Poseidon2 absorb/payment to add) — required for trustless state
   reconstruction.
2. **Forced exits / censorship resistance** (L1 escape hatch), sequencer
   permissioning/decentralization.
3. pk_x y-parity binding; deposit-overflow queue jam (a deposit pushing an L2
   balance past 2^64 can never be consumed → cap per-account deposits
   on-chain); VK rotation/upgrade path; instance-storage TTL management.
4. SAC clawback/auth-flag vetting for the custody asset.
5. Real sequencer service (mempool, persistence, recovery) — the spike
   harness is CLI-driven.
6. Third-party audit of the verifier crate before any real funds.
