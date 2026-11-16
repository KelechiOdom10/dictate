import type { ReactNode } from "react";
import { Check, CircleAlert, LoaderCircle, Mic } from "lucide-react";

export function LevelMeter({
  level,
  label = "Microphone input level",
}: {
  level: number;
  label?: string;
}) {
  const value = Math.max(0, Math.min(1, level));
  return (
    <div className="ui-level">
      <meter min={0} max={1} value={value} aria-label={label} />
      <div className="ui-level-bars" aria-hidden="true">
        {Array.from({ length: 12 }, (_, index) => (
          <span
            key={index}
            style={{
              opacity: value * 12 > index ? 1 : 0.2,
              transform: `scaleY(${0.3 + Math.sin((index + 1) * 1.7) ** 2 * value * 0.7})`,
            }}
          />
        ))}
      </div>
    </div>
  );
}
export type RecordingState =
  | { kind: "recording"; seconds: number; level?: number }
  | { kind: "transcribing" }
  | { kind: "done" }
  | { kind: "error"; message: string }
  | { kind: "ready"; message: string };
export function RecordingCapsule({
  state,
  shortcut,
}: {
  state: RecordingState;
  shortcut?: ReactNode;
}) {
  const labels: Record<RecordingState["kind"], string> = {
    ready: "Listening",
    recording: "Recording",
    transcribing: "Transcribing…",
    done: "Text inserted",
    error: "Something went wrong",
  };
  const icons: Record<RecordingState["kind"], ReactNode> = {
    ready: <Mic size={18} />,
    recording: <span className="ui-record-dot" />,
    transcribing: <LoaderCircle size={18} className="ui-spin" />,
    done: <Check size={18} />,
    error: <CircleAlert size={18} />,
  };
  return (
    <div className={`ui-capsule ui-capsule-${state.kind}`}>
      <span aria-hidden="true">{icons[state.kind]}</span>
      <div
        className="ui-capsule-copy"
        role={state.kind === "error" ? "alert" : "status"}
      >
        <span>{labels[state.kind]}</span>
        {(state.kind === "ready" || state.kind === "error") && (
          <span className="ui-capsule-message">{state.message}</span>
        )}
      </div>
      {state.kind === "recording" && (
        <>
          <span className="ui-timer">
            {Math.floor(state.seconds / 60)
              .toString()
              .padStart(2, "0")}
            :
            {Math.floor(state.seconds % 60)
              .toString()
              .padStart(2, "0")}
          </span>
          {state.level !== undefined && <LevelMeter level={state.level} />}
        </>
      )}
      {shortcut}
    </div>
  );
}
