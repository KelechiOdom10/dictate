import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import type { ModelEntry, ModelState, SpeechModel } from "./models.generated";
import { problemSchema } from "./problem";
export type { ModelEntry, ModelState, SpeechModel } from "./models.generated";

export const speechModelSchema = z.enum([
  "tiny.en",
  "base.en",
  "small.en",
  "medium.en",
  "large-v3-turbo",
  "parakeet-unified-en",
]) satisfies z.ZodType<SpeechModel>;

const modelStateSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("available") }).strict(),
  z
    .object({
      kind: z.literal("downloading"),
      received: z.number().nonnegative(),
      total: z.number().positive(),
      cancelling: z.boolean(),
    })
    .strict(),
  z.object({ kind: z.literal("preparing") }).strict(),
  z.object({ kind: z.literal("installed") }).strict(),
  z.object({ kind: z.literal("failed"), message: z.string() }).strict(),
]) satisfies z.ZodType<ModelState>;

export const modelEntrySchema = z
  .object({
    id: speechModelSchema,
    name: z.string(),
    description: z.string(),
    bytes: z.number().int().positive(),
    accuracy: z.number().int().min(1).max(5),
    measuredMs: z.number().int().positive(),
    recommended: z.boolean(),
    licence: z.object({ name: z.string(), url: z.url() }).strict().nullable(),
    active: z.boolean(),
    state: modelStateSchema,
  })
  .strict() satisfies z.ZodType<ModelEntry>;

export const readModels = async () =>
  z.array(modelEntrySchema).parse(await invoke("list_models"));
export const downloadModel = (model: SpeechModel) =>
  invoke<null>("download_model", { model });
export const cancelDownload = (model: SpeechModel) =>
  invoke<null>("cancel_download", { model });
export const deleteModel = (model: SpeechModel) =>
  invoke<null>("delete_model", { model });
export const openModelLicence = (model: SpeechModel) =>
  invoke<null>("open_model_licence", { model });
export const readWhisperStatus = async () =>
  problemSchema.nullable().parse(await invoke("whisper_status"));

export const WHISPER_INSTALL = "brew install whisper.cpp";
/** Length of the generated sentence behind `measuredMs`. */
export const MEASURED_CLIP_SECONDS = 5.9;

/** Matches `models::label` in Rust, used by the menu bar. */
export const modelLabel = (model: Pick<ModelEntry, "name" | "recommended">) =>
  model.recommended ? `${model.name} (Recommended)` : model.name;

const accuracyWords = ["Basic", "Fair", "Good", "Very good", "Best"];
export const accuracyLabel = (accuracy: number) =>
  accuracyWords[accuracy - 1] ?? "Unknown";

const seconds = new Intl.NumberFormat(undefined, {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
});
export const speedLabel = (ms: number) =>
  ms < 1000
    ? `${Math.round(ms / 10) * 10} ms`
    : `${seconds.format(ms / 1000)} s`;

const size = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
export function formatBytes(bytes: number) {
  return bytes >= 1e9
    ? `${size.format(bytes / 1e9)} GB`
    : `${Math.round(bytes / 1e6)} MB`;
}
