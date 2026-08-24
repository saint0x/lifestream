import { useEffect, useState } from "react";
import { useParams, Navigate } from "react-router-dom";
import { repository } from "@/lib/repository";
import { LiveCard } from "@/components/content/LiveCard";
import { ContentCard } from "@/components/content/ContentCard";
import type { Category, Film, LiveStream, Series } from "@/types";
import "./CategoryPage.css";

export function CategoryPage() {
  const { slug } = useParams<{ slug: string }>();
  const [cat, setCat] = useState<Category | null>(null);
  const [liveInCat, setLiveInCat] = useState<ReadonlyArray<LiveStream>>([]);
  const [vodInCat, setVodInCat] = useState<ReadonlyArray<Series | Film>>([]);
  const [totalVodTitles, setTotalVodTitles] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pageSize = 18;

  useEffect(() => {
    if (!slug) return;
    const controller = new AbortController();
    setLoading(true);
    setError(null);

    void repository
      .fetchCategoryBrowse(slug, { limit: pageSize, offset: 0 }, controller.signal)
      .then((payload) => {
        setCat(payload.category);
        setLiveInCat(payload.liveStreams);
        setVodInCat([...payload.series, ...payload.films]);
        setTotalVodTitles(payload.totalVodTitles);
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setCat(null);
          setError(err instanceof Error ? err.message : "Unable to load this category.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [slug]);

  const loadMore = () => {
    if (!slug || loadingMore) return;
    setLoadingMore(true);
    setError(null);
    void repository
      .fetchCategoryBrowse(slug, { limit: pageSize, offset: vodInCat.length })
      .then((payload) => {
        setVodInCat((current) => [...current, ...payload.series, ...payload.films]);
        setTotalVodTitles(payload.totalVodTitles);
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : "Unable to load more titles.");
      })
      .finally(() => setLoadingMore(false));
  };

  if (!slug) return <Navigate to="/live" replace />;
  if (!loading && !cat && !error) return <Navigate to="/live" replace />;

  if (!cat) {
    return (
      <div className="ls-category">
        <div className="ls-category__empty">
          {loading ? "Loading category…" : error ?? "Category is unavailable."}
        </div>
      </div>
    );
  }

  return (
    <div className="ls-category">
      <header
        className="ls-category__hero"
        style={{ backgroundImage: `url(${cat.coverImage})` }}
      >
        <div className="ls-category__scrim" />
        <div className="ls-category__body">
          <div className="ls-category__kicker mono">/ category / {cat.slug}</div>
          <h1 className="ls-category__title">{cat.name}</h1>
          <div className="ls-category__meta mono">
            <span>
              <strong>{cat.liveViewers.toLocaleString()}</strong> viewers
            </span>
            <span className="ls-category__sep">/</span>
            <span>
              <strong>{cat.liveChannels}</strong> live channels
            </span>
            <span className="ls-category__sep">/</span>
            <span>
              <strong>{totalVodTitles}</strong> on-demand titles
            </span>
          </div>
          <div className="ls-category__tags">
            {cat.tags.map((t) => (
              <span key={t} className="ls-category__tag mono">{t}</span>
            ))}
          </div>
        </div>
      </header>

      {liveInCat.length > 0 && (
        <section className="ls-category__section">
          <div className="ls-category__section-label mono">Live now</div>
          <div className="ls-category__live-grid">
            {liveInCat.map((s) => (
              <LiveCard key={s.id} stream={s} />
            ))}
          </div>
        </section>
      )}

      <section className="ls-category__section">
        <div className="ls-category__section-label mono">On-demand</div>
        <div className="ls-category__vod-grid">
          {vodInCat.length === 0 ? (
            <div className="ls-category__empty">
              No on-demand titles yet for {cat.name}.
            </div>
          ) : (
            vodInCat.map((item) => (
              <ContentCard key={item.id} item={item} layout="poster" />
            ))
          )}
        </div>
        {vodInCat.length < totalVodTitles ? (
          <button
            type="button"
            className="ls-category__load-more"
            onClick={loadMore}
            disabled={loadingMore}
          >
            {loadingMore ? "Loading" : "Load more"}
          </button>
        ) : null}
      </section>
    </div>
  );
}
