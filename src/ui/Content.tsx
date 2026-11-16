import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import {
  Check,
  CircleAlert,
  Download,
  HardDrive,
  Info,
  Trash2,
} from "lucide-react";
import { Button } from "./Button";

export function Card({ children }: { children: ReactNode }) {
  return <div className="ui-card">{children}</div>;
}
export function ListItem({
  date,
  title,
  meta,
  preview,
  actions,
}: {
  date: string;
  title: string;
  meta?: string;
  preview?: string;
  actions?: ReactNode;
}) {
  return (
    <article className="ui-list-item">
      <ListItemBody date={date} title={title} meta={meta} preview={preview} />
      {actions && <div className="ui-actions">{actions}</div>}
    </article>
  );
}
function ListItemBody({
  date,
  title,
  meta,
  preview,
}: {
  date: string;
  title: string;
  meta?: string;
  preview?: string;
}) {
  return (
    <>
      <span className="ui-list-heading">
        <span className="ui-list-title">{title}</span>
        <span className="ui-date">{date}</span>
      </span>
      {meta && <span className="ui-list-meta">{meta}</span>}
      {preview && <span className="ui-preview">{preview}</span>}
    </>
  );
}
export type SelectListItem<Id> = {
  id: Id;
  title: string;
  date: string;
  meta?: string;
};
export function SelectList<Id extends string | number>({
  label,
  items,
  value,
  onChange,
}: {
  label: string;
  items: SelectListItem<Id>[];
  value: Id;
  onChange: (id: Id) => void;
}) {
  const buttons = useRef(new Map<Id, HTMLButtonElement>());
  function move(offset: number) {
    const index = items.findIndex((item) => item.id === value) + offset;
    const next = items[Math.min(Math.max(index, 0), items.length - 1)];
    if (!next) return;
    onChange(next.id);
    buttons.current.get(next.id)?.focus();
  }
  return (
    <div
      className="ui-select-list"
      role="group"
      aria-label={label}
      onKeyDown={(e) => {
        const offsets: Record<string, number> = {
          ArrowDown: 1,
          ArrowUp: -1,
          Home: -items.length,
          End: items.length,
        };
        if (!(e.key in offsets)) return;
        e.preventDefault();
        move(offsets[e.key]);
      }}
    >
      {items.map(({ id, ...item }) => (
        <button
          key={id}
          ref={(node) => {
            if (node) buttons.current.set(id, node);
            else buttons.current.delete(id);
          }}
          type="button"
          className="ui-list-item ui-list-item-button"
          aria-current={id === value ? "true" : undefined}
          tabIndex={id === value ? 0 : -1}
          onClick={() => onChange(id)}
        >
          <ListItemBody {...item} />
        </button>
      ))}
    </div>
  );
}
export function DetailPanel({
  title,
  subtitle,
  actions,
  metadata,
  children,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  metadata: { label: string; value: string }[];
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="ui-detail" aria-labelledby={id}>
      <div className="ui-detail-header">
        <div>
          <h2 id={id}>{title}</h2>
          {subtitle && <p className="ui-date">{subtitle}</p>}
        </div>
        {actions && <div className="ui-actions">{actions}</div>}
      </div>
      <div className="ui-detail-body">{children}</div>
      <dl>
        {metadata.map((row) => (
          <div key={row.label}>
            <dt>{row.label}</dt>
            <dd>{row.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
export function ProgressBar({
  label,
  value,
  detail,
}: {
  label: string;
  value?: number;
  /** Replaces the percentage, e.g. "120 of 466 MB". */
  detail?: string;
}) {
  return (
    <div className="ui-progress">
      <span>{label}</span>
      <progress aria-label={label} max={100} value={value} />
      {(detail !== undefined || value !== undefined) && (
        <span className="ui-date">
          {detail ?? `${Math.round(value ?? 0)}%`}
        </span>
      )}
    </div>
  );
}
export function EmptyState({
  title,
  children,
  action,
}: {
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="ui-empty">
      <h2>{title}</h2>
      <p>{children}</p>
      {action}
    </div>
  );
}
export function Toast({
  tone = "success",
  children,
  onDismiss,
}: {
  tone?: "success" | "warning" | "danger" | "info";
  children: ReactNode;
  onDismiss?: () => void;
}) {
  return (
    <div
      className={`ui-toast ui-toast-${tone}`}
      role={tone === "danger" ? "alert" : "status"}
    >
      <span aria-hidden="true">
        {tone === "success" ? (
          <Check size={16} />
        ) : tone === "info" ? (
          <Info size={16} />
        ) : (
          <CircleAlert size={16} />
        )}
      </span>
      <span>{children}</span>
      {onDismiss && (
        <Button variant="ghost" onClick={onDismiss}>
          Dismiss
        </Button>
      )}
    </div>
  );
}
export function Confirm({
  question,
  confirmLabel,
  children,
  onConfirm,
  onCancel,
}: {
  question: string;
  confirmLabel: string;
  children?: ReactNode;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => cancel.current?.focus(), []);
  return (
    <div
      className="ui-confirm"
      role="group"
      aria-label={question}
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <div>
        <p className="ui-confirm-question">{question}</p>
        {children && <p className="ui-preview">{children}</p>}
      </div>
      <div className="ui-actions">
        <Button ref={cancel} onClick={onCancel}>
          Cancel
        </Button>
        <Button variant="primary" onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </div>
  );
}
export type ModelRating = {
  label: string;
  /** 0–1, drawn as a meter. */
  value: number;
  /** What the value means in words, e.g. "Good" or "0.4 s". */
  text: string;
};
export type ModelStatus =
  | { kind: "available"; onDownload: () => void }
  | {
      kind: "downloading";
      progress: number;
      detail: string;
      cancelling: boolean;
      onCancel: () => void;
    }
  | { kind: "preparing" }
  | {
      kind: "installed";
      active: boolean;
      onUse: () => void;
      onDelete: () => void;
    }
  | { kind: "error"; message: string; onRetry: () => void };
export function ModelCard({
  name,
  description,
  ratings,
  size,
  badge,
  status,
  disabled = false,
}: {
  name: string;
  description: string;
  ratings: ModelRating[];
  size: string;
  badge?: string;
  status: ModelStatus;
  disabled?: boolean;
}) {
  const [confirming, setConfirming] = useState(false);
  const headingId = useId();
  const active = status.kind === "installed" && status.active;
  const card = useRef<HTMLElement>(null);
  const hadFocus = useRef(false);
  // Use, Cancel and Delete each remove the button that was pressed. Keep
  // keyboard focus in this card instead of letting it fall to the page.
  useEffect(() => {
    const focused = document.activeElement;
    if (
      !hadFocus.current ||
      (focused !== document.body && focused !== card.current)
    )
      return;
    const next =
      card.current?.querySelector<HTMLElement>(
        ".ui-model-actions button:not(:disabled)",
      ) ?? card.current;
    next?.focus();
  }, [status.kind, active, confirming]);
  const actions = (() => {
    switch (status.kind) {
      case "available":
        return (
          <Button
            disabled={disabled}
            aria-label={`Download ${name}`}
            onClick={status.onDownload}
          >
            <Download size={14} aria-hidden="true" /> Download
          </Button>
        );
      case "downloading":
        return (
          <>
            <ProgressBar
              label={status.cancelling ? "Cancelling" : "Downloading"}
              value={status.progress}
              detail={status.detail}
            />
            <Button
              size="sm"
              variant="ghost"
              disabled={status.cancelling}
              aria-label={`Cancel ${name} download`}
              onClick={status.onCancel}
            >
              Cancel
            </Button>
          </>
        );
      case "preparing":
        return (
          <span className="ui-date" role="status">
            Preparing your model…
          </span>
        );
      case "installed":
        return status.active ? (
          <span className="ui-date">Used for every dictation</span>
        ) : (
          <>
            <Button
              size="sm"
              variant="ghost"
              disabled={disabled}
              aria-label={`Delete ${name}`}
              onClick={() => setConfirming(true)}
            >
              <Trash2 size={14} aria-hidden="true" /> Delete
            </Button>
            <Button
              variant="primary"
              disabled={disabled}
              aria-label={`Use ${name}`}
              onClick={status.onUse}
            >
              Use
            </Button>
          </>
        );
      case "error":
        return (
          <>
            <p className="ui-model-error" role="alert">
              <CircleAlert size={14} aria-hidden="true" />
              {status.message}
            </p>
            <Button
              disabled={disabled}
              aria-label={`Retry ${name} download`}
              onClick={status.onRetry}
            >
              Retry
            </Button>
          </>
        );
    }
  })();
  return (
    <section
      ref={card}
      tabIndex={-1}
      onFocus={() => (hadFocus.current = true)}
      onBlur={(e) => {
        if (e.relatedTarget)
          hadFocus.current = card.current?.contains(e.relatedTarget) ?? false;
      }}
      className={`ui-card ui-model${active ? " ui-model-active" : ""}`}
      aria-labelledby={headingId}
      aria-current={active ? "true" : undefined}
    >
      <div className="ui-model-top">
        <div className="ui-model-about">
          <div className="ui-model-heading">
            <h2 id={headingId}>{name}</h2>
            {active && <span className="ui-badge">In use</span>}
            {badge && <span className="ui-badge ui-badge-quiet">{badge}</span>}
          </div>
          <p className="ui-preview">{description}</p>
        </div>
        <dl className="ui-model-ratings">
          {ratings.map((rating) => (
            <div key={rating.label} className="ui-model-rating">
              <dt>{rating.label}</dt>
              <dd>
                <meter
                  min={0}
                  max={1}
                  value={rating.value}
                  aria-label={`${name} ${rating.label.toLowerCase()}`}
                  aria-valuetext={rating.text}
                />
                <span className="ui-date">{rating.text}</span>
              </dd>
            </div>
          ))}
        </dl>
      </div>
      {confirming && status.kind === "installed" && !status.active ? (
        <Confirm
          question={`Delete ${name}?`}
          confirmLabel="Delete model"
          onConfirm={() => {
            setConfirming(false);
            status.onDelete();
          }}
          onCancel={() => setConfirming(false)}
        >
          Frees {size}. You can download it again later.
        </Confirm>
      ) : (
        <div className="ui-model-footer">
          <span className="ui-date ui-model-size">
            <HardDrive size={14} aria-hidden="true" /> {size}
          </span>
          <div className="ui-actions ui-model-actions">{actions}</div>
        </div>
      )}
    </section>
  );
}
