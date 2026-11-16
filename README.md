# dictate

Local dictation: hold a key, speak, and the transcript lands in the app that
already had focus.

The pipeline spike and the first settings surface are in development.

- `docs/spike.md` — the question and the seven checks it has to pass
- `GUIDE.md` — the build order, with the failures you will hit
- `AGENTS.md` — layout and conventions
- `docs/settings.md` — settings runtime, design choices and manual checks

Pipeline contracts live in `src-tauri/src/`: `audio` → `transcribe` → `insert`.

## Building

The Parakeet engine (`transcribe-cpp`) compiles ggml with CMake, so the build
machine needs it: `brew install cmake`. Whisper models still run through
Homebrew's `whisper-cli` (`brew install whisper.cpp`).

## Shipping it to another Mac

```sh
pnpm install
pnpm icons            # build the app icon set from the draft the app ships
pnpm release:mac      # arm64 release build -> dist/dictate-0.1.0-arm64.dmg
```

`pnpm release:mac` refuses nothing but tells you when the build is ad-hoc
signed, which means the other Mac re-grants Microphone and Accessibility after
every new build. To sign with a certificate of your own instead:

```sh
export DICTATE_SIGNING_IDENTITY="dictate local signing"
pnpm release:mac
```

The certificate is created once by hand in Keychain Access; the exact clicks,
the release sizes and the manual checks are in [docs/release.md](docs/release.md).
The build is Apple Silicon only and needs macOS 15 or later on both Macs.

## Installing on a second Mac

This section is written for the person who receives dictate, not for a
developer. It takes about five minutes, plus a model download.

**1. Put the app in Applications.** Double-click `dictate-0.1.0-arm64.dmg`. A
window opens with the dictate icon and an Applications folder. Drag the dictate
icon onto the Applications folder, then close the window and click the eject
arrow next to "dictate" in the sidebar of Finder.

**2. The first open needs one extra step.** macOS blocks apps that did not come
from the App Store. Open Applications, then **right-click (or Control-click)
dictate and choose Open**, then click **Open** in the dialog. From now on a
normal double-click works.

If macOS does not offer that dialog, or you already double-clicked and got
"Apple cannot check it for malicious software": open **System Settings ›
Privacy & Security**, scroll down to the line about dictate, and click **Open
Anyway**. You have about an hour after the message before that button goes
away, and trying to open dictate again brings it back.

**3. Say yes to the microphone.** The first time dictate records, macOS asks
whether dictate may use the microphone. Click **Allow**. If you clicked Don't
Allow by mistake, fix it in **System Settings › Privacy & Security ›
Microphone** by switching dictate on.

**4. Turn on Accessibility so dictate can type into other apps.** dictate
cannot ask for this one itself, so it says "Accessibility access needed" the
first time you dictate. Open **System Settings › Privacy & Security ›
Accessibility**, click the **+**, choose dictate from Applications, and make
sure its switch is on. The switch takes effect straight away; you do not need
to restart dictate.

**5. Download a speech model.** dictate has no speech model built in. Open
dictate's window from the menu-bar icon, go to **Models**, and start the
Parakeet download — it is about 700 MB, so give it a few minutes. Until it
finishes, dictation tells you the model is missing.

**6. Use it.** Click into any text box, then hold **Option-Shift-Space**, speak,
and let go. The text appears where your cursor was. The shortcut is shown,
and can be changed, in dictate's **General** settings.

**Screen Recording** is only needed if you use Annotate (Option-Shift-A), which
records your screen with notes on it. Until you allow it in **System Settings ›
Privacy & Security › Screen Recording**, Annotate asks for it the first time and
dictation is unaffected.
