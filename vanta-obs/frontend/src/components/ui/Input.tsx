import { clsx } from "clsx";
import type { InputHTMLAttributes, ReactNode } from "react";
import "./Input.css";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  readonly icon?: ReactNode;
  readonly iconRight?: ReactNode;
}

export function Input({ icon, iconRight, className, ...rest }: InputProps) {
  return (
    <label className={clsx("ls-input", className)}>
      {icon !== undefined && <span className="ls-input__icon">{icon}</span>}
      <input {...rest} />
      {iconRight !== undefined && <span className="ls-input__icon">{iconRight}</span>}
    </label>
  );
}
