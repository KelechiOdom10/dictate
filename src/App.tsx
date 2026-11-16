import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import SettingsWindow from "./SettingsWindow";
import { useDictationStatus } from "./useDictationStatus";
import { readSettings, settingsSchema } from "./settings";
import { RecordingCapsule, KeyboardShortcut } from "./ui";
import "./App.css";

const client = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function Overlay() {
  const status = useDictationStatus();
  const [shortcut, setShortcut] = useState("");
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (status.phase !== "recording") return;
    const started = Date.now();
    const timer = window.setInterval(
      () => setSeconds((Date.now() - started) / 1000),
      250,
    );
    return () => window.clearInterval(timer);
  }, [status.phase]);
  useEffect(() => {
    let disposed = false;
    const listener = listen("settings-changed", ({ payload }) => {
      const result = settingsSchema.safeParse(payload);
      if (!disposed) {
        if (result.success) {
          setShortcut(result.data.shortcut);
          setSettingsError(null);
        } else
          setSettingsError(
            "Could not read the updated shortcut. Open Settings for details.",
          );
      }
    });
    void listener
      .then(async () => {
        try {
          const settings = await readSettings();
          if (!disposed) setShortcut(settings.shortcut);
        } catch (error) {
          if (!disposed) setSettingsError(String(error));
        }
      })
      .catch((error) => {
        if (!disposed) setSettingsError(String(error));
      });
    return () => {
      disposed = true;
      void listener.then((stop) => stop()).catch(() => {});
    };
  }, []);
  const error = status.error ?? settingsError;
  return (
    <main className="overlay">
      <RecordingCapsule
        state={
          error
            ? {
                kind: "error",
                message: error.length > 110 ? `${error.slice(0, 107)}…` : error,
              }
            : status.phase === "recording"
              ? { kind: "recording", seconds }
              : status.phase === "transcribing"
                ? { kind: "transcribing" }
                : { kind: "ready", message: "Ready" }
        }
        notice={
          status.phase === "idle" ? undefined : (status.notice ?? undefined)
        }
        shortcut={<KeyboardShortcut value={shortcut} />}
      />
    </main>
  );
}

export default function App() {
  return new URLSearchParams(location.search).get("window") === "settings" ? (
    <QueryClientProvider client={client}>
      <SettingsWindow />
    </QueryClientProvider>
  ) : (
    <Overlay />
  );
}
