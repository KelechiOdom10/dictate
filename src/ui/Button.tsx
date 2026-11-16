import type { ComponentProps } from "react";
import { Button as AstryxButton } from "@astryxdesign/core/Button";
import { useThemeName } from "@astryxdesign/core/theme";

export type ControlSize = "sm" | "md" | "lg";

type Props = ComponentProps<"button"> & {
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
  const theme = useThemeName();
  if (theme) {
    const { children, disabled, title, ...rest } = props;
    return (
      <AstryxButton
        {...rest}
        type={type}
        size={size}
        variant={variant}
        label={
          props["aria-label"] ?? (typeof children === "string" ? children : "")
        }
        isDisabled={disabled}
        tooltip={title}
        className={`ui-button ui-size-${size} ${className}`}
        {...{ disabled }}
      >
        {children}
      </AstryxButton>
    );
  }
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
