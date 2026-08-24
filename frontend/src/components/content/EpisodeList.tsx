import { useState } from "react";
import { Link } from "react-router-dom";
import { Play } from "lucide-react";
import type { Episode, Series } from "@/types";
import { formatDuration } from "@/lib/format";
import "./EpisodeList.css";

interface EpisodeListProps {
  readonly series: Series;
}

export function EpisodeList({ series }: EpisodeListProps) {
  const [seasonNumber, setSeasonNumber] = useState<number>(
    series.seasons[0]?.seasonNumber ?? 1,
  );
  const season = series.seasons.find((s) => s.seasonNumber === seasonNumber);

  return (
    <section className="ls-eplist">
      <div className="ls-eplist__head">
        <h2 className="ls-eplist__title">Episodes</h2>
        <div className="ls-eplist__seasons">
          {series.seasons.map((s) => (
            <button
              key={s.seasonNumber}
              type="button"
              onClick={() => setSeasonNumber(s.seasonNumber)}
              className={`ls-eplist__season ${
                s.seasonNumber === seasonNumber ? "is-active" : ""
              }`}
            >
              <span className="mono">S{String(s.seasonNumber).padStart(2, "0")}</span>
              <span>{s.title}</span>
            </button>
          ))}
        </div>
      </div>

      <ul className="ls-eplist__list">
        {season?.episodes.map((ep) => (
          <li key={ep.id}>
            <Link to={`/watch/episode/${ep.id}`} className="ls-eplist__item">
              <div className="ls-eplist__num mono">
                {String(ep.episodeNumber).padStart(2, "0")}
              </div>
              <EpisodeThumbnail episode={ep} fallbackImage={series.images.backdrop} />
              <div className="ls-eplist__body">
                <div className="ls-eplist__row">
                  <div className="ls-eplist__ep-title">{ep.title}</div>
                  <div className="ls-eplist__dur mono">{formatDuration(ep.durationSec)}</div>
                </div>
                <div className="ls-eplist__synopsis">{ep.synopsis}</div>
                <div className="ls-eplist__aired mono">Aired {ep.airedAt}</div>
              </div>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}

interface EpisodeThumbnailProps {
  readonly episode: Episode;
  readonly fallbackImage: string;
}

function EpisodeThumbnail({ episode, fallbackImage }: EpisodeThumbnailProps) {
  const [imageSrc, setImageSrc] = useState(episode.thumbnail || fallbackImage);
  const [failedPrimary, setFailedPrimary] = useState(false);
  const showFallback = failedPrimary && imageSrc === "";

  return (
    <div className="ls-eplist__thumb">
      {showFallback ? (
        <div className="ls-eplist__thumb-fallback" aria-hidden="true">
          <span>{episode.title.slice(0, 1)}</span>
        </div>
      ) : (
        <img
          src={imageSrc}
          alt=""
          loading="lazy"
          onError={() => {
            if (!failedPrimary && fallbackImage && imageSrc !== fallbackImage) {
              setFailedPrimary(true);
              setImageSrc(fallbackImage);
              return;
            }
            setFailedPrimary(true);
            setImageSrc("");
          }}
        />
      )}
      <div className="ls-eplist__play">
        <Play size={14} strokeWidth={2} fill="currentColor" />
      </div>
    </div>
  );
}
