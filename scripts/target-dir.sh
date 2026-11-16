#!/usr/bin/env bash
# Where cargo actually writes, so nothing else has to assume.
#
# `CARGO_TARGET_DIR` moves the whole target directory, and every thread on this
# machine builds with it set to a shared cache. A script that hardcodes
# `src-tauri/target` therefore compiles for minutes and then reports the built app
# as missing, because the app went somewhere else. Ask cargo instead.
#
#   source "$(dirname "$0")/target-dir.sh"
#   APP="$(dictate_app_path)" || exit
#   require_app "$APP"
#
# Expects `ROOT` to be the repository root and cargo to be on PATH.

# The resolved target directory, honouring CARGO_TARGET_DIR and any
# `.cargo/config.toml`. Printed as an absolute path.
dictate_target_dir() {
  cargo metadata --manifest-path "$ROOT/src-tauri/Cargo.toml" --format-version 1 --no-deps |
    python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])'
}

# The arm64 release bundle inside that directory.
dictate_app_path() {
  local target
  target="$(dictate_target_dir)" || return 1
  printf '%s/aarch64-apple-darwin/release/bundle/macos/dictate.app\n' "$target"
}

# Fails loudly, naming the path it looked at, when a build left no bundle there.
require_app() {
  local app="$1"
  [[ -d "$app" ]] && return 0
  local target
  target="$(dictate_target_dir)" 2>/dev/null || target="<unresolved>"
  {
    echo "error: no app bundle at $app"
    echo "       cargo's target directory is $target"
    echo "       set CARGO_TARGET_DIR to move it; this script asks cargo rather than assuming"
    echo "       a successful build with nothing here means the resolution is wrong, not cargo"
  } >&2
  return 1
}
