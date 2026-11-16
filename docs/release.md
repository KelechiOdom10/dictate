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
- **Updates are automatic** once the one-time setup under [Automatic updates](#automatic-updates) is done.

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
| `pnpm check:release-paths` | Proves the scripts find the bundle under `CARGO_TARGET_DIR` |

**`CARGO_TARGET_DIR` is honoured.** The scripts ask cargo for its target
directory (`scripts/target-dir.sh`, via `cargo metadata`) instead of assuming
`src-tauri/target`, and `require_app` fails loudly naming the path it looked at.
This matters because `CARGO_TARGET_DIR` is how the rest of this machine builds —
[docs/qa-round-3.md](qa-round-3.md) sets it to the owner's checkout. With a
shared cache the old hardcoded path compiled for seventeen minutes and then died
at `lipo` with "can't open input file", signing and packing nothing. `pnpm check:release-paths` covers exactly that case, and a
full run with `CARGO_TARGET_DIR` pointing at a moved cache now builds, signs,
verifies and packs.

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

### The app icon follows the theme, and where that is visible

`src-tauri/src/app_icon.rs` draws the same artboard as draft A from the live
palette and hands it to AppKit at launch and whenever Theme or Mode changes. The
plate is the palette's `canvas` and the four outer bars its `text`; the middle
bar — the live one — carries the theme's **identity colour**.

The rule, in one sentence: **the icon paints the theme's identity colour, and
the focus ring is only its contrast-safe variant, which is why the two coincide
for four of the five themes.**

| Theme | Light | Dark | Where it comes from |
|---|---|---|---|
| Ember | `#df5822` | `#f67d51` | the `live` role in `src/ui/palettes.generated.json` |
| Paper | `#1c1b19` | `#ababab` | its focus ring, which is its ink |
| Capy | `#8372d3` | `#a99cf5` | its focus ring, lavender |
| Rosé Pine | `#966dc3` | `#c4a7e7` | its focus ring, iris |
| Neutral | `#71717a` | `#71717a` | its focus ring, grey |

Ember is the exception and stays one. Its focus ring is warm brown `#5c544b`
because a focus ring has to survive against surfaces, and draft A was chosen for
five ink bars with the middle one in ember; painting the brown there would swap
dictate's identity for a muted grey. Its icon therefore uses the same live orange
the app already shows while recording. The other four themes have no separate
identity colour, so their ring is it — and Paper's ring is its ink, so a Paper
icon stays near-monochrome. `System` resolves through `NSApp.effectiveAppearance`;
an OS appearance switch underneath it while the app runs does not redraw the icon.

`scripts/check-icon-colours.mjs` holds each half of that table to its own source —
Ember to the overlay palette's `live` role, the other four to their stylesheets —
and `pnpm release:mac` runs it before it builds, so neither a re-dialled theme nor
a re-tuned overlay can ship beside a stale icon. `pnpm check:icon-colours` runs
it alone.

It goes through AppKit rather than Tauri because Tauri 2 has no app-level icon
setter: `tauri::AppHandle` exposes window, webview, tray and menu icons only.
`NSApplication.setApplicationIconImage` (`objc2-app-kit`) is the mechanism.

Three limits worth knowing before anyone builds on this:

- **This app has no Dock tile.** It runs with `ActivationPolicy::Accessory`, so
  it is absent from the Dock and from the app switcher. The themed icon is
  therefore prepared rather than seen: it becomes visible if the app ever takes
  a Regular activation policy.
- **Finder, Launchpad and Get Info do not follow it.** They read the `.icns`
  inside the bundle, and the only way to change that after installation is to
  rewrite the bundle, which invalidates the signature. They keep showing
  whichever draft was built in, until a new build replaces it.
- **The menu-bar glyph is not themed on purpose.** It is a template image and
  macOS tints templates for light and dark menu bars itself; giving it a colour
  would replace that correct behaviour with a fixed one. Do not "fix" it. The
  artwork is drawn in `src-tauri/src/surface.rs` and stays monochrome black
  plus alpha.

`cargo test --lib app_icon` covers the accent each theme produces and that a
theme change calls the setter. `cargo run --example themed_icons --
target/themed-icons` writes the four PNGs, which is the only way to look at
them without a launch. The on-screen result has not been seen on this machine.
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
`identifier "com.kelechiodom.dictate" and certificate leaf = H"…"`, which names
the certificate instead of the build.

Run on 16 Nov 2026 with a real certificate, `dictate local signing`, leaf
`f7a2e7d3…`:

- `pnpm release:mac` with `DICTATE_SIGNING_IDENTITY` set signs, then verifies:
  `valid on disk`, `satisfies its Designated Requirement`, `origin=dictate local
  signing`, and `spctl` still says `rejected` as above.
- `codesign -d --entitlements -` shows `com.apple.security.device.audio-input`.
- The designated requirement is
  `identifier "com.kelechiodom.dictate" and certificate leaf = H"f7a2e7d3…"`,
  and two bundles with **different contents** — one signed with an extra
  resource file and therefore a different `cdhash` — carry the identical
  requirement. That is the mechanism by which a grant is expected to survive an
  update.

Still unmeasured: installing build two over build one with live Microphone and
Accessibility grants and dictating again. Everything up to that step is now
observed; the last step needs a second Mac or a wiped permission state.

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
| `dictate.app` | 17.59 MiB (18,012 KiB on disk) | |
| `Contents/MacOS/dictate` | 17.53 MiB (18,376,784 bytes) | |
| `dictate-0.1.0-arm64.dmg` | 7.56 MiB (7,927,596 bytes), signed | |

The app is arm64 only: `lipo -archs` prints `arm64`. Its Info.plist carries
`com.kelechiodom.dictate`, `LSMinimumSystemVersion 15.0` and the microphone
usage string. No model weights are in the bundle; Parakeet downloads 697 MiB
(731,357,568 bytes) to `~/Library/Application Support/com.kelechiodom.dictate/models/`
on first use.

## Automatic updates

The second Mac gets new builds without doing anything. On launch (after ten
seconds) and on **Settings › About › Check for updates**, dictate fetches
`https://dictate.kellycooks.dev/updates/latest.json`. A newer version is
downloaded, checked against the update public key, and installed with a
relaunch, but only when no dictation take and no Annotate session is active.
Until then Settings says so: "0.2.0 is downloaded. It installs and restarts
dictate when you aren't dictating or using Annotate." The install holds the
app's runtime lock, so a shortcut press cannot start a take halfway through it.

It fails soft. If the endpoint is unreachable, times out (15 s) or returns
something invalid, a launch check logs a warning and shows nothing; only a
check you asked for shows the error. Development builds (`pnpm tauri dev`)
never check, so a running dev server cannot replace itself.

### One-time setup (you, once)

1. **Make the update key.** This is a minisign key pair, separate from the
   code-signing certificate. It only proves an update came from you.

   ```sh
   pnpm update:key
   ```

   It prompts for a password, writes the private key to
   `~/.config/dictate/update.key` (outside the repo; override with
   `DICTATE_UPDATE_KEY`) and puts the public key in
   `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`. Commit that
   one change. The repo ships with a placeholder there, so until you do this
   the app's update check fails soft with an error and the publish scripts
   refuse to run.

   Back up the private key file and the password somewhere that is not this
   Mac. The public key is baked into every installed build: lose either half
   and no installed copy can ever accept an update, and the second Mac has to
   reinstall from a DMG.

2. **Tell the site where `/updates/` lives.** The site project is separate
   from this repo, so point at its folder:

   ```sh
   export DICTATE_SITE_UPDATES_DIR=~/code/dictate-site/public/updates
   export DICTATE_SITE_DEPLOY='cd ~/code/dictate-site && pnpm exec wrangler pages deploy public'   # optional
   ```

   With no deploy command, the script copies the files and tells you to deploy
   the site. The site must ship its whole folder each time (a Pages deploy
   replaces the previous one), which is why the script copies into the site
   project rather than uploading `/updates/` alone. dictate never holds Cloudflare
   credentials; `wrangler` uses your own login.

### Releasing (every time)

```sh
export DICTATE_SIGNING_IDENTITY="dictate local signing"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='…'     # the update key's password
pnpm release:publish patch "What changed, one line"   # or minor | major | 1.2.3
```

Run it on a clean `main`. It bumps the version (`package.json`,
`tauri.conf.json`, `Cargo.toml`, `Cargo.lock`), commits `release: vX.Y.Z`,
creates the annotated tag `vX.Y.Z` (the message becomes the update notes),
runs `pnpm release:mac` (build, certificate re-sign, DMG), builds and signs the
updater tarball, verifies that signature against the public key in the config,
writes `latest.json`, and copies everything into the site's `updates/`
folder. When it finishes, push `git push origin main vX.Y.Z`.

**Why a tag, and why it runs here.** A release is a tag, not a push: building
on every push to `main` would publish a release per commit and rewrite what
everyone is running. And it runs on this Mac, not in CI, because the build has
to be signed by the self-signed certificate in your keychain; a CI runner would
need that certificate exported into a secret. `scripts/publish-update.sh`
enforces the tag: it refuses unless `HEAD` is exactly `vX.Y.Z` for the
built app's version and the tree is clean. If the upload step fails, fix it
and run `scripts/publish-update.sh` again; no new bump is needed.

What lands in `/updates/`, one release at a time:

| File | For |
|---|---|
| `latest.json` | The manifest the app reads: version, notes, URL, signature |
| `dictate-X.Y.Z-arm64.app.tar.gz` | What the updater installs |
| `dictate-X.Y.Z-arm64.dmg` | A first install, or a manual reinstall |

Tauri's updater on macOS installs a `.app.tar.gz`, not a DMG, so both are
served. The tarball is built after the certificate re-sign in
`make-release.sh`, which is why it carries the signed bundle. (Tauri's own
`createUpdaterArtifacts` would tar the app before that re-sign, so it is not
used.)

### What this does and does not give you

- **The update signature is independent of notarisation.** It is checked inside
  the app, so an update installs with no Gatekeeper prompt. The first install
  from the DMG still needs **Open Anyway** (README), because the app is not
  notarised.
- **Microphone, Accessibility and Screen Recording permissions survive
  updates, as long as the signing identity does.** The bundle identifier
  stays `com.kelechiodom.dictate` and every release is signed with the same
  self-signed certificate, so the designated requirement is unchanged. That is
  exactly why the certificate matters: an ad-hoc build has a new identity each
  time and would ask again after every update. `release:publish` refuses to run
  without `DICTATE_SIGNING_IDENTITY`. Lose the certificate (see step 5 above)
  and the second Mac re-grants once. This is expected from how TCC works, and
  has not been observed here yet.
- **Anyone who can write to `/updates/` still needs your private key** to
  produce an update the app accepts. A tampered download or a hijacked host
  cannot install.
- **Rollback is a re-release.** There is no downgrade; publish a higher version.

### What is proven, and what needs two machines

Proven on this Mac:

- `cargo test --lib update::`: install is allowed only when dictation is idle
  and Annotate is idle (every busy phase of each blocks it), and the wait loop
  runs the install exactly once after a busy stretch.
- `cargo test --test updater`: a local HTTP endpoint serves a well-formed
  manifest, its artifact verifies against the public key, and a corrupted
  signature, a tampered payload and a signature from another key are all
  rejected (the same minisign check the updater plugin runs).
- `scripts/publish-update.sh` end to end against a stand-in bundle with a
  throwaway key: tag guard, tarball, signature, self-check, manifest, copy.

Not provable here, needs the second Mac and a real `0.1.x` to `0.1.y` pair:

- The download, install and relaunch through the real plugin against the live
  site, and that a build installed from the DMG then updates itself.
- That the Microphone and Accessibility grants are still there afterwards.
- The deferral in the running app: start dictating or Annotate while an update
  is waiting and see that it installs only afterwards.
- Cloudflare serving `/updates/latest.json` and the tarball with the right
  content types.
