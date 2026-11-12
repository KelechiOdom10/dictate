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

## Conventions

- The three pipeline stages stay separate and independently callable. A stage you
  cannot call alone is a stage you cannot measure alone.
- Audio is 16 kHz mono f32 at every boundary. Convert once, at capture.
- Every latency or accuracy claim gets a number from this machine, in
  `docs/spike-results.md`. No numbers from blog posts.
- Temporary debug files go in `target/` or a temp dir, never the repo.
- No settings UI, no menu bar item, no signing, no auto-update until the spike
  gate passes.
