import { useDeferredValue, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Check, Copy, Trash2 } from "lucide-react";
import {
  deleteHistory,
  entryTitle,
  formatMs,
  formatSeconds,
  modelLabel,
  readHistory,
  relativeTime,
  type HistoryEntry,
  type TakeStatus,
} from "./history";
import { readSettings, settingsSchema, writeSettings } from "./settings";
import {
  Button,
  DetailPanel,
  EmptyState,
  FilterChips,
  IconButton,
  ListItem,
  SearchField,
  Select,
} from "./ui";

type Filter = "all" | TakeStatus;
const filters: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "pasted", label: "Pasted" },
  { value: "skipped", label: "Skipped" },
  { value: "failed", label: "Failed" },
];
const statusLabels: Record<TakeStatus, string> = {
  pasted: "Pasted",
  skipped: "Skipped",
  failed: "Failed",
};

function useNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

export default function HistoryPage() {
  const client = useQueryClient();
  const now = useNow();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const query = useDeferredValue(search.trim());
  const history = useQuery({
    queryKey: ["history", query],
    queryFn: () => readHistory(query),
    placeholderData: keepPreviousData,
  });
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const retention = useMutation({
    mutationFn: writeSettings,
    onSuccess: (saved) => {
      client.setQueryData(["settings"], saved);
      void client.invalidateQueries({ queryKey: ["history"] });
    },
    onError: (e) => setError(String(e)),
  });
  const remove = useMutation({
    mutationFn: deleteHistory,
    onSuccess: () => client.invalidateQueries({ queryKey: ["history"] }),
    onError: (e) => setError(String(e)),
  });

  useEffect(() => {
    const subscription = listen("history-changed", () => {
      void client.invalidateQueries({ queryKey: ["history"] });
    });
    return () => {
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, [client]);

  const entries = (history.data ?? []).filter(
    (entry) => filter === "all" || entry.status === filter,
  );
  const selected =
    entries.find((entry) => entry.id === selectedId) ?? entries[0];

  function selectNeighbour(offset: number, list: HTMLElement) {
    if (!selected) return;
    const index = entries.indexOf(selected) + offset;
    const next = entries[Math.min(Math.max(index, 0), entries.length - 1)];
    setSelectedId(next.id);
    list
      .querySelectorAll<HTMLButtonElement>(".ui-list-item-button")
      [entries.indexOf(next)]?.focus();
  }

  async function removeSelected() {
    if (!selected) return;
    setError("");
    const index = entries.indexOf(selected);
    const next = entries[index + 1] ?? entries[index - 1];
    await remove.mutateAsync(selected.id);
    setSelectedId(next?.id ?? null);
  }

  const toolbar = (
    <div className="history-toolbar">
      <SearchField
        label="Search history"
        placeholder="Search history"
        value={search}
        onChange={setSearch}
      />
      <Select
        aria-label="Keep history for"
        value={settings.data?.historyRetention ?? "week"}
        disabled={!settings.data || retention.isPending}
        onChange={(e) => {
          if (!settings.data) return;
          setError("");
          retention.mutate({
            ...settings.data,
            historyRetention: settingsSchema.shape.historyRetention.parse(
              e.target.value,
            ),
          });
        }}
      >
        <option value="day">Keep 1 day</option>
        <option value="week">Keep 7 days</option>
        <option value="month">Keep 30 days</option>
        <option value="forever">Keep forever</option>
      </Select>
    </div>
  );

  if (history.isPending)
    return (
      <div className="history">
        {toolbar}
        <div className="loading" role="status">
          Loading your history…
        </div>
      </div>
    );
  if (history.isError)
    return (
      <div className="history">
        {toolbar}
        <div className="notice" role="alert">
          <p>{String(history.error)}</p>
          <Button onClick={() => void history.refetch()}>Try again</Button>
        </div>
      </div>
    );

  const nothingYet = !query && history.data.length === 0;
  return (
    <div className="history">
      {toolbar}
      {!nothingYet && (
        <FilterChips
          label="Show"
          options={filters}
          value={filter}
          onChange={(value) =>
            setFilter(filters.find((f) => f.value === value)?.value ?? "all")
          }
        />
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {nothingYet ? (
        <EmptyState title="Nothing here yet">
          Dictate something and it appears here. Your history stays on this Mac.
        </EmptyState>
      ) : !selected ? (
        <EmptyState
          title={query ? `No matches for “${query}”` : "Nothing to show"}
          action={
            query ? (
              <Button onClick={() => setSearch("")}>Clear search</Button>
            ) : (
              <Button onClick={() => setFilter("all")}>Show all</Button>
            )
          }
        >
          {query
            ? "Try a different word, or the name of an app."
            : `No ${statusLabels[filter as TakeStatus].toLowerCase()} dictations in your history.`}
        </EmptyState>
      ) : (
        <div className="history-split">
          <div
            className="history-list"
            role="group"
            aria-label="Dictations"
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
              e.preventDefault();
              selectNeighbour(e.key === "ArrowDown" ? 1 : -1, e.currentTarget);
            }}
          >
            {entries.map((entry) => (
              <ListItem
                key={entry.id}
                title={entryTitle(entry)}
                meta={[
                  entry.app ?? "Unknown app",
                  entry.status === "pasted" ? null : statusLabels[entry.status],
                ]
                  .filter(Boolean)
                  .join(" · ")}
                date={relativeTime(entry.createdAtMs, now)}
                selected={entry.id === selected.id}
                onSelect={() => setSelectedId(entry.id)}
              />
            ))}
          </div>
          <Detail
            key={selected.id}
            entry={selected}
            now={now}
            deleting={remove.isPending}
            onDelete={() => void removeSelected().catch(() => {})}
            onError={setError}
          />
        </div>
      )}
    </div>
  );
}

const fullDate = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

function Detail({
  entry,
  now,
  deleting,
  onDelete,
  onError,
}: {
  entry: HistoryEntry;
  now: number;
  deleting: boolean;
  onDelete: () => void;
  onError: (error: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const reset = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(reset.current), []);
  async function copy() {
    onError("");
    try {
      await writeText(entry.text);
      setCopied(true);
      clearTimeout(reset.current);
      reset.current = setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      onError(String(e));
    }
  }
  const status =
    entry.status === "pasted"
      ? "Pasted"
      : `${statusLabels[entry.status]}${entry.detail ? ` · ${entry.detail}` : ""}`;
  return (
    <DetailPanel
      title={relativeTime(entry.createdAtMs, now)}
      subtitle={fullDate.format(entry.createdAtMs)}
      actions={
        <>
          <IconButton
            variant="ghost"
            label={copied ? "Copied" : "Copy text"}
            title={copied ? "Copied" : "Copy text"}
            disabled={!entry.text}
            onClick={() => void copy()}
          >
            {copied ? (
              <Check size={16} aria-hidden="true" />
            ) : (
              <Copy size={16} aria-hidden="true" />
            )}
          </IconButton>
          <IconButton
            variant="ghost"
            label="Delete dictation"
            title="Delete dictation"
            disabled={deleting}
            onClick={onDelete}
          >
            <Trash2 size={16} aria-hidden="true" />
          </IconButton>
          <span className="sr-only" role="status">
            {copied ? "Copied to clipboard" : ""}
          </span>
        </>
      }
      metadata={[
        { label: "Application", value: entry.app ?? "Unknown" },
        { label: "Speech model", value: modelLabel(entry.model) },
        { label: "Status", value: status },
        {
          label: "Latency",
          value: [
            `${formatSeconds(entry.audioMs)} audio`,
            entry.inferenceMs === null
              ? null
              : `${formatMs(entry.inferenceMs)} transcription`,
            `${formatMs(entry.totalMs)} total`,
          ]
            .filter(Boolean)
            .join(" · "),
        },
      ]}
    >
      {entry.text ? (
        <p>{entry.text}</p>
      ) : (
        <p className="ui-preview">
          {entry.status === "failed"
            ? "Nothing was transcribed."
            : "No words were heard, so nothing was pasted."}
        </p>
      )}
    </DetailPanel>
  );
}
