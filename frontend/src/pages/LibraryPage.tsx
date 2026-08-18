import { Link } from "react-router-dom";
import { Play, X } from "lucide-react";
import { useAppStore } from "@/lib/store";
import { repository } from "@/lib/repository";
import { formatDuration, clamp01 } from "@/lib/format";
import "./ListPage.css";

export function LibraryPage() {
  const library = useAppStore((s) => s.library);
  const continueWatching = library.continueWatching;
  const remove = useAppStore((s) => s.removeFromContinueWatching);

  return (
    <div className="ls-list">
      <header className="ls-list__head">
        <div className="ls-list__kicker mono">/ yours / library</div>
        <h1 className="ls-list__title">Library</h1>
        <p className="ls-list__sub">
          Continue where you left off — {continueWatching.length} item
          {continueWatching.length === 1 ? "" : "s"} in progress
        </p>
      </header>

      {continueWatching.length === 0 ? (
        <div className="ls-list__empty">
          <Play size={24} strokeWidth={1.5} />
          <div>Nothing in progress.</div>
          <p>Start watching anything and it will show up here.</p>
        </div>
      ) : (
        <div className="ls-list__rows">
          {continueWatching.map((entry) => {
            const content = repository.getByAnyId(entry.contentId);
            if (!content || content.kind === "live") return null;
            const ratio = clamp01(entry.progressSec / entry.durationSec);
            const remaining = entry.durationSec - entry.progressSec;
            const href =
              entry.kind === "series" && entry.episodeId !== undefined
                ? `/watch/episode/${entry.episodeId}`
                : `/watch/film/${content.id}`;

            const ep =
              entry.episodeId !== undefined
                ? repository.getEpisode(entry.episodeId)
                : undefined;

            return (
              <div key={entry.contentId} className="ls-list__row">
                <Link to={href} className="ls-list__row-main">
                  <div className="ls-list__row-thumb">
                    <img
                      src={ep?.thumbnail ?? content.images.thumbnail}
                      alt=""
                    />
                    <div className="ls-list__row-bar">
                      <div
                        className="ls-list__row-fill"
                        style={{ width: `${ratio * 100}%` }}
                      />
                    </div>
                    <div className="ls-list__row-play">
                      <Play size={16} fill="currentColor" />
                    </div>
                  </div>
                  <div className="ls-list__row-body">
                    <div className="ls-list__row-kicker mono">
                      {content.kind === "series" && ep
                        ? `S${ep.seasonNumber} · E${ep.episodeNumber}`
                        : "FILM"}
                    </div>
                    <div className="ls-list__row-title">{content.title}</div>
                    {ep && <div className="ls-list__row-ep">{ep.title}</div>}
                    <div className="ls-list__row-meta mono">
                      {formatDuration(remaining)} remaining · {Math.round(ratio * 100)}% complete
                    </div>
                  </div>
                </Link>
                <button
                  type="button"
                  className="ls-list__row-remove"
                  aria-label="Remove from library"
                  onClick={() => remove(entry.contentId)}
                >
                  <X size={14} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
