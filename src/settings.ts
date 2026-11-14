import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { Settings } from "./settings.generated";
export type { Settings } from "./settings.generated";

export const settingsSchema: z.ZodType<Settings> = z
  .object({
    shortcut: z.string(),
    shortcutMode: z.enum(["hold", "toggle"]),
    microphone: z.string().nullable(),
    launchAtLogin: z.boolean(),
  })
  .strict();

export const readSettings = async () =>
  settingsSchema.parse(await invoke("get_settings"));
export const writeSettings = async (settings: Settings) =>
  settingsSchema.parse(await invoke("save_settings", { settings }));
export const readDevices = async () =>
  z.array(z.string()).parse(await invoke("input_devices"));

export function shortcutLabel(shortcut: string) {
  return shortcut
    .replace(/CommandOrControl|Super|Meta/g, "⌘")
    .replace(/Control/g, "⌃")
    .replace(/Alt|Option/g, "⌥")
    .replace(/Shift/g, "⇧")
    .replace(/Key|Digit/g, "")
    .replace(/\+/g, " ");
}
