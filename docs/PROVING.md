# Proving throughput vs. Stellar block time

Empirical analysis of batch size → proving time, and whether recursion helps
maximize payments proven within one Stellar ledger (~5s).

**Reference machine:** Apple M4 Pro, 12 cores, 24 GB RAM. Toolchain: nargo
1.0.0-beta.11, bb 0.87.0 (native arm64), UltraHonk/keccak for on-chain proofs.
Reproduce with `scripts/bench_proving.sh` (direct ladder) and the
`circuits/recur1`, `circuits/recur2` benchmark circuits (recursion).
All numbers best-of-2 unless noted; measured 2026-07-06.

## 1. Direct proving: batch size ladder

One `batch_nN` circuit proves N payments (each ≈ 453 ACIR opcodes ≈ 16.4K
gates: two depth-8 Merkle updates + one Grumpkin Schnorr + folds).

| N txs | gates (padded) | witness gen | bb prove | e2e (wit+prove) | peak RSS | prove-only tx/s |
|---|---|---|---|---|---|---|
| 4 | 2^16 | 0.21 s | 0.30 s | 0.51 s | 0.16 GB | 13 |
| 16 | 2^18 | 0.23 s | 0.78 s | 1.01 s | 0.71 GB | 21 |
| 64 | 2^20 | 0.39 s | 2.44 s | 2.83 s | 2.7 GB | 26 |
| 128 | 2^21 | 0.59 s | 4.84 s | 5.43 s | ~4.9 GB | 26 |
| 256 | 2^22 | 1.00 s | 7.05 s | 8.05 s | ~5.3 GB | 36 |

Observations:

- **Proving scales ~linearly with gates** (~2.4s per 2^20 gates), with
  slightly *better* per-tx efficiency at larger sizes (fixed costs amortize;
  n256 is sublinear vs n128 likely due to padding slack in 2^21).
- **bb pads circuits to powers of two.** A hypothetical n96 costs the same as
  n128 (both pad to 2^21). Useful batch sizes are the dyadic boundaries:
  n64 fills 2^20, n128 fills 2^21, n256 fills 2^22.
- Memory is a non-issue on 24 GB (n256 peaks ~5.3 GB; even 2^23 would fit).
- Parallel bb instances barely help: 2× n64 concurrently = 4.71s vs 4.9s
  sequential (**4%**); 4× = 8.23s vs 9.8s (**16%**). bb's internal
  multithreading already saturates the cores — more *processes* on one
  machine buy almost nothing; scaling out means more *machines*.

## 2. The 5-second answer (single machine, no recursion)

Within one ~5s ledger close:

- **n64 fits easily** (2.83s end-to-end) → 64 payments/block, 12.8 tx/s.
- **n128 fits if witness generation is pipelined** (built for batch k+1 while
  batch k proves — the sequencer's engine/batcher split already permits
  this): prove-only 4.84s < 5s → **128 payments/block ≈ 25.6 tx/s
  sustained**. Without pipelining it's 5.43s, just over.
- n256 (8.05s) overshoots one block but yields the best tx/s if the product
  accepts ~2-block (≈10s) settlement cadence: 256 per 2 blocks = 25.6 tx/s —
  identical sustained throughput to pipelined n128, cheaper on-chain
  (half the submit fees), at double the latency.

**Practical ceiling on this machine at 5s cadence: ~128 payments/block
(~26 tx/s) with pipelined witness generation.** The Soroban side is never
the limit (verification is log-sized: ~90M insns vs the 400M cap, and fees
~0.12 XLM/batch regardless of N).

## 3. Recursion: measured, not modeled

We built and proved real UltraHonk recursion (inner: `batch_n16` with
poseidon2 transcript; outer: `std::verify_proof_with_type(…, HONK)` proven
with keccak transcript for on-chain compatibility; outer proof verified ✓):

| circuit | verifies | gates | bb prove | peak RSS |
|---|---|---|---|---|
| recur1 | 1 inner proof | 2^20 | 3.09 s | 2.6 GB |
| recur2 | 2 inner proofs | 2^21 | 6.06 s | 5.1 GB |

**One in-circuit UltraHonk verification costs ~2^20 gates ≈ 3s of proving —
the same as proving 64 payments directly.** The marginal cost is flat
(recur2 = 2× recur1): no batching discount for verifying more proofs in one
outer circuit at these sizes.

### Why recursion loses on a single machine

To prove 128 payments recursively (2 chunks of n64 + fold): chunk proving
≈4.7s (measured 2× parallel) + outer ≈6.1s ≈ **10.8s** vs **5.4s direct** —
2× worse. The two structural reasons:

1. The fold overhead (~1M gates/proof) is enormous relative to our tiny
   per-payment cost (16K gates): one aggregation edge = 64 payments.
2. Chunks can't truly prove in parallel on one machine (bb saturates cores).

### Where recursion does pay: a prover farm

With **multiple machines**, chunks prove in parallel for real, and a
dedicated aggregator folds. Pipelined (workers prove batch k+1's chunks while
the aggregator folds batch k):

| topology | per-block output | sustained tx/s | settle latency |
|---|---|---|---|
| 1 machine, direct n128 (baseline) | 128 | ~26 | 1 block |
| 4 workers × n256 + 1 aggregator (fold 4 ≈ 2^22 ≈ 7s) | 1024 / ~8s | **~128** | ~3 blocks |
| scale workers/chunk size further | — | ~linear in workers | + log(k) folds |

Rules of thumb from the measurements:

- **Chunk size ≥ 256** so the 3s fold cost is amortized (fold overhead:
  100% at c=64, ~40% at c=256, ~20% at c=512).
- **Keep the tree shallow** (one fold layer until >8-ish workers); every
  layer adds ~3s+ of aggregator latency.
- The aggregator is the eventual bottleneck (~1 fold edge / 3s); shard it
  (tree) only when worker count forces it.
- The final wrapper must stay keccak-transcript non-recursive UltraHonk —
  that's what the on-chain verifier accepts; inner proofs use poseidon2
  transcript. This exact combination is what we measured.

## 3.5 Production requirement & cloud reference

**Requirement: proving must sustain Stellar's ~5s ledger cadence.** The
sequencer batches eagerly (any tick with >1 pending payment builds a batch),
so the pipeline budget per batch is build + witness + prove + submit ≤ 5s —
in practice **bb prove(CIRCUIT_PKG) ≤ ~3.5s** leaves adequate headroom.
Deployment hardware must be provisioned to meet this, and the batch size is
chosen as the largest N whose prove time fits the budget on that hardware.

Cloud reference (Fly.io, `soribium` app; measured at deploy):

| VM | bb prove n16 | pipeline build→submit | meets 5s? |
|---|---|---|---|
| performance-2x (2 vCPU, 4GB) | (measured at deploy — see below) | — | — |

Scale knob: `fly scale vm performance-4x` (then re-measure and update this
table). The ~$13/mo shared-cpu tier is ruled out by this requirement —
shared vCPUs are burst-throttled and can't hold a steady 5s proving cadence.

## 4. Recommendations

1. **The 5s cadence is a hard requirement** (§3.5): pick the largest batch
   size whose prove time fits ~3.5s on the deployment hardware. On the M4
   Pro that's n128 (with pipelined witness gen); on the current cloud VM
   it's n16 (n64+ needs bigger cores — the `fly scale vm` path). No
   recursion at these sizes.
2. **Scale-out trigger:** adopt recursion only when sustained demand exceeds
   ~25 tx/s AND a prover farm exists. Start with 4 workers × n256 + 1
   aggregator ≈ 128 tx/s at ~3-block latency.
3. **Cheaper first lever:** decouple batch cadence from block time. Soroban
   fees don't grow with batch size, so bigger/less-frequent batches raise
   throughput with zero infra — it's purely a finality/UX trade.
4. Re-run `scripts/bench_proving.sh` when bumping bb versions; these numbers
   are toolchain-sensitive.
