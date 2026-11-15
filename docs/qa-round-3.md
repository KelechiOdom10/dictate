# QA round 3 walkthrough

Settings starts hidden. Once the stored theme has loaded (or loading has failed),
React applies the theme in a layout effect, then announces readiness after
settings have settled and the navigation listener is attached. Readiness never
waits for an animation frame: a hidden native webview may suspend frames until
it is shown. Rust shows the window only if launch or activation requested it.
This prepares the theme before the first visible frame and
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
Opening a notice's Settings action preserves its recovery instructions in
Settings and queues the relevant page until the window is ready.
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

Independent review also moved overlay visibility effects and dismissal
generations onto the main thread. A delayed dismissal checks the current idle
state before fading or hiding, and an error show remains valid across the
worker's transcribing-to-idle transition. Discard schedules its fade before
releasing the take lock and joins the discarded decoder off the UI thread.
The shortcut recorder enters capture state before awaiting native confirmation,
so Tab, blur and navigation can cancel a pending request without a late success
reopening it. Wrapped segmented controls track both axes of their selected row.

The first native attempt left Settings hidden. The earlier readiness gate had
a circular dependency: the hidden window waited for two animation frames before
allowing Rust to show it. A suspended-frame browser regression fails that earlier code and
checks that readiness now completes with the stored theme, or the fallback
theme when settings loading fails. This is regression evidence, not a native
pass. The native log now records Settings show requests, webview readiness and
show success/failure to make the next owner check diagnosable.

The earlier wrapped-History screenshot deliberately injected a 140px maximum
width and used an old saved-shortcut fixture. Normal History keeps both tabs
on one row at the 720px default window width. The default fixture now matches
Rust's new Annotate shortcut, ⌥⇧A; a separate test preserves an explicitly saved
⌥⇧S setting. Existing user shortcuts are not changed.

## Manual verification

No desktop input was automated. Browser screenshots and tests use an isolated
Chromium harness with mocked Tauri IPC. They prove rendering and command wiring,
not native focus, activation or paste delivery.

1. Quit the existing dictate instance and stop its old dev task yourself first;
   do not run two instances. Run the command below from this reviewed worktree.
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
10. Complete one real Annotate session: record, draw, stop, then inspect the
    copied summary and saved package/contact sheet. Generated-video tests do
    not establish native capture, drawing or focus behavior.

The owner can launch this checkout without reusing another worktree's frontend
server. These commands have not been used to launch a native instance by the
reviewer; port 1437 must be free, and Vite will fail rather than choose another:

```sh
cd /Users/kelechiodom/.capy/worktrees/jam_01M7GEN019XF8W6HQQD5ADDRNJ/dictate-settings-visibility
export PATH="$HOME/.nvm/versions/node/v24.22.2/bin:/opt/homebrew/bin:$PATH"
source "$HOME/.cargo/env"
export CARGO_TARGET_DIR="$HOME/Projects/dictate/target"
pnpm tauri dev --no-watch --config '{"build":{"devUrl":"http://localhost:1437","beforeDevCommand":"pnpm dev --host 127.0.0.1 --port 1437 --strictPort"}}'
```

For the separate silent-launch check, append `-- -- --autostart` to the same
command after stopping its previous instance. No TCC reset or settings-file
change is part of this handoff.
