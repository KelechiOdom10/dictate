import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { z } from "zod";
import { problem, problemSchema } from "./problem";

const statusSchema = z.object({
  phase: z.enum([
    "idle",
    "recording",
    "transcribing",
    "done",
    "copied",
    "skipped",
  ]),
  error: problemSchema.nullable(),
  notice: z.string().nullable().default(null),
});

const unreadable = (cause: unknown) => ({
  phase: "idle" as const,
  error: problem("Couldn’t read dictation status", "Restart dictate.", cause),
  notice: null,
});

export function useDictationStatus() {
  const [status, setStatus] = useState<z.infer<typeof statusSchema>>({
    phase: "idle",
    error: null,
    notice: null,
  });
  useEffect(() => {
    let disposed = false;
    const unlisten: UnlistenFn[] = [];
    const refresh = async () => {
      try {
        const result = statusSchema.parse(await invoke("pipeline_status"));
        if (!disposed) setStatus(result);
      } catch (error) {
        if (!disposed) setStatus(unreadable(error));
      }
    };
    void (async () => {
      try {
        for (const event of ["dictation-state", "dictation-error"]) {
          const stop = await listen(event, () => void refresh());
          if (disposed) stop();
          else unlisten.push(stop);
        }
        await refresh();
      } catch (error) {
        if (!disposed) setStatus(unreadable(error));
      }
    })();
    return () => {
      disposed = true;
      unlisten.forEach((stop) => stop());
    };
  }, []);
  return status;
}
