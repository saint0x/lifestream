import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Play, Plus, Check, Info } from "lucide-react";
import type { Film, Series } from "@/types";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { useAppStore } from "@/lib/store";
import "./HeroCarousel.css";

interface HeroCarouselProps {
  readonly items: ReadonlyArray<Series | Film>;
}

export function HeroCarousel({ items }: HeroCarouselProps) {
  const [active, setActive] = useState(0);
  const isInWatchlist = useAppStore((s) => s.isInWatchlist);
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);

  useEffect(() => {
    if (items.length <= 1) return;
    const t = setInterval(() => setActive((i) => (i + 1) % items.length), 8000);
    return () => clearInterval(t);
  }, [items.length]);

  const current = items[active];
  if (!current) return null;

  const inWatchlist = isInWatchlist(current.id);
  const href =
    current.kind === "series" ? `/series/${current.slug}` : `/film/${current.slug}`;

  return (
    <section className="ls-hero">
      {items.map((item, i) => (
        <div
          key={item.id}
          className={`ls-hero__bg ${i === active ? "is-active" : ""}`}
          style={{
            backgroundImage: `url(${item.images.backdrop})`,
            ["--hero-accent" as string]: item.heroColor,
          }}
        />
      ))}
      <div className="ls-hero__gradient" />
      <div className="ls-hero__grid" />

      <div className="ls-hero__content">
        <div className="ls-hero__kicker mono">
          <span className="ls-hero__kicker-dot" />
          {current.isOriginal ? "VANTA ORIGINAL" : "FEATURED"}
          <span className="ls-hero__kicker-sep">—</span>
          <span>{current.kind === "series" ? "SERIES" : "FILM"}</span>
        </div>
        <h1 className="ls-hero__title">
          <span className="serif ls-hero__title-accent">{current.title.split(" ")[0]}</span>
          {current.title.split(" ").slice(1).length > 0 && (
            <span> {current.title.split(" ").slice(1).join(" ")}</span>
          )}
        </h1>
        {current.tagline !== undefined && (
          <div className="ls-hero__tagline serif">{current.tagline}</div>
        )}
        <p className="ls-hero__synopsis">{current.synopsis}</p>
        <div className="ls-hero__meta mono">
          <Badge tone="hd">{current.rating}</Badge>
          <span>{current.year}</span>
          <span className="ls-hero__sep">/</span>
          <span>{current.genres.slice(0, 3).join(" · ")}</span>
          <span className="ls-hero__sep">/</span>
          <span>
            <span className="ls-hero__score">{current.score}</span>
            <span className="faint"> / 100</span>
          </span>
        </div>

        <div className="ls-hero__actions">
          <Link to={href}>
            <Button variant="primary" size="lg" icon={<Play strokeWidth={2} fill="currentColor" />}>
              {current.kind === "series" ? "Watch Episode 1" : "Watch Film"}
            </Button>
          </Link>
          <Button
            variant="outline"
            size="lg"
            icon={inWatchlist ? <Check /> : <Plus />}
            onClick={() => toggleWatchlist(current.id)}
          >
            {inWatchlist ? "On Watchlist" : "Watchlist"}
          </Button>
          <Link to={href}>
            <Button variant="ghost" size="lg" icon={<Info />}>
              Details
            </Button>
          </Link>
        </div>
      </div>

      <div className="ls-hero__nav">
        {items.map((item, i) => (
          <button
            type="button"
            key={item.id}
            onClick={() => setActive(i)}
            className={`ls-hero__nav-item ${i === active ? "is-active" : ""}`}
            aria-label={`Go to slide ${i + 1}: ${item.title}`}
          >
            <span className="ls-hero__nav-num mono">{String(i + 1).padStart(2, "0")}</span>
            <span className="ls-hero__nav-title">{item.title}</span>
            <span className="ls-hero__nav-bar">
              <span className="ls-hero__nav-fill" />
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}
