#!/usr/bin/env bash
# Compile, execute, prove, and write the packed VK for one circuit package,
# then stage the on-chain artifacts under fixtures/<package>/.
#
# This script is the ONLY place bb is invoked: the Soroban verifier supports
# exactly one flavor (UltraHonk, BN254, Keccak-256 transcript, non-ZK,
# non-recursive), so `--scheme ultra_honk --oracle_hash keccak` must be on
# BOTH `bb prove` and `bb write_vk`. A proof or VK produced without the keccak
# oracle will fail on-chain with no useful diagnostic.
#
# VK packing: with the keccak oracle, bb 0.87.0 emits exactly the 1760-byte
# layout the verifier's load_vk_from_bytes expects: 32-byte header (four BE
# u64s: circuit_size, log_circuit_size, public_inputs_size, pub_inputs_offset)
# + 27 G1 commitments x 64 bytes. (The default-oracle VK OZ builds from is
# 1764 bytes with an extra u32 user-PI count at [32..36) that must be
# stripped — measured here to NOT apply to keccak-oracle VKs.) Lengths are
# asserted so any bb layout change fails loudly.
set -euo pipefail

cd "$(dirname "$0")/.."

PKG="${1:?usage: prove.sh <circuit-package>}"
NARGO="${NARGO:-$HOME/.nargo/bin/nargo}"
BB="${BB:-$HOME/.bb/bb}"

PACKED_VK_LEN=1760
PROOF_LEN=14592

OUT="target/${PKG}-out"
FIXTURES="../fixtures/${PKG}"
mkdir -p "$OUT" "$FIXTURES"

echo "==> nargo compile + execute (${PKG})"
"$NARGO" compile --package "$PKG"
"$NARGO" execute --package "$PKG"

echo "==> bb prove"
"$BB" prove \
  --scheme ultra_honk \
  --oracle_hash keccak \
  --bytecode_path "target/${PKG}.json" \
  --witness_path "target/${PKG}.gz" \
  --output_path "$OUT" \
  --output_format bytes_and_fields

echo "==> bb write_vk"
"$BB" write_vk \
  --scheme ultra_honk \
  --oracle_hash keccak \
  --bytecode_path "target/${PKG}.json" \
  --output_path "$OUT" \
  --output_format bytes_and_fields

packed_len="$(wc -c < "$OUT/vk")"
if [ "$packed_len" -ne "$PACKED_VK_LEN" ]; then
  echo "ERROR: bb VK is ${packed_len} bytes, expected ${PACKED_VK_LEN}; bb layout changed?" >&2
  exit 1
fi
cp "$OUT/vk" "$OUT/vk.bin"

proof_len="$(wc -c < "$OUT/proof")"
if [ "$proof_len" -ne "$PROOF_LEN" ]; then
  echo "ERROR: proof is ${proof_len} bytes, expected ${PROOF_LEN}" >&2
  exit 1
fi

cp "$OUT/proof" "$OUT/public_inputs" "$OUT/vk.bin" "$FIXTURES/"
echo "==> staged fixtures/${PKG}/{proof, public_inputs, vk.bin}"
echo "    proof=${proof_len}B vk.bin=${packed_len}B public_inputs=$(wc -c < "$OUT/public_inputs")B"
