import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { flushSync } from "react-dom";
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useQuery } from "@tanstack/react-query";
import General from "./GeneralSettings";
import HistoryPage from "./HistoryPage";
import AnnotatePage from "./AnnotatePage";
import ModelsPage from "./ModelsPage";
import OutputPage from "./OutputPage";
import AdvancedPage from "./AdvancedPage";
import AboutPage from "./AboutPage";
import { Button, SidebarNav, Toast } from "./ui";
import {
  AudioLines,
  SlidersHorizontal,
  Cpu,
  History,
  TextCursorInput,
  Settings2,
  Info,
  PenLine,
} from "lucide-react";
import { useDictationStatus } from "./useDictationStatus";
import { readSettings } from "./settings";
import { SettingsTheme } from "./ui/SettingsTheme";

const subtitles: Record<string, string> = {
  General: "Make dictate feel like second nature.",
  Models:
    "Choose how dictate turns speech into text. Everything runs on this Mac.",
  Annotate:
    "Record your screen, draw on it and talk. Stop to copy it for your agent.",
  History: "Everything you’ve dictated and annotated, kept on this Mac.",
  Output: "Decide what happens to your words once you stop speaking.",
  Advanced: "Fine-tune the microphone, overlay and troubleshooting.",
  About: "Version, source code and where dictate keeps its files.",
};

const pages = [
  { name: "General", icon: SlidersHorizontal },
  { name: "Models", icon: Cpu },
  { name: "Annotate", icon: PenLine },
  { name: "History", icon: History },
  { name: "Output", icon: TextCursorInput },
  { name: "Advanced", icon: Settings2 },
  { name: "About", icon: Info },
];

export default function SettingsWindow({
  initialPage = "General",
}: {
  initialPage?: string;
}) {
  const status = useDictationStatus();
  const [page, setPage] = useState(initialPage);
  const [navigationReady, setNavigationReady] = useState(false);
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  useEffect(() => {
    let disposed = false;
    const subscription = listen("settings-page", ({ payload }) => {
      if (
        typeof payload === "string" &&
        pages.some(({ name }) => name === payload)
      )
        setPage(payload);
    });
    void subscription
      .then(() => {
        if (!disposed) setNavigationReady(true);
      })
      .catch(console.error);
    return () => {
      disposed = true;
      void subscription.then((stop) => stop()).catch(console.error);
    };
  }, []);
  useEffect(() => {
    if (!navigationReady || !settings.isFetched) return;
    void invoke("settings_ready").catch(console.error);
  }, [navigationReady, settings.isFetched]);
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  return (
    <SettingsTheme
      palette={settings.data?.theme}
      mode={settings.data?.appearanceMode}
    >
      <div className="settings-shell">
        <aside>
          <div className="brand">
            <AudioLines size={22} strokeWidth={2} />
            <span>dictate</span>
          </div>
          <SidebarNav
            label="Settings"
            items={pages.map(({ name, icon: Icon }) => ({
              value: name,
              label: name,
              icon: <Icon size={17} strokeWidth={1.5} aria-hidden="true" />,
            }))}
            value={page}
            onChange={(next) => {
              if (
                document.startViewTransition &&
                !matchMedia("(prefers-reduced-motion: reduce)").matches
              ) {
                document.startViewTransition(() =>
                  flushSync(() => setPage(next)),
                );
              } else setPage(next);
            }}
          />
          <footer>
            <span>Version {version.data ?? "…"}</span>
            <Button
              size="sm"
              variant="ghost"
              disabled
              title="Updates are coming in a later release"
            >
              Check for updates
            </Button>
          </footer>
        </aside>
        <main key={page} className="settings-content">
          <header>
            <h1>{page}</h1>
            <p>{subtitles[page]}</p>
          </header>
          {status.error && (
            <Toast
              tone="danger"
              onDismiss={() => {
                void invoke("dismiss_error").catch(console.error);
              }}
            >
              {status.error.title}. {status.error.fix}
            </Toast>
          )}
          {page === "General" ? (
            <General onOpenModels={() => setPage("Models")} />
          ) : page === "Models" ? (
            <ModelsPage />
          ) : page === "Annotate" ? (
            <AnnotatePage />
          ) : page === "History" ? (
            <HistoryPage />
          ) : page === "Output" ? (
            <OutputPage />
          ) : page === "Advanced" ? (
            <AdvancedPage />
          ) : (
            <AboutPage />
          )}
        </main>
      </div>
    </SettingsTheme>
  );
}
