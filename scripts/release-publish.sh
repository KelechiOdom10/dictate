#!/usr/bin/env bash
# The one command: bump the version, tag it, build, sign, pack and publish.
#
#   pnpm release:publish patch        # 0.1.0 -> 0.1.1   (or minor | major | 1.2.3)
#   pnpm release:publish patch "notes shown in the update"
#
# Run it on main with a clean tree. A release is a tag: the build and the
# manifest both refuse to run from a commit that is not exactly vX.Y.Z.
# Needs DICTATE_SIGNING_IDENTITY (the certificate that keeps Microphone and
# Accessibility grants across updates) and TAURI_SIGNING_PRIVATE_KEY_PASSWORD.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
die() { echo "error: $*" >&2; exit 1; }

BUMP="${1:-}"
NOTES="${2:-}"
[[ -n "$BUMP" ]] || die "usage: pnpm release:publish <patch|minor|major|x.y.z> [notes]"
[[ -n "${DICTATE_SIGNING_IDENTITY:-}" ]] || die "set DICTATE_SIGNING_IDENTITY; an ad-hoc build would make every update re-ask for permissions"
git diff --quiet HEAD || die "the working tree has uncommitted changes"
[[ "$(git rev-parse --abbrev-ref HEAD)" == "main" ]] || die "release from main"

CURRENT="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
NEXT="$(python3 - "$CURRENT" "$BUMP" <<'PY'
import re, sys
current, bump = sys.argv[1:]
major, minor, patch = map(int, current.split("."))
if bump == "patch": patch += 1
elif bump == "minor": minor, patch = minor + 1, 0
elif bump == "major": major, minor, patch = major + 1, 0, 0
elif re.fullmatch(r"\d+\.\d+\.\d+", bump): major, minor, patch = map(int, bump.split("."))
else: sys.exit("bump must be patch, minor, major or x.y.z")
print(f"{major}.{minor}.{patch}")
PY
)"
git rev-parse -q --verify "refs/tags/v$NEXT" >/dev/null && die "tag v$NEXT already exists"
echo "==> $CURRENT -> $NEXT"

python3 - "$NEXT" <<'PY'
import json, re, sys
version = sys.argv[1]
for path in ("package.json", "src-tauri/tauri.conf.json"):
    text = open(path).read()
    text = re.sub(r'("version": ")[^"]+', lambda m: m.group(1) + version, text, count=1)
    open(path, "w").write(text)
text = open("src-tauri/Cargo.toml").read()
text = re.sub(r'(?m)^version = "[^"]+"', f'version = "{version}"', text, count=1)
open("src-tauri/Cargo.toml", "w").write(text)
PY
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
(cd src-tauri && cargo update -p dictate --offline >/dev/null)

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -q -m "release: v$NEXT"
git tag -a "v$NEXT" -m "${NOTES:-dictate $NEXT}"

DICTATE_PUBLISH_UPDATE=1 bash scripts/make-release.sh

echo
echo "==> v$NEXT is built and published. Push the commit and tag when you are happy:"
echo "    git push origin main v$NEXT"
