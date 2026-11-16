import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

export const appFoldersSchema = z
  .object({ data: z.string(), logs: z.string() })
  .strict();
export type AppFolders = z.infer<typeof appFoldersSchema>;
export type Location = "data" | "logs" | "source";

export const readAppFolders = async () =>
  appFoldersSchema.parse(await invoke("app_folders"));
export const openLocation = (location: Location) =>
  invoke<null>("open_location", { location });
