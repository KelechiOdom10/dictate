import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// The overlay only exists on screen while the chord is held. Its whole job
// right now is to prove whether it disappears when you let go.
export default function App() {
  const [status, setStatus] = useState("connecting…");

  useEffect(() => {
    invoke<string>("pipeline_status")
      .then(setStatus)
      .catch((error) => setStatus(String(error)));
  }, []);

  return (
    <main className="overlay">
      <div className="dot" />
      <div className="body">
        <p className="label">listening</p>
        <p className="status">{status}</p>
      </div>
      <kbd>⌥⇧Space</kbd>
    </main>
  );
}
