# QA round 1

The screenshots use isolated Storybook fixtures, not the owner's transcripts.
Before: 775b4fe (#32). Models-only: 296aade. After: c561476.
Each view is captured in Ember/Paper and light/dark.

To keep borders only on model cards, drop commit fd44b21 (or revert it).
Geist is bundled locally with its OFL license.

## Walkthrough

The native menu reads installed model files and the saved model choice. Choosing
a model uses the same validated settings save as the Models page; changes and
downloads rebuild the menu. Paste and Copy read the latest nonempty transcript
from retained History. With History disabled or empty they show an error.
Settings and Quit use native Command-comma and Command-Q accelerators.
Update checking is a stub that opens Settings with a notice.

Turning Annotate on now requests Screen Recording access if preflight is false.
The settings button already requested access before opening the pane. Both use
ScreenCaptureAccess.request, which calls CGRequestScreenCaptureAccess; status
uses ScreenCaptureAccess.preflight, which calls CGPreflightScreenCaptureAccess.
Tauri's development mode selects terminal-specific permission guidance.

## Validation

- cargo fmt: passed.
- cargo clippy -- -D warnings: passed.
- cargo test: 168 unit tests and 3 integration tests passed; 10 hardware/manual tests ignored.
- pnpm build: passed.
- pnpm test:stories: 348 passed across four theme combinations.
- pnpm test:ui: 17 passed.
- pnpm format:check: passed.
- Spacing story first failed against the changed shortcut/segments, then passed
  after updating its assertions for the intended compact controls.
- Screenshots inspected: General's right-aligned download and quiet keycaps,
  History's compact tabs and separate retention, and light/dark model cards.
- Full six-domain interface review and native interaction proof remain incomplete.

## Manual checks for Kelechi

No synthetic desktop input was used. Native menu screenshots are still missing.

1. Run the app, click its menu bar icon, and capture the open menu in macOS
   light and dark appearances. Repeat with Ember/Paper selected. Expand Speech
   Model, choose an installed model, reopen it and check its tick and Settings.
2. Dictate a sentence with History enabled. With a disposable document focused,
   open the menu and choose Paste Last Dictation. Confirm text lands in that
   document and the previous clipboard contents return. Choose Copy Last
   Transcript and paste manually to verify it. No automatic desktop driver runs.
3. Use Settings… and Quit dictate, including Command-comma and Command-Q.
   Check for Updates… should show the coming-later notice.
4. In a bundled app without Screen Recording permission, turn Annotate on.
   Confirm macOS asks and lists dictate. Grant access, restart, and confirm the
   status says Allowed. Repeat via Open System Settings when access is denied.
5. In a dev run, confirm denied-access help names the parent terminal. Enable
   that terminal in Screen Recording and restart. Do not reset existing TCC grants
   just to test this branch.
6. Compare the three sets below. Choose whether to keep the optional all-card
   borders before merging.

## Screenshots

### Ember · light

**general-no-model**

| Before | After |
| --- | --- |
| ![Before](before-general-no-model-ember-light.png) | ![After](after-general-no-model-ember-light.png) |

**models-installed**

| Before | After |
| --- | --- |
| ![Before](before-models-installed-ember-light.png) | ![After](after-models-installed-ember-light.png) |

**history**

| Before | After |
| --- | --- |
| ![Before](before-history-ember-light.png) | ![After](after-history-ember-light.png) |

Models-only borders, before the optional all-card commit:

![General](models-only-general-ember-light.png)

![Models](models-only-models-installed-ember-light.png)

![History](models-only-history-ember-light.png)

### Ember · dark

**general-no-model**

| Before | After |
| --- | --- |
| ![Before](before-general-no-model-ember-dark.png) | ![After](after-general-no-model-ember-dark.png) |

**models-installed**

| Before | After |
| --- | --- |
| ![Before](before-models-installed-ember-dark.png) | ![After](after-models-installed-ember-dark.png) |

**history**

| Before | After |
| --- | --- |
| ![Before](before-history-ember-dark.png) | ![After](after-history-ember-dark.png) |

Models-only borders, before the optional all-card commit:

![General](models-only-general-ember-dark.png)

![Models](models-only-models-installed-ember-dark.png)

![History](models-only-history-ember-dark.png)

### Paper · light

**general-no-model**

| Before | After |
| --- | --- |
| ![Before](before-general-no-model-paper-light.png) | ![After](after-general-no-model-paper-light.png) |

**models-installed**

| Before | After |
| --- | --- |
| ![Before](before-models-installed-paper-light.png) | ![After](after-models-installed-paper-light.png) |

**history**

| Before | After |
| --- | --- |
| ![Before](before-history-paper-light.png) | ![After](after-history-paper-light.png) |

Models-only borders, before the optional all-card commit:

![General](models-only-general-paper-light.png)

![Models](models-only-models-installed-paper-light.png)

![History](models-only-history-paper-light.png)

### Paper · dark

**general-no-model**

| Before | After |
| --- | --- |
| ![Before](before-general-no-model-paper-dark.png) | ![After](after-general-no-model-paper-dark.png) |

**models-installed**

| Before | After |
| --- | --- |
| ![Before](before-models-installed-paper-dark.png) | ![After](after-models-installed-paper-dark.png) |

**history**

| Before | After |
| --- | --- |
| ![Before](before-history-paper-dark.png) | ![After](after-history-paper-dark.png) |

Models-only borders, before the optional all-card commit:

![General](models-only-general-paper-dark.png)

![Models](models-only-models-installed-paper-dark.png)

![History](models-only-history-paper-dark.png)

