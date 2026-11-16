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
  | "accentMuted",
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
    "--font-weight-medium": "400",
    "--font-weight-bold": "600",
    "--text-heading-1-size": "var(--font-size-xl)",
    "--text-heading-1-weight": "var(--font-weight-semibold)",
    "--text-heading-2-size": "var(--font-size-lg)",
    "--text-heading-2-weight": "var(--font-weight-semibold)",
    "--text-heading-3-size": "var(--font-size-base)",
    "--text-heading-3-weight": "var(--font-weight-semibold)",
    "--text-large-size": "var(--font-size-lg)",
    "--text-large-weight": "var(--font-weight-semibold)",
    "--text-label-weight": "var(--font-weight-normal)",
  } satisfies DefinedTheme["tokens"];
}

export const pillButtons = {
  button: {
    base: { borderRadius: "var(--radius-pill, 9999px)" },
    "variant:secondary": {
      boxShadow: "inset 0 0 0 1px var(--color-border-emphasized)",
    },
  },
  "side-nav-item": { base: { marginBlockEnd: "var(--spacing-1)" } },
};
