import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { HistoryEntry, HistoryList } from "./history.generated";
export type {
  HistoryEntry,
  HistoryList,
  TakeStatus,
} from "./history.generated";

export const historyEntrySchema = z
  .object({
    id: z.number().int(),
    text: z.string(),
    createdAtMs: z.number().int(),
    app: z.string().nullable(),
    model: z.string(),
    audioMs: z.number().int().nonnegative(),
    inferenceMs: z.number().int().nonnegative().nullable(),
    totalMs: z.number().int().nonnegative(),
    status: z.enum(["pasted", "skipped", "failed"]),
    detail: z.string().nullable(),
  })
  .strict() satisfies z.ZodType<HistoryEntry>;

export const historyListSchema = z
  .object({ entries: z.array(historyEntrySchema), hasMore: z.boolean() })
  .strict() satisfies z.ZodType<HistoryList>;

/** How many takes the list asks for at a time. */
export const HISTORY_PAGE = 500;
export const readHistory = async (search: string, limit: number) =>
  historyListSchema.parse(await invoke("list_history", { search, limit }));
export const deleteHistory = (id: number) =>
  invoke<null>("delete_history", { id });

export function entryTitle(entry: HistoryEntry) {
  const firstLine = entry.text.split("\n")[0]?.trim();
  if (firstLine) return firstLine;
  return entry.status === "failed" ? "Dictation failed" : "No speech detected";
}

export function modelLabel(model: string) {
  return model.startsWith("ggml-")
    ? `Whisper ${model.slice("ggml-".length)}`
    : model;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function relativeTime(then: number, now: number) {
  const elapsed = Math.max(0, now - then);
  if (elapsed < MINUTE) return "Just now";
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)}m ago`;
  if (elapsed < DAY) return `${Math.floor(elapsed / HOUR)}h ago`;
  if (elapsed < 7 * DAY) return `${Math.floor(elapsed / DAY)}d ago`;
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
  }).format(then);
}

const milliseconds = new Intl.NumberFormat(undefined, {
  maximumFractionDigits: 0,
});
const seconds = new Intl.NumberFormat(undefined, {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
export const formatMs = (ms: number) => `${milliseconds.format(ms)} ms`;
export const formatSeconds = (ms: number) => `${seconds.format(ms / 1000)} s`;
