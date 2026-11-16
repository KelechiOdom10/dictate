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
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Switch } from "@astryxdesign/core/Switch";
import { SettingsGroup, SettingsRow as Row } from "./ui";
import { LoadError, Loading, SaveStatus } from "./SettingsStatus";
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

  if (query.isPending) return <Loading>Loading your preferences…</Loading>;
  if (query.isError)
    return (
      <LoadError error={query.error} onRetry={() => void query.refetch()} />
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
          label="Annotate"
          help="Tap the shortcut to record your screen and voice, hold it or hold Shift and drag to draw, tap again to copy."
        >
          <Switch
            label="Annotate"
            isLabelHidden
            value={settings.annotateEnabled}
            isDisabled={busy || recording}
            onChange={(checked) => void save({ annotateEnabled: checked })}
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
          onSave={async (annotateShortcut) => {
            await mutation.mutateAsync({ ...settings, annotateShortcut });
          }}
          onError={setError}
        />
      </SettingsGroup>
      <SettingsGroup
        title="Screen recording"
        icon={<MonitorUp size={14} aria-hidden="true" />}
      >
        <Row
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
          <HStack gap={2} vAlign="center">
            <Badge
              role="status"
              aria-label="Screen recording permission"
              variant={
                permission.isPending
                  ? "neutral"
                  : allowed
                    ? "success"
                    : "warning"
              }
              label={
                permission.isPending ? "…" : allowed ? "Allowed" : "Not allowed"
              }
            />
            <Button
              size="sm"
              variant={allowed ? "secondary" : "primary"}
              label="Open System Settings"
              onClick={() => void openSystemSettings()}
            />
          </HStack>
        </Row>
        <Row
          label="Show in screen recordings"
          help="Off keeps dictate’s ink and bar out of the video. Key frames show your ink either way."
        >
          <Switch
            label="Show in screen recordings"
            isLabelHidden
            value={settings.annotateShowInRecordings}
            isDisabled={busy}
            onChange={(checked) =>
              void save({ annotateShowInRecordings: checked })
            }
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
