import type { KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import Row from "./SettingRow";
import { shortcutLabel } from "./settings";

type Props = {
  value: string;
  recording: boolean;
  disabled: boolean;
  onRecordingChange: (active: boolean) => void;
  onSave: (shortcut: string) => Promise<void>;
  onError: (error: string) => void;
};

export default function ShortcutRecorder({
  value,
  recording,
  disabled,
  onRecordingChange,
  onSave,
  onError,
}: Props) {
  async function capture() {
    onError("");
    try {
      await invoke("capture_shortcut", { enabled: true });
      onRecordingChange(true);
    } catch (e) {
      onError(String(e));
    }
  }

  async function keyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (!recording || event.repeat) return;
    event.preventDefault();
    if (event.key === "Escape") {
      onRecordingChange(false);
      try {
        await invoke("capture_shortcut", { enabled: false });
      } catch (e) {
        onError(String(e));
      }
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
    onRecordingChange(false);
    await onSave([...modifiers, event.code].join("+"));
    try {
      await invoke("capture_shortcut", { enabled: false });
    } catch (e) {
      onError(String(e));
    }
  }

  return (
    <Row
      id="shortcut"
      label="Shortcut"
      help={
        recording
          ? "Press a key combination. Escape cancels."
          : "Use this shortcut from any app."
      }
    >
      <button
        id="shortcut"
        className={`shortcut ${recording ? "capturing" : ""}`}
        aria-describedby="shortcut-help"
        disabled={disabled}
        onClick={(event) => {
          event.currentTarget.focus();
          void capture();
        }}
        onKeyDown={(e) => void keyDown(e)}
        onBlur={() => {
          if (recording) {
            onRecordingChange(false);
            invoke("capture_shortcut", { enabled: false }).catch((e) =>
              onError(String(e)),
            );
          }
        }}
      >
        {recording ? "Press shortcut…" : <kbd>{shortcutLabel(value)}</kbd>}
      </button>
    </Row>
  );
}
