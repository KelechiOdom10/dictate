import { Check, ImageDown, PenLine, Square, X } from "lucide-react";
import type { ReactNode } from "react";

function BarButton({
  label,
  pressed,
  onClick,
  children,
}: {
  label: string;
  pressed?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className="ui-bar-button"
      aria-label={label}
      aria-pressed={pressed}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

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
      <BarButton
        label={pen ? "Stop drawing (Esc)" : "Draw with the mouse"}
        pressed={pen}
        onClick={onPen}
      >
        <PenLine />
      </BarButton>
      <BarButton label="Stop recording" onClick={onStop}>
        <Square />
      </BarButton>
    </div>
  );
}

/**
 * The bar the moment the shortcut is pressed, while the microphone and the
 * screen capture start.
 */
export function AnnotateStartingBar() {
  return (
    <div className="ui-annotate-bar" role="toolbar" aria-label="Annotate">
      <span
        className="ui-record-dot ui-record-dot-starting"
        aria-hidden="true"
      />
      <span role="status" className="ui-annotate-bar-label">
        Starting…
      </span>
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
        <BarButton
          label={imageCopied ? "Image copied" : "Copy image"}
          onClick={onCopyImage}
        >
          {imageCopied ? <Check /> : <ImageDown />}
        </BarButton>
      )}
      <BarButton label="Close" onClick={onClose}>
        <X />
      </BarButton>
    </div>
  );
}
