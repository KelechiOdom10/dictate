// The dictate theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in dictate.settings.json and the generated dictate.settings.ts;
// change them with `astryx experimental theme editor set dictate ...`. Add your own changes below: they win over the settings.
import {defineTheme} from '@astryxdesign/core/theme';
import {dictateSettings} from './dictate.settings';

export const dictateTheme = defineTheme({
  extends: dictateSettings,
  name: 'dictate',
  tokens: {
    '--color-background-body': ['oklch(0.985 0.003 75)', '#1e1e2e'],
    '--color-background-muted': ['oklch(0.965 0.005 75)', '#181825'],
    '--color-background-surface': ['oklch(0.998 0.001 75)', '#252536'],
    '--color-background-card': ['oklch(0.998 0.001 75)', '#252536'],
    '--color-background-popover': ['oklch(0.998 0.001 75)', '#313244'],
    '--color-neutral': ['oklch(0.95 0.006 75)', '#45475a'],
    '--color-text-primary': ['oklch(0.22 0.006 75)', '#cdd6f4'],
    '--color-text-secondary': ['oklch(0.48 0.015 75)', '#bac2de'],
    '--color-icon-primary': ['oklch(0.22 0.006 75)', '#cdd6f4'],
    '--color-icon-secondary': ['oklch(0.48 0.015 75)', '#bac2de'],
    '--color-border': ['oklch(0.925 0.008 75)', '#45475a'],
    '--color-border-emphasized': ['oklch(0.48 0.015 75)', '#7f849c'],
    '--color-accent': ['#a84114', '#f5a46b'],
    '--color-on-accent': ['#ffffff', '#181825'],
    '--color-accent-muted': ['#f9e3d7', '#49352f'],
    '--color-text-accent': ['#a84114', '#f5a46b'],
    '--color-icon-accent': ['#a84114', '#f5a46b'],
    '--focus-outline-color': ['#a84114', '#f5a46b'],
  },
});
