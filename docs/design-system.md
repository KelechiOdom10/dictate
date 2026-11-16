# The dictate design system

The settings window uses Astryx 0.7.16 components over a customized Neutral
theme: warm off-white light surfaces, Catppuccin Mocha-inspired dark surfaces,
Geist, and Ember primary actions and bright accents. Paper keeps ink actions.
The recording overlay remains a separate, compact native-window presentation;
Paper uses a warm red recording dot while its switches and level bars stay ink.
General › Appearance saves Theme (Ember / Paper) and Mode (System / Light / Dark)
in the Rust settings file. Both webviews consume the same settings event. App
resolves System with matchMedia and applies data-palette plus data-mode on the root;
CSS has no separate system-mode override. Storybook exposes both dimensions.
Paper’s dark capsule is pale ink; Ember’s stays warm charcoal, matching the chosen
comparison panels.

Import components from `src/ui`. `App.css` loads the shared stylesheet and tokens.
The library has no Tauri imports. Settings still own native commands, query data,
shortcut registration, microphone polling and saving. The overlay renders real pipeline states, a per-take timer and RMS input levels.
See [recording-overlay.md](recording-overlay.md) for its native boundary and manual checks.

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
| ListItem                   | Date, title, optional meta/preview and action children for a static row. Long titles truncate to one line; the full value lives in the detail view. |
| SelectList                 | Rows with one selected id (aria-current). Owns a roving tabindex: one Tab stop, Up/Down/Home/End move selection and focus. The caller owns the id. |
| DetailPanel                | Title, optional subtitle and header actions, body children, then metadata rows. Transcript formatting stays out of the primitive. |
| SearchField                | Label/value/onChange supports filtering. Clear restores input focus.                                                                                                                                  |
| LevelMeter                 | A normalized 0–1 value drives bars and a labelled native meter. The caller supplies live readings.                                                                                                    |
| RecordingCapsule           | A discriminated state requires timer/level only while recording (`annotating` takes a timer, no level), a message only for ready and a title plus fix for error; skipped supplies a quiet retry instruction. Caller owns timing; status announcements never include a live timer. An optional notice is a secondary line that preserves the phase and timer.                              |
| ModelCard                  | Name, description, illustrative ratings, size and badge describe the model. A status union requires download/retry callbacks or progress only when relevant.                                          |
| ProgressBar                | A labelled 0–100 value; omit value for indeterminate work.                                                                                                                                            |
| EmptyState                 | Title/body and optional action children cover empty results and first use.                                                                                                                            |
| Toast                      | Success/warning/danger has a visible text cue as well as colour; errors announce assertively. Caller owns lifetime and optional dismissal.                                                            |

SettingsTheme scopes Astryx's public Theme provider to settings. Astryx owns
`data-theme` for its light/dark mode, while `data-palette` preserves the separate
Ember/Paper preference; stored settings are unchanged. Settings rows/groups use
Astryx HStack, VStack and Heading; the existing Button facade delegates to Astryx
inside this theme and keeps native disabled attributes. Startup uses Astryx
Switch, including its hidden accessible input and visible track. Existing native
selects and radio groups retain their platform keyboard behavior and ids.
The overlay has no Theme provider and keeps its existing buttons and sizing.
Reduced motion disables
transitions and the busy spinner's rotation; focus rings stay visible.

## Astryx setup and generated conventions

The four owner-requested packages are pinned in package.json and the lockfile.
Registry metadata and the official `meta/astryx` README agree on the repository,
versions, React 19 peers and the prebuilt-CSS consumption path. StyleX 0.19.2
satisfies Core's peer; Vite 8/React 19 consume the distributed JS/CSS without a
StyleX, Babel or PostCSS compiler plugin. We author no `stylex.create()` calls.

`npx @astryxdesign/cli init --agent codex` appended its managed block to AGENTS.md
without replacing the existing project instructions. Its conventions say to keep
an existing page, read component props before use, prefer component props and
theme tokens, extend a theme instead of forking library source, and run
`astryx doctor --built dist`. New pages would start from a template; this change
preserves the existing settings frame and native lifecycle.

The experimental theme editor seeded `themes/dictate/dictate.settings.json` and
its palette. Exact app colors live in `dictate.theme.ts`, extending those generated
settings; `themes/paper.theme.ts` changes only action/focus colors. Run
`pnpm build:themes` after changing either source. The committed precompiled
`src/themes/` modules/styles and CLI-managed `src/astryx-themes.ts` are generated,
not hand-edited. Preview HTML/PNGs and editor history are ignored local artifacts.
The existing bundled Geist font in tokens.css supplies both themes; doctor's
font-stylesheet warning is a scanner limitation, not a runtime font fetch.

`pnpm test:stories` exercises both palettes and modes with axe. Browser tests
assert the palette, resolved mode, library theme and color scheme are applied
before `settings_ready`, including suspended animation frames and load failure.
Astryx's Switch uses an opacity-zero input, so its page story checks the visible
composite field plus the enabled, unchecked accessible control; dedicated stories
also exercise click, Space and disabled activation.

## Storybook and proof

- `pnpm storybook` opens the component workshop (Storybook 10.6.7, React + Vite).
- `pnpm build-storybook` writes the static workshop to `target/storybook`.
- `pnpm test:stories` runs every story in all four theme/mode Chromium contexts, with
  axe failures treated as test failures. Play functions exercise toggle keyboard
  activation, filter selection, radio arrows, search/clear/focus and shortcut
  validation/capture/cancellation.
- `pnpm test:ui` exercises the built app through its existing browser IPC fixture.
- `pnpm capture:ui` builds the workshop and captures every story in all four
  combinations into `target/design-system`, plus control hover, pressed and track
  focus states. It checks page mocks/capsules at 320px, uses the bundled font,
  intercepts static files in Playwright, starts no server and closes Chromium.
- `node scripts/compare-themes.mjs <comparison-source.html>` captures matching
  Paper/Ember reference panels with the local font and builds four comparison
  sheets from the workshop captures. The source is supplied separately; screenshots
  and intermediate HTML stay in target/.


General stories render the actual settings window with Tauri's official mock IPC
edge. Loading/error variants exercise query states. History stories render the real
History page over mock IPC (populated, empty, search, no matches, delete and
retention confirmations). Models and overlay pages use sample data and have no
live downloads. Model accuracy/speed
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
12 / 14 / 18px, icons 14 / 16 / 18px and gaps 6 / 8 / 10px. Settings controls
use the default `md`. The “Controls side by side” story measures the rendered
height, font size, horizontal padding and gaps as well as showing the comparison.

Geist is bundled as a local variable WOFF2 face for weights 400/500/600,
with -apple-system and system-ui fallbacks. Its OFL licence and pinned download
source are beside the font. No runtime font request leaves the app. Labels and md
controls use 13px; helper text and section headings use 12px; page titles use 22px.
The wider helper measure keeps the hold-to-talk instruction on one line at the
native settings width. Timers and contrast figures use tabular numbers.

One brown ramp holds a constant 75° OKLCH hue. Close light steps preserve paper
surfaces; distinct dark steps keep controls visible. Semantic roles reference
primitives; hover, pressed and navigation selection have separate steps. The
39 primitives were checked with Culori for sRGB gamut; crimson’s light text step
needed chroma reduction at fixed lightness/hue. Danger is warm red at 22°,
separated from Ember’s 40° by hue and lightness. Paper’s recording dot uses a
brighter red from the same ramp, preserving PR #28's intended behavior against
the current UI. It remains a visual risk to inspect side by side: errors
also carry labels/icons. Warning is yellow, success green, info blue, all on soft
tints. Overlay controls retain ink focus rings; settings use the palette's
Astryx focus token (Ember or ink).

`node scripts/check-contrast.mjs` paints resolved CSS colours into a Chromium
sRGB canvas, measures WCAG luminance ratios and regenerates the contrast and
palette JSON used by Foundations. It fails on any insufficient pair. Foundations
shows all four combinations, including tinted status surfaces and live/danger
examples. Re-run it after changing tokens. Model downloads share the rating
meter’s borderless styling while retaining native progress semantics.

Components import `ui.css`, which loads tokens and shared base styles.
Storybook's global preview loads only this stylesheet. Page stories separately
load App.css for the application shell. App.css owns the shell and page layout;
it is not a dependency of standalone component stories.

The microphone's zero level is a single quiet track, retaining a labelled meter
value of zero. Model accuracy and speed are labelled meters including the model
name; download work alone uses progress. Rating bars deliberately stay neutral.
Toasts use icons for tone and preserve their message without adding “Saved”.
