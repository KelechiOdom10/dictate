import type { ReactNode } from "react";
import { Theme } from "@astryxdesign/core/theme";
import { themes } from "../astryx-themes";
import { dictatePaperTheme } from "../themes/paper/dictate-paper";
import "@astryxdesign/core/reset.css";
import "@astryxdesign/core/astryx.css";
import "../themes/paper/paper.css";

export function SettingsTheme({
  palette = "ember",
  mode = "system",
  children,
}: {
  palette?: "ember" | "paper";
  mode?: "system" | "light" | "dark";
  children: ReactNode;
}) {
  return (
    <Theme
      theme={palette === "paper" ? dictatePaperTheme : themes.dictate}
      mode={mode}
    >
      {children}
    </Theme>
  );
}
