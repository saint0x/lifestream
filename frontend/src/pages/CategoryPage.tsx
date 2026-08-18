import { useParams, Navigate } from "react-router-dom";
import { repository } from "@/lib/repository";
import { LiveCard } from "@/components/content/LiveCard";
import { ContentCard } from "@/components/content/ContentCard";
import type { Genre } from "@/types";
import "./CategoryPage.css";

const toGenre = (slug: string): Genre | undefined => {
  const match = repository.listCategories().find((c) => c.slug === slug);
  return match?.name;
};

export function CategoryPage() {
  const { slug } = useParams<{ slug: string }>();
  const cat = slug ? repository.getCategory(slug) : undefined;
  const genre = slug ? toGenre(slug) : undefined;

  if (!cat || !genre) return <Navigate to="/browse" replace />;

  const liveInCat = repository.getLiveStreamsByCategory(genre);
  const vodInCat = repository.listByGenre(genre);

  return (
    <div className="ls-category">
      <header
        className="ls-category__hero"
        style={{ backgroundImage: `url(${cat.coverImage})` }}
      >
        <div className="ls-category__scrim" />
        <div className="ls-category__body">
          <div className="ls-category__kicker mono">/ category / {cat.slug}</div>
          <h1 className="ls-category__title">{cat.name}</h1>
          <div className="ls-category__meta mono">
            <span>
              <strong>{cat.liveViewers.toLocaleString()}</strong> viewers
            </span>
            <span className="ls-category__sep">/</span>
            <span>
              <strong>{cat.liveChannels}</strong> live channels
            </span>
            <span className="ls-category__sep">/</span>
            <span>
              <strong>{vodInCat.length}</strong> on-demand titles
            </span>
          </div>
          <div className="ls-category__tags">
            {cat.tags.map((t) => (
              <span key={t} className="ls-category__tag mono">{t}</span>
            ))}
          </div>
        </div>
      </header>

      {liveInCat.length > 0 && (
        <section className="ls-category__section">
          <div className="ls-category__section-label mono">Live now</div>
          <div className="ls-category__live-grid">
            {liveInCat.map((s) => (
              <LiveCard key={s.id} stream={s} />
            ))}
          </div>
        </section>
      )}

      <section className="ls-category__section">
        <div className="ls-category__section-label mono">On-demand</div>
        <div className="ls-category__vod-grid">
          {vodInCat.length === 0 ? (
            <div className="ls-category__empty">
              No on-demand titles yet for {cat.name}.
            </div>
          ) : (
            vodInCat.map((item) => (
              <ContentCard key={item.id} item={item} layout="poster" />
            ))
          )}
        </div>
      </section>
    </div>
  );
}
