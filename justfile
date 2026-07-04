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

# Everything a fresh checkout needs to go green
check: setup-check test-circuits test
