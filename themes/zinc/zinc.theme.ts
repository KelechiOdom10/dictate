// The Neutral theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in zinc.settings.json and the generated zinc.settings.ts;
// change them with `astryx experimental theme editor set zinc ...`. Add your own changes below: they win over the settings.
import { defineTheme } from "@astryxdesign/core/theme";
import { zincSettings } from "./zinc.settings";
import { pillButtons, roleTokens } from "../roles";

export const zincTheme = defineTheme({
  extends: zincSettings,
  name: "zinc",
  tokens: roleTokens(
    {
      body: "#fafafa",
      muted: "#f4f4f5",
      surface: "#ffffff",
      popover: "#ffffff",
      neutral: "#f4f4f5",
      text: "#09090b",
      secondary: "#52525b",
      border: "#e4e4e7",
      emphasized: "#71717a",
      accent: "#18181b",
      onAccent: "#fafafa",
      accentMuted: "#e4e4e7",
      focus: "#71717a",
    },
    {
      body: "#101012",
      muted: "#09090b",
      surface: "#18181b",
      popover: "#18181b",
      neutral: "#27272a",
      text: "#f4f4f5",
      secondary: "#a1a1aa",
      border: "#27272a",
      emphasized: "#71717a",
      accent: "#f4f4f5",
      onAccent: "#09090b",
      accentMuted: "#27272a",
      focus: "#a1a1aa",
    },
  ),
  components: pillButtons,
});
