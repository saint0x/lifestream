import { useState } from "react";
import { Link } from "react-router-dom";
import { Play } from "lucide-react";
import type { Series } from "@/types";
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
              <div className="ls-eplist__thumb">
                <img src={ep.thumbnail} alt="" loading="lazy" />
                <div className="ls-eplist__play">
                  <Play size={14} strokeWidth={2} fill="currentColor" />
                </div>
              </div>
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
