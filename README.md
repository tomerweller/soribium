# Soribium

A payments **ZK-rollup (validium)** for the Stellar network, built with [Noir](https://noir-lang.org)
and UltraHonk proofs verified on Soroban via Protocol 25/26's native BN254 +
Poseidon host functions.

Users deposit a single SEP-41 token (native XLM here) into an on-chain
contract, transact cheaply off-chain with rollup-native keys, and a sequencer
batches those payments into one UltraHonk proof per batch that advances the
on-chain state root. Transaction data lives off-chain (validium), bound by a
proven commitment and served by the sequencer's DA endpoint.

> Research prototype on **testnet only**. Not audited; see [Trust model &
> limitations](#trust-model--limitations).

## What's here

| Component | Path | What it is |
|---|---|---|
| Circuits | `circuits/` | Noir batch state-transition circuit (Poseidon2 tree, Grumpkin Schnorr). Deployed size: `batch_n16` (4 deposits + 16 txs). |
| Contract | `contracts/rollup/` | Soroban contract: SEP-41 custody, deposit queue, `submit_batch` verifying a 5-public-input UltraHonk proof. |
| Harness | `harness/` | Shared Rust: account tree, keys, Poseidon2 (through a Soroban `Env`, so off-chain ≡ on-chain), witness builder, prover driver. |
| Sequencer | `sequencer/` | Long-running backend (axum): mempool, deposit watcher, batch/prove/submit pipeline, DA + state HTTP API. |
| Wallet | `wallet/` | Browser wallet (Vite + React + TS): L2 keys, Freighter deposits, send/withdraw/history, client-verified balances. |

Design details: [`DESIGN.md`](DESIGN.md). Feasibility measurements behind the
design: [`REPORT.md`](REPORT.md).

## Quick start (local, against testnet)

Prereqs: Rust 1.95 + `wasm32v1-none`, `nargo` 1.0.0-beta.11 + `bb` 0.87.0
(`just setup-check` verifies), the Stellar CLI, Node 22, and Docker.

```sh
just bootstrap      # fund sequencer, deploy SAC + rollup to testnet, write .env
just up             # docker compose: sequencer + wallet
open http://localhost:3000
```

**Apple Silicon:** `bb` publishes amd64-linux binaries only, so the sequencer
image is `linux/amd64` (emulated — give Docker ≥4GB + Rosetta). For fast local
dev, run the sequencer natively instead and containerize nothing:

```sh
just bootstrap
just sequencer      # native binary, ~0.8s proving; reads .env
cd wallet && npm run dev            # wallet dev server → VITE_SEQUENCER_URL
```

Wallet-only development needs no backend at all:

```sh
cd wallet && npm run dev:mock &     # fixture-backed mock sequencer
cd wallet && npm run dev
```

## How a payment flows

1. **Deposit** — user signs a Stellar tx (via Freighter) calling
   `deposit(from, l2_pk_x, amount)`; the contract escrows the token and
   enqueues an L2 credit. The sequencer's watcher observes it from the
   contract's FIFO queue.
2. **Transact** — user signs an L2 payment with their Grumpkin key in the
   browser and POSTs it to the sequencer's mempool.
3. **Batch** — the sequencer drains deposits + payments, builds a witness,
   proves the state transition with `bb`, and submits
   `submit_batch(new_root, deposit_count, withdrawals, da_commitment, proof)`.
4. **Verify** — the contract recomputes the deposit/withdrawal fold hashes
   from its own trusted state, assembles the 5 public inputs, verifies the
   UltraHonk proof, advances the root, releases deposits, and pays out
   withdrawals on L1.

The tx blob is published off-chain (sequencer `GET /da/:batch_num`) and bound
by `da_commitment`, the 5th public input — verifiers re-fold the blob and
check it against the on-chain commitment.

## Cloud deployment

The public instance runs on:

- **Wallet**: https://blob.tomerweller.com/soribium/ (GitHub Pages, deployed
  by `.github/workflows/wallet.yml` — the crypto vector tests gate every
  deploy)
- **Sequencer**: https://soribium.fly.dev (Fly.io, deployed by
  `.github/workflows/fly.yml` via remote amd64 builders — required, since bb
  ships no arm64-linux binary)

Ops notes:

- **VM sizing is governed by the 5s-cadence requirement** (docs/PROVING.md
  §3.5): bb prove must stay ≤ ~3.5s so every Stellar ledger can carry a
  batch. The org is currently billing-limited to 2 shared cores, where
  measured proving is n16 = 6.2–8.1s (fails) and n4 = 1.2–1.4s (passes) —
  so the cloud instance runs **batch_n4**. Once the Fly billing unlock
  allows `performance-4x`, re-bootstrap with the n16 VK and scale up.
- Fresh instance: `just bootstrap` (new contract + `.env`) then
  `scripts/deploy_fly.sh` (sets the secret, patches `fly.toml`, remote
  deploys). The SQLite state lives on the `soribium_data` volume; single
  machine by design — never scale horizontally.
- Secrets: only `SEQUENCER_SECRET`, via `fly secrets`; everything else in
  `fly.toml [env]` is a public identifier.

## Testing

```sh
just check              # nargo tests + Rust tests + wallet crypto/build
cargo test              # contract + harness + sequencer
scripts/e2e_testnet.sh  # full deposit→batch→withdraw against testnet, asserted
```

`scripts/e2e_testnet.sh` deploys a fresh contract, boots the native sequencer,
runs two deposits → an auto-batched credit → a signed transfer + withdrawal →
a proved batch, and asserts L2 balances, sequencer-root == on-chain-root, DA
blob availability, and stale-nonce rejection.

## Trust model & limitations

Validity is **trustless** — every root transition is proven, so funds cannot
be stolen. Data availability is **trusted to the sequencer operator**: if it
withholds a batch blob, users can't compute Merkle paths for newer roots and
the system freezes (a validium's defining trade-off). Production hardening:
DAC signatures over `da_commitment` verified in `submit_batch`.

Known deltas tracked for a production version (also in `DESIGN.md`):
single-operator sequencer with no forced-exit / censorship escape hatch;
localStorage key custody in the wallet; 256-account tree capacity;
circuit-level `pk_x` uniqueness (honest builder only); immutable VK
(a new circuit is a new contract instance); the verifier crate is unaudited.
Active spends require even-y Grumpkin keys; lifetime deposit credit is capped
at `u64::MAX` per `pk_x` to prevent unprovable queue heads; the public padding
keypair cannot receive deposits.
