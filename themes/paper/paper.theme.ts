// The Paper theme. `astryx experimental theme editor` wrote this file once and never changes it again.
// The checked settings live in paper.settings.json and the generated paper.settings.ts;
// change them with `astryx experimental theme editor set paper ...`. Add your own changes below: they win over the settings.
import { defineTheme } from "@astryxdesign/core/theme";
import { paperSettings } from "./paper.settings";
import { controlStyles, roleTokens } from "../roles";

export const paperTheme = defineTheme({
  extends: paperSettings,
  name: "paper",
  tokens: roleTokens(
    {
      body: "#fbfaf7",
      muted: "#f3f1ec",
      surface: "#ffffff",
      popover: "#ffffff",
      neutral: "#eceae4",
      text: "#1c1b19",
      secondary: "#5f5b53",
      border: "#e6e2d9",
      emphasized: "#8a857a",
      accent: "#1c1b19",
      onAccent: "#fbfaf7",
      accentMuted: "#e6e2d9",
    },
    {
      body: "#1c1b19",
      muted: "#161513",
      surface: "#252421",
      popover: "#2e2c29",
      neutral: "#33312d",
      text: "#f3f1ec",
      secondary: "#b3ada2",
      border: "#34322e",
      emphasized: "#7d786e",
      accent: "#f3f1ec",
      onAccent: "#1c1b19",
      accentMuted: "#33312d",
    },
  ),
  components: controlStyles,
});
