# Merge plan: app stack, then Annotate stack — 15 November 2026

Two reviewed stacks touch the same four files. This page says how to land
both without resolving anything by hand. Delete it, and close the preview PR,
once both stacks are on `main`.

| Stack | PRs, bottom to top | Top branch |
|---|---|---|
| App | #7 → #8 → #9 → #10 → #12 → #14 → #15 → #16 → #18 → #21 → #23 → #25 → #26 | `fix/pr25-review-fixes` |
| Annotate | #17 → #19 → #20 → #22 → #24 | `fix/annotate-frame-review` |

Two branches support this plan. Neither changes an existing PR branch:

- `preview/app-plus-annotate` (the draft "integration preview" PR): the app
  stack with the Annotate stack merged in. The full gate ran on it.
- `preview/annotate-rebased`: the eight Annotate commits already replayed onto
  the app stack's top (`1d8f431`), with every conflict resolved in the commit
  that caused it. Its tree is byte-identical to the preview branch's tree, and
  each commit passes `cargo clippy --all-targets -D warnings` on its own.

## Morning steps

### 1. Merge the app stack

Merge #26 with the stack merge. `main` has two commits the app stack lacks:
the Annotate design doc, PR #5. GitHub reports #7 and #26 as `MERGEABLE CLEAN`,
and `git merge-tree origin/main origin/fix/pr25-review-fixes` finds no
conflicts.

### 2. Move the Annotate branches onto the new `main`

```sh
git fetch origin
git switch -c annotate-restack origin/preview/annotate-rebased
git rebase --onto origin/main 1d8f43196dfd179dab5f4dd17bf7a86e6112415b
```

Expect no conflicts. Git drops the design-doc commit because its content is
already on `main`. This was rehearsed against a merge of `origin/main` and #26,
and the result had the same tree as `preview/annotate-rebased`. Before pushing,
check that the eight commits are in this order:

```sh
git log --oneline origin/main..HEAD
# HEAD    fix: keep frame times inside the video and thicken contact-sheet ink   (#24)
# HEAD~1  feat: pull key frames, draw ink and build the contact sheet            (#22)
# HEAD~2  fix: bound frame merges and give marks the words said after them       (#22)
# HEAD~3  fix: keep word times on the audio clock after silence                  (#22)
# HEAD~4  feat: build the Annotate package core from strokes and timed words     (#20)
# HEAD~5  feat: transcribe with word-level timestamps for Annotate               (#19)
# HEAD~6  fix: refuse unexcluded screen recordings and keep partial takes        (#19)
# HEAD~7  feat: record one display to MP4 with ScreenCaptureKit                  (#17)
```

Then point each PR branch at its commit. `--force-with-lease` names the
branch's current tip, so the push is refused if anyone moved a branch since
this plan was written:

```sh
git push origin \
  --force-with-lease=feat/screen-capture-stage:cf2445c196def40db860889d1dec137ddc7e2509 \
  --force-with-lease=feat/transcribe-timed:487b97ffe405706f9e560beead5bf240ab92dcf9 \
  --force-with-lease=feat/annotate-package-core:66d29a4c6807416b9777158e81a1c4669eef2678 \
  --force-with-lease=feat/annotate-frames-contact-sheet:e04f3b5d1e4bdb9a8f55696b3c313ace4360097f \
  --force-with-lease=fix/annotate-frame-review:7028334e9e6f4e8edd2fde012ed3220abcfc223a \
  HEAD~7:refs/heads/feat/screen-capture-stage \
  HEAD~5:refs/heads/feat/transcribe-timed \
  HEAD~4:refs/heads/feat/annotate-package-core \
  HEAD~1:refs/heads/feat/annotate-frames-contact-sheet \
  HEAD:refs/heads/fix/annotate-frame-review
```

The PR bases stay the same: #17 targets `main`, and each later PR targets the
branch below it. History stays linear.

### 3. Merge the Annotate stack

Wait for checks on #24, then merge it with the stack merge.

### 4. Clean up

Close the "integration preview (do not merge)" PR. Delete
`preview/app-plus-annotate`, `preview/annotate-rebased` and the local
`annotate-restack` branch.

## How each conflict was resolved

Use these notes if step 2 fails, for example because a branch moved. They
also let a reviewer check that both sides survived. "App" means the app stack,
which is the base during a rebase. "Annotate" means the commit being replayed.

### `src-tauri/Cargo.toml` — #17 and #22

The result is the union of both sides.

- `[dependencies]`: the app's `ts-rs`, `rusqlite`, `ureq` and `sha2`, plus
  Annotate's `tiny-skia`.
- `objc2-app-kit` features: the app's `NSRunningApplication`, `NSWindow` and
  `NSWorkspace`. Annotate added none.
- `objc2-foundation` features: Annotate's `NSError` and `NSURL`, added to the
  shared `NSArray`, `NSData` and `NSString`.
- `core-foundation` stays (app). `screencapturekit`, `block2`, the `objc2-av-*`
  and `objc2-core-*` crates, `[profile.dev.package]` and the macOS
  `[dev-dependencies]` block come from Annotate.

At #17, Git merged this file without help. At #22, take the preview branch's
file as it is: `git checkout origin/preview/app-plus-annotate --
src-tauri/Cargo.toml`.

### `src-tauri/Cargo.lock` — #17 and #22

Do not edit it by hand. Take the app's lock (`git checkout --ours
src-tauri/Cargo.lock` during a rebase) and run `cargo check --all-targets`.
Cargo adds only Annotate's packages, at the versions Annotate locked
(`screencapturekit` 11.1.2, `tiny-skia` 0.11.4, `objc2-av-foundation` 0.3.2).
No app package changes version. The regenerated lock at #22 matches the preview
branch's lock exactly.

### `docs/spike-results.md` — #17 (and the preview merge)

Keep both sides: all of the app's sections first, then Annotate's, in their
original order. The first Annotate heading is "Annotate screen stage". Each
side closes its own code fence, so check that the app's last section ("Warm
microphone") still ends with a closing fence before "## Annotate screen stage".
Later Annotate commits add only to the end of the file, and Git merged them
without help.

### `src-tauri/src/transcribe.rs` — #19 and #22

This is the one that needs judgement. The two sides changed the same function
in different ways:

- **App:** `transcribe(samples, model: &Path) -> Result<String, Problem>`.
  The caller passes the active model, which lib.rs resolves from the Models
  page (`active_model_path`). Errors are `Problem`s with a title and a fix:
  "Transcription failed", "Still transcribing", "Speech model missing" (with the
  Settings › Models fix) and "whisper-cli not found" (with the
  `brew install whisper.cpp` fix). It logs with `log::warn!`. `model_name`,
  `whisper_missing`, `missing_model` and `WHISPER_INSTALL` are public, for
  lib.rs and the Models page.
- **Annotate:** `transcribe(samples) -> Result<String, String>`, which reads
  the model from `DICTATE_WHISPER_MODEL` or `models/ggml-base.en.bin`. It adds
  `Word`, `Segment`, `transcribe_timed`, `segments`, `parse_words` and a
  shared whisper-cli runner. At #22 it also adds `speech_spans`, the
  abbreviation-aware `ends_sentence`, and the split into
  `validate`/`lock`/`run_whisper_locked`.

The merged file keeps the app's API and errors, and adds every Annotate item
on top:

- `transcribe_timed(samples, model: &Path) -> Result<Vec<Word>, Problem>`.
  It takes the model the same way `transcribe` does, so Annotate will use the
  model chosen on the Models page, not a path from the environment. It fails
  in the same order and with the same `Problem`s: bad audio, then "Still
  transcribing", then whisper-cli missing, then model missing. Errors from
  whisper-cli and from parsing its JSON become "Transcription failed", with the
  cause in `detail`.
- Both functions share `validate`, `lock`, `ready` (whisper-cli is found and
  the model file exists) and one `run_whisper(cli, model, samples, args,
  ext)`. That last function is Annotate's runner, with the app's arguments.
  The whisper flags are unchanged: `-l en -np` plus `-nt -otxt` for text, or
  `-ojf -ml 1 -sow` for word times.
- Annotate's `model_path()` is gone, and its `whisper_cli()` gives way to the
  app's version, which returns a `Problem`. Annotate's tests and the
  `transcribe_timed` example resolve the model themselves, the way the app's
  tests already did (`DICTATE_WHISPER_MODEL`, else `models/ggml-base.en.bin`).
- Tests: the app's six tests and Annotate's fifteen. Three tests exist on both
  sides, so the file has 18. The shared `rejects_empty_and_nonfinite_audio`
  and `missing_model_points_to_the_models_page` tests now check both functions.
  All whisper-calling tests take Annotate's `whisper_turn` lock, so they never
  fail each other with "Still transcribing". The app's `measure_catalog_models`
  uses the raw `say` helper, so it does not need the base model installed.

At #19, Annotate has no `speech_spans` yet. Its replayed commit
(`preview/annotate-rebased` `75c3398`) is the merged file without the #22
additions: `transcribe_timed` makes one whisper call over the whole take, and
sentences end on `.`, `!` or `?`. The same commit updates
`examples/transcribe_timed.rs`, which arrives in #19, to pass a model path. At
#22 (`fix: keep word times on the audio clock after silence`), take the
preview branch's file as it is: `git checkout origin/preview/app-plus-annotate
-- src-tauri/src/transcribe.rs`.

### Merged without help

Git merged `AGENTS.md` and `src-tauri/src/lib.rs` (Annotate's `pub mod`
lines and pipeline doc lines) without conflict, and `build.rs` was not
touched on the app side.

## Gate on the preview branch

Run on this Mac with Homebrew whisper.cpp and `ggml-base.en.bin`:

| Command | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | 121 passed, 9 ignored (lib); 2 passed (`tests/annotate_frames.rs`) |
| `pnpm build` | built |
| `pnpm test:stories` | 264 passed in 32 files |
| `pnpm test:ui` | 14 passed |
| `pnpm format:check` | clean |

The whisper integration tests ran for real; none printed `SKIP`. Generated
speech came back word-perfect, and the Annotate onset tests landed at 0 ms and
6 ms error. On the generated minute, `transcribe_timed` returned 205 words with
the last ending at 61,760 ms, the same as Annotate's own results.
