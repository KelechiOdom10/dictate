// The dictate theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in dictate.settings.json and the generated dictate.settings.ts;
// change them with `astryx experimental theme editor set dictate ...`. Add your own changes below: they win over the settings.
import {defineTheme} from '@astryxdesign/core/theme';
import {dictateSettings} from './dictate.settings';

export const dictateTheme = defineTheme({
  extends: dictateSettings,
  name: 'dictate',
  tokens: {
    '--color-background-body': ['oklch(0.985 0.003 75)', '#11132a'],
    '--color-background-muted': ['oklch(0.965 0.005 75)', '#0a0b1e'],
    '--color-background-surface': ['oklch(0.998 0.001 75)', '#171a34'],
    '--color-background-card': ['oklch(0.998 0.001 75)', '#171a34'],
    '--color-background-popover': ['oklch(0.998 0.001 75)', '#2a2d4d'],
    '--color-neutral': ['oklch(0.95 0.006 75)', '#2a2d4d'],
    '--color-text-primary': ['oklch(0.22 0.006 75)', '#d2daf6'],
    '--color-text-secondary': ['oklch(0.48 0.015 75)', '#a2aacc'],
    '--color-icon-primary': ['oklch(0.22 0.006 75)', '#d2daf6'],
    '--color-icon-secondary': ['oklch(0.48 0.015 75)', '#a2aacc'],
    '--color-border': ['oklch(0.925 0.008 75)', '#2a2d4d'],
    '--color-border-emphasized': ['oklch(0.48 0.015 75)', '#7a80a6'],
    '--color-accent': ['oklch(0.22 0.006 75)', '#d2daf6'],
    '--color-on-accent': ['oklch(0.985 0.003 75)', '#0a0b1e'],
    '--color-accent-muted': ['oklch(0.925 0.008 75)', '#1f223f'],
    '--color-text-accent': ['oklch(0.22 0.006 75)', '#d2daf6'],
    '--color-icon-accent': ['oklch(0.22 0.006 75)', '#d2daf6'],
    '--focus-outline-color': ['oklch(0.22 0.006 75)', '#a99cf5'],
  },
  components: {
    button: {
      base: { borderRadius: 'var(--radius-pill, 9999px)' },
    },
  },
});
