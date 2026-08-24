import { Link, useSearchParams } from "react-router-dom";
import { useState, useEffect } from "react";
import { Search as SearchIcon } from "lucide-react";
import { repository } from "@/lib/repository";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { usePageBreadcrumbs } from "@/components/layout/PageNavigation";
import type { SearchResult } from "@/types";
import "./SearchPage.css";

export function SearchPage() {
  const [params, setParams] = useSearchParams();
  const [query, setQuery] = useState(params.get("q") ?? "");
  const [results, setResults] = useState<ReadonlyArray<SearchResult>>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pageSize = 16;
  const currentQuery = params.get("q")?.trim();

  usePageBreadcrumbs([
    { label: "Dashboard", href: "/" },
    { label: "Search", href: currentQuery ? "/search" : undefined },
    ...(currentQuery ? [{ label: currentQuery }] : []),
  ]);

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
        setResults(payload.items);
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
        setResults((current) => [...current, ...payload.items]);
        setHasMore(payload.hasMore);
      })
      .catch((searchError) => {
        setError(searchError instanceof Error ? searchError.message : "Search failed.");
      })
      .finally(() => setLoadingMore(false));
  };

  const resultLabel = (kind: SearchResult["kind"]) => {
    switch (kind) {
      case "series":
        return "Series";
      case "film":
        return "Film";
      case "live":
        return "Live";
      case "episode":
        return "Episode";
      case "creator":
        return "Creator";
      case "profile":
        return "Profile";
      case "category":
        return "Category";
      default:
        return "Result";
    }
  };

  return (
    <div className="ls-search">
      <PageMetadata
        title={currentQuery ? `Search "${currentQuery}" - VANTA` : "Search VANTA"}
        description="Search VANTA's database-backed catalog of premium series, films, episodes, creators, live streams, categories, and metadata."
        path={currentQuery ? `/search?q=${encodeURIComponent(currentQuery)}` : "/search"}
        structuredData={{
          "@context": "https://schema.org",
          "@type": "SearchResultsPage",
          name: "VANTA Search",
          description:
            "Search VANTA's premium exclusive catalog of long-form episodes, films, creators, live streams, and metadata.",
          about: currentQuery ?? "VANTA catalog search",
          mainEntity: results.slice(0, 20).map((item) => ({
            "@type": "Thing",
            name: item.title,
            description: item.subtitle,
            url: `https://streamvanta.tv${item.href}`,
          })),
        }}
      />
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

      {loading && !error && results.length === 0 && (
        <div className="ls-search__empty">Searching the catalog…</div>
      )}

      {params.get("q") && !loading && !error && results.length === 0 && (
        <div className="ls-search__empty">
          No matches. Try a streamer name, a genre, or a one-word title.
        </div>
      )}

      {results.length > 0 && (
        <section className={`ls-search__section ${loading ? "is-refreshing" : ""}`}>
          <div className="ls-search__label mono">Top matches ({results.length})</div>
          <div className="ls-search__results">
            {results.map((item) => (
              <Link key={`${item.kind}-${item.id}`} className="ls-search__result" to={item.href}>
                {item.image ? (
                  <img className="ls-search__result-image" src={item.image} alt="" />
                ) : (
                  <span className="ls-search__result-image ls-search__result-image--empty">
                    {item.title.slice(0, 1)}
                  </span>
                )}
                <span className="ls-search__result-copy">
                  <span className="ls-search__result-title">{item.title}</span>
                  <span className="ls-search__result-subtitle">{item.subtitle}</span>
                </span>
                <span className="ls-search__result-kind mono">{resultLabel(item.kind)}</span>
              </Link>
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
