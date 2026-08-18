import { clsx } from "clsx";
import "./Sparkline.css";

interface SparklineProps {
  readonly values: ReadonlyArray<number>;
  readonly width?: number;
  readonly height?: number;
  readonly accent?: string;
  readonly filled?: boolean;
  readonly className?: string;
}

// Pure SVG sparkline. Accepts raw values, normalizes to viewBox, renders a
// polyline path and an optional area fill. No external chart lib.
export function Sparkline({
  values,
  width = 120,
  height = 32,
  accent = "var(--c-text)",
  filled = true,
  className,
}: SparklineProps) {
  if (values.length === 0) return null;

  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = width / Math.max(values.length - 1, 1);

  const points = values.map((v, i) => {
    const x = i * stepX;
    const y = height - ((v - min) / range) * (height - 4) - 2;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const linePath = `M ${points.join(" L ")}`;
  const areaPath = `${linePath} L ${width},${height} L 0,${height} Z`;

  return (
    <svg
      className={clsx("ls-spark", className)}
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient id={`spark-grad-${accent}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={accent} stopOpacity="0.35" />
          <stop offset="100%" stopColor={accent} stopOpacity="0" />
        </linearGradient>
      </defs>
      {filled && <path d={areaPath} fill={`url(#spark-grad-${accent})`} />}
      <path d={linePath} fill="none" stroke={accent} strokeWidth="1.5" />
    </svg>
  );
}
