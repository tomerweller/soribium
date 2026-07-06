#!/usr/bin/env bash
# Proving-time benchmark across batch sizes, on the current machine.
# Prereq: witnesses exist (cargo run -p harness -- demo-batch[-nN] with
# STAGE_FIXTURES=0), i.e. circuits/target/<pkg>.{json,gz} are fresh.
# Output: TSV lines  pkg  txs  run  witness_s  prove_s  peak_rss_mb
set -euo pipefail
cd "$(dirname "$0")/../circuits"
export PATH="$HOME/.nargo/bin:$HOME/.bb:$PATH"

RUNS="${RUNS:-2}"
OUT="${OUT:-/tmp/bench_proving.tsv}"
: > "$OUT"

bench() {
  local pkg="$1" txs="$2"
  for run in $(seq 1 "$RUNS"); do
    # Witness generation (nargo execute), timed.
    local wt t0 t1
    t0=$(python3 -c 'import time; print(time.time())')
    nargo execute --package "$pkg" bench_wit >/dev/null 2>&1
    t1=$(python3 -c 'import time; print(time.time())')
    wt=$(python3 -c "print(f'{$t1-$t0:.2f}')")

    # bb prove, timed with peak RSS.
    local tmp; tmp=$(mktemp)
    /usr/bin/time -l bb prove --scheme ultra_honk --oracle_hash keccak \
      --bytecode_path "target/${pkg}.json" --witness_path target/bench_wit.gz \
      --output_path /tmp/bb-bench-out >/dev/null 2>"$tmp"
    local pt rss
    pt=$(grep -E "real" "$tmp" | awk '{print $1}')
    rss=$(grep "maximum resident set size" "$tmp" | awk '{printf "%.0f", $1/1048576}')
    rm -f "$tmp"
    echo -e "${pkg}\t${txs}\t${run}\t${wt}\t${pt}\t${rss}" | tee -a "$OUT"
  done
}

bench batch_n4 4
bench batch_n16 16
bench batch_n64 64
bench batch_n128 128
bench batch_n256 256
echo "done -> $OUT"
