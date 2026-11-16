import type { ReactNode } from "react";

/**
 * A radio the size of the thing it chooses.
 *
 * The input stays a native radio: focusable, arrow-key navigable inside its
 * group, and announced as one option among its siblings. Only the paint is
 * ours, because the library has no single-select card — Core's SelectableCard
 * is checkbox-only and the group form lives in an unreleased Lab package — and
 * a hand-rolled button would lose the group semantics a screen reader needs.
 */
export function Choice({
  name,
  value,
  label,
  checked,
  disabled,
  onChange,
  children,
}: {
  name: string;
  value: string;
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: () => void;
  children: ReactNode;
}) {
  return (
    <label className="ui-choice" data-selected={checked ? "true" : undefined}>
      <input
        type="radio"
        name={name}
        value={value}
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
      {children}
    </label>
  );
}
