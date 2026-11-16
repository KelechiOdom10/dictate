import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
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

function Overlay({ appearanceError }: { appearanceError: string | null }) {
  const status = useDictationStatus();
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const [seconds, setSeconds] = useState(0);
  const [level, setLevel] = useState(0);
  const capsule = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = document.documentElement;
    root.classList.add("overlay-window");
    const node = capsule.current;
    if (!node) return;
    const observer = new ResizeObserver(() => {
      const measurement = node.cloneNode(true);
      if (!(measurement instanceof HTMLElement)) return;
      Object.assign(measurement.style, {
        position: "fixed",
        visibility: "hidden",
        width: "max-content",
        maxWidth: "420px",
      });
      measurement.setAttribute("aria-hidden", "true");
      document.body.append(measurement);
      const { width, height } = measurement.getBoundingClientRect();
      measurement.remove();
      void invoke("resize_overlay", {
        width: Math.ceil(width + 16),
        height: Math.ceil(height + 16),
      }).catch(console.error);
    });
    observer.observe(node);
    return () => {
      observer.disconnect();
      root.classList.remove("overlay-window");
    };
  }, []);
  useEffect(() => {
    let disposed = false;
    const subscription = listen("recording-level", ({ payload }) => {
      const result = z.number().finite().min(0).max(1).safeParse(payload);
      if (!disposed && result.success) setLevel(result.data);
    });
    return () => {
      disposed = true;
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, []);
  useEffect(() => {
    if (status.phase !== "recording") return;
    setSeconds(0);
    setLevel(0);
    const started = Date.now();
    const timer = window.setInterval(
      () => setSeconds((Date.now() - started) / 1000),
      250,
    );
    return () => window.clearInterval(timer);
  }, [status.phase]);
  const error =
    appearanceError ??
    status.error ??
    (settings.error ? String(settings.error) : null);
  return (
    <main className="overlay">
      <div ref={capsule} className="overlay-capsule">
        <RecordingCapsule
          state={
            error
              ? {
                  kind: "error",
                  message:
                    error.length <= 90
                      ? `${error} Open Settings to retry.`
                      : "Open dictate Settings for details, then retry.",
                }
              : status.phase === "recording"
                ? {
                    kind: "recording",
                    seconds,
                    level: Math.min(1, Math.sqrt(level) * 2),
                  }
                : status.phase === "transcribing"
                  ? { kind: "transcribing" }
                  : status.phase === "done"
                    ? { kind: "done" }
                    : status.phase === "skipped"
                      ? { kind: "skipped" }
                      : { kind: "ready", message: "Ready" }
          }
          notice={
            status.phase === "recording" || status.phase === "transcribing"
              ? (status.notice ?? undefined)
              : undefined
          }
          shortcut={
            status.phase === "idle" && !error ? (
              <KeyboardShortcut value={settings.data?.shortcut ?? ""} />
            ) : undefined
          }
        />
      </div>
    </main>
  );
}

function Appearance({
  onError,
}: {
  onError: (message: string | null) => void;
}) {
  const query = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  useEffect(() => {
    let disposed = false;
    const subscription = listen("settings-changed", ({ payload }) => {
      const result = settingsSchema.safeParse(payload);
      if (disposed) return;
      if (result.success) {
        client.setQueryData(["settings"], result.data);
        onError(null);
      } else {
        onError("Settings could not refresh. Open Settings and save again.");
        void invoke("report_settings_refresh_error").catch(console.error);
      }
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
  }, [onError]);
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
  const [appearanceError, setAppearanceError] = useState<string | null>(null);
  return (
    <QueryClientProvider client={client}>
      <Appearance onError={setAppearanceError} />
      {new URLSearchParams(location.search).get("window") === "settings" ? (
        <SettingsWindow />
      ) : (
        <Overlay appearanceError={appearanceError} />
      )}
    </QueryClientProvider>
  );
}
