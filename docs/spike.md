# Spike: can this be built?

Two days. One question. Not a product.

## The question

> Can I hold a key, speak a sentence, release, and have the transcript in the
> text field I was already typing in — fast enough that I replace Hex with it?

Everything else in this project is ordinary work. This is the part that might
not be true, so it gets proven before any product doc, any UI, any settings
screen, and any feature list.

## The gate

| # | Check | Pass condition |
|---|---|---|
| 1 | Hotkey fires while a *different* app is focused, without stealing focus | Overlay appears, cursor stays in the target app |
| 2 | Audio actually captured | Buffer length ≈ duration x 16000, first phoneme present |
| 3 | Transcription is correct | One spoken sentence comes back word-perfect |
| 4 | Paste lands in the focused field | Works in TextEdit, VS Code, and a browser input |
| 5 | Clipboard is restored | The thing you copied before dictating is still on the clipboard after |
| 6 | Latency is acceptable | p95 key-release → text present, under 1.5 s on this machine |
| 7 | Accuracy is measured, not assumed | WER on 20 of your own recordings |

Checks 6 and 7 are the ones people skip and then argue about for a month.

## What is deliberately not in the spike

Screenshots, annotation, the scrollable artifact, settings, onboarding, a menu
bar item, signing, auto-update, a second language, model selection UI. If any of
those appear before check 7 has a number, the spike has failed even if the app
works.

## Decision it produces

- **All seven pass** → build the rest, and the product doc is worth writing.
- **1–4 pass, 6 fails** → still viable, but the design changes: you need a
  streaming model and partial results, not record-then-transcribe.
- **1 or 4 fails** → the shape changes fundamentally; stop and rethink the
  interaction before writing any code you would have to keep.
