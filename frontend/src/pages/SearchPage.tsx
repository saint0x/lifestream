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

  useEffect(() => {
    setQuery(params.get("q") ?? "");
  }, [params]);

  const submit = () => {
    setParams(query ? { q: query } : {});
  };

  const results: ReadonlyArray<ContentItem> = query
    ? repository.search(query)
    : [];

  const liveResults = results.filter((r): r is Extract<ContentItem, { kind: "live" }> => r.kind === "live");
  const vodResults = results.filter((r) => r.kind !== "live");

  return (
    <div className="ls-search">
      <header className="ls-search__head">
        <div className="ls-search__kicker mono">/ search</div>
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
        {query && (
          <div className="ls-search__stats mono">
            {results.length} result{results.length === 1 ? "" : "s"} for
            <span className="ls-search__query">"{query}"</span>
          </div>
        )}
      </header>

      {query && results.length === 0 && (
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
    </div>
  );
}
