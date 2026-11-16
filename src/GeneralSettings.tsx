import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AudioWaveform, Keyboard, Mic, Power } from "lucide-react";
import { z } from "zod";
import { Button } from "@astryxdesign/core/Button";
import { Selector } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import {
  readDevices,
  readSettings,
  writeSettings,
  type Settings,
} from "./settings";
import { SettingsRow as Row, SettingsGroup } from "./ui";
import { LoadError, Loading, SaveStatus } from "./SettingsStatus";
import ShortcutRecorder from "./ShortcutRecorder";
import { modelLabel, readModels, speechModelSchema } from "./models";
import MicrophoneTest from "./MicrophoneTest";

const SYSTEM_DEFAULT = "system-default";

export default function General({
  onOpenModels,
}: {
  onOpenModels: () => void;
}) {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const models = useQuery({ queryKey: ["models"], queryFn: readModels });
  const devices = useQuery({ queryKey: ["devices"], queryFn: readDevices });
  const [recording, setRecording] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState("");
  const mutation = useMutation({
    mutationFn: writeSettings,
    onSuccess: (settings) => client.setQueryData(["settings"], settings),
  });

  useEffect(() => {
    const subscription = getCurrentWindow().onFocusChanged(
      ({ payload: focused }) => {
        if (!focused) {
          setRecording(false);
          setTesting(false);
          invoke("capture_shortcut", { enabled: false }).catch((e) =>
            setError(String(e)),
          );
          invoke("microphone_test", { enabled: false }).catch((e) =>
            setError(String(e)),
          );
        } else {
          void client.invalidateQueries({ queryKey: ["settings"] });
          void client.invalidateQueries({ queryKey: ["devices"] });
          void client.invalidateQueries({ queryKey: ["models"] });
        }
      },
    );
    return () => {
      void subscription.then((unsubscribe) => unsubscribe());
      invoke("capture_shortcut", { enabled: false }).catch(console.error);
      invoke("microphone_test", { enabled: false }).catch(console.error);
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

  async function testMicrophone() {
    setError("");
    try {
      await invoke("microphone_test", { enabled: !testing });
      setTesting(!testing);
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
  const installed = (models.data ?? []).filter(
    (model) => model.state.kind === "installed",
  );
  const activeMissing =
    models.data !== undefined &&
    !installed.some((model) => model.id === settings.speechModel);
  const noneInstalled = models.data !== undefined && installed.length === 0;
  const activeName =
    models.data?.find((model) => model.id === settings.speechModel)?.name ??
    settings.speechModel;
  return (
    <>
      <SettingsGroup
        title="Dictation"
        icon={<Keyboard size={14} aria-hidden="true" />}
      >
        <ShortcutRecorder
          value={settings.shortcut}
          recording={recording}
          disabled={busy}
          onRecordingChange={setRecording}
          onSave={async (shortcut) => {
            await mutation.mutateAsync({ ...settings, shortcut });
          }}
          onError={setError}
        />
        <Row
          label="Shortcut mode"
          help={
            settings.shortcutMode === "hold"
              ? "Hold to record. Release to insert your words."
              : "Tap to record. Tap again to insert your words."
          }
        >
          <Selector
            label="Shortcut mode"
            isLabelHidden
            value={settings.shortcutMode}
            isDisabled={busy || recording}
            options={[
              { value: "hold", label: "Hold to talk" },
              { value: "toggle", label: "Toggle" },
            ]}
            onChange={(value) =>
              void save({
                shortcutMode: z.enum(["hold", "toggle"]).parse(value),
              })
            }
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Transcription"
        icon={<AudioWaveform size={14} aria-hidden="true" />}
      >
        <Row
          label="Speech model"
          help={
            noneInstalled
              ? "No speech model yet. Download one to start dictating."
              : activeMissing
                ? `${activeName} isn’t installed. Choose another model.`
                : undefined
          }
        >
          {noneInstalled ? (
            <Button
              label="Download a model"
              variant="primary"
              onClick={onOpenModels}
            />
          ) : (
            <Selector
              label="Speech model"
              isLabelHidden
              value={settings.speechModel}
              isDisabled={
                busy || recording || (!activeMissing && installed.length < 2)
              }
              options={[
                ...(activeMissing
                  ? [
                      {
                        value: settings.speechModel,
                        label: `${activeName} (not installed)`,
                        disabled: true,
                      },
                    ]
                  : []),
                ...installed.map((model) => ({
                  value: model.id,
                  label: modelLabel(model),
                })),
              ]}
              onChange={(value) =>
                void save({ speechModel: speechModelSchema.parse(value) })
              }
            />
          )}
        </Row>
        {!noneInstalled && (
          <Row label="Models">
            <Button
              size="sm"
              variant="secondary"
              label="Manage models"
              onClick={onOpenModels}
            />
          </Row>
        )}
      </SettingsGroup>
      <SettingsGroup
        title="Audio input"
        icon={<Mic size={14} aria-hidden="true" />}
      >
        <Row label="Microphone">
          <Selector
            label="Microphone"
            isLabelHidden
            value={settings.microphone ?? SYSTEM_DEFAULT}
            isDisabled={
              busy ||
              testing ||
              recording ||
              devices.isPending ||
              devices.isError
            }
            options={[
              { value: SYSTEM_DEFAULT, label: "System default" },
              ...(settings.microphone &&
              !devices.data?.includes(settings.microphone)
                ? [
                    {
                      value: settings.microphone,
                      label: `${settings.microphone} (unavailable)`,
                    },
                  ]
                : []),
              ...[...new Set(devices.data ?? [])].map((name) => ({
                value: name,
                label: name,
              })),
            ]}
            onChange={(value) =>
              void save({ microphone: value === SYSTEM_DEFAULT ? null : value })
            }
            onFocus={() => void devices.refetch()}
          />
        </Row>
        <MicrophoneTest
          testing={testing}
          disabled={busy || recording}
          onToggle={testMicrophone}
          onError={setError}
          onTestingChange={setTesting}
        />
      </SettingsGroup>
      {devices.isError && (
        <LoadError
          error={devices.error}
          onRetry={() => void devices.refetch()}
        />
      )}
      <SettingsGroup
        title="Startup"
        icon={<Power size={14} aria-hidden="true" />}
      >
        <Row label="Launch at login">
          <Switch
            label="Launch at login"
            isLabelHidden
            value={settings.launchAtLogin}
            isDisabled={busy || recording}
            onChange={(checked) => void save({ launchAtLogin: checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
