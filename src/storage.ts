import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { StorageUsage } from "./storage.generated";
export type { StorageUsage } from "./storage.generated";

export const storageUsageSchema = z
  .object({
    historyBytes: z.number().int().nonnegative(),
    annotationBytes: z.number().int().nonnegative(),
  })
  .strict() satisfies z.ZodType<StorageUsage>;

export const readStorageUsage = async () =>
  storageUsageSchema.parse(await invoke("storage_usage"));

const units = ["B", "KB", "MB", "GB"] as const;

/** `0 B`, `412 KB`, `18.4 MB`, `1.2 GB`: the size a person reads at a glance. */
export function formatBytes(bytes: number) {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const shown =
    unit === 0 || value >= 100 ? Math.round(value) : value.toFixed(1);
  return `${shown} ${units[unit]}`;
}
