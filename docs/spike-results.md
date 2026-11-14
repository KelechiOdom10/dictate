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
