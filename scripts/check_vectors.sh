#!/usr/bin/env bash
# Cross-stack vector drift gate (CI):
#   1. Regenerate fixtures/vectors.json and circuits/lib/src/tx_vectors.nr
#      from the harness; fail if the checked-in copies differ.
#   2. Assert the Noir sources pin the same golden values (nargo can't read
#      JSON, so its constants are duplicated by design — this catches the
#      duplicate going stale).
# A failure means a crypto/encoding change wasn't propagated everywhere:
# regenerate with the two commands below, re-run the full suites, and look
# hard at WHY the values moved before shipping.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> regenerating vectors from the harness"
cargo run -q --release -p harness -- vectors-json
cargo run -q --release -p harness -- noir-tx-vectors

echo "==> checking for drift vs checked-in copies"
git diff --exit-code fixtures/vectors.json circuits/lib/src/tx_vectors.nr || {
  echo "DRIFT: harness output no longer matches checked-in vectors (see diff above)"
  exit 1
}

echo "==> checking Noir pinned constants against vectors.json"
fail=0
for key in pad.pk_x pad.pk_y pad.r_x pad.r_y pad.s_lo pad.s_hi; do
  val=$(jq -r ".${key}" fixtures/vectors.json)
  grep -qi "$val" circuits/lib/src/tx.nr || { echo "MISSING in tx.nr: $key = $val"; fail=1; }
done
for key in hash2_1_2 hash4_1_2_3_4 empty_root_d8 leaf_1234_100_0 root_leaf_at_5 da_fold_0_42; do
  val=$(jq -r ".${key}" fixtures/vectors.json)
  grep -qri "$val" circuits/lib/src/test.nr || { echo "MISSING in test.nr: $key = $val"; fail=1; }
done
[ "$fail" = 0 ] || exit 1

echo "vectors OK"
