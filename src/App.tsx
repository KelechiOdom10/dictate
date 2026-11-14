import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import SettingsWindow from "./SettingsWindow";
import { readSettings } from "./settings";
import { RecordingCapsule, KeyboardShortcut } from "./ui";
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
      .then((settings) => setShortcut(settings.shortcut))
      .catch((e) => setStatus(String(e)));
  }, []);
  return (
    <main className="overlay">
      <RecordingCapsule
        state={{ kind: "ready", message: status }}
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
