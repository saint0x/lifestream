import { useSearchParams } from "react-router-dom";
import { useState, useEffect } from "react";
import { Search as SearchIcon } from "lucide-react";
import { repository } from "@/lib/repository";
import { ContentCard } from "@/components/content/ContentCard";
import { LiveCard } from "@/components/content/LiveCard";
import type { ContentItem } from "@/types";
import "./SearchPage.css";

export function SearchPage() {
  const [params, setParams] = useSearchParams();
  const [query, setQuery] = useState(params.get("q") ?? "");
  const [results, setResults] = useState<ReadonlyArray<ContentItem>>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pageSize = 16;

  useEffect(() => {
    setQuery(params.get("q") ?? "");
  }, [params]);

  useEffect(() => {
    const q = params.get("q")?.trim() ?? "";
    if (!q) {
      setResults([]);
      setHasMore(false);
      setLoading(false);
      setError(null);
      return;
    }
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void repository
      .searchRemotePage(q, { limit: pageSize, offset: 0 }, controller.signal)
      .then((payload) => {
        setResults([...payload.series, ...payload.films, ...payload.liveStreams]);
        setHasMore(payload.hasMore);
      })
      .catch((searchError) => {
        if (!controller.signal.aborted) {
          setResults([]);
          setHasMore(false);
          setError(searchError instanceof Error ? searchError.message : "Search failed.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [params]);

  const submit = () => {
    setParams(query ? { q: query } : {});
  };

  const loadMore = () => {
    const q = params.get("q")?.trim() ?? "";
    if (!q || loadingMore) return;
    setLoadingMore(true);
    setError(null);
    void repository
      .searchRemotePage(q, { limit: pageSize, offset: results.length })
      .then((payload) => {
        setResults((current) => [
          ...current,
          ...payload.series,
          ...payload.films,
          ...payload.liveStreams,
        ]);
        setHasMore(payload.hasMore);
      })
      .catch((searchError) => {
        setError(searchError instanceof Error ? searchError.message : "Search failed.");
      })
      .finally(() => setLoadingMore(false));
  };

  const liveResults = results.filter((r): r is Extract<ContentItem, { kind: "live" }> => r.kind === "live");
  const vodResults = results.filter((r) => r.kind !== "live");

  return (
    <div className="ls-search">
      <header className="ls-search__head">
        <div className="ls-search__kicker mono">/ search</div>
        <h1 className="ls-search__title">Search</h1>
        <form
          className="ls-search__form"
          onSubmit={(e) => {
            e.preventDefault();
            submit();
          }}
        >
          <label className="ls-search__input">
            <SearchIcon size={18} strokeWidth={1.5} />
            <input
              value={query}
              autoFocus
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search titles, streamers, genres, tags…"
              aria-label="Search"
            />
          </label>
        </form>
        {params.get("q") && (
          <div className="ls-search__stats mono">
            {loading ? "Searching" : `${results.length} result${results.length === 1 ? "" : "s"}`} for
            <span className="ls-search__query">"{params.get("q")}"</span>
          </div>
        )}
      </header>

      {error && <div className="ls-search__empty">{error}</div>}

      {loading && !error && (
        <div className="ls-search__empty">Searching the catalog…</div>
      )}

      {params.get("q") && !loading && !error && results.length === 0 && (
        <div className="ls-search__empty">
          No matches. Try a streamer name, a genre, or a one-word title.
        </div>
      )}

      {liveResults.length > 0 && (
        <section className="ls-search__section">
          <div className="ls-search__label mono">Live ({liveResults.length})</div>
          <div className="ls-search__live-grid">
            {liveResults.map((s) => (
              <LiveCard key={s.id} stream={s} />
            ))}
          </div>
        </section>
      )}

      {vodResults.length > 0 && (
        <section className="ls-search__section">
          <div className="ls-search__label mono">
            On-demand ({vodResults.length})
          </div>
          <div className="ls-search__vod-grid">
            {vodResults.map((item) => (
              <ContentCard key={item.id} item={item} layout="poster" />
            ))}
          </div>
        </section>
      )}

      {hasMore && !loading && !error ? (
        <button
          type="button"
          className="ls-search__load-more"
          onClick={loadMore}
          disabled={loadingMore}
        >
          {loadingMore ? "Loading" : "Load more"}
        </button>
      ) : null}
    </div>
  );
}
