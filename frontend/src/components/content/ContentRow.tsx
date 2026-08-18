import { useRef } from "react";
import { ChevronLeft, ChevronRight, ArrowRight } from "lucide-react";
import type { ContentItem } from "@/types";
import { ContentCard } from "./ContentCard";
import { Link } from "react-router-dom";
import "./ContentRow.css";

interface ContentRowProps {
  readonly title: string;
  readonly kicker?: string;
  readonly items: ReadonlyArray<ContentItem>;
  readonly layout?: "poster" | "landscape" | "wide";
  readonly seeAllHref?: string;
  readonly progressById?: Readonly<Record<string, number>>;
}

export function ContentRow({
  title,
  kicker,
  items,
  layout = "poster",
  seeAllHref,
  progressById,
}: ContentRowProps) {
  const scrollerRef = useRef<HTMLDivElement>(null);

  const scroll = (dir: 1 | -1) => {
    const el = scrollerRef.current;
    if (!el) return;
    el.scrollBy({ left: dir * (el.clientWidth * 0.8), behavior: "smooth" });
  };

  if (items.length === 0) return null;

  return (
    <section className="ls-row">
      <header className="ls-row__head">
        <div className="ls-row__title-block">
          {kicker !== undefined && <div className="ls-row__kicker mono">{kicker}</div>}
          <h2 className="ls-row__title">{title}</h2>
        </div>
        <div className="ls-row__controls">
          {seeAllHref !== undefined && (
            <Link to={seeAllHref} className="ls-row__all mono">
              All <ArrowRight size={12} />
            </Link>
          )}
          <button
            type="button"
            className="ls-row__arrow"
            onClick={() => scroll(-1)}
            aria-label="Scroll left"
          >
            <ChevronLeft size={14} />
          </button>
          <button
            type="button"
            className="ls-row__arrow"
            onClick={() => scroll(1)}
            aria-label="Scroll right"
          >
            <ChevronRight size={14} />
          </button>
        </div>
      </header>
      <div className="ls-row__scroller scroll-x" ref={scrollerRef}>
        {items.map((item) => (
          <ContentCard
            key={item.id}
            item={item}
            layout={layout}
            showProgress={progressById?.[item.id]}
          />
        ))}
      </div>
    </section>
  );
}
