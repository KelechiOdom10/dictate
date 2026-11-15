# QA round 3 walkthrough

Settings starts hidden. Once the stored theme has loaded (or loading has failed),
React waits for a paint and tells Rust it is ready. Rust shows the window only
if launch or activation requested it. This avoids the initial white frame and
preserves silent login launch. Closing hides Settings; reopening from the Dock,
Finder or Spotlight requests it again. Its default size is 720 × 580; content
scrolls rather than shrinking the controls.

The overlay accepts pointer input without becoming keyboard-focusable. Finish
uses the same stop transition as the shortcut, under the same lock. Discard
stops capture and drops the transcription worker without delivery or a History
entry. Neither action can start a take when the recording has already ended.

Before paste, Accessibility must report a focused, writable text field, text
area or combo box. Missing, timed-out or non-text targets fall back to copying.
The delivery stage returns what it actually did, and History and the capsule
use that result. Accessibility cannot prove that a target app consumed ⌘V;
focus can still change between checking and sending. Test the native apps below.

The three Output choices use the existing paste-method and restore-clipboard
settings together. Paste restores the previous clipboard after its existing
300 ms grace period. Copy only and Paste and keep leave the transcript there.
No new settings format or migration is needed for this single-user app.

Error capsules show a title and Settings action, then fade out after six seconds.
The underlying error remains in Settings and the tray until dismissed or fixed.
Closing Settings hides an idle capsule; a newly granted screen permission clears
its corresponding error when checked again. Dismissal generations prevent an old
fade from hiding a newer recording. The shortcut recorder shows Press shortcut
and an esc keycap; a failed registration leaves the old shortcut and reports the
conflict on its row. New settings default Annotate to ⌥⇧A; saved shortcuts remain
unchanged. This machine's stored macOS symbolic shortcuts contain no matching
Option+Shift+A binding. Other installed apps can still reserve it.

Motion uses 150–200 ms ease-out transitions: a sliding segmented thumb, sidebar
page crossfade where View Transitions are supported, subtle list/notice entry
and overlay dismissal. Reduced motion keeps the state cues and removes motion.

## Manual verification

No desktop input was automated. Browser screenshots and tests use an isolated
Chromium harness with mocked Tauri IPC. They prove rendering and command wiring,
not native focus, activation or paste delivery.

1. Run `pnpm tauri dev` from this worktree with the shared `CARGO_TARGET_DIR`.
   Open Settings in dark and light. Check that there is no white flash, the
   default window is compact, and every section remains reachable by scrolling.
2. Close Settings. Reopen dictate from Spotlight, then repeat using Dock/Finder.
   Settings should return each time. Launch with `--autostart` separately and
   confirm Settings stays hidden unless there is a startup error.
3. With Screen Recording denied, try Annotate. Check the compact notice, click
   Settings to reach Annotate, and retry without clicking: it should fade after
   six seconds. Close Settings while it is visible: it should disappear. Grant
   permission and return: the old permission issue should clear.
4. Record in hold and toggle modes. Click Finish and confirm exactly one
   transcript. Record again and click Discard: no paste or new History item.
   Keep a text field focused while clicking the overlay and confirm it retains
   keyboard focus. Repeat with a very quick next take during the dismissal fade.
5. Select Paste. Dictate into TextEdit, a VS Code editor and a browser input.
   Confirm text lands and the previous clipboard returns. Focus Finder or a
   non-editable control: dictation should remain on the clipboard, History
   should say Copied, and the capsule should say “Copied — paste it anywhere”.
6. Select Copy only and repeat in an editable field: no insertion, transcript
   remains on the clipboard. Select Paste and keep on clipboard: both insertion
   and a subsequent manual paste should contain the transcript.
7. Click each shortcut row. Check Press shortcut and esc; Escape restores the
   previous chord. Set a new chord. With Hex holding its Annotate chord, try
   assigning that same chord and check the inline conflict and preserved value.
8. Switch sidebar pages and History segments, then repeat with macOS Reduce
   Motion enabled. Check the static selected state in both cases, smooth
   animation when enabled, and no motion when reduced.
9. Repeat a roughly 14-second take. Compare History total with the new
   `audio_stop_ms`, `stop_to_worker_ms`, `debug_save_ms`, inference and restore
   log values. The offline conversion result is in `spike-results.md`; native
   end-to-end improvement remains unmeasured until this check.
