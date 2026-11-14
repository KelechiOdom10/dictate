import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { Button, LevelMeter } from "./ui";

type Props = {
  testing: boolean;
  disabled: boolean;
  onToggle: () => Promise<void>;
  onError: (error: string) => void;
  onTestingChange: (active: boolean) => void;
};

export default function MicrophoneTest({
  testing,
  disabled,
  onToggle,
  onError,
  onTestingChange,
}: Props) {
  const [level, setLevel] = useState(0);
  useEffect(() => {
    if (!testing) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const value = z
          .number()
          .finite()
          .parse(await invoke("microphone_level"));
        if (!cancelled) {
          setLevel(Math.min(1, value * 5));
          timer = setTimeout(poll, 80);
        }
      } catch (e) {
        if (!cancelled) {
          onError(String(e));
          onTestingChange(false);
          invoke("microphone_test", { enabled: false }).catch(console.error);
        }
      }
    };
    void poll();
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [testing, onError, onTestingChange]);

  return (
    <div className="mic-test">
      <div className="meter-copy">
        <LevelMeter level={testing ? level : 0} />
        <span>
          {testing
            ? "Listening · audio is not saved"
            : "Check your input level"}
        </span>
      </div>
      <Button
        className="text-button"
        onClick={() => void onToggle()}
        disabled={disabled}
      >
        {testing ? "Stop test" : "Test microphone"}
      </Button>
    </div>
  );
}
