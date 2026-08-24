import { Link } from "react-router-dom";
import "./PageTrail.css";

export interface PageTrailItem {
  readonly label: string;
  readonly href?: string;
}

interface PageTrailProps {
  readonly items: ReadonlyArray<PageTrailItem>;
  readonly className?: string;
  readonly suffix?: string;
  readonly showDot?: boolean;
}

export function PageTrail({ items, className, suffix, showDot = false }: PageTrailProps) {
  return (
    <nav className={`ls-page-trail ${className ?? ""}`} aria-label="Page navigation">
      {showDot ? <span className="ls-page-trail__dot" /> : null}
      {items.map((item, index) => {
        const isLast = index === items.length - 1;
        return (
          <span className="ls-page-trail__item" key={`${item.label}-${index}`}>
            {index > 0 ? <span className="ls-page-trail__sep">/</span> : null}
            {item.href && !isLast ? (
              <Link to={item.href}>{item.label}</Link>
            ) : (
              <span aria-current={isLast ? "page" : undefined}>{item.label}</span>
            )}
          </span>
        );
      })}
      {suffix ? (
        <>
          <span className="ls-page-trail__dash">-</span>
          <span className="ls-page-trail__suffix">{suffix}</span>
        </>
      ) : null}
    </nav>
  );
}
