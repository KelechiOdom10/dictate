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
APP="${1:-$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app}"
OUT_DIR="$ROOT/release"

if [[ ! -d "$APP" ]]; then
  echo "error: no app bundle at $APP" >&2
  echo "       build it first: pnpm release:mac" >&2
  exit 1
fi

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
