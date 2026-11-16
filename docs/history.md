# History

Settings › History lists every take: what was said, where, with which model and
how long it took. It lives only on this Mac.

## Storage

`src-tauri/src/history.rs` owns one SQLite file, `history.sqlite3`, in the app
data folder next to `settings.json` (bundled SQLite through `rusqlite`). Each take
is one row in `takes`:

| Column | Meaning |
|---|---|
| `text` | The pasted transcript. Empty for skipped takes and failures before transcription. |
| `created_at_ms` | Key-down time, Unix milliseconds. |
| `app` | Name of the frontmost app at key-down (`NSWorkspace`), or null. |
| `model` | Whisper model file stem, e.g. `ggml-base.en`. |
| `audio_ms` | Captured audio length. |
| `inference_ms` | `transcribe::transcribe` wall time; null when transcription did not run. |
| `total_ms` | Key-up to the end of paste (or to the skip/failure). |
| `status` | `pasted`, `skipped` or `failed`. |
| `detail` | Skip reason or the failure's title, fix and cause. |

Audio is never stored. There is no setting for it yet; one would come with
playback.

Schema changes are appended to `MIGRATIONS` and tracked with SQLite's
`user_version`. A file from a newer app is refused rather than downgraded; the
app then runs with history off and says so.

## Retention

`historyRetention` in `settings.json` is `day`, `week` (default), `month` or
`forever`, read with `#[serde(default)]`. Pruning deletes rows older than the
window at startup, after every take and on every history read, so a shorter
window applies as soon as it is saved. Shortening the window in the page asks
first, because it deletes older takes at once. Deleting one take also asks.

## Manual checks (Kelechi only)

Agents must not send keystrokes to real apps, so these need your hands:

1. Run `pnpm tauri dev`, dictate a sentence into Notes, then open Settings ›
   History. The row shows the first line, "Notes" and "Just now"; the detail
   shows the full text, model and three timings.
2. Tap the shortcut without speaking. A "No speech detected" row appears with
   status Skipped and its reason.
3. Revoke Accessibility for dictate and dictate again. The row is Failed, its
   text is kept, and Copy puts it on the clipboard.
4. Search for a word and for an app name; clear the search with the × button.
5. Delete a row (confirm), then switch Keep to 1 day (confirm) and check older
   rows are gone. Quit and relaunch: the list and the setting survive.
