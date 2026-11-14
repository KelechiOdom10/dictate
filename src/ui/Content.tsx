import { useId, type ReactNode } from "react";
import { Button } from "./Button";

export function Card({ children }: { children: ReactNode }) {
  return <div className="ui-card">{children}</div>;
}
export function ListItem({
  date,
  title,
  preview,
  actions,
}: {
  date: string;
  title: string;
  preview: string;
  actions?: ReactNode;
}) {
  return (
    <article className="ui-list-item">
      <p className="ui-date">{date}</p>
      <h2>{title}</h2>
      <p className="ui-preview">{preview}</p>
      {actions && <div className="ui-actions">{actions}</div>}
    </article>
  );
}
export function DetailPanel({
  title,
  metadata,
  children,
}: {
  title: string;
  metadata: { label: string; value: string }[];
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="ui-detail" aria-labelledby={id}>
      <h2 id={id}>{title}</h2>
      <dl>
        {metadata.map((row) => (
          <div key={row.label}>
            <dt>{row.label}</dt>
            <dd>{row.value}</dd>
          </div>
        ))}
      </dl>
      <div className="ui-detail-body">{children}</div>
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
    <div className="empty">
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
  tone?: "success" | "warning" | "danger";
  children: ReactNode;
  onDismiss?: () => void;
}) {
  return (
    <div
      className={`ui-toast ui-toast-${tone}`}
      role={tone === "danger" ? "alert" : "status"}
    >
      <span className="ui-toast-tone">
        {tone === "success"
          ? "Saved"
          : tone === "warning"
            ? "Attention"
            : "Error"}
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
      <ProgressBar label="Accuracy" value={accuracy} />
      <ProgressBar label="Speed" value={speed} />
      <div className="ui-model-footer">
        <span className="ui-date">{size}</span>
        {actions[status.kind]}
      </div>
    </Card>
  );
}
