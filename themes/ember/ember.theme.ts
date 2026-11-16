// The Ember theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in ember.settings.json and the generated ember.settings.ts;
// change them with `astryx experimental theme editor set ember ...`. Add your own changes below: they win over the settings.
import { defineTheme } from "@astryxdesign/core/theme";
import { emberSettings } from "./ember.settings";
import { pillButtons, roleTokens } from "../roles";

export const emberTheme = defineTheme({
  extends: emberSettings,
  name: "ember",
  tokens: roleTokens(
    {
      body: "oklch(0.985 0.003 75)",
      muted: "oklch(0.965 0.005 75)",
      surface: "oklch(0.998 0.001 75)",
      popover: "oklch(0.998 0.001 75)",
      neutral: "oklch(0.95 0.006 75)",
      text: "oklch(0.22 0.006 75)",
      secondary: "oklch(0.48 0.015 75)",
      border: "oklch(0.925 0.008 75)",
      emphasized: "oklch(0.48 0.015 75)",
      accent: "oklch(0.22 0.006 75)",
      onAccent: "oklch(0.985 0.003 75)",
      accentMuted: "oklch(0.925 0.008 75)",
    },
    {
      body: "oklch(0.21 0.005 75)",
      muted: "oklch(0.195 0.004 75)",
      surface: "oklch(0.25 0.008 75)",
      popover: "oklch(0.29 0.01 75)",
      neutral: "oklch(0.305 0.011 75)",
      text: "oklch(0.95 0.006 75)",
      secondary: "oklch(0.75 0.014 75)",
      border: "oklch(0.29 0.01 75)",
      emphasized: "oklch(0.6 0.014 75)",
      accent: "oklch(0.95 0.006 75)",
      onAccent: "oklch(0.21 0.005 75)",
      accentMuted: "oklch(0.29 0.01 75)",
    },
  ),
  components: pillButtons,
});
