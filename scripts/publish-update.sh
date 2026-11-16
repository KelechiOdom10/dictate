#!/usr/bin/env bash
# Turns a built, signed dictate.app into the files the site serves under
# /updates/: the DMG (first install), an updater tarball with its signature,
# and latest.json. Run by `pnpm release:publish`; runnable alone to retry a
# failed upload.
#
#   scripts/publish-update.sh [path/to/dictate.app]
#
# Needs: HEAD exactly on a vX.Y.Z tag that matches the app's version, a clean
# tree, the update key (docs/release.md), and for the upload step
# DICTATE_SITE_UPDATES_DIR (the site project's updates folder).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"
# shellcheck disable=SC1091
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"

APP="${1:-$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app}"
KEY="${DICTATE_UPDATE_KEY:-$HOME/.config/dictate/update.key}"
BASE_URL="${DICTATE_UPDATES_URL:-https://dictate.kellycooks.dev/updates}"
OUT="$ROOT/release/updates"
die() { echo "error: $*" >&2; exit 1; }

[[ -d "$APP" ]] || die "no app bundle at $APP; run pnpm release:mac first"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
TAG="$(git describe --exact-match --tags HEAD 2>/dev/null || true)"
[[ "$TAG" == "v$VERSION" ]] || die "HEAD is on tag '${TAG:-none}', but the app is $VERSION. Updates publish from tag v$VERSION only."
git diff --quiet HEAD || die "the working tree has uncommitted changes"

PUBKEY="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['plugins']['updater']['pubkey'])")"
[[ "$PUBKEY" != REPLACE* ]] || die "no update public key in tauri.conf.json; run pnpm update:key"
[[ -f "$KEY" ]] || die "no update private key at $KEY; see docs/release.md"

DMG="$ROOT/release/dictate-$VERSION-arm64.dmg"
[[ -f "$DMG" ]] || die "no DMG at $DMG"
TARBALL="dictate-$VERSION-arm64.app.tar.gz"

rm -rf "$OUT"
mkdir -p "$OUT"
cp "$DMG" "$OUT/"
# The tarball is made here, after the certificate re-sign in make-release.sh,
# so it holds the signed bundle. COPYFILE_DISABLE keeps AppleDouble files out.
COPYFILE_DISABLE=1 tar -czf "$OUT/$TARBALL" -C "$(dirname "$APP")" "$(basename "$APP")"
pnpm tauri signer sign -f "$KEY" "$OUT/$TARBALL" >/dev/null

echo "==> verifying the signature against the public key in the app config"
(cd src-tauri && cargo run -q --example verify_update -- "$PUBKEY" "$OUT/$TARBALL" "$OUT/$TARBALL.sig")

NOTES="$(git tag -l --format='%(contents)' "$TAG")"
python3 - "$OUT" "$VERSION" "$NOTES" "$BASE_URL/$TARBALL" "$TARBALL" <<'PY'
import datetime, json, sys
out, version, notes, url, tarball = sys.argv[1:]
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": {
        "darwin-aarch64": {"signature": open(f"{out}/{tarball}.sig").read().strip(), "url": url}
    },
}
json.dump(manifest, open(f"{out}/latest.json", "w"), indent=2)
PY
rm "$OUT/$TARBALL.sig"
echo "==> built $OUT: $(ls "$OUT" | tr '\n' ' ')"

if [[ -z "${DICTATE_SITE_UPDATES_DIR:-}" ]]; then
  echo "==> DICTATE_SITE_UPDATES_DIR not set: copy $OUT/* into the site's updates folder and deploy the site."
  exit 0
fi
# Old builds go first so the folder holds one release; latest.json is copied
# last so no one is pointed at a file that has not landed yet.
mkdir -p "$DICTATE_SITE_UPDATES_DIR"
rm -f "$DICTATE_SITE_UPDATES_DIR"/dictate-*.dmg "$DICTATE_SITE_UPDATES_DIR"/dictate-*.app.tar.gz
cp "$OUT/$TARBALL" "$OUT/dictate-$VERSION-arm64.dmg" "$DICTATE_SITE_UPDATES_DIR/"
cp "$OUT/latest.json" "$DICTATE_SITE_UPDATES_DIR/"
echo "==> copied into $DICTATE_SITE_UPDATES_DIR"
if [[ -n "${DICTATE_SITE_DEPLOY:-}" ]]; then
  echo "==> deploying: $DICTATE_SITE_DEPLOY"
  bash -c "$DICTATE_SITE_DEPLOY"
else
  echo "==> deploy the site to publish it (DICTATE_SITE_DEPLOY runs a command for you)."
fi
