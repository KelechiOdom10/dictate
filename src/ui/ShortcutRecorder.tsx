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
      onCancel();
      return;
    }
    event.preventDefault();
    if (event.key === "Escape") {
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
    onCommit([...modifiers, event.code].join("+"));
  }
  // A capture that ends puts focus back on the button, and a focus ring left
  // behind after Escape reads as a stray line. The ring returns on the next
  // key or pointer, so keyboard navigation still shows it.
  const button = useRef<HTMLButtonElement>(null);
  const [quiet, setQuiet] = useState(false);
  const wasRecording = useRef(recording);
  // The capture state adds a dot and dims the keys, so the button's own width
  // is pinned to its resting width and the row cannot move.
  const [resting, setResting] = useState<number>();
  useEffect(() => {
    if (recording || !button.current) return;
    setResting(button.current.getBoundingClientRect().width);
  }, [recording, value]);
  useEffect(() => {
    if (wasRecording.current && !recording) {
      setQuiet(true);
      button.current?.focus({ preventScroll: true });
    }
    wasRecording.current = recording;
  }, [recording]);
  useEffect(() => {
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
        if (recording) onCancel();
      }}
      style={recording && resting ? { inlineSize: resting } : undefined}
    >
      {recording ? (
        <span className="ui-capture-wait">
          <StatusDot variant="error" label="Waiting for a shortcut" isPulsing />
        </span>
      ) : (
        <KeyboardShortcut value={value} />
      )}
    </Button>
  );
}
