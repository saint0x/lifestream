import type { ReactNode } from "react";
import { clsx } from "clsx";
import { ArrowUpRight, ArrowDownRight } from "lucide-react";
import { Sparkline } from "./Sparkline";
import "./StatCard.css";

interface StatCardProps {
  readonly label: string;
  readonly value: string;
  readonly unit?: string;
  readonly delta?: number; // e.g. +12.4 or -3.2 (percentage)
  readonly spark?: ReadonlyArray<number>;
  readonly footer?: ReactNode;
  readonly accent?: string;
  readonly size?: "sm" | "md" | "lg";
  readonly className?: string;
}

export function StatCard({
  label,
  value,
  unit,
  delta,
  spark,
  footer,
  accent = "var(--c-text)",
  size = "md",
  className,
}: StatCardProps) {
  const positive = delta !== undefined && delta >= 0;

  return (
    <div
      className={clsx("ls-stat", `ls-stat--${size}`, className)}
      style={{ ["--stat-accent" as string]: accent }}
    >
      <div className="ls-stat__head">
        <div className="ls-stat__label mono">{label}</div>
        {delta !== undefined && (
          <span
            className={clsx(
              "ls-stat__delta mono",
              positive ? "ls-stat__delta--up" : "ls-stat__delta--down",
            )}
          >
            {positive ? <ArrowUpRight size={11} /> : <ArrowDownRight size={11} />}
            {Math.abs(delta).toFixed(1)}%
          </span>
        )}
      </div>
      <div className="ls-stat__value-row">
        <div className="ls-stat__value">
          {value}
          {unit !== undefined && <span className="ls-stat__unit mono">{unit}</span>}
        </div>
        {spark && spark.length > 0 && (
          <Sparkline values={spark} width={96} height={32} accent={accent} />
        )}
      </div>
      {footer !== undefined && <div className="ls-stat__footer mono">{footer}</div>}
    </div>
  );
}
