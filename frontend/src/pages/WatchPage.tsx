import { useEffect, useState } from "react";
import { useParams, Link, Navigate } from "react-router-dom";
import { ChevronLeft, Play } from "lucide-react";
import { repository } from "@/lib/repository";
import { VideoPlayer } from "@/components/player/VideoPlayer";
import { useAppStore } from "@/lib/store";
import { formatDuration } from "@/lib/format";
import { getApiBaseUrl, requestJson } from "@/lib/api";
import type { PlaybackGrant } from "@/types";
import "./WatchPage.css";

interface WatchPageProps {
  readonly kind: "episode" | "film";
}

export function WatchPage({ kind }: WatchPageProps) {
  const { id } = useParams<{ id: string }>();
  const recordProgress = useAppStore((s) => s.recordProgress);
  const [playbackGrant, setPlaybackGrant] = useState<PlaybackGrant | null>(null);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const [playbackLoading, setPlaybackLoading] = useState(false);
  const episode = kind === "episode" && id ? repository.getEpisode(id) : undefined;
  const series = kind === "episode" && episode ? repository.getSeriesById(episode.seriesId) : undefined;
  const film = kind === "film" && id ? repository.getFilmById(id) : undefined;
  const playbackSessionUrl =
    kind === "episode"
      ? episode?.playbackSessionUrl
      : film?.playbackSessionUrl;

  useEffect(() => {
    if (!playbackSessionUrl) {
      setPlaybackGrant(null);
      setPlaybackError("Playback is not available for this title yet.");
      setPlaybackLoading(false);
      return;
    }

    setPlaybackLoading(true);
    setPlaybackError(null);
    void requestJson<PlaybackGrant>(playbackSessionUrl, { method: "POST" })
      .then((grant) => {
        setPlaybackGrant(grant);
      })
      .catch((error) => {
        setPlaybackGrant(null);
        setPlaybackError(error instanceof Error ? error.message : "Unable to start playback.");
      })
      .finally(() => {
        setPlaybackLoading(false);
      });
  }, [playbackSessionUrl]);

  if (kind === "episode") {
    if (!episode) return <Navigate to="/" replace />;
    if (!series) return <Navigate to="/" replace />;

    const season = series.seasons.find((s) => s.seasonNumber === episode.seasonNumber);
    const idxInSeason = season?.episodes.findIndex((e) => e.id === episode.id) ?? -1;
    const nextEpisode =
      idxInSeason >= 0 && season
        ? season.episodes[idxInSeason + 1]
        : undefined;

    return (
      <div className="ls-watch">
        <header className="ls-watch__bar">
          <Link to={`/series/${series.slug}`} className="ls-watch__back">
            <ChevronLeft size={14} /> Back to {series.title}
          </Link>
          <div className="ls-watch__crumbs mono">
            <span>{series.title}</span>
            <span>/</span>
            <span>S{String(episode.seasonNumber).padStart(2, "0")}</span>
            <span>/</span>
            <span>E{String(episode.episodeNumber).padStart(2, "0")}</span>
          </div>
        </header>

        <div className="ls-watch__player">
          {playbackLoading ? <div className="ls-watch__state">Preparing playback session…</div> : null}
          {playbackError ? <div className="ls-watch__state ls-watch__state--error">{playbackError}</div> : null}
          <VideoPlayer
            poster={playbackGrant?.posterUrl ? `${getApiBaseUrl()}${playbackGrant.posterUrl}` : series.images.backdrop}
            title={`${series.title} — ${episode.title}`}
            subtitle="[ they don't know the signal is a song ]"
            durationSec={episode.durationSec}
            initialProgressSec={episode.progressSec ?? 0}
            sourceUrl={playbackGrant ? `${getApiBaseUrl()}${playbackGrant.manifestUrl}` : null}
            onProgress={(sec) => {
              recordProgress({
                contentId: series.id,
                kind: "series",
                episodeId: episode.id,
                progressSec: sec,
                durationSec: episode.durationSec,
                lastWatchedAt: new Date().toISOString(),
              });
            }}
          />
        </div>

        <section className="ls-watch__below">
          <div className="ls-watch__info">
            <div className="ls-watch__kicker mono">
              SEASON {episode.seasonNumber} · EPISODE {episode.episodeNumber}
            </div>
            <h1 className="ls-watch__title">{episode.title}</h1>
            <p className="ls-watch__synopsis">{episode.synopsis}</p>
          </div>

          {nextEpisode && (
            <Link to={`/watch/episode/${nextEpisode.id}`} className="ls-watch__next">
              <div className="ls-watch__next-thumb">
                <img src={nextEpisode.thumbnail} alt="" />
                <div className="ls-watch__next-play">
                  <Play size={20} fill="currentColor" />
                </div>
              </div>
              <div className="ls-watch__next-body">
                <div className="ls-watch__next-label mono">UP NEXT</div>
                <div className="ls-watch__next-title">
                  E{nextEpisode.episodeNumber} · {nextEpisode.title}
                </div>
                <div className="ls-watch__next-meta mono">
                  {formatDuration(nextEpisode.durationSec)}
                </div>
              </div>
            </Link>
          )}
        </section>
      </div>
    );
  }

  if (!film) return <Navigate to="/" replace />;

  return (
    <div className="ls-watch">
      <header className="ls-watch__bar">
        <Link to={`/film/${film.slug}`} className="ls-watch__back">
          <ChevronLeft size={14} /> Back to {film.title}
        </Link>
        <div className="ls-watch__crumbs mono">
          <span>LIFESTREAM</span>
          <span>/</span>
          <span>FILM</span>
        </div>
      </header>

      <div className="ls-watch__player">
        {playbackLoading ? <div className="ls-watch__state">Preparing playback session…</div> : null}
        {playbackError ? <div className="ls-watch__state ls-watch__state--error">{playbackError}</div> : null}
        <VideoPlayer
          poster={playbackGrant?.posterUrl ? `${getApiBaseUrl()}${playbackGrant.posterUrl}` : film.images.backdrop}
          title={film.title}
          durationSec={film.durationSec}
          initialProgressSec={film.progressSec ?? 0}
          sourceUrl={playbackGrant ? `${getApiBaseUrl()}${playbackGrant.manifestUrl}` : null}
          onProgress={(sec) => {
            recordProgress({
              contentId: film.id,
              kind: "film",
              progressSec: sec,
              durationSec: film.durationSec,
              lastWatchedAt: new Date().toISOString(),
            });
          }}
        />
      </div>

      <section className="ls-watch__below">
        <div className="ls-watch__info">
          <div className="ls-watch__kicker mono">
            FILM · {film.year} · {film.rating}
          </div>
          <h1 className="ls-watch__title">{film.title}</h1>
          <p className="ls-watch__synopsis">{film.synopsis}</p>
        </div>
      </section>
    </div>
  );
}
