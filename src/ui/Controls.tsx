import {
  useId,
  useLayoutEffect,
  useRef,
  type InputHTMLAttributes,
  type SelectHTMLAttributes,
} from "react";
import { ChevronDown, Search, X } from "lucide-react";
import { Button, IconButton, type ControlSize } from "./Button";

export function Toggle({
  className = "",
  size = "md",
  ...props
}: Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "role" | "size"> & {
  size?: ControlSize;
}) {
  return (
    <input
      {...props}
      className={`ui-toggle ui-size-${size} ${className}`}
      type="checkbox"
      role="switch"
    />
  );
}
export function Select({
  className = "",
  size = "md",
  ...props
}: Omit<SelectHTMLAttributes<HTMLSelectElement>, "size"> & {
  size?: ControlSize;
}) {
  return (
    <span className={`ui-select-wrap ui-size-${size}`}>
      <select {...props} className={`ui-select ${className}`} />
      <ChevronDown aria-hidden="true" />
    </span>
  );
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
  size = "md",
  label,
  options,
  value,
  onChange,
}: {
  size?: ControlSize;
  label: string;
  options: Choice[];
  value: string;
  onChange: (value: string) => void;
}) {
  const name = useId();
  const group = useRef<HTMLFieldSetElement>(null);
  useLayoutEffect(() => {
    const node = group.current;
    if (!node) return;
    const update = () => {
      const selected =
        node.querySelector<HTMLInputElement>("input:checked")?.parentElement;
      if (!selected) return;
      node.style.setProperty("--thumb-x", `${selected.offsetLeft}px`);
      node.style.setProperty("--thumb-y", `${selected.offsetTop}px`);
      node.style.setProperty("--thumb-width", `${selected.offsetWidth}px`);
      node.style.setProperty("--thumb-height", `${selected.offsetHeight}px`);
    };
    const observer = new ResizeObserver(update);
    observer.observe(node);
    update();
    return () => observer.disconnect();
  }, [value, options]);
  return (
    <fieldset ref={group} className={`ui-segments ui-size-${size}`}>
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
  size = "md",
  label,
  value,
  onChange,
  disabled,
  placeholder = "Search…",
}: {
  size?: ControlSize;
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className={`ui-search ui-size-${size}`}>
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
          size={size}
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
