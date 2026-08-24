import { useParams, Link, Navigate } from "react-router-dom";
import { useEffect, useState } from "react";
import { Play, Plus, Check, Share2, Star } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { ContentRow } from "@/components/content/ContentRow";
import { formatRuntime } from "@/lib/format";
import { shareCurrentPage } from "@/lib/share";
import type { Film, Series } from "@/types";
import "./DetailPage.css";

export function FilmPage() {
  const { slug } = useParams<{ slug: string }>();
  const [film, setFilm] = useState(repository.hasState() && slug ? repository.getFilmBySlug(slug) : undefined);
  const [related, setRelated] = useState<ReadonlyArray<Series | Film>>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const inWatchlist = useAppStore((s) => (film ? s.watchlist.has(film.id) : false));
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);
  const [shareStatus, setShareStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!slug) return;
    const controller = new AbortController();
    setLoading(true);
    setLoadError(null);

    void repository
      .fetchFilmBySlug(slug, controller.signal)
      .then(async (item) => {
        setFilm(item);
        const [seriesPage, filmsPage] = await Promise.all([
          repository.fetchSeriesPage({ genre: item.genres[0] ?? "Drama", limit: 10 }, controller.signal),
          repository.fetchFilmsPage({ genre: item.genres[0] ?? "Drama", limit: 10 }, controller.signal),
        ]);
        setRelated(
          [...seriesPage.items, ...filmsPage.items]
            .filter((candidate) => candidate.id !== item.id)
            .slice(0, 10),
        );
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setFilm(undefined);
          setLoadError(err instanceof Error ? err.message : "Unable to load this title.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [slug]);

  if (!slug) return <Navigate to="/" replace />;
  if (!film) {
    return (
      <div className="ls-detail">
        <div className="ls-detail__content">
          <div className="ls-detail__status">
            {loading ? "Loading title…" : loadError ?? "Title is unavailable."}
          </div>
        </div>
      </div>
    );
  }

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
              FILM
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
              <Button
                variant="ghost"
                size="lg"
                icon={<Share2 />}
                onClick={() => {
                  void shareCurrentPage(film.title)
                    .then(setShareStatus)
                    .catch(() => setShareStatus("Unable to share this title."));
                }}
              >
                Share
              </Button>
            </div>
            {shareStatus ? <div className="ls-detail__status">{shareStatus}</div> : null}
          </div>

          <aside className="ls-detail__credits">
            <div className="ls-detail__credit-label mono">Credits</div>
            {film.credits.map((c) => (
              <div key={c.id} className="ls-detail__credit">
                {c.avatar ? (
                  <img className="ls-detail__credit-avatar" src={c.avatar} alt="" />
                ) : null}
                <div className="ls-detail__credit-copy">
                  {c.personSlug ? (
                    <Link className="ls-detail__credit-name" to={`/@${c.personSlug}`}>
                      {c.name}
                    </Link>
                  ) : (
                    <div className="ls-detail__credit-name">{c.name}</div>
                  )}
                  <div className="ls-detail__credit-role mono">
                    {c.role}
                    {c.character != null && ` · ${c.character}`}
                  </div>
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
            layout="landscape"
          />
        )}
      </div>
    </div>
  );
}
