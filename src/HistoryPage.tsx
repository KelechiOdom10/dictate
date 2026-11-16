import { useDeferredValue, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Check, Copy, FolderOpen, Trash2 } from "lucide-react";
import {
  deleteHistory,
  entryTitle,
  formatMs,
  formatSeconds,
  HISTORY_PAGE,
  modelLabel,
  readHistory,
  relativeTime,
  type HistoryEntry,
  type TakeStatus,
} from "./history";
import {
  readSettings,
  settingsSchema,
  writeSettings,
  type Settings,
} from "./settings";
import {
  Button,
  Confirm,
  DetailPanel,
  EmptyState,
  FilterChips,
  IconButton,
  KeyboardShortcut,
  SearchField,
  SegmentedControl,
  Select,
  SelectList,
} from "./ui";
import {
  annotationMeta,
  annotationTitle,
  clock,
  copyAnnotation,
  readAnnotations,
  revealAnnotation,
  type AnnotationEntry,
} from "./annotations";

type Filter = "all" | TakeStatus;
type Retention = Settings["historyRetention"];
const filters: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "pasted", label: "Pasted" },
  { value: "copied", label: "Copied" },
  { value: "skipped", label: "Skipped" },
  { value: "failed", label: "Failed" },
];
const statusLabels: Record<TakeStatus, string> = {
  pasted: "Pasted",
  copied: "Copied",
  skipped: "Skipped",
  failed: "Failed",
};
const retentionLabels: Record<Retention, string> = {
  day: "1 day",
  week: "7 days",
  month: "30 days",
  forever: "forever",
};
const retentionOrder: Retention[] = ["day", "week", "month", "forever"];

function useNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(timer);
  }, []);
  return now;
}

const views = [
  { value: "dictations", label: "Dictations" },
  { value: "annotations", label: "Annotations" },
];

export default function HistoryPage({
  initialView = "dictations",
}: {
  initialView?: "dictations" | "annotations";
}) {
  const [view, setView] = useState<string>(initialView);
  return (
    <div className="history">
      <SegmentedControl
        label="Show"
        options={views}
        value={view}
        onChange={setView}
      />
      {view === "annotations" ? <Annotations /> : <Dictations />}
    </div>
  );
}

function Dictations() {
  const client = useQueryClient();
  const now = useNow();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [pendingRetention, setPendingRetention] = useState<Retention | null>(
    null,
  );
  const [error, setError] = useState("");
  const query = useDeferredValue(search.trim());
  const [limit, setLimit] = useState({ query, count: HISTORY_PAGE });
  const count = limit.query === query ? limit.count : HISTORY_PAGE;
  const history = useQuery({
    queryKey: ["history", query, count],
    queryFn: () => readHistory(query, count),
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

  const entries = (history.data?.entries ?? []).filter(
    (entry) => filter === "all" || entry.status === filter,
  );
  const selected =
    entries.find((entry) => entry.id === selectedId) ?? entries[0];
  const current = settings.data?.historyRetention ?? "week";

  function saveRetention(value: Retention) {
    if (!settings.data) return;
    setPendingRetention(null);
    setError("");
    retention.mutate({ ...settings.data, historyRetention: value });
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
    <>
      <div className="history-toolbar">
        <SearchField
          label="Search history"
          placeholder="Search history"
          value={search}
          onChange={setSearch}
        />
        <Select
          aria-label="Keep history for"
          value={pendingRetention ?? current}
          disabled={!settings.data || retention.isPending}
          onChange={(e) => {
            const value = settingsSchema.shape.historyRetention.parse(
              e.target.value,
            );
            if (retentionOrder.indexOf(value) < retentionOrder.indexOf(current))
              setPendingRetention(value);
            else saveRetention(value);
          }}
        >
          {retentionOrder.map((value) => (
            <option key={value} value={value}>
              Keep {retentionLabels[value]}
            </option>
          ))}
        </Select>
      </div>
      {pendingRetention && (
        <Confirm
          question={`Delete dictations older than ${retentionLabels[pendingRetention]}?`}
          confirmLabel="Delete older dictations"
          onConfirm={() => saveRetention(pendingRetention)}
          onCancel={() => setPendingRetention(null)}
        >
          They are removed now, and from then on after{" "}
          {retentionLabels[pendingRetention]}.
        </Confirm>
      )}
    </>
  );

  if (history.isPending)
    return (
      <>
        {toolbar}
        <div className="loading" role="status">
          Loading your history…
        </div>
      </>
    );
  if (history.isError)
    return (
      <>
        {toolbar}
        <div className="notice" role="alert">
          <p>{String(history.error)}</p>
          <Button onClick={() => void history.refetch()}>Try again</Button>
        </div>
      </>
    );

  const nothingYet = !query && history.data.entries.length === 0;
  const more = history.data.hasMore && (
    <div className="history-more">
      <p className="ui-date">
        Showing the latest {history.data.entries.length.toLocaleString()}.
      </p>
      <Button
        size="sm"
        disabled={history.isFetching}
        onClick={() => setLimit({ query, count: count + HISTORY_PAGE })}
      >
        {history.isFetching ? "Loading…" : "Load older dictations"}
      </Button>
    </div>
  );
  return (
    <>
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
          {settings.data ? (
            <>
              {settings.data.shortcutMode === "hold" ? "Hold " : "Press "}
              <KeyboardShortcut value={settings.data.shortcut} /> and speak.
              Each dictation appears here.
            </>
          ) : (
            "Each dictation appears here."
          )}
        </EmptyState>
      ) : !selected ? (
        <EmptyState
          title={query ? `No matches for “${query}”` : "Nothing to show"}
          action={
            query ? (
              <Button onClick={() => setSearch("")}>Clear search</Button>
            ) : (
              <>
                <Button onClick={() => setFilter("all")}>Show all</Button>
                {more}
              </>
            )
          }
        >
          {query
            ? "Try a different word, or the name of an app."
            : `No ${statusLabels[filter as TakeStatus].toLowerCase()} dictations in your history.`}
        </EmptyState>
      ) : (
        <div className="history-split">
          <div className="history-list">
            <SelectList
              label="Dictations"
              items={entries.map((entry) => ({
                id: entry.id,
                title: entryTitle(entry),
                meta: [
                  entry.app ?? "Unknown app",
                  entry.status === "pasted" ? null : statusLabels[entry.status],
                ]
                  .filter(Boolean)
                  .join(" · "),
                date: relativeTime(entry.createdAtMs, now),
              }))}
              value={selected.id}
              onChange={setSelectedId}
            />
            {more}
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
    </>
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
  const [confirming, setConfirming] = useState(false);
  const trash = useRef<HTMLButtonElement>(null);
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
      title="Dictation"
      subtitle={`${relativeTime(entry.createdAtMs, now)} · ${fullDate.format(entry.createdAtMs)}`}
      actions={
        <>
          <IconButton
            variant="ghost"
            label={copied ? "Copied" : "Copy text"}
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
            ref={trash}
            variant="ghost"
            label="Delete dictation"
            disabled={deleting || confirming}
            onClick={() => setConfirming(true)}
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
        { label: "Audio", value: formatSeconds(entry.audioMs) },
        {
          label: "Transcription",
          value:
            entry.inferenceMs === null
              ? "Not run"
              : formatMs(entry.inferenceMs),
        },
        { label: "Release to finish", value: formatMs(entry.totalMs) },
      ]}
    >
      {confirming && (
        <Confirm
          question="Delete this dictation?"
          confirmLabel="Delete dictation"
          onConfirm={onDelete}
          onCancel={() => {
            setConfirming(false);
            requestAnimationFrame(() => trash.current?.focus());
          }}
        >
          It can’t be recovered.
        </Confirm>
      )}
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

function Annotations() {
  const client = useQueryClient();
  const now = useNow();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const annotations = useQuery({
    queryKey: ["annotations"],
    queryFn: readAnnotations,
  });
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });

  useEffect(() => {
    const subscription = listen("annotations-changed", () => {
      void client.invalidateQueries({ queryKey: ["annotations"] });
    });
    return () => {
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, [client]);

  if (annotations.isPending)
    return (
      <div className="loading" role="status">
        Loading your annotations…
      </div>
    );
  if (annotations.isError)
    return (
      <div className="notice" role="alert">
        <p>{String(annotations.error)}</p>
        <Button onClick={() => void annotations.refetch()}>Try again</Button>
      </div>
    );
  const entries = annotations.data;
  const selected =
    entries.find((entry) => entry.id === selectedId) ?? entries[0];
  if (!selected)
    return (
      <EmptyState title="No annotations yet">
        {settings.data ? (
          <>
            Tap <KeyboardShortcut value={settings.data.annotateShortcut} /> to
            record your screen, draw and talk. Each session appears here.
          </>
        ) : (
          "Each Annotate session appears here."
        )}
      </EmptyState>
    );
  return (
    <>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="history-split">
        <div className="history-list">
          <SelectList
            label="Annotations"
            items={entries.map((entry) => ({
              id: entry.id,
              title: annotationTitle(entry),
              meta: annotationMeta(entry),
              date: relativeTime(Date.parse(entry.startedAt), now),
            }))}
            value={selected.id}
            onChange={setSelectedId}
          />
        </div>
        <AnnotationDetail
          key={selected.id}
          entry={selected}
          now={now}
          onError={setError}
        />
      </div>
    </>
  );
}

function AnnotationDetail({
  entry,
  now,
  onError,
}: {
  entry: AnnotationEntry;
  now: number;
  onError: (error: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const reset = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(reset.current), []);
  const started = Date.parse(entry.startedAt);
  async function copy() {
    onError("");
    try {
      await copyAnnotation(entry.id);
      setCopied(true);
      clearTimeout(reset.current);
      reset.current = setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      onError(String(e));
    }
  }
  async function reveal() {
    onError("");
    try {
      await revealAnnotation(entry.id);
    } catch (e) {
      onError(String(e));
    }
  }
  return (
    <DetailPanel
      title="Annotation"
      subtitle={`${relativeTime(started, now)} · ${fullDate.format(started)}`}
      metadata={[
        { label: "Application", value: entry.app ?? "Unknown" },
        { label: "Length", value: clock(entry.durationMs) },
        { label: "Marks", value: entry.marks.toLocaleString() },
        {
          label: "Frames",
          value: entry.hasContactSheet
            ? `${entry.frames.toLocaleString()} with a contact sheet`
            : "None",
        },
        {
          label: "Stop to package",
          value:
            entry.packagingMs === null
              ? "Not recorded"
              : formatSeconds(entry.packagingMs),
        },
        { label: "Folder", value: entry.folder },
      ]}
    >
      {entry.text ? (
        <p>{entry.text}</p>
      ) : (
        <p className="ui-preview">Nothing was said in this session.</p>
      )}
      <div className="ui-actions annotation-actions">
        <Button size="sm" onClick={() => void copy()}>
          {copied ? (
            <Check size={16} aria-hidden="true" />
          ) : (
            <Copy size={16} aria-hidden="true" />
          )}
          {copied ? "Copied" : "Copy again"}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void reveal()}>
          <FolderOpen size={16} aria-hidden="true" />
          Reveal in Finder
        </Button>
        <span className="sr-only" role="status">
          {copied ? "Summary copied to clipboard" : ""}
        </span>
      </div>
    </DetailPanel>
  );
}
