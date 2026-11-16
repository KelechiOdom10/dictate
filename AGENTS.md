# dictate

Hold a key, speak, get text in whatever app has focus. A local-first
replacement for the dictation tool currently in use.

## Status

Spike. Read `docs/spike.md` for the question being answered and `GUIDE.md` for
the order to build it in. Nothing beyond the spike is in scope until its seven
checks have numbers.

## Commands

```sh
source "$HOME/.cargo/env"   # rust is installed but not on PATH in a fresh shell
pnpm install
pnpm tauri dev              # dev window + rust rebuild on save
pnpm build                  # frontend typecheck + bundle
cargo check --manifest-path src-tauri/Cargo.toml
```

## Layout

| Path | What lives there |
|---|---|
| `src/` | React UI. Overlay, settings later. No pipeline logic. |
| `src-tauri/src/lib.rs` | Plugin registration, the Tauri command surface |
| `src-tauri/src/audio.rs` | Capture. `start`/`stop`, 16 kHz mono f32 |
| `src-tauri/src/transcribe.rs` | Speech to text. Samples in, text out |
| `src-tauri/src/insert.rs` | Paste into the focused app, restore clipboard |
| `src-tauri/src/screen.rs` | Annotate screen recording. `start`/`stop`, one display to MP4 |
| `models/` | Whisper GGML weights. Never committed. |
| `docs/` | The spike question, and later the kickoff docs |

## Who writes what

Changed by the owner, Kelechi, on 14 Nov 2026, stated directly in chat: agents
build this app, including the three pipeline bodies (`audio`, `transcribe`,
`insert`). The goal is a polished dictation app in the style of Hex and Handy,
built with his engineering system.

Each pipeline stage lands in its own PR with a plain-English walkthrough of what
the code does and why, so the owner can debug it later. The owner merges.

Rejected alternative: the owner writes the three pipeline bodies himself. Dropped
because it was slowing the build down.

## Conventions

- New UI is built from `src/ui` components; every new or changed component ships
  with stories for its states. See `docs/design-system.md` for the component contracts.
- The three pipeline stages stay separate and independently callable. A stage you
  cannot call alone is a stage you cannot measure alone.
- Audio is 16 kHz mono f32 at every boundary. Convert once, at capture.
- Every latency or accuracy claim gets a number from this machine, in
  `docs/spike-results.md`. No numbers from blog posts.
- Temporary debug files go in `target/` or a temp dir, never the repo.
- UI is in scope (owner, 14 Nov 2026): menu bar item, settings window, recording
  overlay, history, model manager. Signing and auto-update are still out of scope
  until asked for.

<!-- ASTRYX:START -->
Astryx v0.7.16 · 174 components
CLI: run every command as `pnpm exec astryx <cmd>` (shown below as `astryx ...`).

SETUP (once, in app entry e.g. main.tsx) — without these, components render unstyled:
  import "@astryxdesign/core/reset.css";
  import "@astryxdesign/core/astryx.css";

WORKFLOW — start every page from a template, never from scratch:
1. `astryx build "<idea>"` — names the template to scaffold from and the parts it lacks.
2. `astryx template <name> <path>` — scaffold it; keep its frame, gap and padding; replace the content.
3. `astryx template <Block>` for parts it lacks; `astryx component <Name>` to read props before using one.
Changing an existing page? Keep it: skip step 2 and add blocks/components inside its sections.
Before you deliver a build, run `astryx doctor --built <dir>`; don't deliver it if it fails.

RULES:
- No <div> — components handle all layout and spacing.
- Read `astryx docs layout` before changing a template's frame.
- Dense data = rows (Table, List/Item), not Card-wrapped lists. Badge = counts only.
- Style with component props first, then tokens (var(--color-*|--spacing-*|--radius-*)). No raw hex/px. No StyleX/Tailwind compiler here — don't use xstyle/utility classes directly. Some templates use StyleX and need a compiler plugin; see `astryx docs styling-overview`.
- THEMES: palettes and custom colors go through the theme, never :root overrides. To make or change a theme (colors, fonts, corners, spacing, a look from a picture), prefer the experimental theme editor over hand-written theme files: `astryx experimental theme editor new <name> --like neutral`, then `editor set <name> <thing> <value>` or `editor how "<what you want>"`; wire it with `astryx theme add <name>`; after each change look at it with `astryx experimental theme editor show <name> --png` and open the PNGs it writes (changes nothing in the app); a theme change is not done until you have looked at the pictures. In code, extend an imported theme to customize it; `theme eject <slug>` only forks source. See `astryx docs theme`.
- SELF-CHECK: re-read the file; replace any raw <div>/<span>, imported .css/@apply, or hardcoded #hex/px with the component or a token.

`astryx help` lists every command. Key ones beyond the workflow:
  search "<query>"   find any component / hook / doc / template
  discover <words>   integrations you could add
  docs <topic>       getting-started, principles, tokens, theme … (`astryx docs` lists all)
  docs cli           commands and API reference
  docs authoring     build an integration package, and every field it holds
  upgrade --from <old version> --apply   run after a dependency bump
<!-- ASTRYX:END -->
