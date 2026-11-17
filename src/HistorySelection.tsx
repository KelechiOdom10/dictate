import { useEffect, useRef, useState } from "react";
import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { List, ListItem } from "@astryxdesign/core/List";
import { Text } from "@astryxdesign/core/Text";
import { Toolbar } from "@astryxdesign/core/Toolbar";
import { X } from "lucide-react";

type Noun = { one: string; many: string };

export const count = (n: number, noun: Noun) =>
  `${n.toLocaleString()} ${n === 1 ? noun.one : noun.many}`;

/**
 * Checked rows, kept apart from the row being viewed. Only rows still shown
 * count, so a search or filter that hides a checked row also unchecks it.
 */
export function useSelection<Id>(shown: Id[]) {
  const [selecting, setSelecting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [ids, setIds] = useState<Id[]>([]);
  const all = useRef<HTMLInputElement>(null);
  const checked = ids.filter((id) => shown.includes(id));
  return {
    selecting,
    checked,
    all,
    confirming,
    setConfirming,
    start: () => setSelecting(true),
    cancel: () => {
      setSelecting(false);
      setIds([]);
    },
    clear: () => {
      setIds([]);
      all.current?.focus();
    },
    toggle: (id: Id, on: boolean) =>
      setIds(on ? [...checked, id] : checked.filter((other) => other !== id)),
    setAll: (on: boolean) => setIds(on ? shown : []),
  };
}

export type Selection<Id> = ReturnType<typeof useSelection<Id>>;

/**
 * The toolbar row: "Select" until selecting starts, then the select-all
 * checkbox naming its scope, and Done. Escape ends selecting.
 */
export function SelectionBar<Id>({
  selection,
  shown,
  scope,
}: {
  selection: Selection<Id>;
  shown: number;
  scope: string;
}) {
  const n = selection.checked.length;
  const { selecting, confirming, cancel } = selection;
  useEffect(() => {
    if (!selecting || confirming) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) cancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [selecting, confirming, cancel]);
  if (!selecting)
    return (
      <Button
        size="sm"
        variant="ghost"
        label="Select"
        isDisabled={shown === 0}
        onClick={selection.start}
      />
    );
  return (
    <HStack gap={2} vAlign="center">
      <CheckboxInput
        ref={selection.all}
        size="sm"
        label={`Select all ${shown.toLocaleString()} ${scope}`}
        value={n === 0 ? false : n === shown ? true : "indeterminate"}
        onChange={selection.setAll}
      />
      <Button size="sm" variant="ghost" label="Done" onClick={cancel} />
    </HStack>
  );
}

/**
 * Floats at the bottom of the list pane while anything is checked: the
 * count, Delete and a clear button. It sits in the flow after the rows, so
 * at the end of the pane it rests below the last row instead of over it.
 */
export function SelectionFloat<Id>({
  selection,
  noun,
  consequence,
  onDelete,
}: {
  selection: Selection<Id>;
  noun: Noun;
  consequence: string;
  onDelete: (ids: Id[]) => Promise<void>;
}) {
  const [deleting, setDeleting] = useState(false);
  const n = selection.checked.length;
  if (n === 0) return null;
  return (
    <Toolbar
      className="ui-selection-float"
      label={`Selected ${noun.many}`}
      size="sm"
      variant="section"
      gap={2}
      startContent={
        <Text role="status" textWrap="nowrap">
          {n.toLocaleString()} selected
        </Text>
      }
      endContent={
        <>
          <Button
            variant="destructive"
            label={`Delete ${n.toLocaleString()}…`}
            isDisabled={deleting}
            onClick={() => selection.setConfirming(true)}
          />
          <IconButton
            variant="ghost"
            label="Clear selection"
            tooltip="Clear selection"
            icon={<X size={16} aria-hidden="true" />}
            onClick={selection.clear}
          />
          <AlertDialog
            isOpen={selection.confirming}
            onOpenChange={selection.setConfirming}
            title={`Delete ${count(n, noun)}?`}
            description={consequence}
            actionLabel={`Delete ${count(n, noun)}`}
            isActionLoading={deleting}
            onAction={async () => {
              setDeleting(true);
              try {
                await onDelete(selection.checked);
              } finally {
                setDeleting(false);
                selection.setConfirming(false);
              }
            }}
          />
        </>
      }
    />
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
) {
  let done = 0;
  for (const id of ids) {
    try {
      await remove(id);
      done++;
    } catch (e) {
      throw `Deleted ${done.toLocaleString()} of ${count(ids.length, noun)}. ${String(e)}`;
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
