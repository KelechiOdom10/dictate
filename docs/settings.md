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

Capture selects the saved cpal input device each time it starts. Missing devices
produce an error rather than silently recording another source. Microphone names
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

The Voicenotes direction uses neutral surfaces, pill controls, calm system type
and a restrained success green. Colours, type, spacing and radii live in
`src/tokens.css`. Page-local rows, recorder and microphone-test components stay
small; a component library and Storybook belong to the next task.

Rejected: browser storage, because Rust must read settings without a webview;
a second store plugin, because atomic JSON covers four fields; a Dock icon,
because dictation should stay in the background; copying reference assets.
Signing, updates and the five placeholder pages remain outside this change.

## Manual checks

1. Open the tray menu: Open Settings, current shortcut and Quit must appear.
2. Record another chord; close settings, focus a text field and try both modes.
   Check the idle, recording and transcribing icons and overlay focus behaviour.
3. Select a microphone, test its level, stop the test, close and reopen settings.
   Disconnect that device and verify the error; choose System default to recover.
4. Toggle login launch and verify its macOS Login Items entry. Disable it after
   testing, or log out/in to test an enabled entry with the installed app.
5. Restart the app and confirm preferences persist. Check light and dark mode,
   keyboard focus, all placeholders, and error recovery if a shortcut is taken.

`pnpm test:ui` builds and exercises the frontend in Chromium with the Tauri IPC
edge replaced by a fixture. It covers saving, rejected shortcuts, load failures,
placeholders, light/dark and a narrow viewport. It does not prove OS hotkey or
login behaviour. Install its browser once with `pnpm exec playwright install chromium`.
