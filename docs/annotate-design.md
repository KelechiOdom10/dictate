# Annotate: design and build plan

Status: proposal, 14 Nov 2026. Docs only; nothing here is built.

Annotate lets you record your screen, draw on it, and talk over it. When you stop,
you get a folder an AI agent can read, and a short text version is on the
clipboard ready to paste. The owner uses this feature daily in another app, and it
is the reason that app beats the alternatives. This doc says how to build it in
dictate, what is still unknown, and the order to build it in.

Every claim below is tagged:

- **[verified]**: checked on this machine (Apple M4 Pro, macOS 27.0.1) or read in
  source on 14 Nov 2026.
- **[reference]**: observed in the capture folder the reference app produced
  (`~/Library/Application Support/scrawl/captures/1794668188178.scrawl/`).
- **[inferred]**: reasoning or recall, not checked. Each one needs a spike or a
  source before code depends on it.

## 1. What the feature does

The interaction, taken from the reference app's settings page and its capture
folder **[reference]**:

| Step | Input | Result |
|---|---|---|
| Start | Tap the shortcut (⌥⇧S) | Screen and microphone recording starts. A small floating bar shows that it is recording. |
| Draw | Hold the shortcut and move the pointer. No click needed. | Red ink follows the pointer, then fades. |
| Pen | Click the pen on the floating bar, then drag | Drag to draw. Click the pen again or press Esc to stop. |
| Stop | Tap the shortcut again | Recording stops, the package is built, and a summary is copied. |
| Send | Paste anywhere | The agent gets the folder path, key frames and what was said. |

Settings **[reference]**: an on/off toggle, the shortcut, Screen Recording
permission status ("Allowed"), and "Show in screen recordings", which is off by
default and, when on, lets the app's own overlay, ink and floating bar appear in
the video.

What the reference package contains **[reference]**:

| File | Content |
|---|---|
| `recording.mp4` | H.264 video of one display, 2560×1662 for a 1512×982 pt display at 2× scale. 52 MB for 44 s (about 9.5 Mbit/s). |
| `audio.wav` | 16 kHz mono 16-bit PCM. This is the same format dictate already uses. |
| `frames/frame-NN.png` | 7 key frames with ink drawn in, cropped to the active windows (2004×1488) |
| `frames-full/` | The same frames without the crop (2560×1662) |
| `contact-sheet.png` | The frames in one 1600×699 grid. Each frame has a numbered badge in its corner and the ink drawn in. |
| `scrawl.json` | Session data: target display, transcript with segment and word times, strokes, marks, clicks, cursor track, key frames, window list, crop data |
| `capture.json` | How the recording was set up: clock, video start time, size and scale, missing permissions |
| `summary.md` | The text that is pasted: one header line with the folder path, a note telling the agent to open the images, then one line per key frame with its time and either the words said or the circled region |

Details in the reference data that the design uses:

- Strokes are stored as flat `[x, y, dt]` triples. x and y are normalised to the
  display and multiplied by 10,000. dt is milliseconds since the stroke started.
  Points arrive about every 8 ms, so the pointer is sampled at about 120 Hz
  **[reference]**.
- A *mark* is one or more strokes grouped together. It has a bounding box
  normalised to the display (`boxNorm`), a shape (`circle`), and a plain-language
  description such as "circled region x 0.28–0.87, y 0.05–0.55". Mark 1 groups
  three strokes drawn over 6.5 s **[reference]**.
- There is one key frame per mark, taken at the end of the mark. Two marks 4 s
  apart share frame 7. Each key frame carries `said`: the words spoken since the
  previous key frame **[reference]**.
- Everything is timed in ms from the start of the session. The video's own start
  is stored as an offset (`videoOffsetMs`) **[reference]**.

The design does not copy names or branding. The folder, files and schema below are
our own.

## 2. Screen capture

### Recommendation

Use **ScreenCaptureKit in-process, through the `screencapturekit` Rust crate**
(v11, with feature `macos_15_0`). Set **macOS 15 as the minimum** for Annotate.
Write the MP4 with `SCRecordingOutput`. Exclude our own app with an
`SCContentFilter` that names our app. This becomes a fourth stage,
`src-tauri/src/screen.rs`, separate from the other three, with the same
`start`/`stop` shape as `audio.rs`.

Evidence:

- `screencapturekit` 11.1.2 was released 2 Nov 2026 and has 3.5M downloads. Its
  README lists display, window and app capture, `with_excluding_windows`,
  `SCRecordingOutput` direct-to-file recording behind `macos_15_0`, and microphone
  capture **[verified, crates.io README]**.
- The Command Line Tools SDK on this machine includes `ScreenCaptureKit.framework`,
  with `SCRecordingOutput.h`, `excludingWindows`, `showsCursor` and
  `captureMicrophone` **[verified]**. No Xcode is needed.
- A 10-line Swift probe built with plain `swiftc` (2.8 s) called
  `SCShareableContent` and listed 1 display and 15 windows **[verified]**. So a
  Swift sidecar is possible as a fallback, also without Xcode.

### Alternatives considered

| Option | Verdict | Why |
|---|---|---|
| `screencapturekit` crate, in-process | **Chosen** | Pure Rust, maintained, covers recording output and exclusion. Matches the stack. No second process or IPC protocol. |
| Swift sidecar (`swiftc` binary, JSON over stdio) | Fallback | First-party API with no binding gaps, and it compiles here. Costs a second language, a build step, a bundled binary that must be signed later, and a process lifecycle. Use it only if the crate misses something we need. |
| `objc2-screen-capture-kit` (raw generated bindings) | Rejected | Already in the `objc2` family that `insert.rs` uses, but every call is `unsafe` with manual block and delegate code. That is more code for the same result. |
| `scap` crate | Rejected | 0.0.8, last released Aug 2025, 69k downloads **[verified]**. Cross-platform wrapper with less control over filters and recording output. |
| `CGDisplayStream` / `CGWindowListCreateImage` | Rejected | The older APIs. Apple has deprecated them in favour of ScreenCaptureKit **[inferred]**. No hardware encoder path. |
| Shell out to `screencapture -v` | Rejected | Cannot exclude our overlay and has no frame timing. ffmpeg is not installed either **[verified]**. |

### Permission flow (TCC Screen Recording)

- Check with `CGPreflightScreenCaptureAccess()` and ask with
  `CGRequestScreenCaptureAccess()`. Settings shows the result as Allowed or Not
  allowed, with a button that opens System Settings → Privacy & Security → Screen
  & System Audio Recording.
- **Dev builds lie about this.** The probe above, run from a shell inside another
  app, got `CGPreflightScreenCaptureAccess=true` without dictate ever being
  granted **[verified]**. macOS gives the permission to the *responsible* process,
  which is the terminal or IDE that ran `pnpm tauri dev` **[inferred: standard
  TCC behaviour; it explains the probe result]**. Test permission only from a
  bundled `.app` started from Finder.
- Unsigned rebuilds can lose the grant, because TCC keys it to the code signature
  **[inferred]**. `spike-results.md` already records the same problem for
  Accessibility.
- Recent macOS versions ask the user again from time to time whether an app may
  keep recording the screen **[inferred, recalled]**. The app must handle a grant
  that disappears: if the check fails at start, show the Settings link and do not
  start.
- Missing permissions go into the package (`missingPermissions`), like the
  reference app does **[reference]**. An agent can then tell "no video" apart from
  "nothing happened".

### Excluding our own windows

- Default: build `SCContentFilter(display, excludingApplications: [dictate])`.
  Find our app in `SCShareableContent.applications` by process id. This removes
  the overlay, ink window, floating bar and settings window in one rule, including
  windows created after recording starts **[inferred from the API shape; spike S3
  confirms]**.
- "Show in screen recordings" on: use a filter with no exclusions. Useful for
  demos.
- Do not use `NSWindow.sharingType = .none` for this. Its effect on
  ScreenCaptureKit has changed across macOS versions **[inferred, recalled]**. The
  filter is the documented method.
- Ink still appears in the key frames either way, because we draw it into the
  frames ourselves after recording (section 5). With the default setting, the
  video stays clean and the frames show the marks. The reference app behaves the
  same way: its toggle was off, but the frames show ink **[reference]**.

### Performance settings

- Record **one display**: the one under the pointer when recording starts. This
  matches the reference (`target.mode = display`) **[reference]**.
- Record at native pixel size up to 2560 px on the long edge, 15 fps, H.264 with
  the hardware encoder through `SCRecordingOutput`. Text has to stay readable for
  an agent, and screen content rarely needs 30 fps. Spike S4 measures CPU and file
  size at 15 and 30 fps **[inferred settings; numbers pending]**.
- Show the cursor (`showsCursor = true`), because pointing is part of what the
  user is saying.

## 3. Drawing overlay

### Window setup

- One transparent, borderless window covers the recorded display, including the
  menu bar. The reference strokes reach y = 0 **[reference]**.
- It sits above everything else (screen-saver level), on every Space, and over
  full-screen apps (collection behaviour `canJoinAllSpaces | fullScreenAuxiliary
  | stationary`).
- It ignores mouse events by default (`set_ignore_cursor_events(true)` in Tauri
  2) and never takes focus.
- It must be an **`NSPanel` with the `nonactivatingPanel` style**, not a plain
  `NSWindow`. Clicking a normal window of an Accessory-policy app still activates
  the app, and that would steal focus from the app being annotated **[inferred;
  spike S1]**. Either convert the Tauri window with `tauri-nspanel` or swap the
  class with `objc2` (already a dependency). The floating bar needs the same
  treatment.
- Ink is drawn with a `<canvas>` in the overlay's webview. Each stroke stays solid
  while it is being drawn, holds for about 1.5 s after it ends, then fades out
  over 0.5 s. Fading is display only; the recorded stroke data keeps everything.

### Two ways to draw, neither steals focus

**Hold to draw (the main path).** The overlay stays click-through the whole time.
While the shortcut is held, a Rust thread reads the global pointer position at
120 Hz with `CGEvent::location` from `core-graphics` (already a dependency). It
sends the points to the overlay as Tauri events and appends them to the stroke.
No click and no focus change are involved. The reference app says "no click
needed" and samples at about 120 Hz **[reference]**, which fits this approach.
Holding Shift also draws in the reference app; leave that out of v1.

**Tap or hold.** The same shortcut starts, stops and draws, so key-down and key-up
have to be told apart:

```
recording, shortcut Pressed  -> open a provisional stroke, start the pointer poll
shortcut Released:
  held < 250 ms and pointer moved < 6 pt  -> it was a tap: discard the stroke, stop recording
  otherwise                               -> close the stroke, keep it
```

`tauri-plugin-global-shortcut` delivers both Pressed and Released for ⌥⇧Space on
this machine **[verified in docs/state.md]**. For ⌥⇧S this is **[inferred]**;
spike S2 checks it. Key auto-repeat must not open a second stroke: ignore Pressed
while one is open.

**Pen mode (drag to draw).** Clicking the pen on the floating bar turns off
click-through on the overlay (`set_ignore_cursor_events(false)`). Because the
overlay is a non-activating panel, mouse-down draws without activating dictate.
Esc is registered as a global shortcut only while pen mode is on, and
unregistered when it ends. The overlay is never the key window, so it would not
receive Esc otherwise. Clicking the pen again also ends pen mode. If anything
goes wrong, the overlay must go back to click-through. A stuck overlay that
blocks clicks on the whole screen is the worst failure this feature can have, so
add a 60 s pen-mode timeout as a backstop.

### Floating bar

A small non-activating panel shows the elapsed time, the pen button, a stop
button and a microphone level. It is excluded from the recording by the same
filter. It reuses the overlay pattern already in `tauri.conf.json`:
`focus: false`, `alwaysOnTop`, `skipTaskbar`.

### Coordinates

Points are stored in display points, normalised to 0–1 with a top-left origin,
which is CG's global space shifted to the display's origin. The schema uses
integers ×10,000, like the reference **[reference]**. Normalised values do not
change when the frames are scaled, so the contact sheet, the cropped frames and
the agent all read the same numbers.

## 4. Audio and transcript

**Reuse `audio::start` / `audio::stop` (PR #1) as they are.** They already produce
16 kHz mono f32, which is the repo's one audio format. A 5-minute recording is
4.8M samples, about 19 MB in memory, which is fine.

**One conflict must be designed out [verified in source].** `audio.rs` keeps one
global recording. `start()` is idempotent, so a second start while recording
returns Ok and keeps the same buffer. If someone dictates with ⌥⇧Space during an
Annotate recording, the dictation's `stop()` would take Annotate's audio and end
its capture. Rule for v1: **while Annotate is recording, the dictation shortcut
does nothing** except show a hint in the floating bar. Later, `audio` can hand out
a capture handle per caller. That is a refactor, not something v1 needs.

**Timestamps.** Add `transcribe::transcribe_timed(samples) -> Transcript` next to
the existing `transcribe`. It runs the same `whisper-cli` with `-ojf -ml 1 -sow`
and parses `transcription[].offsets.{from,to}` (ms) and `text` into words. The
existing function stays as it is, so the stages remain separate.

Measured on this machine, 53.7 s of `say` speech, whisper.cpp 1.9.5, word-level
JSON **[verified]**:

| Model | Wall time | Output |
|---|---:|---|
| base.en | 1.59 s | 168 word segments with ms offsets, last word ending at 53,480 ms |
| small.en | 2.73 s | same |

So a 1-minute recording transcribes in about 2–3 s. That is acceptable at stop.
Longer recordings scale roughly linearly **[inferred]**. Segments (sentences) are
rebuilt from the words: split at sentence-ending punctuation or at a pause of
700 ms or more. These segments are the "speech beats" used to pick frames.

**One clock.** Session time 0 is the moment the start tap is handled
(`Instant::now()`). Every stream records its offset from it: the audio start
(when `audio::start` returns), the video's first frame time, stroke times and
click times. The reference app does the same with `videoOffsetMs` **[reference]**.
Lining up audio and video is a real risk (R4). The fallback is ScreenCaptureKit's
own microphone capture (`captureMicrophone`, macOS 15), which puts audio on the
video's clock **[verified in header; behaviour inferred]**. The cost is a second
path for microphone audio.

## 5. The package

### Folder

```
~/Library/Application Support/com.kelechiodom.dictate/annotations/<yyyy-mm-dd_hh-mm-ss>/
  annotation.json      one file: the whole session
  summary.md           the text that is pasted
  recording.mp4
  audio.wav            16 kHz mono PCM16 (what whisper read)
  frames/01.png …      key frames with ink, long edge ≤ 1568 px
  frames-full/01.png … native resolution, with ink
  contact-sheet.png    grid of frames/, numbered, long edge ≤ 1600 px
```

The 1568 px limit on `frames/` follows the image-size guidance of common chat
models **[inferred; check against the current Claude and OpenAI docs before
fixing it]**. The reference contact sheet is 1600 px wide **[reference]**.
Cropping to the active windows, which the reference app does, is a v2 item. v1
frames are the full display.

### Marks

A mark is one or more strokes grouped together. A new stroke joins the previous
mark if it starts within 2.5 s of that mark's last stroke ending **and** its
bounding box overlaps or touches the mark's box. Otherwise it starts a new mark.
This reproduces the reference's three-stroke mark 1 **[reference: strokes 0–2 in
4.9–11.5 s]**. For shape, v1 uses `circle` when the start and end points are
within 15% of the box's diagonal, and `stroke` otherwise. Each description is
generated from the numbers, for example `circled region x 0.28–0.87, y 0.05–0.55`.

### Choosing key frames

The frames are the most important part of the package. Rules, applied in order:

1. **One frame per mark**, taken 100 ms before the mark ends, when the ink is
   finished but the screen has not yet moved on.
2. **Merge** marks that end within 1.5 s of each other into one frame. That frame
   lists all of their mark ids (the reference's frame 7) **[reference]**.
3. **Speech beats with no mark**: if a speech segment ends more than 4 s away from
   any chosen frame, add a frame at the end of that segment. This covers "look at
   this" said while pointing without drawing.
4. **Clicks** (optional in v1): if a gap longer than 8 s with no frame contains a
   click, add a frame 300 ms after the click, when the UI has reacted.
5. **Limit**: at most 12 frames. When over the limit, drop speech-beat frames
   first, then click frames. Never drop mark frames.
6. **`said`** for frame *n* = the words whose start time falls between frame
   *n−1* and frame *n*. Words after the last frame go to the last frame.

These rules are pure functions over `(strokes, words, clicks, duration)`. They are
the part to unit-test hardest. The fixtures should be made-up timelines, not the
owner's real capture.

**Pulling frames from the video.** Use `AVAssetImageGenerator` with zero time
tolerance on `recording.mp4`, after recording ends. Frame times are only known
after transcription, so one path that runs after recording covers every rule.
Call it through `objc2-av-foundation`. If the bindings get in the way, use a small
`swiftc` helper (spike S5). Rejected alternative: keeping recent frames in memory
during recording. That holds IOSurface buffers that ScreenCaptureKit needs back,
and costs memory for frames we may never use.

**Drawing the ink in.** Draw each frame's strokes with `tiny-skia` (anti-aliased,
red `#FF3B30`, 4 pt width scaled to the frame). Draw all strokes that ended at
most 2 s before the frame, so the frame matches what the user saw. Number badges
on the contact sheet use `ab_glyph` and a font bundled in the app.

### `annotation.json` (v1 schema)

```jsonc
{
  "schema": "dictate.annotation/1",
  "id": "2026-11-14_14-56-36",
  "startedAt": "2026-11-14T14:56:36.159Z",
  "durationMs": 49430,
  "display": { "id": 1, "frame": { "x": 0, "y": 0, "w": 1512, "h": 982 }, "scale": 2 },
  "frontmostApp": { "name": "…", "bundleId": "…" },        // at start
  "video": { "file": "recording.mp4", "offsetMs": 140, "width": 2560, "height": 1662, "fps": 15 },
  "audio": { "file": "audio.wav", "offsetMs": 35, "sampleRate": 16000 },
  "transcript": {
    "model": "ggml-small.en", "text": "…",
    "segments": [{ "t0": 1319, "t1": 3564, "text": "…" }],
    "words":    [{ "t0": 1319, "t1": 1964, "text": "Here" }]
  },
  "strokes": [{ "t0": 4956, "t1": 5826, "pts": [6898, 106, 0, …] }],   // x,y ×10^4 normalised; dt ms
  "marks": [{ "id": "m1", "strokes": [0, 1, 2], "t0": 4956, "t1": 11513,
              "box": [0.1338, 0.0, 0.9102, 0.8948], "shape": "circle",
              "description": "circled region x 0.13–0.91, y 0.00–0.89" }],
  "clicks": [{ "t": 6456, "x": 0.6938, "y": 0.0182, "button": "left" }],
  "frames": [{ "n": 1, "t": 11413, "reason": "mark", "marks": ["m1"],
               "file": "frames/01.png", "full": "frames-full/01.png", "said": "…" }],
  "contactSheet": "contact-sheet.png",
  "missingPermissions": []
}
```

Compared with the reference, this joins `capture.json` and `scrawl.json` into one
file, drops the 120 Hz cursor track and the window-list snapshots (v2), and uses
plain `box` instead of `boxNorm`. All times are ms since the session started, and
all positions are normalised to the display. These two rules are what let an agent
line up the frames, words and marks without guessing.

### `summary.md` and "copy, ready to paste"

`summary.md` is the clipboard text. It has to be useful even if the agent never
opens a file, and it gives the absolute path so agents that can read files do:

```
[annotation 0:49 · <frontmost app> · /Users/…/annotations/2026-11-14_14-56-36]
Open contact-sheet.png and frames/ in that folder first: the marks only make sense on the images.
Positions are fractions of the screen, x left→right, y top→bottom. Full data: annotation.json.
- #1 (0:11) circled region x 0.13–0.91, y 0.00–0.89 — “this toggle should be on the right …”
- #2 (0:21) circled region x 0.20–0.74, y 0.18–0.70 — “and this list is too dense …”
- #3 (0:29) “the error at the bottom is cut off …”
```

The reference puts either the speech or the region on each line **[reference]**.
This version puts both on one line, because an agent needs to know what was
circled *and* what was said about it.

What goes on the clipboard, written with the `NSPasteboard` code from PR #3:

- **Default:** one item with `public.utf8-plain-text` = `summary.md`. This works
  in terminal agents (Claude Code, Codex), which can open the path, and in any
  text field.
- **Floating-bar action "Copy image":** the contact sheet as `public.png`, for web
  chats that cannot read local paths.
- Do **not** put text and an image in one item by default. Each app chooses which
  representation it takes, so a mixed item can paste only the image into one app
  and only the text into another **[inferred; spike S6]**.
- Annotate has no clipboard restore. The user asked for the copy and pastes it
  later, unlike dictation's temporary paste.

## 6. Risks, ranked, each with the smallest spike that settles it

| # | Risk | Why it ranks here | Smallest spike | Pass condition |
|---|---|---|---|---|
| R1 | The overlay or floating bar takes focus or activates dictate in pen mode, so typing and clicks go to the wrong app | It breaks the whole feature, and it is a known AppKit trap **[inferred]** | `examples/overlay_focus.rs`: one full-screen Tauri window converted to a non-activating `NSPanel`, with click-through toggled every 3 s. Type into TextEdit and drag on the overlay while it runs. | TextEdit keeps the caret and the typed characters in both states. `NSApp.isActive` stays false. Clicks pass through when click-through is on. |
| R2 | ⌥⇧S does not reliably deliver Released, or auto-repeats Pressed, so taps and holds get confused | The whole interaction depends on it. Only ⌥⇧Space has been checked **[verified for Space, inferred for S]**. | Register ⌥⇧S in the dev app and log each event with `Instant` times for 20 taps and 10 holds of 2 s | 30/30 classified correctly at 250 ms. Each hold gives exactly one Pressed. |
| R3 | Screen Recording permission works in dev but not in the bundled app, or is lost on every rebuild | Dev runs get the terminal's permission **[verified]**, so every test run hides this | `pnpm tauri build`, open the `.app` from Finder, call `CGPreflight`/`CGRequest`, rebuild, check again | Permission is requested for dictate itself. Write down whether a rebuild keeps it. |
| R4 | Audio from cpal and video from ScreenCaptureKit drift or have different start delays, so `said` is attached to the wrong frame | Wrong words on a frame are a quiet failure the user will not notice | 20 s recording while clapping on screen and on mic, 3 times (a click sound plus a flashing window works). Compare the clap's time in `audio.wav` with the flash frame in the MP4. | Offset error under 100 ms. If it fails, switch to SCK microphone capture. |
| R5 | Excluding our app with `excludingApplications` misses windows created after start, or the floating bar shows up anyway | The default video would show our UI | Record 10 s, opening the overlay and floating bar 3 s in. Pull a frame at 5 s. | Neither window is in the frame. |
| R6 | Recording CPU or energy cost is too high for a daily tool | Owner's machine and battery | Record 60 s at 15 and 30 fps at 2560 px. Note `top` CPU %, MP4 size and dropped frames. | Below 15% of one core at 15 fps **[target, not a measurement]**. |
| R7 | Pulling frames from the video is slow or inaccurate through `objc2-av-foundation` | It is on the stop-to-paste path | Pull 12 frames at chosen times from the R6 MP4 and time it | Under 1 s for 12 frames. Each frame within 1/fps of the requested time. |
| R8 | Paste formats are handled differently by different targets | It decides what "ready to paste" means | Put (a) text only, (b) PNG only, (c) a text+PNG item on the pasteboard, then paste into Claude Code, a web chat and TextEdit | Write down the result for each target. Text-only works everywhere. |
| R9 | Dictation during Annotate takes Annotate's audio | Certain if not designed out **[verified in source]** | No spike: the rule in section 4 plus a test | — |
| R10 | Long recordings (10 min or more): memory, whisper time, MP4 size | Rare use, but it should fail cleanly | Stop at a hard 10-minute cap in v1. The floating bar warns at 9 minutes. | — |

Spikes S1–S6 in the build plan below refer to R1, R2, R3/R5, R6, R7 and R8.

## 7. Build plan

Order: the spikes that can kill the design go first, then one PR per stage, so
each one can be measured alone. That is the repo convention. Each PR includes a
plain-English walkthrough, per AGENTS.md. **PRs #1–#4 (audio, transcribe, insert,
silent takes) must merge before task 2.** Tasks 1 and 3 do not depend on them.

1. **Spikes S1 + S2 (focus and shortcut).** Throwaway examples, with results in
   `docs/spike-results.md`. Acceptance: the R1 and R2 pass conditions, with
   numbers. If R1 fails with `NSPanel`, stop and redesign pen mode before going
   further.
2. **Annotate session without video.** ⌥⇧S tap to start and stop, a state machine
   (`Idle → Recording → Packaging → Idle`), a session folder with `audio.wav` and
   a minimal `annotation.json` (times, duration, transcript). Dictation shortcut
   ignored while recording. Acceptance: tap, speak for 10 s, tap → the folder
   exists, the WAV is about 160k samples, and dictation during a recording does
   not change it. Validation: unit tests for the state machine and the
   dictation-blocking rule. One manual run logged.
3. **`screen.rs` stage (S3/S4/S5 included).** `screen::start(display) / stop() ->
   VideoInfo` using `screencapturekit` 11 with `macos_15_0` and
   `SCRecordingOutput`, our app excluded, permission check and request.
   Acceptance: a 30 s MP4 plays in QuickTime, our windows are absent (R5), the
   bundled `.app` permission behaviour is recorded (R3), and CPU, size and
   dropped frames at 15 fps are in `spike-results.md` (R6). Callable alone from
   an example binary.
4. **Hold-to-draw ink overlay.** A non-activating click-through panel over the
   recorded display, the 120 Hz pointer poll while the shortcut is held, the
   tap-or-hold rule, ink that fades, strokes stored normalised with session
   times. Acceptance: draw three circles while TextEdit keeps focus and typed
   text lands. The strokes in `annotation.json` match on screen within 1%.
   Validation: unit tests for tap-or-hold classification and coordinate
   normalisation, including a second display at a non-zero origin.
5. **Floating bar and pen mode.** Elapsed time, pen, stop. Pen turns off
   click-through, Esc is registered only during pen mode, there is a timeout
   backstop. Acceptance: drag to draw. Esc and the pen button both return to
   click-through. Clicking the bar never activates dictate (R1 check repeated in
   the real app).
6. **Timed transcription.** `transcribe::transcribe_timed` with `-ojf -ml 1 -sow`,
   a word parser, and segments rebuilt from the words. Acceptance: on the
   `say`-generated fixture, word times increase, the last word ends within
   500 ms of the audio length, and the 60 s timing is recorded. The existing
   `transcribe` is unchanged.
7. **Package builder (pure core).** Mark grouping, key-frame choice, `said`
   assignment, `annotation.json` and `summary.md` from in-memory data. No
   AppKit. Acceptance: table tests that cover a mark grouping three strokes, two
   marks merged into one frame, a speech-only frame, the 12-frame limit, and
   words after the last frame. JSON round-trips through serde.
8. **Frames and contact sheet (S5 included).** Frames pulled from the MP4 (R7),
   ink drawn in with `tiny-skia`, `frames/`, `frames-full/` and
   `contact-sheet.png` with numbered badges. Acceptance: for a 60 s recording
   with 4 marks, stop-to-package takes under 5 s total, with the timing logged,
   and each frame's ink sits within 1% of the matching mark box.
9. **Copy, ready to paste (S6 included).** Text summary on the clipboard at stop,
   "Copy image" on the floating bar, a macOS notification saying it was copied.
   Acceptance: the R8 matrix is recorded, and pasting into Claude Code gives an
   agent that opens the contact sheet without being asked.
10. **Settings and history.** An Annotate section with the toggle, the shortcut,
    permission status with an "Open System Settings" button, and "Show in screen
    recordings". Recordings listed in history with "Reveal in Finder" and
    "Copy again". This depends on the settings window existing.

Out of scope for v1: cropping to the active windows, window-list snapshots,
recording several displays, a replay buffer, "Send to…" a specific session, and
Shift-to-draw.

### Builder task description for task 1 (ready to paste)

**Goal.** Settle the two Annotate risks that could kill the design: focus safety
of a non-activating, click-through overlay, and tap-or-hold detection on ⌥⇧S.
Throwaway code, results only.

**Context.** `docs/annotate-design.md` §3 and §6 (R1, R2). The existing overlay
config is in `src-tauri/tauri.conf.json`. Activation policy is Accessory in
`lib.rs`. `objc2` and `objc2-app-kit` are already dependencies on the PR #3
branch. Pick `tauri-nspanel` or an `objc2` class swap, and say why.

**Acceptance criteria.**
- A full-screen overlay on the main display, at screen-saver level, on all Spaces,
  with click-through toggled every 3 s for 60 s. While it runs, typing into
  TextEdit always lands in TextEdit. Dragging on the overlay while it accepts the
  mouse logs points and never makes dictate active (log `NSApp.isActive`).
- ⌥⇧S registered and each Pressed/Released logged with times. 20 taps and 10
  two-second holds, each classified with the 250 ms / 6 pt rule. Report the
  confusion counts and whether auto-repeat sent extra Pressed events.
- Results with numbers in `docs/spike-results.md`. Spike code removed or kept
  under `examples/`. No change to the dictation path.

**Validation.** `cargo check`, `pnpm build`, the two manual runs above done by the
owner or recorded on screen, and the numbers in the doc. Open a PR. Do not merge.
