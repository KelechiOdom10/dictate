#!/usr/bin/env bash
# Signs the built dictate.app with a code-signing identity from the keychain.
#
#   DICTATE_SIGNING_IDENTITY="dictate local signing" scripts/sign-macos.sh
#
# Why this exists: macOS matches a microphone or accessibility grant against the
# app's designated requirement. An ad-hoc signature's requirement is that build's
# own hash, so every rebuild is a different app to the privacy database and every
# permission is asked again. A named certificate produces a requirement that
# names the certificate instead, so a grant survives an update (docs/distribution.md
# §2). This is not notarisation and it is not a Developer ID; the second Mac still
# needs the one-time Gatekeeper bypass in README.md.
#
# Nothing here creates a certificate or touches the login keychain. The
# certificate is created once by hand, in Keychain Access, by the owner; see
# docs/release.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app}"
ENTITLEMENTS="$ROOT/src-tauri/Entitlements.plist"
IDENTIFIER="com.kelechiodom.dictate"

if [[ ! -d "$APP" ]]; then
  echo "error: no app bundle at $APP" >&2
  echo "       build it first: pnpm release:mac" >&2
  exit 1
fi

if [[ -z "${DICTATE_SIGNING_IDENTITY:-}" ]]; then
  cat >&2 <<'MISSING'
error: DICTATE_SIGNING_IDENTITY is not set, so there is nothing to sign with.

Create the certificate once, then export its name for every release:

  1. Keychain Access > Certificate Assistant > Create a Certificate…
  2. Name: dictate local signing
     Identity Type: Self Signed Root
     Certificate Type: Code Signing
  3. Then: export DICTATE_SIGNING_IDENTITY="dictate local signing"

Full steps, including how to check the certificate exists, are in docs/release.md.
MISSING
  exit 1
fi

# A name that is not in the keychain would sign nothing useful, so check before
# codesign produces a confusing failure of its own.
if ! security find-identity -v -p codesigning | grep -qF "$DICTATE_SIGNING_IDENTITY"; then
  echo "error: no code-signing identity named \"$DICTATE_SIGNING_IDENTITY\" in any keychain." >&2
  echo "       identities this Mac can sign with:" >&2
  security find-identity -v -p codesigning | sed 's/^/         /' >&2
  exit 1
fi

echo "signing $APP"
echo "  identity:     $DICTATE_SIGNING_IDENTITY"
echo "  identifier:   $IDENTIFIER"
echo "  entitlements: $ENTITLEMENTS (microphone)"

# --deep is deliberately absent: it re-signs nested code with the wrong
# requirements. dictate has no nested helpers, so three passes cover it: any
# frameworks that appear inside the bundle, then the app itself.
if [[ -d "$APP/Contents/Frameworks" ]]; then
  while IFS= read -r -d '' nested; do
    codesign --force --timestamp=none --options runtime \
      --sign "$DICTATE_SIGNING_IDENTITY" "$nested"
  done < <(find "$APP/Contents/Frameworks" -maxdepth 1 -name '*.framework' -print0)
fi

codesign --force --timestamp=none --options runtime \
  --identifier "$IDENTIFIER" \
  --entitlements "$ENTITLEMENTS" \
  --sign "$DICTATE_SIGNING_IDENTITY" \
  "$APP"

codesign --verify --strict --verbose=2 "$APP"
echo
echo "designated requirement:"
codesign -d -r- "$APP" 2>&1 | sed -n 's/^designated => /  /p'
echo
echo "Gatekeeper will still say \"rejected\" for a self-signed app; that is expected."
echo "The second Mac clears it once with Open Anyway (README.md, Installing on a second Mac)."
