import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { ContentCard } from "@/components/content/ContentCard";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { Button } from "@/components/ui/Button";
import { repository } from "@/lib/repository";
import type { Film, Genre, Series } from "@/types";
import "./CatalogPage.css";

interface CatalogPageProps {
  readonly kind: "series" | "film" | "all";
  readonly originalsOnly?: boolean;
}

const genreOptions: ReadonlyArray<"All" | Genre> = [
  "All",
  "Drama",
  "Thriller",
  "Science Fiction",
  "Cinematic Tech",
  "Action",
  "Comedy",
  "Documentary",
  "Horror",
  "Crime",
  "Fantasy",
];

type SortKey = "trending" | "newest" | "score" | "title";
const PAGE_SIZE = 24;

function sortMixed(
  items: ReadonlyArray<Series | Film>,
  sort: SortKey,
): ReadonlyArray<Series | Film> {
  return items.slice().sort((a, b) => {
    if (sort === "newest") return b.year - a.year;
    if (sort === "score") return b.score - a.score;
    if (sort === "title") return a.title.localeCompare(b.title);
    return (
      Number(b.trending) - Number(a.trending) ||
      b.score - a.score ||
      b.year - a.year
    );
  });
}

export function CatalogPage({ kind, originalsOnly = false }: CatalogPageProps) {
  const [params] = useSearchParams();
  const initialGenre = genreOptions.includes(
    params.get("genre") as "All" | Genre,
  )
    ? (params.get("genre") as "All" | Genre)
    : "All";
  const [genre, setGenre] = useState<"All" | Genre>(initialGenre);
  const [sort, setSort] = useState<SortKey>("trending");
  const [originals, setOriginals] = useState(originalsOnly);
  const [results, setResults] = useState<ReadonlyArray<Series | Film>>([]);
  const [total, setTotal] = useState(0);
  const [seriesOffset, setSeriesOffset] = useState(0);
  const [filmOffset, setFilmOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setOriginals(originalsOnly);
  }, [originalsOnly]);

  useEffect(() => {
    setGenre(initialGenre);
  }, [initialGenre]);

  const loadCatalogPage = useCallback(
    async (
      nextSeriesOffset: number,
      nextFilmOffset: number,
      signal?: AbortSignal,
    ): Promise<{
      readonly items: ReadonlyArray<Series | Film>;
      readonly total: number;
      readonly seriesOffset: number;
      readonly filmOffset: number;
      readonly hasMore: boolean;
    }> => {
      const options = {
        genre,
        originalsOnly: originals,
        sort,
        limit: PAGE_SIZE,
      };

      if (kind === "series") {
        const page = await repository.fetchSeriesPage(
          { ...options, offset: nextSeriesOffset },
          signal,
        );
        return {
          items: page.items,
          total: page.total,
          seriesOffset: nextSeriesOffset + page.items.length,
          filmOffset: 0,
          hasMore: page.hasMore,
        };
      }

      if (kind === "film") {
        const page = await repository.fetchFilmsPage(
          { ...options, offset: nextFilmOffset },
          signal,
        );
        return {
          items: page.items,
          total: page.total,
          seriesOffset: 0,
          filmOffset: nextFilmOffset + page.items.length,
          hasMore: page.hasMore,
        };
      }

      const [seriesPage, filmsPage] = await Promise.all([
        repository.fetchSeriesPage(
          { ...options, originalsOnly: true, offset: nextSeriesOffset },
          signal,
        ),
        repository.fetchFilmsPage(
          { ...options, originalsOnly: true, offset: nextFilmOffset },
          signal,
        ),
      ]);
      return {
        items: [...seriesPage.items, ...filmsPage.items],
        total: seriesPage.total + filmsPage.total,
        seriesOffset: nextSeriesOffset + seriesPage.items.length,
        filmOffset: nextFilmOffset + filmsPage.items.length,
        hasMore: seriesPage.hasMore || filmsPage.hasMore,
      };
    },
    [genre, kind, originals, sort],
  );

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);

    void loadCatalogPage(0, 0, controller.signal)
      .then((page) => {
        setResults(sortMixed(page.items, sort));
        setTotal(page.total);
        setSeriesOffset(page.seriesOffset);
        setFilmOffset(page.filmOffset);
        setHasMore(page.hasMore);
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setResults([]);
          setTotal(0);
          setHasMore(false);
          setError(
            err instanceof Error ? err.message : "Unable to load this catalog.",
          );
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [loadCatalogPage, sort]);

  const loadMore = () => {
    setLoadingMore(true);
    setError(null);
    void loadCatalogPage(seriesOffset, filmOffset)
      .then((page) => {
        setResults((current) => sortMixed([...current, ...page.items], sort));
        setTotal(page.total);
        setSeriesOffset(page.seriesOffset);
        setFilmOffset(page.filmOffset);
        setHasMore(page.hasMore);
      })
      .catch((err) => {
        setError(
          err instanceof Error ? err.message : "Unable to load more titles.",
        );
      })
      .finally(() => setLoadingMore(false));
  };

  const pageTitle = originalsOnly
    ? "VANTA Originals"
    : kind === "series"
      ? "VANTA Series"
      : kind === "film"
        ? "VANTA Films"
        : "Browse VANTA";
  const pageDescription = originalsOnly
    ? "Browse VANTA Originals, premium exclusive long-form episodic content and films from creator-led studios."
    : kind === "series"
      ? "Browse premium long-form episodic series on VANTA, built for viewers, creators, and advertiser-ready attention."
      : kind === "film"
        ? "Browse premium films on VANTA from creator-led studios and cinematic publishers."
        : "Browse VANTA's premium exclusive catalog of series, films, originals, and creator-led content.";

  return (
    <div className="ls-catalog">
      <PageMetadata
        title={`${pageTitle} - Premium long-form streaming`}
        description={pageDescription}
        path={originalsOnly ? "/originals" : kind === "series" ? "/series" : kind === "film" ? "/films" : "/"}
        structuredData={{
          "@context": "https://schema.org",
          "@type": "CollectionPage",
          name: pageTitle,
          description: pageDescription,
          hasPart: results.slice(0, 12).map((item) => ({
            "@type": item.kind === "series" ? "TVSeries" : "Movie",
            name: item.title,
            description: item.synopsis,
            genre: item.genres,
            datePublished: String(item.year),
            image: item.images.poster,
          })),
        }}
      />
      <header className="ls-catalog__head">
        <div>
          <div className="ls-catalog__kicker mono">
            /{" "}
            {kind === "series"
              ? "series"
              : kind === "film"
                ? "films"
                : "browse"}
          </div>
          <h1 className="ls-catalog__title">
            {originalsOnly
              ? "VANTA Originals"
              : kind === "series"
                ? "All series"
                : kind === "film"
                  ? "All films"
                  : "Browse"}
          </h1>
          <p className="ls-catalog__sub">
            {loading ? "Loading titles" : `${results.length} of ${total}`} ·
            sorted by {sort}
          </p>
        </div>
        <div className="ls-catalog__filters">
          <div className="ls-catalog__group">
            <div className="ls-catalog__label mono">Genre</div>
            <div className="ls-catalog__chips">
              {genreOptions.map((g) => (
                <button
                  type="button"
                  key={g}
                  className={`ls-catalog__chip ${genre === g ? "is-active" : ""}`}
                  onClick={() => setGenre(g)}
                >
                  {g}
                </button>
              ))}
            </div>
          </div>
          <div className="ls-catalog__group">
            <div className="ls-catalog__label mono">Sort</div>
            <div className="ls-catalog__chips">
              {(["trending", "newest", "score", "title"] as const).map((s) => (
                <button
                  type="button"
                  key={s}
                  className={`ls-catalog__chip ${sort === s ? "is-active" : ""}`}
                  onClick={() => setSort(s)}
                >
                  {s === "title" ? "A–Z" : s}
                </button>
              ))}
            </div>
          </div>
          <div className="ls-catalog__group">
            <div className="ls-catalog__label mono">Only</div>
            <label className="ls-catalog__toggle">
              <input
                type="checkbox"
                checked={originals}
                onChange={(e) => setOriginals(e.target.checked)}
              />
              <span className="ls-catalog__toggle-track">
                <span className="ls-catalog__toggle-dot" />
              </span>
              <span>Originals</span>
            </label>
          </div>
        </div>
      </header>

      {error ? <div className="ls-catalog__state">{error}</div> : null}

      {loading && !error ? (
        <div className="ls-catalog__state">Loading titles…</div>
      ) : null}

      {!loading && !error && results.length === 0 ? (
        <div className="ls-catalog__state">No titles match these filters.</div>
      ) : null}

      {results.length > 0 ? (
        <div className="ls-catalog__grid">
          {results.map((item) => (
            <ContentCard key={item.id} item={item} layout="poster" />
          ))}
        </div>
      ) : null}

      {hasMore ? (
        <div className="ls-catalog__more">
          <Button variant="outline" onClick={loadMore} disabled={loadingMore}>
            {loadingMore ? "Loading" : "Load more"}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
