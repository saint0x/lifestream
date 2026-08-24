import { Link } from "react-router-dom";
import { seriesCreditsVariant } from "@/lib/featureFlags";
import type { Credit } from "@/types";

interface SeriesCreditsProps {
  readonly credits: ReadonlyArray<Credit>;
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("") || "V";
}

function CreditName({ credit, className }: { readonly credit: Credit; readonly className: string }) {
  return credit.personSlug ? (
    <Link className={className} to={`/@${credit.personSlug}`}>
      {credit.name}
    </Link>
  ) : (
    <div className={className}>{credit.name}</div>
  );
}

export function SeriesCredits({ credits }: SeriesCreditsProps) {
  const variant = seriesCreditsVariant();

  if (variant === "glass-squares") {
    return (
      <aside className="ls-detail__credits ls-detail__credits--squares">
        <div className="ls-detail__credit-label mono">Credits</div>
        <div className="ls-detail__credit-grid">
          {credits.map((credit) => (
            <article key={credit.id} className="ls-detail__credit-card">
              {credit.avatar ? (
                <img className="ls-detail__credit-card-avatar" src={credit.avatar} alt="" />
              ) : (
                <div className="ls-detail__credit-card-avatar ls-detail__credit-card-avatar--fallback">
                  {initials(credit.name)}
                </div>
              )}
              <div className="ls-detail__credit-card-copy">
                <CreditName credit={credit} className="ls-detail__credit-card-name" />
                <div className="ls-detail__credit-card-role mono">
                  {credit.role}
                  {credit.character != null && ` / ${credit.character}`}
                </div>
              </div>
            </article>
          ))}
        </div>
      </aside>
    );
  }

  return (
    <aside className="ls-detail__credits">
      <div className="ls-detail__credit-label mono">Credits</div>
      {credits.map((credit) => (
        <div key={credit.id} className="ls-detail__credit">
          {credit.avatar ? (
            <img className="ls-detail__credit-avatar" src={credit.avatar} alt="" />
          ) : null}
          <div className="ls-detail__credit-copy">
            <CreditName credit={credit} className="ls-detail__credit-name" />
            <div className="ls-detail__credit-role mono">
              {credit.role}
              {credit.character != null && ` · ${credit.character}`}
            </div>
          </div>
        </div>
      ))}
    </aside>
  );
}
