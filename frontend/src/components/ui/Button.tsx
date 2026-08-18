import { clsx } from "clsx";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import "./Button.css";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "outline";
type Size = "sm" | "md" | "lg";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  readonly variant?: Variant;
  readonly size?: Size;
  readonly icon?: ReactNode;
  readonly iconRight?: ReactNode;
  readonly full?: boolean;
}

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  iconRight,
  full = false,
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      className={clsx("ls-btn", `ls-btn--${variant}`, `ls-btn--${size}`, full && "ls-btn--full", className)}
      {...rest}
    >
      {icon !== undefined && <span className="ls-btn__icon">{icon}</span>}
      {children !== undefined && <span className="ls-btn__label">{children}</span>}
      {iconRight !== undefined && <span className="ls-btn__icon">{iconRight}</span>}
    </button>
  );
}
