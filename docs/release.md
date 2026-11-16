# Releasing dictate

Source to a DMG the second Mac can install, with no paid Apple account and no
App Store. The background research, the alternatives that were rejected and the
verification behind each claim live in [distribution.md](distribution.md); this
file is the procedure.

Constraints that shaped it:

- **Apple Silicon only.** Parakeet runs on the GPU through Metal, and an Intel
  build would be far slower than the numbers in `spike-results.md`.
- **macOS 15 minimum.** ggml's Metal code declares `@available(macOS 15.0, *)`;
  below that the link step misses `___isPlatformVersionAtLeast`.
- **No notarisation.** That needs a paid Developer ID. A self-signed
  certificate gives a stable identity for the privacy database, and the second
  Mac still does the one-time Gatekeeper bypass in the README.
- **No auto-update.** Out of scope; it needs a public download location.

## The commands

```sh
pnpm install
pnpm icons                      # app icon set from the selected draft
pnpm release:mac                # -> release/dictate-<version>-arm64.dmg
```

`pnpm release:mac` runs `scripts/make-release.sh`, which drops any stale
`transcribe-cpp-sys` CMake build directory whose deployment target disagrees
with `minimumSystemVersion`, builds
`--target aarch64-apple-darwin --bundles app`, signs with the certificate when
`DICTATE_SIGNING_IDENTITY` is set, and packs the DMG with
`scripts/make-dmg.sh`.

With no identity it still signs, ad-hoc, with the bundle identifier, the
hardened runtime and the microphone entitlement. That is not cosmetic. Left
alone, the bundle keeps only the signature the linker put on the executable: no
`_CodeSignature` directory, no sealed resources, and a code identity of
`dictate-<hash of the binary>` instead of `com.kelechiodom.dictate`, so
`codesign --verify` fails with "code has no resources but signature indicates
they must be present" and the privacy panes list the app under that hash name.

Individual steps, when you want them alone:

| Command | What it does |
|---|---|
| `pnpm icons` | Rebuilds `src-tauri/icons/` from the selected draft |
| `pnpm icons:renders` | Renders every draft to `target/icon-renders/` for comparison |
| `DICTATE_SIGNING_IDENTITY="…" pnpm sign:mac [app]` | Signs an existing bundle, fails loudly with no identity |
| `pnpm dmg [app]` | Packs an existing bundle into `release/` |

CMake caches the first deployment target it sees, so after changing
`minimumSystemVersion` the build fails with
`error: 'path' is unavailable: introduced in macOS 10.15` until
`src-tauri/target/*/release/build/transcribe-cpp-sys-*` is deleted. The release
script does that every run.

## The icon

The drafts are in [`icon-drafts/`](icon-drafts/). Three Dock candidates (`a`,
`b`, `c`) and the menu-bar glyph, which is drawn in code
(`src-tauri/src/surface.rs`) rather than loaded from a file.

Swapping the shipped icon is one line: `SELECTED` in
`scripts/build-icons.mjs`. Then `pnpm icons` and `pnpm release:mac`.
`tauri icon` is not byte-deterministic, so regenerating an unchanged draft
rewrites the PNGs in `src-tauri/icons/` with identical pixels and different
bytes; commit them anyway, since the bundle builds from what is checked in.
`tauri icon` rasterises the SVG at 16, 32, 128, 256 and 512 pt at 1x and 2x and
packs them into `icon.icns`; `tauri.conf.json` already lists the ICNS and the
PNGs, so nothing else changes.

## Signing with a certificate you make yourself

This is free, and it is the difference between the second Mac re-granting
Microphone and Accessibility after every new build and granting them once.
Nothing in the repo creates the certificate: you make it in Keychain Access,
because it needs your login password.

**1. Create it.** Open **Keychain Access** (Spotlight, or Applications ›
Utilities). From the menu bar choose **Keychain Access › Certificate Assistant
› Create a Certificate…**

- Name: `dictate local signing`
- Identity Type: **Self Signed Root**
- Certificate Type: **Code Signing**

Click **Create**, then **Done**. Leave "Let me override defaults" unchecked.

**2. Trust it for code signing.** In Keychain Access select **login** in the
sidebar, then **Certificates**, and double-click `dictate local signing`.
Expand **Trust**, set **When using this certificate** to **Always Trust**, and
close the window. macOS asks for your login password here.

Without this step the certificate exists but `codesign` will not see it, and
`security find-identity -v -p codesigning` reports zero valid identities.

**3. Check it is visible.** This should print one line ending in
`"dictate local signing"`:

```sh
security find-identity -v -p codesigning
```

**4. Sign every release with it.**

```sh
export DICTATE_SIGNING_IDENTITY="dictate local signing"
pnpm release:mac
```

`scripts/sign-macos.sh` refuses to run when the variable is empty, lists the
usable identities when the name does not match one, signs with the hardened
runtime and `src-tauri/Entitlements.plist` (which carries
`com.apple.security.device.audio-input`, required for the microphone once the
hardened runtime is on), verifies the signature, and prints the designated
requirement.

**5. Keep the certificate.** Export it once (right-click the certificate ›
Export Items, `.p12`) and keep the file. The designated requirement names the
certificate, so replacing it — a new Mac, a lost login keychain — makes every
granted permission stale and asks the second Mac to grant them again.

### What to expect after signing

`spctl -a -vv` still reports `rejected`. That is correct: Gatekeeper accepts
only Apple-issued signatures, so the second Mac clears it once with **Open
Anyway** (README). What signing changes is the designated requirement:

```sh
codesign -d -r- src-tauri/target/aarch64-apple-darwin/release/bundle/macos/dictate.app
```

With an ad-hoc signature it prints `cdhash H"…"`, which is different for every
build. With the certificate it prints
`identifier "com.kelechiodom.dictate" and certificate leaf = H"…"`, which stays
the same, which is why a Microphone or Accessibility grant should survive the
next build. That last step — install build two over build one with live grants
and dictate — has not been run on this machine; it needs a second build and a
real permission grant.

## Checking the DMG

```sh
hdiutil attach release/dictate-0.1.0-arm64.dmg -nobrowse -readonly
ls /Volumes/dictate                       # dictate.app and Applications
open /Volumes/dictate/dictate.app         # launches from the read-only image
hdiutil detach /Volumes/dictate
```

Do not leave two copies running: a packaged build and a `pnpm tauri dev` build
both register the same global shortcuts, and the second one loses.

## Sizes on this machine

Measured on 16 Nov 2026, macOS 27.0.1, Apple Silicon, rustc 1.99.1, with
`CMAKE_OSX_DEPLOYMENT_TARGET` at 15.0.

| Artifact | Size | Time |
|---|---|---|
| `--release` compile, empty `target/` | — | 3 min 12 s |
| rebuild after an icon or config change | — | 1 min 31 s |
| `dictate.app` | 17.28 MiB (17,688 KiB on disk) | |
| `Contents/MacOS/dictate` | 17.21 MiB (18,046,544 bytes) | |
| `dictate-0.1.0-arm64.dmg` | 7.26 MiB (7,612,487 bytes) | |

The app is arm64 only: `lipo -archs` prints `arm64`. Its Info.plist carries
`com.kelechiodom.dictate`, `LSMinimumSystemVersion 15.0` and the microphone
usage string. No model weights are in the bundle; Parakeet downloads 697 MiB
(731,357,568 bytes) to `~/Library/Application Support/com.kelechiodom.dictate/models/`
on first use.
