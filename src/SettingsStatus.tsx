import type { UseQueryResult } from "@tanstack/react-query";
import { Button } from "./ui";

/** What a settings page shows before its settings have loaded. */
export function SettingsPending({ query }: { query: UseQueryResult }) {
  if (query.isError)
    return (
      <div className="notice" role="alert">
        <p>{String(query.error)}</p>
        <Button onClick={() => void query.refetch()}>Try again</Button>
      </div>
    );
  return (
    <div className="loading" role="status">
      Loading your preferences…
    </div>
  );
}

export function SaveStatus({ busy, error }: { busy: boolean; error: string }) {
  return (
    <>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <p className="save-status" role="status">
        {busy ? "Saving…" : "Changes save automatically."}
      </p>
    </>
  );
}
