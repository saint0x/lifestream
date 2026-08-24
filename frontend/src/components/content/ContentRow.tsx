import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, ArrowRight } from "lucide-react";
import { clsx } from "clsx";
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
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const updateScrollState = useCallback(() => {
    const el = scrollerRef.current;
    if (!el) return;

    const firstCard = el.querySelector<HTMLElement>(".ls-card");
    const styles = window.getComputedStyle(el);
    const gap = Number.parseFloat(styles.columnGap || styles.gap) || 0;
    const cardWidth = firstCard?.getBoundingClientRect().width ?? 0;
    const visibleCardCount =
      cardWidth > 0 ? Math.max(1, Math.floor((el.clientWidth + gap) / (cardWidth + gap))) : items.length;
    const hasOverflow = el.scrollWidth - el.clientWidth > 2 || items.length > visibleCardCount;
    const atStart = el.scrollLeft <= 2;
    const atEnd = el.scrollLeft + el.clientWidth >= el.scrollWidth - 2;

    setCanScrollLeft(hasOverflow && !atStart);
    setCanScrollRight(hasOverflow && (!atEnd || atStart));
  }, [items.length]);

  useEffect(() => {
    const el = scrollerRef.current;
    if (!el) return;

    const frames = [
      window.requestAnimationFrame(updateScrollState),
      window.requestAnimationFrame(() => window.requestAnimationFrame(updateScrollState)),
    ];
    const settleTimer = window.setTimeout(updateScrollState, 180);
    const resizeObserver = new ResizeObserver(updateScrollState);
    const mutationObserver = new MutationObserver(updateScrollState);
    resizeObserver.observe(el);
    Array.from(el.children).forEach((child) => resizeObserver.observe(child));
    mutationObserver.observe(el, { childList: true, subtree: true });
    const onResize = () => updateScrollState();
    window.addEventListener("resize", onResize);
    return () => {
      frames.forEach((frame) => window.cancelAnimationFrame(frame));
      window.clearTimeout(settleTimer);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      window.removeEventListener("resize", onResize);
    };
  }, [items.length, layout, updateScrollState]);

  const scroll = (dir: 1 | -1) => {
    const el = scrollerRef.current;
    if (!el) return;
    el.scrollBy({
      left: dir * Math.max(el.clientWidth * 0.86, 320),
      behavior: "smooth",
    });
  };

  if (items.length === 0) return null;

  return (
    <section className={clsx("ls-row", `ls-row--${layout}`)}>
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
        </div>
      </header>
      <div className="ls-row__rail">
        <div
          className={clsx("ls-row__fade ls-row__fade--left", canScrollLeft && "is-visible")}
          aria-hidden="true"
        />
        <div
          className={clsx("ls-row__fade ls-row__fade--right", canScrollRight && "is-visible")}
          aria-hidden="true"
        />
        <button
          type="button"
          className={clsx("ls-row__side-arrow ls-row__side-arrow--left", canScrollLeft && "is-visible")}
          onClick={() => scroll(-1)}
          disabled={!canScrollLeft}
          aria-label="Scroll left"
        >
          <ChevronLeft size={18} />
        </button>
        <button
          type="button"
          className={clsx("ls-row__side-arrow ls-row__side-arrow--right", canScrollRight && "is-visible")}
          onClick={() => scroll(1)}
          disabled={!canScrollRight}
          aria-label="Scroll right"
        >
          <ChevronRight size={18} />
        </button>
        <div className="ls-row__scroller scroll-x" ref={scrollerRef} onScroll={updateScrollState}>
          {items.map((item) => (
            <ContentCard
              key={item.id}
              item={item}
              layout={layout}
              showProgress={progressById?.[item.id]}
            />
          ))}
        </div>
      </div>
    </section>
  );
}
