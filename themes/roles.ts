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

// Shared shape for every control, so one edit moves all five themes: no
// resting borders, a hairline on hover and focus, and compact pills. Colours
// come from the role tokens above, never from a literal.
export const controlStyles = {
  card: {
    base: {
      borderColor: "color-mix(in oklch, var(--color-border), transparent 45%)",
    },
  },
  button: {
    base: {
      borderRadius: "var(--radius-element)",
      columnGap: "var(--spacing-1)",
    },
    "variant:secondary": {
      boxShadow: "none",
      backgroundColor: "color-mix(in oklch, var(--color-neutral), transparent 8%)",
      ":hover": { boxShadow: "inset 0 0 0 1px var(--color-border)" },
      ":focus-visible": {
        boxShadow: "inset 0 0 0 1px var(--color-border-emphasized)",
      },
    },
  },
  selector: {
    base: {
      borderRadius: "var(--radius-element)",
      paddingInline: "var(--spacing-2)",
      columnGap: "var(--spacing-1)",
    },
  },
  "text-input": {
    base: {
      borderColor: "transparent",
      ":hover": { borderColor: "var(--color-border)" },
      ":focus-within": { borderColor: "var(--color-border-emphasized)" },
    },
  },
  badge: {
    base: {
      paddingInline: "var(--spacing-2)",
      columnGap: "var(--spacing-1)",
    },
  },
  "segmented-control-item": {
    base: { paddingInline: "var(--spacing-2)", columnGap: "var(--spacing-1)" },
  },
  "side-nav-item": { base: { marginBlockEnd: "var(--spacing-1)" } },
  // The sidebar sits a step behind the page in every theme and mode, so the
  // two surfaces never read as one flat sheet.
  "side-nav": { base: { backgroundColor: "var(--color-background-muted)" } },
};
