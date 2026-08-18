import { Link } from "react-router-dom";
import { Play, Plus, Check } from "lucide-react";
import { clsx } from "clsx";
import type { ContentItem } from "@/types";
import { useAppStore } from "@/lib/store";
import { formatViewers, formatRuntime, clamp01 } from "@/lib/format";
import { Badge } from "@/components/ui/Badge";
import "./ContentCard.css";

interface ContentCardProps {
  readonly item: ContentItem;
  readonly layout?: "poster" | "landscape" | "wide";
  readonly showProgress?: number; // 0–1
  readonly className?: string;
}

export function ContentCard({
  item,
  layout = "poster",
  showProgress,
  className,
}: ContentCardProps) {
  const isInWatchlist = useAppStore((s) => s.watchlist.has(item.id));
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);

  const href =
    item.kind === "series"
      ? `/series/${item.slug}`
      : item.kind === "film"
        ? `/film/${item.slug}`
        : `/live/${item.slug}`;

  const poster =
    layout === "poster"
      ? (item.kind === "live" ? item.thumbnail : item.images.poster)
      : (item.kind === "live" ? item.thumbnail : item.images.thumbnail);

  const accent = item.kind === "live" ? "#ff2d55" : item.heroColor;

  return (
    <Link
      to={href}
      className={clsx("ls-card", `ls-card--${layout}`, className)}
      style={{ ["--card-accent" as string]: accent }}
    >
      <div className="ls-card__media">
        <img src={poster} alt={item.title} loading="lazy" />
        <div className="ls-card__scrim" />
        <div className="ls-card__top">
          {item.kind === "live" ? (
            <Badge tone="live">LIVE</Badge>
          ) : item.isOriginal ? (
            <Badge tone="original">LS ORIGINAL</Badge>
          ) : null}
          {item.kind !== "live" && item.isOriginal === false && item.trending ? (
            <Badge tone="new">NEW</Badge>
          ) : null}
        </div>

        <button
          className={clsx(
            "ls-card__watchlist",
            isInWatchlist && "ls-card__watchlist--active",
          )}
          aria-label={isInWatchlist ? "Remove from watchlist" : "Add to watchlist"}
          onClick={(e) => {
            e.preventDefault();
            toggleWatchlist(item.id);
          }}
        >
          {isInWatchlist ? <Check size={12} /> : <Plus size={12} />}
        </button>

        <div className="ls-card__play">
          <span className="ls-card__play-btn">
            <Play size={14} strokeWidth={2} fill="currentColor" />
          </span>
        </div>

        {showProgress !== undefined && showProgress > 0 && (
          <div className="ls-card__progress">
            <div
              className="ls-card__progress-fill"
              style={{ width: `${clamp01(showProgress) * 100}%` }}
            />
          </div>
        )}
      </div>

      <div className="ls-card__body">
        <div className="ls-card__title">{item.title}</div>
        <div className="ls-card__meta mono">
          {item.kind === "live" ? (
            <>
              <span className="ls-card__live-dot" />
              <span>{formatViewers(item.viewers)} watching</span>
              <span className="ls-card__meta-sep">·</span>
              <span>{item.category}</span>
            </>
          ) : item.kind === "film" ? (
            <>
              <span>{item.year}</span>
              <span className="ls-card__meta-sep">·</span>
              <span>{formatRuntime(item.durationSec)}</span>
              <span className="ls-card__meta-sep">·</span>
              <span>{item.rating}</span>
            </>
          ) : (
            <>
              <span>{item.year}</span>
              <span className="ls-card__meta-sep">·</span>
              <span>{item.seasons.length} season{item.seasons.length > 1 ? "s" : ""}</span>
              <span className="ls-card__meta-sep">·</span>
              <span>{item.rating}</span>
            </>
          )}
        </div>
      </div>
    </Link>
  );
}
