import { useDictationStatus } from "./useDictationStatus";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Text } from "@astryxdesign/core/Text";
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
  idleHelp = "",
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
  const request = useRef(0);
  const arming = useRef<Promise<unknown>>(Promise.resolve());
  const saving = useRef(false);
  useEffect(() => {
    if (!recording) request.current += 1;
  }, [recording]);
  useEffect(
    () => () => {
      request.current += 1;
    },
    [],
  );
  async function capture() {
    const generation = ++request.current;
    setConflict("");
    onError("");
    onRecordingChange(true);
    try {
      arming.current = invoke("capture_shortcut", { enabled: true });
      await arming.current;
    } catch (e) {
      if (request.current === generation) {
        onRecordingChange(false);
        setConflict(String(e));
      }
    }
  }
  async function cancel() {
    request.current += 1;
    onRecordingChange(false);
    try {
      await invoke("capture_shortcut", { enabled: false });
    } catch (e) {
      onError(String(e));
    }
  }
  async function commit(shortcut: string) {
    if (saving.current) return;
    const generation = request.current;
    saving.current = true;
    try {
      await arming.current;
      if (generation !== request.current) return;
      await onSave(shortcut);
    } catch (error) {
      if (generation !== request.current) return;
      setConflict(
        /shortcut unavailable|shortcut.*register/i.test(String(error))
          ? "Used by another app — pick another"
          : String(error),
      );
    } finally {
      saving.current = false;
      if (generation === request.current) await cancel();
    }
  }
  return (
    <SettingsRow
      label={label}
      help={
        (conflict ? <Text role="alert">{conflict}</Text> : "") ||
        (startupConflict ? "Used by another app — pick another" : "") ||
        idleHelp ||
        undefined
      }
    >
      <Recorder
        label={label}
        value={value}
        recording={recording}
        disabled={disabled}
        onStart={() => void capture()}
        onCancel={() => void cancel()}
        onCommit={(shortcut) => void commit(shortcut)}
        onError={setConflict}
      />
    </SettingsRow>
  );
}
