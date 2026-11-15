# Spike results

The seven-check gate is still open. Unmeasured checks remain unverified.

## Audio capture — 14 November 2026

Prediction supplied before this run: a four-second recording should return about
64,000 samples; the expected first failure was dropping the stream and getting zero.

A temporary Rust example called the real `audio::start()`, waited two seconds,
called `start()` again, waited another two seconds, then called `stop()`.
It ran on this machine against the MacBook Pro Microphone: 96,000 Hz, one
channel, F32. Result: **64,170 samples**, or **4.010625 seconds at 16 kHz**.
`is_recording()` was true after start and false after stop. A second stop returned
`Err("Microphone is not recording")`. The probe exited successfully.

The count supports the prediction and confirms repeated start did not discard
its first half. The dropped-stream prediction did not occur. This was a direct
Rust-stage check, not a physical hotkey hold. The example was removed afterward.
First-phoneme preservation and audible quality have not been checked.

The synthetic one-second 48 kHz stereo 440 Hz sine test returned exactly
16,000 mono samples. Tests also check waveform error, integer normalization,
filtering above 8 kHz, and flushing a 10 ms recording at 44.1 kHz.

Manual completion: run `pnpm tauri dev`, focus another app, hold ⌥⇧Space for
about four seconds while speaking a sentence with a known first word, then
release. Expect the device log and `[hotkey]` count near 64,000. Listen to a
debug capture (`DICTATE_DEBUG_WAV=1 pnpm tauri dev` logs its temp WAV path;
delete the recording after listening) to confirm the first word and ending are intact before claiming
spike check 2 passes.

## Transcription — 14 November 2026

Measured on this machine (Apple M4 Pro), using Homebrew whisper.cpp 1.9.5.
The fixture is generated at test time with macOS `say -v Samantha` and converted
with `afconvert` to 16 kHz mono PCM. It contains 42,732 samples (2.67075 s):
“The quick brown fox jumps over the lazy dog.” No recording is checked in.

| Model | Stage duration | Returned text |
|---|---:|---|
| base.en, first invocation after installation | 6,499 ms | The quick brown fox jumps over the lazy dog. |
| small.en, subsequent invocation | 793 ms | The quick brown fox jumps over the lazy dog. |
| base.en, repeat invocation | 282 ms | The quick brown fox jumps over the lazy dog. |

Each invocation starts a fresh subprocess and loads the model again. The timer
covers `transcribe` (WAV writing, subprocess startup, model loading/inference,
text reading and temporary-directory cleanup); it excludes speech generation.
The first-run difference is observed, its cause has not been isolated. These are
single synthetic-speech observations, not a controlled model benchmark, p95,
owner-recorded accuracy measurement, or the spike's release-to-insert latency.
All seven gate checks remain open; a physical spoken sentence is still needed
for check 3.

Reproduce from the feature checkout (weights stay ignored):

```sh
source "$HOME/.cargo/env"
brew install whisper-cpp
mkdir -p models
curl --fail --location https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin -o models/ggml-base.en.bin
curl --fail --location https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin -o models/ggml-small.en.bin
cargo test --manifest-path src-tauri/Cargo.toml generated_speech_transcribes_through_whisper_cli -- --nocapture
DICTATE_WHISPER_MODEL="$PWD/models/ggml-small.en.bin" cargo test --manifest-path src-tauri/Cargo.toml generated_speech_transcribes_through_whisper_cli -- --nocapture
```

`DICTATE_WHISPER_CLI` can point to a separately built whisper-cli executable.
Without an override, discovery checks PATH and both Homebrew bin directories.
The default model is in this build checkout's `models/`, independent of launch
working directory. This is a spike development default, not bundled packaging.
The integration test prints `SKIP` if the CLI/model is missing or the host is not macOS.

Manual check 3: run `pnpm tauri dev`, focus another app, hold ⌥⇧Space, say one
sentence, release. Expect the sample count followed by
`[transcribe] release_to_text_ms=… text="…"`, word-perfect. That timer starts at
the release handler before capture shutdown and includes worker scheduling and
all transcription work. If inference is already running, another transcription
returns a clear busy error instead of queuing model processes; wait and repeat.
The release path now attempts paste after this log. The physical hotkey path has not been exercised in this run.


## Paste stage — 14 November 2026

Prediction before the clipboard probe: the requested 300 ms delay should sleep
for at least 300 ms; restoring must reproduce all item formats byte for byte.
Two tests use unique, isolated macOS pasteboards, never the user's clipboard.
They cover two items, binary and RTF representations, an empty clipboard, and a
new copy during the delay. Both passed. Measured delays on the final probe:
**310, 310, 310 ms**. This measures the wait, not target-app paste consumption;
300 ms remains a provisional value until the three-app checks below pass.
Removing the restore write made the multiple-item test fail (0 items versus 2),
then restoring the implementation made both tests pass.

AppKit printed a synchronous promise-fulfillment warning because the tests read
pasteboard items written by their own process from a test thread. Assertions
passed; this does not prove external applications consume the clipboard in time.
A read-only `AXIsProcessTrusted` probe returned true for the task shell. This is
not evidence of Accessibility trust for the unsigned dictate app. The app checks
its own trust before touching the clipboard and names the Settings fix on error.

Checks 4 and 5 remain **unverified**. No synthetic keystroke was sent to a user's
focused application by this run. The bundle identifier, accessory activation
policy, and overlay focus configuration are unchanged. Physical focus behaviour,
permission denial in the app, target-app insertion and clipboard restoration
through the hotkey still need this manual session:

1. Run `pnpm tauri dev`. Copy a distinctive sentinel, then focus a TextEdit field.
2. Hold ⌥⇧Space, speak, release. If refused, grant **dictate** under System
   Settings → Privacy & Security → Accessibility, then retry. Rebuilt unsigned
   apps may need the grant refreshed.
3. Confirm the overlay never takes the cursor and the transcript appears once.
   Paste into a separate field afterward: it must contain the sentinel.
4. Repeat in VS Code and a browser input; repeat with rich text or an image copied
   first, and verify the original remains pasteable afterward.
5. Record failures and counts here. For check 6, collect at least 20 dictations
   per app and independently observe when text appears. The log
   `release_to_paste_complete_ms` includes capture stop, scheduling, transcription,
   event posting, restore delay and clipboard restoration. It is an upper-bound
   pipeline completion measurement, **not** confirmation of visible insertion.

Paste uses Command-V rather than setting an accessibility element's value so it
follows the target application's normal text-editing path, including custom and
browser controls. Text-only clipboard backup was rejected because it loses
images, rich text and multiple items. CGEvent uses the existing native stack
rather than adding a second event abstraction. The delay gives asynchronous
paste handlers a chance to read the temporary transcript before restoration;
there is no cross-application acknowledgement, so an unusually slow app can
still read too late. A newer copy detected during the delay is kept and reported
instead of overwritten. macOS offers no atomic compare-and-restore operation,
so an external clipboard write racing the final check remains possible.

## Silent-take guard — 14 November 2026

Prediction: digital silence has RMS 0; generated speech should clear the
provisional 0.003 RMS floor, while its first 150 ms should fail the 300 ms
minimum regardless of energy. RMS is the square root of mean squared sample
amplitude on normalized 16 kHz mono floats.

Measured here using `say -v Samantha` with “The quick brown fox jumps over the
lazy dog.”, then `afconvert -f WAVE -d LEI16@16000 -c 1`:

| Buffer | Duration | RMS | Gate |
|---|---:|---:|---|
| Entire generated sentence | 2.67075 s | 0.150187 | pass |
| First second of that sentence | 1 s | 0.150811 | pass |
| First 150 ms of that sentence | 0.15 s | 0.115536 | skip: duration |
| Generated digital silence | 2 s | 0 | skip: energy |

Direct whisper-cli/base.en reproduction on 300 ms digital silence returned
`you`. This confirms that a nonempty transcript alone cannot distinguish a
silent take. The 300 ms minimum rejects accidental taps even when loud; the
0.003 floor (about −50.5 dBFS) is about 50 times below the first-second speech
RMS. These are provisional thresholds, not voice activity detection: room noise
above the floor can pass, and quiet speech or speech surrounded by long silence
can be rejected. No real-mic noise-floor or quiet-speech calibration was done.
No words, including `you`, are blacklisted. Empty and punctuation-only text is
dropped before paste.

Regression tests were first run with both gates disabled: all three failed.
With the gates enabled, silence/short/low-energy takes are rejected and one
second of freshly generated speech passes. The shared pipeline guard returns
before launching Whisper for rejected audio. Debug WAVs are opt-in, float32
16 kHz mono in the system temporary directory, and retained for listening.

Manual completion: launch `DICTATE_DEBUG_WAV=1 pnpm tauri dev`, focus a disposable
text field, quickly tap ⌥⇧Space: nothing should type and `[hotkey] skipped:`
should explain the duration. Hold silently for a second and expect the energy
reason; speak a sentence and confirm it pastes. Listen to the logged WAV and
delete it afterward. Repeat with quiet speech and normal room noise. Physical
hotkey, real-mic gate behavior, first/last phonemes, and focused-app insertion
were not verified in this run; the seven-check gate remains open.

## Annotate screen stage — 15 November 2026

`src-tauri/src/screen.rs` records one display to an H.264 MP4 through
ScreenCaptureKit's `SCRecordingOutput` (`screencapturekit` 11.1.2, feature
`macos_15_0`). `examples/screen_capture.rs` calls it alone.

**No recording was made in this run.** The agent ran while the Mac was locked
with the display asleep: `CGSessionCopyCurrentDictionary` reported
`CGSSessionScreenIsLocked = 1` and `CGDisplayIsAsleep(main) = 1`. A plain
`swiftc` probe (no Rust involved) got 0 displays, 13 windows and 8 applications
from `SCShareableContent`, with no error. The example stops cleanly in that state:
`error: Display 1 is not among the capturable displays []`. R5, R6 and the
30-second playback check are **not measured**. The steps below measure them.

What was checked:

| Check | Result |
|---|---|
| `CGPreflightScreenCaptureAccess()` from the agent's shell | `true`. This is inherited from the parent app's grant, as the design doc predicted. It says nothing about dictate's own grant. |
| Link on this machine (Command Line Tools only, no Xcode) | Failed at first with undefined `__swift_FORCE_LOAD_$_swiftCompatibility56`. Then the binary aborted at launch: `Library not loaded: @rpath/libswift_Concurrency.dylib`. `build.rs` now adds the CLT Swift library path and an `/usr/lib/swift` rpath. Both the example and the `dictate` binary now link and load. `otool -l` shows the rpath on `target/debug/dictate`. |
| Output size rule | Unit tests: native pixels up to 2560 on the long edge, aspect kept, even dimensions. A 3024×1964 display (this Mac) records at 2560×1662. |
| Bundled-app wrapper for R3 | Ad-hoc signed `ScreenProbe.app` built with the steps below (`codesign -dv` shows identifier `com.kelechiodom.dictate.screen-probe`). Not launched, because launching it shows a permission prompt on the owner's screen. |

### Manual steps (owner, unlocked Mac)

Build once:

```sh
cd src-tauri
cargo build --release --example screen_capture
```

**30-second recording and R6 (CPU, size, dropped frames at 15 fps).** Leave a
normal working screen up (an editor and a browser) and move between them during
the run. Run from Terminal or iTerm2. They hold the Screen Recording grant, so
this measures cost, not permission.

```sh
before=$(ps -o time= -p "$(pgrep -x replayd)")
/usr/bin/time -l ./target/release/examples/screen_capture --seconds 30 --fps 15
ps -o time= -p "$(pgrep -x replayd)"; echo "replayd before: $before"
avmediainfo <path printed by the example>
```

Record here: the example's `duration`, `size`, `frames delivered` and
`expected`; `/usr/bin/time`'s user + sys seconds (our process); and the change in
`replayd` CPU time (the system encoder). CPU % of one core = (our CPU s +
replayd CPU s) / 30. Pass: below 15 %. `avmediainfo` must show one H.264 video
track of about 30 s at 2560×1662. Then open the file in QuickTime and confirm it
plays. A dropped frame is a gap, not an `idle` frame: `idle` means the screen did
not change. If `complete + idle` is well below `expected`, scrub the video for
stutter and write down what you see.

**R5 (our windows are absent).** The example keeps a hidden window from the start,
like the real app's hidden main window. With `--overlay-at`, it opens a new red
400×400 window in the middle of the screen. The window is click-through and never
takes focus.

```sh
./target/release/examples/screen_capture --seconds 10 --overlay-at 3
./target/release/examples/screen_capture --seconds 10 --overlay-at 3 --include-own-windows
```

You will see the red square on screen in both runs. Open each MP4 in QuickTime
and scrub to 5 s. Pass: the first video has no red square and printed
`own windows excluded: yes`. The second video (the control) does show it. If the
first run prints `own windows excluded: NO`, ScreenCaptureKit did not list our
process, and the filter could not name it. Record that.

**R3 (permission in a bundled app).** Wrap the example in an app bundle with its
own identity, so macOS asks for that app and not for the terminal:

```sh
APP=/tmp/ScreenProbe.app
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS"
cp target/release/examples/screen_capture "$APP/Contents/MacOS/"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.kelechiodom.dictate.screen-probe</string>
<key>CFBundleName</key><string>ScreenProbe</string>
<key>CFBundleExecutable</key><string>screen_capture</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSUIElement</key><true/>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
open -W "$APP" --stdout /tmp/probe.out --stderr /tmp/probe.out --args --seconds 5
cat /tmp/probe.out
```

1. First launch: expect `permission preflight: false`, a system prompt naming
   ScreenProbe, and `error: Screen Recording permission is not granted…`.
2. Allow ScreenProbe in System Settings → Privacy & Security → Screen & System
   Audio Recording. Run the same `open` line again. Expect preflight `true` and a
   5 s MP4.
3. Rebuild (`touch src/screen.rs && cargo build --release --example
   screen_capture`), repeat the bundle steps so the code signature changes, and
   run again. Write down whether the grant survived the rebuild.
4. Remove the test entry afterwards: `tccutil reset ScreenCapture
   com.kelechiodom.dictate.screen-probe`.

The real dictate bundle (`pnpm tauri build`) cannot run this check yet, because
no Tauri command calls `screen` until the Annotate session task wires it in. The
bundle above has the same shape: a bundle id, ad-hoc signed, started by
LaunchServices.

## Annotate timed transcription — 15 November 2026

`transcribe::transcribe_timed` runs whisper-cli with `-ojf -ml 1 -sow`, so each
JSON segment is one word with ms offsets. Measured on this machine (Apple M4
Pro, Homebrew whisper.cpp 1.9.5). The fixture is generated at test time with
`say -v Samantha` (17 sentences of UI feedback), converted to 16 kHz mono:
**61,979 ms** of audio. No recording is checked in. Three runs per model:

| Model | `transcribe_timed` wall time | Words | Segments rebuilt | Last word ends |
|---|---:|---:|---:|---:|
| base.en | 1,126 / 1,100 / 1,021 ms | 205 | 14 | 61,760 ms (219 ms before the end) |
| small.en | 2,049 / 2,126 / 2,139 ms | 205 | 22 | 61,800 ms (179 ms before the end) |

The timer covers WAV writing, the subprocess, model loading, inference and JSON
parsing. It excludes speech generation. Word start and end times never go
backwards in any run. Segments split after `.`, `!` or `?`, or at a pause of
700 ms or more; small.en puts in more sentence punctuation, so it makes more
segments from the same audio. These numbers agree with the design doc's
53.7 s estimate (base.en 1.59 s, small.en 2.73 s). This is synthetic speech;
a real spoken minute has not been measured.

Reproduce (the test prints `SKIP` with the reason when whisper-cli or the model
is missing):

```sh
cargo test --manifest-path src-tauri/Cargo.toml generated_minute -- --nocapture
DICTATE_WHISPER_MODEL="$PWD/models/ggml-small.en.bin" cargo test --manifest-path src-tauri/Cargo.toml generated_minute -- --nocapture
cargo run --manifest-path src-tauri/Cargo.toml --example transcribe_timed -- speech.wav
```

### Word times after silence — 15 November 2026

The review of PR #19 found that whisper-cli pulls word times towards the start
of its window when the audio opens with silence. `transcribe_timed` now cuts
the audio into runs of speech with a simple energy detector
(`transcribe::speech_spans`: 20 ms RMS windows, voiced above 4× the quietest
tenth of windows, clamped to 0.005–0.02; runs split at quiet stretches of
1.5 s or more, padded by 250 ms), transcribes each run alone and adds its
offset back. Calls to whisper-cli hold the transcription lock for the whole
take.

Fixtures are built in the tests from `say -v Samantha` plus generated noise at
about -50 dBFS. True onset = padding + the first 10 ms window of the `say` clip
above -40 dBFS. Target: the first word within 300 ms of the true onset.

| Fixture | Word | True onset | Before (base.en) | After (base.en) | After (small.en) |
|---|---|---:|---:|---:|---:|
| 4 s silence, "Move the button. Then make it blue.", 4 s silence | Move | 4,000 ms | 0 ms (**−4,000**) | 4,000 ms (0) | 4,000 ms (0) |
| 1 s silence, "Move the button.", 4 s silence, "Then make it blue.", 1 s | Move | 1,000 ms | 0 ms (**−1,000**) | 1,000 ms (0) | 1,000 ms (0) |
| same, after the 4 s gap | Then | 6,024 ms | 5,800 ms (−224) | 6,030 ms (+6) | 6,030 ms (+6) |

The 60 s fixture is unchanged: base.en 1,033 ms, 205 words, last word at
61,760 ms; small.en 2,080 ms. Each extra run costs one more whisper-cli start
(about 150 ms with base.en), so a take with many long pauses gets slower. The
padding bounds the leftover error: if whisper snaps a run's first word to the
start of the run, the time is at most 250 ms early.

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib transcribe::tests -- --nocapture --test-threads 1
```

## Annotate frames and contact sheet — 15 November 2026

`frames::render` pulls each key frame out of `recording.mp4` with
`AVAssetImageGenerator` (through `objc2-av-foundation`, async call, zero time
tolerance, all frames requested at once), draws the ink with `tiny-skia`, and
writes `frames-full/frame-NN.png`, `frames/frame-NN.png` (long edge ≤ 1568 px)
and `contact-sheet.png` (long edge ≤ 2400 px). Frames are inked, scaled and
encoded in parallel, one thread per frame.

Fixture (`tests/annotate_frames.rs`): a 60 s, 2560×1600, 15 fps H.264 MP4
written by the test with AVAssetWriter (no screen recording, no ffmpeg). Each
video frame carries its index as a barcode. Four circled marks on a second
display at origin (−1512, −200), plus one speech-only stretch: 5 key frames.
Apple M4 Pro, debug test build with tiny-skia and png optimised (Cargo
`profile.dev.package`).

| Measure | Run 1 | Run 2 | Target |
|---|---:|---:|---|
| Stop to package (`build` + `write_package` + `frames::render`) | 264 ms | 251 ms | < 5 s for 60 s and 4 marks |
| Frame extraction, 5 frames | 135 ms | 121 ms | R7: < 1 s for 12 |
| Ink + scale + PNG, 5 frames in parallel | 63 ms | 64 ms | — |
| Contact sheet (2398×1020) | 63 ms | 64 ms | — |
| Delivered minus requested video time | −47, −13, −27, −47, −47 ms | same | R7: within 1/fps (66.7 ms) |

Every frame's barcode matched the expected video frame, and each frame's red
ink sat within 1% of its mark box on all four sides; the speech frame had no
ink. The negative time errors are expected: the generator returns the frame on
screen at that time, which started up to one frame interval earlier.

The fixture's flat colours compress unusually well, so PNG cost was also
measured on a real 2560×1662 screen capture (the reference app's
`frames-full/frame-01.png`, release build): 12 full + 12 downscaled frames
encoded in parallel in 84–94 ms, about 10 ms and 2.2 MB per full frame.

Not measured here: extraction from a real ScreenCaptureKit MP4 (its keyframe
interval may differ from AVAssetWriter's) and 12 frames from one file. Whisper
time is not in the stop-to-package number; it adds about 1.0 s (base.en) or
2.1 s (small.en) for a 60 s take (see "Annotate timed transcription"), so the
whole stop-to-package path is about 1.3–2.4 s.

```sh
cargo test --manifest-path src-tauri/Cargo.toml --test annotate_frames -- --nocapture
DICTATE_SAMPLE_PACKAGE="$PWD/target/annotate-sample" cargo test --manifest-path src-tauri/Cargo.toml --test annotate_frames
```
