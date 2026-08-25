import { clsx } from "clsx";
import type { ReactNode } from "react";
import "./Badge.css";

type Tone = "neutral" | "live" | "new" | "original" | "hd" | "premium";

interface BadgeProps {
  readonly tone?: Tone;
  readonly icon?: ReactNode;
  readonly children: ReactNode;
  readonly className?: string;
}

export function Badge({ tone = "neutral", icon, children, className }: BadgeProps) {
  return (
    <span className={clsx("ls-badge", `ls-badge--${tone}`, className)}>
      {icon !== undefined && <span className="ls-badge__icon">{icon}</span>}
      <span>{children}</span>
    </span>
  );
}
