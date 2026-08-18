import { useParams, Link, Navigate } from "react-router-dom";
import { Play, Plus, Check, Share2, Star } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { EpisodeList } from "@/components/content/EpisodeList";
import { ContentRow } from "@/components/content/ContentRow";
import "./DetailPage.css";

export function SeriesPage() {
  const { slug } = useParams<{ slug: string }>();
  const series = slug ? repository.getSeriesBySlug(slug) : undefined;
  const inWatchlist = useAppStore((s) => (series ? s.watchlist.has(series.id) : false));
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);

  if (!series) return <Navigate to="/" replace />;

  const firstEpisode = series.seasons[0]?.episodes[0];
  const related = repository
    .listByGenre(series.genres[0] ?? "Drama")
    .filter((c) => c.id !== series.id)
    .slice(0, 10);

  return (
    <div className="ls-detail" style={{ ["--hero-accent" as string]: series.heroColor }}>
      <div
        className="ls-detail__hero"
        style={{ backgroundImage: `url(${series.images.backdrop})` }}
      >
        <div className="ls-detail__hero-scrim" />
        <div className="ls-detail__hero-grid" />
      </div>

      <div className="ls-detail__content">
        <div className="ls-detail__header">
          <div>
            <div className="ls-detail__kicker mono">
              <span className="ls-detail__kicker-dot" />
              {series.isOriginal ? "LIFESTREAM ORIGINAL" : "SERIES"}
              <span className="ls-detail__kicker-sep">—</span>
              {series.status.toUpperCase()}
            </div>
            <h1 className="ls-detail__title">{series.title}</h1>
            {series.tagline !== undefined && (
              <div className="ls-detail__tagline serif">{series.tagline}</div>
            )}
            <div className="ls-detail__meta mono">
              <Badge tone="hd">{series.rating}</Badge>
              <span>{series.year}</span>
              <span className="ls-detail__sep">/</span>
              <span>{series.seasons.length} season{series.seasons.length > 1 ? "s" : ""}</span>
              <span className="ls-detail__sep">/</span>
              <span>{series.totalEpisodes} episodes</span>
              <span className="ls-detail__sep">/</span>
              <span className="ls-detail__score">
                <Star size={11} fill="currentColor" strokeWidth={0} /> {series.score}/100
              </span>
              <span className="ls-detail__sep">/</span>
              <span>{series.genres.join(" · ")}</span>
            </div>
            <p className="ls-detail__synopsis">{series.synopsis}</p>

            <div className="ls-detail__actions">
              {firstEpisode && (
                <Link to={`/watch/episode/${firstEpisode.id}`}>
                  <Button variant="primary" size="lg" icon={<Play fill="currentColor" />}>
                    Play S{firstEpisode.seasonNumber} E{firstEpisode.episodeNumber}
                  </Button>
                </Link>
              )}
              <Button
                variant="outline"
                size="lg"
                icon={inWatchlist ? <Check /> : <Plus />}
                onClick={() => toggleWatchlist(series.id)}
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
            {series.credits.map((c) => (
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

        <EpisodeList series={series} />

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
