# The dictate design system

The reference is Voicenotes' calm list and recording UI: whitespace, neutral
surfaces, small system type and pill actions. We use our own layout, copy and
icons. Colours, spacing, type, radii and durations live in `src/tokens.css`;
`src/ui/tokens.ts` exposes CSS references for TypeScript callers, without a second
copy of their values. Semantic colours switch with the system appearance.
Storybook's System / Light / Dark toolbar overrides appearance for inspection.
The recording capsule deliberately retains a dark surface in both themes.

Import components from `src/ui`. `App.css` loads the shared stylesheet and tokens.
The library has no Tauri imports. Settings still own native commands, query data,
shortcut registration, microphone polling and saving. The overlay keeps its
existing status/shortcut display; its timer and live pipeline states are stories
until a later task supplies those values.

## Contracts

| Component                  | Why this API                                                                                                                                                                                          |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Button, IconButton         | Three visual variants use native button props. IconButton requires its accessible label. Use disabled for unavailable actions.                                                                        |
| Chip, FilterChips          | Chips are pressed buttons. FilterChips owns no selection; value/onChange let the containing view own its filter.                                                                                      |
| Toggle, Select             | Native checkbox/switch and select semantics preserve platform keyboard handling. Supply a visible label or aria-label.                                                                                |
| SegmentedControl           | A labelled native radio group gives one selected value and arrow-key navigation. Options can be disabled.                                                                                             |
| KeyboardShortcut           | A canonical `Super+Shift+KeyD` string renders individual keycaps.                                                                                                                                     |
| ShortcutRecorder           | Controlled recording/value and start/cancel/commit callbacks keep OS registration and saving in the caller. Modifier-only presses wait; unmodified keys report an error; Escape, Tab and blur cancel. |
| SettingsRow, SettingsGroup | Row associates label and help with a control id; pass `aria-describedby="<id>-help"` to the control. Group supplies a section heading. Children keep complex settings composable.                     |
| SidebarNav                 | Items plus value/onChange keep navigation in the window. The selected item uses aria-current.                                                                                                         |
| Card                       | A surface around children, without extra modes.                                                                                                                                                       |
| ListItem                   | Date, title, preview and optional action children express the shared history hierarchy. Title is an h2 under the page h1.                                                                             |
| DetailPanel                | Title, metadata rows and body children keep transcript formatting out of the primitive.                                                                                                               |
| SearchField                | Label/value/onChange supports filtering. Clear restores input focus.                                                                                                                                  |
| LevelMeter                 | A normalized 0–1 value drives bars and a labelled native meter. The caller supplies live readings.                                                                                                    |
| RecordingCapsule           | A discriminated state requires timer/level only while recording and a message only for ready/error. Caller owns timing; status announcements never include a live timer.                              |
| ModelCard                  | Name, description, illustrative ratings, size and badge describe the model. A status union requires download/retry callbacks or progress only when relevant.                                          |
| ProgressBar                | A labelled 0–100 value; omit value for indeterminate work.                                                                                                                                            |
| EmptyState                 | Title/body and optional action children cover empty results and first use.                                                                                                                            |
| Toast                      | Success/warning/danger has a visible text cue as well as colour; errors announce assertively. Caller owns lifetime and optional dismissal.                                                            |

CSS variables fit the existing app and keep theme values available to CSS and
TypeScript. Tailwind would add another styling system. A headless component
library would add a dependency without improving these native selects, checkboxes
and radios. Custom composites can adopt a proven primitive when their behaviour
requires one. This follows “subtract before you add.” Reduced motion disables
transitions and the busy spinner's rotation; focus rings stay visible.

## Storybook and proof

- `pnpm storybook` opens the component workshop (Storybook 10.6.7, React + Vite).
- `pnpm build-storybook` writes the static workshop to `target/storybook`.
- `pnpm test:stories` runs every story in light and dark Chromium contexts, with
  axe failures treated as test failures. Play functions exercise toggle keyboard
  activation, filter selection, radio arrows, search/clear/focus and shortcut
  validation/capture/cancellation.
- `pnpm test:ui` exercises the built app through its existing browser IPC fixture.
- `pnpm capture:ui` builds the workshop, captures Foundations, five components,
  General light/dark and all recording states into `target/design-system`, and
  checks the page mocks/capsule at 390px. It intercepts static files in Playwright;
  it starts no server and closes Chromium when finished.

General stories render the actual settings window with Tauri's official mock IPC
edge. Loading/error variants exercise query states. History, Models and overlay
pages use sample data and have no live downloads or storage. Model accuracy/speed
ratings are illustrative, never measurements from this machine. axe and browser
fixtures do not prove VoiceOver, native WebKit, global hotkeys, login items,
permissions or end-to-end dictation; those need native verification.

## Inspected references

Mobbin was authenticated. Only the named products below informed the design;
unrelated search matches were ignored. Reference screenshots stay outside git.

- [Voicenotes list, filter chips and note hierarchy](https://mobbin.com/screens/ca7c778d-1aeb-476c-ad5f-ac72b82721d9).
- [Voicenotes record pill and note editor](https://mobbin.com/screens/0de9c36a-817c-4c66-83cc-155a4d6dda39).
- [Granola iOS timer, level and end controls](https://mobbin.com/screens/a646dca1-d8b7-44b1-9e50-ed7c0db8a591).
- [Fireflies transcript and metadata hierarchy](https://mobbin.com/screens/61d8e7bd-c468-479f-9752-3004ce68c345).

## Control and palette refinement

Use `size="sm" | "md" | "lg"` on Button, IconButton, Select, Chip,
SegmentedControl, SearchField, ShortcutRecorder and KeyboardShortcut. Toggle uses
the same size for its alignment box; the switch track stays 22px high. The
control heights are 28 / 32 / 40px, type 12 / 13 / 14px, horizontal padding
10 / 12 / 16px, icons 14 / 16 / 18px and gaps 6 / 8 / 10px. Settings controls
use the default `md`. The “Controls side by side” story measures the rendered
height and font size as well as showing the comparison.

The system SF Pro stack keeps this Mac utility native and needs no downloaded
font. Labels and md controls use 13px; helper text and deliberate section
overlines use 12px; page titles use 22px. We keep sentence case and normal
tracking. Timers, dates, rating percentages and contrast figures use tabular
numbers. Groups have 32px separation and 16px row insets.

The neutral ramp has a constant 255° hue in OKLCH, with very low chroma: a cool
slate temperature that feels precise without turning the interface blue. Pale
surface steps are close together; the dark end has wider lightness separation.
Near-black carries primary actions in light mode, with the inverse in dark mode.
Status uses green / orange / red / blue text on soft tinted surfaces, plus an
icon or explicit label. Only the four rendered status roles have ramps. All
semantic colours reference primitives. Culori verified the primitives within
sRGB; chroma was reduced at constant lightness and hue where necessary.

`node scripts/check-contrast.mjs` paints resolved CSS colours into a Chromium
sRGB canvas, measures WCAG luminance ratios and regenerates
`src/ui/contrast.generated.json`. Foundations shows both appearances and the
threshold for every measured pair, including status-action hover surfaces.
A failed pair makes the command fail. Re-run it after editing colour tokens.

Components import `ui.css`, which loads tokens and shared base styles.
Storybook's global preview loads only this stylesheet. Page stories separately
load App.css for the application shell. App.css owns the shell and page layout;
it is not a dependency of standalone component stories.

The microphone's zero level is a single quiet track, retaining a labelled meter
value of zero. Model accuracy and speed are labelled meters including the model
name; download work alone uses progress. Rating bars deliberately stay neutral.
Toasts use icons for tone and preserve their message without adding “Saved”.
