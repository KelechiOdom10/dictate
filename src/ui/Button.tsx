import type { ButtonHTMLAttributes } from "react";

export type ControlSize = "sm" | "md" | "lg";

type Props = ButtonHTMLAttributes<HTMLButtonElement> & {
  size?: ControlSize;
  variant?: "primary" | "secondary" | "ghost";
};
export function Button({
  size = "md",
  variant = "secondary",
  className = "",
  type = "button",
  ...props
}: Props) {
  return (
    <button
      type={type}
      className={`ui-button ui-size-${size} ui-button-${variant} ${className}`}
      {...props}
    />
  );
}
export function IconButton({
  label,
  className = "",
  ...props
}: Omit<Props, "aria-label"> & { label: string }) {
  return (
    <Button
      {...props}
      aria-label={label}
      className={`ui-icon-button ${className}`}
    />
  );
}
