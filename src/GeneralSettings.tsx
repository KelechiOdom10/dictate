import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AudioWaveform, Keyboard, Mic, Power, SunMoon } from "lucide-react";
import { z } from "zod";
import {
  settingsSchema,
  readDevices,
  readSettings,
  writeSettings,
  type Settings,
} from "./settings";
import {
  Button,
  Select,
  Toggle,
  SettingsRow as Row,
  SettingsGroup,
} from "./ui";
import ShortcutRecorder from "./ShortcutRecorder";
import { modelLabel, readModels, speechModelSchema } from "./models";
import MicrophoneTest from "./MicrophoneTest";

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
          id="mode"
          label="Shortcut mode"
          help={
            settings.shortcutMode === "hold"
              ? "Hold to record. Release to insert your words."
              : "Tap to record. Tap again to insert your words."
          }
        >
          <Select
            id="mode"
            aria-describedby="mode-help"
            value={settings.shortcutMode}
            disabled={busy || recording}
            onChange={(e) =>
              void save({
                shortcutMode: z.enum(["hold", "toggle"]).parse(e.target.value),
              })
            }
          >
            <option value="hold">Hold to talk</option>
            <option value="toggle">Toggle</option>
          </Select>
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Transcription"
        icon={<AudioWaveform size={14} aria-hidden="true" />}
      >
        <Row
          id="speech-model"
          label="Speech model"
          help={
            noneInstalled ? (
              <span className="row-warning">
                No speech model yet. Download one to start dictating.
              </span>
            ) : (
              <>
                {activeMissing ? (
                  <span className="row-warning">
                    {activeName} isn’t installed. Choose another model.
                  </span>
                ) : (
                  "Used for every dictation."
                )}{" "}
                <button
                  type="button"
                  className="row-link"
                  onClick={onOpenModels}
                >
                  Manage models
                </button>
              </>
            )
          }
        >
          {noneInstalled ? (
            <Button
              id="speech-model"
              aria-label="Download a model"
              aria-describedby="speech-model-help"
              variant="primary"
              onClick={onOpenModels}
            >
              Download a model
            </Button>
          ) : (
            <Select
              id="speech-model"
              aria-describedby="speech-model-help"
              value={settings.speechModel}
              disabled={
                busy || recording || (!activeMissing && installed.length < 2)
              }
              onChange={(e) =>
                void save({
                  speechModel: speechModelSchema.parse(e.target.value),
                })
              }
            >
              {activeMissing && (
                <option value={settings.speechModel} disabled>
                  {activeName} (not installed)
                </option>
              )}
              {installed.map((model) => (
                <option key={model.id} value={model.id}>
                  {modelLabel(model)}
                </option>
              ))}
            </Select>
          )}
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Audio input"
        icon={<Mic size={14} aria-hidden="true" />}
      >
        <Row
          id="microphone"
          label="Microphone"
          help="Choose where dictate listens."
        >
          <Select
            id="microphone"
            aria-describedby="microphone-help"
            value={settings.microphone ?? ""}
            disabled={
              busy ||
              testing ||
              recording ||
              devices.isPending ||
              devices.isError
            }
            onChange={(e) => void save({ microphone: e.target.value || null })}
            onFocus={() => void devices.refetch()}
          >
            <option value="">System default</option>
            {settings.microphone &&
              !devices.data?.includes(settings.microphone) && (
                <option value={settings.microphone}>
                  {settings.microphone} (unavailable)
                </option>
              )}
            {[...new Set(devices.data ?? [])].map((name) => (
              <option key={name}>{name}</option>
            ))}
          </Select>
        </Row>
        <MicrophoneTest
          testing={testing}
          disabled={busy || recording}
          onToggle={testMicrophone}
          onError={setError}
          onTestingChange={setTesting}
        />
        {devices.isError && (
          <p className="error" role="alert">
            {String(devices.error)}{" "}
            <Button onClick={() => void devices.refetch()}>
              Refresh microphones
            </Button>
          </p>
        )}
      </SettingsGroup>
      <SettingsGroup
        title="Startup"
        icon={<Power size={14} aria-hidden="true" />}
      >
        <Row
          id="login"
          label="Launch at login"
          help="Keep dictate ready in your menu bar."
        >
          <Toggle
            id="login"
            checked={settings.launchAtLogin}
            disabled={busy || recording}
            aria-describedby="login-help"
            onChange={(e) => void save({ launchAtLogin: e.target.checked })}
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Appearance"
        icon={<SunMoon size={14} aria-hidden="true" />}
      >
        <Row
          id="theme"
          label="Theme"
          help="A little warmth, with or without colour."
        >
          <Select
            id="theme"
            aria-describedby="theme-help"
            value={settings.theme}
            disabled={busy}
            onChange={(e) =>
              void save({
                theme: settingsSchema.shape.theme.parse(e.target.value),
              })
            }
          >
            <option value="ember">Ember</option>
            <option value="paper">Paper</option>
          </Select>
        </Row>
        <Row
          id="appearance-mode"
          label="Mode"
          help="Follow your Mac, or choose a look."
        >
          <Select
            id="appearance-mode"
            aria-describedby="appearance-mode-help"
            value={settings.appearanceMode}
            disabled={busy}
            onChange={(e) =>
              void save({
                appearanceMode: settingsSchema.shape.appearanceMode.parse(
                  e.target.value,
                ),
              })
            }
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </Select>
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
