import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { MonitorUp, PenLine } from "lucide-react";
import { readSettings, writeSettings, type Settings } from "./settings";
import {
  openScreenRecordingSettings,
  readScreenPermission,
} from "./annotations";
import { Button, SettingsGroup, SettingsRow as Row, Toggle } from "./ui";
import ShortcutRecorder from "./ShortcutRecorder";

export default function AnnotatePage() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const permission = useQuery({
    queryKey: ["screen-permission"],
    queryFn: readScreenPermission,
  });
  const development = useQuery({
    queryKey: ["development-build"],
    queryFn: () => invoke<boolean>("development_build"),
  });
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState("");
  const mutation = useMutation({
    mutationFn: writeSettings,
    onSuccess: (settings) => client.setQueryData(["settings"], settings),
  });

  // Coming back from System Settings is the moment the grant may change.
  useEffect(() => {
    const subscription = getCurrentWindow().onFocusChanged(
      ({ payload: focused }) => {
        if (focused)
          void client.invalidateQueries({ queryKey: ["screen-permission"] });
        else {
          setRecording(false);
          invoke("capture_shortcut", { enabled: false }).catch((e) =>
            setError(String(e)),
          );
        }
      },
    );
    return () => {
      void subscription.then((unsubscribe) => unsubscribe());
      invoke("capture_shortcut", { enabled: false }).catch(console.error);
    };
  }, [client]);

  async function save(patch: Partial<Settings>) {
    if (!query.data) return;
    setError("");
    try {
      await mutation.mutateAsync({ ...query.data, ...patch });
    } catch (e) {
      setError(String(e));
    }
  }

  async function openSystemSettings() {
    setError("");
    try {
      await openScreenRecordingSettings();
    } catch (e) {
      setError(String(e));
    }
  }

  if (query.isPending)
    return (
      <div className="loading" role="status">
        Loading your preferences…
      </div>
    );
  if (query.isError)
    return (
      <div className="notice" role="alert">
        <p>{String(query.error)}</p>
        <Button onClick={() => void query.refetch()}>Try again</Button>
      </div>
    );
  const settings = query.data;
  const busy = mutation.isPending;
  const allowed = permission.data === true;
  return (
    <>
      <SettingsGroup
        title="Annotate"
        icon={<PenLine size={14} aria-hidden="true" />}
      >
        <Row
          id="annotate-enabled"
          label="Annotate"
          help="Tap the shortcut to record your screen and voice, hold it to draw, tap again to copy."
        >
          <Toggle
            id="annotate-enabled"
            checked={settings.annotateEnabled}
            disabled={busy || recording}
            aria-describedby="annotate-enabled-help"
            onChange={(e) => void save({ annotateEnabled: e.target.checked })}
          />
        </Row>
        <ShortcutRecorder
          id="annotate-shortcut"
          label="Shortcut"
          idleHelp="Starts and stops Annotate from any app."
          value={settings.annotateShortcut}
          recording={recording}
          disabled={busy || !settings.annotateEnabled}
          onRecordingChange={setRecording}
          onSave={(annotateShortcut) => save({ annotateShortcut })}
          onError={setError}
        />
      </SettingsGroup>
      <SettingsGroup
        title="Screen recording"
        icon={<MonitorUp size={14} aria-hidden="true" />}
      >
        <Row
          id="screen-permission"
          label="Permission"
          help={
            permission.isPending
              ? "Checking…"
              : allowed
                ? "Allowed. Annotate can record the screen."
                : development.data
                  ? "In development builds, turn on your terminal app (e.g. iTerm) in Screen Recording, then restart."
                  : "Not allowed. Turn on dictate in System Settings, then reopen dictate."
          }
        >
          <div className="ui-actions">
            <output
              id="screen-permission"
              className={
                allowed ? "permission-status" : "permission-status row-warning"
              }
              aria-describedby="screen-permission-help"
            >
              {permission.isPending ? "…" : allowed ? "Allowed" : "Not allowed"}
            </output>
            <Button
              size="sm"
              variant={allowed ? "ghost" : "primary"}
              onClick={() => void openSystemSettings()}
            >
              Open System Settings
            </Button>
          </div>
        </Row>
        <Row
          id="show-in-recordings"
          label="Show in screen recordings"
          help="Off keeps dictate’s ink and bar out of the video. Key frames show your ink either way."
        >
          <Toggle
            id="show-in-recordings"
            checked={settings.annotateShowInRecordings}
            disabled={busy}
            aria-describedby="show-in-recordings-help"
            onChange={(e) =>
              void save({ annotateShowInRecordings: e.target.checked })
            }
          />
        </Row>
      </SettingsGroup>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <p className="save-status" role="status">
        {busy ? "Saving…" : "Changes save automatically."}
      </p>
    </>
  );
}
