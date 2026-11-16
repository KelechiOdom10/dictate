#!/usr/bin/env bash
# One-time: make the key pair that signs updates, and put its public half in
# the app config.
#
#   pnpm update:key
#
# The private key is written OUTSIDE the repo, to ~/.config/dictate/update.key
# (override with DICTATE_UPDATE_KEY), and is never committed. It is not the
# code-signing certificate; it only proves an update came from you. Back it up
# and remember the password: lose either and no installed copy can ever accept
# an update again, because the public key is baked into each build.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"

KEY="${DICTATE_UPDATE_KEY:-$HOME/.config/dictate/update.key}"
CONF="${DICTATE_TAURI_CONF:-$ROOT/src-tauri/tauri.conf.json}"

if [[ -e "$KEY" ]]; then
  echo "error: $KEY already exists. Refusing to replace a key that installed copies trust." >&2
  exit 1
fi
mkdir -p "$(dirname "$KEY")"
chmod 700 "$(dirname "$KEY")"

# Prompts for a password. Pass --ci to use none (testing only).
pnpm tauri signer generate -w "$KEY" "$@"
chmod 600 "$KEY"

PUBKEY="$(cat "$KEY.pub")"
python3 - "$CONF" "$PUBKEY" <<'PY'
import json, sys
path, pubkey = sys.argv[1:]
conf = json.load(open(path))
conf["plugins"]["updater"]["pubkey"] = pubkey
with open(path, "w") as f:
    json.dump(conf, f, indent=2)
    f.write("\n")
PY

echo
echo "private key: $KEY   (keep it; do not commit it)"
echo "public key written to $CONF — commit that change."
echo "Sign releases with: TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<password> pnpm release:publish <bump>"
