import { useAppStore } from "@/lib/store";
import { ContentCard } from "@/components/content/ContentCard";
import { Bookmark } from "lucide-react";
import "./ListPage.css";

export function WatchlistPage() {
  const watchlist = useAppStore((s) => s.watchlistDetails);
  const items = [...watchlist.series, ...watchlist.films];

  return (
    <div className="ls-list">
      <header className="ls-list__head">
        <div className="ls-list__kicker mono">/ yours / watchlist</div>
        <h1 className="ls-list__title">Watchlist</h1>
        <p className="ls-list__sub">
          {items.length} title{items.length === 1 ? "" : "s"} saved for later
        </p>
      </header>

      {items.length === 0 ? (
        <div className="ls-list__empty">
          <Bookmark size={24} strokeWidth={1.5} />
          <div>Nothing here yet.</div>
          <p>Tap the plus icon on any series or film to save it for later.</p>
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
