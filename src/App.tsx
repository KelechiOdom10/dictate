import { useEffect, useState, useRef, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { listen } from "@tauri-apps/api/event";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
} from "@tanstack/react-query";
import SettingsWindow from "./SettingsWindow";
import { AnnotateBarWindow, InkWindow } from "./AnnotateWindows";
import { useDictationStatus } from "./useDictationStatus";
import { readSettings, settingsSchema } from "./settings";
import { RecordingCapsule, KeyboardShortcut } from "./ui";
import { problem, type Problem } from "./problem";
import "./App.css";

const OVERLAY_MAX_WIDTH = 420;
// A notice describes the take or session in progress, so it must not carry
// over to the done, copied or skipped pill that follows.
const NOTICE_PHASES = new Set([
  "recording",
  "transcribing",
  "annotating",
  "packaging",
]);

const client = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function Overlay({ appearanceError }: { appearanceError: Problem | null }) {
  const status = useDictationStatus();
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const [hiding, setHiding] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const [level, setLevel] = useState(0);
  const capsule = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const subscriptions = [
      listen("overlay-hiding", () => setHiding(true)),
      listen("overlay-showing", () => setHiding(false)),
    ];
    return () => {
      subscriptions.forEach(
        (subscription) =>
          void subscription.then((stop) => stop()).catch(console.error),
      );
    };
  }, []);
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
        maxWidth: `${OVERLAY_MAX_WIDTH}px`,
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
    if (status.phase !== "recording" && status.phase !== "annotating") return;
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
    (settings.error
      ? problem(
          "Settings couldn’t load",
          "Open Settings to try again.",
          settings.error,
        )
      : null);
  return (
    <main className="overlay">
      <div
        ref={capsule}
        className={`overlay-capsule ${hiding ? "overlay-hiding" : ""}`}
        style={
          { "--overlay-max-width": `${OVERLAY_MAX_WIDTH}px` } as CSSProperties
        }
      >
        <RecordingCapsule
          onFinish={() => void invoke("dictation_stop").catch(console.error)}
          onCancel={() => void invoke("dictation_cancel").catch(console.error)}
          onOpenSettings={() =>
            void invoke("open_notice_settings").catch(console.error)
          }
          state={
            error
              ? { kind: "error", title: error.title, fix: error.fix }
              : status.phase === "recording"
                ? {
                    kind: "recording",
                    seconds,
                    level: Math.min(1, Math.sqrt(level) * 2),
                  }
                : status.phase === "annotating"
                  ? { kind: "annotating", seconds }
                  : status.phase === "packaging"
                    ? { kind: "packaging" }
                    : status.phase === "transcribing"
                      ? { kind: "transcribing" }
                      : status.phase === "done"
                        ? { kind: "done" }
                        : status.phase === "copied"
                          ? { kind: "copied" }
                          : status.phase === "skipped"
                            ? { kind: "skipped" }
                            : { kind: "ready", message: "Ready" }
          }
          notice={
            error || !NOTICE_PHASES.has(status.phase)
              ? undefined
              : (status.notice ?? undefined)
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
  canReportRefreshError,
}: {
  onError: (problem: Problem | null) => void;
  canReportRefreshError: boolean;
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
        onError(
          problem(
            "Settings couldn’t refresh",
            "Open Settings and save again.",
            result.error,
          ),
        );
        if (canReportRefreshError)
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
  }, [onError, canReportRefreshError]);
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
  }, [query.data?.theme, query.data?.appearanceMode, query.isFetched]);
  return null;
}

const windowKind = new URLSearchParams(location.search).get("window");
const isAnnotatePanel = windowKind === "ink" || windowKind === "annotate-bar";

export default function App() {
  const [appearanceError, setAppearanceError] = useState<Problem | null>(null);
  return (
    <QueryClientProvider client={client}>
      <Appearance
        onError={setAppearanceError}
        canReportRefreshError={!isAnnotatePanel}
      />
      {windowKind === "settings" ? (
        <SettingsWindow />
      ) : windowKind === "ink" ? (
        <InkWindow />
      ) : windowKind === "annotate-bar" ? (
        <AnnotateBarWindow />
      ) : (
        <Overlay appearanceError={appearanceError} />
      )}
    </QueryClientProvider>
  );
}
