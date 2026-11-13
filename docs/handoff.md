# Handoff: dictate, and the programme it belongs to

Written 13 Nov 2026, at the end of spike session 1. This file is self-contained:
a new agent with no access to the previous conversation can act on it. If you
*do* have the repo, the other files named here are the depth behind each
section. If you do not, everything load-bearing is below.

`docs/state.md` is the living status file and outranks this one for *where
things are now*. This file outranks it for *why*, and for what must not change.

---

## 1. Your role

You are the guide on this project. Not the implementer.

The owner writes the three pipeline bodies himself: `audio`, `transcribe`,
`insert`. You scaffold around them, explain mechanisms, review, debug, write
docs, config, wiring and tests.

This is not a stylistic preference. The repo exists so that he can explain every
line of the pipeline; a stage written by someone else is a stage he cannot debug,
and a dictation app that fails silently on his own machine is the exact failure
mode this project was chosen to avoid. If you write `audio::start()` for him, you
have destroyed the deliverable while appearing to accelerate it.

When he asks you to implement one of those three, your job is to ask what he
expects to happen, then help him find the bug in what he wrote.

---

## 2. What this is

`dictate`: a local-first replacement for the dictation tool (Hex) he already
pays for. Hold a key, speak, release, transcript lands in whichever app had
focus. Tauri 2, Rust core, React overlay, whisper.cpp. Nothing leaves the
machine, so cost per minute is zero.

The pipeline is three contracts, already stubbed and wired:

```
audio::start() -> audio::stop() -> Vec<f32>   // 16 kHz mono, in memory
transcribe::transcribe(&[f32]) -> String
insert::paste(app, &str) -> ()
```

Signatures exist so no stage can be built in a way that is only testable end to
end. Keep them separately callable.

### Why this repo exists

It is phase 1 of a 16-week programme (section 7), not a product. Its teaching
subject is **latency**, plus macOS permissions and text insertion. It is
deliberately allowed to teach nothing about AI: no model API, no evals, no
retrieval. The AI phases come later and reuse none of this code.

The behavioural gate is not "the app works". It is: five consecutive days of
real use in place of Hex, p95 key-release-to-text under 1.5 s measured on this
machine, and word error rate measured on twenty of his own recordings.

---

## 3. State, as of 13 Nov 2026

| Piece | State |
|---|---|
| Repo | `~/Projects/dictate`, branch `main`, 4+ commits, clean tree |
| Toolchain | Rust 1.99.1 via rustup (not on PATH in a fresh shell), Tauri 2, React 19, TS |
| Overlay window | done — 460×132, `focus:false`, `alwaysOnTop`, `skipTaskbar`, undecorated, hidden by default |
| Activation policy | done — `Accessory`, so the app never becomes active and never steals focus |
| Global chord | done — ⌥⇧Space, registered in Rust, handler logs `[hotkey] … state=…` |
| Show/hide on Pressed/Released | done, and **both events verified firing** |
| `pipeline_status`, `record_start`, `record_stop`, `finish_dictation` | commands wired |
| `audio::start` / `audio::stop` | **stub — session 2, the next work** |
| `transcribe::transcribe` | stub |
| `insert::paste` | stub |
| React overlay UI | placeholder, adequate for the spike |
| `docs/spike-results.md` | does not exist yet; created in session 5 |

Spike sessions 1 of 5 are done. Sessions 2–5 are audio, transcription, paste, and
measurement.

---

## 4. The decision made this session, with its evidence

The interaction model was the fork that had to be settled before audio could be
designed. Answer: **hold-to-talk**.

Evidence: with another app focused, holding ⌥⇧Space showed the overlay and
releasing hid it, and the handler logged both `Pressed` and `Released`. So
key-down and key-up are both delivered on this machine and this plugin version.

Consequences, in order of importance:

1. Capture opens on **key-down**, not on model load or on release. This is the
   difference between losing the first phoneme and not; if the first phoneme is
   missing he will spend a week blaming the model.
2. `audio::stop()` drains the buffer on key-up and the buffer does not need to
   survive an arbitrary gap. A toggle would have needed that.
3. An alternative that circulates in the guide — "move registration into Rust
   because `ShortcutEvent` carries state" — never applied. Registration was in
   Rust from the start; the JS binding was never used.
4. To make session 2's check observable, the `Released` handler now logs
   `[hotkey] N samples (X.XXs @ 16 kHz)` instead of discarding the drained
   buffer. Without it `stop()` emptied the buffer and a later `record_stop`
   returned 0, which is indistinguishable from a dead microphone.

Do not relitigate hold-to-talk. If a future session finds the release event
unreliable, that is a bug to investigate, not a licence to switch to toggle
without measuring what broke.

---

## 5. Next action, in order

### 5a. One minute, closes check 1

With the overlay up, type into the app he came from. Characters must land there,
not in the overlay. Show/hide proves the event plumbing; focus staying put is
the actual pass condition, and paste inherits the same bug if it is wrong.

### 5b. Session 2 — audio capture (~3 h)

Add `cpal`, implement `audio::start` / `audio::stop`. He writes the body.

The three liabilities, in the order they usually bite:

1. **Stream lifetime.** A `cpal::Stream` held in a local variable stops capture
   the instant `start()` returns, with no error. Store it in a `static`
   (`OnceLock`/`Mutex`) or in Tauri managed state. Symptom: `stop()` returns an
   empty vec.
2. **Sample format.** Use `build_input_stream_raw`; handle `F32` and `I16`
   (`s as f32 / 32768.0`). Built-in Mac mics are f32, USB interfaces often are
   not.
3. **Device rate.** 44.1 or 48 kHz, not 16 kHz. Resample with `rubato`. Naive
   decimation aliases, and aliased speech sounds fine to a human and transcribes
   badly, which sends the debugging into the model.

A fourth, found in this thread and not in any guide: make `start()` idempotent.
If a held key auto-repeats `Pressed`, a second `start()` must not reset the
buffer mid-sentence.

Check: hold the chord ~4 s, expect ≈ duration × 16000 samples in the terminal
and the first word not clipped. Then dump a wav with `hound` and listen; worth
ten minutes is a per-100 ms RMS print, because flat zero means a silent device
and you want that known before transcription exists.

**Gate before the run** (see section 8): he states the expected sample count and
which liability he expects to bite. If he runs first and predicts after, the
measurement teaches nothing.

### 5c. Sessions 3–5

`GUIDE.md` has each one with its failure mode. Do not skip ahead:
transcription, then paste (TCC/Accessibility bit), then the measurement pass
that writes `docs/spike-results.md` with seven rows, each pass/fail and each
with a number from this machine.

---

## 6. The gate: seven checks in `docs/spike.md`

Nothing outside these is in scope. Each must end with a number, not an
impression.

| # | Check | Pass condition |
|---|---|---|
| 1 | Hotkey fires while another app is focused, without stealing focus | Overlay appears, cursor stays in the target app |
| 2 | Audio actually captured | Buffer ≈ duration × 16000, first phoneme present |
| 3 | Transcription correct | One spoken sentence, word-perfect |
| 4 | Paste lands in the focused field | TextEdit, VS Code, and a browser input |
| 5 | Clipboard restored | What he copied before is still on the clipboard after |
| 6 | Latency acceptable | p95 key-release → text present, under 1.5 s, ≥20 dictations |
| 7 | Accuracy measured | WER on 20 of his own recordings |

Checks 6 and 7 are the ones people skip. If they are skipped, the spike has
failed even if the app works.

Decision the spike produces: all seven pass → build the rest and the product doc
is worth writing. 1–4 pass and 6 fails → still viable, but the design changes to
streaming partial results, which changes what `audio` must do. 1 or 4 fails →
stop and rethink the interaction before writing code that would have to be kept.

---

## 7. The programme this belongs to

Four phases, one system, 16 weeks, ~268 hours (228 build, 40 theory), at
15–20 hours/week. $0 for weeks 1–9; $5–10/month for the last phase. No free
always-on host exists in late 2026, which is why nothing here is hosted.

| Phase | Weeks | Hours | Teaching subject | Not |
|---|---|---|---|---|
| **Hex/dictate** | 1–2 | 34 | macOS permissions, hotkey without focus theft, insertion, latency, WER | not agent engineering |
| **footy-ai upgrade** | 3 | 18 | behavioural evals, tracing, routing, prompt versioning | not a rebuild — the hard unknown is already solved there |
| **Dash** | 4–9 | ~103 | retrieval quality, ACL-aware permissions, citations, cost | capped at three connectors, permanently |
| **Jarvis** | 10–16 | ~113 | identity resolution, memory, approval authority, classifier calibration | not a new ingestion stack; it reuses Dash's spine |

Later repos do not exist yet. **This repo holds the governing plan**
(`docs/plan.md`). Do not create the later repos early, and do not let this repo
grow into one of them.

Cadence that makes it followable: a 30-min Monday planning pass that writes the
week's gate; three 2-hour weekday evenings; one 5-hour Saturday block for the
hard part; a 1-hour Sunday pass that runs the eval suite and writes three
paragraphs on what broke. A week that produces no artifact was consumption.

---

## 8. Five gates that outrank shipping

These are the reason the project is worth doing. They are cheap to abandon and
expensive to lose.

| Gate | When | Fails if |
|---|---|---|
| Predict before measuring | Before every measurement | He cannot state the expected number and the mechanism that causes it |
| Hand-roll, then framework, then diff | Each layer, once | A framework was adopted without replacing one first |
| Teach-back | End of each phase | He needs a library or product name to explain the mechanism |
| Rejection log | Every non-trivial decision | He cannot name the alternative he rejected and why |
| Hard stop | At each phase's hour budget | Building continued past the budget to avoid explaining it |

The teach-back for this phase already has its five questions in `GUIDE.md`:
why 16 kHz mono, why the stream must outlive the function, why paste beats
element insertion, what aliasing does to a transcript, why a synthetic keystroke
needs a permission a normal keystroke does not. Eight hundred words, mechanism
only, no crate names.

---

## 9. What must not happen

- Do not write the pipeline bodies. Restated because it is the rule that will be
  violated first.
- Do not add a settings UI, menu bar item, signing, notarisation, or
  auto-update before the seven checks have numbers.
- Do not claim a latency or accuracy number that did not come from this machine.
  No numbers from blog posts, model cards, or other people's benchmarks.
- Do not change the bundle identifier (`com.kelechiodom.dictate`). macOS keys
  Accessibility grants to bundle identity; changing it silently revokes
  permission and looks like a code bug.
- Do not add a fourth pipeline stage, a cloud fallback, or a second language.
- Do not rewrite `docs/plan.md` into a shipping roadmap. If the plan is revised,
  the revision is a dated section appended to it, with the reason, and the
  rejection log keeps the old version. A learning programme that becomes a
  shipping plan has quietly lost its only asset.
- Do not move the programme's later phases forward because this phase is
  frustrating. Frustration is the curriculum; see the hard stop.

---

## 10. Working with him

Senior TypeScript engineer, five years shipping, London. Existing `footy-ai` repo
already does NL → typed query plan → validated SQL with a Zod-constrained tool
schema and a custom test double for the model. Do not explain basics, do not
define what a mutex is, do not add encouraging framing to technical claims.

What he responds to: mechanism, the specific failure and why it happens, and a
decision with its evidence and its rejected alternative. What he will discard:
checklists that treat him as a beginner, hedged advice with no number, and any
suggestion that adds scope before the current gate passes.

He has explicitly asked to be guided rather than driven. Hold the line on
sequence even when he is enthusiastic about the next phase; that is the job.

---

## 11. Where things live

```
docs/state.md        living status: where things are now, and the immediate action
docs/spike.md        the question and the seven checks. The only scope that counts.
GUIDE.md             build order, session by session, each with its likely failure
docs/plan.md         the governing 16-week programme; this repo is its phase 1
docs/handoff.md      this file
docs/spike-results.md  created in session 5; seven rows, numbers, mechanisms

src-tauri/src/lib.rs       plugin registration, window wiring, command surface
src-tauri/src/audio.rs     capture contract + stub (his to write)
src-tauri/src/transcribe.rs  STT contract + stub (his to write)
src-tauri/src/insert.rs    paste contract + stub (his to write)
src/                       React overlay. No pipeline logic.
models/                    whisper GGML weights, never committed
```

Commands:

```sh
source "$HOME/.cargo/env"    # rust is installed but not on PATH in a fresh shell
cd ~/Projects/dictate
pnpm install
pnpm tauri dev               # overlay app; nothing visible until the chord is held
cargo check --manifest-path src-tauri/Cargo.toml
```

`pnpm tauri dev` launching with no window and no Dock icon is correct, not a bug.

---

## 12. Session log (append-only)

**13 Nov 2026 — session 1 close.** ⌥⇧Space delivers both Pressed and Released;
hold-to-talk confirmed by observation, not assumption. `Released` now logs the
drained sample count so session 2's check is readable from the terminal.
`docs/state.md` corrected (commit count, decision, next action). Check 1 still
needs the typing test in 5a. Session 2 not started.

---

## 13. Loose ends and known risks

- **Check 1 is unproven on focus.** Overlay behaviour is proven; "cursor stays in
  the target app" is not yet observed. One minute of work, do it before audio.
- **The prediction for session 2 is outstanding.** He has not yet stated the
  expected sample count or which liability he expects. Ask before the first run.
- **TCC will bite in session 4.** A dev build needs Accessibility permission and
  a synthetic keystroke from an untrusted app is dropped silently with no error.
  Permission attaches to the app, not the terminal, and may need re-granting
  after rebuilds until signing is consistent. This is expected, not a bug in
  `insert`.
- **The Whisper model choice is unmeasured.** `ggml-base.en.bin` (~142 MB) first,
  then `ggml-small.en.bin`, and both numbers get written down. Session 3.
- **This handoff assumes a new agent may not see the previous chat.** If the
  receiving bot cannot read files, the owner must paste the contents of this file
  plus `docs/spike.md`; do not paraphrase them from memory.
- **Do not trust the hour estimates as measurements.** They are design judgement
  from `docs/plan.md`, and the plan says so itself.
