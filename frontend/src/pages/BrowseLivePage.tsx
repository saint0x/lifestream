import { useState } from "react";
import { Link } from "react-router-dom";
import { repository } from "@/lib/repository";
import { LiveCard } from "@/components/content/LiveCard";
import type { Genre } from "@/types";
import "./BrowseLivePage.css";

const allCategories: ReadonlyArray<"all" | Genre> = [
  "all",
  "Tech",
  "Gaming",
  "Music",
  "Talk",
  "Sports",
];

export function BrowseLivePage() {
  const [filter, setFilter] = useState<"all" | Genre>("all");
  const [sort, setSort] = useState<"viewers" | "newest">("viewers");

  const streams = repository.listLiveStreams();
  const filtered = streams
    .filter((s) => filter === "all" || s.category === filter)
    .slice()
    .sort((a, b) => {
      if (sort === "viewers") return b.viewers - a.viewers;
      return new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime();
    });

  const totalViewers = streams.reduce((sum, s) => sum + s.viewers, 0);

  return (
    <div className="ls-browse">
      <header className="ls-browse__head">
        <div>
          <div className="ls-browse__kicker mono">/ live</div>
          <h1 className="ls-browse__title">Live, right now</h1>
          <p className="ls-browse__sub">
            {streams.length} channels · {totalViewers.toLocaleString()} viewers ·
            zero latency, zero lag
          </p>
        </div>
        <div className="ls-browse__filters">
          <div className="ls-browse__group">
            <div className="ls-browse__label mono">Category</div>
            <div className="ls-browse__chips">
              {allCategories.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={`ls-browse__chip ${filter === c ? "is-active" : ""}`}
                  onClick={() => setFilter(c)}
                >
                  {c === "all" ? "All" : c}
                </button>
              ))}
            </div>
          </div>
          <div className="ls-browse__group">
            <div className="ls-browse__label mono">Sort by</div>
            <div className="ls-browse__chips">
              <button
                type="button"
                className={`ls-browse__chip ${sort === "viewers" ? "is-active" : ""}`}
                onClick={() => setSort("viewers")}
              >
                Viewers
              </button>
              <button
                type="button"
                className={`ls-browse__chip ${sort === "newest" ? "is-active" : ""}`}
                onClick={() => setSort("newest")}
              >
                Recently started
              </button>
            </div>
          </div>
        </div>
      </header>

      <section className="ls-browse__categories">
        <div className="ls-browse__label mono">Browse categories</div>
        <div className="ls-browse__cat-grid">
          {repository.listCategories().map((c) => (
            <Link
              key={c.slug}
              to={`/category/${c.slug}`}
              className="ls-browse__cat"
              style={{ backgroundImage: `url(${c.coverImage})` }}
            >
              <div className="ls-browse__cat-scrim" />
              <div className="ls-browse__cat-body">
                <div className="ls-browse__cat-name">{c.name}</div>
                <div className="ls-browse__cat-meta mono">
                  {c.liveViewers.toLocaleString()} viewers · {c.liveChannels} live
                </div>
              </div>
            </Link>
          ))}
        </div>
      </section>

      <section className="ls-browse__streams">
        <div className="ls-browse__label mono">
          {filtered.length} channel{filtered.length === 1 ? "" : "s"}
        </div>
        <div className="ls-browse__grid">
          {filtered.map((s) => (
            <LiveCard key={s.id} stream={s} />
          ))}
        </div>
      </section>
    </div>
  );
}
