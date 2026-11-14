import type { ReactNode } from "react";

export default function Row({
  id,
  label,
  help,
  children,
}: {
  id: string;
  label: string;
  help: string;
  children: ReactNode;
}) {
  return (
    <div className="setting-row">
      <div className="row-copy">
        <label htmlFor={id}>{label}</label>
        <p id={`${id}-help`}>{help}</p>
      </div>
      <div className="row-control">{children}</div>
    </div>
  );
}
