import { defineTheme } from "@astryxdesign/core/theme";
import { dictateTheme } from "./dictate/dictate.theme";

export const paperTheme = defineTheme({
  extends: dictateTheme,
  name: "dictate-paper",
  tokens: {
    "--color-accent": ["oklch(0.22 0.006 75)", "#d2daf6"],
    "--color-on-accent": ["oklch(0.985 0.003 75)", "#0a0b1e"],
    "--color-accent-muted": ["oklch(0.925 0.008 75)", "#1f223f"],
    "--color-text-accent": ["oklch(0.22 0.006 75)", "#d2daf6"],
    "--color-icon-accent": ["oklch(0.22 0.006 75)", "#d2daf6"],
    "--focus-outline-color": ["oklch(0.22 0.006 75)", "#d2daf6"],
  },
});
