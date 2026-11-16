import { invoke } from "@tauri-apps/api/core";
import { SettingsRow, ShortcutRecorder as Recorder } from "./ui";

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
    await onSave(shortcut);
    await cancel();
  }
  return (
    <SettingsRow
      id="shortcut"
      label="Shortcut"
      help={
        recording
          ? "Press a key combination. Escape cancels."
          : "Use this shortcut from any app."
      }
    >
      <Recorder
        id="shortcut"
        aria-describedby="shortcut-help"
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
