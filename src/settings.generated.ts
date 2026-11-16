export type ShortcutMode = "hold" | "toggle";
export type Theme = "ember" | "paper";
export type AppearanceMode = "system" | "light" | "dark";
export type OverlayPosition = "top" | "bottom";
export type Settings = {
  version: number;
  shortcut: string;
  shortcutMode: ShortcutMode;
  microphone: string | null;
  launchAtLogin: boolean;
  theme: Theme;
  appearanceMode: AppearanceMode;
  overlayPosition: OverlayPosition;
};
