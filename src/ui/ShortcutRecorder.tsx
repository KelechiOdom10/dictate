import type { KeyboardEvent } from "react";
import { Button, type ControlSize } from "./Button";

const keyLabels: Record<string, string> = {
  CommandOrControl: "⌘",
  Super: "⌘",
  Meta: "⌘",
  Control: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
};
export function KeyboardShortcut({
  value,
  size = "md",
}: {
  value: string;
  size?: ControlSize;
}) {
  return (
    <span className={`ui-keycaps ui-size-${size}`}>
      {value
        .split("+")
        .filter(Boolean)
        .map((key, index) => (
          <kbd key={index}>
            {keyLabels[key] ?? key.replace(/^(Key|Digit)/, "")}
          </kbd>
        ))}
    </span>
  );
}
type Props = {
  size?: ControlSize;
  id?: string;
  value: string;
  recording: boolean;
  disabled?: boolean;
  "aria-describedby"?: string;
  onStart: () => void;
  onCancel: () => void;
  onCommit: (shortcut: string) => void;
  onError: (error: string) => void;
};
export function ShortcutRecorder({
  size = "md",
  value,
  recording,
  onStart,
  onCancel,
  onCommit,
  onError,
  ...props
}: Props) {
  function keyDown(event: KeyboardEvent<HTMLButtonElement>) {
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
  return (
    <Button
      {...props}
      size={size}
      variant="primary"
      className={`ui-shortcut ${recording ? "ui-capturing" : ""}`}
      onClick={(event) => {
        event.currentTarget.focus();
        if (!recording) onStart();
      }}
      onKeyDown={keyDown}
      onBlur={() => {
        if (recording) onCancel();
      }}
    >
      {recording ? (
        "Press shortcut…"
      ) : (
        <KeyboardShortcut value={value} size={size} />
      )}
    </Button>
  );
}
