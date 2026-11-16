import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { readSettings, writeSettings, type Settings } from "./settings";

const key = ["settings"];

/**
 * Reads settings and saves changes in order. Each change shows at once and is
 * written on top of every change before it; a rejected change reloads what
 * the runtime holds, keeping the last error.
 */
export function useSettings() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: key, queryFn: readSettings });
  const [error, setError] = useState("");
  const mutation = useMutation({
    mutationKey: key,
    scope: { id: "settings" },
    mutationFn: (patch: Partial<Settings>) =>
      writeSettings({ ...client.getQueryData<Settings>(key)!, ...patch }),
    onMutate: (patch) =>
      client.setQueryData<Settings>(key, (settings) =>
        settings ? { ...settings, ...patch } : settings,
      ),
    onSuccess: (settings) => {
      if (client.isMutating({ mutationKey: key }) === 1)
        client.setQueryData(key, settings);
    },
    onError: () => client.invalidateQueries({ queryKey: key }),
  });
  async function save(patch: Partial<Settings>) {
    if (!query.data) return;
    setError("");
    try {
      await mutation.mutateAsync(patch);
    } catch (e) {
      setError(String(e));
    }
  }
  return { query, save, busy: mutation.isPending, error, setError };
}
