// The Capy theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in capy.settings.json and the generated capy.settings.ts;
// change them with `astryx experimental theme editor set capy ...`. Add your own changes below: they win over the settings.
import { defineTheme } from "@astryxdesign/core/theme";
import { capySettings } from "./capy.settings";
import { pillButtons, roleTokens } from "../roles";

export const capyTheme = defineTheme({
  extends: capySettings,
  name: "capy",
  tokens: roleTokens(
    {
      body: "#f5f6fc",
      muted: "#eceef8",
      surface: "#ffffff",
      popover: "#ffffff",
      neutral: "#e4e7f4",
      text: "#1a1d38",
      secondary: "#4f557a",
      border: "#dfe2f0",
      emphasized: "#7a80a6",
      accent: "#1a1d38",
      onAccent: "#f5f6fc",
      accentMuted: "#e4e7f4",
    },
    {
      body: "#11132a",
      muted: "#0a0b1e",
      surface: "#171a34",
      popover: "#2a2d4d",
      neutral: "#2a2d4d",
      text: "#d2daf6",
      secondary: "#a2aacc",
      border: "#2a2d4d",
      emphasized: "#7a80a6",
      accent: "#d2daf6",
      onAccent: "#0a0b1e",
      accentMuted: "#1f223f",
    },
  ),
  components: pillButtons,
});
