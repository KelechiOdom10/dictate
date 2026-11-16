# Recording overlay

The native overlay stays hidden until dictation starts or a recoverable error
needs attention. It cannot become a key window, ignores cursor events and uses
`orderFrontRegardless` on macOS instead of the key-window show path. Its webview
and window are transparent. Only the shared `RecordingCapsule` paints a surface.

Capture computes mono RMS on the same input block it retains for transcription.
During a take Rust sends one float on `recording-level` every 40 ms; samples never
cross IPC. The webview validates the float, applies a square-root display curve
and updates the meter. Its elapsed timer resets for each take; timer updates are
outside the status announcement so screen readers do not announce every second.

After transcription and paste succeed, Rust emits `done` for 900 ms. Empty,
short or quiet takes emit `skipped` for 1800 ms without pasting. Each error
source builds a `Problem` (`src-tauri/src/problem.rs`): a short title and its own
fix line, shown as written in the capsule and Settings, plus a `detail` cause for
logs and history. `src/problems.fixture.json` holds the real Accessibility,
settings-refresh, no-samples and missing-model problems; a Rust test keeps it in
step and the "Error / real pipeline messages" story renders it. A take id prevents an older completion timer or level worker from
interfering with a newer take. The recording and transcription states still
preserve the secondary microphone-fallback notice.

`overlayPosition` accepts `top` or `bottom`, defaults to bottom for older files,
and survives settings saves. It has no settings control yet. Placement reads the
focused app's window rectangle through Accessibility, selects the screen with
its centre, and places the capsule 24 logical pixels inside that screen's work
area. Each Accessibility call times out after 0.1 s, so a hung app cannot stall
the hotkey, and the lookup runs after the `Runtime` lock is released. If
Accessibility, the focused-window rectangle or the timeout fails, placement falls
back to the primary screen. Resizing preserves the selected edge and
horizontal centre. The webview measures an invisible intrinsic-size copy so a
small previous state cannot constrain the next state; only dimensions cross IPC.

## Manual desktop checks (Kelechi only)

These are unverified by automation. Do not run synthetic input against desktop
apps. The browser tests use an isolated Chromium process, and clipboard tests
use unique test pasteboards.

1. From this worktree run `source "$HOME/.cargo/env"` and `pnpm tauri dev` in a
   foreground terminal. Open Settings, note the configured chord, and close the
   settings window. With your own hands focus a disposable text field in an app
   of your choice. Hold the chord and speak; the capsule must appear while the
   app and its insertion caret retain focus. Release the chord; transcription,
   paste, a short tick and hiding must follow. Type a few characters yourself
   after the pill disappears to confirm the field still owns focus.
2. Put that window on a second display with different scaling, focus it yourself
   and repeat. Check bottom centre, the Dock/menu-bar clearance, transparency
   around the capsule and resizing between all states. While recording, click
   through the pill onto a harmless area of the underlying window yourself:
   the click must reach that app, and dictate must never activate.
3. Tap the chord for less than 0.30 seconds, then hold it silently for a second.
   Both should show the quiet message and hide, with no inserted text. Record a
   new take immediately after completion; the previous timer must not hide it.
4. With dictate quit, back up its app-data `settings.json`, change
   `overlayPosition` to `top`, relaunch and repeat on each display. Check the top
   work-area inset; save another preference and restart to confirm persistence.
   Restore your original file afterwards. Removing the field should use bottom.
5. Check microphone denial, Accessibility denial, a missing model and the saved
   microphone being disconnected. Confirm the fix is understandable and full
   details remain in Settings. Restore permissions/device/model afterwards.
6. Enable macOS Reduce Motion and repeat recording/transcribing. The meter must
   respond without animated interpolation, and the busy state must retain its
   text when the spinner stops. VoiceOver announcements and native WebKit text
   rendering also need a manual pass.

## Automated proof

`cargo test --manifest-path src-tauri/Cargo.toml` covers RMS, retained audio,
settings migration/round-trip, generated contracts and pipeline guards without
opening the real microphone. Four existing hardware checks remain ignored.
`pnpm test:ui` drives the built app against its browser IPC fixture: live levels,
timer reset, completion/skip distinction, invalid appearance events, every state
in four theme combinations and overflow at 320 px. It writes individual capsule
screenshots to `target/overlay/`. `pnpm test:stories` runs stories and axe in all
four combinations. Browser fixtures do not prove native placement, actual event
cadence, focus, click-through or end-to-end microphone/transcribe/paste behaviour.
