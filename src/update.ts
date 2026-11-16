import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { z } from "zod";

export const updateStateSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("idle") }),
  z.object({ kind: z.literal("checking") }),
  z.object({ kind: z.literal("upToDate") }),
  z.object({ kind: z.literal("downloading"), version: z.string() }),
  z.object({ kind: z.literal("waiting"), version: z.string() }),
  z.object({ kind: z.literal("installing"), version: z.string() }),
  z.object({ kind: z.literal("failed"), message: z.string() }),
]);
export type UpdateState = z.infer<typeof updateStateSchema>;

/** The sentence shown beside the button. Empty while there is nothing to say. */
export function updateMessage(state: UpdateState): string {
  switch (state.kind) {
    case "idle":
      return "";
    case "checking":
      return "Checking…";
    case "upToDate":
      return "dictate is up to date.";
    case "downloading":
      return `Downloading ${state.version}…`;
    case "waiting":
      return `${state.version} is downloaded. It installs and restarts dictate when you aren't dictating or using Annotate.`;
    case "installing":
      return `Installing ${state.version}. dictate restarts in a moment.`;
    case "failed":
      return state.message;
  }
}

export const isUpdateBusy = (state: UpdateState) =>
  ["checking", "downloading", "waiting", "installing"].includes(state.kind);

export const checkForUpdates = async (): Promise<UpdateState> => {
  try {
    return updateStateSchema.parse(await invoke("check_for_updates"));
  } catch (error) {
    return { kind: "failed", message: `Couldn't check for updates: ${error}` };
  }
};

/** The live update state, and a function that asks for a check now. */
export function useUpdates() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen("update-state", (event) => {
          const next = updateStateSchema.safeParse(event.payload);
          if (next.success && !disposed) setState(next.data);
        });
        if (disposed) unlisten();
        else stop = unlisten;
        const current = updateStateSchema.safeParse(
          await invoke("update_status"),
        );
        if (current.success && !disposed) setState(current.data);
      } catch (error) {
        console.warn("update status unavailable", error);
      }
    })();
    return () => {
      disposed = true;
      stop?.();
    };
  }, []);
  const check = useCallback(async () => setState(await checkForUpdates()), []);
  return { state, check };
}
