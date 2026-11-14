export type ShortcutMode = "hold" | "toggle";
export type Settings = {
  shortcut: string;
  shortcutMode: ShortcutMode;
  microphone: string | null;
  launchAtLogin: boolean;
};
