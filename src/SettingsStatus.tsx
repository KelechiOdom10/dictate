import type { UseQueryResult } from "@tanstack/react-query";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Text } from "@astryxdesign/core/Text";

export function LoadError({
  error,
  onRetry,
}: {
  error: unknown;
  onRetry: () => void;
}) {
  return (
    <Banner
      role="alert"
      status="error"
      title={String(error)}
      collapsible={false}
      endContent={<Button size="sm" label="Try again" onClick={onRetry} />}
    />
  );
}

export function Loading({ children }: { children: string }) {
  return (
    <Text role="status" color="secondary">
      {children}
    </Text>
  );
}

/** What a settings page shows before its settings have loaded. */
export function SettingsPending({ query }: { query: UseQueryResult }) {
  if (query.isError)
    return (
      <LoadError error={query.error} onRetry={() => void query.refetch()} />
    );
  return <Loading>Loading your preferences…</Loading>;
}

export function ErrorNotice({ error }: { error: string }) {
  if (!error) return null;
  return (
    <Banner role="alert" status="error" title={error} collapsible={false} />
  );
}

export function SaveStatus({ busy, error }: { busy: boolean; error: string }) {
  return (
    <>
      <ErrorNotice error={error} />
      <Text role="status" type="supporting">
        {busy ? "Saving…" : "Changes save automatically."}
      </Text>
    </>
  );
}
