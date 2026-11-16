import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { readSettings, writeSettings, type Settings } from "./settings";

/** Reads settings and saves one change at a time, keeping the last error. */
export function useSettings() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const [error, setError] = useState("");
  const mutation = useMutation({
    mutationFn: writeSettings,
    onSuccess: (settings) => client.setQueryData(["settings"], settings),
  });
  async function save(patch: Partial<Settings>) {
    if (!query.data) return;
    setError("");
    try {
      await mutation.mutateAsync({ ...query.data, ...patch });
    } catch (e) {
      setError(String(e));
    }
  }
  return { query, save, busy: mutation.isPending, error, setError };
}
