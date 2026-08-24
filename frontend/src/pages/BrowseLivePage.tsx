import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { repository } from "@/lib/repository";
import { LiveCard } from "@/components/content/LiveCard";
import { PageMetadata } from "@/components/seo/PageMetadata";
import { PageTrail } from "@/components/navigation/PageTrail";
import type { Category, Genre, LiveStream } from "@/types";
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
  const [streams, setStreams] = useState<ReadonlyArray<LiveStream>>([]);
  const [categories, setCategories] = useState<ReadonlyArray<Category>>([]);
  const [totalViewers, setTotalViewers] = useState(0);
  const [totalChannels, setTotalChannels] = useState(0);
  const [limit, setLimit] = useState(24);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void repository
      .fetchLiveDiscovery({ category: filter, sort, limit }, controller.signal)
      .then((payload) => {
        setStreams(payload.streams);
        setCategories(payload.categories);
        setTotalViewers(payload.totalViewers);
        setTotalChannels(payload.totalChannels);
      })
      .catch((err) => {
        if (controller.signal.aborted) return;
        setStreams([]);
        setCategories([]);
        setTotalViewers(0);
        setTotalChannels(0);
        setError(err instanceof Error ? err.message : "Unable to load live channels.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [filter, sort, limit]);

  useEffect(() => {
    setLimit(24);
  }, [filter, sort]);

  return (
    <div className="ls-browse">
      <PageMetadata
        title="Live creator streams - VANTA"
        description="Browse live creator streams on VANTA, including premium exclusive streams, active categories, and real-time audience inventory."
        path="/live"
        structuredData={{
          "@context": "https://schema.org",
          "@type": "CollectionPage",
          name: "Live creator streams",
          description:
            "Live creator streams on VANTA for viewers, creators, and advertiser-ready active audience inventory.",
          hasPart: streams.slice(0, 24).map((stream) => ({
            "@type": "BroadcastEvent",
            name: stream.title,
            description: stream.streamer.bio,
            startDate: stream.startedAt,
            isLiveBroadcast: true,
            image: stream.thumbnail,
            performer: {
              "@type": "Person",
              name: stream.streamer.displayName,
            },
          })),
        }}
      />
      <header className="ls-browse__head">
        <div>
          <PageTrail
            className="ls-browse__kicker mono"
            items={[
              { label: "Dashboard", href: "/" },
              { label: "Live" },
            ]}
          />
          <h1 className="ls-browse__title">Live, right now</h1>
          <p className="ls-browse__sub">
            {totalChannels} channels · {totalViewers.toLocaleString()} viewers ·
            live channels and active audiences
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
          {categories.map((c) => (
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
          {streams.length} channel{streams.length === 1 ? "" : "s"}
        </div>
        {loading ? <div className="ls-browse__state mono">Loading live channels…</div> : null}
        {error ? <div className="ls-browse__state ls-browse__state--error">{error}</div> : null}
        {!loading && !error && streams.length === 0 ? (
          <div className="ls-browse__state mono">No live channels match this view.</div>
        ) : null}
        <div className="ls-browse__grid">
          {streams.map((s) => (
            <LiveCard key={s.id} stream={s} />
          ))}
        </div>
        {!loading && !error && streams.length < totalChannels ? (
          <button
            type="button"
            className="ls-browse__load-more"
            onClick={() => setLimit((current) => current + 24)}
          >
            Load more
          </button>
        ) : null}
      </section>
    </div>
  );
}
