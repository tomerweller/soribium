# Soribium Test-Suite Audit & Proposed Architecture

Repo: `/Users/tomer/dev/stellar-zk-rollup` (branch `security/circuit-remediation`)
Date: 2026-07-16. Read-only static analysis; no suites were executed.

---

## 1. Inventory of existing tests

### 1.1 Circuits — Noir `#[test]` (nargo test), 23 tests total

| File | Tests | Coverage |
|---|---|---|
| `circuits/lib/src/test.nr` | 9 | Poseidon2 hash2/hash4 vs pinned harness constants, empty depth-8 root, leaf hash, DA fold step, empty-account-is-zero-leaf, single-leaf root, `update_root` roundtrip, 1 negative (`update_root` rejects wrong old root). Constants hand-transcribed from `cargo run -p harness -- vectors`. |
| `circuits/lib/src/schnorr_test.nr` | 13 | Generator/on-curve checks, harness signature vector verifies, and the security-remediation negatives: wrong message, tampered s, wrong signer, s=0, s_lo/s_hi non-u128 limbs, off-curve pk, off-curve R, `is_even_y` parity. 9 `should_fail` tests across the lib. |
| `circuits/trivial/src/main.nr` | 1 | Trivial. |

**How run:** `just test-circuits` / `just check` — **manual only. nargo never runs in any CI workflow.**

**Critical observation:** there are **zero circuit tests for `tx.nr` (`apply_deposit`, `apply_tx`) and `batch.nr`** — the actual state-transition relation. The remediation added guards there (amount>0, PAD_PK_X blacklist, even-y spend, empty-slot ghost-state pinning, u64 overdraft check) but the only remediation regression tests live at the Schnorr-primitive level.

### 1.2 Contract — `cargo test -p rollup`, 17 tests

| File | Tests | Coverage |
|---|---|---|
| `src/test.rs` | 1 | `is_canonical_field` boundaries (0, r−1, r, 0xff…ff). |
| `tests/verify_fixture.rs` | 4 | Real UltraHonk trivial-circuit fixture: verifies; tampered proof, wrong PI, wrong proof length rejected. |
| `tests/verify_batch.rs` | 2 | Real `batch_n4` fixture proof verifies (160-byte / 5 PIs); wrong-root PI rejected. |
| `tests/custody_loop.rs` | 9 | Strongest suite in the repo. Full custody loop (2 deposits → proven batch → transfer + withdrawal payout → replay rejected), PI blob layout check, and adversarial: tampered `da_commitment`, wrong `new_root`, tampered withdrawal amount, redirected withdrawal dest, wrong `deposit_count` (both directions), missing deposits (queue-prefix binding), deposit validation (0/negative/≥2^64 amounts, non-canonical pk_x, zero pk_x, `PAD_PK_X` rejected), lifetime credit cap boundary. |

**How run:** CI `fly.yml` (`cargo test -p rollup -p harness -p sequencer`) — but only **on push to `main`** (path-filtered), coupled to the Fly deploy job. Also `just test`. Fixture-based by design (no nargo/bb needed).

### 1.3 Harness — `cargo test -p harness`, 5 tests

| File | Tests | Coverage |
|---|---|---|
| `src/keys.rs` | 4 | sign/verify roundtrip, s-limb split reconstruction, even-y canonicalization of `from_sk`, pad keypair matches published circuit constants. |
| `src/tree.rs` | 1 | Merkle path verifies against root (pinned vectors). |
| `src/batch.rs` | **0** | `build_batch` — 370 lines, the entire witness builder mirroring the circuit (admission rules, folds, padding convention) — **untested**. |
| `src/prover.rs`, `src/l1.rs`, `src/poseidon.rs` | 0 | Prover.toml generation, `address_to_field`, hasher wrapper — untested directly (transitively via fixtures/wallet vectors only). |

### 1.4 Sequencer — `cargo test -p sequencer`, 1 test

| File | Tests | Coverage |
|---|---|---|
| `src/hexutil.rs` | 1 | hex parse strictness. |
| `src/engine.rs` (860 lines) | **0** | Mempool admission (`submit_tx`), eager-batching triggers (`try_build_batch`), eviction loop (`build_batch_now`), proof validation (`record_proof`), confirm-by-replay (`confirm_batch`), requeue (`fail_batch`), boot reconciliation (`load_and_reconcile`) — **all untested**. |
| `src/db.rs` (567), `src/api.rs`, `src/batcher.rs`, `src/watcher.rs` | 0 | Untested. |
| `src/bin/wallet_sim.rs` | — | Manual test client used by shell scripts. |

### 1.5 Wallet — vitest, ~31 cases

| File | Cases | Coverage |
|---|---|---|
| `src/crypto/vectors.test.ts` | ~20 | The "crypto gate": Poseidon2 hash2/hash4/daFold, pk(7)/pk(101)/pk(202), byte-exact Schnorr signature vector (sk=7,k=13,msg=42) + verify negatives, deposit/withdraw/DA fold chains vs `fixtures/batch_n4/meta.json` (exercises `addressToField` and `txMessage`), hex encode/decode strictness, 8 random sign/verify roundtrips, depth-8 Merkle root/leaf/path vs `test.nr` constants + path negatives. |
| `src/crypto/derive.test.ts` | 3 | Freighter-sig key derivation: deterministic, valid scalar + even-y pk, differs on differing sig. **No pinned golden value** — a silent change to the derivation formula would pass. |
| `src/errors.test.ts` | 5 | `friendlyError` never renders `[object Object]`; specific API-error mappings. |
| `src/learn/demo.test.ts` | 3 | Explainer demo tree with real Poseidon2; tampered-leaf attack demo. |

**How run:** CI `wallet.yml` "Crypto vector gate" before Pages deploy — but only **on push to `main` with `wallet/**` path changes**. Also `just wallet-test` and the Docker build.

### 1.6 E2E / scripts — manual only

| Script | Coverage |
|---|---|
| `scripts/e2e_local.sh` | Fixture `batch_n4` against Protocol-26 localnet: deposits → `submit_batch` with real proof → root advance, queue drained, withdrawal payout delta (+100), escrow balance; resource measurements. |
| `scripts/e2e_testnet.sh` | Full acceptance: fresh contract on testnet, native sequencer boot, deposits → auto-batch credit → signed transfer + withdrawal → root parity (sequencer == chain), DA blob served with proof, **replay idempotency** (nonce-0 resubmit returns original receipt) and **gap-nonce rejection**. Polling with sleeps; ~minutes; exits 1 on any mismatch. |
| `scripts/smoke_sequencer.sh` | Pipeline smoke; hardcodes a session-specific scratchpad path (stale/fragile — effectively dead). |
| `scripts/bench_proving.sh` | Benchmarks, not a test. |

None of these run in CI.

### 1.7 What CI actually gates vs what exists

| Suite | Exists | PR CI | Push-to-main CI | Nightly |
|---|---|---|---|---|
| nargo (circuits) | 23 tests | ✗ | **✗ (never)** | ✗ |
| cargo (rollup/harness/sequencer) | 23 tests | ✗ | ✓ only if Rust/circuit paths changed, as deploy precondition | ✗ |
| vitest (wallet) | ~31 | ✗ | ✓ only if `wallet/**` changed | ✗ |
| e2e scripts | 3 | ✗ | ✗ | ✗ |

**There is no PR gating at all** — tests run only after merge, as the gate on production deploys. Worse, the path filters mean the *cross-stack* vector gate is structurally broken: a circuit change that alters an encoding runs `cargo test` but **not** the wallet vector suite (no `wallet/**` diff), and never runs `nargo test` anywhere. The one invariant the project itself names as existential (byte-exact cross-stack equivalence) is the one the CI topology cannot catch end-to-end.

---

## 2. Gap analysis against the trust-model invariants

### Invariant 1 — Byte-exact cross-stack crypto equivalence
**Status: covered in substance, broken in mechanism.**
- Golden vectors exist but are **hand-transcribed constants duplicated in three places**: `test.nr`/`schnorr_test.nr` globals, `harness main.rs vectors()/sig_vectors()` (print-only), `wallet vectors.test.ts` constants, plus `fixtures/batch_n4/meta.json`. Nothing asserts the three copies agree except human eyes; harness `vectors` output is never diffed in CI.
- CI path filtering means circuit-side changes never re-run the wallet gate or nargo (see 1.7).
- `contracts/rollup/src/publics.rs::fold` / `address_to_field` (the contract's own Poseidon2 via `soroban_poseidon`) have **no direct equivalence test** against `harness::batch::fold` / `l1::address_to_field` — equivalence is proven only transitively through the one checked-in `batch_n4` proof. A divergence in a new domain or an `amount as u128` edge case wouldn't surface until a live batch failed to verify.
- `tx_message` (6-input hash) has no standalone pinned vector in the circuit tests (covered only via the DA-fold chain in the wallet test and via the fixture).

### Invariant 2 — Circuit soundness
**Status: primitives well tested; the state-transition relation untested.**
Missing negative/`should_fail` circuit tests, all in `circuits/lib/src/tx.nr` semantics:
- **Overdraft**: `apply_tx` with `amount > from_balance` must fail `assert_u64(checked_debited)`.
- **Recipient credit overflow**: `to_balance + amount ≥ 2^64` must fail.
- **Zero amount** on active tx / deposit (remediation guard — no regression test).
- **PAD_PK_X as active sender / recipient / deposit target** (remediation guard — no regression test at the `apply_*` level).
- **Odd-y active sender** rejected (`is_even_y` is tested as a function, not as an `apply_tx` gate).
- **Deposit pk mismatch on occupied slot**; **ghost state under a zero leaf** (`old_balance/old_nonce ≠ 0` with `old_pk_x = 0` — the remediation's empty-slot pinning).
- **Inactive-entry identity**: an inactive deposit/tx must not move the root or the folds (positive + adversarial witness attempts).
- **Signature binds the message fields**: signing nonce n but supplying `from_nonce = n+1` in the witness must fail (this is the circuit's replay protection — currently only implied).
- **Fold gating**: withdrawal fold only advances on `is_active·is_withdraw`; DA fold only on `is_active`.
- No positive end-to-end `batch::batch::<2,2>` circuit test replaying the `meta.json` scenario in-nargo (would pin the whole relation without bb).

### Invariant 3 — Contract
**Status: strong for the fixture path; gaps at edges.**
- `MAX_WITHDRAWALS` (9 withdrawals → `TooManyWithdrawals`) — untested.
- Withdrawal amount bounds inside `submit_batch` (`wd.amount <= 0` / `>= MAX_AMOUNT`) — untested.
- Multi-batch deposit-queue consumption (partial FIFO prefix across two batches; `dep_head/dep_tail` progression; `get_pending_deposit` trap on consumed seq) — untested; only single-batch full-consumption exists.
- **Auth semantics**: `submit_batch` only requires `sequencer.require_auth()` for *any* address — submission is effectively permissionless (safe by proof-binding, but that design assumption deserves an explicit test: a random third party CAN land a valid envelope; a test would document it as intended rather than an oversight).
- VK immutability: there is no setter (good), but nothing tests that constructor re-registration/re-init is impossible, or that an envelope proven under a *different* VK fails (a second fixture with a different circuit size would give this for free).
- Event emission (`events::Deposit`/`Batch`) — untested.
- `storage.rs` (queue arithmetic, lifetime-credit persistence) has no unit tests; covered only via the client tests.

### Invariant 4 — Sequencer state machine
**Status: essentially untested outside manual e2e. Largest single gap in the repo.**
Specific untested critical paths (all in `sequencer/src/engine.rs`):
- `submit_tx` admission matrix: idempotent (sender,nonce) resubmission; pending-nonce shadow (`expected_nonce = tree nonce + mempool depth`); `pending_out` balance shadowing; withdraw strkey shape check; `RecipientUnknown` vs recipient-created-by-pending-deposit; signature rejection with non-canonical limbs.
- **Admission ≡ circuit rules**: nothing asserts that every admitted tx is buildable (`build_batch` accepts it) and every circuit-rejectable tx is refused at admission. The eviction loop in `build_batch_now` exists precisely because they can diverge — and the eviction loop itself (bad tx evicted, rest rebuilt; deposit-jam stall path; empty-after-eviction) is untested.
- `try_build_batch` triggers: `>1 pending tx` eager fire, deposit-queue-full, max-wait, inflight suppression, `chain_synced` gate — untested.
- `record_proof` validation (roots+da match, 160-byte PI, 14 592-byte proof) — untested (these are the "sequencer can't ship the wrong proof" checks).
- `confirm_batch` deterministic replay: blob → `build_batch` → root must match; **root-mismatch abort path untested** (this is the "one code path for build and apply" safety property).
- `fail_batch` requeue and `load_and_reconcile`: the whole crash matrix — chain ahead by one with matching inflight (finish confirm), chain ahead without inflight (halt), batch-num divergence (halt), root divergence (halt), interrupted `proving` normalized to failed+requeued — **all five branches untested**, only reachable today by literally crashing a live sequencer.
- `parse_blob` round-trip (blob_json → parse_blob → identical build inputs) — untested; this is also the documented external DA-verifier recipe.
- `db.rs`: status transitions, `mempool_pending_for` ordering, cursor persistence — untested.
- `watcher.rs`: cursor advance, trap avoidance on consumed deposits — untested.

### Invariant 5 — Wallet
- **Key derivation has no pinned golden vector.** `derive.test.ts` checks determinism and range only. If the domain string, hash choice, or reduction in `deriveSkFromSignature` changes, tests stay green while **every user's L2 key silently changes** (funds unreachable). Needs `SIG (fixed bytes) → sk (pinned hex)`.
- `api/sign.ts` (local sign+verify before POST) and `api/sequencer.ts` — untested; `dev/mock-server.mjs` exists but no test drives the client against it.
- DA re-fold verification (`DaVerifier` widget / any production re-fold) — untested against a real blob shape from `engine::blob_json`.
- No component/hook tests (acceptable at this stage, but the Send flow's "verify locally before POST" is the one worth having).

### Invariant 6 — DA / validium
- No unit test anywhere that a stored/served DA blob re-folds to the committed `da_commitment` (Rust: `parse_blob` + `tx_message` fold; TS: same). Covered only implicitly by e2e's "blob served with proof present" length check — which checks *presence*, not *bindingness*.

### Flakiness / determinism risks
- e2e scripts: sleep-polling against live networks; fine as manual acceptance, unsuitable for CI as-is.
- `smoke_sequencer.sh` is bound to a stale scratchpad path — dead weight.
- Fixture drift: fixtures are regenerated manually (`just prove-demo`) with pinned bb 0.87.0/nargo beta.11; nothing detects when checked-in fixtures no longer match the current circuits (a circuit change + forgotten regeneration keeps `cargo test` green against *old* proofs while the deployed system uses a *new* VK).
- `wallet vectors.test.ts` random-roundtrip loop uses `randScalar` — non-deterministic but harmless (failures would be real bugs).

### Remediation lock-in check
The Schnorr hardening (s-limbs, s≠0, on-curve) **is** locked in by `schnorr_test.nr` + wallet negatives. The tx-level hardening (amount>0, PAD blacklist, even-y spend gate, empty-slot pinning) is locked in **only** on the contract/harness *input-validation* side (`custody_loop::deposit_validation`, `BuildError` variants) — **not at the circuit layer where the guard actually lives**. A regression deleting those `assert`s from `tx.nr` would pass every test in the repo today.

---

## 3. Proposed test-suite architecture

Shaped for a solo maintainer heading to production: everything fast and hermetic gates PRs; everything requiring bb/networks is nightly or manual; one source of truth for cross-stack vectors. Stays inside nargo test / cargo test / vitest, with two justified additions (`proptest` for Rust, nothing new for TS — vitest suffices).

### Layer 0 — Golden vectors: one source of truth
**Problem solved:** hand-copied constants in 3+ places; CI path filters can't see cross-stack drift.
- Promote `harness -- vectors` from print-to-stdout to **emit `fixtures/vectors.json`** (hash vectors, sig vector, tx_message vector, fold-chain vectors, address_to_field vector, derive-input placeholder). The harness is the right generator: it already computes all of them via `Hasher` (soroban Env — the same field arithmetic the contract uses).
- Consumers:
  - **Wallet**: `vectors.test.ts` imports the JSON instead of inline constants (vitest imports JSON natively).
  - **Noir**: keep pinned globals (nargo can't read files), but add a tiny check script (`scripts/check_vectors.sh`) that regenerates the JSON and greps the constants out of `test.nr`/`schnorr_test.nr`/`tx.nr` (PAD_* globals) and diffs.
  - **Contract**: a rollup test folds with `publics::fold`/`address_to_field` and asserts against `include_str!` of the JSON — this *directly* closes the publics.rs↔harness equivalence gap.
- **CI rule:** the vector job runs on changes to *any* of `circuits/`, `harness/`, `wallet/src/crypto/`, `contracts/rollup/src/publics.rs` — no per-stack path filtering for this job.
- Runtime: seconds. Runs: PR CI, always.

### Layer 1 — Circuit unit + negative tests (`nargo test`)
- New `circuits/lib/src/tx_test.nr`: the ~12 missing `apply_tx`/`apply_deposit`/`batch` tests from §2-Invariant-2 (positive identity/transfer/withdraw/deposit cases + `should_fail` negatives), including one `batch::<2,2>` replay of the meta.json scenario pinning all five public inputs.
- Runtime: tens of seconds (constraint-eval only, no proving). Runs: **PR CI** (install pinned nargo via noirup; cacheable), pre-commit optional.
- Top 5 first: (1) overdraft `should_fail`; (2) sig-binds-nonce `should_fail` (witness nonce ≠ signed nonce); (3) PAD_PK_X active-sender `should_fail`; (4) inactive-entry root/fold immobility; (5) full `batch::<2,2>` five-PI pin.

### Layer 2 — Rust unit + property tests (`cargo test`, `proptest`)
- `harness/src/batch.rs` `#[cfg(test)]`: happy-path witness (folds match independent recomputation; new_root matches applying same ops to a second tree), each `BuildError` variant triggered, padding-slot invariants, `parse_blob(blob_json(w)) == inputs` round-trip, DA re-fold of a built blob equals `witness.da_commitment` (**the validium verifier recipe as a unit test**).
- `proptest` (new dev-dependency, justified: the admission-rule space is combinatorial): random valid account trees + tx sequences → `build_batch` succeeds and folds/root verify; random single-field mutations (nonce±1, amount>balance, flipped sig limb) → typed rejection. ~200 cases, <10 s.
- `contracts/rollup`: layer-0 fold-equivalence test; `MAX_WITHDRAWALS`; withdrawal amount bounds; multi-batch queue consumption; permissionless-submit documentation test.
- Runs: **PR CI**, <1 min with rust-cache.
- Top 5 first: (1) build_batch↔blob↔re-fold round-trip; (2) BuildError matrix; (3) proptest valid-batch generator; (4) publics.rs fold equivalence; (5) MAX_WITHDRAWALS + amount bounds.

### Layer 3 — Sequencer engine integration (in-memory SQLite, no chain, no bb)
- The engine is already an actor taking `Command`s and owning `Connection` + `Tree` — construct `Engine` directly (or via `spawn`) with `Connection::open_in_memory()` and a test `Config`; no HTTP, no soroban RPC, no bb. The only seam to fake is "the chain": `ObservedDeposits` is already an injectable command, and `ConfirmBatch`/`FailBatch` let tests play the batcher. `record_proof` can be fed synthetic 160-byte PIs + a 14 592-byte dummy proof (it validates shape, not proof soundness — correct at this layer).
- Suites: admission matrix (≥10 cases from §2-Invariant-4), pipeline state machine (build→record→submitting→submitted→confirm; inflight suppression; eager triggers with a mockable clock or age-threshold config), eviction loop, confirm-replay root-mismatch abort, fail-requeue, and a **boot-recovery matrix** driving `load_and_reconcile` through all five branches against a prepared DB file + synthetic chain (root, batch_num) pairs.
- Runs: **PR CI**, seconds. Framework: plain `cargo test -p sequencer`; tokio only where oneshot replies need it.
- Top 5 first: (1) boot-recovery matrix; (2) admission ≡ build agreement (every admitted mempool set builds without eviction); (3) confirm_batch replay mismatch aborts without committing; (4) fail_batch requeues to pending exactly once; (5) eager-batching trigger truth table.

### Layer 4 — Proof-level (real bb) and fixture freshness
- Keep the excellent checked-in-fixture pattern for PR CI (zero bb cost).
- Add a **nightly workflow**: install pinned nargo+bb (cached), `just prove-demo` to regenerate fixtures from current circuits, then `cargo test -p rollup` against the *fresh* fixtures and `git diff --exit-code fixtures/` as a drift alarm. This catches "circuit changed, fixtures stale" — currently invisible. n4/n16 proving is sub-second-to-seconds locally; CI cost is dominated by toolchain install (~3–5 min cached).
- Local: `just check` already covers this when fixtures are regenerated; document `prove-demo && test` as the release ritual.

### Layer 5 — E2E
- **Nightly (optional, later):** `e2e_local.sh` against `stellar container start local` in a GitHub-Actions service container — hermetic, no faucets. Needs docker localnet with Protocol 26; medium setup effort.
- **Manual, pre-release:** `e2e_testnet.sh` stays the acceptance gate (it already asserts deltas, root parity, replay idempotency, gap nonces). Delete or fix `smoke_sequencer.sh` (stale hardcoded paths).
- Never on PR: network flakiness, faucet dependence, minutes of polling.

### Layer 6 — Wallet
- Keep the vector gate; rewire to `fixtures/vectors.json` (Layer 0).
- **Pinned derive vector** (P0, see roadmap).
- API-client tests: drive `api/sequencer.ts` + `api/sign.ts` against `dev/mock-server.mjs` (already exists) in vitest — assert sign-verify-before-POST and error mapping. No new framework needed.
- Component/hook tests: only the Send flow, later, via @testing-library/react (the single justified new dev-dep in TS, deferred to P2).
- Runs: **PR CI** on all paths that Layer 0 touches, not just `wallet/**`.

### CI topology (target)
- **`ci.yml` on `pull_request` + `push: main`** (new): vector regen-diff → nargo test → cargo test workspace → wallet vitest + tsc. ~5–8 min cached. Path filtering only to *skip doc-only changes*, never to skip cross-stack jobs on code changes.
- `fly.yml` / `wallet.yml`: deploy only, `needs:` the ci workflow (or keep their test steps as a second belt).
- **`nightly.yml`**: fixture regeneration with real bb + (later) localnet e2e.
- Pre-commit (optional): `cargo test -p harness` + `vitest --run` — the sub-30 s subset.

---

## 4. Prioritized roadmap

**P0 — do first (existential invariants currently ungated):**
1. **PR CI workflow running all existing suites, including `nargo test`, without cross-stack path filtering.** Protects: everything already written (currently post-merge only; circuits never). Effort: **S**. Highest risk-reduction-per-hour in the repo.
2. **Circuit negative tests for `apply_tx`/`apply_deposit`/`batch`** (`tx_test.nr`, ~12 tests incl. overdraft, sig-nonce binding, PAD/even-y gates, ghost-state pin, five-PI batch replay). Protects: invariant 2 — and locks in the security remediation at the layer it was applied. Effort: **M**.
3. **Pinned golden vector for `deriveSkFromSignature`** (fixed 64-byte sig → exact sk hex). Protects: invariant 5 — a silent derivation change loses every user's funds and no current test would notice. Effort: **S** (one test, ten minutes).
4. **`harness::build_batch` unit tests + blob/DA round-trip** (`parse_blob∘blob_json = id`; re-fold blob = `da_commitment`; `BuildError` matrix). Protects: invariants 1, 4, 6. Effort: **M**.
5. **Vectors single source of truth** (`fixtures/vectors.json` from harness; wallet imports it; regen-diff CI job; contract fold-equivalence test against it). Protects: invariant 1 (incl. the untested `publics.rs` ↔ harness seam). Effort: **M**.

**P1 — next:**
6. **Sequencer engine integration suite** with in-memory SQLite: boot-recovery matrix, admission matrix, pipeline transitions, confirm-replay mismatch, fail-requeue, trigger truth table. Protects: invariant 4 (currently 1 test for 1 900 lines). Effort: **L** (the single biggest coverage hole; worth splitting — boot matrix first).
7. **Contract edge tests**: `MAX_WITHDRAWALS`, withdrawal amount bounds in `submit_batch`, multi-batch partial queue consumption (needs an `envelope.json` for the `batch_n16` fixture), explicit permissionless-submit test. Protects: invariant 3. Effort: **M**.
8. **proptest for build_batch** (random valid batches verify; random mutations reject). Protects: invariants 2/4 agreement. Effort: **M**.
9. **Nightly fixture-regeneration workflow** (nargo+bb, prove-demo, diff, re-test). Protects: invariant 3 against toolchain/circuit drift. Effort: **M**.

**P2 — later:**
10. Wallet API-client tests against `dev/mock-server.mjs` (sign-before-POST, error mapping). Effort: **S–M**.
11. Localnet e2e in nightly CI (docker Protocol-26 container). Effort: **L**.
12. Wallet Send-flow component test (@testing-library/react). Effort: **M**.
13. Watcher/db unit tests; delete stale `smoke_sequencer.sh`. Effort: **S**.
14. TS property tests (fast-check) for encode/decode roundtrips — only if crypto surface grows. Effort: **S**.

---

## Appendix: headline numbers
- ~75 tests exist across 4 stacks; **0 run on PRs**; nargo tests run in **no** CI at all.
- Lines with zero direct tests: `sequencer/src/engine.rs` (860), `sequencer/src/db.rs` (567), `harness/src/batch.rs` (370), `circuits/lib/src/tx.nr` (194) + `batch.nr` (37).
- Best-in-repo pattern to build on: `contracts/rollup/tests/custody_loop.rs` (real-proof adversarial fixtures) and `wallet/src/crypto/vectors.test.ts` (the vector gate) — the architecture above mostly generalizes these two ideas.

---

## 5. Implementation status (2026-07-16)

P0 and P1 are implemented (same day as the audit). What shipped, by roadmap item:

| # | Item | Where |
|---|---|---|
| P0.1 | PR CI, all suites, no cross-stack path filters | `.github/workflows/ci.yml` (circuits / rust / wallet jobs) |
| P0.2 | Circuit state-transition tests (16) | `circuits/lib/src/tx_test.nr` + generated `tx_vectors.nr` (`harness -- noir-tx-vectors`) |
| P0.3 | Pinned derive golden vector | `wallet/src/crypto/derive.test.ts` |
| P0.4 | build_batch unit suite + blob/DA round-trip | `harness/src/batch.rs::tests`, `sequencer/src/engine.rs::tests` |
| P0.5 | Vectors single source of truth | `fixtures/vectors.json` (`harness -- vectors-json`), `scripts/check_vectors.sh`, `contracts/rollup/tests/vectors_equiv.rs` |
| P1.6 | Engine integration suite (8 tests incl. 5-branch boot matrix) | `sequencer/src/engine.rs::engine_tests` |
| P1.7 | Contract edge cases | `contracts/rollup/tests/edge_cases.rs` |
| P1.8 | build_batch proptests | `harness/src/batch.rs::prop_tests` |
| P1.9 | Nightly fixture-drift alarm | `.github/workflows/nightly.yml` |

Bugs found and fixed while implementing:
1. **Batch-failure wedge**: `fail_batch` kept the failed row under its
   `batch_num` (PRIMARY KEY); the rebuild reused the number and every later
   `insert_batch` hit the UNIQUE constraint — batching permanently stuck
   after any failed submission. Failed rows are now deleted after requeue
   (also in boot recovery's interrupted-prove path).
2. **Confirm-replay divergence**: `confirm_batch` replayed into the live
   tree before the root check; a corrupt batch row would leave in-memory
   state silently diverged. Replay now runs on a clone.
3. (Pre-dating the audit, caught the same day it predicted the class:)
   stale `batch_n16`/`batch_n64` fixture VKs from the circuit security
   remediation broke e2e — the nightly drift alarm now automates that check.

Remaining: P2 items (§4) — wallet API-client tests vs the mock server,
localnet e2e in nightly, Send-flow component test, watcher/db unit tests,
`smoke_sequencer.sh` cleanup.
