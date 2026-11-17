import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { List, ListItem } from "@astryxdesign/core/List";
import { Text } from "@astryxdesign/core/Text";
import { Toolbar } from "@astryxdesign/core/Toolbar";
import { Trash2, X } from "lucide-react";
import { Divider } from "@astryxdesign/core/Divider";
import { VStack } from "@astryxdesign/core/VStack";

type Noun = { one: string; many: string };

export const count = (n: number, noun: Noun) =>
  `${n.toLocaleString()} ${n === 1 ? noun.one : noun.many}`;

/**
 * Checked rows, kept apart from the row being viewed. A row hidden by a
 * search or filter keeps its check, but is left out of the count and is never
 * deleted until it is shown again. While a delete runs, selecting cannot be
 * started or ended.
 */
export function useSelection<Id>(shown: Id[]) {
  const [selecting, setSelecting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [ids, setIds] = useState<Id[]>([]);
  const all = useRef<HTMLInputElement>(null);
  const checked = ids.filter((id) => shown.includes(id));
  return {
    selecting,
    checked,
    hidden: ids.length - checked.length,
    all,
    confirming,
    setConfirming: (open: boolean) => {
      if (!deleting) setConfirming(open);
    },
    deleting,
    start: () => {
      if (!deleting) setSelecting(true);
    },
    cancel: () => {
      if (deleting) return;
      setSelecting(false);
      setIds([]);
    },
    toggle: (id: Id, on: boolean) =>
      setIds((ids) =>
        on ? [...ids, id] : ids.filter((other) => other !== id),
      ),
    setAll: (on: boolean) =>
      setIds((ids) => [
        ...ids.filter((id) => !shown.includes(id)),
        ...(on ? shown : []),
      ]),
    async remove(onDelete: (ids: Id[]) => Promise<Id[]>) {
      const asked = checked;
      setDeleting(true);
      const deleted = await onDelete(asked).finally(() => {
        setDeleting(false);
        setConfirming(false);
      });
      if (deleted.length < asked.length) {
        setIds((ids) => ids.filter((id) => !deleted.includes(id)));
        return;
      }
      setSelecting(false);
      setIds([]);
    },
  };
}

export type Selection<Id> = ReturnType<typeof useSelection<Id>>;

/**
 * The toolbar's way in. While selecting, everything else lives in the
 * floating bar, and Escape ends selecting.
 */
export function SelectButton<Id>({
  selection,
  shown,
}: {
  selection: Selection<Id>;
  shown: number;
}) {
  const { selecting, confirming, deleting } = selection;
  const cancel = useRef(selection.cancel);
  cancel.current = selection.cancel;
  const button = useRef<HTMLButtonElement>(null);
  const wasSelecting = useRef(false);
  useEffect(() => {
    if (wasSelecting.current && !selecting) button.current?.focus();
    wasSelecting.current = selecting;
  }, [selecting]);
  useEffect(() => {
    if (!selecting || confirming || deleting) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) cancel.current();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [selecting, confirming, deleting]);
  if (selecting) return null;
  return (
    <Button
      ref={button}
      size="sm"
      variant="ghost"
      label="Select"
      isDisabled={shown === 0 || deleting}
      onClick={selection.start}
    />
  );
}

/** Follows the settings pane, so the bar centres on it rather than the window. */
function usePane(active: boolean) {
  const [pane, setPane] = useState({ left: 0, width: 0 });
  const dock = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    const main = dock.current?.closest('main, [role="main"]');
    if (!main) return;
    const measure = () => {
      const box = main.getBoundingClientRect();
      setPane({ left: box.left + main.clientLeft, width: main.clientWidth });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(main);
    window.addEventListener("resize", measure);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [active]);
  return [dock, pane] as const;
}

/**
 * While selecting, a pill pinned to the bottom of the History pane: the
 * select-all checkbox and the count, then Delete, then X to stop selecting.
 * A spacer as tall as the bar keeps the last row clear of it.
 */
export function SelectionFloat<Id>({
  selection,
  shown,
  scope,
  noun,
  consequence,
  isSettling = false,
  onDelete,
}: {
  selection: Selection<Id>;
  shown: number;
  scope: string;
  noun: Noun;
  consequence: string;
  /** The list is still loading the rows the count is about, so Delete waits. */
  isSettling?: boolean;
  onDelete: (ids: Id[]) => Promise<Id[]>;
}) {
  const { deleting, hidden } = selection;
  const [leaving, setLeaving] = useState(false);
  const [wasSelecting, setWasSelecting] = useState(selection.selecting);
  if (wasSelecting !== selection.selecting) {
    setWasSelecting(selection.selecting);
    setLeaving(!selection.selecting);
  }
  const [dock, pane] = usePane(selection.selecting || leaving);
  useEffect(() => {
    if (!leaving) return;
    const done = setTimeout(() => setLeaving(false), 150);
    return () => clearTimeout(done);
  }, [leaving]);
  const n = selection.checked.length;
  if (!selection.selecting && !leaving) return null;
  return (
    <VStack
      className={
        leaving
          ? "ui-selection-dock ui-selection-leaving"
          : "ui-selection-dock ui-selection-reserve"
      }
      ref={dock}
      inert={leaving}
      style={
        {
          "--pane-center": `${pane.left + pane.width / 2}px`,
          "--pane-width": `${pane.width}px`,
        } as CSSProperties
      }
    >
      <Toolbar
        className="ui-selection-pill"
        label={`Selected ${noun.many}`}
        size="md"
        gap={2}
        startContent={
          <HStack gap={2} vAlign="center">
            <CheckboxInput
              ref={selection.all}
              label={`Select all ${shown.toLocaleString()} ${scope}`}
              isLabelHidden
              value={n === 0 ? false : n === shown ? true : "indeterminate"}
              onChange={selection.setAll}
            />
            <Text
              role="status"
              textWrap="nowrap"
              className="ui-selection-count"
            >
              {n.toLocaleString()} selected
              {hidden > 0 && ` · ${hidden.toLocaleString()} hidden`}
            </Text>
          </HStack>
        }
        endContent={
          <>
            <Divider orientation="vertical" />
            <Button
              variant="ghost"
              label="Delete"
              icon={<Trash2 size={16} aria-hidden="true" />}
              isDisabled={n === 0 || deleting || isSettling}
              onClick={() => selection.setConfirming(true)}
            />
            <IconButton
              variant="ghost"
              label="Stop selecting"
              tooltip="Stop selecting"
              icon={<X size={16} aria-hidden="true" />}
              isDisabled={deleting}
              onClick={selection.cancel}
            />
            <AlertDialog
              isOpen={selection.confirming}
              onOpenChange={selection.setConfirming}
              title={`Delete ${count(n, noun)}?`}
              description={consequence}
              actionLabel={`Delete ${count(n, noun)}`}
              isActionLoading={deleting}
              onAction={() => selection.remove(onDelete)}
            />
          </>
        }
      />
    </VStack>
  );
}

/**
 * Deletes one at a time through the same call a single delete makes, and
 * says how far it got if one fails.
 */
export async function deleteEach<Id>(
  ids: Id[],
  remove: (id: Id) => Promise<unknown>,
  noun: Noun,
  deleted: Id[],
) {
  for (const id of ids) {
    try {
      await remove(id);
      deleted.push(id);
    } catch (e) {
      throw `Deleted ${deleted.length.toLocaleString()} of ${count(ids.length, noun)}. ${String(e)}`;
    }
  }
}

function ChecklistRow({
  title,
  meta,
  date,
  isCurrent,
  checked,
  onChange,
}: {
  title: string;
  meta?: string;
  date: string;
  isCurrent: boolean;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  const box = useRef<HTMLInputElement>(null);
  return (
    <ListItem
      className={
        isCurrent ? "ui-history-row ui-history-row-current" : "ui-history-row"
      }
      label={title}
      description={meta}
      interactiveRef={box}
      startContent={
        <CheckboxInput
          ref={box}
          size="sm"
          label={`Select ${title}`}
          isLabelHidden
          value={checked}
          onChange={onChange}
        />
      }
      endContent={<Text type="supporting">{date}</Text>}
    />
  );
}

/** The narrow layout while selecting: each whole row checks its box. */
export function Checklist<Id extends string | number>({
  label,
  rows,
  current,
  selection,
}: {
  label: string;
  rows: { id: Id; title: string; meta?: string; date: string }[];
  current: Id | null;
  selection: Selection<Id>;
}) {
  return (
    <Card padding={0}>
      <List hasDividers edge="bleed" aria-label={label}>
        {rows.map((row) => (
          <ChecklistRow
            key={row.id}
            title={row.title}
            meta={row.meta}
            date={row.date}
            isCurrent={row.id === current}
            checked={selection.checked.includes(row.id)}
            onChange={(on) => selection.toggle(row.id, on)}
          />
        ))}
      </List>
    </Card>
  );
}
