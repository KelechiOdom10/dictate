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
