import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from "@tanstack/react-query";
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
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
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
  const error =
    status.error ?? (settings.error ? String(settings.error) : null);
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
        shortcut={<KeyboardShortcut value={settings.data?.shortcut ?? ""} />}
      />
    </main>
  );
}

function Appearance() {
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  useEffect(() => {
    let disposed = false;
    const subscription = listen("settings-changed", ({ payload }) => {
      const result = settingsSchema.safeParse(payload);
      if (result.success && !disposed)
        client.setQueryData(["settings"], result.data);
      else if (!result.success) console.error(result.error);
    });
    void subscription
      .then(() => {
        if (!disposed)
          void client.invalidateQueries({ queryKey: ["settings"] });
      })
      .catch(console.error);
    return () => {
      disposed = true;
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, []);
  useEffect(() => {
    const system = matchMedia("(prefers-color-scheme: dark)");
    const root = document.documentElement;
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      root.classList.add("appearance-changing");
      document.documentElement.dataset.theme = query.data?.theme ?? "ember";
      const mode = query.data?.appearanceMode ?? "system";
      document.documentElement.dataset.mode =
        mode === "system" ? (system.matches ? "dark" : "light") : mode;
      root.getBoundingClientRect();
      frame = requestAnimationFrame(() =>
        root.classList.remove("appearance-changing"),
      );
    };
    update();
    system.addEventListener("change", update);
    return () => {
      system.removeEventListener("change", update);
      cancelAnimationFrame(frame);
      root.classList.remove("appearance-changing");
    };
  }, [query.data?.theme, query.data?.appearanceMode]);
  return null;
}

export default function App() {
  return (
    <QueryClientProvider client={client}>
      <Appearance />
      {new URLSearchParams(location.search).get("window") === "settings" ? (
        <SettingsWindow />
      ) : (
        <Overlay />
      )}
    </QueryClientProvider>
  );
}
