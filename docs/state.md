# State

One screen. Updated when the next person (or session) would otherwise have to
reconstruct it. The plan that governs this repo is `docs/plan.md`.

**If you are a new agent picking this up cold, read `docs/handoff.md` first.**
This file says where things are; that one says why, who writes what, and what
must not change.

## Where this is

Spike, session 1 of 5. The question is in `docs/spike.md`; the build order and the
failure you will hit at each step is `GUIDE.md`.

```
~/Projects/dictate        git repo, branch main, 4 commits
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
| Show on `Pressed`, hide on `Released`, log both | done, both events fire |
| `pipeline_status`, `record_start`, `record_stop`, `finish_dictation` commands | wired |
| `audio::start` / `audio::stop` | **stub — the next work** |
| `transcribe::transcribe` | stub |
| `insert::paste` | stub |
| React overlay UI | placeholder, adequate |

## The interaction model, decided

⌥⇧Space delivers **both** `Pressed` and `Released` on this machine (13 Nov 2026),
so this is hold-to-talk: capture opens on key-down to keep the first phoneme,
the buffer drains on key-up. Not a toggle. The alternative in `GUIDE.md` — move
registration into Rust — never applied; the handler has always been in Rust.

## The immediate next action

Close check 1 in one minute: with the overlay up, type into the app you came
from and confirm the characters land there. Show/hide proves the event plumbing;
focus staying put is the actual pass condition in `docs/spike.md`, and it is the
one that decides whether paste can work at all.

Then session 2: add `cpal`, implement `audio::start`/`stop`. `Released` now logs
the sample count, so the check reads itself: ≈ duration × 16000 in the terminal.

Two things that will cost you time, both already in the `audio.rs` doc comment:
the stream must be stored somewhere that outlives the function or capture stops
silently, and the device rate is 44.1/48 kHz, so resample rather than decimate.
A third: make `start()` idempotent. If a held key auto-repeats `Pressed`, a
second `start()` must not reset the buffer mid-sentence.

Do session 2 before anything else. It is the session that decides whether the
rest of the spike is measuring a microphone or blaming a model.

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
- Add signing, notarisation or auto-update until the owner asks. (Settings UI and
  the menu bar item are in scope as of 14 Nov 2026.)
- Write the pipeline bodies on someone else's behalf. The point of this repo is
  that its owner can explain every line; a stage someone else wrote is a stage
  they cannot debug.
- Trust a latency or accuracy claim that did not come from this machine.
  `docs/spike-results.md` is where those numbers go, and it does not exist yet.
