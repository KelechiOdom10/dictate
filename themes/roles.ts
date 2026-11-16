import type { DefinedTheme } from "@astryxdesign/core/theme";

type Side = Record<
  | "body"
  | "muted"
  | "surface"
  | "popover"
  | "neutral"
  | "text"
  | "secondary"
  | "border"
  | "emphasized"
  | "accent"
  | "onAccent"
  | "accentMuted"
  | "focus",
  string
>;

export function roleTokens(light: Side, dark: Side) {
  const pair = (role: keyof Side): [string, string] => [
    light[role],
    dark[role],
  ];
  return {
    "--color-background-body": pair("body"),
    "--color-background-muted": pair("muted"),
    "--color-background-surface": pair("surface"),
    "--color-background-card": pair("surface"),
    "--color-background-popover": pair("popover"),
    "--color-neutral": pair("neutral"),
    "--color-text-primary": pair("text"),
    "--color-text-secondary": pair("secondary"),
    "--color-icon-primary": pair("text"),
    "--color-icon-secondary": pair("secondary"),
    "--color-border": pair("border"),
    "--color-border-emphasized": pair("emphasized"),
    "--color-accent": pair("accent"),
    "--color-on-accent": pair("onAccent"),
    "--color-accent-muted": pair("accentMuted"),
    "--color-text-accent": pair("accent"),
    "--color-icon-accent": pair("accent"),
    "--focus-outline-color": pair("focus"),
  } satisfies DefinedTheme["tokens"];
}

export const pillButtons = {
  button: { base: { borderRadius: "var(--radius-pill, 9999px)" } },
};
