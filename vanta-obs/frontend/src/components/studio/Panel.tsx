import { ChevronDown } from "lucide-react";
import { useState, type ReactNode } from "react";

export function Panel({
  title,
  icon,
  summary,
  defaultCollapsed = true,
  children,
}: {
  readonly title: string;
  readonly icon: ReactNode;
  readonly summary?: ReactNode;
  readonly defaultCollapsed?: boolean;
  readonly children: ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  return (
    <section className={collapsed ? "obs-panel is-collapsed" : "obs-panel"}>
      <div className="obs-panel__head mono">
        <button
          type="button"
          className="obs-panel__toggle"
          aria-label={collapsed ? `Expand ${title}` : `Collapse ${title}`}
          aria-expanded={!collapsed}
          onClick={() => setCollapsed((current) => !current)}
        >
          <ChevronDown />
        </button>
        <span className="obs-panel__title">
          {icon}
          <span>{title}</span>
        </span>
        {summary ? <span className="obs-panel__summary">{summary}</span> : null}
      </div>
      <div className="obs-panel__body" hidden={collapsed}>
        {children}
      </div>
    </section>
  );
}
