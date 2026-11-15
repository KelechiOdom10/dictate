export type ShortcutMode = "hold" | "toggle";
export type Settings = {
  version: number;
  shortcut: string;
  shortcutMode: ShortcutMode;
  microphone: string | null;
  launchAtLogin: boolean;
};
