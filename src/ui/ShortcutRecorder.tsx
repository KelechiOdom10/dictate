import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "@astryxdesign/core/Button";
import { Kbd } from "@astryxdesign/core/Kbd";
import { StatusDot } from "@astryxdesign/core/StatusDot";

const keyNames: Record<string, string> = {
  CommandOrControl: "meta",
  Super: "meta",
  Meta: "meta",
  Control: "ctrl",
  Alt: "alt",
  Option: "alt",
  Shift: "shift",
  Escape: "escape",
  Enter: "enter",
  Tab: "tab",
  Backspace: "backspace",
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};
export function shortcutKeys(value: string) {
  return value
    .split("+")
    .filter(Boolean)
    .map((key) => keyNames[key] ?? key.replace(/^(Key|Digit)/, ""))
    .join("+");
}
const spokenNames: Record<string, string> = {
  CommandOrControl: "Command",
  Super: "Command",
  Meta: "Command",
  Option: "Alt",
};
function spokenKeys(value: string) {
  return value
    .split("+")
    .filter(Boolean)
    .map((key) => spokenNames[key] ?? key.replace(/^(Key|Digit|Arrow)/, ""))
    .join(" + ");
}
export function KeyboardShortcut({
  value,
  className,
}: {
  value: string;
  className?: string;
}) {
  return (
    <Kbd
      className={className ? `ui-keycap ${className}` : "ui-keycap"}
      keys={shortcutKeys(value)}
    />
  );
}
type Props = {
  value: string;
  label: string;
  recording: boolean;
  disabled?: boolean;
  onStart: () => void;
  onCancel: () => void;
  onCommit: (shortcut: string) => void;
  onError: (error: string) => void;
};
export function ShortcutRecorder({
  value,
  label,
  recording,
  disabled,
  onStart,
  onCancel,
  onCommit,
  onError,
}: Props) {
  function keyDown(event: KeyboardEvent<HTMLElement>) {
    if (!recording || event.repeat) return;
    if (event.key === "Tab") {
      restoreFocus.current = false;
      onCancel();
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      restoreFocus.current = true;
      onCancel();
      return;
    }
    if (["Meta", "Alt", "Control", "Shift"].includes(event.key)) return;
    const modifiers = [
      event.metaKey && "Super",
      event.ctrlKey && "Control",
      event.altKey && "Alt",
      event.shiftKey && "Shift",
    ].filter(Boolean);
    if (!modifiers.length) {
      onError("Include Command, Option, Control or Shift in your shortcut.");
      return;
    }
    restoreFocus.current = true;
    onCommit([...modifiers, event.code].join("+"));
  }
  // A capture that ends puts focus back on the button, and a focus ring left
  // behind after Escape reads as a stray line. The ring returns on the next
  // key or pointer, so keyboard navigation still shows it.
  const button = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const [quiet, setQuiet] = useState(false);
  const wasRecording = useRef(recording);
  // The capture state adds a dot and dims the keys, so the button's own width
  useEffect(() => {
    if (wasRecording.current && !recording) {
      setQuiet(true);
      if (restoreFocus.current) button.current?.focus({ preventScroll: true });
      restoreFocus.current = false;
    }
    wasRecording.current = recording;
  }, [recording]);
  useEffect(() => {
    // The library rings a row through `:has(:focus-visible)` at id-level
    // specificity, so a focused, quiet button still lights up the row around
    // it. Zeroing the token the ring is drawn with beats that cleanly.
    const row = button.current?.closest("li");
    row?.classList.toggle("ui-quiet-row", quiet);
    if (!quiet) return;
    const clear = () => setQuiet(false);
    window.addEventListener("keydown", clear, { once: true });
    window.addEventListener("pointerdown", clear, { once: true });
    return () => {
      window.removeEventListener("keydown", clear);
      window.removeEventListener("pointerdown", clear);
    };
  }, [quiet]);
  return (
    <Button
      ref={button}
      className={quiet ? "ui-quiet-focus" : undefined}
      label={
        recording
          ? `${label}, Press shortcut Esc to cancel`
          : `${label}, ${spokenKeys(value)}`
      }
      variant={recording ? "primary" : "secondary"}
      isDisabled={disabled}
      onClick={(event) => {
        event.currentTarget.focus();
        if (!recording) onStart();
      }}
      onKeyDown={keyDown}
      onBlur={() => {
        restoreFocus.current = false;
        if (recording) onCancel();
      }}
    >
      {/* Both states sit in one grid cell with an invisible copy of the widest
          one, so the button is its largest size from the start and the row
          never moves when a capture begins. */}
      <span className="ui-capture-stack">
        <span aria-hidden="true" className="ui-capture-ghost">
          <KeyboardShortcut value={value} />
        </span>
        <span aria-hidden="true" className="ui-capture-ghost ui-capture-live">
          <StatusDot variant="error" label="Waiting for a shortcut" isPulsing />
          Press shortcut <Kbd className="ui-keycap" keys="escape" />
        </span>
        {recording ? (
          <span className="ui-capture-live">
            <StatusDot
              variant="error"
              label="Waiting for a shortcut"
              isPulsing
            />
            Press shortcut <Kbd className="ui-keycap" keys="escape" />
          </span>
        ) : (
          <KeyboardShortcut value={value} />
        )}
      </span>
    </Button>
  );
}
