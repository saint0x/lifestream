import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "@/lib/store";
import { ContentCard } from "@/components/content/ContentCard";
import { PageTrail } from "@/components/navigation/PageTrail";
import { Bookmark } from "lucide-react";
import { repository } from "@/lib/repository";
import type { Film, Series } from "@/types";
import "./ListPage.css";

export function WatchlistPage() {
  const watchlist = useAppStore((s) => s.watchlist);
  const watchlistIds = useMemo(() => Array.from(watchlist), [watchlist]);
  const watchlistKey = useMemo(() => watchlistIds.join("\n"), [watchlistIds]);
  const [items, setItems] = useState<ReadonlyArray<Series | Film>>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const ids = watchlistKey ? watchlistKey.split("\n") : [];
    if (ids.length === 0) {
      setItems([]);
      return;
    }

    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void Promise.all(
      ids.map((id) => repository.fetchContentById(id, controller.signal)),
    )
      .then((content) => {
        setItems(content.filter((item): item is Series | Film => item.kind === "series" || item.kind === "film"));
      })
      .catch((err) => {
        if (!controller.signal.aborted) {
          setError(err instanceof Error ? err.message : "Unable to load watchlist.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });

    return () => controller.abort();
  }, [watchlistKey]);

  return (
    <div className="ls-list">
      <header className="ls-list__head">
        <PageTrail
          className="ls-list__kicker mono"
          items={[
            { label: "Dashboard", href: "/" },
            { label: "Watchlist" },
          ]}
        />
        <h1 className="ls-list__title">Watchlist</h1>
        <p className="ls-list__sub">
          {items.length} title{items.length === 1 ? "" : "s"} saved for later
        </p>
      </header>

      {watchlistIds.length === 0 ? (
        <div className="ls-list__empty">
          <Bookmark size={24} strokeWidth={1.5} />
          <div>Nothing here yet.</div>
          <p>Tap the plus icon on any series or film to save it for later.</p>
        </div>
      ) : loading ? (
        <div className="ls-list__empty">
          <Bookmark size={24} strokeWidth={1.5} />
          <div>Loading watchlist.</div>
        </div>
      ) : error ? (
        <div className="ls-list__empty">
          <Bookmark size={24} strokeWidth={1.5} />
          <div>{error}</div>
        </div>
      ) : (
        <div className="ls-list__grid">
          {items.map((item) => (
            <ContentCard key={item!.id} item={item!} layout="poster" />
          ))}
        </div>
      )}
    </div>
  );
}
