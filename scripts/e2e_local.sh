#!/usr/bin/env bash
# End-to-end spike run against a Protocol 26 localnet:
#   deposit x2 -> submit_batch (real UltraHonk proof) -> withdrawal payout,
# with resource measurements captured via simulateTransaction.
#
# Prereqs: `stellar container start local --protocol-version 26` already
# healthy (script checks), fixtures regenerated (`just prove-demo` /
# `cargo run -p harness -- demo-batch`), wasm built (`stellar contract build`).
#
# Usage: scripts/e2e_local.sh [network]   (default: local)
set -euo pipefail
cd "$(dirname "$0")/.."

NET="${1:-local}"
if [ "$NET" = "local" ]; then
  RPC=http://localhost:8000/rpc
else
  RPC=https://soroban-testnet.stellar.org
fi

rpc() {
  curl -sf -X POST "$RPC" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}"
}

echo "==> checking network health + protocol"
HEALTH=$(rpc getHealth '{}' | jq -r .result.status)
[ "$HEALTH" = "healthy" ] || { echo "RPC not healthy"; exit 1; }
PROTO=$(rpc getNetwork '{}' | jq -r .result.protocolVersion)
echo "    protocol version: $PROTO"
[ "$PROTO" -ge 26 ] || { echo "need Protocol >= 26 (BN254 MSM host fns)"; exit 1; }

echo "==> identities"
stellar keys generate seq-e2e --network "$NET" --fund 2>/dev/null || true
SEQ=$(stellar keys address seq-e2e)
# Re-fund unconditionally: local networks reset on container restart.
stellar keys fund seq-e2e --network "$NET" 2>/dev/null || true
echo "    sequencer/depositor: $SEQ"

echo "==> native-asset SAC (the pinned SEP-41 token)"
TOKEN=$(stellar contract asset deploy --asset native --source seq-e2e --network "$NET" 2>/dev/null \
  || stellar contract id asset --asset native --network "$NET")
echo "    token: $TOKEN"

echo "==> deploy rollup"
GENESIS=$(jq -r .old_root fixtures/batch_n4/meta.json | sed 's/^0x//')
VK_HEX=$(xxd -p fixtures/batch_n4/vk.bin | tr -d '\n')
ROLLUP=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/rollup.wasm \
  --source seq-e2e --network "$NET" -- \
  --token "$TOKEN" --vk "$VK_HEX" --genesis_root "$GENESIS")
echo "    rollup: $ROLLUP"

ALICE_PK=$(jq -r '.deposits[0].pk_x' fixtures/batch_n4/meta.json | sed 's/^0x//')
BOB_PK=$(jq -r '.deposits[1].pk_x' fixtures/batch_n4/meta.json | sed 's/^0x//')
WD_DEST=$(jq -r '.withdrawals[0].dest' fixtures/batch_n4/meta.json)

measure() { # name, tx-base64
  local name="$1" txb64="$2"
  local sim size
  sim=$(rpc simulateTransaction "{\"transaction\":\"$txb64\"}")
  size=$(printf '%s' "$txb64" | base64 -d | wc -c | tr -d ' ')
  echo "$sim" | jq --arg name "$name" --arg size "$size" \
    '{op: $name, tx_bytes_unsigned: ($size|tonumber),
      cpu_insns: (.result.transactionData // "" | try (fromjson) catch null),
      min_resource_fee: .result.minResourceFee,
      cost: .result.cost}' 2>/dev/null \
    || echo "{\"op\":\"$name\",\"tx_bytes_unsigned\":$size,\"sim\":$(echo "$sim" | jq .result.minResourceFee)}"
}

echo "==> deposit 1 (alice pk, 1000)"
DEP_TX=$(stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" --build-only -- \
  deposit --from "$SEQ" --l2_pk_x "$ALICE_PK" --amount 1000)
measure deposit "$DEP_TX" | jq -c '{op, tx_bytes_unsigned, min_resource_fee, cost}'
stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" -- \
  deposit --from "$SEQ" --l2_pk_x "$ALICE_PK" --amount 1000 > /dev/null

echo "==> deposit 2 (bob pk, 500)"
stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" -- \
  deposit --from "$SEQ" --l2_pk_x "$BOB_PK" --amount 500 > /dev/null

echo "==> submit_batch (simulate for costs, then send)"
ENVELOPE=$(cat fixtures/batch_n4/envelope.json)
BATCH_TX=$(stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" --build-only -- \
  submit_batch --sequencer "$SEQ" --envelope "$ENVELOPE")
measure submit_batch "$BATCH_TX" | jq -c '{op, tx_bytes_unsigned, min_resource_fee, cost}'
rpc simulateTransaction "{\"transaction\":\"$BATCH_TX\"}" \
  | jq '{cpu: .result.cost.cpuInsns, mem: .result.cost.memBytes, minResourceFee: .result.minResourceFee}'

stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" -- \
  submit_batch --sequencer "$SEQ" --envelope "$ENVELOPE" > /dev/null
echo "    batch submitted"

echo "==> post-state assertions"
ROOT=$(stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" --send=no -- root 2>/dev/null | tr -d '"')
WANT=$(jq -r .new_root fixtures/batch_n4/meta.json | sed 's/^0x//')
[ "$ROOT" = "$WANT" ] && echo "    root advanced ✓" || { echo "root mismatch: $ROOT != $WANT"; exit 1; }

PENDING=$(stellar contract invoke --id "$ROLLUP" --source seq-e2e --network "$NET" --send=no -- pending_deposit_count 2>/dev/null | tr -d '"')
[ "$PENDING" = "0" ] && echo "    deposit queue drained ✓" || { echo "pending=$PENDING"; exit 1; }

WD_BAL=$(stellar contract invoke --id "$TOKEN" --source seq-e2e --network "$NET" --send=no -- balance --id "$WD_DEST" 2>/dev/null | tr -d '"')
[ "$WD_BAL" = "100" ] && echo "    withdrawal paid out (100) ✓" || { echo "wd balance=$WD_BAL"; exit 1; }

ESCROW=$(stellar contract invoke --id "$TOKEN" --source seq-e2e --network "$NET" --send=no -- balance --id "$ROLLUP" 2>/dev/null | tr -d '"')
[ "$ESCROW" = "1400" ] && echo "    escrow balance 1400 ✓" || { echo "escrow=$ESCROW"; exit 1; }

echo "==> e2e complete"
