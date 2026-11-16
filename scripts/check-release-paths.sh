#!/usr/bin/env bash
# Checks that the release scripts find the app wherever cargo actually put it.
#
#   pnpm check:release-paths
#
# The bug this exists for: `make-release.sh` hardcoded
# `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app`, so
# with CARGO_TARGET_DIR set — a shared cache, which is how the rest of this
# machine builds — it compiled for seventeen minutes into that cache and then
# died at `lipo` because it was looking in the wrong directory. It never signed
# and never packed anything.
#
# Needs no build and runs in a second: a real bundle is copied into a scratch
# target directory and the packaging is run against it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"
# shellcheck disable=SC1091
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
# shellcheck source=target-dir.sh
source "$ROOT/scripts/target-dir.sh"

SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
FAILED=0

fail() {
  echo "  FAIL: $*" >&2
  FAILED=1
}
pass() {
  echo "  ok: $*"
}

BUNDLE="aarch64-apple-darwin/release/bundle/macos/dictate.app"

echo "resolving the target directory"
DEFAULT="$(unset CARGO_TARGET_DIR; dictate_app_path)"
expected="$ROOT/src-tauri/target/$BUNDLE"
if [[ "$DEFAULT" == "$expected" ]]; then
  pass "no CARGO_TARGET_DIR resolves to cargo's own target directory"
else
  fail "no CARGO_TARGET_DIR resolved to $DEFAULT, expected $expected"
fi

CACHE="$SCRATCH/cache"
moved="$(CARGO_TARGET_DIR="$CACHE" dictate_app_path)"
if [[ "$moved" == "$CACHE/$BUNDLE" ]]; then
  pass "CARGO_TARGET_DIR moves the bundle path"
else
  fail "CARGO_TARGET_DIR=$CACHE resolved to $moved"
fi

echo "failing loudly when the bundle is not there"
output="$(CARGO_TARGET_DIR="$CACHE" require_app "$moved" 2>&1)" && fail "require_app accepted a missing bundle"
if [[ "$output" == *"$CACHE"* ]]; then
  pass "the message names the path it looked at"
else
  fail "the message does not name $CACHE: $output"
fi

# The rest needs a bundle to copy. Any existing build will do; none is a skip,
# not a failure, so this still runs on a clean checkout.
DONOR="$expected"
if [[ ! -d "$DONOR" ]]; then
  DONOR="$(find "$ROOT/src-tauri/target" -maxdepth 6 -name dictate.app -type d -print -quit 2>/dev/null || true)"
fi
if [[ -z "${DONOR:-}" || ! -d "$DONOR" ]]; then
  echo "no built bundle on this machine, so packaging itself is skipped"
  exit "$FAILED"
fi

mkdir -p "$CACHE/aarch64-apple-darwin/release/bundle/macos"
ditto "$DONOR" "$CACHE/$BUNDLE"

echo "packaging from the shared cache"
DMG_DIR="$SCRATCH/dmg"
if CARGO_TARGET_DIR="$CACHE" DICTATE_RELEASE_DIR="$DMG_DIR" bash "$ROOT/scripts/make-dmg.sh" >"$SCRATCH/dmg.log" 2>&1; then
  if compgen -G "$DMG_DIR/*.dmg" >/dev/null; then
    pass "make-dmg.sh packed $(basename "$(compgen -G "$DMG_DIR/*.dmg" | head -1)") from CARGO_TARGET_DIR"
  else
    fail "make-dmg.sh exited 0 but wrote no image into $DMG_DIR"
  fi
else
  fail "make-dmg.sh failed: $(tail -3 "$SCRATCH/dmg.log")"
fi

echo "signing resolves the same bundle"
# With no identity set it must stop at the certificate check, not the app check:
# that proves it found the app in the moved directory.
if output="$(unset DICTATE_SIGNING_IDENTITY; CARGO_TARGET_DIR="$CACHE" bash "$ROOT/scripts/sign-macos.sh" 2>&1)"; then
  fail "sign-macos.sh exited 0 with no identity set"
elif [[ "$output" == *DICTATE_SIGNING_IDENTITY* ]]; then
  pass "sign-macos.sh found the bundle and stopped for the missing identity"
else
  fail "sign-macos.sh stopped for the wrong reason: $output"
fi

exit "$FAILED"
