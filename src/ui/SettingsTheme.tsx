import { useSyncExternalStore, type ReactNode } from "react";
import { Theme } from "@astryxdesign/core/theme";
import { themes, type ThemeSlug } from "../astryx-themes";
import type { Settings } from "../settings";
import "@astryxdesign/core/reset.css";
import "@astryxdesign/core/astryx.css";

export type ThemeId = Settings["themeLight"];
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
    description: "Warm brown and ink.",
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
    description: "Deep indigo with lavender accents.",
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

const darkQuery = "(prefers-color-scheme: dark)";

/** Whether the Mac is in dark mode, kept current as it changes. */
export function useSystemDark() {
  return useSyncExternalStore(
    (change) => {
      const query = matchMedia(darkQuery);
      query.addEventListener("change", change);
      return () => query.removeEventListener("change", change);
    },
    () => matchMedia(darkQuery).matches,
  );
}

/**
 * The appearance that is actually on screen: System follows the Mac, and the
 * other two say so outright. Two themes need this resolved before either can
 * be picked, so every window asks the same question one way.
 */
export function shownDark(mode: AppearanceMode, systemDark: boolean) {
  return mode === "system" ? systemDark : mode === "dark";
}

/** The theme that applies in that appearance. */
export function themeForMode(
  themes: { themeLight: ThemeId; themeDark: ThemeId },
  dark: boolean,
) {
  return dark ? themes.themeDark : themes.themeLight;
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
