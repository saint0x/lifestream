import { useEffect, useMemo, useState } from "react";
import { ArrowRight, Play } from "lucide-react";
import {
  appHref,
  fetchHomeCatalog,
  VANTA_APP_BASE_URL,
  type CatalogItem,
  type HomeCatalog,
  type LiveItem,
} from "@/lib/catalog";

type FeaturedItem = CatalogItem | LiveItem;

function itemImage(item: FeaturedItem): string {
  return item.kind === "live" ? item.thumbnail : item.images.backdrop || item.images.thumbnail;
}

function fallbackImage(item: FeaturedItem): string {
  return item.kind === "live" ? item.thumbnail : item.images.thumbnail || item.images.poster;
}

function itemMeta(item: FeaturedItem): string {
  if (item.kind === "live") return `${item.category} live now`;
  return [item.rating, String(item.year), item.genres[0]].filter(Boolean).join(" / ");
}

function itemCopy(item: FeaturedItem): string {
  if (item.kind === "live") return `${item.streamer.displayName} is streaming now.`;
  return item.tagline || item.synopsis;
}

export function HomeLanding() {
  const [catalog, setCatalog] = useState<HomeCatalog | null>(null);
  const [error, setError] = useState("");
  const [active, setActive] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    void fetchHomeCatalog(controller.signal)
      .then((nextCatalog) => {
        setCatalog(nextCatalog);
        setError("");
      })
      .catch(() => setError("Shows are loading slowly. Open Vanta to watch now."));
    return () => controller.abort();
  }, []);

  const featured = useMemo<readonly FeaturedItem[]>(
    () => [...(catalog?.trendingSeries ?? []), ...(catalog?.trendingFilms ?? []), ...(catalog?.featuredLive ?? [])].slice(0, 8),
    [catalog],
  );
  const current = featured[active % Math.max(featured.length, 1)];

  useEffect(() => {
    if (featured.length <= 1) return;
    const timer = window.setInterval(() => setActive((index) => (index + 1) % featured.length), 6500);
    return () => window.clearInterval(timer);
  }, [featured.length]);

  document.title = "Vanta | Stream creator shows free";

  return (
    <main className="vl-page vl-page--home">
      <header className="vl-nav vl-nav--home">
        <a className="vl-mark" href="/">
          <span className="vl-mark__symbol" aria-hidden="true" />
          <span className="vl-mark__text">VANTA</span>
        </a>
        <a className="vl-watch-link" href={VANTA_APP_BASE_URL}>
          Watch now
        </a>
      </header>

      <section className={current ? "vl-stream-hero" : "vl-stream-hero is-empty"} aria-label="Featured Vanta programming">
        {featured.map((item, index) => (
          <img
            className={index === active % featured.length ? "is-active" : undefined}
            key={item.id}
            src={itemImage(item)}
            alt=""
            onError={(event) => {
              const nextSrc = fallbackImage(item);
              if (event.currentTarget.src !== nextSrc) event.currentTarget.src = nextSrc;
            }}
          />
        ))}
        <div className="vl-stream-hero__shade" />
        <div className="vl-stream-hero__copy">
          <span className="vl-label">Stream free</span>
          <h1>{current ? current.title : "Exclusive creator shows."}</h1>
          <p>{current ? itemCopy(current) : error || "Stream the best exclusive creator content for free."}</p>
          {current ? (
            <div className="vl-stream-hero__meta">
              <span>{itemMeta(current)}</span>
              <span>Exclusive on Vanta</span>
            </div>
          ) : null}
          <a className="vl-button vl-button--primary" href={current ? appHref(current) : VANTA_APP_BASE_URL}>
            <Play size={16} fill="currentColor" />
            Watch now
          </a>
        </div>
        <div className="vl-stream-hero__thumbs" aria-label="Featured title selector">
          {featured.slice(0, 6).map((item, index) => (
            <button
              className={index === active % featured.length ? "is-active" : undefined}
              key={item.id}
              onClick={() => setActive(index)}
              type="button"
            >
              <img
                src={fallbackImage(item)}
                alt=""
                onError={(event) => {
                  event.currentTarget.style.visibility = "hidden";
                }}
              />
              <span>{item.title}</span>
            </button>
          ))}
        </div>
      </section>

      <div className="vl-home-depth">
        <section className="vl-home-promise">
          <span>Exclusive creator shows.</span>
          <span>Live premieres.</span>
          <span>Free to watch.</span>
        </section>

        <section className="vl-home-final">
          <h2>Stream the best exclusive creator content for free.</h2>
          <a className="vl-button vl-button--primary" href={VANTA_APP_BASE_URL}>
            <ArrowRight size={16} />
            Watch now
          </a>
        </section>
      </div>
    </main>
  );
}
