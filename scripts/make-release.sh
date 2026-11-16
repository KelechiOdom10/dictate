#!/usr/bin/env bash
# One command from source to a DMG the second Mac can install.
#
#   pnpm release:mac                                    # build, ad-hoc sign, pack
#   DICTATE_SIGNING_IDENTITY="dictate local signing" pnpm release:mac
#                                                       # build, sign, pack
#
# Apple Silicon only, on purpose (docs/distribution.md §1): Parakeet runs on the
# GPU through Metal and an Intel build would be much slower than the numbers in
# docs/spike-results.md. The minimum system version is macOS 15, set in
# tauri.conf.json; ggml's Metal code will not link below it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"
# shellcheck disable=SC1091
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"

APP="$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app"

# cmake caches the first deployment target it sees, so a cache built when the
# minimum was lower keeps passing -mmacosx-version-min=10.13 and the ggml build
# fails with "'path' is unavailable". Drop only the caches whose target
# disagrees with tauri.conf.json; a matching cache is worth its minutes.
MINIMUM="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['bundle']['macOS']['minimumSystemVersion'])")"
for cache in "$ROOT"/src-tauri/target/*/release/build/transcribe-cpp-sys-*/CMakeCache.txt; do
  [[ -e "$cache" ]] || continue
  cached="$(sed -n 's/^CMAKE_OSX_DEPLOYMENT_TARGET:[A-Z]*=//p' "$cache")"
  if [[ "$cached" != "$MINIMUM" ]]; then
    echo "==> dropping cmake cache built for macOS ${cached:-unknown} (want $MINIMUM)"
    rm -rf "$(dirname "$cache")"
  fi
done

# The icon's accent is a hand copy of each theme's focus ring; refuse to ship a
# build whose copy has gone stale.
echo "==> checking the icon colours match the themes"
node "$ROOT/scripts/check-icon-colours.mjs"

echo "==> building arm64 release"
# CI=true skips the Finder AppleScript that lays out the DMG window.
CI=true pnpm tauri build --target aarch64-apple-darwin --bundles app

echo
echo "==> binary"
lipo -archs "$APP/Contents/MacOS/dictate"
du -sh "$APP" "$APP/Contents/MacOS/dictate"

if [[ -n "${DICTATE_SIGNING_IDENTITY:-}" ]]; then
  echo
  echo "==> signing"
  "$ROOT/scripts/sign-macos.sh" "$APP"
else
  echo
  echo "==> no DICTATE_SIGNING_IDENTITY set, so this build is ad-hoc signed."
  echo "    Ad-hoc means the second Mac re-grants Microphone and Accessibility"
  echo "    on every new build. To sign: see docs/release.md."
  # Without this the bundle keeps only the signature the linker left on the
  # executable: no _CodeSignature, no sealed resources, and the identifier is
  # dictate-<hash of the binary> rather than the bundle id. codesign --verify
  # then fails with "code has no resources but signature indicates they must be
  # present", and macOS reads the app under that hash name in the privacy panes.
  codesign --force --timestamp=none --options runtime \
    --identifier com.kelechiodom.dictate \
    --entitlements "$ROOT/src-tauri/Entitlements.plist" \
    --sign - "$APP"
  codesign --verify --strict --verbose=2 "$APP"
  codesign -d -r- "$APP" 2>&1 | sed -n 's/^designated => /  /p'
fi

echo
echo "==> packing dmg"
"$ROOT/scripts/make-dmg.sh" "$APP"
