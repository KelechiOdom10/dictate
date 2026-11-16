import { Check, ImageDown, PenLine, Square, X } from "lucide-react";
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

/**
 * The bar after a session: the summary is already on the clipboard. "Copy
 * image" swaps it for the contact sheet, for chats that cannot open a path.
 */
export function AnnotateCopiedBar({
  hasImage,
  imageCopied,
  onCopyImage,
  onClose,
}: {
  hasImage: boolean;
  imageCopied: boolean;
  onCopyImage: () => void;
  onClose: () => void;
}) {
  return (
    <div
      className="ui-annotate-bar ui-annotate-bar-copied"
      role="toolbar"
      aria-label="Annotation"
    >
      <Check size={16} aria-hidden="true" />
      <span role="status" className="ui-annotate-bar-label">
        {imageCopied ? "Image copied" : "Copied"}
      </span>
      {hasImage && (
        <IconButton
          label={imageCopied ? "Image copied" : "Copy image"}
          variant="ghost"
          size="sm"
          onClick={onCopyImage}
        >
          {imageCopied ? <Check /> : <ImageDown />}
        </IconButton>
      )}
      <IconButton label="Close" variant="ghost" size="sm" onClick={onClose}>
        <X />
      </IconButton>
    </div>
  );
}
