import type { SpeechModel } from "./models.generated";
export type ShortcutMode = "hold" | "toggle";
export type Theme = "ember" | "paper";
export type AppearanceMode = "system" | "light" | "dark";
export type OverlayPosition = "top" | "bottom";
export type PasteMethod = "paste" | "copyOnly";
export type MicMode = "onDemand" | "warm";
export type HistoryRetention = "day" | "week" | "month" | "forever";
export type Settings = {
  version: number;
  shortcut: string;
  shortcutMode: ShortcutMode;
  microphone: string | null;
  launchAtLogin: boolean;
  theme: Theme;
  appearanceMode: AppearanceMode;
  overlayPosition: OverlayPosition;
  historyRetention: HistoryRetention;
  speechModel: SpeechModel;
  pasteMethod: PasteMethod;
  restoreClipboard: boolean;
  trailingSpace: boolean;
  capitalizeFirst: boolean;
  micMode: MicMode;
  soundFeedback: boolean;
  debugRecordings: boolean;
  annotateShortcut: string;
  /**
   * Off: the Annotate shortcut is not registered at all.
   */
  annotateEnabled: boolean;
  /**
   * On: the recording also shows dictate's ink and floating bar.
   */
  annotateShowInRecordings: boolean;
};
