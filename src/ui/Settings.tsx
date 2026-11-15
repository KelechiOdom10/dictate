import { useId, type ReactNode } from "react";

export function SettingsRow({
  id,
  label,
  help,
  children,
}: {
  id: string;
  label: string;
  help: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="ui-setting-row">
      <div className="ui-row-copy">
        <label htmlFor={id}>{label}</label>
        <p id={`${id}-help`}>{help}</p>
      </div>
      <div className="ui-row-control">{children}</div>
    </div>
  );
}
export function SettingsGroup({
  title,
  icon,
  children,
}: {
  title: string;
  icon?: ReactNode;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="ui-settings-group" aria-labelledby={id}>
      <h2 id={id}>
        {icon}
        {title}
      </h2>
      <div className="ui-group">{children}</div>
    </section>
  );
}
export function SidebarNav({
  label,
  items,
  value,
  onChange,
}: {
  label: string;
  items: { value: string; label: string; icon?: ReactNode }[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <nav className="ui-sidebar-nav" aria-label={label}>
      {items.map((item) => (
        <button
          type="button"
          key={item.value}
          aria-current={value === item.value ? "page" : undefined}
          className={value === item.value ? "selected" : ""}
          onClick={() => onChange(item.value)}
        >
          {item.icon}
          {item.label}
        </button>
      ))}
    </nav>
  );
}
