import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import SettingsWindow from "./SettingsWindow";
import { readSettings, shortcutLabel } from "./settings";
import "./App.css";

const client = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function Overlay() {
  const [status, setStatus] = useState("Ready");
  const [shortcut, setShortcut] = useState("");
  useEffect(() => {
    invoke<string>("pipeline_status")
      .then(setStatus)
      .catch((e) => setStatus(String(e)));
    readSettings()
      .then((settings) => setShortcut(shortcutLabel(settings.shortcut)))
      .catch((e) => setStatus(String(e)));
  }, []);
  return (
    <main className="overlay">
      <div className="dot" />
      <div className="body">
        <p className="label">Listening</p>
        <p className="status">{status}</p>
      </div>
      <kbd>{shortcut}</kbd>
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
