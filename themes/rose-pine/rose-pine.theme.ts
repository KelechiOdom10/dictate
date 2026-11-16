// The Rosé Pine theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in rose-pine.settings.json and the generated rose-pine.settings.ts;
// change them with `astryx experimental theme editor set rose-pine ...`. Add your own changes below: they win over the settings.
import { defineTheme } from "@astryxdesign/core/theme";
import { rosePineSettings } from "./rose-pine.settings";
import { pillButtons, roleTokens } from "../roles";

export const rosePineTheme = defineTheme({
  extends: rosePineSettings,
  name: "rose-pine",
  tokens: roleTokens(
    {
      body: "#faf4ed",
      muted: "#f4ede8",
      surface: "#fffaf3",
      popover: "#fffaf3",
      neutral: "#f2e9e1",
      text: "#575279",
      secondary: "#5f5b7a",
      border: "#dfdad9",
      emphasized: "#797593",
      accent: "#575279",
      onAccent: "#faf4ed",
      accentMuted: "#f2e9e1",
      focus: "#907aa9",
    },
    {
      body: "#1c1a29",
      muted: "#191724",
      surface: "#26233a",
      popover: "#26233a",
      neutral: "#1f1d2e",
      text: "#e0def4",
      secondary: "#908caa",
      border: "#3a3750",
      emphasized: "#908caa",
      accent: "#e0def4",
      onAccent: "#191724",
      accentMuted: "#26233a",
      focus: "#c4a7e7",
    },
  ),
  components: pillButtons,
});
