# Soribium sequencer image.
#
# Targets linux/amd64: Barretenberg (bb) publishes only amd64-linux binaries
# for 0.87.0, so this is the portable choice for real deployment (Fly.io et
# al. run amd64). On an Apple Silicon dev box either run the sequencer
# natively (`just sequencer`) or run this image under emulation with adequate
# RAM (bb needs ~1GB for n16). Build explicitly:
#   docker build --platform linux/amd64 -t soribium-sequencer .
FROM --platform=linux/amd64 rust:1.95-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY contracts/rollup/Cargo.toml contracts/rollup/Cargo.toml
COPY harness/Cargo.toml harness/Cargo.toml
COPY sequencer/Cargo.toml sequencer/Cargo.toml
# Build just the sequencer binary and its dependency graph (not the wasm
# contract — that's produced on the host at bootstrap time).
COPY contracts contracts
COPY harness harness
COPY sequencer sequencer
RUN cargo build --release -p sequencer --bins

FROM --platform=linux/amd64 debian:bookworm-slim AS tools
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates tar gzip git \
    && rm -rf /var/lib/apt/lists/*
# Pinned toolchain (same versions/commits as the repo's CI + Phase A).
ENV NARGO_VERSION=1.0.0-beta.11 BB_VERSION=0.87.0 STELLAR_VERSION=27.0.0
RUN curl -fsSL "https://raw.githubusercontent.com/noir-lang/noirup/c3bc9922bf7eeafdaba08fb6518776c4ba263a8c/install" -o /tmp/noirup.sh \
    && bash /tmp/noirup.sh \
    && /root/.nargo/bin/noirup -v "$NARGO_VERSION"
RUN curl -fsSL "https://raw.githubusercontent.com/AztecProtocol/aztec-packages/073ea66ad92c53ebbf7be70d28973a68a8628942/barretenberg/bbup/install" -o /tmp/bbup.sh \
    && bash /tmp/bbup.sh \
    && /root/.bb/bbup -v "$BB_VERSION"
RUN curl -fsSL "https://github.com/stellar/stellar-cli/releases/download/v${STELLAR_VERSION}/stellar-cli-${STELLAR_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin stellar

# trixie (glibc 2.41): the bb amd64 binary requires GLIBC >= 2.38 /
# GLIBCXX >= 3.4.31, newer than bookworm's 2.36.
FROM --platform=linux/amd64 debian:trixie-slim AS runtime
# git: nargo clones the noir-lang/poseidon dependency when compiling circuits.
# libdbus-1-3: the stellar CLI links it (keyring integration).
# jq: bb's CRS (SRS) download helper shells out to it on first prove.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git libdbus-1-3 jq \
    && (apt-get install -y --no-install-recommends libssl3t64 || apt-get install -y --no-install-recommends libssl3) \
    && rm -rf /var/lib/apt/lists/*
# Toolchain at the paths prove.sh / prover.rs expect ($HOME/.nargo/bin, $HOME/.bb).
COPY --from=tools /root/.nargo /root/.nargo
COPY --from=tools /root/.bb /root/.bb
COPY --from=tools /usr/local/bin/stellar /usr/local/bin/stellar
ENV PATH="/root/.nargo/bin:/root/.bb:${PATH}"
COPY --from=builder /app/target/release/sequencer /usr/local/bin/sequencer
COPY --from=builder /app/target/release/wallet-sim /usr/local/bin/wallet-sim
# Ship the Noir workspace and pre-compile the circuit so runtime only runs
# execute + prove.
COPY circuits /app/circuits
# Bake both deployable batch sizes (CIRCUIT_PKG selects at runtime); the
# poseidon git dependency gets cloned here so runtime needs no network.
RUN cd /app/circuits && nargo compile --package batch_n16 && nargo compile --package batch_n4
# Warm bb's CRS cache (downloaded on first prove) and validate the full
# witness->prove chain at build time, so runtime proving never needs network.
RUN cd /app/circuits && nargo execute --package batch_n16 crs_warm \
    && mkdir -p /tmp/crs-warm \
    && bb prove --scheme ultra_honk --oracle_hash keccak \
         --bytecode_path target/batch_n16.json --witness_path target/crs_warm.gz \
         --output_path /tmp/crs-warm \
    && test -s /tmp/crs-warm/proof && rm -rf /tmp/crs-warm target/crs_warm.gz
ENV CIRCUITS_DIR=/app/circuits
ENV DB_PATH=/data/sequencer.db
ENV LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080
VOLUME ["/data"]
ENTRYPOINT ["sequencer"]
