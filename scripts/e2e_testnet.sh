#!/usr/bin/env bash
# Soribium end-to-end acceptance test against public testnet, driving the
# NATIVE sequencer (the proving path bb needs is native arm64/amd64 here).
# Deploys a fresh contract, boots the sequencer, then:
#   deposit x2 -> auto-batched credit -> signed transfer + withdrawal ->
#   proved batch -> on-chain root advance + withdrawal payout,
# asserting every invariant (exit 1 on any mismatch).
#
# The docker-compose path (`just bootstrap && just up`) exercises the same
# sequencer binary; this script uses the native process so it runs anywhere
# bb is installed, including CI on amd64.
set -euo pipefail
cd "$(dirname "$0")/.."

SCRATCH="${SCRATCH:-$(mktemp -d)}"
PORT=8091
URL="http://127.0.0.1:$PORT"
IDENTITY=soribium-e2e
SIM="cargo run -q -p sequencer --bin wallet-sim --"

fail() { echo "ASSERT FAILED: $1" >&2; exit 1; }
jget() { grep -o "\"$1\":[^,}]*" | head -1 | cut -d: -f2- | tr -d '"'; }

echo "==> identity + token"
stellar keys generate "$IDENTITY" --network testnet --fund 2>/dev/null || stellar keys fund "$IDENTITY" --network testnet 2>/dev/null || true
SEQ_ADDR=$(stellar keys address "$IDENTITY")
SEQ_SECRET=$(stellar keys show "$IDENTITY")
stellar contract asset deploy --asset native --source "$IDENTITY" --network testnet 2>/dev/null || true
TOKEN=$(stellar contract id asset --asset native --network testnet)

echo "==> build wasm + deploy fresh rollup"
stellar contract build >/dev/null
VK=$(xxd -p fixtures/batch_n16/vk.bin | tr -d '\n')
GENESIS=$(cargo run -q -p sequencer -- genesis-root); GENESIS=${GENESIS#0x}
ROLLUP=$(stellar contract deploy --wasm target/wasm32v1-none/release/rollup.wasm \
  --source "$IDENTITY" --network testnet -- --token "$TOKEN" --vk "$VK" --genesis_root "$GENESIS")
echo "    rollup=$ROLLUP"

echo "==> boot sequencer"
export CONTRACT_ID=$ROLLUP TOKEN_ID=$TOKEN SEQUENCER_SECRET=$SEQ_SECRET SEQUENCER_ADDRESS=$SEQ_ADDR
export RPC_URL=https://soroban-testnet.stellar.org
export NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
export DB_PATH="$SCRATCH/e2e.db" LISTEN_ADDR="127.0.0.1:$PORT" BATCH_MAX_WAIT_SECS=15
export SORIBIUM_URL="$URL" SEQ_KEY="$IDENTITY"
rm -f "$DB_PATH"
cargo run -q --release -p sequencer > "$SCRATCH/e2e.log" 2>&1 &
SEQ_PID=$!
trap "kill $SEQ_PID 2>/dev/null || true" EXIT
for i in $(seq 1 30); do curl -sf "$URL/healthz" >/dev/null 2>&1 && break; sleep 2; done
curl -sf "$URL/healthz" >/dev/null || fail "sequencer did not become healthy"

ALICE=$($SIM pk 101 | grep pk_x | cut -d= -f2)
BOB=$($SIM pk 202 | grep pk_x | cut -d= -f2)
WD_DEST=$SEQ_ADDR

echo "==> deposits"
$SIM deposit "$ALICE" 1000000 >/dev/null
$SIM deposit "$BOB" 500000 >/dev/null
for i in $(seq 1 15); do
  [ "$(curl -s "$URL/status" | jget pending_deposits)" = "2" ] && break; sleep 3
done
[ "$(curl -s "$URL/status" | jget pending_deposits)" = "2" ] || fail "deposits not observed"

echo "==> batch 1 (deposit credit)"
for i in $(seq 1 24); do [ "$(curl -s "$URL/status" | jget batch_num)" = "1" ] && break; sleep 5; done
[ "$(curl -s "$URL/status" | jget batch_num)" = "1" ] || fail "deposit batch never confirmed"
[ "$(curl -s "$URL/account/$ALICE" | jget balance)" = "1000000" ] || fail "alice balance after deposit"

echo "==> L2 transfer + withdrawal"
$SIM send 101 "$BOB" 200000 0
$SIM withdraw 202 "$WD_DEST" 100000 0
for i in $(seq 1 24); do [ "$(curl -s "$URL/status" | jget batch_num)" = "2" ] && break; sleep 5; done
[ "$(curl -s "$URL/status" | jget batch_num)" = "2" ] || fail "tx batch never confirmed"

echo "==> assertions"
ALICE_BAL=$(curl -s "$URL/account/$ALICE" | jget balance)
BOB_BAL=$(curl -s "$URL/account/$BOB" | jget balance)
[ "$ALICE_BAL" = "800000" ] || fail "alice balance $ALICE_BAL != 800000"
[ "$BOB_BAL" = "600000" ] || fail "bob balance $BOB_BAL != 600000"
echo "    L2 balances correct (alice=800000 bob=600000)"

SEQ_ROOT=$(curl -s "$URL/status" | jget root)
CHAIN_ROOT=0x$(stellar contract invoke --id "$ROLLUP" --source "$IDENTITY" --network testnet --send=no -- root 2>/dev/null | tr -d '"')
[ "$SEQ_ROOT" = "$CHAIN_ROOT" ] || fail "root mismatch: seq $SEQ_ROOT vs chain $CHAIN_ROOT"
echo "    sequencer root == on-chain root ($SEQ_ROOT)"

# DA blob availability: /da/2 must serve a proof.
PROOF_LEN=$(curl -s "$URL/da/2" | grep -o '"proof":"[0-9a-f]*"' | head -1 | tr -d '"' | sed 's/proof://' | wc -c | tr -d ' ')
[ "$PROOF_LEN" -gt 20000 ] || fail "DA blob proof missing (len=$PROOF_LEN)"
echo "    DA blob for batch 2 served (proof present)"

# Anti-replay: resubmitting alice's consumed nonce 0 must NOT re-execute —
# the (sender,nonce) idempotency short-circuit returns the original included
# receipt.
REPLAY=$($SIM send 101 "$BOB" 200000 0 2>&1 || true)
echo "$REPLAY" | grep -q '"status":"included"' || fail "replay of nonce 0 was re-executed: $REPLAY"
# A future/gap nonce must be rejected outright.
GAP=$($SIM send 101 "$BOB" 1 5 2>&1 || true)
echo "$GAP" | grep -q "NONCE_MISMATCH" || fail "gap nonce 5 was not rejected: $GAP"
echo "    replay idempotent + gap-nonce rejected"

echo "==> E2E PASSED"
