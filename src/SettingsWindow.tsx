import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useQuery } from "@tanstack/react-query";
import General from "./GeneralSettings";
import { Button, EmptyState, SidebarNav, Toast } from "./ui";
import {
  AudioLines,
  SlidersHorizontal,
  Cpu,
  History,
  TextCursorInput,
  Settings2,
  Info,
  ArrowLeft,
} from "lucide-react";
import { useDictationStatus } from "./useDictationStatus";

const pages = [
  { name: "General", icon: SlidersHorizontal },
  { name: "Models", icon: Cpu },
  { name: "History", icon: History },
  { name: "Output", icon: TextCursorInput },
  { name: "Advanced", icon: Settings2 },
  { name: "About", icon: Info },
];

export default function SettingsWindow() {
  const status = useDictationStatus();
  const [page, setPage] = useState("General");
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  return (
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
            icon: <Icon size={17} strokeWidth={1.7} aria-hidden="true" />,
          }))}
          value={page}
          onChange={setPage}
        />
        <footer>
          <span>Version {version.data ?? "…"}</span>
          <Button disabled title="Updates are coming in a later release">
            Check for updates
          </Button>
        </footer>
      </aside>
      <main className="settings-content">
        <header>
          <h1>{page}</h1>
          <p>
            {page === "General"
              ? "Make dictate feel like second nature."
              : `Your ${page.toLowerCase()} preferences, all in one place.`}
          </p>
        </header>
        {status.error && (
          <Toast
            tone="danger"
            onDismiss={() => {
              void invoke("dismiss_error").catch(console.error);
            }}
          >
            {status.error}
          </Toast>
        )}
        {page === "General" ? (
          <General />
        ) : (
          <EmptyState
            title="Coming soon"
            action={
              <Button onClick={() => setPage("General")}>
                <ArrowLeft size={14} aria-hidden="true" /> Back to General
              </Button>
            }
          >
            {page} will be available in a future release.
          </EmptyState>
        )}
      </main>
    </div>
  );
}
