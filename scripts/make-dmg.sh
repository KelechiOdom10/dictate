#!/usr/bin/env bash
# Packs the built dictate.app into a drag-to-Applications DMG.
#
#   scripts/make-dmg.sh                 # -> release/dictate-<version>-arm64.dmg
#   scripts/make-dmg.sh path/to/app     # a specific bundle
#
# The image holds dictate.app and an Applications symlink, which is the whole
# convention anyone on macOS already knows: open it, drag the app onto the
# folder, eject. No Finder AppleScript runs, so it never stops on an Automation
# prompt in a fresh shell (CI=true tells the bundler the same thing).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${DICTATE_RELEASE_DIR:-$ROOT/release}"

# shellcheck source=target-dir.sh
source "$ROOT/scripts/target-dir.sh"

# An explicit path wins; otherwise ask cargo where the build went rather than
# assuming src-tauri/target, because CARGO_TARGET_DIR moves it.
if [[ $# -gt 0 ]]; then
  APP="$1"
else
  APP="$(dictate_app_path)" || {
    echo "error: cargo could not report a target directory" >&2
    exit 1
  }
fi

require_app "$APP" || {
  echo "       build it first: pnpm release:mac" >&2
  exit 1
}

NAME="$(basename "$APP" .app)"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
DMG="$OUT_DIR/${NAME}-${VERSION}-arm64.dmg"

mkdir -p "$OUT_DIR"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# ditto, not cp -R: it keeps resource forks, permissions and the code signature.
ditto "$APP" "$STAGE/$NAME.app"
ln -s /Applications "$STAGE/Applications"

rm -f "$DMG"
hdiutil create \
  -volname "$NAME" \
  -srcfolder "$STAGE" \
  -fs HFS+ \
  -format UDZO \
  -ov \
  -quiet \
  "$DMG"

hdiutil verify "$DMG" >/dev/null

echo "dmg: $DMG"
echo "app: $(du -sh "$APP" | cut -f1)   image: $(du -sh "$DMG" | cut -f1)"
