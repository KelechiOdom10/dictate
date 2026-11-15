# QA round 2

Screenshots come from isolated Storybook fixtures, not the owner's transcripts.
Before: be185fc (#38). After: this branch. Each view is captured in Ember and
Paper, light and dark, at 2x.

Files are named `{before,after}-{view}-{theme}-{mode}.png`, where view is
`segments`, `history`, `general`, `shortcut-rest` or `shortcut-hover`.

## What changed

- Dark segmented control: the thumb moves from brown-650 (L 0.35) to a new
  brown-625 (L 0.42) on the brown-750 (L 0.305) track. Its hairline uses a new
  `--segment-edge`, because the old ring borrowed `--keycap-edge`, which in dark
  was the thumb's own colour. `--segment-shadow` is stronger in dark (0.35
  alpha) than in light (0.08).
- One control scale, roomier: horizontal padding is 12 / 14 / 18px for
  sm / md / lg (was 10 / 12 / 16). Heights stay 28 / 32 / 40px. The segment track
  inset grows from 2px to 3px.
- Shortcut keycaps: the pill's inline padding is now `--control-gap`, with a
  matching negative end margin so the keycaps at rest still line up with the
  other controls' right edge. The outer keycaps clear the rounded ends by about
  the same 4px as the gap between keycaps, at rest and on hover.

## Contrast

`scripts/check-contrast.mjs` now also checks text on `segment-thumb`.

| Pair | Light | Dark |
|---|---|---|
| text / segment-thumb | 17.23:1, Lc 103.8 | 7.33:1, Lc −82.8 |
| muted / control | 5.63:1, Lc 72.5 | 6.05:1, Lc −53.4 |

Ember and Paper give the same values. All 188 pairs pass. The lowest is 4.69:1.

## Manual check for Kelechi

In `pnpm tauri dev`, dark mode: open History and confirm the selected segment
stands out from the track. Open General, hover the shortcut and confirm the
keycaps don't touch the pill's rounded ends.
