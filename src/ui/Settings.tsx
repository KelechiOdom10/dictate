import { useId, type ReactNode } from "react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { Heading } from "@astryxdesign/core/Heading";

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
    <HStack className="ui-setting-row" gap={4} vAlign="center" hAlign="between">
      <VStack className="ui-row-copy" gap={0}>
        <label htmlFor={id}>{label}</label>
        <p id={`${id}-help`}>{help}</p>
      </VStack>
      <HStack className="ui-row-control" hAlign="end">
        {children}
      </HStack>
    </HStack>
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
      <Heading level={2} id={id}>
        {icon}
        {title}
      </Heading>
      <VStack gap={0} className="ui-group">
        {children}
      </VStack>
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
export function PathText({ children }: { children: string }) {
  return (
    <code className="ui-path" title={children}>
      {children}
    </code>
  );
}
