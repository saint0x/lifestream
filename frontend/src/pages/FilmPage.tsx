import { useParams, Link, Navigate } from "react-router-dom";
import { Play, Plus, Check, Share2, Star } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { ContentRow } from "@/components/content/ContentRow";
import { formatRuntime } from "@/lib/format";
import "./DetailPage.css";

export function FilmPage() {
  const { slug } = useParams<{ slug: string }>();
  const film = slug ? repository.getFilmBySlug(slug) : undefined;
  const inWatchlist = useAppStore((s) => (film ? s.watchlist.has(film.id) : false));
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);

  if (!film) return <Navigate to="/" replace />;

  const related = repository
    .listByGenre(film.genres[0] ?? "Drama")
    .filter((c) => c.id !== film.id)
    .slice(0, 10);

  return (
    <div className="ls-detail" style={{ ["--hero-accent" as string]: film.heroColor }}>
      <div
        className="ls-detail__hero"
        style={{ backgroundImage: `url(${film.images.backdrop})` }}
      >
        <div className="ls-detail__hero-scrim" />
        <div className="ls-detail__hero-grid" />
      </div>

      <div className="ls-detail__content">
        <div className="ls-detail__header">
          <div>
            <div className="ls-detail__kicker mono">
              <span className="ls-detail__kicker-dot" />
              {film.isOriginal ? "LIFESTREAM ORIGINAL" : "FILM"}
              <span className="ls-detail__kicker-sep">—</span>
              {formatRuntime(film.durationSec)}
            </div>
            <h1 className="ls-detail__title">{film.title}</h1>
            {film.tagline !== undefined && (
              <div className="ls-detail__tagline serif">{film.tagline}</div>
            )}
            <div className="ls-detail__meta mono">
              <Badge tone="hd">{film.rating}</Badge>
              <span>{film.year}</span>
              <span className="ls-detail__sep">/</span>
              <span>{formatRuntime(film.durationSec)}</span>
              <span className="ls-detail__sep">/</span>
              <span className="ls-detail__score">
                <Star size={11} fill="currentColor" strokeWidth={0} /> {film.score}/100
              </span>
              <span className="ls-detail__sep">/</span>
              <span>{film.genres.join(" · ")}</span>
            </div>
            <p className="ls-detail__synopsis">{film.synopsis}</p>

            <div className="ls-detail__actions">
              <Link to={`/watch/film/${film.id}`}>
                <Button variant="primary" size="lg" icon={<Play fill="currentColor" />}>
                  Play Film
                </Button>
              </Link>
              <Button
                variant="outline"
                size="lg"
                icon={inWatchlist ? <Check /> : <Plus />}
                onClick={() => toggleWatchlist(film.id)}
              >
                {inWatchlist ? "On Watchlist" : "Add to Watchlist"}
              </Button>
              <Button variant="ghost" size="lg" icon={<Share2 />}>
                Share
              </Button>
            </div>
          </div>

          <aside className="ls-detail__credits">
            <div className="ls-detail__credit-label mono">Credits</div>
            {film.credits.map((c) => (
              <div key={c.id} className="ls-detail__credit">
                <div className="ls-detail__credit-name">{c.name}</div>
                <div className="ls-detail__credit-role mono">
                  {c.role}
                  {c.character !== undefined && ` · ${c.character}`}
                </div>
              </div>
            ))}
          </aside>
        </div>

        {related.length > 0 && (
          <ContentRow
            kicker="More like this"
            title="You might also like"
            items={related}
            layout="poster"
          />
        )}
      </div>
    </div>
  );
}
