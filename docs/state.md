# State

One screen. Updated when the next person (or session) would otherwise have to
reconstruct it. The plan that governs this repo is `docs/plan.md`.

## Where this is

Spike, session 1 of 5. The question is in `docs/spike.md`; the build order and the
failure you will hit at each step is `GUIDE.md`.

```
~/Projects/dictate        git repo, branch main, 3 commits
Rust 1.99.1               installed via rustup, not on PATH in a fresh shell
Tauri 2 + React 19 + TS   scaffolded by create-tauri-app, builds and runs
```

`pnpm tauri dev` launches with no window and no Dock icon. That is correct: the
window is the overlay and it appears only while the chord is held.

## What is built

| Piece | State |
|---|---|
| Overlay window (`focus:false`, `alwaysOnTop`, `skipTaskbar`, undecorated, hidden) | done |
| Activation policy `Accessory` on macOS | done |
| ⌥⇧Space registered via `tauri-plugin-global-shortcut` | done, registration succeeds |
| Show on `Pressed`, hide on `Released`, log both | done |
| `pipeline_status`, `record_start`, `record_stop`, `finish_dictation` commands | wired |
| `audio::start` / `audio::stop` | **stub — the next work** |
| `transcribe::transcribe` | stub |
| `insert::paste` | stub |
| React overlay UI | placeholder, adequate |

## The immediate next action

Run `pnpm tauri dev`, focus another app, hold ⌥⇧Space.

- Overlay appears **and disappears** on release → both events fire. Build
  hold-to-talk, then write `audio`.
- Overlay appears and **stays** → only `Pressed` fires. Move registration into
  Rust (`ShortcutEvent` carries `state`) or make it a toggle. Decide before
  session 2, because it changes what `audio` has to do.
- Nothing appears → check the terminal for the `[hotkey]` line before touching
  code.

Then session 2: add `cpal`, implement `audio::start`/`stop`, check the sample
count is ≈ duration × 16000. The stream must outlive the function or capture
silently stops — see the doc comment in `audio.rs`.

## Decisions already made, and why

| Decision | Why |
|---|---|
| Tauri, not SwiftUI or Electron | No Xcode on this machine; TS is where the leverage is; Electron's footprint would hurt a daily-use app |
| Paste (⌘V), not `AXUIElement` insertion | Works in Electron and browser fields; keeps Accessibility permission to keystroke simulation only |
| Shell out to `whisper-cli` first, `whisper-rs` later | Gives a latency baseline to beat in-process; the subprocess→library move is a deliberate before/after |
| Audio never crosses IPC | 10 s at 16 kHz is 160k floats as JSON. Rust keeps the buffer, IPC carries text |
| Bundle id `com.kelechiodom.dictate` | TCC keys Accessibility grants to bundle identity; changing it later silently revokes permission |

## Do not

- Rebuild footy-ai. It gets an eval and tracing pass, not a rewrite.
- Add settings UI, a menu bar item, signing, notarisation, or auto-update before
  the seven checks in `docs/spike.md` have numbers.
- Write the pipeline bodies on someone else's behalf. The point of this repo is
  that its owner can explain every line; a stage someone else wrote is a stage
  they cannot debug.
- Trust a latency or accuracy claim that did not come from this machine.
  `docs/spike-results.md` is where those numbers go, and it does not exist yet.
