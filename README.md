# dictate

Local dictation: hold a key, speak, and the transcript lands in the app that
already had focus.

The pipeline spike and the first settings surface are in development.

- `docs/spike.md` — the question and the seven checks it has to pass
- `GUIDE.md` — the build order, with the failures you will hit
- `AGENTS.md` — layout and conventions
- `docs/settings.md` — settings runtime, design choices and manual checks

Pipeline contracts live in `src-tauri/src/`: `audio` → `transcribe` → `insert`.
