# dictate

Local dictation: hold a key, speak, and the transcript lands in the app that
already had focus.

Currently a **spike** — the pipeline is proven before anything is designed or
built around it.

- `docs/spike.md` — the question and the seven checks it has to pass
- `GUIDE.md` — the build order, with the failures you will hit
- `AGENTS.md` — layout and conventions

Pipeline contracts live in `src-tauri/src/`: `audio` → `transcribe` → `insert`.
