import {
  useCallback,
  useDeferredValue,
  useEffect,
  useLayoutEffect,
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
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Collapsible, CollapsibleGroup } from "@astryxdesign/core/Collapsible";
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
import { VisuallyHidden } from "@astryxdesign/core/VisuallyHidden";
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
import { formatBytes, readStorageUsage } from "./storage";
import { KeyboardShortcut } from "./ui";
import { ErrorNotice, LoadError, Loading } from "./SettingsStatus";
import {
  Checklist,
  deleteEach,
  SelectButton,
  SelectionFloat,
  useSelection,
  type Selection,
} from "./HistorySelection";
import {
  annotationMeta,
  annotationTitle,
  clock,
  copyAnnotation,
  deleteAnnotation,
  readAnnotations,
  revealAnnotation,
  type AnnotationEntry,
} from "./annotations";

type Filter = "all" | TakeStatus;
type Retention = Settings["historyRetention"];
const filters: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "pasted", label: "Inserted" },
  { value: "copied", label: "Copied" },
  { value: "skipped", label: "Nothing heard" },
  { value: "failed", label: "Failed" },
];
const statusLabels: Record<TakeStatus, string> = {
  pasted: "Inserted",
  copied: "Copied",
  skipped: "Nothing heard",
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
const dictationNoun = { one: "dictation", many: "dictations" };
const annotationNoun = { one: "annotation", many: "annotations" };

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
      <VStack paddingBlockEnd={6}>
        <SegmentedControl label="Show" value={view} onChange={setView}>
          <SegmentedControlItem value="dictations" label="Dictations" />
          <SegmentedControlItem value="annotations" label="Annotations" />
        </SegmentedControl>
      </VStack>
      {view === "annotations" ? <Annotations /> : <Dictations />}
    </VStack>
  );
}

const PANE_MIN_WIDTH = 260;

function useTwoPanes() {
  const [twoPanes, setTwoPanes] = useState(true);
  const panes = useCallback((grid: HTMLDivElement | null) => {
    if (!grid) return;
    const measure = () => {
      const gap = parseFloat(getComputedStyle(grid).columnGap) || 0;
      setTwoPanes(grid.clientWidth >= 2 * PANE_MIN_WIDTH + gap);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => observer.disconnect();
  }, []);
  return [panes, twoPanes] as const;
}

function scrollParent(node: Element) {
  for (let el = node.parentElement; el; el = el.parentElement) {
    const { overflowY } = getComputedStyle(el);
    if (
      (overflowY === "auto" || overflowY === "scroll") &&
      el.scrollHeight > el.clientHeight
    )
      return el;
  }
  return document.scrollingElement ?? document.documentElement;
}

type Row<Id> = { id: Id; title: string; meta?: string; date: string };

function Accordion<Id extends string | number>({
  label,
  rows,
  value,
  onChange,
  children,
}: {
  label: string;
  rows: Row<Id>[];
  value: Id | null;
  onChange: (id: Id | null) => void;
  children: ReactNode;
}) {
  const anchor = useRef<{ trigger: Element; top: number } | null>(null);
  useLayoutEffect(() => {
    const held = anchor.current;
    anchor.current = null;
    if (!held?.trigger.isConnected) return;
    const moved = held.trigger.getBoundingClientRect().top - held.top;
    if (moved) scrollParent(held.trigger).scrollTop += moved;
  }, [value]);
  return (
    <Card padding={2} role="group" aria-label={label}>
      <CollapsibleGroup
        hasDividers
        value={value === null ? [] : String(value)}
        onChange={(next) =>
          onChange(rows.find((row) => String(row.id) === next)?.id ?? null)
        }
      >
        {rows.map((row) => (
          <Collapsible
            key={row.id}
            className={
              row.id === value
                ? "ui-history-accordion ui-history-accordion-current"
                : "ui-history-accordion"
            }
            value={String(row.id)}
            onClickCapture={(event) => {
              const trigger = event.currentTarget.querySelector(
                ":scope > button[aria-expanded]",
              );
              if (trigger && trigger.contains(event.target as Node))
                anchor.current = {
                  trigger,
                  top: trigger.getBoundingClientRect().top,
                };
            }}
            trigger={
              <HStack gap={2} hAlign="between" vAlign="center">
                <VStack gap={0.5}>
                  <Text maxLines={2}>{row.title}</Text>
                  {row.meta && <Text type="supporting">{row.meta}</Text>}
                </VStack>
                <Text type="supporting" textWrap="nowrap">
                  {row.date}
                </Text>
              </HStack>
            }
          >
            {row.id === value && children}
          </Collapsible>
        ))}
      </CollapsibleGroup>
    </Card>
  );
}

function Rows<Id extends string | number>({
  label,
  rows,
  value,
  onChange,
  selection,
}: {
  label: string;
  rows: Row<Id>[];
  value: Id;
  onChange: (id: Id) => void;
  selection: Selection<Id>;
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
            className={
              row.id === value && !selection.selecting
                ? "ui-history-row ui-history-row-current"
                : "ui-history-row"
            }
            itemKey={row.id}
            label={row.title}
            description={row.meta}
            startContent={
              selection.selecting && (
                <CheckboxInput
                  size="sm"
                  label={`Select ${row.title}`}
                  isLabelHidden
                  value={selection.checked.includes(row.id)}
                  onChange={(on) => selection.toggle(row.id, on)}
                />
              )
            }
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
  isInline,
  children,
}: {
  title: string;
  subtitle: string;
  actions?: ReactNode;
  metadata: { label: string; value: string }[];
  isInline: boolean;
  children: ReactNode;
}) {
  const body = (
    <>
      <HStack gap={2} hAlign="between" vAlign={isInline ? "center" : "start"}>
        {isInline ? (
          <Text type="supporting">{subtitle}</Text>
        ) : (
          <VStack gap={1}>
            <Heading level={2}>{title}</Heading>
            <Text type="supporting">{subtitle}</Text>
          </VStack>
        )}
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
    </>
  );
  if (isInline)
    return (
      <VStack gap={4} role="region" aria-label={title}>
        {body}
      </VStack>
    );
  return (
    <Card role="region" aria-label={title}>
      <VStack gap={4}>{body}</VStack>
    </Card>
  );
}

function Dictations() {
  const client = useQueryClient();
  const now = useNow();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [panes, twoPanes] = useTwoPanes();
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
  const usage = useQuery({ queryKey: ["storage"], queryFn: readStorageUsage });
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
  const open = entries.find((entry) => entry.id === selectedId);
  const selected = open ?? entries[0];
  const current = settings.data?.historyRetention ?? "week";
  const selection = useSelection(entries.map((entry) => entry.id));
  const scope = [
    query ? `matching “${query}”` : "shown",
    filter === "all" ? "" : `as ${statusLabels[filter]}`,
  ]
    .filter(Boolean)
    .join(" ");

  async function removeChecked(ids: number[]) {
    setError("");
    try {
      await deleteEach(ids, deleteHistory, dictationNoun);
      selection.cancel();
    } catch (e) {
      setError(String(e));
    } finally {
      if (selectedId !== null && ids.includes(selectedId)) setSelectedId(null);
      void client.invalidateQueries({ queryKey: ["history"] });
      void client.invalidateQueries({ queryKey: ["storage"] });
    }
  }

  function saveRetention(value: Retention) {
    if (!settings.data) return;
    setPendingRetention(null);
    setError("");
    retention.mutate({ ...settings.data, historyRetention: value });
  }

  async function removeEntry(entry: HistoryEntry) {
    setError("");
    const index = entries.indexOf(entry);
    const next = entries[index + 1] ?? entries[index - 1];
    await remove.mutateAsync(entry.id);
    setSelectedId(next?.id ?? null);
  }

  const nothingYet =
    !query && history.isSuccess && history.data.entries.length === 0;
  const toolbar = (
    <HStack gap={3} vAlign="center" hAlign="between" wrap="wrap">
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
          <SelectButton selection={selection} shown={entries.length} />
        )}
        {history.isSuccess && !nothingYet && !selection.selecting && (
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
      <HStack gap={2} vAlign="center">
        <Text type="supporting">
          {usage.data
            ? `${formatBytes(usage.data.historyBytes)} on this Mac · keep`
            : "Measuring this Mac · keep"}
        </Text>
        <Selector
          label="Keep history for"
          isLabelHidden
          variant="secondary"
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
      </HStack>
    </HStack>
  );

  if (history.isPending)
    return (
      <>
        {toolbar}
        <Loading>Loading your history…</Loading>
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
      </>
    );

  const rows = entries.map((entry) => ({
    id: entry.id,
    title: entryTitle(entry),
    meta: [
      entry.app ?? "Unknown app",
      entry.status === "pasted" ? null : statusLabels[entry.status],
    ]
      .filter(Boolean)
      .join(" · "),
    date: relativeTime(entry.createdAtMs, now),
  }));
  const detailOf = (entry: HistoryEntry, isInline: boolean) => (
    <DictationDetail
      key={entry.id}
      entry={entry}
      now={now}
      isInline={isInline}
      deleting={remove.isPending}
      onDelete={() => void removeEntry(entry).catch(() => {})}
      onError={setError}
    />
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
        <Grid
          ref={panes}
          columns={{ minWidth: PANE_MIN_WIDTH, repeat: "fit" }}
          gap={4}
          align="start"
        >
          {twoPanes ? (
            <>
              <VStack gap={2}>
                <Rows
                  label="Dictations"
                  rows={rows}
                  value={selected.id}
                  onChange={setSelectedId}
                  selection={selection}
                />
                {more}
                <SelectionFloat
                  selection={selection}
                  shown={entries.length}
                  scope={scope}
                  noun={dictationNoun}
                  consequence="They can’t be recovered."
                  onDelete={removeChecked}
                />
              </VStack>
              {detailOf(selected, false)}
            </>
          ) : (
            <VStack gap={2}>
              {selection.selecting ? (
                <Checklist
                  label="Dictations"
                  rows={rows}
                  current={open?.id ?? null}
                  selection={selection}
                />
              ) : (
                <Accordion
                  label="Dictations"
                  rows={rows}
                  value={open?.id ?? null}
                  onChange={setSelectedId}
                >
                  {open && detailOf(open, true)}
                </Accordion>
              )}
              {more}
              <SelectionFloat
                selection={selection}
                shown={entries.length}
                scope={scope}
                noun={dictationNoun}
                consequence="They can’t be recovered."
                onDelete={removeChecked}
              />
            </VStack>
          )}
        </Grid>
      )}
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
  isInline,
  deleting,
  onDelete,
  onError,
}: {
  entry: HistoryEntry;
  now: number;
  isInline: boolean;
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
      ? "Inserted"
      : `${statusLabels[entry.status]}${entry.detail ? ` · ${entry.detail}` : ""}`;
  return (
    <Detail
      title="Dictation"
      isInline={isInline}
      subtitle={`${relativeTime(entry.createdAtMs, now)} · ${fullDate.format(entry.createdAtMs)}`}
      actions={
        <>
          <IconButton
            variant="ghost"
            label="Copy text"
            tooltip="Copy the text"
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
            tooltip="Delete this dictation"
            isDisabled={deleting || confirming}
            icon={<Trash2 size={16} aria-hidden="true" />}
            onClick={() => setConfirming(true)}
          />
          <VisuallyHidden role="status" aria-live="polite">
            {copied ? "Copied to clipboard" : ""}
          </VisuallyHidden>
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
  const [panes, twoPanes] = useTwoPanes();
  const [error, setError] = useState("");
  const annotations = useQuery({
    queryKey: ["annotations"],
    queryFn: readAnnotations,
  });
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const usage = useQuery({ queryKey: ["storage"], queryFn: readStorageUsage });
  const selection = useSelection(
    annotations.data?.map((entry) => entry.id) ?? [],
  );

  async function removeChecked(ids: string[]) {
    setError("");
    try {
      await deleteEach(ids, deleteAnnotation, annotationNoun);
      selection.cancel();
    } catch (e) {
      setError(String(e));
    } finally {
      if (selectedId !== null && ids.includes(selectedId)) setSelectedId(null);
      void client.invalidateQueries({ queryKey: ["annotations"] });
      void client.invalidateQueries({ queryKey: ["storage"] });
    }
  }

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
  const open = entries.find((entry) => entry.id === selectedId);
  const selected = open ?? entries[0];
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
  const rows = entries.map((entry) => ({
    id: entry.id,
    title: annotationTitle(entry),
    meta: annotationMeta(entry),
    date: relativeTime(Date.parse(entry.startedAt), now),
  }));
  const detailOf = (entry: AnnotationEntry, isInline: boolean) => (
    <AnnotationDetail
      key={entry.id}
      entry={entry}
      now={now}
      isInline={isInline}
      onError={setError}
      onDeleted={() => {
        const index = entries.indexOf(entry);
        const next = entries[index + 1] ?? entries[index - 1];
        setSelectedId(next?.id ?? null);
        void client.invalidateQueries({ queryKey: ["annotations"] });
      }}
    />
  );
  return (
    <>
      <ErrorNotice error={error} />
      <HStack gap={3} vAlign="center" hAlign="between" wrap="wrap">
        <SelectButton selection={selection} shown={entries.length} />
        <Text type="supporting">
          {usage.data
            ? `${formatBytes(usage.data.annotationBytes)} on this Mac · ${entries.length.toLocaleString()} ${entries.length === 1 ? "session" : "sessions"} · kept until deleted`
            : "Measuring this Mac"}
        </Text>
      </HStack>
      <Grid
        ref={panes}
        columns={{ minWidth: PANE_MIN_WIDTH, repeat: "fit" }}
        gap={4}
        align="start"
      >
        {twoPanes ? (
          <>
            <VStack gap={2}>
              <Rows
                label="Annotations"
                rows={rows}
                value={selected.id}
                onChange={setSelectedId}
                selection={selection}
              />
              <SelectionFloat
                selection={selection}
                shown={entries.length}
                scope="shown"
                noun={annotationNoun}
                consequence="Their recordings, frames and summaries are removed. They can’t be recovered."
                onDelete={removeChecked}
              />
            </VStack>
            {detailOf(selected, false)}
          </>
        ) : selection.selecting ? (
          <VStack gap={2}>
            <Checklist
              label="Annotations"
              rows={rows}
              current={open?.id ?? null}
              selection={selection}
            />
            <SelectionFloat
              selection={selection}
              shown={entries.length}
              scope="shown"
              noun={annotationNoun}
              consequence="Their recordings, frames and summaries are removed. They can’t be recovered."
              onDelete={removeChecked}
            />
          </VStack>
        ) : (
          <Accordion
            label="Annotations"
            rows={rows}
            value={open?.id ?? null}
            onChange={setSelectedId}
          >
            {open && detailOf(open, true)}
          </Accordion>
        )}
      </Grid>
    </>
  );
}

function AnnotationDetail({
  entry,
  now,
  isInline,
  onError,
  onDeleted,
}: {
  entry: AnnotationEntry;
  now: number;
  isInline: boolean;
  onError: (error: string) => void;
  onDeleted: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const reset = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(reset.current), []);
  const started = Date.parse(entry.startedAt);
  async function remove() {
    onError("");
    setDeleting(true);
    try {
      await deleteAnnotation(entry.id);
      onDeleted();
    } catch (e) {
      onError(String(e));
    } finally {
      setDeleting(false);
    }
  }
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
      isInline={isInline}
      subtitle={`${relativeTime(started, now)} · ${fullDate.format(started)}`}
      actions={
        <>
          <IconButton
            variant="ghost"
            label="Copy again"
            tooltip="Copy the summary"
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
            label="Reveal in Finder"
            tooltip="Show the session folder"
            icon={<FolderOpen size={16} aria-hidden="true" />}
            onClick={() => void reveal()}
          />
          <IconButton
            variant="ghost"
            label="Delete annotation"
            tooltip="Delete this session"
            isDisabled={deleting || confirming}
            icon={<Trash2 size={16} aria-hidden="true" />}
            onClick={() => setConfirming(true)}
          />
          <VisuallyHidden role="status" aria-live="polite">
            {copied ? "Summary copied to clipboard" : ""}
          </VisuallyHidden>
        </>
      }
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
      <AlertDialog
        isOpen={confirming}
        onOpenChange={setConfirming}
        title="Delete this annotation?"
        description="The recording, the frames and the summary are removed. It can’t be recovered."
        actionLabel="Delete annotation"
        onAction={() => {
          setConfirming(false);
          void remove();
        }}
      />
    </Detail>
  );
}
