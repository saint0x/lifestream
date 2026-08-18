import { useState, useMemo } from "react";
import { ContentCard } from "@/components/content/ContentCard";
import { repository } from "@/lib/repository";
import type { Film, Genre, Series } from "@/types";
import "./CatalogPage.css";

interface CatalogPageProps {
  readonly kind: "series" | "film";
}

const genreOptions: ReadonlyArray<"All" | Genre> = [
  "All",
  "Drama",
  "Thriller",
  "Sci-Fi",
  "Action",
  "Comedy",
  "Documentary",
  "Horror",
  "Crime",
  "Fantasy",
];

type SortKey = "trending" | "newest" | "score" | "title";

export function CatalogPage({ kind }: CatalogPageProps) {
  const [genre, setGenre] = useState<"All" | Genre>("All");
  const [sort, setSort] = useState<SortKey>("trending");
  const [originals, setOriginals] = useState(false);

  const source: ReadonlyArray<Series | Film> =
    kind === "series" ? repository.listSeries() : repository.listFilms();

  const results = useMemo(() => {
    const filtered = source.filter((c) => {
      if (genre !== "All" && !c.genres.includes(genre)) return false;
      if (originals && !c.isOriginal) return false;
      return true;
    });
    const sorted = filtered.slice().sort((a, b) => {
      if (sort === "newest") return b.year - a.year;
      if (sort === "score") return b.score - a.score;
      if (sort === "title") return a.title.localeCompare(b.title);
      return Number(b.trending) - Number(a.trending);
    });
    return sorted;
  }, [source, genre, sort, originals]);

  return (
    <div className="ls-catalog">
      <header className="ls-catalog__head">
        <div>
          <div className="ls-catalog__kicker mono">
            / {kind === "series" ? "series" : "films"}
          </div>
          <h1 className="ls-catalog__title">
            {kind === "series" ? "All series" : "All films"}
          </h1>
          <p className="ls-catalog__sub">
            {results.length} of {source.length} · sorted by {sort}
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

      <div className="ls-catalog__grid">
        {results.map((item) => (
          <ContentCard key={item.id} item={item} layout="poster" />
        ))}
      </div>
    </div>
  );
}
