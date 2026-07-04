#!/usr/bin/env bash
# Phase B1c checkpoint: drive the full sequencer pipeline against testnet —
# deposit x2 -> signed transfer + withdrawal -> auto-built proven batch ->
# on-chain root advance + withdrawal payout. Assumes a fresh contract was
# deployed (scratchpad/rollup.txt) and the sequencer key funded.
set -euo pipefail
cd "$(dirname "$0")/.."

SCRATCH=/private/tmp/claude-501/-Users-tomer-dev-stellar-zk-rollup/62c6f966-91af-4386-b338-5ec7c7d6aa60/scratchpad
export CONTRACT_ID=$(cat "$SCRATCH/rollup.txt")
export TOKEN_ID=$(cat "$SCRATCH/token.txt")
export SEQUENCER_SECRET=$(cat "$SCRATCH/seq_secret.txt")
export SEQUENCER_ADDRESS=$(stellar keys address soribium-seq)
export RPC_URL=https://soroban-testnet.stellar.org
export NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
export DB_PATH="$SCRATCH/seq.db"
export LISTEN_ADDR=127.0.0.1:8090
export BATCH_MAX_WAIT_SECS=15
export SORIBIUM_URL=http://127.0.0.1:8090
export SEQ_KEY=soribium-seq
rm -f "$DB_PATH"

SIM="cargo run -q -p sequencer --bin wallet-sim --"
ALICE_PK=$($SIM pk 101 | grep pk_x | cut -d= -f2)
BOB_PK=$($SIM pk 202 | grep pk_x | cut -d= -f2)
WD_DEST=$(stellar keys address soribium-seq)  # withdraw back to the seq G-addr
echo "alice=$ALICE_PK"
echo "bob=$BOB_PK"

echo "==> booting sequencer"
cargo run -q -p sequencer > "$SCRATCH/seq.log" 2>&1 &
SEQ_PID=$!
trap "kill $SEQ_PID 2>/dev/null || true" EXIT
sleep 15

echo "==> L1 deposits (alice 1000000, bob 500000 stroops)"
$SIM deposit "$ALICE_PK" 1000000 >/dev/null
$SIM deposit "$BOB_PK" 500000 >/dev/null

echo "==> waiting for watcher to observe deposits"
for i in $(seq 1 12); do
  PENDING=$(curl -s "$SORIBIUM_URL/status" | grep -o '"pending_deposits":[0-9]*' | cut -d: -f2)
  [ "$PENDING" = "2" ] && break
  sleep 3
done
curl -s "$SORIBIUM_URL/status"; echo

echo "==> a batch with just deposits will auto-fire after BATCH_MAX_WAIT_SECS"
for i in $(seq 1 20); do
  BN=$(curl -s "$SORIBIUM_URL/status" | grep -o '"batch_num":[0-9]*' | cut -d: -f2)
  [ "$BN" = "1" ] && break
  sleep 5
done
echo "batch_num after deposit batch: $(curl -s "$SORIBIUM_URL/status" | grep -o '"batch_num":[0-9]*')"

echo "==> alice balance/nonce"
curl -s "$SORIBIUM_URL/account/$ALICE_PK"; echo

echo "==> L2 transfer alice->bob 200000 (nonce 0) + withdrawal bob->seq 100000 (nonce 0)"
$SIM send 101 "$BOB_PK" 200000 0
$SIM withdraw 202 "$WD_DEST" 100000 0

echo "==> waiting for second batch"
for i in $(seq 1 20); do
  BN=$(curl -s "$SORIBIUM_URL/status" | grep -o '"batch_num":[0-9]*' | cut -d: -f2)
  [ "$BN" = "2" ] && break
  sleep 5
done

echo "==> final status + balances"
curl -s "$SORIBIUM_URL/status"; echo
echo "alice:"; curl -s "$SORIBIUM_URL/account/$ALICE_PK"; echo
echo "bob:"; curl -s "$SORIBIUM_URL/account/$BOB_PK"; echo
echo "da blob for batch 2:"; curl -s "$SORIBIUM_URL/da/2" | head -c 300; echo
echo "==> on-chain root:"; stellar contract invoke --id "$CONTRACT_ID" --source soribium-seq --network testnet --send=no -- root 2>/dev/null

echo "==> log tail"; tail -15 "$SCRATCH/seq.log"
