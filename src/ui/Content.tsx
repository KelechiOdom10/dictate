import { useId, type ReactNode } from "react";
import { Check, CircleAlert, Info } from "lucide-react";
import { Button } from "./Button";

export function Card({ children }: { children: ReactNode }) {
  return <div className="ui-card">{children}</div>;
}
type ListItemSelection =
  | { selected: boolean; onSelect: () => void; actions?: never }
  | { selected?: never; onSelect?: never; actions?: ReactNode };
export function ListItem({
  date,
  title,
  meta,
  preview,
  selected,
  onSelect,
  actions,
}: {
  date: string;
  title: string;
  meta?: string;
  preview?: string;
} & ListItemSelection) {
  const body = (
    <>
      <span className="ui-list-heading">
        <span className="ui-list-title">{title}</span>
        <span className="ui-date">{date}</span>
      </span>
      {meta && <span className="ui-list-meta">{meta}</span>}
      {preview && <span className="ui-preview">{preview}</span>}
    </>
  );
  if (onSelect)
    return (
      <button
        type="button"
        className="ui-list-item ui-list-item-button"
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
      >
        {body}
      </button>
    );
  return (
    <article className="ui-list-item">
      {body}
      {actions && <div className="ui-actions">{actions}</div>}
    </article>
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
}: {
  label: string;
  value?: number;
}) {
  return (
    <div className="ui-progress">
      <span>{label}</span>
      <progress aria-label={label} max={100} value={value} />
      {value !== undefined && (
        <span className="ui-date">{Math.round(value)}%</span>
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
export type ModelStatus =
  | { kind: "available"; onDownload: () => void }
  | { kind: "downloading"; progress: number }
  | { kind: "installed" }
  | { kind: "error"; message: string; onRetry: () => void };
export function ModelCard({
  name,
  description,
  accuracy,
  speed,
  size,
  badge,
  status,
}: {
  name: string;
  description: string;
  accuracy: number;
  speed: number;
  size: string;
  badge?: string;
  status: ModelStatus;
}) {
  const actions: Record<ModelStatus["kind"], ReactNode> = {
    available: status.kind === "available" && (
      <Button onClick={status.onDownload}>Download {name}</Button>
    ),
    downloading: status.kind === "downloading" && (
      <ProgressBar label={`Downloading ${name}`} value={status.progress} />
    ),
    installed: <span className="ui-badge">Installed</span>,
    error: status.kind === "error" && (
      <>
        <p role="alert">{status.message}</p>
        <Button onClick={status.onRetry}>Retry {name}</Button>
      </>
    ),
  };
  return (
    <Card>
      <div className="ui-model-heading">
        <h2>{name}</h2>
        {badge && <span className="ui-badge">{badge}</span>}
      </div>
      <p className="ui-preview">{description}</p>
      <ModelRating name={name} label="Accuracy" value={accuracy} />
      <ModelRating name={name} label="Speed" value={speed} />
      <div className="ui-model-footer">
        <span className="ui-date">{size}</span>
        {actions[status.kind]}
      </div>
    </Card>
  );
}

function ModelRating({
  name,
  label,
  value,
}: {
  name: string;
  label: string;
  value: number;
}) {
  return (
    <div className="ui-progress">
      <span>{label}</span>
      <meter
        min={0}
        max={100}
        value={value}
        aria-label={`${name} ${label.toLowerCase()}`}
      />
      <span className="ui-date">{value}%</span>
    </div>
  );
}
