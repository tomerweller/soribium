#!/usr/bin/env bash
# Deploy the sequencer to Fly.io from the bootstrap-generated .env:
# sets the secret, patches the public identifiers into fly.toml, and
# remote-builds (required: the image is amd64-only and Apple Silicon
# can't build it locally).
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] || { echo "no .env — run scripts/bootstrap_testnet.sh first"; exit 1; }
set -a; . ./.env; set +a

APP="${FLY_APP:-soribium}"

echo "==> patching fly.toml [env] with public identifiers"
sed -i '' \
  -e "s|CONTRACT_ID = \".*\"|CONTRACT_ID = \"$CONTRACT_ID\"|" \
  -e "s|TOKEN_ID = \".*\"|TOKEN_ID = \"$TOKEN_ID\"|" \
  -e "s|SEQUENCER_ADDRESS = \".*\"|SEQUENCER_ADDRESS = \"$SEQUENCER_ADDRESS\"|" \
  fly.toml

echo "==> setting SEQUENCER_SECRET (staged; applied with the deploy)"
fly secrets set --app "$APP" --stage "SEQUENCER_SECRET=$SEQUENCER_SECRET" >/dev/null

echo "==> remote deploy"
fly deploy --app "$APP" --remote-only

echo "==> status"
fly status --app "$APP" | head -12
curl -s "https://${APP}.fly.dev/status" && echo
