# Speech engines: why Hex and Handy use Parakeet, and how dictate adds it

15 November 2026. Investigation only: no app code changes. Facts are cited to a
file or a model card. Numbers come from this Mac (Apple M4 Pro). Anything
inferred or not measured is marked **(inferred)** or **(not measured)**.

## Status on `main`, 16 November 2026

Read this first: the plan below has partly shipped, so parts of it describe a
past state of the app.

- **Tasks A and B are shipped.** Parakeet Unified English 0.6B Q8_0 runs in
  process on `transcribe-cpp` with a warm session held between takes
  (`transcribe.rs:421` `with_session`), takes stream while recording and
  finalize on release (`transcribe.rs:163` `LiveTake`, started at
  `lib.rs:1294` `start_live_take`), and short takes fall back to batch.
- **Task C is shipped, minus its migration prompt.** Parakeet is the default
  model (`settings.rs:151`), the Models page shows the NVIDIA Open Model
  Licence and its link (`ModelsPage.tsx:158`), and the one-time prompt for
  existing users was built and then deliberately removed: an existing profile
  simply keeps the model it has, with no offer and no silent download.
- **Task D is not done.** Whisper models still run through the Homebrew
  `whisper-cli` (`transcribe.rs:350` `needs_whisper_cli`), so CMake is only
  needed for the `transcribe-cpp` build and Homebrew is still a requirement
  for Whisper.
- **Everything else is the 15 November investigation as written.** Its
  external facts (model cards, sizes, the pinned SHA, Handy's benchmark) are
  unchanged; its description of dictate's state — "no warm model", "no
  streaming", "your settings run `base.en`" — is what the plan replaced, and
  the numbers in §3 stand as the measurements the plan was decided on.

## The answer in short

On 15 November 2026 the gap between dictate and Hex/Handy had four causes:

1. **Model.** Your settings then ran `base.en`
   (`~/Library/Application Support/com.kelechiodom.dictate/settings.json`:
   `"speechModel": "base.en"`; Parakeet is the default now). On Handy's FLEURS
   English benchmark, base.en gets 7.6% of words wrong. Parakeet Unified gets
   3.99%, about half as many.
2. **No warm model.** Every take starts a new `whisper-cli` process, which
   reloads the weights (`transcribe.rs` `run_whisper`). A model held in memory
   answers a short clip in 45 ms here.
3. **No streaming.** dictate starts transcribing only when you let go. Hex and
   Handy feed audio to the model while you talk, so at release only the last
   fraction of a second is left. That is Hex's "No wait".
4. **Whisper decodes word by word.** Its time grows with the length of the
   transcript. Once, in a 59 s take here, base.en also dropped a whole
   sentence.

**Recommendation.** Add `transcribe-cpp` (MIT; the same Rust crate Hex and
Handy ship) with Metal. Make **Parakeet Unified English 0.6B, Q8_0
(731 MB)** the recommended default. Keep the model loaded, and stream audio into
it while recording. Keep Whisper as an option, run through the same crate, then
drop the Homebrew `whisper-cli` dependency. For Annotate, use the engine's
word timestamps. The build plan is at the end.

## 1. The models

### Parakeet Unified English 0.6B

Source: [nvidia/parakeet-unified-en-0.6b](https://huggingface.co/nvidia/parakeet-unified-en-0.6b),
released 7 April 2026.

- **What it is.** A FastConformer encoder (24 layers) with an RNN‑T decoder,
  600M parameters. It was trained jointly for offline and streaming use. "One
  model for both tasks… minimum latency of 160 ms." Streaming latency (chunk +
  right context) can be set anywhere from 2.08 s down to 0.16 s.
- **Output.** English with punctuation and capitalisation. Not a Parakeet TDT
  model: it is a unified RNN‑T.
- **Accuracy** (Open ASR Leaderboard average WER, from the card):

  | Model | Offline | 2.08 s | 1.12 s | 0.56 s | 0.16 s |
  |---|---:|---:|---:|---:|---:|
  | parakeet-tdt-0.6b-v2 | 6.04 | 7.99 | 22.83 | 69.55 | — |
  | nemotron-speech-streaming-en-0.6b | 6.92 | 7.46 | 6.92 | 7.09 | 7.84 |
  | **parakeet-unified-en-0.6b** | **5.91** | **6.14** | **6.29** | **6.52** | 8.44 |

- **Licence.** The **NVIDIA Open Model License**, on both the NVIDIA card and
  the `handy-computer` GGUF card (`license: other`). Handy's `catalog.json`
  labels it `cc-by-4.0`, which is wrong. Commercial use and redistribution are
  allowed with the licence notice **(inferred from the licence summary; have
  it read before shipping)**. dictate downloads the model and doesn't bundle it.
  It should still show the licence name and link on the Models page.
- **Sizes** (GGUF, `handy-computer/parakeet-unified-en-0.6b-gguf` at
  `7e948f2`): F32 2.47 GB, F16 1.24 GB, **Q8_0 731 MB** (what Hex and Handy
  use), Q6_K 602 MB, Q5_K_M 541 MB, Q4_K_M 477 MB.

### Why it beats Whisper for dictation

| | Parakeet Unified (RNN‑T) | Whisper (encoder–decoder) |
|---|---|---|
| Decoding | One fast decoder step per output frame | Autoregressive: a full decoder pass per text token, so cost grows with transcript length |
| Streaming | Built in: the same weights decode 0.16–2 s chunks | None. Works in 30 s windows; streaming means re-running windows |
| Hallucination on silence or long audio | Transducers emit nothing on silence **(inferred, matches tests below)** | Known failure mode; observed here: base.en dropped a sentence at 59 s |
| Word times | Token and word timestamps from the decoder | Rough; dictate needs `speech_spans` cutting to stop 4 s drift |
| Vocabulary prompt | No (Hex patches its own rescoring) | Yes (initial prompt) |
| Languages | English only | Many |

Handy's catalog ships FLEURS English WER, measured with the same engine
(`src-tauri/src/catalog/catalog.json`, `results`). Speed is a multiple of real
time on a Ryzen 4750U with Vulkan; it is Handy's figure, not ours.

| Model | WER en | Speed (×RT) | Q8_0 size |
|---|---:|---:|---:|
| **parakeet-unified-en-0.6b** | **3.99%** | 25.3 | 731 MB |
| parakeet-tdt-0.6b-v2 | 4.11% | 27.4 | 730 MB |
| whisper-large-v3-turbo | 4.38% | 3.1 | 886 MB |
| whisper-medium.en | 4.88% | 4.4 | 831 MB |
| cohere-transcribe-03-2026 | 5.08% | 8.1 | 2.41 GB |
| canary-180m-flash | 5.98% | 33.1 | 218 MB |
| whisper-small.en | 6.14% | 12.5 | 270 MB |
| nemotron-speech-streaming-en-0.6b | 6.43% | 27.1 | 730 MB |
| whisper-base.en (**your current model**) | 7.60% | 32.0 | 85 MB |
| nemotron-3.5-asr-streaming-0.6b | 7.90% | 17.1 | 751 MB |

Hex's "1 in 63 wrong words" is about 1.6% WER. Its test set isn't in the
public repo **(not verified)**.

### The others, briefly

- **Canary 180M Flash** ([card](https://huggingface.co/nvidia/canary-180m-flash), CC‑BY‑4.0).
  182M parameters: a FastConformer encoder with a Transformer decoder. Handles
  en/de/fr/es and translates between them. Word timestamps are "experimental".
  Small (218 MB Q8) and quick. Handy recommends it for weak hardware. Not
  streaming.
- **Nemotron Speech Streaming EN 0.6B / Nemotron 3.5 ASR Streaming**
  ([card](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b),
  NVIDIA Open Model License). A *cache‑aware* streaming FastConformer‑RNNT: it
  processes only new audio and reuses cached encoder state. Best at 80 ms
  chunks. 3.5 adds about 40 language-locales. Less accurate than Parakeet
  Unified at dictation-sized latencies.
- **Cohere Transcribe 03‑2026** (Apache‑2.0, 2B, 14 languages; gated on
  Hugging Face). Hex shows it as "fewest wrong words". On Handy's English
  FLEURS run it scores 5.08%. It is a 2.4 GB download and not streaming.

## 2. How Hex and Handy run it

**Both use `transcribe-cpp`**, Rust bindings for
[transcribe.cpp](https://github.com/handy-computer/transcribe.cpp). That is a
ggml/GGUF C++ library, a sibling of whisper.cpp, covering 16 model families
with Metal, Vulkan and CUDA backends. Neither uses ONNX Runtime, sherpa-onnx or
CoreML for Parakeet any more.

- **Handy** ([cjpais/Handy](https://github.com/cjpais/Handy) @ `aca8985`, Tauri 2 + Rust):
  - `src-tauri/Cargo.toml:83-88`: "Every model runs through transcribe-cpp
    (GGUF/ggml, native backends)". `transcribe-cpp 0.4.2` with the `metal`
    feature on Apple Silicon (line 165).
  - `src-tauri/src/engine_supervisor/`: the model runs in a **separate worker
    process** that the app supervises. A hung GPU driver can't freeze the app,
    and only one model is loaded at a time.
  - `src-tauri/src/managers/transcription.rs:103-960`: a `StreamRouter` feeds
    16 kHz frames from the recorder to a stream worker during recording.
    Release sends `Finalize`. The overlay shows committed and tentative text.
    Non-streaming models fall back to batch.
  - Models: `src-tauri/src/catalog/catalog.json` pins each model to a Hugging
    Face repo, revision, file, size and SHA‑256. Downloads go through `hf-hub`
    with cancellation (`managers/model.rs`).
- **Hex** has two apps:
  - The **legacy Swift app** ([kitlangton/Hex](https://github.com/kitlangton/Hex) @ `3e30072`)
    runs Parakeet **TDT v2/v3 through FluidAudio on CoreML**
    (`Hex/Clients/ParakeetClient.swift`, `HexCore/.../ParakeetModel.swift`) and
    Whisper through WhisperKit. It needed workarounds for short clips
    (`docs/parakeet-short-audio-plan.md`).
  - The **current Rust app** ([anomalyco/hex](https://github.com/anomalyco/hex) @ `369a303`)
    is the "Version 3.4.0" window in the screenshots, with the "Writes while you
    talk" text in `src/desktop/i18n.rs:1274`. It runs **Parakeet Unified Q8_0
    through `transcribe-cpp`**, patched (`Cargo.toml:45-54`). It keeps
    `transcribe-rs`/ONNX only as an optional `onnx-benchmark` feature. Its
    `docs/plans/transcription-quality.md` is replacing its last ONNX user. Its
    `docs/plans/engine-service.md` measured a warm 2.64 s clip at 46 ms on an
    M5 Max. `src/capture_host.rs:845-870` corrects Parakeet word times with a
    measured lag (start +280 ms, +520 ms after a pause, end +195 ms), fitted
    against Apple SpeechAnalyzer.

## 3. Measured on this Mac

Apple M4 Pro, 15 November 2026. Fixtures were made with `say -v Samantha` and
converted to 16 kHz mono with `afconvert`, in a temp directory. Nothing is
checked in. `fox` is 2.67 s. `memo` is 19.2 s, 61 words of dictation with
names, a number and "Kubernetes". `gap` is 3 s of silence, fox, 2 s of silence,
fox (10.3 s). `long` is memo three times (59 s). Parakeet ran through a
throwaway Rust program on `transcribe-cpp =0.4.2` (default `metal`), with
`hf …/parakeet-unified-en-0.6b-gguf@7e948f2` Q8_0, SHA‑256 `4b50b6dd…` matching
Handy's catalog. Whisper ran as dictate runs it today: Homebrew `whisper-cli`
1.9.5, a fresh process per take, with the `-l en -np -nt` flags.

**Release → text, transcription stage only (median):**

| Audio | whisper-cli base.en | whisper-cli small.en | Parakeet batch (warm) | Parakeet streaming: finalize after release |
|---|---:|---:|---:|---:|
| fox 2.7 s | 233 ms | 422 ms | 45 ms | 46 ms |
| memo 19.2 s | 411 ms | 724 ms | 163 ms | 70 ms |
| gap 10.3 s | — | — | 100 ms | 70 ms |
| long 59 s | 690 ms | 1,410 ms | 536 ms | **68 ms** |

- whisper: n=5 after one discarded run (n=3 for `long`), wall clock including
  process start. `whisper_print_timings` on memo shows where the time goes:
  base.en load 52 ms, encode 41 ms, token decode 120 ms; small.en load 132 ms,
  encode 111 ms, decode 278 ms.
- Parakeet: n=5 batch, n=3 streaming. Streaming feeds 100 ms chunks. Feeding
  costs 1.0 s of compute across 19 s of audio, and the slowest single chunk took
  84 ms, so it keeps up with live audio comfortably.
- **Model load.** The first load after a new build took **3,463 ms**, mostly
  compiling Metal shaders once. After that, a new process loaded in **67–71 ms**.
  The first decode after a load took 61–363 ms, so warm it up at startup.
- **Accuracy on TTS speech is not a fair test.** Every engine transcribed fox
  and memo correctly (Parakeet printed "3" for "three"; so did Whisper). One
  difference was real: on `long`, **whisper base.en dropped "Can we meet on
  Thursday at 3 to go through the hiring plan? Also,"** in the second
  repetition, the same in all 3 runs. small.en and Parakeet (batch and
  streaming) returned all 183 words. Owner-recorded speech is still needed for
  real accuracy numbers.
- **A streaming flaw to design around.** On the 2.7 s fox clip, streaming
  returned "Quick brown fox…" and **lost the first word**. Batch on the same
  clip was correct. Longer clips were fine. Use batch for short takes (see
  task B).
- **Word timings** (batch, `TimestampKind::Word`). After 3 s of silence, "The"
  started at **3,440 ms**. The same word starts at 480 ms in fox alone, so the
  expected start is about 3,480 ms. Whisper was 4 s early here before
  `speech_spans` was added. The second "The" came out at 8,000 ms against an
  expected ~8,150 ms. Words come back with punctuation attached ("Sarah,",
  "dog."), as `segments()` already expects.
- **Binary.** The benchmark binary, with `transcribe-cpp`, ggml and Metal linked
  statically, is **9.0 MB** as an unstripped release build. It built in
  20 s. It needs **CMake** on the build machine: `brew install cmake`, which
  isn't installed here. I used a pip CMake in a temp venv.

## 4. Options for dictate

| Option | Apple Silicon latency | Ships as | Model download | Streaming | Licence | Effort |
|---|---|---|---|---|---|---|
| **A. `transcribe-cpp` (ggml, Metal)**, all models | **Measured:** 45–70 ms at release | Static, about +9 MB, no runtime deps | 731 MB Q8 (477 MB Q4_K_M) | Yes, with committed/tentative text | MIT crate + model licences | Medium: one crate, same shape as `whisper-cli` today |
| B. `transcribe-rs` / `ort` (ONNX Runtime) | **Not measured** | ONNX Runtime dylib to sign and bundle **(inferred tens of MB)** | ONNX exports (int8 ~600 MB, **inferred**) | Not for Parakeet Unified **(not verified)** | MIT + MIT | Medium. Hex is removing ONNX (spinning thread pools, `meeting-live-transcription.md:63`) |
| C. sherpa-onnx | **Not measured** | C library + ONNX Runtime | Parakeet TDT ONNX exports | Zipformer streaming; no Parakeet Unified export found **(not verified)** | Apache‑2.0 | Medium–high, and nobody we compared against uses it for this |
| D. FluidAudio/CoreML Swift sidecar | **Not measured** (ANE; legacy Hex used it) | Swift helper bundled with the app, plus IPC | CoreML Parakeet TDT v2/v3 (~650 MB) | Not Parakeet Unified in legacy Hex | Apache‑2.0 (FluidAudio) | High: second language, sidecar signing; Hex itself moved off it |
| E. Keep `whisper-cli` and add a Parakeet backend | Parakeet as in A | Two engines, and Homebrew stays a requirement | As A + Whisper GGML | Parakeet only | — | A done halfway; fine as a first step |

### How option A maps onto dictate's code

On `main` today. The plan was written against `fix/annotate-review-fixes`,
which is merged, and the first three bullets below are that plan implemented.
The numbers it asks to record are in `docs/spike-results.md:528`.

- **`transcribe.rs`.** Keep the public functions as they are: `transcribe`,
  `transcribe_timed`, `segments`, `model_name`, `missing_model`. That keeps the
  stage independently callable, as `AGENTS.md` requires. Inside, replace
  `run_whisper`/`write_wav` with a `transcribe_cpp::Model` held in a
  process-wide slot. Load it when the model is chosen and at startup, then run
  one 1 s silent decode to warm it. The `TRANSCRIPTION` mutex and the "Still
  transcribing" error stay. `whisper_cli()`/`whisper_missing()` go away once
  Whisper also runs on GGUF (task D).
- **`transcribe_timed`.** `session.run(samples, RunOptions { timestamps: Word, .. })`
  returns `words` with `t0_ms`/`t1_ms` on the audio clock. For Parakeet,
  `speech_spans` isn't needed for accuracy, but keeping it is cheap and bounds
  memory on long sessions. Add Hex's lag correction only if a measurement on
  owner-recorded speech says so. On TTS here, starts were within 40–150 ms.
- **`models.rs`.** `SpeechModel` gains `ParakeetUnifiedEn`. `ModelInfo` gains
  an engine/format field and a `licence`. The catalog pins
  `handy-computer/parakeet-unified-en-0.6b-gguf` at revision `7e948f21…`,
  file `parakeet-unified-en-0.6b-Q8_0.gguf`, 731,357,568 bytes, SHA‑256
  `4b50b6dd862bf6e346929aaf4f5eaacec003bfa3f56462d6c874b41ef2f38795`. The
  existing streaming-download, length and SHA check works as is.
  `measured_ms` gets re-measured with the same 5.9 s fixture.
- **`lib.rs` recorder → transcriber.** For streaming, the audio callback that
  already produces 16 kHz mono f32 also sends 100 ms frames to a stream worker
  (Handy's `StreamRouter` pattern). Release calls `finalize`. Short takes and
  any stream error fall back to batch on the full buffer.

## 5. Recommendation

- **Engine.** `transcribe-cpp` 0.4.x with `metal`, in process, as in Rust Hex.
  Handy's crash-isolating worker process is a good idea, but defer it until a
  GPU hang is actually seen on a Mac.
- **Default model.** Parakeet Unified English 0.6B, Q8_0. Mark it Recommended.
  Offer Canary 180M Flash as the small option for low disk space or 8 GB Macs.
- **Whisper.** Keep it as an option (vocabulary prompt, other languages). Move
  it to `transcribe-cpp` GGUF files (`whisper-large-v3-turbo`, `small.en`) so
  there is one engine and no Homebrew step. Existing GGML downloads keep
  working through `whisper-cli` until then.
- **Annotate.** Use the engine's word timestamps from `transcribe_timed`, which
  keeps its contract. Re-run the existing timed tests. Calibrate a lag only
  against owner-recorded speech.
- **Rejected.** ONNX via `transcribe-rs`/`ort` and sherpa-onnx: unmeasured,
  extra runtime to sign, and both reference apps have moved off ONNX. A
  FluidAudio sidecar: a second language and an IPC layer, and legacy Hex left
  it for the same Rust engine.

## 6. Build plan

**Task A (shipped): Parakeet through transcribe-cpp, warm, batch.**
Add `transcribe-cpp = "0.4.2"` (Metal). Add Parakeet Unified Q8_0 to the
`models.rs` catalog using the pin above. Route Parakeet through `transcribe` and
`transcribe_timed` with a model held in memory and warmed at startup or
selection. Leave Whisper on `whisper-cli`. Note in the README that the build
needs CMake.
*Acceptance:* downloading Parakeet from the Models page verifies the SHA. The
generated-speech tests pass with Parakeet: fox text exact, and median warm
`transcribe` ≤ 100 ms on fox. `transcribe_timed` on 3 s silence + fox puts the
first word within ±250 ms of 3,480 ms. Switching models unloads the old one.
Record the new numbers in `docs/spike-results.md`. *Validation:* `cargo test`,
`pnpm build`, and one physical ⌥⇧Space dictation logging `release_to_text_ms`.

**Task B (shipped): stream while recording.**
Send 100 ms frames to a stream session during capture, and finalize on release.
Use batch for takes under 4 s, and on any stream error.
*Acceptance:* on memo, finalize after the last frame takes ≤ 150 ms and the
text matches batch. The fox take is correct, through the batch path. Cancel
discards the stream. Long takes (59 s fixture) return all words.
*Validation:* a fixture test that feeds paced chunks, plus `release_to_text_ms`
from 10 physical takes.

**Task C: make Parakeet the default and show the licence.**
New installs get Parakeet as Recommended. The Models page shows the licence name
and link, and the release notes say why. *Shipped, with one change of mind:* a
one-time prompt for existing users was built and then removed, because a
migration offer on top of a working default was more ceremony than the app
needs — an existing profile keeps whatever model it already has, and nothing is
downloaded without a click. *Acceptance:* a fresh profile shows Parakeet
Recommended. A profile set to `base.en` stays on `base.en` until the user
switches. *Validation:* UI check in `pnpm tauri dev`.

**Task D (not started): Whisper on GGUF, then remove whisper-cli.**
Move the catalog's Whisper entries to `handy-computer` GGUF files (pinned) on
the same engine. Delete `whisper_cli`, `whisper_missing` and the Homebrew
instructions.
*Acceptance:* every catalog model transcribes the fox fixture. No
`whisper-cli` anywhere in `src-tauri/src`.

## Sources

- NVIDIA model cards: [parakeet-unified-en-0.6b](https://huggingface.co/nvidia/parakeet-unified-en-0.6b),
  [canary-180m-flash](https://huggingface.co/nvidia/canary-180m-flash),
  [nemotron-speech-streaming-en-0.6b](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b).
  Cohere: [cohere-transcribe-03-2026](https://huggingface.co/CohereLabs/cohere-transcribe-03-2026) (API metadata; the card is gated).
- GGUF: [handy-computer/parakeet-unified-en-0.6b-gguf](https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf).
- Engine: [handy-computer/transcribe.cpp](https://github.com/handy-computer/transcribe.cpp) @ `adfc502`; crate `transcribe-cpp` 0.4.2 (MIT).
- Apps: Handy @ `aca8985`, Hex (Rust) @ `369a303`, Hex (Swift, legacy) @ `3e30072`.
