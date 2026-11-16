import {
  useId,
  useRef,
  type InputHTMLAttributes,
  type SelectHTMLAttributes,
} from "react";
import { Search, X } from "lucide-react";
import { Button, IconButton } from "./Button";

export function Toggle({
  className = "",
  ...props
}: Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "role">) {
  return (
    <input
      {...props}
      className={`ui-toggle ${className}`}
      type="checkbox"
      role="switch"
    />
  );
}
export function Select({
  className = "",
  ...props
}: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={`ui-select ${className}`} />;
}
export function Chip({
  selected = false,
  children,
  ...props
}: Omit<React.ComponentProps<typeof Button>, "variant"> & {
  selected?: boolean;
}) {
  return (
    <Button
      {...props}
      variant={selected ? "primary" : "secondary"}
      aria-pressed={selected}
    >
      {children}
    </Button>
  );
}
export type Choice = { value: string; label: string; disabled?: boolean };
export function FilterChips({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: Choice[];
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="ui-chips" role="group" aria-label={label}>
      {options.map((option) => (
        <Chip
          key={option.value}
          selected={value === option.value}
          disabled={option.disabled}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </Chip>
      ))}
    </div>
  );
}
export function SegmentedControl({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: Choice[];
  value: string;
  onChange: (value: string) => void;
}) {
  const name = useId();
  return (
    <fieldset className="ui-segments">
      <legend className="sr-only">{label}</legend>
      {options.map((option) => (
        <label key={option.value}>
          <input
            type="radio"
            name={name}
            value={option.value}
            checked={value === option.value}
            disabled={option.disabled}
            onChange={() => onChange(option.value)}
          />
          <span>{option.label}</span>
        </label>
      ))}
    </fieldset>
  );
}
export function SearchField({
  label,
  value,
  onChange,
  disabled,
  placeholder = "Search…",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className="ui-search">
      <Search size={16} aria-hidden="true" />
      <label className="sr-only" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        ref={input}
        type="search"
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      />
      {value && (
        <IconButton
          label={`Clear ${label.toLowerCase()}`}
          variant="ghost"
          disabled={disabled}
          onClick={() => {
            onChange("");
            input.current?.focus();
          }}
        >
          <X size={14} aria-hidden="true" />
        </IconButton>
      )}
    </div>
  );
}
