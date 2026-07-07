# Proving throughput vs. Stellar block time

Empirical analysis of batch size → proving time, and whether recursion helps
maximize payments proven within one Stellar ledger (~5s).

**Reference machine:** Apple M4 Pro, 12 cores, 24 GB RAM. Toolchain: nargo
1.0.0-beta.11, bb 0.87.0 (native arm64), UltraHonk/keccak for on-chain proofs.
Reproduce with `scripts/bench_proving.sh` (direct ladder) and the
`circuits/recur1`, `circuits/recur2` benchmark circuits (recursion).
All numbers best-of-2 unless noted; measured 2026-07-06.

**Second machine:** Apple M5 Pro, 18 cores, 48 GB RAM. Same toolchain (nargo
1.0.0-beta.11, bb 0.87.0 native arm64). Rerun of the full suite (direct
ladder, recursion, parallel contention) measured 2026-07-07 — see "Rerun on
M5 Pro" under each section below.

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

### Rerun on M5 Pro (18 cores, 48 GB; measured 2026-07-07)

| N txs | witness gen | bb prove | e2e (wit+prove) | peak RSS | prove-only tx/s |
|---|---|---|---|---|---|
| 4 | 0.10 s | 0.17 s | 0.27 s | 0.16 GB | 24 |
| 16 | 0.14 s | 0.49 s | 0.63 s | 0.67 GB | 33 |
| 64 | 0.28 s | 1.67 s | 1.95 s | 2.61 GB | 38 |
| 128 | 0.47 s | 3.41 s | 3.88 s | 5.09 GB | 38 |
| 256 | 0.82 s | 4.95 s | 5.77 s | 5.05 GB | 52 |

- **~1.4–1.8× faster proving across the board** vs the M4 Pro (18 vs 12
  cores), with the biggest relative gains at small N and the gap narrowing
  at large N as bb's own multithreading saturates more of the extra cores.
- **Parallel bb instances scale meaningfully better here**: 2× n64
  concurrently = 2.27 s vs 3.34 s sequential (**32% savings**, vs the M4
  Pro's 4%); 4× = 3.69 s vs 6.68 s sequential (**45% savings**, vs 16%). One
  bb instance doesn't saturate 18 cores as completely as it does 12, so
  concurrent *processes* on one machine buy noticeably more here — though
  scaling out to more machines is still the more reliable lever.
- **Practical effect on the 5s-cadence ceiling (§2, §4):** n128 prove-only
  (3.41s) already clears the ~3.5s production budget with *no* pipelining,
  and n256 (4.95s) now fits inside a single ~5s ledger directly — the M4 Pro
  needed pipelined witness-gen for n128, or a 2-block cadence for n256, to
  hit those sizes.

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

**Reproducing this benchmark** (no checked-in script — `circuits/recur1` and
`circuits/recur2`'s `Prover.toml`s are hand-generated and gitignored):

```sh
export PATH="$HOME/.nargo/bin:$HOME/.bb:$PATH"
cd circuits
# 1. Need a compiled batch_n16 + witness (cargo run -p harness -- demo-batch-n16
#    from repo root does both, or nargo compile/execute directly).
# 2. bb's OLD_API tool packs an inner proof + vk into exactly the Prover.toml
#    shape recur1/recur2 expect (verification_key/proof/public_inputs/key_hash
#    as Field arrays; poseidon2 oracle, sized for in-circuit verification).
#    It reads target/witness.gz specifically, not target/<pkg>.gz.
cp target/batch_n16.gz target/witness.gz
bb OLD_API write_recursion_inputs_ultra_honk \
  --bytecode_path target/batch_n16.json --output_path target/recur_inputs
cp target/recur_inputs/Prover.toml recur1/Prover.toml
# recur2 needs proof_a/pi_a/proof_b/pi_b — duplicate the same inner proof
# into both slots (measures marginal verify cost, not two distinct batches).
python3 - <<'EOF'
import re
f = {m.group(1): m.group(2) for m in re.finditer(
    r'^(\w+) = (\[.*?\]|".*?")\s*$', open('target/recur_inputs/Prover.toml').read(), re.M)}
open('recur2/Prover.toml', 'w').write(
    f"key_hash = {f['key_hash']}\nverification_key = {f['verification_key']}\n"
    f"proof_a = {f['proof']}\npi_a = {f['public_inputs']}\n"
    f"proof_b = {f['proof']}\npi_b = {f['public_inputs']}\n")
EOF
# 3. Compile/execute/prove each outer circuit with keccak (on-chain transcript).
for pkg in recur1 recur2; do
  nargo compile --package "$pkg" && nargo execute --package "$pkg"
  bb prove --scheme ultra_honk --oracle_hash keccak \
    --bytecode_path "target/${pkg}.json" --witness_path "target/${pkg}.gz" \
    --output_path "target/${pkg}-out" --output_format bytes_and_fields
  bb write_vk --scheme ultra_honk --oracle_hash keccak \
    --bytecode_path "target/${pkg}.json" --output_path "target/${pkg}-out" \
    --output_format bytes_and_fields
  bb verify --scheme ultra_honk --oracle_hash keccak \
    --proof_path "target/${pkg}-out/proof" \
    --public_inputs_path "target/${pkg}-out/public_inputs" \
    --vk_path "target/${pkg}-out/vk"
done
```

Note `key_hash` comes back as `0x0…0` from the OLD_API tool — witness
generation and native verification both succeed with it unchanged, so it's
not cryptographically checked at this proof size (likely a vk-caching hint
in the backend, not a binding constraint).

**One in-circuit UltraHonk verification costs ~2^20 gates ≈ 3s of proving —
the same as proving 64 payments directly.** The marginal cost is flat
(recur2 = 2× recur1): no batching discount for verifying more proofs in one
outer circuit at these sizes.

### Rerun on M5 Pro (18 cores, 48 GB; measured 2026-07-07)

Regenerated the inner `batch_n16` witness/proof (poseidon2 transcript,
`bb OLD_API write_recursion_inputs_ultra_honk` to produce the
verification_key/proof/public_inputs/key_hash fields), fed it into
`circuits/recur1` and `circuits/recur2` (recur2 verifies the same inner proof
twice — marginal-cost measurement, not two distinct batches), proved each
outer circuit with the keccak transcript, and independently verified both
proofs with `bb verify` (both ✓).

| circuit | verifies | gates | bb prove | peak RSS |
|---|---|---|---|---|
| recur1 | 1 inner proof | 2^20 | 1.76 s | 2.67 GB |
| recur2 | 2 inner proofs | 2^21 | 3.66 s | 4.97 GB |

Same relationship holds: recur1 (1.76s) ≈ direct n64 (1.67s) — one in-circuit
verification still costs about what proving 64 payments directly costs, and
recur2 ≈ 2× recur1 (flat marginal cost). Both ~1.7× faster than the M4 Pro,
in line with the direct-ladder speedup.

### Why recursion loses on a single machine

To prove 128 payments recursively (2 chunks of n64 + fold): chunk proving
≈4.7s (measured 2× parallel) + outer ≈6.1s ≈ **10.8s** vs **5.4s direct** —
2× worse. The two structural reasons:

1. The fold overhead (~1M gates/proof) is enormous relative to our tiny
   per-payment cost (16K gates): one aggregation edge = 64 payments.
2. Chunks can't truly prove in parallel on one machine (bb saturates cores).

**M5 Pro rerun of the same arithmetic:** chunk proving = 2.27s (measured 2×
n64 parallel, §1 rerun) + outer (recur2, 2 inner proofs) = 3.66s ≈ **5.93s**
vs **3.41s direct** — still worse, but only **1.74×** rather than 2×. The 18
cores give real (if partial) parallelism to the chunk step where the 12-core
M4 Pro had almost none, narrowing recursion's disadvantage without closing
it — reason 1 (fixed fold overhead swamping a 16K-gate payment) still
dominates.

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

Cloud reference (Fly.io, `soribium` app; measured 2026-07-06, 3 runs each):

| VM | circuit | bb prove | meets ≤3.5s budget? |
|---|---|---|---|
| shared-cpu-2x, 4GB | batch_n16 (2^18) | 6.2–8.1 s | **no** |
| shared-cpu-2x, 4GB | batch_n4 (2^16) | 1.2–1.4 s | **yes** — deployed config |

Live pipeline (from production logs, real batches): **build → proof recorded
= 3.0–3.6 s** ✓; proof → on-chain confirmation adds ~8 s (submit + testnet
ledger close + 2s confirm polling), which is chain latency, not prover work.

The org is currently billing-limited to 2 shared cores per machine
(performance tiers and >2 cores need a Fly billing unlock). Under that
limit, **batch_n4 is the largest size meeting the budget**, so the cloud
instance runs n4 (4 payments/batch ≈ 0.8 tx/s sustained at 5s cadence).
Scale path once unlocked: `fly scale vm performance-4x`, re-measure n16
(expected ~2–3s from the core-count ratio), re-bootstrap with the n16 VK
(the VK is contract-immutable), and update this table.

## 4. Recommendations

1. **The 5s cadence is a hard requirement** (§3.5): pick the largest batch
   size whose prove time fits ~3.5s on the deployment hardware. On the M4
   Pro that's n128 (with pipelined witness gen); on the M5 Pro it's **n128
   with no pipelining needed** (3.41s), with n256 (4.95s) fitting a single
   block too; on the current cloud VM (2 shared cores) it's **n4**, with n16
   unlocking at performance-4x. No recursion at these sizes on any of them.
2. **Scale-out trigger:** adopt recursion only when sustained demand exceeds
   ~25 tx/s AND a prover farm exists. Start with 4 workers × n256 + 1
   aggregator ≈ 128 tx/s at ~3-block latency.
3. **Cheaper first lever:** decouple batch cadence from block time. Soroban
   fees don't grow with batch size, so bigger/less-frequent batches raise
   throughput with zero infra — it's purely a finality/UX trade.
4. Re-run `scripts/bench_proving.sh` when bumping bb versions; these numbers
   are toolchain-sensitive.
