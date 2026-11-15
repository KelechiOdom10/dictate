import { useDictationStatus } from "./useDictationStatus";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SettingsRow, ShortcutRecorder as Recorder } from "./ui";

type Props = {
  id?: string;
  label?: string;
  idleHelp?: string;
  value: string;
  recording: boolean;
  disabled: boolean;
  onRecordingChange: (active: boolean) => void;
  onSave: (shortcut: string) => Promise<void>;
  onError: (error: string) => void;
};
export default function ShortcutRecorder({
  id = "shortcut",
  label = "Shortcut",
  idleHelp = "Use this shortcut from any app.",
  value,
  recording,
  disabled,
  onRecordingChange,
  onSave,
  onError,
}: Props) {
  const status = useDictationStatus();
  const startupConflict =
    status.error?.title ===
    (id === "annotate-shortcut"
      ? "Annotate shortcut unavailable"
      : "Shortcut unavailable");
  const [conflict, setConflict] = useState("");
  async function capture() {
    setConflict("");
    onError("");
    try {
      await invoke("capture_shortcut", { enabled: true });
      onRecordingChange(true);
    } catch (e) {
      onError(String(e));
    }
  }
  async function cancel() {
    onRecordingChange(false);
    try {
      await invoke("capture_shortcut", { enabled: false });
    } catch (e) {
      onError(String(e));
    }
  }
  async function commit(shortcut: string) {
    onRecordingChange(false);
    try {
      await onSave(shortcut);
    } catch (error) {
      if (/shortcut unavailable|shortcut.*register/i.test(String(error)))
        setConflict("Used by another app — pick another");
      else onError(String(error));
    } finally {
      await cancel();
    }
  }
  return (
    <SettingsRow
      id={id}
      label={label}
      help={
        conflict ||
        (startupConflict ? "Used by another app — pick another" : "") ||
        (recording ? "Press a key combination. Escape cancels." : idleHelp)
      }
    >
      <Recorder
        id={id}
        aria-describedby={`${id}-help`}
        value={value}
        recording={recording}
        disabled={disabled}
        onStart={() => void capture()}
        onCancel={() => void cancel()}
        onCommit={(shortcut) => void commit(shortcut)}
        onError={onError}
      />
    </SettingsRow>
  );
}
