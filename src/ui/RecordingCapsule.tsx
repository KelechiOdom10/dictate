import type { ReactNode } from "react";
import {
  Check,
  CircleAlert,
  ClipboardCheck,
  LoaderCircle,
  Mic,
  MicOff,
  Square,
  X,
} from "lucide-react";

export function LevelMeter({
  level,
  label = "Microphone input level",
}: {
  level: number;
  label?: string;
}) {
  const value = Math.max(0, Math.min(1, level));
  return (
    <div className={`ui-level ${value === 0 ? "ui-level-idle" : ""}`}>
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
function clock(seconds: number) {
  const pad = (value: number) => Math.floor(value).toString().padStart(2, "0");
  return `${pad(seconds / 60)}:${pad(seconds % 60)}`;
}
export type RecordingState =
  | { kind: "recording"; seconds: number; level?: number }
  | { kind: "annotating"; seconds: number }
  | { kind: "packaging" }
  | { kind: "transcribing" }
  | { kind: "done" }
  | { kind: "copied" }
  | { kind: "skipped" }
  | { kind: "error"; title: string; fix: string }
  | { kind: "ready"; message: string };
export function RecordingCapsule({
  state,
  shortcut,
  notice,
  onFinish,
  onCancel,
  onOpenSettings,
}: {
  state: RecordingState;
  shortcut?: ReactNode;
  notice?: string;
  onFinish?: () => void;
  onCancel?: () => void;
  onOpenSettings?: () => void;
}) {
  const labels: Record<Exclude<RecordingState["kind"], "error">, string> = {
    ready: "Listening",
    recording: "Recording",
    transcribing: "Transcribing…",
    done: "Text inserted",
    copied: "Copied — paste it anywhere",
    skipped: "No speech captured",
    annotating: "Annotating",
    packaging: "Packaging…",
  };
  const icons: Record<RecordingState["kind"], ReactNode> = {
    ready: <Mic size={18} />,
    recording: <span className="ui-record-dot" />,
    transcribing: <LoaderCircle size={18} className="ui-spin" />,
    done: <Check size={18} />,
    copied: <ClipboardCheck size={18} />,
    skipped: <MicOff size={18} />,
    annotating: <span className="ui-record-dot" />,
    packaging: <LoaderCircle size={18} className="ui-spin" />,
    error: <CircleAlert size={18} />,
  };
  return (
    <div className={`ui-capsule ui-capsule-${state.kind}`}>
      <span aria-hidden="true">{icons[state.kind]}</span>
      <div
        className="ui-capsule-copy"
        role={state.kind === "error" ? "alert" : "status"}
      >
        <span>{state.kind === "error" ? state.title : labels[state.kind]}</span>

        {state.kind === "skipped" && (
          <span className="ui-capsule-message">
            Hold the shortcut and speak a little longer.
          </span>
        )}
        {notice && <span className="ui-capsule-message">{notice}</span>}
        {state.kind === "ready" && (
          <span className="ui-capsule-message">{state.message}</span>
        )}
        {state.kind === "error" && !onOpenSettings && (
          <span className="ui-capsule-message">{state.fix}</span>
        )}
      </div>
      {(state.kind === "recording" || state.kind === "annotating") && (
        <span className="ui-timer">{clock(state.seconds)}</span>
      )}
      {state.kind === "recording" && state.level !== undefined && (
        <LevelMeter level={state.level} />
      )}
      {state.kind === "recording" && (
        <>
          <button
            className="capsule-action"
            aria-label="Finish dictation"
            onClick={onFinish}
          >
            <Square size={14} />
          </button>
          <button
            className="capsule-action"
            aria-label="Discard dictation"
            onClick={onCancel}
          >
            <X size={16} />
          </button>
        </>
      )}
      {state.kind === "error" && onOpenSettings && (
        <button
          className="capsule-action"
          aria-label="Open relevant settings"
          onClick={onOpenSettings}
        >
          Settings
        </button>
      )}
      {shortcut}
    </div>
  );
}
