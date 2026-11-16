import type { ReactNode } from "react";
import { Theme } from "@astryxdesign/core/theme";
import { themes, type ThemeSlug } from "../astryx-themes";
import type { Settings } from "../settings";
import "@astryxdesign/core/reset.css";
import "@astryxdesign/core/astryx.css";

export type ThemeId = Settings["theme"];
export type AppearanceMode = Settings["appearanceMode"];

export const THEMES: {
  id: ThemeId;
  slug: ThemeSlug;
  name: string;
  description: string;
}[] = [
  {
    id: "ember",
    slug: "ember",
    name: "Ember",
    description: "Warm brown, with an ember focus ring.",
  },
  {
    id: "paper",
    slug: "paper",
    name: "Paper",
    description: "Ink on warm white.",
  },
  {
    id: "capy",
    slug: "capy",
    name: "Capy",
    description: "Deep indigo, with a lavender focus ring.",
  },
  {
    id: "rosePine",
    slug: "rose-pine",
    name: "Rosé Pine",
    description: "Rosé Pine by night, Dawn by day.",
  },
  {
    id: "neutral",
    slug: "zinc",
    name: "Neutral",
    description: "Plain zinc greys.",
  },
];

export function themeFor(id: ThemeId | undefined) {
  return themes[(THEMES.find((theme) => theme.id === id) ?? THEMES[0]).slug];
}

export function SettingsTheme({
  theme,
  mode = "system",
  children,
}: {
  theme?: ThemeId;
  mode?: AppearanceMode;
  children: ReactNode;
}) {
  return (
    <Theme theme={themeFor(theme)} mode={mode}>
      {children}
    </Theme>
  );
}
