import { useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useQuery } from "@tanstack/react-query";
import General from "./GeneralSettings";
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
const pages = [
  { name: "General", icon: SlidersHorizontal },
  { name: "Models", icon: Cpu },
  { name: "History", icon: History },
  { name: "Output", icon: TextCursorInput },
  { name: "Advanced", icon: Settings2 },
  { name: "About", icon: Info },
];

export default function SettingsWindow() {
  const [page, setPage] = useState("General");
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  return (
    <div className="settings-shell">
      <aside>
        <div className="brand">
          <AudioLines size={22} strokeWidth={2} />
          <span>dictate</span>
        </div>
        <nav aria-label="Settings">
          {pages.map(({ name, icon: Icon }) => (
            <button
              key={name}
              aria-current={page === name ? "page" : undefined}
              className={page === name ? "selected" : ""}
              onClick={() => setPage(name)}
            >
              <Icon size={17} strokeWidth={1.7} />
              {name}
            </button>
          ))}
        </nav>
        <footer>
          <span>Version {version.data ?? "…"}</span>
          <button disabled title="Updates are coming in a later release">
            Check for updates
          </button>
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
        {page === "General" ? (
          <General />
        ) : (
          <div className="empty">
            <div className="empty-icon">
              <AudioLines size={26} />
            </div>
            <h2>Coming soon</h2>
            <p>{page} will be available in a future release.</p>
            <button onClick={() => setPage("General")}>
              <ArrowLeft size={14} /> Back to General
            </button>
          </div>
        )}
      </main>
    </div>
  );
}
