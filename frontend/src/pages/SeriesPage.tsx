import { useParams, Link, Navigate } from "react-router-dom";
import { useEffect, useState } from "react";
import { Play, Plus, Check, Share2, Star } from "lucide-react";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { AlertMeButton } from "@/components/alerts/AlertMeButton";
import { EpisodeList } from "@/components/content/EpisodeList";
import { ContentRow } from "@/components/content/ContentRow";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { PageTrail } from "@/components/navigation/PageTrail";
import { shareCurrentPage } from "@/lib/share";
import type { Film, Series } from "@/types";
import "./DetailPage.css";

export function SeriesPage() {
  const { slug } = useParams<{ slug: string }>();
  const [series, setSeries] = useState(repository.hasState() && slug ? repository.getSeriesBySlug(slug) : undefined);
  const [related, setRelated] = useState<ReadonlyArray<Series | Film>>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const inWatchlist = useAppStore((s) => (series ? s.watchlist.has(series.id) : false));
  const toggleWatchlist = useAppStore((s) => s.toggleWatchlist);
  const [shareStatus, setShareStatus] = useState<string | null>(null);

  useEffect(() => {
    if (!slug) return;
    const controller = new AbortController();
    setLoading(true);
    setLoadError(null);

    void repository
      .fetchSeriesBySlug(slug, controller.signal)
      .then(async (item) => {
        setSeries(item);
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
          setSeries(undefined);
          setLoadError(err instanceof Error ? err.message : "Unable to load this series.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [slug]);

  if (!slug) return <Navigate to="/" replace />;
  if (!series) {
    return (
      <div className="ls-detail">
        <div className="ls-detail__content">
          <div className="ls-detail__status">
            {loading ? "Loading series…" : loadError ?? "Series is unavailable."}
          </div>
        </div>
      </div>
    );
  }

  const firstEpisode = series.seasons[0]?.episodes[0];

  return (
    <div className="ls-detail" style={{ ["--hero-accent" as string]: series.heroColor }}>
      <PageMetadata
        title={`${series.title} - VANTA series`}
        description={`${series.synopsis} Watch ${series.title}, a premium long-form episodic series on VANTA.`}
        path={`/series/${series.slug}`}
        image={series.images.backdrop}
        type="video.tv_show"
        structuredData={{
          "@context": "https://schema.org",
          "@type": "TVSeries",
          name: series.title,
          description: series.synopsis,
          genre: series.genres,
          contentRating: series.rating,
          datePublished: String(series.year),
          image: [series.images.poster, series.images.backdrop],
          numberOfEpisodes: series.totalEpisodes,
          numberOfSeasons: series.seasons.length,
          actor: series.credits
            .filter((credit) => /actor|cast|star/i.test(credit.role))
            .map((credit) => ({ "@type": "Person", name: credit.name })),
          creator: series.credits
            .filter((credit) => /creator|writer|director/i.test(credit.role))
            .map((credit) => ({ "@type": "Person", name: credit.name })),
          episode: series.seasons.flatMap((season) =>
            season.episodes.map((episode) => ({
              "@type": "TVEpisode",
              name: episode.title,
              description: episode.synopsis,
              seasonNumber: episode.seasonNumber,
              episodeNumber: episode.episodeNumber,
              datePublished: episode.airedAt,
              duration: `PT${episode.durationSec}S`,
              image: episode.thumbnail,
            })),
          ),
        }}
      />
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
            <PageTrail
              className="ls-detail__kicker mono"
              showDot
              suffix={series.status.toUpperCase()}
              items={[
                { label: "Dashboard", href: "/" },
                { label: "Series", href: "/series" },
                { label: series.title },
              ]}
            />
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
              <Button
                variant="ghost"
                size="lg"
                icon={<Share2 />}
                onClick={() => {
                  void shareCurrentPage(series.title)
                    .then(setShareStatus)
                    .catch(() => setShareStatus("Unable to share this series."));
                }}
              >
                Share
              </Button>
              <AlertMeButton
                targetKind="series"
                targetId={series.id}
                targetSlug={series.slug}
                targetTitle={series.title}
                alertTypes={["new_episode", "series_drop"]}
              />
            </div>
            {shareStatus ? <div className="ls-detail__status">{shareStatus}</div> : null}
          </div>

          <aside className="ls-detail__credits">
            <div className="ls-detail__credit-label mono">Credits</div>
            {series.credits.map((c) => (
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

        <EpisodeList series={series} />

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
