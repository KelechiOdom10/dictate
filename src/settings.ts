import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { Settings } from "./settings.generated";
export type { Settings } from "./settings.generated";

export const settingsSchema = z
  .object({
    version: z.number().int().nonnegative(),
    shortcut: z.string(),
    shortcutMode: z.enum(["hold", "toggle"]),
    microphone: z.string().nullable(),
    launchAtLogin: z.boolean(),
    theme: z.enum(["ember", "paper"]),
    overlayPosition: z.enum(["top", "bottom"]),
    appearanceMode: z.enum(["system", "light", "dark"]),
  })
  .strict() satisfies z.ZodType<Settings>;

export const readSettings = async () =>
  settingsSchema.parse(await invoke("get_settings"));
export const writeSettings = async (settings: Settings) =>
  settingsSchema.parse(await invoke("save_settings", { settings }));
export const readDevices = async () =>
  z.array(z.string()).parse(await invoke("input_devices"));
