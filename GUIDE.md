# Guide: the spike, in order

Read `docs/spike.md` first. This is the how.

Each session has one goal, one check, and the failure you are most likely to hit.
Do them in order and do not start the next one until the check passes. The whole
thing is roughly two working days, split into six sittings.

The three contracts are already stubbed and wired into `lib.rs`:

```
audio::start() -> audio::stop() -> Vec<f32>   // 16 kHz mono, in memory
transcribe::transcribe(&[f32])  -> String
insert::paste(app, &str)        -> ()
```

You write the bodies. The signatures exist so you cannot accidentally build a
pipeline that only works end to end and can never be measured in parts.

---

## Session 0 — prove the toolchain (30 min)

```sh
source "$HOME/.cargo/env"
cd ~/Projects/dictate
pnpm install
pnpm tauri dev
```

**Check:** a window opens, `cargo` compiles ~300 crates the first time, and
saving `src/App.tsx` hot-reloads.

**Failure you will hit:** `error: linker command failed` or a missing
`xcrun`. Tauri on macOS needs the Command Line Tools, which you have, plus the
`macos` target. If the link step fails, run `xcode-select --install` and confirm
`xcrun --show-sdk-path` prints a path.

Spend the compile time reading `src-tauri/tauri.conf.json`. Specifically the
`identifier` — TCC permissions are keyed to the bundle identity, so if you change
`com.kelechiodom.dictate` later, macOS will forget you granted Accessibility and
you will re-grant it for no visible reason. Decide the identifier now.

---

## Session 1 — the hotkey, without stealing focus (2 h)

This is half the risk. Get it wrong and the whole interaction is wrong.

Register a global shortcut, have it fire while a *different* app is focused, and
show your overlay without the target app losing focus.

```ts
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
```

**Check:** focus VS Code, press the chord, an overlay appears, and your cursor is
still in the VS Code editor. Press a key and it types into VS Code, not into
your overlay.

**Failure you will hit:** the window takes focus when it shows, so your pasted
text goes to the wrong place. Fix it on the window, not in the handler: create the
overlay with `focus: false` and `skipTaskbar: true` in `tauri.conf.json`, and if
it still steals focus, set the window's activation policy to `Accessory` so the
app never becomes the active app.

**The API trap worth knowing before you design around it:** hold-to-talk needs
key-*up*, and the JS binding for the global-shortcut plugin does not reliably
surface a release event for every chord. Check it yourself before you write the
interaction around it — register a handler, log the whole event object, and press
and release the chord.

- If you get `Pressed` and `Released`: build hold-to-talk, it is the better
  interaction.
- If you only get `Pressed`: either move registration into Rust, where
  `GlobalShortcutEvent` carries `state`, or ship press-to-start /
  press-to-stop for the spike. Toggle is what most dictation tools default to
  anyway.

Do not discover this in session 3. Check it in the first hour.

---

## Session 2 — audio capture (3 h)

Add `cpal` to `src-tauri/Cargo.toml` and fill in `audio::start` / `audio::stop`.

The three things that will actually cost you time:

1. **The stream must outlive the function.** If your `cpal::Stream` is a local
   variable, capture stops the instant `start()` returns, with no error. Store it
   in a `static` (`OnceLock`/`Mutex`) or in Tauri's managed state. Symptom:
   `stop()` returns an empty vec.
2. **Sample format is not uniform.** Use `build_input_stream_raw` and handle
   `SampleFormat::F32` and `SampleFormat::I16` (`s as f32 / 32768.0`). Built-in
   Mac mics are f32; USB interfaces usually are not.
3. **Device rate is not 16 kHz.** You will get 44.1 or 48 kHz. Resample with
   `rubato`, not by dropping samples — naive decimation aliases, and aliased
   speech sounds fine to you and transcribes badly, which sends you hunting
   through the model instead of the resampler.

**Check:** say a sentence with a known first word. `stop()` returns roughly
`duration_seconds * 16000` samples, and the first word is not clipped — which
means the buffer has to start filling on key-*down*, not when the model loads.
Play the samples back (`hound` to a wav, open it) and listen. If the first
phoneme is missing you will spend a week blaming the model.

**Worth 10 minutes:** print RMS per 100 ms while recording. If it is flat zero,
you have a silent device or a dead stream, and you want to know that before
transcription exists.

---

## Session 3 — transcription (3 h)

Pick the model before the code, because it decides your latency budget.

```sh
mkdir -p models
curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin \
  -o models/ggml-base.en.bin          # ~142 MB
brew install whisper-cpp              # gives you whisper-cli
```

Fill in `transcribe::transcribe`: write a temp 16-bit PCM wav with `hound`, shell
out to `whisper-cli -m models/ggml-base.en.bin -f <wav> -nt -np`, trim, time it,
log the duration.

**Check:** one spoken sentence, word-perfect, and a printed duration. Then repeat
with `ggml-small.en.bin` and write both numbers down. That comparison is the
first real measurement in this project and it is the kind of decision you should
never make from a blog post again.

**Failure you will hit:** the model returns confident nonsense. Nine times out of
ten the input is not 16 kHz mono — a stereo buffer passed as mono, or a 48 kHz
buffer labelled 16 kHz, halves or triples the playback speed inside the model.
Check the sample count against the duration before you touch the prompt.

---

## Session 4 — paste (2 h)

Add `enigo` and fill in `insert::paste`: read clipboard, write clipboard, post
Cmd+V, sleep 300 ms, restore.

**Check:** works in TextEdit, VS Code, and a browser input. Then the important
one — copy something, dictate, and paste into a fourth app. You should get what
you copied before, not your transcript.

**Failure you will hit:** nothing appears and no error is raised. This is TCC,
not your code. The dev build needs Accessibility permission, and a synthetic
event from an untrusted app is dropped silently. Check
System Settings → Privacy & Security → Accessibility for your app (not your
terminal), and grant it after each rebuild until you are signing consistently.
This is also the moment you learn why session 0 made you decide the bundle
identifier first.

---

## Session 5 — measure, then decide (2 h)

Run the gate in `docs/spike.md` and write the numbers down. Not "feels fast" —
p95 over at least twenty dictations, per app.

Then WER on 20 of your own recordings: transcribe them, diff against what you
actually said, count errors over total words. It takes an hour and it is the
single most useful hour in the spike, because it converts "the transcription is
pretty good" into a number you can improve.

Write `docs/spike-results.md` with seven rows, each pass or fail, each with the
measurement. If something failed, write the mechanism you believe caused it.

---

## After the spike

If all seven pass, run your own route: `/kelechi:kickoff` with `Mode: new repo`.
The spike answers step 4 (stack forks) with evidence instead of opinion, and
steps 1–3 then describe a product you have already proven is buildable.

If check 6 failed but 1–4 passed, do not write product docs yet — the streaming
design is a different product and it changes what `audio` has to do.

## The teach-back, before you move on

Eight hundred words, mechanism only, no crate names. Explain: why 16 kHz mono,
why the stream must outlive the function, why paste beats element insertion, what
aliasing does to a transcript, and why a synthetic keystroke needs a permission
that a normal keystroke does not. If you cannot write that from memory, redo the
session it came from — the app working is not the same as you knowing why.
