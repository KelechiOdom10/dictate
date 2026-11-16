import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { AnnotationEntry } from "./annotations.generated";
export type { AnnotationEntry } from "./annotations.generated";

export const annotationEntrySchema = z
  .object({
    id: z.string(),
    startedAt: z.string(),
    durationMs: z.number().int().nonnegative(),
    app: z.string().nullable(),
    frames: z.number().int().nonnegative(),
    marks: z.number().int().nonnegative(),
    text: z.string(),
    folder: z.string(),
    hasContactSheet: z.boolean(),
    packagingMs: z.number().int().nonnegative().nullable(),
    partial: z.array(z.string()),
  })
  .strict() satisfies z.ZodType<AnnotationEntry>;

export const readAnnotations = async () =>
  z.array(annotationEntrySchema).parse(await invoke("list_annotations"));
export const revealAnnotation = (id: string) =>
  invoke<null>("reveal_annotation", { id });
export const copyAnnotation = (id: string) =>
  invoke<null>("copy_annotation", { id });
export const deleteAnnotation = (id: string) =>
  invoke<null>("delete_annotation", { id });
export const readScreenPermission = async () =>
  z.boolean().parse(await invoke("screen_permission"));
export const openScreenRecordingSettings = () =>
  invoke<null>("open_screen_recording_settings");

/** `1:05`, as `summary.md` writes durations. */
export function clock(ms: number) {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)}:${(seconds % 60).toString().padStart(2, "0")}`;
}

export function annotationTitle(entry: AnnotationEntry) {
  const said = entry.text.trim();
  if (said) return said;
  return entry.marks === 1
    ? "1 mark, nothing said"
    : `${entry.marks} marks, nothing said`;
}

export function annotationMeta(entry: AnnotationEntry) {
  const frames = entry.frames === 1 ? "1 frame" : `${entry.frames} frames`;
  return [
    entry.partial.length > 0 && "Partial",
    entry.app,
    clock(entry.durationMs),
    frames,
  ]
    .filter(Boolean)
    .join(" · ");
}
