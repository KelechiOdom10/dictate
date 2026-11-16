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


## Startup recovery: microphone lookup — 15 November 2026

Measured on this Mac with 30 sequential calls to the same CPAL device-listing
path formerly called on key-down. `cargo test --manifest-path
src-tauri/Cargo.toml measure_input_device_listing -- --ignored --nocapture`:
first call 105.576 ms, median 46.802 ms, p95 50.972 ms, maximum 105.576 ms.
These are device-discovery timings, not end-to-end capture latency.

Named-device selection now reuses cached CPAL handles. The cache is populated
by listing devices in Settings or on the first named selection. A cached handle
must still return a valid input configuration; failed validation or a missing
name refreshes the list. This also checks for a reconnected microphone rather
than remembering its absence. System-default selection needs no list. The
separate availability scan before opening the stream has been removed.

`cargo test --manifest-path src-tauri/Cargo.toml measure_cached_device_selection
-- --ignored --nocapture` measured 30 selections: median 24.428 ms, p95
26.603 ms, maximum 27.558 ms. This includes configuration validation; stream
creation is outside this measurement. A missing saved name still requires a
fresh enumeration to detect reconnection, then falls back to the current system
default. Physical unplug/replug during capture remains unverified.

The explicit `disconnected_preference` smoke test used a nonexistent saved name,
opened the MacBook Pro Microphone at 96 kHz mono F32, returned fallback=true,
retained samples for a 150 ms take and read a finite level during a separate
150 ms monitor session. No capture was written to disk. This proves default
fallback at the audio boundary; it does not prove native overlay focus, physical
hotkey conflicts, tray rendering, or end-to-end paste after recovery.

## Settings review follow-up: named-microphone startup — 15 November 2026

The named-device path previously fetched the default input configuration twice:
when checking the cached handle, then again when opening the recording stream.
Device selection now returns the handle and its configuration together. A failed
cached configuration still refreshes the device list; missing names still use the
system default. Both recording and microphone monitoring use this path.

Measured on this Mac with the MacBook Pro Microphone explicitly selected by name
(96 kHz, mono F32), a populated device cache, and 30 sequential starts per version:

| `start_device(Some(name))` → stream playing | Before | After |
| --- | ---: | ---: |
| First call | 93.160 ms | 61.480 ms |
| Median | 94.836 ms | 77.419 ms |
| p95 | 98.309 ms | 82.595 ms |
| Maximum | 99.981 ms | 82.925 ms |

Command: `cargo test --manifest-path src-tauri/Cargo.toml
measure_named_microphone_startup -- --ignored --nocapture`. The same test ran
before and after the audio change in an unoptimised test build. Each timer ends
when `start_device` returns; a separate 150 ms capture afterward verifies samples
arrive before stopping. No audio is written to disk. These measurements include
the audio-thread request, device selection, stream creation and `play()`, but
exclude physical key delivery, overlay rendering and first-sample latency.
They do not close the end-to-end spike latency gate. Physical unplug/replug
remains unverified.

## Speech model speed — 15 November 2026

Measured on this machine (Apple M4 Pro), Homebrew whisper.cpp 1.9.5, with the
`measure_catalog_models` test in `src-tauri/src/transcribe.rs`. The fixture is
generated at test time with `say -v Samantha` and converted to 16 kHz mono:
94,335 samples (5.90 s) of “Please move the Thursday design review to Friday
morning, and send Priya the updated budget before lunch.” Each model gets one
warm-up call, then five timed calls of the real `transcribe` stage (WAV
writing, subprocess startup, model load, inference, cleanup). Every file was
checked against the catalog's size and SHA-256 before the run.

| Model | Median | Min | Max | Returned text |
|---|---:|---:|---:|---|
| tiny.en | 188 ms | 177 ms | 194 ms | …send free of the updated budget before launch. |
| base.en | 232 ms | 224 ms | 235 ms | …send free of the updated budget before launch. |
| small.en | 437 ms | 435 ms | 502 ms | …send Priya the updated budget before lunch. |
| medium.en | 1,037 ms | 1,031 ms | 1,052 ms | …send free of the updated budget before launch. |
| large-v3-turbo | 1,084 ms | 1,075 ms | 1,093 ms | …send pre of the updated budget before launch. |

The medians are the speed meters on the Models page (`models::CATALOG`). A
first run of the same test about 20 minutes earlier gave medians within 3% of
these. Only small.en got every word of this one sentence right, which is one
synthetic sentence and not an accuracy measurement. The accuracy meters stay
labelled as estimates from model size.

Reproduce (about 3.9 GB of weights, kept outside the repo):

```sh
DICTATE_MEASURE_MODELS_DIR=/path/to/ggml/files \
  cargo test --manifest-path src-tauri/Cargo.toml measure_catalog_models -- --ignored --nocapture
```

## Warm microphone — 15 November 2026

Advanced → *Keep the microphone ready* leaves the input stream open between
takes and keeps the last 300 ms in a ring buffer. A take starts by handing that
buffer over instead of opening a stream.

Measured on this Mac (Apple M4 Pro, system default input) with
`warm_microphone_starts_takes_with_preroll_and_stays_open` in
`src-tauri/src/audio.rs`, four runs:

| | Result |
| --- | ---: |
| `start_device(None)` with the mic warm | 0.615–0.654 ms |
| Samples from a 200 ms hold, first take | 8,042–8,128 (0.50–0.51 s) |
| Samples from a 200 ms hold, second take | 8,042–8,128 |

Compare the cold named-device start above: 61–83 ms before the stream plays.
The extra ~300 ms in each take is the pre-roll from before the key press, and
the second take proves the stream stays warm after a take ends. Unoptimised test
build; the timer stops when `start_device` returns. Physical key delivery and a
real clipped-word comparison are left to the manual checks.

```sh
cargo test --manifest-path src-tauri/Cargo.toml warm_microphone -- --ignored --nocapture
```

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

## Annotate session, stop to package (task 3-in-app + 9)

15 Nov 2026, Apple M4 Pro, debug build, `tests/annotate_session.rs`. The
session path `annotate::package` runs on a generated 20 s MP4 (2560×1600,
15 fps) with three circled marks and a fixed word list in place of whisper:
the MP4 moved into the folder, `audio.wav`, `annotation.json`, `summary.md`,
`frames/`, `frames-full/` and `contact-sheet.png`.

| Step | Time |
|---|---:|
| Stop to package (whole path, no whisper) | 229 ms |
| Frame extraction, 3 frames | 113 ms |
| Ink + scale + PNG, 3 frames | 63 ms |
| Contact sheet (2398×518) | 37 ms |

`packagingMs` in `annotation.json` stores the same measurement for every real
session, from the stop tap to the finished package, and History shows it as
"Stop to package". Real sessions add `screen::stop` finalising the MP4 and
whisper (about 1–2 s for 60 s, above). Not measured yet: a real session on
this machine, which needs the manual run in the PR.

```sh
cargo test --manifest-path src-tauri/Cargo.toml --test annotate_session -- --nocapture
```

## Parakeet in process — 15 November 2026

Apple M4 Pro, debug build with ggml at `opt-level = 3`, `transcribe-cpp`
0.4.2 with Metal. Model: Parakeet Unified English 0.6B, Q8_0
(`parakeet-unified-en-0.6b-Q8_0.gguf`, 731,357,568 bytes), held in memory by
`transcribe.rs` and warmed with one 1 s decode by `transcribe::prepare`.
Fixtures are generated at test time with `say -v Samantha`, 16 kHz mono.

| Check | Result | Target |
|---|---:|---:|
| Download from the pinned URL, length + SHA-256 verified | pass, 47 s | verifies |
| Fox (2.67 s) text | `The quick brown fox jumps over the lazy dog.` | exact |
| Fox, warm `transcribe` median (n = 11, three runs) | 60 / 61 / 73 ms | ≤ 100 ms |
| 3 s silence + fox, first word from `transcribe_timed` | "The" at 3,470 ms | 3,480 ± 250 ms |
| Switch to a whisper model or a missing file | held model dropped, `ggml_metal_free` logged | unloads |
| `prepare` (load + warm decode), shaders already compiled | 137–166 ms | — |

The 5.9 s sentence from "Speech model speed" above, warm, n = 5 per run:
medians 152, 106 and 134 ms (min 80 ms, max 431 ms). 134 ms is the Models page
speed meter, against 232 ms for base.en and 437 ms for small.en through
whisper-cli. Parakeet returned "…send Pri of the updated budget…", so it missed
"Priya" where small.en didn't; one synthetic sentence, not an accuracy
measurement.

Not measured yet: `release_to_text_ms` for a physical ⌥⇧Space take, which
needs the manual step in the PR. The app logs it as
`[dictate] release_to_text_ms=… inference_ms=… audio_ms=… model=…`.

Reproduce (put the GGUF at `models/parakeet-unified-en-0.6b-Q8_0.gguf`, or
point `DICTATE_PARAKEET_MODEL` at it):

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib transcribe::tests::parakeet -- --nocapture --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml --lib measure_parakeet_warm_latency -- --ignored --nocapture --test-threads=1
TMPDIR="$PWD/target" cargo test --manifest-path src-tauri/Cargo.toml --lib parakeet_downloads_from_its_pinned_url -- --ignored --nocapture
```

## Parakeet streaming while recording — 15 November 2026

Same machine, build and model as above. With Parakeet active, capture sends the
take as 16 kHz frames of about 100 ms (`audio::frames`) to a stream on the held
model (`transcribe::LiveTake`). Release calls `LiveTake::finish`, which
finalizes the stream. Takes under 4 s, a stream that fails, and a stream that
heard a different length of audio than the take go through batch
`transcribe` instead. Whisper models stay batch-only.

Fixtures are generated with `say -v Samantha` and fed one 100 ms frame at a
time by `paced_take` in `transcribe.rs`. `finish_ms` runs from `finish` to text,
which is the transcription part of `release_to_text_ms`.

| Check | Result | Target |
|---|---:|---:|
| Memo (19.2 s, 62 words), fed in real time: `finish_ms`, three runs | 128 / 130 / 109 ms | ≤ 150 ms |
| Memo words, streamed vs batch | same 62 words | match |
| Minute (memo three times, 57.7 s), fed at 4× real time: `finish_ms` | 92 / 86 / 100 ms | — |
| Minute words | 186 of 186 | all words |
| Fox (2.67 s) through `LiveTake` | batch path, exact text | correct |
| Stream fed half the take | batch path, same text as `transcribe` | falls back |
| Dropping a `LiveTake` mid-take | 0 ms, engine free for the next `transcribe` | discards |

Batch on the memo took about 160 ms in Task A's numbers, so streaming saves
little at 19 s. It matters more as takes grow: batch on 59 s was 536 ms in
`docs/stt-engines.md`, streaming finishes in about 90 ms.

Streaming and batch hear the same words but punctuate differently. On the memo,
streaming wrote "…on the launch. We moved…" and "…upgrade the night before.
Can we meet…", where batch ran the sentences together with commas.
Streaming also dropped the final full stop after "Thanks" in one run. The word
check ignores punctuation for that reason. The next section fixes the full stop.

Not measured yet: `release_to_text_ms` from physical takes, which needs the
manual step in the PR. The app logs `[transcribe] streamed: finish_ms=…` or
`[transcribe] batch: <reason>` for every take, next to the existing
`[dictate] release_to_text_ms=…` line.

Reproduce:

```sh
DICTATE_PARAKEET_MODEL=/path/to/parakeet-unified-en-0.6b-Q8_0.gguf \
  cargo test --manifest-path src-tauri/Cargo.toml --lib transcribe::tests -- --nocapture --test-threads=1
```

### Streamed takes keep their final full stop — 15 November 2026

Parakeet writes a take's closing punctuation only once it hears audio after the
last word. A real take ends at release, so the stream finalized with nothing
after "Thanks". `decode_live` now feeds 500 ms of silence before `finalize`.
Handy's live path finalizes without padding, and the transcribe-cpp stream API
has no flush option.

Memo, streamed with the default commit policy, by length of silence fed before
`finalize` (one run each, frames fed as fast as possible):

| Silence | Ending | `finalize` |
|---:|---|---:|
| 0 ms | "…Friday morning. Thanks" | 84–103 ms |
| 80 ms | "…Friday morning. Thanks" | 94 ms |
| 160–320 ms | "…Friday morning. Thanks." | 95–106 ms |
| 400–500 ms | "…Friday morning, thanks." (batch's text) | 95–102 ms |

The `OnFinalize` commit policy gave the same endings. Feeding the silence took
under 1 ms.

`measure_parakeet_streamed_finish` (ignored by default) streams the memo in
real time three times after a warm-up:

| Build | Ending | `finish_ms` |
|---|---|---:|
| Without the silence | "…Friday morning. Thanks" | 117 ms |
| With 500 ms of silence, 9 runs over 4 invocations | "…Friday morning, thanks." in all 9 | 115–131 ms, with outliers of 155 and 185 ms |

The outliers came while other builds shared the machine (load average 4 to 20).
That is why the 150 ms check now lives in the ignored measure test, not in the
default `cargo test`.

```sh
DICTATE_PARAKEET_MODEL=/path/to/parakeet-unified-en-0.6b-Q8_0.gguf \
  cargo test --manifest-path src-tauri/Cargo.toml --lib measure_parakeet_streamed_finish -- --ignored --nocapture --test-threads=1
```

### QA round 3: stop-path latency (15 Nov 2026)

The capture's 13,936 ms take used the built-in microphone at 96 kHz. The local
log at 19:41:23 reports release-to-text 1,230 ms, inference 107 ms and total
1,554 ms. Thus 1,123 ms preceded inference; delivery added 324 ms, including
305 ms waiting before clipboard restoration. Toggle handling has no deliberate
wait. The completion-pill display delay happens after total is recorded.

An offline buffer of exactly 1,337,856 mono samples (13.936 s at 96 kHz), converted
through the real capture resampler, measured 1,083 ms in the existing debug
profile. Optimizing the FFT dependencies alone measured 1,002 ms. Optimizing
those dependencies and dictate's generic Rust code measured 16 ms, preserving
exactly 222,976 output samples at 16 kHz. These are individual local conversion
measurements, not an end-to-end latency or p95 claim. Debug builds now optimize
that code; release builds were already optimized. Conversion also walks the
input in blocks instead of repeatedly shifting the entire remaining buffer.

Reproduce without microphone or desktop input:

```sh
source "$HOME/.cargo/env"
CARGO_TARGET_DIR=/Users/kelechiodom/Projects/dictate/target \
  cargo test --manifest-path src-tauri/Cargo.toml --lib thirteen_second_take -- --nocapture
```

The stop path now logs audio-stop, worker-queue and debug-WAV-save durations, so
a hands-on take can distinguish device shutdown from conversion and file I/O.
The original log does not separate these stages. A fresh native end-to-end take
is still required to measure the full improvement.

Independent Capy review on this Mac reran the same offline test on 15 Nov 2026:
**10 ms**, with the same exact 222,976 output samples. This is one fresh
conversion run, not native dictation latency, an end-to-end improvement or p95.

## Quiet takes and a microphone shared with a call — 16 November 2026

**The 0.003 RMS dictation gate rejects audio Parakeet can transcribe.** This
is established with both the owner's real recorded speech and an independent
voice-processing holder. Removing that gate, rather than choosing another
amplitude threshold, is the fix. Low amplitude alone does not mean no speech.

Apple M4 Pro, built-in MacBook Pro Microphone, debug build, Parakeet Unified
English Q8_0. Measurements use 16 kHz mono f32 at the transcription boundary.
No transcript or audio from the owner's recordings is committed here.

### Real speech, unchanged and deliberately attenuated

`measure_real_speech_levels` reads an existing 16 kHz mono WAV and selects the
highest-RMS complete five-second block. Here it used the owner's real Annotate
recording from 16:18:07 on 16 November. The app log identifies that recording's
source as the built-in microphone, 96 kHz, one channel. The 16-word baseline
below is Parakeet's output, not a human-verified accuracy score. Scaling changes
amplitude only; it is not a measurement of someone speaking quietly.

| Input | RMS | Parakeet words | Same words as unchanged audio |
|---|---:|---:|---|
| Real speech, five seconds | 0.026990 | 16 | baseline |
| Same speech, attenuated | 0.002295 | 16 | yes |
| Same speech, attenuated | 0.000239 | 16 | yes |
| Same speech, attenuated | 0.000075 | 16 | yes |
| Five seconds of digital silence | 0 | 0 | — |

All three attenuated inputs would have been discarded before inference by the
old gate. The direct transcription test passed with exit code 0. A separate
automated regression generates a public sentence with `say`, scales it to
these levels, and checks the words. The real-speech measurement never prints
the private transcript.

```sh
source "$HOME/.cargo/env"
export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"
export DICTATE_PARAKEET_MODEL="$HOME/Library/Application Support/com.kelechiodom.dictate/models/parakeet-unified-en-0.6b-Q8_0.gguf"
DICTATE_SPEECH_WAV="/path/to/existing-real-speech.wav" \
  cargo test --manifest-path src-tauri/Cargo.toml --lib measure_real_speech_levels \
  -- --ignored --nocapture --test-threads=1
```

### Capture while another process holds voice processing

A scratch Swift AVAudioEngine process enabled input voice processing, installed
an input tap and ran concurrently with a fresh CPAL input stream. This models
one call-app mechanism; it is not a test of a particular conferencing app.

| Condition | CPAL source format | Measured RMS |
|---|---|---:|
| No deliberate playback, no holder, 3 s | 96 kHz, mono | 0.002926 |
| No deliberate playback, repeated 3–4 s takes | 96 kHz, mono | 0.003268–0.004059 |
| Voice-processing holder, no deliberate playback | 96 kHz, 3 channels, downmixed | 0.000061–0.000075 |
| Holder + speaker-replayed synthetic sentence, quiet playback | 96 kHz, 3 channels, downmixed | 0.000075 |
| Holder + speaker-replayed synthetic sentence, louder playback | 96 kHz, 3 channels, downmixed | 0.000094 |

Parakeet returned the full replayed sentence from both held-microphone takes,
without gain normalization; it returned no words on the held-microphone ambient
take. This shows that a shared stream can be very quiet yet usable. It does
not prove every call app behaves the same way, or that the microphone was
exclusively locked. Ordinary replay without the holder also decoded at
0.002295 and 0.000239 after scaling. Those exploratory test assertions completed
but their processes aborted during model teardown; they are supporting
observations, not passing test exits. The real-speech test above explicitly
unloads the model and exits cleanly.

The ambient values are recordings with no deliberate playback, not controlled
acoustic silence. They straddle 0.003 while intelligible speech can be far below
it. There is no defensible universal speech/no-speech cutoff at that number.
System input gain was 69; output gain was temporarily 70 for speaker-replay
experiments and restored to 13. Speaker replay is synthetic speech, not a human
normal/quiet voice comparison. That hands-on comparison remains outstanding.

### What the device code does, and what changes

On-demand capture resolves the system default and reads its current format on
every new stream. A selected named device gets a fresh config even when its
device handle is cached; a missing named device falls back to the default.
Warm streams check the requested preference, the current default's name and
whether a previously missing device has returned before reuse. They do not
watch a same-device format change or an app taking voice processing mid-take.
CPAL's stream-error callback reports actual errors, not valid low-amplitude
buffers. In the reproduced holder case, buffers arrived without such an error.
The owner's logs likewise show a switch from mono to three channels on the
18:14 failed take; the original rejected audio was not saved, so its contents
cannot be reconstructed.

The patch keeps default-device selection and channel averaging unchanged: the
measured shared capture already contains transcribable speech. Dictation now
sends finite, nonzero audio of at least 300 ms to the chosen transcriber,
regardless of RMS. Exact digital silence still skips inference; nonfinite and
too-short takes remain rejected. No adaptive constant or gain boost is added.
Timed Annotate speech segmentation is separate and unchanged by this fix.

`audio::stop` still returns independent 16 kHz mono f32 samples. A second stop
entry point returns those same samples together with the actual device name,
channel count and input rate, atomically, for dictation diagnostics. Empty
transcripts remain visible in History with that device, the possibility of
another app holding/processing it, and a Settings action in the existing detail
pane. This is a possibility, not a claim to detect the call app.

Still requires real hands: normal human speech, genuinely quiet human speech
and a controlled silent take through the built-in mic; repeat while using the
owner's actual call app; join/leave a call during an active/warm take; switch
the system default during a take; confirm the native completion notice, History
detail and focused-app paste behavior. The measurements above do not certify
those native interactions.
