# Settings and menu bar

The settings window changes Rust-owned preferences; the webview never owns the
recording pipeline. `get_settings` reads the current configuration and the actual
macOS login-item state. `save_settings` validates the chord and microphone,
registers the new shortcut, and writes `settings.json` atomically in the app data
directory. A failed write restores the previous shortcut and login setting.
Changes are rejected during a take or transcription. Rust types generate the
checked TypeScript contract; its test catches drift, and Zod parses IPC replies.

The shortcut recorder temporarily unregisters the existing chord, so it can
record that same chord. Escape, loss of focus, navigation and closing settings
restore it. Its button explicitly takes focus because macOS WebKit does not
focus buttons on mouse click. Hold records between press and release. Toggle
records between two presses. Transcription and paste finish before a new take.

Capture selects the saved cpal input device each time it starts. If a saved device is missing, capture uses the default microphone and reports
the fallback in the overlay, settings and tray tooltip. Microphone names
are cpal's identifiers here; two devices with the same name select the first.
The input test opens a separate stream, retains no audio, and stops when settings
lose focus or close. The three pipeline stages remain separately callable and
their sample boundary remains 16 kHz mono f32.

Manual launches open settings. Login launches pass `--autostart` and stay hidden.
Closing settings hides it and keeps the process alive. The app always uses the
Accessory activation policy: no Dock icon, including while settings are open.
This preserves the non-focusing recording overlay; the tray is the way back.
The template waveform uses different shapes for idle, recording and transcribing.

## Design choices

The chosen Ember/Paper direction uses warm surfaces, pill controls, local
Geist and ink actions. General’s Appearance group saves Theme and Mode;
Ember and System are the defaults for older files too. Both windows apply changes
live through settings-changed. System follows macOS appearance changes. Colours, type, spacing and radii live in
`src/tokens.css`. Shared controls and their state stories live in `src/ui`. The recorder adapter
and microphone-test component keep native capture and persistence outside the
library. See [design-system.md](design-system.md) for contracts and commands.

Rejected: browser storage, because Rust must read settings without a webview;
a second store plugin, because atomic JSON covers four fields; a Dock icon,
because dictation should stay in the background; copying reference assets.
Signing, updates and the five placeholder pages remain outside this change.

## Manual checks

1. Open the tray menu: Open Settings, current shortcut and Quit must appear.
2. Record another chord; close settings, focus a text field and try both modes.
   Check the idle, recording and transcribing icons and overlay focus behaviour.
3. Select a microphone, test its level, stop the test, close and reopen settings.
   Disconnect that device and verify capture uses the default microphone and
   reports why; choose System default to save that preference.
4. Toggle login launch and verify its macOS Login Items entry. Disable it after
   testing, or log out/in to test an enabled entry with the installed app.
5. Restart the app and confirm preferences persist. Check light and dark mode,
   keyboard focus, all placeholders, and error recovery if a shortcut is taken.

`pnpm test:ui` builds and exercises the frontend in Chromium with the Tauri IPC
edge replaced by a fixture. It covers saving, rejected shortcuts, load failures,
placeholders, light/dark and a narrow viewport. It does not prove OS hotkey or
login behaviour. Install its browser once with `pnpm exec playwright install chromium`.

## Recovery

Settings fields default independently, unknown fields are tolerated, and version 0
(files without a version) migrates to version 1 in memory. The next save writes
the migrated version. A newer version is retained when a compatible file loads.
Invalid JSON or a bad shortcut is moved to `settings.json.bad-<timestamp>` beside
the original before defaults are used. A backup failure is reported and the
original remains untouched. Defaults are not written until a setting is saved.

A shortcut another app owns leaves dictate running without a shortcut. Settings
opens even on a login launch so another chord can be chosen. Errors are retained
in Rust, so a webview that loads after startup still receives them. The overlay
shows a short message; settings and the tray tooltip have the full detail.
Dismiss in settings or start another take to clear the message. Saving preferences
emits `settings-changed` to refresh the overlay shortcut without restarting.

Recovery checks: quit dictate, replace its app-data `settings.json` with `{broken`,
and launch. Confirm settings opens with defaults, an error appears, and the
original contents survive in a `.bad-` file. With dictate quit, put a chord owned
by another app in the file, launch and choose an available chord in settings.
Confirm dictation then works and the overlay label updates. Repeat with
`--autostart`; recovery must still open settings. No automated test claims to
prove macOS shortcut ownership or microphone permissions.
