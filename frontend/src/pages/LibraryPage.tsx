import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Play, X } from "lucide-react";
import { useAppStore } from "@/lib/store";
import { repository } from "@/lib/repository";
import { PageTrail } from "@/components/navigation/PageTrail";
import { formatDuration, clamp01 } from "@/lib/format";
import type { Film, Series } from "@/types";
import "./ListPage.css";

export function LibraryPage() {
  const library = useAppStore((s) => s.library);
  const continueWatching = library.continueWatching;
  const remove = useAppStore((s) => s.removeFromContinueWatching);
  const [contentById, setContentById] = useState<ReadonlyMap<string, Series | Film>>(new Map());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (continueWatching.length === 0) {
      setContentById(new Map());
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError(null);

    void Promise.all(
      continueWatching.map((entry) => repository.fetchContentById(entry.contentId, controller.signal)),
    )
      .then((items) => {
        const next = new Map<string, Series | Film>();
        for (const item of items) {
          if (item.kind === "series" || item.kind === "film") {
            next.set(item.id, item);
          }
        }
        setContentById(next);
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load library.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [continueWatching]);

  return (
    <div className="ls-list">
      <header className="ls-list__head">
        <PageTrail
          className="ls-list__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Library" },
          ]}
        />
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
      ) : loading ? (
        <div className="ls-list__empty">
          <Play size={24} strokeWidth={1.5} />
          <div>Loading library.</div>
        </div>
      ) : error ? (
        <div className="ls-list__empty">
          <Play size={24} strokeWidth={1.5} />
          <div>{error}</div>
        </div>
      ) : (
        <div className="ls-list__rows">
          {continueWatching.map((entry) => {
            const content = contentById.get(entry.contentId);
            if (!content) return null;
            const ratio = clamp01(entry.progressSec / entry.durationSec);
            const remaining = entry.durationSec - entry.progressSec;
            const href =
              entry.kind === "series" && entry.episodeId !== undefined
                ? `/watch/episode/${entry.episodeId}`
                : `/watch/film/${content.id}`;

            const ep =
              entry.episodeId !== undefined
                ? content.kind === "series"
                  ? content.seasons
                      .flatMap((season) => season.episodes)
                      .find((episode) => episode.id === entry.episodeId)
                  : undefined
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

      <section className="ls-list__section">
        <div className="ls-list__label mono">Membership access</div>
        {library.memberships.length === 0 ? (
          <div className="ls-list__empty">
            <Play size={24} strokeWidth={1.5} />
            <div>No active memberships.</div>
          </div>
        ) : (
          <div className="ls-list__rows">
            {library.memberships.map((membership) => (
              <div key={membership.creatorId} className="ls-list__row">
                <div className="ls-list__row-main ls-list__row-main--plain">
                  <div className="ls-list__row-body">
                    <div className="ls-list__row-kicker mono">{membership.status}</div>
                    <div className="ls-list__row-title">{membership.creatorDisplayName}</div>
                    <div className="ls-list__row-meta mono">
                      {membership.tierName}
                      {membership.renewsAt ? ` · renews ${membership.renewsAt}` : ""}
                      {membership.endsAt ? ` · ends ${membership.endsAt}` : ""}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="ls-list__section">
        <div className="ls-list__label mono">Purchased access</div>
        {library.purchases.length === 0 ? (
          <div className="ls-list__empty">
            <Play size={24} strokeWidth={1.5} />
            <div>No purchases yet.</div>
          </div>
        ) : (
          <div className="ls-list__rows">
            {library.purchases.map((purchase) => (
              <div key={purchase.id} className="ls-list__row">
                <div className="ls-list__row-main ls-list__row-main--plain">
                  <div className="ls-list__row-body">
                    <div className="ls-list__row-kicker mono">{purchase.status}</div>
                    <div className="ls-list__row-title">{purchase.title}</div>
                    <div className="ls-list__row-meta mono">
                      {purchase.creatorDisplayName}
                      {purchase.expiresAt ? ` · expires ${purchase.expiresAt}` : ""}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
