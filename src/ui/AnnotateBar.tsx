import { PenLine, Square } from "lucide-react";
import { IconButton } from "./Button";

function clock(seconds: number) {
  const pad = (value: number) => Math.floor(value).toString().padStart(2, "0");
  return `${pad(seconds / 60)}:${pad(seconds % 60)}`;
}

export function AnnotateBar({
  seconds,
  pen,
  onPen,
  onStop,
}: {
  seconds: number;
  pen: boolean;
  onPen: () => void;
  onStop: () => void;
}) {
  return (
    <div className="ui-annotate-bar" role="toolbar" aria-label="Annotate">
      <span className="ui-record-dot" aria-hidden="true" />
      <span className="ui-timer" role="timer" aria-label="Recording time">
        {clock(seconds)}
      </span>
      <IconButton
        label={pen ? "Stop drawing (Esc)" : "Draw with the mouse"}
        aria-pressed={pen}
        variant="ghost"
        size="sm"
        onClick={onPen}
      >
        <PenLine />
      </IconButton>
      <IconButton
        label="Stop recording"
        variant="ghost"
        size="sm"
        onClick={onStop}
      >
        <Square />
      </IconButton>
    </div>
  );
}
