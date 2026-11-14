import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Keyboard, Mic, Power } from "lucide-react";
import { z } from "zod";
import {
  readDevices,
  readSettings,
  writeSettings,
  type Settings,
} from "./settings";
import Row from "./SettingRow";
import ShortcutRecorder from "./ShortcutRecorder";
import MicrophoneTest from "./MicrophoneTest";

export default function General() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
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
        <button onClick={() => void query.refetch()}>Try again</button>
      </div>
    );
  const settings = query.data;
  const busy = mutation.isPending;
  return (
    <>
      <section aria-labelledby="dictation-heading">
        <h2 id="dictation-heading">
          <Keyboard size={14} /> Dictation
        </h2>
        <div className="group">
          <ShortcutRecorder
            value={settings.shortcut}
            recording={recording}
            disabled={busy}
            onRecordingChange={setRecording}
            onSave={(shortcut) => save({ shortcut })}
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
            <select
              id="mode"
              aria-describedby="mode-help"
              value={settings.shortcutMode}
              disabled={busy || recording}
              onChange={(e) =>
                void save({
                  shortcutMode: z
                    .enum(["hold", "toggle"])
                    .parse(e.target.value),
                })
              }
            >
              <option value="hold">Hold to talk</option>
              <option value="toggle">Toggle</option>
            </select>
          </Row>
        </div>
      </section>
      <section aria-labelledby="audio-heading">
        <h2 id="audio-heading">
          <Mic size={14} /> Audio input
        </h2>
        <div className="group">
          <Row
            id="microphone"
            label="Microphone"
            help="Choose where dictate listens."
          >
            <select
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
              onChange={(e) =>
                void save({ microphone: e.target.value || null })
              }
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
            </select>
          </Row>
          <MicrophoneTest
            testing={testing}
            disabled={busy || recording}
            onToggle={testMicrophone}
            onError={setError}
            onTestingChange={setTesting}
          />
        </div>
        {devices.isError && (
          <p className="error" role="alert">
            {String(devices.error)}{" "}
            <button onClick={() => void devices.refetch()}>
              Refresh microphones
            </button>
          </p>
        )}
      </section>
      <section aria-labelledby="startup-heading">
        <h2 id="startup-heading">
          <Power size={14} /> Startup
        </h2>
        <div className="group">
          <Row
            id="login"
            label="Launch at login"
            help="Keep dictate ready in your menu bar."
          >
            <input
              id="login"
              className="switch"
              type="checkbox"
              role="switch"
              checked={settings.launchAtLogin}
              disabled={busy || recording}
              aria-describedby="login-help"
              onChange={(e) => void save({ launchAtLogin: e.target.checked })}
            />
          </Row>
        </div>
      </section>
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
