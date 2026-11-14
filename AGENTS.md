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
