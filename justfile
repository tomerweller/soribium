export PATH := env_var("HOME") + "/.nargo/bin:" + env_var("HOME") + "/.bb:" + env_var("PATH")

# List recipes
default:
    @just --list

# Verify pinned toolchain is present
setup-check:
    nargo --version
    bb --version
    stellar --version
    cargo --version

# Compile all Noir circuits
build-circuits:
    cd circuits && nargo compile

# Run Noir tests
test-circuits:
    cd circuits && nargo test

# Prove one circuit package and stage fixtures (compile+execute+prove+vk)
prove pkg:
    circuits/scripts/prove.sh {{pkg}}

# Run all Rust tests (contract + harness; uses checked-in fixtures)
test:
    cargo test

# Build the contract wasm
build-contract:
    stellar contract build

# Regenerate all proof fixtures (nargo + bb required)
prove-demo:
    cargo run -q -p harness -- demo-batch
    cargo run -q -p harness -- demo-batch-n16

# Full custody loop against localnet with resource measurements
# (requires: stellar container start local --protocol-version 26 --limits unlimited)
e2e-local: build-contract
    scripts/e2e_local.sh

# Deploy a fresh Soribium instance to testnet + write .env
bootstrap:
    scripts/bootstrap_testnet.sh

# Run the sequencer natively (fast proving on Apple Silicon); reads .env
sequencer:
    set -a && . ./.env && set +a && cargo run --release -p sequencer

# Wallet dev server against the fixture mock (no backend needed)
wallet-dev:
    cd wallet && npm run dev:mock & sleep 1 && cd wallet && npm run dev

# Wallet crypto + build
wallet-test:
    cd wallet && npm test -- --run && npm run build

# Full local stack via docker-compose (needs .env from `just bootstrap`)
up:
    docker compose up -d --build

down:
    docker compose down

# Everything a fresh checkout needs to go green
check: setup-check test-circuits test wallet-test
