# How the app and Annotate stacks were integrated

Two stacks were built in parallel from the same `main` in November 2026: the app
stack (#7 → #26) and the Annotate stack (#17 → #24). Each layer landed on `main`
on its own, in dependency order. This page records the four files where the two
stacks disagreed, and how each disagreement was settled, because that reasoning
otherwise survives only in commit messages.

## The conflicting files

| File | App stack | Annotate stack | What landed |
|---|---|---|---|
| `src-tauri/Cargo.toml` | `ts-rs`, `rusqlite`, `ureq`, `sha2`; extra `objc2-app-kit` features for window and workspace access | `screencapturekit`, `tiny-skia`, AVFoundation/CoreMedia/CoreVideo, a release-profile `opt-level` for `tiny-skia` | The union. Feature lists were merged rather than chosen, so `NSRunningApplication`/`NSWindow`/`NSWorkspace` sit beside `NSError`/`NSURL`. |
| `src-tauri/Cargo.lock` | — | — | Regenerated from `main`'s lock per layer, never hand-merged. Each delta is only the new crates, with no version churn. |
| `src-tauri/src/transcribe.rs` | The model became a caller argument, and every failure became a `Problem` with a title and a fix | Word and segment times, `transcribe_timed`, and splitting audio into speech runs before transcription | One whisper-cli runner serves both entry points. `transcribe` and `transcribe_timed` both take the model path and fail with the same `Problem`s; word times add the run offset back to each word. |
| `docs/spike-results.md` | Measured sections for each app layer | The Annotate screen-stage measurements | Both kept, the app's sections first, the Annotate section appended. No measurement was dropped. |

`AGENTS.md` and `src-tauri/src/lib.rs` merged without conflict: one layout row for
`screen.rs`, and a module declaration alongside the app's module list.

## Why the layers still landed individually

A single preview branch existed to prove the two trees combined before either
stack landed, and to settle the four files above ahead of time. That proof was
then used rather than the branch itself: keeping each layer as its own PR keeps
each review attached to a diff small enough to read, and keeps the commit history
honest about what changed for which reason.

Each Annotate branch therefore took a merge of the current `main` before it
landed, rather than a rebase. The reviewed content of a layer did not change;
what changed is that its tree already contained the app stack, so the diff
reviewed was the diff merged. The one code adaptation this required is recorded
by the timed-transcription example, which now resolves its model the way the
tests do because `transcribe_timed` gained a model argument.

## State

As of 16 November 2026 both stacks are on `main`: #7–#26 for the app, #17–#24 for
Annotate. The integrated layers that wire Annotate into the running app continue
above that point, and they start from a tree where these four files are already
settled.
