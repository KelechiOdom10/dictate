import {
  useDeferredValue,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Check, Copy, FolderOpen, Search, Trash2 } from "lucide-react";
import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Grid } from "@astryxdesign/core/Grid";
import { Heading } from "@astryxdesign/core/Heading";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { List, ListItem } from "@astryxdesign/core/List";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import {
  SegmentedControl,
  SegmentedControlItem,
} from "@astryxdesign/core/SegmentedControl";
import { Selector } from "@astryxdesign/core/Selector";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
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
import { KeyboardShortcut, SettingsGroup, SettingsRow } from "./ui";
import { ErrorNotice, LoadError, Loading } from "./SettingsStatus";
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
const retentionChoices: Record<Retention, string> = {
  day: "1 day",
  week: "7 days",
  month: "30 days",
  forever: "Forever",
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

export default function HistoryPage({
  initialView = "dictations",
}: {
  initialView?: "dictations" | "annotations";
}) {
  const [view, setView] = useState<string>(initialView);
  return (
    <VStack gap={4}>
      <SegmentedControl label="Show" value={view} onChange={setView}>
        <SegmentedControlItem value="dictations" label="Dictations" />
        <SegmentedControlItem value="annotations" label="Annotations" />
      </SegmentedControl>
      {view === "annotations" ? <Annotations /> : <Dictations />}
    </VStack>
  );
}

type Row<Id> = { id: Id; title: string; meta?: string; date: string };
function Rows<Id extends string | number>({
  label,
  rows,
  value,
  onChange,
}: {
  label: string;
  rows: Row<Id>[];
  value: Id;
  onChange: (id: Id) => void;
}) {
  return (
    <Card padding={0}>
      <List
        hasDividers
        edge="bleed"
        aria-label={label}
        onKeyDown={(event) => {
          const offsets: Record<string, number> = {
            ArrowDown: 1,
            ArrowUp: -1,
            Home: -rows.length,
            End: rows.length,
          };
          if (!(event.key in offsets)) return;
          event.preventDefault();
          const index =
            rows.findIndex((row) => row.id === value) + offsets[event.key];
          const next = rows[Math.min(Math.max(index, 0), rows.length - 1)];
          if (!next) return;
          onChange(next.id);
          const list = event.currentTarget;
          requestAnimationFrame(() =>
            list
              .querySelectorAll<HTMLElement>("button")
              [rows.indexOf(next)]?.focus(),
          );
        }}
      >
        {rows.map((row) => (
          <ListItem
            key={row.id}
            itemKey={row.id}
            label={row.title}
            description={row.meta}
            endContent={<Text type="supporting">{row.date}</Text>}
            isSelected={row.id === value}
            onClick={() => onChange(row.id)}
          />
        ))}
      </List>
    </Card>
  );
}

function Detail({
  title,
  subtitle,
  actions,
  metadata,
  children,
}: {
  title: string;
  subtitle: string;
  actions?: ReactNode;
  metadata: { label: string; value: string }[];
  children: ReactNode;
}) {
  return (
    <Card role="region" aria-label={title}>
      <VStack gap={4}>
        <HStack gap={2} hAlign="between" vAlign="start">
          <VStack gap={0}>
            <Heading level={2}>{title}</Heading>
            <Text type="supporting">{subtitle}</Text>
          </VStack>
          {actions && (
            <HStack gap={1} vAlign="center">
              {actions}
            </HStack>
          )}
        </HStack>
        {children}
        <MetadataList>
          {metadata.map((row) => (
            <MetadataListItem key={row.label} label={row.label}>
              {row.value}
            </MetadataListItem>
          ))}
        </MetadataList>
      </VStack>
    </Card>
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

  const nothingYet =
    !query && history.isSuccess && history.data.entries.length === 0;
  const toolbar = (
    <HStack gap={2} vAlign="center" wrap="wrap">
      <TextInput
        type="search"
        label="Search history"
        isLabelHidden
        placeholder="Search history"
        startIcon={Search}
        hasClear
        value={search}
        onChange={setSearch}
      />
      {history.isSuccess && !nothingYet && (
        <Selector
          label="Status"
          isLabelHidden
          variant="secondary"
          value={filter}
          options={filters}
          onChange={(value) =>
            setFilter(filters.find((f) => f.value === value)?.value ?? "all")
          }
        />
      )}
    </HStack>
  );
  const keepFor = (
    <SettingsGroup title="Storage">
      <SettingsRow
        label="Keep history for"
        help="Older dictations are deleted from this Mac automatically."
      >
        <Selector
          label="Keep history for"
          isLabelHidden
          value={pendingRetention ?? current}
          isDisabled={!settings.data || retention.isPending}
          options={retentionOrder.map((value) => ({
            value,
            label: retentionChoices[value],
          }))}
          onChange={(next) => {
            const value = settingsSchema.shape.historyRetention.parse(next);
            if (retentionOrder.indexOf(value) < retentionOrder.indexOf(current))
              setPendingRetention(value);
            else saveRetention(value);
          }}
        />
      </SettingsRow>
      <AlertDialog
        isOpen={pendingRetention !== null}
        onOpenChange={(open) => {
          if (!open) setPendingRetention(null);
        }}
        title={`Delete dictations older than ${retentionLabels[pendingRetention ?? current]}?`}
        description={`They are removed now, and from then on after ${retentionLabels[pendingRetention ?? current]}.`}
        actionLabel="Delete older dictations"
        onAction={() => {
          if (pendingRetention) saveRetention(pendingRetention);
        }}
      />
    </SettingsGroup>
  );

  if (history.isPending)
    return (
      <>
        {toolbar}
        <Loading>Loading your history…</Loading>
        {keepFor}
      </>
    );
  if (history.isError)
    return (
      <>
        {toolbar}
        <LoadError
          error={history.error}
          onRetry={() => void history.refetch()}
        />
        {keepFor}
      </>
    );

  const more = history.data.hasMore && (
    <HStack gap={2} vAlign="center" hAlign="between">
      <Text type="supporting">
        Showing the latest {history.data.entries.length.toLocaleString()}.
      </Text>
      <Button
        size="sm"
        isDisabled={history.isFetching}
        label={history.isFetching ? "Loading…" : "Load older dictations"}
        onClick={() => setLimit({ query, count: count + HISTORY_PAGE })}
      />
    </HStack>
  );
  return (
    <>
      {toolbar}
      <ErrorNotice error={error} />
      {nothingYet ? (
        <EmptyState
          headingLevel={2}
          title="Nothing here yet"
          description="Each dictation appears here."
          actions={
            settings.data && (
              <Text>
                {settings.data.shortcutMode === "hold" ? "Hold " : "Press "}
                <KeyboardShortcut value={settings.data.shortcut} /> and speak.
              </Text>
            )
          }
        />
      ) : !selected ? (
        <EmptyState
          headingLevel={2}
          title={query ? `No matches for “${query}”` : "Nothing to show"}
          description={
            query
              ? "Try a different word, or the name of an app."
              : `No ${statusLabels[filter as TakeStatus].toLowerCase()} dictations in your history.`
          }
          actions={
            query ? (
              <Button label="Clear search" onClick={() => setSearch("")} />
            ) : (
              <>
                <Button label="Show all" onClick={() => setFilter("all")} />
                {more}
              </>
            )
          }
        />
      ) : (
        <Grid columns={{ minWidth: 260, repeat: "fit" }} gap={4} align="start">
          <VStack gap={2}>
            <Rows
              label="Dictations"
              rows={entries.map((entry) => ({
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
          </VStack>
          <DictationDetail
            key={selected.id}
            entry={selected}
            now={now}
            deleting={remove.isPending}
            onDelete={() => void removeSelected().catch(() => {})}
            onError={setError}
          />
        </Grid>
      )}
      {keepFor}
    </>
  );
}

const fullDate = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

function DictationDetail({
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
    <Detail
      title="Dictation"
      subtitle={`${relativeTime(entry.createdAtMs, now)} · ${fullDate.format(entry.createdAtMs)}`}
      actions={
        <>
          <IconButton
            variant="ghost"
            label={copied ? "Copied" : "Copy text"}
            isDisabled={!entry.text}
            icon={
              copied ? (
                <Check size={16} aria-hidden="true" />
              ) : (
                <Copy size={16} aria-hidden="true" />
              )
            }
            onClick={() => void copy()}
          />
          <IconButton
            variant="ghost"
            label="Delete dictation"
            isDisabled={deleting || confirming}
            icon={<Trash2 size={16} aria-hidden="true" />}
            onClick={() => setConfirming(true)}
          />
          <Text role="status" aria-live="polite" type="supporting">
            {copied ? "Copied to clipboard" : ""}
          </Text>
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
      <AlertDialog
        isOpen={confirming}
        onOpenChange={setConfirming}
        title="Delete this dictation?"
        description="It can’t be recovered."
        actionLabel="Delete dictation"
        onAction={() => {
          setConfirming(false);
          onDelete();
        }}
      />
      {entry.text ? (
        <Text as="p">{entry.text}</Text>
      ) : (
        <Text as="p" color="secondary">
          {entry.status === "failed"
            ? "Nothing was transcribed."
            : "No words were heard, so nothing was pasted."}
        </Text>
      )}
    </Detail>
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
    return <Loading>Loading your annotations…</Loading>;
  if (annotations.isError)
    return (
      <LoadError
        error={annotations.error}
        onRetry={() => void annotations.refetch()}
      />
    );
  const entries = annotations.data;
  const selected =
    entries.find((entry) => entry.id === selectedId) ?? entries[0];
  if (!selected)
    return (
      <EmptyState
        headingLevel={2}
        title="No annotations yet"
        description="Each Annotate session appears here."
        actions={
          settings.data && (
            <Text>
              Tap <KeyboardShortcut value={settings.data.annotateShortcut} /> to
              record your screen, draw and talk.
            </Text>
          )
        }
      />
    );
  return (
    <>
      <ErrorNotice error={error} />
      <Grid columns={{ minWidth: 260, repeat: "fit" }} gap={4} align="start">
        <Rows
          label="Annotations"
          rows={entries.map((entry) => ({
            id: entry.id,
            title: annotationTitle(entry),
            meta: annotationMeta(entry),
            date: relativeTime(Date.parse(entry.startedAt), now),
          }))}
          value={selected.id}
          onChange={setSelectedId}
        />
        <AnnotationDetail
          key={selected.id}
          entry={selected}
          now={now}
          onError={setError}
        />
      </Grid>
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
    <Detail
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
      {entry.partial.length > 0 && (
        <Banner
          status="warning"
          title="Partial session."
          description={entry.partial.join(" ")}
          collapsible={false}
        />
      )}
      {entry.text ? (
        <Text as="p">{entry.text}</Text>
      ) : (
        <Text as="p" color="secondary">
          Nothing was said in this session.
        </Text>
      )}
      <HStack gap={2} vAlign="center">
        <Button
          size="sm"
          label={copied ? "Copied" : "Copy again"}
          icon={
            copied ? (
              <Check size={16} aria-hidden="true" />
            ) : (
              <Copy size={16} aria-hidden="true" />
            )
          }
          onClick={() => void copy()}
        />
        <Button
          size="sm"
          variant="ghost"
          label="Reveal in Finder"
          icon={<FolderOpen size={16} aria-hidden="true" />}
          onClick={() => void reveal()}
        />
        <Text role="status" aria-live="polite" type="supporting">
          {copied ? "Summary copied to clipboard" : ""}
        </Text>
      </HStack>
    </Detail>
  );
}
