# State

One screen. Updated when the next person (or session) would otherwise have to
reconstruct it. The plan that governs this repo is `docs/plan.md`.

**If you are a new agent picking this up cold, read `docs/handoff.md` first.**
This file says where things are; that one says why, who writes what, and what
must not change.

## Where this is

Spike; the three pipeline stages are implemented on the PR stack. The question
is in `docs/spike.md`; build order and likely failures are in `GUIDE.md`.

```
~/Projects/dictate        git repo; pipeline PR stack in progress
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
| `audio::start` / `audio::stop` | implemented; direct mic probe passed, hotkey / listening check pending |
| `transcribe::transcribe` | implemented in dependent PR #2; generated-speech probe passed, owner speech pending |
| `insert::paste` | implemented; isolated clipboard tests pass, three-app hotkey checks pending |
| React overlay UI | placeholder, adequate |

## The interaction model, decided

⌥⇧Space delivers **both** `Pressed` and `Released` on this machine (13 Nov 2026),
so this is hold-to-talk: capture opens on key-down to keep the first phoneme,
the buffer drains on key-up. Not a toggle. The alternative in `GUIDE.md` — move
registration into Rust — never applied; the handler has always been in Rust.

## The immediate next action

On the pipeline PR stack, run the manual paste
and clipboard checks in `docs/spike-results.md`. Accessibility denial, overlay
focus, TextEdit, VS Code and browser insertion remain unverified in this run.
The release handler now transcribes, posts Command-V, restores the clipboard,
and logs `release_to_paste_complete_ms`, including its provisional 300 ms wait.
This branch does not close the seven-check gate.

The original capture-stage handoff follows; its physical checks remain open.

Close check 1 in one minute: with the overlay up, type into the app you came
from and confirm the characters land there. Show/hide proves the event plumbing;
focus staying put is the actual pass condition in `docs/spike.md`, and it is the
one that decides whether paste can work at all.

Session 2 capture is implemented with a thread-owned stream, native-format
conversion, mono downmix and filtered resampling to 16 kHz. `Released` logs
the sample count. The direct four-second microphone probe returned 64,170 samples (see
`docs/spike-results.md`). The remaining manual check is ≈ duration × 16000 in the
terminal, plus listening for the first phoneme.

Two capture invariants implemented in `audio.rs`:
the stream must be stored somewhere that outlives the function or capture stops
silently, and the device rate may differ from 16 kHz, so resample rather than decimate.
A third: make `start()` idempotent. If a held key auto-repeats `Pressed`, a
second `start()` must not reset the buffer mid-sentence.

For an audible capture, launch with `DICTATE_DEBUG_WAV=1 pnpm tauri dev`.
Each completed take logs a unique WAV path in the system temporary directory.
Open it to check first/last phonemes, then delete it. Dumps are opt-in, persist
until deleted, and contain your recording.

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
- Land pipeline stages together. The owner authorized agents to build them
  separately, each with a plain-English walkthrough; see `AGENTS.md`.
- Trust a latency or accuracy claim that did not come from this machine.
  `docs/spike-results.md` holds the measurements so far; unmeasured checks stay open.
