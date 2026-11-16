import type { KeyboardEvent } from "react";
import { Button } from "@astryxdesign/core/Button";
import { Kbd } from "@astryxdesign/core/Kbd";

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
export function KeyboardShortcut({ value }: { value: string }) {
  return <Kbd keys={shortcutKeys(value)} />;
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
  return (
    <Button
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
    >
      {recording ? (
        <>
          Press shortcut <Kbd keys="escape" />
        </>
      ) : (
        <KeyboardShortcut value={value} />
      )}
    </Button>
  );
}
